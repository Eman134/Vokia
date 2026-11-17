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
const FOUND_NAMES_MAX: usize = 20;
const FIRST_LINE: u32 = 1;
const NAME_MAX_CHARS: usize = 120;

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

// Só o que a captura real confirmou: Read (offset/limit, file.totalLines), Glob (numFiles/filenames),
// TaskCreate (task.id) e TaskUpdate (taskId/status).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ToolDetail {
    Read { from: u32, to: u32 },
    Found { count: u32, names: Vec<String> },
    Created {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    Updated {
        #[serde(rename = "taskId")]
        task_id: String,
        status: String,
    },
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<ToolDetail>,
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

    // Devolve true quando a linha abriu ou fechou uma ferramenta: só aí a atividade ao vivo muda.
    pub(super) fn push_line(&mut self, line: &[u8], now: Instant) -> bool {
        let since_last_line = now.saturating_duration_since(self.last_line_at);
        self.last_line_at = now;
        let Ok(value) = serde_json::from_slice::<Value>(line) else {
            return false;
        };
        match value["type"].as_str() {
            Some(ASSISTANT_KIND) => self.assistant(&value, since_last_line.as_millis() as u64, now),
            Some(USER_KIND) => self.user(&value, now),
            _ => false,
        }
    }

    pub(super) fn snapshot(&self) -> TurnActivity {
        TurnActivity {
            thinking_ms: self.thinking_ms,
            tools: self.tools.clone(),
        }
    }

    pub(super) fn finish(self) -> TurnActivity {
        TurnActivity {
            thinking_ms: self.thinking_ms,
            tools: self.tools,
        }
    }

    fn assistant(&mut self, line: &Value, since_last_line_ms: u64, now: Instant) -> bool {
        let Some(blocks) = line["message"]["content"].as_array() else {
            return false;
        };
        let mut thought = false;
        let mut opened = false;
        for block in blocks {
            match block["type"].as_str() {
                Some(kind) if THINKING_BLOCKS.contains(&kind) => thought = true,
                Some(TOOL_USE_BLOCK) => opened |= self.start_tool(block, now),
                _ => {}
            }
        }
        if thought {
            let spent = line["thinking_duration_ms"]
                .as_u64()
                .unwrap_or_else(|| since_last_line_ms.max(MIN_CLOCK_THINKING_MS));
            self.thinking_ms = Some(self.thinking_ms.unwrap_or(0).saturating_add(spent));
        }
        opened
    }

    fn start_tool(&mut self, block: &Value, now: Instant) -> bool {
        let Some(call_id) = block["id"].as_str() else {
            return false;
        };
        if self.tools.len() >= MAX_TOOLS || self.tools.iter().any(|tool| tool.call_id == call_id) {
            return false;
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
            detail: entry_detail(name, input),
        });
        true
    }

    fn user(&mut self, line: &Value, now: Instant) -> bool {
        let Some(blocks) = line["message"]["content"].as_array() else {
            return false;
        };
        let results: Vec<&Value> = blocks
            .iter()
            .filter(|block| block["type"].as_str() == Some(TOOL_RESULT_BLOCK))
            .collect();
        // O structuredPatch e o tool_use_result ficam na linha, não no bloco: só valem com um único resultado.
        let outcome = match results.as_slice() {
            [_] => Some(&line["tool_use_result"]),
            _ => None,
        };
        let patch = outcome.and_then(|outcome| patch_counts(&outcome["structuredPatch"]));
        let mut closed = false;
        for result in results {
            let Some(open) = result["tool_use_id"]
                .as_str()
                .and_then(|id| self.open.remove(id))
            else {
                continue;
            };
            closed = true;
            let tool = &mut self.tools[open.index];
            tool.duration_ms =
                Some(now.saturating_duration_since(open.started_at).as_millis() as u64);
            if result["is_error"].as_bool() == Some(true) {
                tool.status = ToolStatus::Failed;
                tool.added = None;
                tool.removed = None;
                tool.detail = None;
                continue;
            }
            tool.status = ToolStatus::Ok;
            if let (true, Some((added, removed))) = (open.patchable, patch) {
                tool.added = Some(added);
                tool.removed = Some(removed);
            }
            tool.detail = capped_read(tool.detail.take(), outcome.and_then(total_lines));
            if let Some(detail) = outcome.and_then(|outcome| result_detail(&tool.name, outcome)) {
                tool.detail = Some(detail);
            }
        }
        closed
    }
}

fn entry_detail(name: &str, input: &Value) -> Option<ToolDetail> {
    match name.to_ascii_lowercase().as_str() {
        "read" => read_range(input),
        "taskcreate" => Some(ToolDetail::Created { id: None }),
        "taskupdate" => Some(ToolDetail::Updated {
            task_id: name_of(input["taskId"].as_str()?),
            status: name_of(input["status"].as_str()?),
        }),
        _ => None,
    }
}

// Sem limit não há fim do intervalo: a linha não mostra nada. Sem offset, a leitura começa na primeira linha.
fn read_range(input: &Value) -> Option<ToolDetail> {
    let limit = u32::try_from(input["limit"].as_u64().filter(|limit| *limit > 0)?).ok()?;
    let from = match input["offset"].as_u64() {
        Some(offset) => u32::try_from(offset).ok()?,
        None => FIRST_LINE,
    };
    Some(ToolDetail::Read {
        from,
        to: from.saturating_add(limit - 1),
    })
}

// O fim do intervalo não passa do total do arquivo; um offset além do fim não lê linha nenhuma.
fn capped_read(detail: Option<ToolDetail>, total: Option<u32>) -> Option<ToolDetail> {
    match (detail, total) {
        (Some(ToolDetail::Read { from, to }), Some(total)) => {
            (from <= total).then_some(ToolDetail::Read { from, to: to.min(total) })
        }
        (detail, _) => detail,
    }
}

fn total_lines(outcome: &Value) -> Option<u32> {
    u32::try_from(outcome["file"]["totalLines"].as_u64()?).ok()
}

fn result_detail(name: &str, outcome: &Value) -> Option<ToolDetail> {
    match name.to_ascii_lowercase().as_str() {
        "glob" => found(outcome),
        "taskcreate" => Some(ToolDetail::Created {
            id: outcome["task"]["id"].as_str().map(str::to_string),
        }),
        _ => None,
    }
}

fn found(outcome: &Value) -> Option<ToolDetail> {
    let count = u32::try_from(outcome["numFiles"].as_u64()?).ok()?;
    let names = outcome["filenames"]
        .as_array()?
        .iter()
        .take(FOUND_NAMES_MAX)
        .filter_map(Value::as_str)
        .map(name_of)
        .collect();
    Some(ToolDetail::Found { count, names })
}

fn name_of(text: &str) -> String {
    clean(text).chars().take(NAME_MAX_CHARS).collect()
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
        "todowrite" | "taskcreate" | "taskupdate" => ToolCategory::Plan,
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
        "taskcreate" => "subject",
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
#[path = "../../tests/activity_tests.rs"]
mod tests;
