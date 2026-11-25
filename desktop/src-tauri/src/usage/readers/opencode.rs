use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::StatusCode;
use rusqlite::{Connection, OpenFlags};
use serde_json::Value as Json;

use crate::usage::model::{
    ProviderPlanUsage, UsageWindow, FIVE_HOUR_MINS, MONTHLY_MINS, WEEKLY_MINS,
};

use super::{
    build_http_client, home_dir, parse_timestamp_secs, push_unique_path, resolve_endpoint_url,
    UsageReadError, DEFAULT_HTTP_TIMEOUT_SECS,
};

pub const DEFAULT_OPENCODE_USAGE_URL: &str = "https://opencode.ai/zen/go/v1/usage";
const SQLITE_BUSY_TIMEOUT_MS: u32 = 100;

#[derive(Clone, Debug)]
pub struct OpenCodeReaderConfig {
    pub endpoint_url: Option<String>,
    pub explicit_api_key: Option<String>,
    pub db_paths: Vec<PathBuf>,
    pub auth_json_paths: Vec<PathBuf>,
    /// When `None`, reads `OPENCODE_API_KEY`. When `Some("")`, treats env as unset.
    pub env_api_key: Option<String>,
    pub timeout: Duration,
}

impl Default for OpenCodeReaderConfig {
    fn default() -> Self {
        let dirs = default_opencode_data_dirs();
        let db_paths = dirs.iter().map(|dir| dir.join("opencode.db")).collect();
        let auth_json_paths = dirs.iter().map(|dir| dir.join("auth.json")).collect();
        Self {
            endpoint_url: None,
            explicit_api_key: None,
            db_paths,
            auth_json_paths,
            env_api_key: None,
            timeout: Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS),
        }
    }
}

pub fn default_opencode_data_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(explicit) = std::env::var_os("OPENCODE_DATA_DIR").map(PathBuf::from) {
        push_unique_path(&mut dirs, explicit);
    }
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        push_unique_path(&mut dirs, xdg.join("opencode"));
    }
    if let Some(home) = home_dir() {
        push_unique_path(&mut dirs, home.join(".local").join("share").join("opencode"));
        #[cfg(target_os = "macos")]
        push_unique_path(
            &mut dirs,
            home.join("Library")
                .join("Application Support")
                .join("opencode"),
        );
    }
    #[cfg(windows)]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            push_unique_path(&mut dirs, local_app_data.join("opencode"));
        }
        if let Some(app_data) = std::env::var_os("APPDATA").map(PathBuf::from) {
            push_unique_path(&mut dirs, app_data.join("opencode"));
        }
    }
    dirs
}

/// Resolves an OpenCode Go API key in strict read-only precedence order:
/// 1. Explicit configured key
/// 2. `opencode.db` table `credential` (`SQLITE_OPEN_READ_ONLY` + `PRAGMA busy_timeout = 100`)
/// 3. `auth.json` entry `opencode-go`
/// 4. `OPENCODE_API_KEY` environment variable
/// 5. `auth.json` entry `opencode`
pub fn resolve_opencode_api_key(config: &OpenCodeReaderConfig) -> Option<String> {
    if let Some(explicit) = config.explicit_api_key.as_deref().and_then(non_empty) {
        return Some(explicit);
    }

    for db_path in &config.db_paths {
        if let Some(key) = read_key_from_opencode_db(db_path) {
            return Some(key);
        }
    }

    for auth_path in &config.auth_json_paths {
        if let Some(key) = read_key_from_auth_json(auth_path, "opencode-go") {
            return Some(key);
        }
    }

    let env_key = match &config.env_api_key {
        Some(value) => non_empty(value),
        None => std::env::var("OPENCODE_API_KEY")
            .ok()
            .and_then(|value| non_empty(&value)),
    };
    if let Some(key) = env_key {
        return Some(key);
    }

    for auth_path in &config.auth_json_paths {
        if let Some(key) = read_key_from_auth_json(auth_path, "opencode") {
            return Some(key);
        }
    }

    None
}

