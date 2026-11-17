use std::collections::HashMap;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::console::{ConsoleKind, ConsoleLine};

const MAX_TOOLS: usize = 100;
const TEXT_MAX_CHARS: usize = 300;
const MIN_CLOCK_THINKING_MS: u64 = 1000;
const MAX_DIFF_LINES: usize = 2000;
const MAX_DIFF_CHARS: usize = 96_000;
const MAX_LCS_CELLS: usize = 200_000;
const MAX_PATCH_HUNKS: usize = 40;
const MAX_PATCH_HUNK_LINES: usize = 400;
const ASSISTANT_KIND: &str = "assistant";
const USER_KIND: &str = "user";
const THINKING_BLOCKS: [&str; 2] = ["thinking", "redacted_thinking"];
const TOOL_USE_BLOCK: &str = "tool_use";
const TOOL_RESULT_BLOCK: &str = "tool_result";
const MCP_PREFIX: &str = "mcp__";
const PATCH_ADDED: char = '+';
const PATCH_REMOVED: char = '-';

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCategory {
    Read,
    Edit,
    Command,
    Search,
    List,
    Web,
    Mcp,
    Agent,
    Plan,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Running,
    Ok,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolActivity {
    pub call_id: String,
    pub name: String,
    pub category: ToolCategory,
    pub target: String,
    pub status: ToolStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub added: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removed: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnActivity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thinking_ms: Option<u64>,
    pub tools: Vec<ToolActivity>,
}

struct OpenTool {
    index: usize,
    started_at: Instant,
    patchable: bool,
}

pub(super) struct ActivityParser {
    last_line_at: Instant,
    thinking_ms: Option<u64>,
    tools: Vec<ToolActivity>,
    open: HashMap<String, OpenTool>,
}

impl ActivityParser {
    pub(super) fn new(now: Instant) -> Self {
        Self {
            last_line_at: now,
            thinking_ms: None,
            tools: Vec::new(),
            open: HashMap::new(),
        }
    }

    pub(super) fn push_line(&mut self, line: &[u8], now: Instant) {
        let since_last_line = now.saturating_duration_since(self.last_line_at);
        self.last_line_at = now;
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            return;
        };
        match value["type"].as_str() {
            Some(ASSISTANT_KIND) => self.assistant(&value, since_last_line.as_millis() as u64, now),
            Some(USER_KIND) => self.user(&value, now),
            _ => {}
        }
    }

    pub(super) fn finish(self) -> TurnActivity {
        TurnActivity {
            thinking_ms: self.thinking_ms,
            tools: self.tools,
        }
    }

    fn assistant(&mut self, line: &Value, since_last_line_ms: u64, now: Instant) {
        let Some(blocks) = line["message"]["content"].as_array() else {
            return;
        };
        let mut thought = false;
        for block in blocks {
            match block["type"].as_str() {
                Some(kind) if THINKING_BLOCKS.contains(&kind) => thought = true,
                Some(TOOL_USE_BLOCK) => self.start_tool(block, now),
                _ => {}
            }
        }
        if thought {
            let spent = line["thinking_duration_ms"]
                .as_u64()
                .unwrap_or_else(|| since_last_line_ms.max(MIN_CLOCK_THINKING_MS));
            self.thinking_ms = Some(self.thinking_ms.unwrap_or(0).saturating_add(spent));
        }
    }

    fn start_tool(&mut self, block: &Value, now: Instant) {
        let Some(call_id) = block["id"].as_str() else {
            return;
        };
        if self.tools.len() >= MAX_TOOLS || self.tools.iter().any(|tool| tool.call_id == call_id) {
            return;
        }
        let name = block["name"].as_str().unwrap_or_default();
        let input = &block["input"];
        let category = category_of(name);
        let (added, removed) = written_lines(name, input);
        self.open.insert(
            call_id.to_string(),
            OpenTool {
                index: self.tools.len(),
                started_at: now,
                patchable: matches!(name.to_ascii_lowercase().as_str(), "edit" | "multiedit"),
            },
        );
        self.tools.push(ToolActivity {
            call_id: cut(call_id),
            name: clean(name),
            category,
            target: clean(target_of(name, input)),
            status: ToolStatus::Running,
            added,
            removed,
            duration_ms: None,
        });
    }

    fn user(&mut self, line: &Value, now: Instant) {
        let Some(blocks) = line["message"]["content"].as_array() else {
            return;
        };
        let results: Vec<&Value> = blocks
            .iter()
            .filter(|block| block["type"].as_str() == Some(TOOL_RESULT_BLOCK))
            .collect();
        // O structuredPatch fica na linha, não no bloco: só vale quando a linha tem um único resultado.
        let patch = match results.as_slice() {
            [_] => patch_counts(&line["tool_use_result"]["structuredPatch"]),
            _ => None,
        };
        for result in results {
            let Some(open) = result["tool_use_id"]
                .as_str()
                .and_then(|id| self.open.remove(id))
            else {
                continue;
            };
            let tool = &mut self.tools[open.index];
            tool.duration_ms =
                Some(now.saturating_duration_since(open.started_at).as_millis() as u64);
            if result["is_error"].as_bool() == Some(true) {
                tool.status = ToolStatus::Failed;
                tool.added = None;
                tool.removed = None;
                continue;
            }
            tool.status = ToolStatus::Ok;
            if let (true, Some((added, removed))) = (open.patchable, patch) {
                tool.added = Some(added);
                tool.removed = Some(removed);
            }
        }
    }
}

