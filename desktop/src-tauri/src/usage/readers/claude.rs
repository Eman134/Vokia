use std::path::PathBuf;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::Value as Json;

use crate::usage::model::{
    ProviderPlanUsage, ProviderUsageGroup, UsageWindow, FIVE_HOUR_MINS, WEEKLY_MINS,
};

use super::{
    build_http_client, home_dir, parse_timestamp_ms, parse_timestamp_secs, push_unique_path,
    resolve_endpoint_url, UsageReadError, DEFAULT_HTTP_TIMEOUT_SECS,
};

pub const DEFAULT_CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
pub const DEFAULT_CLAUDE_PROBE_COOLDOWN_MS: u64 = 15 * 60 * 1_000;
const CLAUDE_TOKEN_MARGIN_MS: u64 = 60_000;
const ANTHROPIC_OAUTH_BETA: &str = "oauth-2025-04-20";
const CLAUDE_USER_AGENT: &str = "claude-code/2.1.0";

#[derive(Clone, Debug)]
pub struct ClaudeReaderConfig {
    pub endpoint_url: Option<String>,
    pub credentials_paths: Vec<PathBuf>,
    /// When `None`, reads `CLAUDE_CODE_OAUTH_TOKEN`. When `Some("")`, treats env as unset.
    pub env_oauth_token: Option<String>,
    /// When `None`, reads `ANTHROPIC_API_KEY`. When `Some("")`, treats env as unset.
    pub env_api_key: Option<String>,
    pub probe_cooldown_ms: u64,
    pub timeout: Duration,
}

impl Default for ClaudeReaderConfig {
    fn default() -> Self {
        Self {
            endpoint_url: None,
            credentials_paths: default_credentials_paths(),
            env_oauth_token: None,
            env_api_key: None,
            probe_cooldown_ms: DEFAULT_CLAUDE_PROBE_COOLDOWN_MS,
            timeout: Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS),
        }
    }
}

pub fn default_credentials_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(config_dir) = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from) {
        push_unique_path(&mut paths, config_dir.join(".credentials.json"));
        push_unique_path(&mut paths, config_dir.join("credentials.json"));
    }
    if let Some(home) = home_dir() {
        let claude_dir = home.join(".claude");
        push_unique_path(&mut paths, claude_dir.join(".credentials.json"));
        push_unique_path(&mut paths, claude_dir.join("credentials.json"));
    }
    paths
}

/// Parses a passive `rate_limit_event` line from Claude Code CLI stream-json output.
#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_rate_limit_event_json(line: &str, now_ms: u64) -> Option<ProviderPlanUsage> {
    let value: Json = serde_json::from_str(line.trim()).ok()?;
    if value.get("type").and_then(Json::as_str) != Some("rate_limit_event") {
        return None;
    }
    let windows = value.pointer("/rate_limit_info/unifiedWindows")?;
    let primary = windows
        .get("five_hour")
        .and_then(|w| parse_stream_window(w, FIVE_HOUR_MINS));
    let secondary = windows
        .get("seven_day")
        .and_then(|w| parse_stream_window(w, WEEKLY_MINS));

    let mut groups = Vec::new();
    if let Some(map) = windows.as_object() {
        let mut extra_keys: Vec<&String> = map
            .keys()
            .filter(|k| k.as_str() != "five_hour" && k.as_str() != "seven_day")
            .collect();
        extra_keys.sort();
        for key in extra_keys {
            let duration = if key.contains("five") || key.contains("5h") {
                FIVE_HOUR_MINS
            } else {
                WEEKLY_MINS
            };
            if let Some(window) = parse_stream_window(&map[key], duration) {
                groups.push(ProviderUsageGroup {
                    id: key.clone(),
                    label: humanize_claude_bucket(key),
                    models: Vec::new(),
                    window,
                });
            }
        }
    }

    if primary.is_none() && secondary.is_none() && groups.is_empty() {
        return None;
    }

    Some(ProviderPlanUsage {
        provider_id: "claude".to_string(),
        label: Some("Claude Code".to_string()),
        primary,
        secondary,
        tertiary: None,
        groups,
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    })
}

