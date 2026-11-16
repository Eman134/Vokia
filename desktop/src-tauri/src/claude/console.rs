use serde::Serialize;
use serde_json::Value;

const CONSOLE_TEXT_MAX_CHARS: usize = 4000;
const CUT_SUFFIX: &str = "… (cortado)";
const REDACTED: &str = "[oculto]";
const ASSISTANT_KIND: &str = "assistant";
const USER_KIND: &str = "user";
const TEXT_BLOCK: &str = "text";
const TOOL_USE_BLOCK: &str = "tool_use";
const TOOL_RESULT_BLOCK: &str = "tool_result";
const SECRET_PREFIXES: [&str; 11] = [
    "sk-ant-",
    "sk-",
    "ghp_",
    "gho_",
    "ghu_",
    "ghs_",
    "ghr_",
    "github_pat_",
    "xoxb-",
    "xoxp-",
    "AKIA",
];
const SECRET_MIN_LEN: usize = 16;
const SECRET_NAME_HINTS: [&str; 9] = [
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "API_KEY",
    "APIKEY",
    "PRIVATE_KEY",
    "CREDENTIAL",
    "AUTHORIZATION",
];
const BEARER: &str = "Bearer";
const WORD_EDGES: [char; 5] = ['"', '\'', '`', ',', ';'];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleKind {
    Start,
    Text,
    Tool,
    Output,
    Error,
    Permission,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleLine {
    pub kind: ConsoleKind,
    pub text: String,
}

impl ConsoleLine {
    pub fn new(kind: ConsoleKind, text: &str) -> Self {
        Self {
            kind,
            text: truncate(&redact(text)),
        }
    }
}

#[derive(Default)]
pub(super) struct ConsoleParser;

impl ConsoleParser {
    pub(super) fn lines(&mut self, line: &[u8]) -> Vec<ConsoleLine> {
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            return Vec::new();
        };
        let kind = value["type"].as_str().unwrap_or_default();
        let Some(blocks) = value["message"]["content"].as_array() else {
            return Vec::new();
        };
        match kind {
            ASSISTANT_KIND => assistant_lines(blocks),
            USER_KIND => user_lines(blocks),
            _ => Vec::new(),
        }
    }
}

fn assistant_lines(blocks: &[Value]) -> Vec<ConsoleLine> {
    let mut lines = Vec::new();
    for block in blocks {
        match block["type"].as_str() {
            Some(TEXT_BLOCK) => {
                if let Some(text) = block["text"]
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                {
                    lines.push(ConsoleLine::new(ConsoleKind::Text, text));
                }
            }
            Some(TOOL_USE_BLOCK) => {
                let name = block["name"].as_str().unwrap_or_default();
                lines.push(ConsoleLine::new(
                    ConsoleKind::Tool,
                    &tool_summary(name, &block["input"]),
                ));
                if let Some(body) = write_body(name, &block["input"]) {
                    lines.push(ConsoleLine::new(ConsoleKind::Output, &body));
                }
            }
            _ => {}
        }
    }
    lines
}

fn user_lines(blocks: &[Value]) -> Vec<ConsoleLine> {
    blocks
        .iter()
        .filter(|block| block["type"].as_str() == Some(TOOL_RESULT_BLOCK))
        .filter_map(|block| {
            let kind = if block["is_error"].as_bool() == Some(true) {
                ConsoleKind::Error
            } else {
                ConsoleKind::Output
            };
            let text = block_text(&block["content"]);
            if text.trim().is_empty() {
                return None;
            }
            Some(ConsoleLine::new(kind, &text))
        })
        .collect()
}

fn write_body(name: &str, input: &Value) -> Option<String> {
    let field = match name {
        "Write" => "content",
        "Edit" => "new_string",
        _ => return None,
    };
    input[field]
        .as_str()
        .filter(|body| !body.trim().is_empty())
        .map(str::to_string)
}

fn block_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub(super) fn tool_summary(name: &str, input: &Value) -> String {
    let field = match name {
        "Bash" => Some("command"),
        "Read" | "Write" | "Edit" => Some("file_path"),
        "Glob" | "Grep" => Some("pattern"),
        _ => None,
    };
    match field.and_then(|field| input[field].as_str()) {
        Some(value) => format!("{name} {value}"),
        None => name.to_string(),
    }
}