fn category_of(name: &str) -> ToolCategory {
    let lower = name.to_ascii_lowercase();
    if lower.starts_with(MCP_PREFIX) {
        return ToolCategory::Mcp;
    }
    match lower.as_str() {
        "read" => ToolCategory::Read,
        "edit" | "multiedit" | "write" | "notebookedit" => ToolCategory::Edit,
        "bash" => ToolCategory::Command,
        "grep" | "glob" => ToolCategory::Search,
        "ls" => ToolCategory::List,
        "webfetch" | "websearch" => ToolCategory::Web,
        "task" | "agent" => ToolCategory::Agent,
        "todowrite" => ToolCategory::Plan,
        _ => ToolCategory::Other,
    }
}

fn target_of<'a>(name: &str, input: &'a Value) -> &'a str {
    let field = match name.to_ascii_lowercase().as_str() {
        "read" | "edit" | "multiedit" | "write" => "file_path",
        "notebookedit" => "notebook_path",
        "bash" => "command",
        "grep" | "glob" => "pattern",
        "ls" => "path",
        "webfetch" => "url",
        "websearch" => "query",
        "task" | "agent" => "description",
        _ => return "",
    };
    input[field].as_str().unwrap_or_default()
}

fn cut(text: &str) -> String {
    text.chars().take(TEXT_MAX_CHARS).collect()
}

fn clean(text: &str) -> String {
    cut(&ConsoleLine::new(ConsoleKind::Text, text).text)
}

// Contagem na chegada do tool_use; o structuredPatch, quando vem no resultado, a substitui no Edit.
fn written_lines(name: &str, input: &Value) -> (Option<u32>, Option<u32>) {
    match name.to_ascii_lowercase().as_str() {
        "write" => match input["content"].as_str() {
            Some(content) => (Some(split_lines(content).len() as u32), Some(0)),
            None => (None, None),
        },
        "edit" => edit_counts(input).map_or((None, None), |(a, r)| (Some(a), Some(r))),
        "multiedit" => {
            let Some(edits) = input["edits"].as_array() else {
                return (None, None);
            };
            let (added, removed) = edits
                .iter()
                .filter_map(edit_counts)
                .fold((0, 0), |(a, r), (added, removed)| (a + added, r + removed));
            (Some(added), Some(removed))
        }
        _ => (None, None),
    }
}

fn edit_counts(input: &Value) -> Option<(u32, u32)> {
    let old = input["old_string"].as_str()?;
    let new = input["new_string"].as_str()?;
    Some(diff_counts(old, new))
}

