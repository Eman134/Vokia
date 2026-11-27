use std::path::PathBuf;
use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{json, Value as Json};

use crate::usage::model::{
    ProviderPlanUsage, ProviderUsageGroup, UsageWindow, DAILY_MINS, FIVE_HOUR_MINS, WEEKLY_MINS,
};

use super::{
    build_http_client, home_dir, parse_timestamp_ms, parse_timestamp_secs, push_unique_path,
    resolve_endpoint_url, UsageReadError, DEFAULT_HTTP_TIMEOUT_SECS,
};

pub const DEFAULT_LOAD_CODE_ASSIST_URL: &str =
    "https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist";
pub const DEFAULT_RETRIEVE_QUOTA_URL: &str =
    "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary";
pub const DEFAULT_ANTIGRAVITY_USER_AGENT: &str = "antigravity/1.104.0";
pub const TOKEN_EXPIRY_MARGIN_MS: u64 = 60_000;

const SIX_HOURS_SECS: u64 = 6 * 3_600;

#[derive(Clone, Debug)]
pub struct GeminiReaderConfig {
    pub load_code_assist_url: Option<String>,
    pub retrieve_quota_url: Option<String>,
    pub token_paths: Vec<PathBuf>,
    /// When `None`, reads `GEMINI_API_KEY`. When `Some("")`, treats env as unset.
    pub env_gemini_api_key: Option<String>,
    pub user_agent: String,
    pub timeout: Duration,
}

impl Default for GeminiReaderConfig {
    fn default() -> Self {
        let user_agent = std::env::var("VOKIA_ANTIGRAVITY_USER_AGENT")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| DEFAULT_ANTIGRAVITY_USER_AGENT.to_string());
        Self {
            load_code_assist_url: None,
            retrieve_quota_url: None,
            token_paths: default_antigravity_token_paths(),
            env_gemini_api_key: None,
            user_agent,
            timeout: Duration::from_secs(DEFAULT_HTTP_TIMEOUT_SECS),
        }
    }
}

pub fn default_antigravity_token_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(gemini_dir) = std::env::var_os("GEMINI_DIR").map(PathBuf::from) {
        push_unique_path(&mut paths, gemini_dir.join("antigravity-oauth-token"));
        push_unique_path(
            &mut paths,
            gemini_dir
                .join("antigravity-acp")
                .join("antigravity-oauth-token"),
        );
        push_unique_path(
            &mut paths,
            gemini_dir
                .join("antigravity-cli")
                .join("antigravity-oauth-token"),
        );
        push_unique_path(&mut paths, gemini_dir.join("oauth_creds.json"));
    }
    if let Some(home) = home_dir() {
        let gemini_home = home.join(".gemini");
        push_unique_path(&mut paths, gemini_home.join("antigravity-oauth-token"));
        push_unique_path(
            &mut paths,
            gemini_home
                .join("antigravity-acp")
                .join("antigravity-oauth-token"),
        );
        push_unique_path(
            &mut paths,
            gemini_home
                .join("antigravity-cli")
                .join("antigravity-oauth-token"),
        );
        push_unique_path(&mut paths, gemini_home.join("oauth_creds.json"));
        push_unique_path(
            &mut paths,
            home.join(".config")
                .join("antigravity")
                .join("antigravity-oauth-token"),
        );
    }
    #[cfg(windows)]
    {
        if let Some(app_data) = std::env::var_os("APPDATA").map(PathBuf::from) {
            push_unique_path(
                &mut paths,
                app_data
                    .join("antigravity-cli")
                    .join("antigravity-oauth-token"),
            );
            push_unique_path(
                &mut paths,
                app_data.join("antigravity").join("antigravity-oauth-token"),
            );
        }
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            push_unique_path(
                &mut paths,
                local_app_data
                    .join("antigravity-cli")
                    .join("antigravity-oauth-token"),
            );
        }
    }
    paths
}

