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
    let text = crate::provider_env::redact_registered_secrets(text);
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
#[path = "../../tests/console_tests.rs"]
mod tests;