fn read_key_from_opencode_db(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).ok()?;
    let _ = conn.pragma_update(None, "busy_timeout", SQLITE_BUSY_TIMEOUT_MS);

    let columns = read_credential_columns(&conn)?;
    let key_col = pick_column(
        &columns,
        &["id", "provider", "provider_id", "name", "providerID", "key"],
    )?;
    let value_col = pick_column(
        &columns,
        &[
            "data",
            "value",
            "credential",
            "secret",
            "token",
            "api_key",
            "apiKey",
            "key",
        ],
    )?;

    if key_col == value_col {
        return None;
    }

    let sql = format!("SELECT \"{value_col}\" FROM credential WHERE \"{key_col}\" = ?1 LIMIT 1");
    for provider_key in ["opencode-go", "opencode"] {
        let raw: Option<String> = conn
            .query_row(&sql, [provider_key], |row| row.get::<_, String>(0))
            .ok();
        if let Some(key) = raw.as_deref().and_then(extract_key_from_raw_value) {
            return Some(key);
        }
    }
    None
}

fn read_credential_columns(conn: &Connection) -> Option<Vec<String>> {
    let mut stmt = conn.prepare("PRAGMA table_info(credential)").ok()?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .ok()?;
    let mut cols = Vec::new();
    for row in rows {
        if let Ok(name) = row {
            if is_safe_sql_ident(&name) {
                cols.push(name);
            }
        }
    }
    (!cols.is_empty()).then_some(cols)
}

fn pick_column<'a>(columns: &'a [String], candidates: &[&str]) -> Option<&'a str> {
    for candidate in candidates {
        if let Some(found) = columns
            .iter()
            .find(|col| col.eq_ignore_ascii_case(candidate))
        {
            return Some(found.as_str());
        }
    }
    None
}

fn is_safe_sql_ident(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn read_key_from_auth_json(path: &Path, provider_key: &str) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let value: Json = serde_json::from_str(&raw).ok()?;
    let entry = value.get(provider_key)?;
    extract_key_from_json(entry)
}

fn extract_key_from_raw_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('{') || trimmed.starts_with('"') {
        if let Ok(parsed) = serde_json::from_str::<Json>(trimmed) {
            return extract_key_from_json(&parsed);
        }
    }
    Some(trimmed.to_string())
}

fn extract_key_from_json(value: &Json) -> Option<String> {
    if let Some(raw) = value.as_str() {
        return non_empty(raw);
    }
    for field in ["key", "apiKey", "api_key", "token", "access", "access_token"] {
        if let Some(raw) = value.get(field).and_then(Json::as_str).and_then(non_empty) {
            return Some(raw);
        }
    }
    None
}

