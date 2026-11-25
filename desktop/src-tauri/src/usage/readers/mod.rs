pub mod claude;
pub mod codex;
pub mod gemini;
pub mod opencode;

use std::fmt::{self, Display};
use std::path::PathBuf;
use std::sync::{Arc, Once};
use std::time::Duration;

use chrono::DateTime;
use serde_json::Value as Json;

use super::{now_millis, OptionalUsageReaderFn, UsageEngine};

pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 10;
pub const DEFAULT_PROCESS_TIMEOUT_SECS: u64 = 10;

const MS_THRESHOLD: u64 = 10_000_000_000;
const MS_PER_SECOND: u64 = 1_000;
const REDACTED: &str = "[REDACTED]";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsageReadError {
    pub message: String,
    pub rate_limited: bool,
}

impl UsageReadError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: redact_secrets(&message.into(), &[]),
            rate_limited: false,
        }
    }

    pub fn redacted(message: impl Into<String>, secrets: &[&str]) -> Self {
        Self {
            message: redact_secrets(&message.into(), secrets),
            rate_limited: false,
        }
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self {
            message: redact_secrets(&message.into(), &[]),
            rate_limited: true,
        }
    }
}

impl Display for UsageReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for UsageReadError {}

impl From<String> for UsageReadError {
    fn from(message: String) -> Self {
        Self::new(message)
    }
}

impl From<&str> for UsageReadError {
    fn from(message: &str) -> Self {
        Self::new(message)
    }
}

/// Removes explicit secrets and common credential/token patterns so no local
/// OAuth token or API key ever leaks into logs, error messages, SQLite or UI events.
pub fn redact_secrets(text: &str, secrets: &[&str]) -> String {
    let mut sanitized = text.to_string();
    for secret in secrets {
        let trimmed = secret.trim();
        if !trimmed.is_empty() {
            sanitized = sanitized.replace(trimmed, REDACTED);
        }
    }
    sanitized = redact_bearer_tokens(&sanitized);
    sanitized = redact_labeled_secrets(&sanitized);
    redact_prefixed_tokens(&sanitized)
}

fn is_token_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '+' | '/' | '=')
}

fn redact_bearer_tokens(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let needle = "bearer ";
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(rel) = lower[cursor..].find(needle) {
        let start = cursor + rel;
        let token_start = start + needle.len();
        out.push_str(&input[cursor..token_start]);

        let rest = &input[token_start..];
        let mut token_len = 0;
        for ch in rest.chars() {
            if is_token_char(ch) {
                token_len += ch.len_utf8();
            } else {
                break;
            }
        }
        if token_len > 0 {
            out.push_str(REDACTED);
            cursor = token_start + token_len;
        } else {
            cursor = token_start;
        }
    }
    out.push_str(&input[cursor..]);
    out
}

fn redact_labeled_secrets(input: &str) -> String {
    const LABELS: [&str; 6] = [
        "access_token",
        "accesstoken",
        "refresh_token",
        "refreshtoken",
        "client_secret",
        "api_key",
    ];
    let mut current = input.to_string();
    for label in LABELS {
        current = redact_key_value_label(&current, label);
    }
    current
}

fn redact_key_value_label(input: &str, label: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;

    while let Some(rel) = lower[cursor..].find(label) {
        let start = cursor + rel;
        let after_label = start + label.len();
        let rest = &input[after_label..];
        let mut idx = 0;
        let bytes = rest.as_bytes();
        while idx < bytes.len() && matches!(bytes[idx], b'"' | b'\'' | b' ') {
            idx += 1;
        }
        if idx < bytes.len() && matches!(bytes[idx], b':' | b'=') {
            idx += 1;
            while idx < bytes.len() && matches!(bytes[idx], b' ' | b'"' | b'\'') {
                idx += 1;
            }
            let val_start = after_label + idx;
            let val_rest = &input[val_start..];
            let mut val_len = 0;
            for ch in val_rest.chars() {
                if is_token_char(ch) {
                    val_len += ch.len_utf8();
                } else {
                    break;
                }
            }
            if val_len > 0 {
                out.push_str(&input[cursor..val_start]);
                out.push_str(REDACTED);
                cursor = val_start + val_len;
                continue;
            }
        }
        out.push_str(&input[cursor..after_label]);
        cursor = after_label;
    }
    out.push_str(&input[cursor..]);
    out
}

fn redact_prefixed_tokens(input: &str) -> String {
    const PREFIXES: [&str; 7] = [
        "ya29.",
        "sk-",
        "AIza",
        "eyJ",
        "oc_",
        "opencode_",
        "gho_",
    ];
    let mut current = input.to_string();
    for prefix in PREFIXES {
        let mut out = String::with_capacity(current.len());
        let mut cursor = 0;
        while let Some(rel) = current[cursor..].find(prefix) {
            let start = cursor + rel;
            let rest = &current[start..];
            let mut len = 0;
            for ch in rest.chars() {
                if is_token_char(ch) {
                    len += ch.len_utf8();
                } else {
                    break;
                }
            }
            if len >= prefix.len() + 6 {
                out.push_str(&current[cursor..start]);
                out.push_str(REDACTED);
                cursor = start + len;
            } else {
                out.push_str(&current[cursor..start + prefix.len()]);
                cursor = start + prefix.len();
            }
        }
        out.push_str(&current[cursor..]);
        current = out;
    }
    current
}