fn parse_stream_window(value: &Json, duration_mins: u32) -> Option<UsageWindow> {
    let utilization = value.get("utilization").and_then(Json::as_f64)?;
    let resets_at = value
        .get("resetsAt")
        .or_else(|| value.get("resets_at"))
        .and_then(parse_timestamp_secs)
        .unwrap_or(0);
    let percent = (utilization * 100.0).round().clamp(0.0, 100.0) as u8;
    let mut window = UsageWindow::new(percent, resets_at, duration_mins);
    window.rejected = value
        .get("status")
        .and_then(Json::as_str)
        .is_some_and(|s| s.eq_ignore_ascii_case("rejected"));
    Some(window)
}

/// Parses Anthropic's `/api/oauth/usage` response JSON into `ProviderPlanUsage`.
pub fn parse_oauth_usage_json(
    body: &str,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    let value: Json = serde_json::from_str(body)
        .map_err(|err| UsageReadError::new(format!("Resposta JSON inválida do Claude: {err}")))?;

    let primary = value
        .get("five_hour")
        .and_then(|w| parse_oauth_window(w, FIVE_HOUR_MINS));
    let secondary = value
        .get("seven_day")
        .and_then(|w| parse_oauth_window(w, WEEKLY_MINS));

    let mut groups = Vec::new();
    for (key, label, model) in [
        ("seven_day_sonnet", "Claude Sonnet", "sonnet"),
        ("seven_day_opus", "Claude Opus", "opus"),
        ("seven_day_oauth_apps", "OAuth Apps", ""),
    ] {
        if let Some(window) = value
            .get(key)
            .and_then(|w| parse_oauth_window(w, WEEKLY_MINS))
        {
            groups.push(ProviderUsageGroup {
                id: key.to_string(),
                label: label.to_string(),
                models: if model.is_empty() {
                    Vec::new()
                } else {
                    vec![model.to_string()]
                },
                window,
            });
        }
    }

    let credits_available = value
        .get("extra_usage")
        .and_then(|extra| extra.get("is_enabled"))
        .and_then(Json::as_bool);

    if primary.is_none() && secondary.is_none() && groups.is_empty() {
        return Ok(None);
    }

    Ok(Some(ProviderPlanUsage {
        provider_id: "claude".to_string(),
        label: Some("Claude Code".to_string()),
        primary,
        secondary,
        tertiary: None,
        groups,
        credits_available,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    }))
}

fn parse_oauth_window(value: &Json, duration_mins: u32) -> Option<UsageWindow> {
    if value.is_null() {
        return None;
    }
    let utilization = value.get("utilization").and_then(Json::as_f64)?;
    let percent = utilization.round().clamp(0.0, 100.0) as u8;
    let resets_at = value
        .get("resets_at")
        .or_else(|| value.get("resetsAt"))
        .and_then(parse_timestamp_secs)
        .unwrap_or(0);
    Some(UsageWindow::new(percent, resets_at, duration_mins))
}

fn humanize_claude_bucket(key: &str) -> String {
    match key {
        "seven_day_sonnet" => "Claude Sonnet".to_string(),
        "seven_day_opus" => "Claude Opus".to_string(),
        other => other.replace('_', " "),
    }
}

enum ClaudeTokenLookup {
    Valid(String),
    Expired,
    NotFound,
}