fn non_empty(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Converts OpenCode Go `/zen/go/v1/usage` JSON into `ProviderPlanUsage`:
/// - `rolling` (300 min / 5 h) -> `primary`
/// - more restrictive of `weekly` (10.080 min) and `monthly` (43.200 min) -> `secondary`
/// - the other long window -> `tertiary`
pub fn parse_opencode_usage_json(
    body: &str,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    let value: Json = serde_json::from_str(body)
        .map_err(|err| UsageReadError::new(format!("Resposta JSON inválida do OpenCode Go: {err}")))?;

    let root = value
        .get("usage")
        .or_else(|| value.get("limits"))
        .or_else(|| value.get("windows"))
        .filter(|v| v.is_object())
        .unwrap_or(&value);

    let rolling = root
        .get("rolling")
        .or_else(|| root.get("fiveHour"))
        .or_else(|| root.get("five_hour"))
        .and_then(|w| parse_opencode_window(w, FIVE_HOUR_MINS, now_ms));
    let weekly = root
        .get("weekly")
        .and_then(|w| parse_opencode_window(w, WEEKLY_MINS, now_ms));
    let monthly = root
        .get("monthly")
        .and_then(|w| parse_opencode_window(w, MONTHLY_MINS, now_ms));

    let (secondary, tertiary) = match (weekly, monthly) {
        (Some(w), Some(m)) => {
            if m.percent > w.percent {
                (Some(m), Some(w))
            } else {
                (Some(w), Some(m))
            }
        }
        (Some(w), None) => (Some(w), None),
        (None, Some(m)) => (Some(m), None),
        (None, None) => (None, None),
    };

    if rolling.is_none() && secondary.is_none() && tertiary.is_none() {
        return Ok(None);
    }

    Ok(Some(ProviderPlanUsage {
        provider_id: "opencode".to_string(),
        label: Some("OpenCode Go".to_string()),
        primary: rolling,
        secondary,
        tertiary,
        groups: Vec::new(),
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    }))
}

fn parse_opencode_window(value: &Json, duration_mins: u32, now_ms: u64) -> Option<UsageWindow> {
    if value.is_null() {
        return None;
    }
    let percent_f64 = value
        .get("usagePercent")
        .or_else(|| value.get("usage_percent"))
        .or_else(|| value.get("usedPercent"))
        .or_else(|| value.get("used_percent"))
        .or_else(|| value.get("percent"))
        .and_then(Json::as_f64)
        .or_else(|| {
            value
                .get("remainingPercent")
                .or_else(|| value.get("remaining_percent"))
                .and_then(Json::as_f64)
                .map(|rem| 100.0 - rem)
        })
        .or_else(|| {
            let used = value.get("used").and_then(Json::as_f64)?;
            let limit = value.get("limit").and_then(Json::as_f64)?;
            (limit > 0.0).then(|| (used / limit) * 100.0)
        })?;

    let percent = percent_f64.round().clamp(0.0, 100.0) as u8;

    let resets_at = value
        .get("resetsAt")
        .or_else(|| value.get("resets_at"))
        .or_else(|| value.get("resetAt"))
        .or_else(|| value.get("reset_at"))
        .and_then(parse_timestamp_secs)
        .or_else(|| {
            value
                .get("resetInSec")
                .or_else(|| value.get("reset_in_sec"))
                .or_else(|| value.get("resetInSeconds"))
                .or_else(|| value.get("reset_in_seconds"))
                .and_then(Json::as_u64)
                .map(|secs| (now_ms / 1_000).saturating_add(secs))
        })
        .unwrap_or(0);

    Some(UsageWindow::new(percent, resets_at, duration_mins))
}

/// Queries OpenCode Go usage endpoint (`GET https://opencode.ai/zen/go/v1/usage`).
/// Silently returns `Ok(None)` when no Go key is found or when HTTP 401/403 indicates free mode.
pub async fn read_opencode_usage(
    config: &OpenCodeReaderConfig,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    let config_for_key = config.clone();
    let api_key = tokio::task::spawn_blocking(move || resolve_opencode_api_key(&config_for_key))
        .await
        .ok()
        .flatten();

    let Some(api_key) = api_key else {
        return Ok(None);
    };

    let url = resolve_endpoint_url(DEFAULT_OPENCODE_USAGE_URL, config.endpoint_url.as_deref())?;
    let client = build_http_client(config.timeout)?;

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|err| {
            UsageReadError::redacted(
                format!("Falha de rede ao consultar cota do OpenCode Go: {err}"),
                &[&api_key],
            )
        })?;

    let status = response.status();
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Ok(None);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(UsageReadError::rate_limited(
            "Limite de requisições (HTTP 429) ao consultar cota do OpenCode Go.",
        ));
    }
    if !status.is_success() {
        return Err(UsageReadError::new(format!(
            "Endpoint de cota do OpenCode Go retornou HTTP {}.",
            status.as_u16()
        )));
    }

    let body = response.text().await.map_err(|err| {
        UsageReadError::redacted(
            format!("Falha ao ler resposta de cota do OpenCode Go: {err}"),
            &[&api_key],
        )
    })?;

    parse_opencode_usage_json(&body, now_ms)
        .map_err(|err| UsageReadError::redacted(err.message, &[&api_key]))
}