// Melhor esforço: segredos em formato desconhecido podem passar; o console não é persistido.
fn redact(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut hide_next = false;
    for piece in text.split_inclusive(char::is_whitespace) {
        let word = piece.trim_end();
        let tail = &piece[word.len()..];
        if word.is_empty() {
            out.push_str(piece);
            continue;
        }
        let core = word.trim_matches(WORD_EDGES);
        if hide_next {
            if core == BEARER {
                out.push_str(piece);
            } else {
                out.push_str(&word.replacen(core, REDACTED, 1));
                out.push_str(tail);
                hide_next = false;
            }
        } else if core == BEARER {
            out.push_str(piece);
            hide_next = true;
        } else if let Some(masked) = assignment_masked(word, core) {
            out.push_str(&masked);
            out.push_str(tail);
        } else if is_secret_label(core) {
            out.push_str(piece);
            hide_next = true;
        } else if let Some(masked) = prefixed_secret_masked(word, core) {
            out.push_str(&masked);
            out.push_str(tail);
        } else {
            out.push_str(piece);
        }
    }
    out
}

fn assignment_masked(word: &str, core: &str) -> Option<String> {
    let (name, value) = core.split_once('=')?;
    if value.is_empty() || !is_sensitive_name(name) {
        return None;
    }
    let index = word.rfind(value)?;
    Some(format!(
        "{}{REDACTED}{}",
        &word[..index],
        &word[index + value.len()..]
    ))
}

fn is_secret_label(core: &str) -> bool {
    core.ends_with(':') && is_sensitive_name(core.trim_end_matches(':').trim_matches(WORD_EDGES))
}

fn prefixed_secret_masked(word: &str, core: &str) -> Option<String> {
    let has_secret_prefix = SECRET_PREFIXES
        .iter()
        .any(|prefix| core.starts_with(*prefix));
    let is_secret = has_secret_prefix && core.chars().count() >= SECRET_MIN_LEN;
    is_secret.then(|| word.replacen(core, REDACTED, 1))
}

