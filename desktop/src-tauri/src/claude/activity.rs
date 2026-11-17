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
#[path = "../../tests/activity_tests.rs"]
mod tests;
