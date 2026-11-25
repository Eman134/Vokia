use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value as Json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::time::timeout;

use crate::claude::{base_command, kill_process_tree};
use crate::usage::model::{
    ProviderPlanUsage, ProviderUsageGroup, UsageWindow, FIVE_HOUR_MINS, WEEKLY_MINS,
};

use super::{parse_timestamp_secs, UsageReadError, DEFAULT_PROCESS_TIMEOUT_SECS};

const INIT_REQUEST_ID: u64 = 1;
const RATE_LIMITS_REQUEST_ID: u64 = 2;
const METHOD_RATE_LIMITS_READ: &str = "account/rateLimits/read";
const METHOD_RATE_LIMITS_UPDATED: &str = "account/rateLimits/updated";

#[derive(Clone, Debug)]
pub struct CodexReaderConfig {
    pub command: Option<String>,
    pub timeout: Duration,
}

impl Default for CodexReaderConfig {
    fn default() -> Self {
        Self {
            command: None,
            timeout: Duration::from_secs(DEFAULT_PROCESS_TIMEOUT_SECS),
        }
    }
}

/// Parses either a JSON-RPC response to `account/rateLimits/read`, a push notification
/// `account/rateLimits/updated`, or a bare rate-limits payload object.
pub fn parse_rate_limits_rpc_message(
    value: &Json,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(Json::as_str)
            .unwrap_or("erro RPC do Codex");
        if is_unauthenticated_message(message) {
            return Ok(None);
        }
        return Err(UsageReadError::new(format!(
            "Erro ao ler cota do Codex via app-server: {message}"
        )));
    }

    let payload = if let Some(result) = value.get("result") {
        result
    } else if value.get("method").and_then(Json::as_str) == Some(METHOD_RATE_LIMITS_UPDATED) {
        match value.get("params") {
            Some(params) => params,
            None => return Ok(None),
        }
    } else {
        value
    };

    if payload.is_null() {
        return Ok(None);
    }

    let by_limit_id = payload
        .get("rateLimitsByLimitId")
        .or_else(|| payload.get("rate_limits_by_limit_id"))
        .and_then(Json::as_object);

    let main_bucket = payload
        .get("rateLimits")
        .or_else(|| payload.get("rate_limits"))
        .or_else(|| by_limit_id.and_then(|map| map.get("codex")))
        .or_else(|| {
            if payload.get("primary").is_some() || payload.get("secondary").is_some() {
                Some(payload)
            } else {
                None
            }
        });

    let main_limit_id = main_bucket
        .and_then(|bucket| {
            bucket
                .get("limitId")
                .or_else(|| bucket.get("limit_id"))
                .and_then(Json::as_str)
        })
        .unwrap_or("codex");

    let primary = main_bucket
        .and_then(|bucket| bucket.get("primary"))
        .and_then(|w| parse_codex_window(w, FIVE_HOUR_MINS));
    let secondary = main_bucket
        .and_then(|bucket| bucket.get("secondary"))
        .and_then(|w| parse_codex_window(w, WEEKLY_MINS));

    let credits_available = extract_credits_available(main_bucket, payload);
    let reset_credits_available = extract_reset_credits(main_bucket, payload);

    let mut groups = Vec::new();
    if let Some(map) = by_limit_id {
        let mut keys: Vec<&String> = map.keys().collect();
        keys.sort();
        for key in keys {
            if key == main_limit_id || key == "codex" {
                continue;
            }
            let bucket = &map[key];
            let display_name = bucket
                .get("limitName")
                .or_else(|| bucket.get("limit_name"))
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or(key.as_str());
            let models = vec![display_name.to_string()];

            let bucket_primary = bucket
                .get("primary")
                .and_then(|w| parse_codex_window(w, FIVE_HOUR_MINS));
            let bucket_secondary = bucket
                .get("secondary")
                .and_then(|w| parse_codex_window(w, WEEKLY_MINS));

            match (bucket_primary, bucket_secondary) {
                (Some(p), Some(s)) => {
                    groups.push(ProviderUsageGroup {
                        id: format!("{key}:primary"),
                        label: format!("{display_name} ({})", p.kind.label()),
                        models: models.clone(),
                        window: p,
                    });
                    groups.push(ProviderUsageGroup {
                        id: format!("{key}:secondary"),
                        label: format!("{display_name} ({})", s.kind.label()),
                        models,
                        window: s,
                    });
                }
                (Some(window), None) | (None, Some(window)) => {
                    groups.push(ProviderUsageGroup {
                        id: key.clone(),
                        label: display_name.to_string(),
                        models,
                        window,
                    });
                }
                (None, None) => {}
            }
        }
    }

    if primary.is_none() && secondary.is_none() && groups.is_empty() {
        return Ok(None);
    }

    Ok(Some(ProviderPlanUsage {
        provider_id: "codex".to_string(),
        label: Some("Codex".to_string()),
        primary,
        secondary,
        tertiary: None,
        groups,
        credits_available,
        reset_credits_available,
        updated_at_ms: now_ms,
        read_failed: false,
    }))
}