fn is_sensitive_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    SECRET_NAME_HINTS.iter().any(|hint| upper.contains(*hint))
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= CONSOLE_TEXT_MAX_CHARS {
        return text.to_string();
    }
    let keep = CONSOLE_TEXT_MAX_CHARS - CUT_SUFFIX.chars().count();
    let cut = text
        .char_indices()
        .nth(keep)
        .map_or(text.len(), |(index, _)| index);
    format!("{}{CUT_SUFFIX}", &text[..cut])
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(lines: &[serde_json::Value]) -> Vec<(ConsoleKind, String)> {
        let mut parser = ConsoleParser::default();
        lines
            .iter()
            .flat_map(|line| parser.lines(line.to_string().as_bytes()))
            .map(|line| (line.kind, line.text))
            .collect()
    }

    fn assistant(blocks: serde_json::Value) -> serde_json::Value {
        json!({ "type": "assistant", "message": { "content": blocks } })
    }

    fn user(blocks: serde_json::Value) -> serde_json::Value {
        json!({ "type": "user", "message": { "content": blocks } })
    }

    fn tool_use(id: &str, name: &str, input: serde_json::Value) -> serde_json::Value {
        json!({ "type": "tool_use", "id": id, "name": name, "input": input })
    }

    fn result(id: &str, content: &str, is_error: bool) -> serde_json::Value {
        json!({ "type": "tool_result", "tool_use_id": id, "is_error": is_error, "content": content })
    }

    #[test]
    fn conteudo_do_write_vem_logo_depois_da_linha_tool() {
        let lines = parse(&[assistant(json!([tool_use("t1", "Write", json!({ "file_path": "notas.txt", "content": "linha1\nlinha2" }))]))]);

        assert_eq!(
            lines,
            vec![
                (ConsoleKind::Tool, "Write notas.txt".to_string()),
                (ConsoleKind::Output, "linha1\nlinha2".to_string()),
            ]
        );
    }

    #[test]
    fn edit_multilinha_mostra_so_o_texto_novo_sem_o_antigo() {
        let lines = parse(&[assistant(json!([tool_use("t1", "Edit", json!({ "file_path": "notas.txt", "old_string": "a\nb", "new_string": "c\nd" }))]))]);

        assert_eq!(
            lines,
            vec![
                (ConsoleKind::Tool, "Edit notas.txt".to_string()),
                (ConsoleKind::Output, "c\nd".to_string()),
            ]
        );
    }

    #[test]
    fn segredos_no_conteudo_do_write_viram_oculto_e_o_texto_em_volta_continua() {
        let content = "API_KEY=ghp_abcdefghijklmnop1234 fim\nAuthorization: Bearer abc123def456ghi789 x\nsk-ant-api03-XXXXXXXXXXXXXXXX solto";
        let lines = parse(&[assistant(json!([tool_use("t1", "Write", json!({ "file_path": "config.txt", "content": content }))]))]);

        assert_eq!(
            lines[1],
            (
                ConsoleKind::Output,
                "API_KEY=[oculto] fim\nAuthorization: Bearer [oculto] x\n[oculto] solto".to_string()
            )
        );
    }

    #[test]
    fn segredo_no_texto_novo_do_edit_vira_oculto() {
        let lines = parse(&[assistant(json!([tool_use("t1", "Edit", json!({ "file_path": "config.txt", "old_string": "x", "new_string": "API_KEY=ghp_abcdefghijklmnop1234" }))]))]);

        assert_eq!(lines[1], (ConsoleKind::Output, "API_KEY=[oculto]".to_string()));
    }

    #[test]
    fn resultado_real_de_write_e_edit_aparece_e_erro_vira_error() {
        let ok = parse(&[user(json!([result("t1", "gravado com sucesso", false)]))]);
        let failed = parse(&[user(json!([result("t1", "arquivo bloqueado", true)]))]);

        assert_eq!(ok, vec![(ConsoleKind::Output, "gravado com sucesso".to_string())]);
        assert_eq!(failed, vec![(ConsoleKind::Error, "arquivo bloqueado".to_string())]);
    }

    #[test]
    fn conteudo_de_4001_caracteres_corta_e_de_4000_sai_inteiro() {
        let limit = CONSOLE_TEXT_MAX_CHARS;
        let exact = "a".repeat(limit);
        let over = "a".repeat(limit + 1);
        let exact_lines = parse(&[assistant(json!([tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": exact }))]))]);
        let over_lines = parse(&[assistant(json!([tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": over }))]))]);

        assert_eq!(exact_lines[1].1.chars().count(), limit);
        assert!(!exact_lines[1].1.ends_with(CUT_SUFFIX));
        assert_eq!(over_lines[1].1.chars().count(), limit);
        assert!(over_lines[1].1.ends_with(CUT_SUFFIX));
    }

    #[test]
    fn dois_tool_use_no_mesmo_bloco_mantem_cada_conteudo_com_sua_ferramenta() {
        let blocks = assistant(json!([
            tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": "CONTEUDO_W" })),
            tool_use("t2", "Edit", json!({ "file_path": "b.txt", "old_string": "ANT", "new_string": "NOV" })),
        ]));
        let results = user(json!([result("t2", "resultado-edit", false), result("t1", "resultado-write", false)]));

        let lines = parse(&[blocks, results]);

        assert_eq!(
            lines,
            vec![
                (ConsoleKind::Tool, "Write a.txt".to_string()),
                (ConsoleKind::Output, "CONTEUDO_W".to_string()),
                (ConsoleKind::Tool, "Edit b.txt".to_string()),
                (ConsoleKind::Output, "NOV".to_string()),
                (ConsoleKind::Output, "resultado-edit".to_string()),
                (ConsoleKind::Output, "resultado-write".to_string()),
            ]
        );
    }

    #[test]
    fn bloco_de_pensamento_no_mesmo_bloco_do_write_nao_gera_linha() {
        let lines = parse(&[assistant(json!([
            { "type": "thinking", "thinking": "PENSAMENTO_E2E" },
            tool_use("t1", "Write", json!({ "file_path": "a.txt", "content": "conteudo" })),
        ]))]);

        assert_eq!(
            lines,
            vec![
                (ConsoleKind::Tool, "Write a.txt".to_string()),
                (ConsoleKind::Output, "conteudo".to_string()),
            ]
        );
    }

    #[test]
    fn write_sem_content_e_edit_sem_new_string_geram_so_a_linha_tool() {
        let lines = parse(&[assistant(json!([
            tool_use("t1", "Write", json!({ "file_path": "a.txt" })),
            tool_use("t2", "Edit", json!({ "file_path": "b.txt", "old_string": "ANTIGO" })),
        ]))]);

        assert_eq!(
            lines,
            vec![
                (ConsoleKind::Tool, "Write a.txt".to_string()),
                (ConsoleKind::Tool, "Edit b.txt".to_string()),
            ]
        );
    }

    #[test]
    fn bash_read_e_grep_continuam_com_resumo_e_resultado_real() {
        let lines = parse(&[
            assistant(json!([
                tool_use("t1", "Bash", json!({ "command": "git status" })),
                tool_use("t2", "Read", json!({ "file_path": "a.txt" })),
                tool_use("t3", "Grep", json!({ "pattern": "TODO" })),
            ])),
            user(json!([result("t1", "On branch main", false)])),
        ]);

        assert_eq!(
            lines,
            vec![
                (ConsoleKind::Tool, "Bash git status".to_string()),
                (ConsoleKind::Tool, "Read a.txt".to_string()),
                (ConsoleKind::Tool, "Grep TODO".to_string()),
                (ConsoleKind::Output, "On branch main".to_string()),
            ]
        );
    }
}
