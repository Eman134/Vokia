use std::collections::HashMap;

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
const WRITE_TOOLS: [&str; 2] = ["Write", "Edit"];

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
pub(super) struct ConsoleParser {
    pending_writes: HashMap<String, String>,
}

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
            ASSISTANT_KIND => self.assistant_lines(blocks),
            USER_KIND => self.user_lines(blocks),
            _ => Vec::new(),
        }
    }

    fn assistant_lines(&mut self, blocks: &[Value]) -> Vec<ConsoleLine> {
        blocks
            .iter()
            .filter_map(|block| match block["type"].as_str() {
                Some(TEXT_BLOCK) => {
                    let text = block["text"].as_str()?;
                    (!text.trim().is_empty()).then(|| ConsoleLine::new(ConsoleKind::Text, text))
                }
                Some(TOOL_USE_BLOCK) => {
                    let name = block["name"].as_str().unwrap_or_default();
                    let summary = tool_summary(name, &block["input"]);
                    // O resultado de Write/Edit pode trazer o conteúdo do arquivo: o console mostra só o resumo.
                    if WRITE_TOOLS.contains(&name) {
                        if let Some(id) = block["id"].as_str() {
                            self.pending_writes.insert(id.to_string(), summary.clone());
                        }
                    }
                    Some(ConsoleLine::new(ConsoleKind::Tool, &summary))
                }
                _ => None,
            })
            .collect()
    }

    fn user_lines(&mut self, blocks: &[Value]) -> Vec<ConsoleLine> {
        blocks
            .iter()
            .filter(|block| block["type"].as_str() == Some(TOOL_RESULT_BLOCK))
            .filter_map(|block| {
                let kind = if block["is_error"].as_bool() == Some(true) {
                    ConsoleKind::Error
                } else {
                    ConsoleKind::Output
                };
                if let Some(summary) = block["tool_use_id"]
                    .as_str()
                    .and_then(|id| self.pending_writes.remove(id))
                {
                    return Some(ConsoleLine::new(kind, &summary));
                }
                let text = block_text(&block["content"]);
                if text.trim().is_empty() {
                    return None;
                }
                Some(ConsoleLine::new(kind, &text))
            })
            .collect()
    }
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