/// Classifies direct `GEMINI_API_KEY` (pay-as-you-go) 429 / `limit=0` quota errors.
#[cfg_attr(not(test), allow(dead_code))]
pub fn is_gemini_api_key_quota_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("429")
        || lower.contains("limit: 0")
        || lower.contains("limit=0")
        || lower.contains("resource_exhausted")
        || lower.contains("quota exceeded")
}

fn has_direct_gemini_api_key(config: &GeminiReaderConfig) -> bool {
    match &config.env_gemini_api_key {
        Some(value) => !value.trim().is_empty(),
        None => std::env::var("GEMINI_API_KEY")
            .ok()
            .is_some_and(|value| !value.trim().is_empty()),
    }
}

/// Reads a valid (`expiry > now + 60s`) OAuth `access_token` from the local
/// `antigravity-oauth-token` file without ever storing or refreshing client secrets.
pub fn read_valid_oauth_token(
    paths: &[PathBuf],
    now_ms: u64,
) -> Result<Option<String>, UsageReadError> {
    let mut saw_expired = false;
    let min_expiry_ms = now_ms.saturating_add(TOKEN_EXPIRY_MARGIN_MS);

    for path in paths {
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(path).map_err(|_| {
            UsageReadError::new("Não foi possível ler o arquivo de token local do Antigravity.")
        })?;
        let parsed: Json = serde_json::from_str(&raw).map_err(|_| {
            UsageReadError::new("Arquivo de token local do Antigravity com JSON inválido.")
        })?;

        let container = parsed
            .get("token")
            .or_else(|| parsed.get("credentials"))
            .filter(|v| v.is_object())
            .unwrap_or(&parsed);

        let token = container
            .get("access_token")
            .or_else(|| container.get("accessToken"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let Some(token) = token else {
            continue;
        };

        let expiry_ms = container
            .get("expiry_date")
            .or_else(|| container.get("expiryDate"))
            .or_else(|| container.get("expires_at"))
            .or_else(|| container.get("expiresAt"))
            .or_else(|| container.get("expiry"))
            .and_then(parse_timestamp_ms);

        match expiry_ms {
            Some(expires_at_ms) if expires_at_ms > min_expiry_ms => return Ok(Some(token)),
            _ => {
                saw_expired = true;
            }
        }
    }

    if saw_expired {
        Err(UsageReadError::new(
            "Token OAuth local do Antigravity expirado; abra uma sessão ativa da CLI para renovar.",
        ))
    } else {
        Ok(None)
    }
}

/// Parses `retrieveUserQuotaSummary` response (`groups[]`) into `ProviderPlanUsage`.
/// Treats an omitted `remainingFraction` with a valid `resetTime` as exhausted (`0%` remaining / `100%` used).
pub fn parse_quota_summary_json(
    body: &str,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    let value: Json = serde_json::from_str(body).map_err(|err| {
        UsageReadError::new(format!(
            "Resposta JSON inválida de retrieveUserQuotaSummary: {err}"
        ))
    })?;

    let Some(raw_groups) = value
        .get("groups")
        .or_else(|| value.pointer("/command/data/groups"))
        .and_then(Json::as_array)
    else {
        return Ok(None);
    };

    let mut groups = Vec::new();
    let mut worst_five_hour: Option<UsageWindow> = None;
    let mut worst_weekly: Option<UsageWindow> = None;

    for (idx, group) in raw_groups.iter().enumerate() {
        let display_name = group
            .get("displayName")
            .or_else(|| group.get("name"))
            .or_else(|| group.get("label"))
            .or_else(|| group.get("groupName"))
            .or_else(|| group.get("groupId"))
            .or_else(|| group.get("id"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Gemini Group {}", idx + 1));

        let group_id = group
            .get("groupId")
            .or_else(|| group.get("id"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| slugify(&display_name));

        let models = extract_group_models(group);

        let buckets: Vec<&Json> = group
            .get("buckets")
            .or_else(|| group.get("quotaBuckets"))
            .or_else(|| group.get("windows"))
            .and_then(Json::as_array)
            .map(|arr| arr.iter().collect())
            .unwrap_or_else(|| vec![group]);

        let multi_bucket = buckets.len() > 1;
        for (bucket_idx, bucket) in buckets.into_iter().enumerate() {
            let Some(window) = parse_gemini_bucket(bucket, now_ms) else {
                continue;
            };
            if window.duration_mins == FIVE_HOUR_MINS {
                update_worst(&mut worst_five_hour, &window);
            } else if window.duration_mins == WEEKLY_MINS {
                update_worst(&mut worst_weekly, &window);
            }

            let suffix = if window.duration_mins == FIVE_HOUR_MINS {
                "5h"
            } else if window.duration_mins == WEEKLY_MINS {
                "weekly"
            } else {
                "window"
            };
            let id = if multi_bucket {
                format!("{group_id}:{suffix}:{bucket_idx}")
            } else {
                group_id.clone()
            };
            let label = if multi_bucket {
                format!("{display_name} ({})", window.kind.label())
            } else {
                display_name.clone()
            };

            groups.push(ProviderUsageGroup {
                id,
                label,
                models: models.clone(),
                window,
            });
        }
    }

    if groups.is_empty() {
        return Ok(None);
    }

    Ok(Some(ProviderPlanUsage {
        provider_id: "antigravity".to_string(),
        label: Some("Gemini".to_string()),
        primary: worst_five_hour,
        secondary: worst_weekly,
        tertiary: None,
        groups,
        credits_available: None,
        reset_credits_available: None,
        updated_at_ms: now_ms,
        read_failed: false,
    }))
}

fn parse_gemini_bucket(bucket: &Json, now_ms: u64) -> Option<UsageWindow> {
    let resets_at = bucket
        .get("resetTime")
        .or_else(|| bucket.get("reset_time"))
        .or_else(|| bucket.get("resetsAt"))
        .and_then(parse_timestamp_secs)
        .unwrap_or(0);

    let remaining_fraction = bucket
        .get("remainingFraction")
        .or_else(|| bucket.get("remaining_fraction"))
        .and_then(Json::as_f64);

    let (percent, rejected) = match remaining_fraction {
        Some(fraction) if fraction.is_finite() => {
            let clamped = fraction.clamp(0.0, 1.0);
            let used = ((1.0 - clamped) * 100.0).round().clamp(0.0, 100.0) as u8;
            (used, clamped <= 0.0)
        }
        // Omitted remainingFraction with a valid resetTime means exhausted (0% remaining).
        None if resets_at > 0 => (100, true),
        _ => return None,
    };

    let duration_mins = classify_bucket_duration_mins(bucket, resets_at, now_ms);
    let mut window = UsageWindow::new(percent, resets_at, duration_mins);
    window.rejected = rejected;
    Some(window)
}

fn classify_bucket_duration_mins(bucket: &Json, resets_at: u64, now_ms: u64) -> u32 {
    if let Some(mins) = bucket
        .get("windowDurationMins")
        .or_else(|| bucket.get("durationMins"))
        .and_then(Json::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|&n| n > 0)
    {
        return mins;
    }

    if let Some(secs) = bucket
        .get("durationSeconds")
        .or_else(|| bucket.get("windowDurationSeconds"))
        .and_then(Json::as_u64)
        .filter(|&n| n >= 60)
    {
        return (secs / 60) as u32;
    }

    if let Some(raw_duration) = bucket.get("windowDuration").and_then(Json::as_str) {
        if let Some(secs_str) = raw_duration.trim().strip_suffix('s') {
            if let Ok(secs) = secs_str.parse::<u64>() {
                if secs >= 60 {
                    return (secs / 60) as u32;
                }
            }
        }
    }

    let mut descriptor = String::new();
    for field in [
        "bucketId",
        "id",
        "label",
        "name",
        "period",
        "window",
        "windowType",
        "quotaType",
        "displayName",
    ] {
        if let Some(text) = bucket.get(field).and_then(Json::as_str) {
            descriptor.push(' ');
            descriptor.push_str(&text.to_ascii_lowercase());
        }
    }

    if descriptor.contains("week")
        || descriptor.contains("7d")
        || descriptor.contains("7 d")
        || descriptor.contains("seven")
        || descriptor.contains("semanal")
    {
        return WEEKLY_MINS;
    }
    if descriptor.contains("5h")
        || descriptor.contains("5 h")
        || descriptor.contains("5-h")
        || descriptor.contains("five-hour")
        || descriptor.contains("five_hour")
        || descriptor.contains("five hour")
        || descriptor.contains("rolling")
        || descriptor.contains("short")
    {
        return FIVE_HOUR_MINS;
    }
    if descriptor.contains("daily") || descriptor.contains("24h") || descriptor.contains("1d") {
        return DAILY_MINS;
    }

    let now_secs = now_ms / 1_000;
    if resets_at > now_secs.saturating_add(SIX_HOURS_SECS) {
        WEEKLY_MINS
    } else {
        FIVE_HOUR_MINS
    }
}

fn extract_group_models(group: &Json) -> Vec<String> {
    let mut models = Vec::new();
    let arr = group
        .get("models")
        .or_else(|| group.get("modelIds"))
        .and_then(Json::as_array);
    if let Some(items) = arr {
        for item in items {
            let candidate = item
                .as_str()
                .or_else(|| item.get("modelId").and_then(Json::as_str))
                .or_else(|| item.get("id").and_then(Json::as_str))
                .or_else(|| item.get("name").and_then(Json::as_str));
            if let Some(name) = candidate.map(str::trim).filter(|s| !s.is_empty()) {
                if !models.iter().any(|existing| existing == name) {
                    models.push(name.to_string());
                }
            }
        }
    }
    models
}

fn update_worst(target: &mut Option<UsageWindow>, candidate: &UsageWindow) {
    match target {
        Some(existing) if candidate.percent > existing.percent => {
            *existing = candidate.clone();
        }
        None => {
            *target = Some(candidate.clone());
        }
        _ => {}
    }
}

fn slugify(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_end_matches('-');
    if trimmed.is_empty() {
        "gemini".to_string()
    } else {
        trimmed.to_string()
    }
}

fn extract_companion_project(body: &str) -> Result<Option<String>, UsageReadError> {
    let value: Json = serde_json::from_str(body).map_err(|err| {
        UsageReadError::new(format!("Resposta JSON inválida de loadCodeAssist: {err}"))
    })?;
    let project = value
        .get("cloudaicompanionProject")
        .and_then(|field| {
            field
                .as_str()
                .or_else(|| field.get("id").and_then(Json::as_str))
        })
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok(project)
}

/// Queries Gemini / Antigravity plan quota via Google Cloud Code Assist:
/// 1. Skips (`Ok(None)`) when direct `GEMINI_API_KEY` (pay-as-you-go) is active.
/// 2. Reads a valid local `access_token` (`expiry > now + 60s`) from `antigravity-oauth-token`.
/// 3. Calls `POST loadCodeAssist` -> extracts `cloudaicompanionProject`.
/// 4. Calls `POST retrieveUserQuotaSummary` -> converts `groups[]` into `ProviderPlanUsage`.
pub async fn read_gemini_usage(
    config: &GeminiReaderConfig,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    if has_direct_gemini_api_key(config) {
        return Ok(None);
    }

    let custom_endpoints =
        config.load_code_assist_url.is_some() || config.retrieve_quota_url.is_some();
    match read_gemini_usage_http(config, now_ms).await {
        Ok(Some(usage)) => Ok(Some(usage)),
        Ok(None) if !custom_endpoints => read_usage_via_agy_cli(config.timeout, now_ms).await,
        Ok(None) => Ok(None),
        Err(err) if !custom_endpoints => {
            if let Ok(Some(usage)) = read_usage_via_agy_cli(config.timeout, now_ms).await {
                Ok(Some(usage))
            } else {
                Err(err)
            }
        }
        Err(err) => Err(err),
    }
}

async fn read_usage_via_agy_cli(
    timeout: Duration,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    let mut command = tokio::process::Command::new("agy");
    command
        .args(["-p", "/usage", "--output-format", "json"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(windows)]
    {
        command.creation_flags(0x0800_0000);
    }
    let output = tokio::time::timeout(timeout, command.output())
        .await
        .map_err(|_| UsageReadError::new("Timeout ao consultar `agy -p /usage`."))?
        .map_err(|err| UsageReadError::new(format!("Falha ao executar `agy -p /usage`: {err}")))?;
    if !output.status.success() {
        return Ok(None);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_quota_summary_json(stdout.trim(), now_ms)
}

async fn read_gemini_usage_http(
    config: &GeminiReaderConfig,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    let Some(access_token) = read_valid_oauth_token(&config.token_paths, now_ms)? else {
        return Ok(None);
    };

    let load_url = resolve_endpoint_url(
        DEFAULT_LOAD_CODE_ASSIST_URL,
        config.load_code_assist_url.as_deref(),
    )?;
    let quota_url = resolve_endpoint_url(
        DEFAULT_RETRIEVE_QUOTA_URL,
        config.retrieve_quota_url.as_deref(),
    )?;
    let client = build_http_client(config.timeout)?;

    let load_body = json!({
        "metadata": {
            "ideType": "IDE_UNSPECIFIED",
            "platform": "PLATFORM_UNSPECIFIED",
            "pluginType": "GEMINI"
        }
    });

    let load_resp = client
        .post(&load_url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", &config.user_agent)
        .json(&load_body)
        .send()
        .await
        .map_err(|err| {
            UsageReadError::redacted(
                format!("Falha de rede ao consultar loadCodeAssist: {err}"),
                &[&access_token],
            )
        })?;

    check_google_status("loadCodeAssist", load_resp.status())?;

    let load_text = load_resp.text().await.map_err(|err| {
        UsageReadError::redacted(
            format!("Falha ao ler resposta de loadCodeAssist: {err}"),
            &[&access_token],
        )
    })?;

    let Some(project) = extract_companion_project(&load_text)
        .map_err(|err| UsageReadError::redacted(err.message, &[&access_token]))?
    else {
        return Ok(None);
    };

    let quota_resp = client
        .post(&quota_url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("User-Agent", &config.user_agent)
        .json(&json!({ "project": project }))
        .send()
        .await
        .map_err(|err| {
            UsageReadError::redacted(
                format!("Falha de rede ao consultar retrieveUserQuotaSummary: {err}"),
                &[&access_token],
            )
        })?;

    check_google_status("retrieveUserQuotaSummary", quota_resp.status())?;

    let quota_text = quota_resp.text().await.map_err(|err| {
        UsageReadError::redacted(
            format!("Falha ao ler resposta de retrieveUserQuotaSummary: {err}"),
            &[&access_token],
        )
    })?;

    parse_quota_summary_json(&quota_text, now_ms)
        .map_err(|err| UsageReadError::redacted(err.message, &[&access_token]))
}

fn check_google_status(endpoint: &str, status: StatusCode) -> Result<(), UsageReadError> {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(UsageReadError::rate_limited(format!(
            "Limite de requisições (HTTP 429) ao consultar {endpoint} do Gemini/Antigravity."
        )));
    }
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return Err(UsageReadError::new(format!(
            "Token OAuth do Antigravity recusado em {endpoint} (HTTP {}).",
            status.as_u16()
        )));
    }
    if !status.is_success() {
        return Err(UsageReadError::new(format!(
            "Endpoint {endpoint} do Gemini/Antigravity retornou HTTP {}.",
            status.as_u16()
        )));
    }
    Ok(())
}