fn patch_counts(patch: &Value) -> Option<(u32, u32)> {
    let hunks = patch.as_array()?;
    let mut added = 0;
    let mut removed = 0;
    for hunk in hunks.iter().take(MAX_PATCH_HUNKS) {
        let Some(lines) = hunk["lines"].as_array() else {
            continue;
        };
        for line in lines.iter().take(MAX_PATCH_HUNK_LINES) {
            match line.as_str().and_then(|text| text.chars().next()) {
                Some(PATCH_ADDED) => added += 1,
                Some(PATCH_REMOVED) => removed += 1,
                _ => {}
            }
        }
    }
    Some((added, removed))
}

fn split_lines(text: &str) -> Vec<&str> {
    let end = text
        .char_indices()
        .nth(MAX_DIFF_CHARS)
        .map_or(text.len(), |(index, _)| index);
    let mut lines: Vec<&str> = text[..end]
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    lines.truncate(MAX_DIFF_LINES);
    lines
}

// Mesma conta do Orca: linhas fora da subsequência comum mais longa; acima do teto de células, só prefixo e sufixo.
fn diff_counts(old: &str, new: &str) -> (u32, u32) {
    let old_lines = split_lines(old);
    let new_lines = split_lines(new);
    let prefix = old_lines
        .iter()
        .zip(&new_lines)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = old_lines[prefix..]
        .iter()
        .rev()
        .zip(new_lines[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old_mid = &old_lines[prefix..old_lines.len() - suffix];
    let new_mid = &new_lines[prefix..new_lines.len() - suffix];
    if old_mid.len() * new_mid.len() > MAX_LCS_CELLS {
        return (new_mid.len() as u32, old_mid.len() as u32);
    }
    let common = lcs_length(old_mid, new_mid);
    (
        (new_mid.len() - common) as u32,
        (old_mid.len() - common) as u32,
    )
}

fn lcs_length(old: &[&str], new: &[&str]) -> usize {
    let mut previous = vec![0usize; new.len() + 1];
    let mut current = vec![0usize; new.len() + 1];
    for old_line in old {
        for (column, new_line) in new.iter().enumerate() {
            current[column + 1] = if old_line == new_line {
                previous[column] + 1
            } else {
                current[column].max(previous[column + 1])
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[new.len()]
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;

    const REAL_SAMPLE: &str = include_str!("../../../e2e/fixtures/claude-stream-real.jsonl");
    const REAL_FIRST_THINKING_MS: u64 = 355;
    const REAL_SECOND_THINKING_MS: u64 = 189;

    struct Feed {
        parser: ActivityParser,
        base: Instant,
        elapsed_ms: u64,
    }

    impl Feed {
        fn new() -> Feed {
            let base = Instant::now();
            Feed {
                parser: ActivityParser::new(base),
                base,
                elapsed_ms: 0,
            }
        }

        fn push_after(&mut self, gap_ms: u64, line: Value) {
            self.elapsed_ms += gap_ms;
            self.parser.push_line(
                line.to_string().as_bytes(),
                self.base + Duration::from_millis(self.elapsed_ms),
            );
        }

        fn push(&mut self, line: Value) {
            self.push_after(10, line);
        }

        fn finish(self) -> TurnActivity {
            self.parser.finish()
        }
    }

    fn tool_use(id: &str, name: &str, input: Value) -> Value {
        json!({ "type": "tool_use", "id": id, "name": name, "input": input })
    }

    fn assistant(blocks: Vec<Value>) -> Value {
        json!({ "type": "assistant", "message": { "content": blocks } })
    }

    fn result_block(id: &str, is_error: bool) -> Value {
        json!({ "type": "tool_result", "tool_use_id": id, "is_error": is_error, "content": "ok" })
    }

    fn user(blocks: Vec<Value>) -> Value {
        json!({ "type": "user", "message": { "content": blocks } })
    }

    fn user_with_patch(blocks: Vec<Value>, patch: Value) -> Value {
        json!({ "type": "user", "message": { "content": blocks }, "tool_use_result": { "structuredPatch": patch } })
    }

    fn run_tool(name: &str, input: Value) -> ToolActivity {
        let mut feed = Feed::new();
        feed.push(assistant(vec![tool_use("t1", name, input)]));
        feed.finish().tools.remove(0)
    }

    fn edit_counts_of(old: &str, new: &str) -> (Option<u32>, Option<u32>) {
        let tool = run_tool("Edit", json!({ "file_path": "a.txt", "old_string": old, "new_string": new }));
        (tool.added, tool.removed)
    }

    fn numbered(prefix: &str, from: usize, to: usize) -> Vec<String> {
        (from..to).map(|index| format!("{prefix}{index}")).collect()
    }

    #[test]
    fn categoria_vem_do_nome_da_ferramenta() {
        let expected = [
            ("Read", ToolCategory::Read),
            ("Edit", ToolCategory::Edit),
            ("MultiEdit", ToolCategory::Edit),
            ("Write", ToolCategory::Edit),
            ("Bash", ToolCategory::Command),
            ("Grep", ToolCategory::Search),
            ("Glob", ToolCategory::Search),
            ("LS", ToolCategory::List),
            ("WebFetch", ToolCategory::Web),
            ("WebSearch", ToolCategory::Web),
            ("Task", ToolCategory::Agent),
            ("TodoWrite", ToolCategory::Plan),
            ("mcp__servidor__ferramenta", ToolCategory::Mcp),
            ("FerramentaNova", ToolCategory::Other),
        ];

        let actual: Vec<(&str, ToolCategory)> = expected
            .iter()
            .map(|(name, _)| (*name, run_tool(name, json!({})).category))
            .collect();

        assert_eq!(actual, expected);
    }

    #[test]
    fn alvo_vem_do_campo_certo_de_cada_ferramenta() {
        let targets = [
            ("Read", json!({ "file_path": "src/a.rs" }), "src/a.rs"),
            ("Bash", json!({ "command": "git status" }), "git status"),
            ("Grep", json!({ "pattern": "TODO" }), "TODO"),
            ("LS", json!({ "path": "src" }), "src"),
            ("WebFetch", json!({ "url": "https://exemplo.test" }), "https://exemplo.test"),
            ("WebSearch", json!({ "query": "rust lcs" }), "rust lcs"),
            ("Task", json!({ "description": "revisar" }), "revisar"),
            ("FerramentaNova", json!({ "file_path": "x" }), ""),
        ];

        for (name, input, expected) in targets {
            assert_eq!(run_tool(name, input).target, expected, "ferramenta {name}");
        }
    }

    #[test]
    fn edit_com_substituicao_conta_uma_adicionada_e_uma_removida() {
        assert_eq!(edit_counts_of("a\nb\nc", "a\nX\nc"), (Some(1), Some(1)));
    }

    #[test]
    fn edit_so_com_adicao_nao_remove_nada() {
        assert_eq!(edit_counts_of("a", "a\nb\nc"), (Some(2), Some(0)));
    }

    #[test]
    fn edit_so_com_remocao_nao_adiciona_nada() {
        assert_eq!(edit_counts_of("a\nb\nc", "a"), (Some(0), Some(2)));
    }

    #[test]
    fn edit_com_linhas_iguais_conta_zero() {
        assert_eq!(edit_counts_of("a\nb", "a\nb"), (Some(0), Some(0)));
    }

    #[test]
    fn edit_com_fim_de_linha_crlf_conta_igual_ao_lf() {
        assert_eq!(edit_counts_of("a\r\nb\r\nc", "a\nX\nc"), (Some(1), Some(1)));
        assert_eq!(edit_counts_of("a\r\nb\r\n", "a\nb\n"), (Some(0), Some(0)));
    }

    #[test]
    fn edit_sem_new_string_nao_inventa_contagem() {
        let tool = run_tool("Edit", json!({ "file_path": "a.txt", "old_string": "a" }));

        assert_eq!((tool.added, tool.removed), (None, None));
    }

    #[test]
    fn multiedit_soma_as_edicoes() {
        let tool = run_tool(
            "MultiEdit",
            json!({ "file_path": "a.txt", "edits": [
                { "old_string": "a", "new_string": "b" },
                { "old_string": "x", "new_string": "x\ny\nz" },
            ] }),
        );

        assert_eq!((tool.added, tool.removed), (Some(3), Some(1)));
    }

    #[test]
    fn arquivo_acima_do_teto_usa_so_prefixo_e_sufixo_e_abaixo_dele_usa_lcs() {
        let shared = numbered("c", 1, 500);
        let big_old = [vec!["P".to_string(), "A".to_string()], shared.clone(), vec!["S".to_string()]].concat();
        let big_new = [vec!["P".to_string()], shared.clone(), vec!["B".to_string(), "S".to_string()]].concat();
        let small_shared = numbered("c", 1, 100);
        let small_old = [vec!["P".to_string(), "A".to_string()], small_shared.clone(), vec!["S".to_string()]].concat();
        let small_new = [vec!["P".to_string()], small_shared, vec!["B".to_string(), "S".to_string()]].concat();

        let big = edit_counts_of(&big_old.join("\n"), &big_new.join("\n"));
        let small = edit_counts_of(&small_old.join("\n"), &small_new.join("\n"));

        assert_eq!(small, (Some(1), Some(1)), "abaixo do teto a LCS acha as linhas comuns");
        assert_eq!(big, (Some(500), Some(500)), "acima do teto só prefixo e sufixo são descontados");
    }

    #[test]
    fn write_novo_e_sobrescrita_contam_todas_as_linhas_como_adicionadas() {
        let new_file = run_tool("Write", json!({ "file_path": "a.txt", "content": "a\nb\nc" }));
        let overwrite = run_tool("Write", json!({ "file_path": "a.txt", "content": "a\nb\nc\n" }));
        let crlf = run_tool("Write", json!({ "file_path": "a.txt", "content": "a\r\nb\r\n" }));

        assert_eq!((new_file.added, new_file.removed), (Some(3), Some(0)));
        assert_eq!((overwrite.added, overwrite.removed), (Some(3), Some(0)));
        assert_eq!((crlf.added, crlf.removed), (Some(2), Some(0)));
    }

    #[test]
    fn write_sem_content_nao_inventa_contagem() {
        let tool = run_tool("Write", json!({ "file_path": "a.txt" }));

        assert_eq!((tool.added, tool.removed), (None, None));
    }

    #[test]
    fn structured_patch_do_resultado_substitui_a_contagem_do_edit() {
        let mut feed = Feed::new();
        feed.push(assistant(vec![tool_use(
            "t1",
            "Edit",
            json!({ "file_path": "a.txt", "old_string": "a", "new_string": "b" }),
        )]));
        feed.push(user_with_patch(
            vec![result_block("t1", false)],
            json!([{ "lines": ["-x", "-y", "+z", " contexto"] }, { "lines": ["+w"] }]),
        ));

        let tool = feed.finish().tools.remove(0);

        assert_eq!((tool.status, tool.added, tool.removed), (ToolStatus::Ok, Some(2), Some(2)));
    }

    #[test]
    fn structured_patch_so_vale_para_edit_e_multiedit() {
        let mut feed = Feed::new();
        feed.push(assistant(vec![tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": "a\nb" }))]));
        feed.push(user_with_patch(vec![result_block("t1", false)], json!([{ "lines": ["+a", "+b", "+c", "+d"] }])));

        let tool = feed.finish().tools.remove(0);

        assert_eq!((tool.added, tool.removed), (Some(2), Some(0)));
    }

    #[test]
    fn linha_com_dois_resultados_ignora_o_structured_patch_que_nao_se_sabe_de_quem_e() {
        let mut feed = Feed::new();
        feed.push(assistant(vec![
            tool_use("t1", "Edit", json!({ "file_path": "a.txt", "old_string": "a", "new_string": "b" })),
            tool_use("t2", "Edit", json!({ "file_path": "b.txt", "old_string": "a", "new_string": "b\nc" })),
        ]));
        feed.push(user_with_patch(
            vec![result_block("t1", false), result_block("t2", false)],
            json!([{ "lines": ["+1", "+2", "+3", "+4", "+5"] }]),
        ));

        let tools = feed.finish().tools;

        assert_eq!(
            tools.iter().map(|tool| (tool.added, tool.removed)).collect::<Vec<_>>(),
            vec![(Some(1), Some(1)), (Some(2), Some(1))]
        );
    }

    #[test]
    fn chamadas_em_paralelo_e_resultados_fora_de_ordem_casam_pelo_id() {
        let mut feed = Feed::new();
        feed.push(assistant(vec![
            tool_use("t1", "Read", json!({ "file_path": "a.txt" })),
            tool_use("t2", "Bash", json!({ "command": "ls" })),
            tool_use("t3", "Grep", json!({ "pattern": "x" })),
        ]));
        feed.push_after(300, user(vec![result_block("t3", false), result_block("t1", true)]));
        feed.push_after(200, user(vec![result_block("t2", false)]));

        let tools = feed.finish().tools;

        assert_eq!(
            tools.iter().map(|tool| (tool.call_id.as_str(), tool.status)).collect::<Vec<_>>(),
            vec![("t1", ToolStatus::Failed), ("t2", ToolStatus::Ok), ("t3", ToolStatus::Ok)]
        );
        assert_eq!(
            tools.iter().map(|tool| tool.duration_ms).collect::<Vec<_>>(),
            vec![Some(300), Some(500), Some(300)]
        );
    }

    #[test]
    fn ferramenta_sem_resultado_fica_em_execucao_e_sem_duracao() {
        let mut feed = Feed::new();
        feed.push(assistant(vec![tool_use("t1", "Bash", json!({ "command": "sleep 99" }))]));

        let tool = feed.finish().tools.remove(0);

        assert_eq!((tool.status, tool.duration_ms), (ToolStatus::Running, None));
    }

    #[test]
    fn is_error_marca_a_ferramenta_como_falha_e_zera_a_contagem_do_edit() {
        let mut feed = Feed::new();
        feed.push(assistant(vec![tool_use(
            "t1",
            "Edit",
            json!({ "file_path": "a.txt", "old_string": "a", "new_string": "b" }),
        )]));
        feed.push(user_with_patch(vec![result_block("t1", true)], json!([{ "lines": ["+x"] }])));

        let tool = feed.finish().tools.remove(0);

        assert_eq!((tool.status, tool.added, tool.removed), (ToolStatus::Failed, None, None));
    }

    #[test]
    fn resultado_sem_tool_use_nao_derruba_nem_cria_ferramenta() {
        let mut feed = Feed::new();
        feed.push(user(vec![result_block("fantasma", false)]));
        feed.push(user(vec![result_block("fantasma", true)]));

        assert!(feed.finish().tools.is_empty());
    }

    #[test]
    fn resultado_repetido_do_mesmo_id_nao_reabre_a_ferramenta() {
        let mut feed = Feed::new();
        feed.push(assistant(vec![tool_use("t1", "Read", json!({ "file_path": "a.txt" }))]));
        feed.push(user(vec![result_block("t1", false)]));
        feed.push(user(vec![result_block("t1", true)]));

        assert_eq!(feed.finish().tools[0].status, ToolStatus::Ok);
    }

    #[test]
    fn linhas_malformadas_e_de_outros_tipos_sao_ignoradas() {
        let mut feed = Feed::new();
        feed.parser.push_line(b"isto nao e json", feed.base);
        feed.push(json!({ "type": "system", "subtype": "init" }));
        feed.push(json!({ "type": "assistant" }));
        feed.push(json!({ "type": "user", "message": { "content": "texto solto" } }));
        feed.push(json!({ "type": "result", "subtype": "success" }));

        assert_eq!(feed.finish(), TurnActivity::default());
    }

    #[test]
    fn limite_de_cem_ferramentas_descarta_as_seguintes_e_ids_repetidos() {
        let mut feed = Feed::new();
        let blocks: Vec<Value> = (0..105)
            .map(|index| tool_use(&format!("t{index}"), "Read", json!({ "file_path": "a.txt" })))
            .collect();
        feed.push(assistant(blocks));
        feed.push(assistant(vec![tool_use("t0", "Read", json!({ "file_path": "b.txt" }))]));
        feed.push(user(vec![result_block("t104", false), result_block("t99", false)]));

        let tools = feed.finish().tools;

        assert_eq!(tools.len(), MAX_TOOLS);
        assert_eq!(tools[0].target, "a.txt");
        assert_eq!(tools.last().map(|tool| tool.call_id.as_str()), Some("t99"));
        assert_eq!(tools.last().map(|tool| tool.status), Some(ToolStatus::Ok));
    }

    #[test]
    fn alvo_mascara_segredos_e_corta_em_300_caracteres() {
        let secret = run_tool("Bash", json!({ "command": "export GITHUB_TOKEN=ghp_e2eFakeToken1234567890abcdefABCDEF" }));
        let bearer = run_tool("Bash", json!({ "command": "curl -H 'Authorization: Bearer e2e-fake-bearer-123456' x" }));
        let long = run_tool("Bash", json!({ "command": "x".repeat(TEXT_MAX_CHARS + 100) }));

        assert!(secret.target.contains("[oculto]") && !secret.target.contains("ghp_e2e"), "{}", secret.target);
        assert!(bearer.target.contains("[oculto]") && !bearer.target.contains("e2e-fake-bearer"), "{}", bearer.target);
        assert_eq!(long.target.chars().count(), TEXT_MAX_CHARS);
    }

    #[test]
    fn pensamento_usa_a_duracao_da_linha_e_soma_os_blocos() {
        let mut feed = Feed::new();
        let thinking = json!({ "type": "assistant", "thinking_duration_ms": 355, "message": { "content": [{ "type": "thinking", "thinking": "" }] } });
        let redacted = json!({ "type": "assistant", "thinking_duration_ms": 45, "message": { "content": [{ "type": "redacted_thinking" }] } });
        feed.push_after(5000, thinking);
        feed.push(redacted);

        assert_eq!(feed.finish().thinking_ms, Some(400));
    }

    #[test]
    fn pensamento_sem_duracao_usa_o_relogio_com_minimo_de_um_segundo() {
        let blocks = || json!([{ "type": "thinking", "thinking": "" }]);
        let mut slow = Feed::new();
        slow.push_after(2500, json!({ "type": "assistant", "message": { "content": blocks() } }));
        let mut fast = Feed::new();
        fast.push_after(120, json!({ "type": "assistant", "message": { "content": blocks() } }));

        assert_eq!(slow.finish().thinking_ms, Some(2500));
        assert_eq!(fast.finish().thinking_ms, Some(MIN_CLOCK_THINKING_MS));
    }

    #[test]
    fn turno_sem_pensamento_nao_tem_thinking_ms() {
        let mut feed = Feed::new();
        feed.push(assistant(vec![json!({ "type": "text", "text": "oi" })]));

        assert_eq!(feed.finish().thinking_ms, None);
    }

    #[test]
    fn amostra_real_da_cli_vira_quatro_ferramentas_e_o_pensamento_medido() {
        let mut feed = Feed::new();
        for line in REAL_SAMPLE.lines() {
            feed.push(serde_json::from_str(line).expect("linha da amostra é JSON"));
        }

        let activity = feed.finish();

        let summary: Vec<(ToolCategory, ToolStatus, Option<u32>, Option<u32>)> = activity
            .tools
            .iter()
            .map(|tool| (tool.category, tool.status, tool.added, tool.removed))
            .collect();
        assert_eq!(
            summary,
            vec![
                (ToolCategory::Read, ToolStatus::Ok, None, None),
                (ToolCategory::Read, ToolStatus::Ok, None, None),
                (ToolCategory::Edit, ToolStatus::Ok, Some(1), Some(1)),
                (ToolCategory::Command, ToolStatus::Ok, None, None),
            ]
        );
        let targets: Vec<&str> = activity.tools.iter().map(|tool| tool.target.as_str()).collect();
        assert!(targets[0].ends_with("package.json"), "{targets:?}");
        assert!(targets[1].ends_with("teste.txt") && targets[2].ends_with("teste.txt"), "{targets:?}");
        assert_eq!(targets[3], "git status");
        assert_eq!(activity.thinking_ms, Some(REAL_FIRST_THINKING_MS + REAL_SECOND_THINKING_MS));
        assert!(activity.tools.iter().all(|tool| tool.duration_ms.is_some()));
    }
}