fn parse_codex_window(value: &Json, default_duration_mins: u32) -> Option<UsageWindow> {
    if value.is_null() {
        return None;
    }
    let used_percent = value
        .get("usedPercent")
        .or_else(|| value.get("used_percent"))
        .or_else(|| value.get("percent"))
        .and_then(Json::as_f64)?;
    let percent = used_percent.round().clamp(0.0, 100.0) as u8;
    let duration_mins = value
        .get("windowDurationMins")
        .or_else(|| value.get("window_duration_mins"))
        .or_else(|| value.get("durationMins"))
        .and_then(Json::as_u64)
        .and_then(|n| u32::try_from(n).ok())
        .filter(|&n| n > 0)
        .unwrap_or(default_duration_mins);
    let resets_at = value
        .get("resetsAt")
        .or_else(|| value.get("resets_at"))
        .and_then(parse_timestamp_secs)
        .unwrap_or(0);
    Some(UsageWindow::new(percent, resets_at, duration_mins))
}

fn extract_credits_available(main_bucket: Option<&Json>, payload: &Json) -> Option<bool> {
    let credits = main_bucket
        .and_then(|bucket| bucket.get("credits"))
        .or_else(|| payload.get("credits"))?;
    if credits.is_null() {
        return None;
    }
    let has_credits = credits
        .get("hasCredits")
        .or_else(|| credits.get("has_credits"))
        .and_then(Json::as_bool)
        .unwrap_or(false);
    let unlimited = credits
        .get("unlimited")
        .and_then(Json::as_bool)
        .unwrap_or(false);
    Some(has_credits || unlimited)
}

fn extract_reset_credits(main_bucket: Option<&Json>, payload: &Json) -> Option<u32> {
    let reset_credits = payload
        .get("rateLimitResetCredits")
        .or_else(|| payload.get("rate_limit_reset_credits"))
        .or_else(|| {
            main_bucket.and_then(|bucket| {
                bucket
                    .get("rateLimitResetCredits")
                    .or_else(|| bucket.get("rate_limit_reset_credits"))
            })
        })?;
    let count = reset_credits
        .get("availableCount")
        .or_else(|| reset_credits.get("available_count"))
        .and_then(Json::as_u64)
        .and_then(|n| u32::try_from(n).ok())?;
    (count > 0).then_some(count)
}

fn is_unauthenticated_message(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("not logged in")
        || lower.contains("unauthenticated")
        || lower.contains("authentication required")
        || lower.contains("sign in")
        || lower.contains("no active account")
        || lower.contains("not authenticated")
        || lower.contains("unknown subcommand")
        || lower.contains("unrecognized subcommand")
}

/// Runs the minimal JSON-RPC handshake against a `codex app-server` stdio stream:
/// `initialize` -> `initialized` -> `account/rateLimits/read`.
pub async fn read_codex_via_rpc_io<R, W>(
    mut reader: R,
    mut writer: W,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": INIT_REQUEST_ID,
        "method": "initialize",
        "params": {
            "clientInfo": {
                "name": "vokia",
                "version": env!("CARGO_PKG_VERSION"),
            }
        }
    });
    write_rpc_line(&mut writer, &init_req).await?;

    let mut pushed_usage: Option<ProviderPlanUsage> = None;
    let mut initialized = false;
    let mut line = String::new();

    while !initialized {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .await
            .map_err(|err| UsageReadError::new(format!("Falha ao ler stdout do Codex: {err}")))?;
        if read == 0 {
            return Ok(pushed_usage);
        }
        let Ok(msg) = serde_json::from_str::<Json>(line.trim()) else {
            continue;
        };
        if msg.get("method").and_then(Json::as_str) == Some(METHOD_RATE_LIMITS_UPDATED) {
            if let Ok(Some(usage)) = parse_rate_limits_rpc_message(&msg, now_ms) {
                pushed_usage = Some(usage);
            }
            continue;
        }
        if msg.get("id").and_then(Json::as_u64) == Some(INIT_REQUEST_ID) {
            if let Some(error) = msg.get("error") {
                let message = error
                    .get("message")
                    .and_then(Json::as_str)
                    .unwrap_or("falha ao inicializar codex app-server");
                if is_unauthenticated_message(message) {
                    return Ok(None);
                }
                return Err(UsageReadError::new(format!(
                    "Falha ao inicializar codex app-server: {message}"
                )));
            }
            initialized = true;
        }
    }

    let initialized_note = json!({
        "jsonrpc": "2.0",
        "method": "initialized",
        "params": {}
    });
    write_rpc_line(&mut writer, &initialized_note).await?;

    let read_req = json!({
        "jsonrpc": "2.0",
        "id": RATE_LIMITS_REQUEST_ID,
        "method": METHOD_RATE_LIMITS_READ,
        "params": {}
    });
    write_rpc_line(&mut writer, &read_req).await?;

    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .await
            .map_err(|err| UsageReadError::new(format!("Falha ao ler cota do Codex: {err}")))?;
        if read == 0 {
            return Ok(pushed_usage);
        }
        let Ok(msg) = serde_json::from_str::<Json>(line.trim()) else {
            continue;
        };
        if msg.get("method").and_then(Json::as_str) == Some(METHOD_RATE_LIMITS_UPDATED) {
            if let Ok(Some(usage)) = parse_rate_limits_rpc_message(&msg, now_ms) {
                pushed_usage = Some(usage);
            }
            continue;
        }
        if msg.get("id").and_then(Json::as_u64) == Some(RATE_LIMITS_REQUEST_ID) {
            return parse_rate_limits_rpc_message(&msg, now_ms);
        }
    }
}