fn resolve_claude_oauth_token(config: &ClaudeReaderConfig, now_ms: u64) -> ClaudeTokenLookup {
    let env_token = match &config.env_oauth_token {
        Some(value) => non_empty(value),
        None => std::env::var("CLAUDE_CODE_OAUTH_TOKEN")
            .ok()
            .and_then(|v| non_empty(&v)),
    };
    if let Some(token) = env_token {
        return ClaudeTokenLookup::Valid(token);
    }

    let mut saw_expired = false;
    for path in &config.credentials_paths {
        let Ok(raw) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(json) = serde_json::from_str::<Json>(&raw) else {
            continue;
        };
        let oauth = json
            .get("claudeAiOauth")
            .or_else(|| json.get("claude_ai_oauth"))
            .unwrap_or(&json);
        let token = oauth
            .get("accessToken")
            .or_else(|| oauth.get("access_token"))
            .and_then(Json::as_str)
            .and_then(non_empty);
        let Some(token) = token else {
            continue;
        };
        let expiry_ms = oauth
            .get("expiresAt")
            .or_else(|| oauth.get("expires_at"))
            .and_then(parse_timestamp_ms);
        if let Some(expires_at_ms) = expiry_ms {
            if expires_at_ms <= now_ms.saturating_add(CLAUDE_TOKEN_MARGIN_MS) {
                saw_expired = true;
                continue;
            }
        }
        return ClaudeTokenLookup::Valid(token);
    }

    if saw_expired {
        ClaudeTokenLookup::Expired
    } else {
        ClaudeTokenLookup::NotFound
    }
}

fn has_direct_api_key(config: &ClaudeReaderConfig) -> bool {
    match &config.env_api_key {
        Some(value) => !value.trim().is_empty(),
        None => std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .is_some_and(|v| !v.trim().is_empty()),
    }
}

fn non_empty(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Reads Claude quota safely:
/// 1. Prefers an existing, non-expired passive snapshot (`rate_limit_event`) within the
///    conservative probe cooldown (`15 min`) so zero extra requests hit Anthropic's API.
/// 2. When no snapshot exists yet (e.g. app open before first turn) or a window crossed
///    `resetsAt`, performs a single spaced OAuth usage probe respecting `UsageReadGate`.
pub async fn read_claude_usage(
    config: &ClaudeReaderConfig,
    cached_passive: Option<ProviderPlanUsage>,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    if let Some(prev) = &cached_passive {
        if !prev.has_expired_window(now_ms)
            && now_ms.saturating_sub(prev.updated_at_ms) < config.probe_cooldown_ms
        {
            return Ok(Some(prev.clone()));
        }
    }

    let token = match resolve_claude_oauth_token(config, now_ms) {
        ClaudeTokenLookup::Valid(token) => token,
        ClaudeTokenLookup::Expired => {
            return Err(UsageReadError::new(
                "Token OAuth local do Claude expirado; abra uma sessão da CLI para renovar.",
            ));
        }
        ClaudeTokenLookup::NotFound => {
            if let Some(prev) = cached_passive {
                return Ok(Some(prev));
            }
            if has_direct_api_key(config) {
                return Ok(None);
            }
            return Ok(None);
        }
    };

    let url = resolve_endpoint_url(DEFAULT_CLAUDE_USAGE_URL, config.endpoint_url.as_deref())?;
    let client = build_http_client(config.timeout)?;

    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header("anthropic-beta", ANTHROPIC_OAUTH_BETA)
        .header("Accept", "application/json")
        .header("User-Agent", CLAUDE_USER_AGENT)
        .send()
        .await
        .map_err(|err| {
            UsageReadError::redacted(
                format!("Falha de rede ao consultar cota do Claude: {err}"),
                &[&token],
            )
        })?;

    let status = response.status();
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(UsageReadError::rate_limited(
            "Limite de requisições (HTTP 429) ao consultar cota do Claude.",
        ));
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(UsageReadError::new(format!(
            "Sessão OAuth do Claude recusada (HTTP {}).",
            status.as_u16()
        )));
    }
    if !status.is_success() {
        return Err(UsageReadError::new(format!(
            "Endpoint de cota do Claude retornou HTTP {}.",
            status.as_u16()
        )));
    }

    let body = response.text().await.map_err(|err| {
        UsageReadError::redacted(
            format!("Falha ao ler resposta de cota do Claude: {err}"),
            &[&token],
        )
    })?;

    parse_oauth_usage_json(&body, now_ms)
        .map_err(|err| UsageReadError::redacted(err.message, &[&token]))
}