pub(crate) fn is_loopback_url(url: &str) -> bool {
    let trimmed = url.trim();
    let rest = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"));
    let Some(rest) = rest else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.starts_with("[::1]") {
        let tail = &authority[5..];
        return tail.is_empty() || tail.starts_with(':');
    }
    let host = authority.split(':').next().unwrap_or("");
    host.eq_ignore_ascii_case("127.0.0.1") || host.eq_ignore_ascii_case("localhost")
}

pub(crate) fn resolve_endpoint_url(
    default_url: &str,
    override_url: Option<&str>,
) -> Result<String, UsageReadError> {
    let Some(custom) = override_url.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(default_url.to_string());
    };
    if custom == default_url || is_loopback_url(custom) {
        return Ok(custom.to_string());
    }
    Err(UsageReadError::new(
        "Override de URL de leitura de cota só é permitido em loopback (127.0.0.1/localhost).",
    ))
}

pub(crate) fn build_http_client(timeout: Duration) -> Result<reqwest::Client, UsageReadError> {
    static RUSTLS_INIT: Once = Once::new();
    RUSTLS_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
    reqwest::Client::builder()
        .timeout(timeout)
        .connect_timeout(timeout.min(Duration::from_secs(5)))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|err| UsageReadError::new(format!("Falha ao inicializar cliente HTTP: {err}")))
}

pub(crate) fn parse_timestamp_secs(value: &Json) -> Option<u64> {
    if let Some(num) = value.as_u64() {
        return Some(normalize_secs(num));
    }
    if let Some(num) = value.as_i64() {
        return (num > 0).then(|| normalize_secs(num as u64));
    }
    if let Some(num) = value.as_f64() {
        return (num.is_finite() && num > 0.0).then(|| normalize_secs(num.round() as u64));
    }
    if let Some(raw) = value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(num) = raw.parse::<u64>() {
            return Some(normalize_secs(num));
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
            let ts = dt.timestamp();
            return (ts > 0).then_some(ts as u64);
        }
    }
    None
}

pub(crate) fn parse_timestamp_ms(value: &Json) -> Option<u64> {
    if let Some(num) = value.as_u64() {
        return Some(normalize_ms(num));
    }
    if let Some(num) = value.as_i64() {
        return (num > 0).then(|| normalize_ms(num as u64));
    }
    if let Some(num) = value.as_f64() {
        return (num.is_finite() && num > 0.0).then(|| normalize_ms(num.round() as u64));
    }
    if let Some(raw) = value.as_str().map(str::trim).filter(|s| !s.is_empty()) {
        if let Ok(num) = raw.parse::<u64>() {
            return Some(normalize_ms(num));
        }
        if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
            let ts = dt.timestamp_millis();
            return (ts > 0).then_some(ts as u64);
        }
    }
    None
}

fn normalize_secs(raw: u64) -> u64 {
    if raw >= MS_THRESHOLD {
        raw / MS_PER_SECOND
    } else {
        raw
    }
}

fn normalize_ms(raw: u64) -> u64 {
    if raw < MS_THRESHOLD {
        raw.saturating_mul(MS_PER_SECOND)
    } else {
        raw
    }
}

pub(crate) fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

pub(crate) fn push_unique_path(list: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !list.iter().any(|existing| existing == &candidate) {
        list.push(candidate);
    }
}

pub type CommandResolver = Arc<dyn Fn() -> Option<String> + Send + Sync>;

/// Registers the four built-in quota readers (Claude, Codex, OpenCode Go, Gemini/Antigravity)
/// in the shared `UsageEngine`.
pub fn register_default_readers(engine: &UsageEngine, codex_command: Option<CommandResolver>) {
    let claude_gate = engine.read_gate();
    let claude_reader: OptionalUsageReaderFn = Arc::new(move || {
        let gate = claude_gate.clone();
        Box::pin(async move {
            let config = claude::ClaudeReaderConfig::default();
            let cached = gate.get("claude");
            claude::read_claude_usage(&config, cached, now_millis()).await
        })
    });
    engine.register_optional_reader("claude", claude_reader);

    let codex_reader: OptionalUsageReaderFn = Arc::new(move || {
        let command = codex_command.as_ref().and_then(|resolver| (resolver)());
        Box::pin(async move {
            let config = codex::CodexReaderConfig {
                command,
                ..codex::CodexReaderConfig::default()
            };
            codex::read_codex_usage(&config, now_millis()).await
        })
    });
    engine.register_optional_reader("codex", codex_reader);

    let opencode_reader: OptionalUsageReaderFn = Arc::new(|| {
        Box::pin(async {
            let config = opencode::OpenCodeReaderConfig::default();
            opencode::read_opencode_usage(&config, now_millis()).await
        })
    });
    engine.register_optional_reader("opencode", opencode_reader);

    let gemini_reader: OptionalUsageReaderFn = Arc::new(|| {
        Box::pin(async {
            let config = gemini::GeminiReaderConfig::default();
            gemini::read_gemini_usage(&config, now_millis()).await
        })
    });
    engine.register_optional_reader("antigravity", gemini_reader);
}

#[cfg(test)]
#[path = "../../../tests/usage_readers_tests.rs"]
mod tests;