async fn write_rpc_line<W: AsyncWrite + Unpin>(
    writer: &mut W,
    value: &Json,
) -> Result<(), UsageReadError> {
    let mut payload = serde_json::to_string(value)
        .map_err(|err| UsageReadError::new(format!("Falha ao serializar RPC do Codex: {err}")))?;
    payload.push('\n');
    writer
        .write_all(payload.as_bytes())
        .await
        .map_err(|err| UsageReadError::new(format!("Falha ao enviar RPC ao Codex: {err}")))?;
    writer
        .flush()
        .await
        .map_err(|err| UsageReadError::new(format!("Falha ao descarregar stdin do Codex: {err}")))
}

fn resolve_codex_program(configured: Option<&str>) -> Option<(PathBuf, Vec<String>)> {
    let raw = configured
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("codex");
    let tokens = shlex::split(raw)?;
    let (program, rest) = tokens.split_first()?;
    let path = which::which(program).ok()?;
    Some((path, rest.to_vec()))
}

/// Queries Codex quota via a short-lived `codex app-server` subprocess (10 s timeout).
/// Returns `Ok(None)` cleanly if the CLI is not installed or the account is not logged in.
pub async fn read_codex_usage(
    config: &CodexReaderConfig,
    now_ms: u64,
) -> Result<Option<ProviderPlanUsage>, UsageReadError> {
    let Some((program, base_args)) = resolve_codex_program(config.command.as_deref()) else {
        return Ok(None);
    };

    let mut cmd = base_command(&program);
    cmd.kill_on_drop(true)
        .args(base_args)
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(UsageReadError::new(format!(
                "Falha ao iniciar codex app-server: {err}"
            )));
        }
    };

    let pid = child.id();
    let Some(stdin) = child.stdin.take() else {
        if let Some(pid) = pid {
            kill_process_tree(pid);
        }
        return Err(UsageReadError::new(
            "codex app-server abriu sem stdin disponível.",
        ));
    };
    let Some(stdout) = child.stdout.take() else {
        if let Some(pid) = pid {
            kill_process_tree(pid);
        }
        return Err(UsageReadError::new(
            "codex app-server abriu sem stdout disponível.",
        ));
    };

    let reader = BufReader::new(stdout);
    let rpc_result = timeout(
        config.timeout,
        read_codex_via_rpc_io(reader, stdin, now_ms),
    )
    .await;

    let mut stderr_text = String::new();
    if let Some(mut stderr) = child.stderr.take() {
        let mut buf = Vec::new();
        let _ = timeout(
            Duration::from_millis(150),
            tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut buf),
        )
        .await;
        stderr_text = String::from_utf8_lossy(&buf).into_owned();
    }

    if let Some(pid) = pid {
        kill_process_tree(pid);
    }
    let _ = child.kill().await;
    let _ = child.wait().await;

    match rpc_result {
        Ok(Ok(usage)) => Ok(usage),
        Ok(Err(err)) => {
            if is_unauthenticated_message(&stderr_text) {
                Ok(None)
            } else {
                Err(err)
            }
        }
        Err(_) => Err(UsageReadError::new(format!(
            "Tempo limite ({}s) excedido ao consultar cota do Codex.",
            config.timeout.as_secs()
        ))),
    }
}
