use std::path::Path;
use std::sync::atomic::AtomicU32;

use serde_json::Value as Json;
use tokio::process::Command;

use crate::claude::{
    base_command, classify_provider_error, Access, ClaudeError, Reply, TurnActivity, TurnUsage,
};
use crate::mcp::McpSession;
use crate::permissions::PermissionLevel;
use crate::providers::ProviderConfig;

#[cfg_attr(not(test), allow(dead_code))] // usado só nos testes
pub(crate) fn build_command(
    program: &Path,
    user_args: &[String],
    role: &str,
    model: &str,
    access: Access<'_>,
    message: &str,
) -> Command {
    build_command_with_resume(program, user_args, role, model, access, message, None)
}

pub(crate) fn build_command_with_resume(
    program: &Path,
    user_args: &[String],
    _role: &str,
    model: &str,
    access: Access<'_>,
    _message: &str,
    resume_conversation: Option<&str>,
) -> Command {
    let mut command = base_command(program);
    command.kill_on_drop(true);
    command.args(user_args);
    command.args([
        "-p=",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--model",
        model,
        "--disable-slash-commands",
    ]);
    if let Some(conv_id) = resume_conversation.map(str::trim).filter(|s| !s.is_empty()) {
        command.args(["--conversation", conv_id]);
    }
    match access {
        Access::TextOnly => {
            command.args(["--sandbox", "--mode", "plan"]);
        }
        Access::Project(project) => {
            command.current_dir(project.dir);
            command.arg("--add-dir").arg(project.dir);
            command.arg("--add-dir").arg(project.attachments);
            match project.level {
                PermissionLevel::Leitura => {
                    command.args(["--sandbox", "--mode", "plan"]);
                }
                PermissionLevel::Edicao => {
                    command.args(["--sandbox", "--mode", "accept-edits"]);
                }
                PermissionLevel::Liberado => {
                    command.arg("--dangerously-skip-permissions");
                }
            }
        }
    }
    command
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_turn(
    program: &Path,
    user_args: &[String],
    role: &str,
    model: &str,
    access: Access<'_>,
    message: &str,
    pid: &AtomicU32,
    provider: &ProviderConfig,
    vokia_cli: Option<(&McpSession, &Path)>,
    resume_conversation: Option<&str>,
    console: &super::ConsoleSink<'_>,
) -> Result<Reply, ClaudeError> {
    let mut command =
        build_command_with_resume(program, user_args, role, model, access, message, resume_conversation);
    if let Some((session, cli_path)) = vokia_cli {
        super::inject_vokia_cli_env(&mut command, session, cli_path);
    }
    // O agy exige o valor colado ("-p=") e, com stream-json, lê uma mensagem NDJSON por linha no stdin.
    let prompt = format!(
        "{}\n",
        serde_json::json!({"event": "user", "message": {"content": format!("{role}\n\n{message}")}})
    );
    let mut activity = TurnActivity::default();
    let mut last_at = std::time::Instant::now();
    let stdout = super::run_cli_process(
        command,
        access,
        pid,
        Some(prompt),
        Some((provider, model)),
        |line| {
            let now = std::time::Instant::now();
            let elapsed_ms = (now.saturating_duration_since(last_at).as_millis() as u64).max(100);
            last_at = now;
            let Ok(value) = serde_json::from_str::<Json>(line.trim()) else {
                return;
            };
            if value.get("event").and_then(Json::as_str) == Some("step_update") {
                let step = &value["step_update"];
                let step_type = step.get("step_type").and_then(Json::as_str).unwrap_or("");
                if matches!(step_type, "thinking" | "thought" | "reasoning") {
                    if let Some(delta) = step
                        .get("text_delta")
                        .or_else(|| step.get("text"))
                        .and_then(Json::as_str)
                        .filter(|s| !s.is_empty())
                    {
                        match &mut activity.thinking_text {
                            Some(existing) => existing.push_str(delta),
                            None => activity.thinking_text = Some(delta.to_string()),
                        }
                    }
                    activity.thinking_ms = Some(activity.thinking_ms.unwrap_or(0).saturating_add(elapsed_ms));
                    console.live_activity(activity.clone());
                } else if matches!(step_type, "tool_use" | "tool_call") {
                    console.live_activity(activity.clone());
                }
            }
        },
    )
    .await?;
    parse_stream_output_with_provider(&stdout, Some(provider), model)
}

#[cfg_attr(not(test), allow(dead_code))] // usado só nos testes
pub(crate) fn parse_stream_output(stdout: &[u8]) -> Result<Reply, ClaudeError> {
    parse_stream_output_with_provider(stdout, None, "")
}

pub(crate) fn parse_stream_output_with_provider(
    stdout: &[u8],
    provider: Option<&ProviderConfig>,
    model: &str,
) -> Result<Reply, ClaudeError> {
    let raw = String::from_utf8_lossy(stdout);
    let mut result_text: Option<String> = None;
    let mut assistant_chunks: Vec<String> = Vec::new();
    let mut delta_text = String::new();
    let mut saw_json = false;
    let mut assistant_model: Option<String> = None;
    let mut usage: Option<TurnUsage> = None;
    let mut found_session_id: Option<String> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Json>(trimmed) else {
            continue;
        };
        saw_json = true;
        if found_session_id.is_none() {
            if let Some(sid) = value
                .get("conversation_id")
                .or_else(|| value.get("session_id"))
                .or_else(|| value.pointer("/result/conversation_id"))
                .or_else(|| value.pointer("/result/session_id"))
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                found_session_id = Some(sid.to_string());
            }
        }
        if let Some(event) = value.get("event").and_then(Json::as_str) {
            match event {
                "step_update" => {
                    let step = &value["step_update"];
                    if step.get("step_type").and_then(Json::as_str) == Some("agent_response") {
                        if let Some(delta) = step.get("text_delta").and_then(Json::as_str) {
                            delta_text.push_str(delta);
                        }
                    }
                }
                "result" => {
                    let result = &value["result"];
                    let status = result.get("status").and_then(Json::as_str).unwrap_or("");
                    if status.eq_ignore_ascii_case("ERROR") {
                        let message = result
                            .get("error")
                            .and_then(Json::as_str)
                            .filter(|s| !s.is_empty())
                            .unwrap_or("erro desconhecido no Antigravity");
                        return Err(event_error(provider, model, message));
                    }
                    if let Some(text) = result.get("response").and_then(Json::as_str) {
                        if !text.trim().is_empty() {
                            result_text = Some(text.trim_end().to_string());
                        }
                    }
                    let input_tokens = result.pointer("/usage/input_tokens").and_then(Json::as_u64);
                    let output_tokens =
                        result.pointer("/usage/output_tokens").and_then(Json::as_u64);
                    let cached_input_tokens = result
                        .pointer("/usage/cache_read_tokens")
                        .and_then(Json::as_u64);
                    if input_tokens.is_some()
                        || output_tokens.is_some()
                        || cached_input_tokens.is_some()
                    {
                        let resolved_model = (!model.trim().is_empty()).then(|| model.to_string());
                        let resolved_cost = estimate_antigravity_cost_usd(
                            resolved_model.as_deref(),
                            input_tokens,
                            output_tokens,
                            cached_input_tokens,
                        );
                        usage = Some(TurnUsage {
                            provider: provider.map(|p| p.id.clone()),
                            model: resolved_model,
                            input_tokens,
                            output_tokens,
                            cached_input_tokens,
                            cost_usd: resolved_cost,
                            is_fallback: None,
                        });
                    }
                }
                _ => {}
            }
            continue;
        }
        let Some(kind) = value.get("type").and_then(Json::as_str) else {
            continue;
        };
        match kind {
            "result" => {
                let is_error = value
                    .get("is_error")
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                let text = value.get("result").and_then(Json::as_str).unwrap_or("");
                if is_error {
                    let message = if text.is_empty() {
                        "erro desconhecido no Antigravity"
                    } else {
                        text
                    };
                    if let Some(provider) = provider {
                        if let Some(classified) = classify_provider_error(
                            &provider.id,
                            model,
                            message,
                            &provider.rate_limit_patterns,
                            &provider.model_unavailable_patterns,
                        ) {
                            return Err(classified);
                        }
                    }
                    return Err(ClaudeError::Reported(message.to_string()));
                }
                if !text.is_empty() {
                    result_text = Some(text.to_string());
                }
                let parsed_model = value
                    .get("model")
                    .and_then(Json::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| assistant_model.clone());
                let input_tokens = value
                    .pointer("/usage/input_tokens")
                    .or_else(|| value.pointer("/usage/promptTokenCount"))
                    .and_then(Json::as_u64);
                let output_tokens = value
                    .pointer("/usage/output_tokens")
                    .or_else(|| value.pointer("/usage/candidatesTokenCount"))
                    .and_then(Json::as_u64);
                let cached_input_tokens = value
                    .pointer("/usage/cache_read_input_tokens")
                    .or_else(|| value.pointer("/usage/cachedContentTokenCount"))
                    .and_then(Json::as_u64);
                let cost_usd = value
                    .get("total_cost_usd")
                    .or_else(|| value.get("cost_usd"))
                    .and_then(Json::as_f64);
                let resolved_model = parsed_model.or_else(|| {
                    (!model.trim().is_empty()).then(|| model.to_string())
                });
                let resolved_cost = cost_usd.or_else(|| {
                    estimate_antigravity_cost_usd(
                        resolved_model.as_deref(),
                        input_tokens,
                        output_tokens,
                        cached_input_tokens,
                    )
                });
                if resolved_model.is_some()
                    || input_tokens.is_some()
                    || output_tokens.is_some()
                    || cached_input_tokens.is_some()
                    || resolved_cost.is_some()
                {
                    usage = Some(TurnUsage {
                        provider: provider.map(|p| p.id.clone()),
                        is_fallback: None,
                        model: resolved_model,
                        input_tokens,
                        output_tokens,
                        cached_input_tokens,
                        cost_usd: resolved_cost,
                    });
                }
            }
            "assistant" => {
                if let Some(m) = value
                    .pointer("/message/model")
                    .and_then(Json::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    assistant_model = Some(m.to_string());
                }
                if let Some(text) = extract_assistant_text(&value) {
                    if !text.trim().is_empty() {
                        assistant_chunks.push(text);
                    }
                }
            }
            "error" => {
                let message = value["error"]["message"]
                    .as_str()
                    .or_else(|| value["message"].as_str())
                    .unwrap_or("erro desconhecido no Antigravity");
                if let Some(provider) = provider {
                    if let Some(classified) = classify_provider_error(
                        &provider.id,
                        model,
                        message,
                        &provider.rate_limit_patterns,
                        &provider.model_unavailable_patterns,
                    ) {
                        return Err(classified);
                    }
                }
                return Err(ClaudeError::Reported(message.to_string()));
            }
            _ => {}
        }
    }

    let text = if let Some(text) = result_text {
        text
    } else if !assistant_chunks.is_empty() {
        assistant_chunks.join("\n\n")
    } else if !delta_text.trim().is_empty() {
        delta_text.trim_end().to_string()
    } else if !saw_json {
        raw.trim().to_string()
    } else {
        String::new()
    };

    if usage.is_none() && assistant_model.is_some() {
        usage = Some(TurnUsage {
            provider: provider.map(|p| p.id.clone()),
            model: assistant_model,
            ..Default::default()
        });
    }

    Ok(Reply {
        text,
        limits: None,
        activity: TurnActivity::default(),
        usage,
        session_id: found_session_id,
    })
}

fn event_error(provider: Option<&ProviderConfig>, model: &str, message: &str) -> ClaudeError {
    if let Some(provider) = provider {
        if let Some(classified) = classify_provider_error(
            &provider.id,
            model,
            message,
            &provider.rate_limit_patterns,
            &provider.model_unavailable_patterns,
        ) {
            return classified;
        }
    }
    ClaudeError::Reported(message.to_string())
}

fn extract_assistant_text(value: &Json) -> Option<String> {
    let content = value.get("message")?.get("content")?;
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }
    let blocks = content.as_array()?;
    let parts: Vec<&str> = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Json::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Json::as_str))
        .collect();
    (!parts.is_empty()).then(|| parts.join(""))
}

fn estimate_antigravity_cost_usd(
    model: Option<&str>,
    input: Option<u64>,
    output: Option<u64>,
    cached: Option<u64>,
) -> Option<f64> {
    let inp = input.unwrap_or(0) as f64;
    let out = output.unwrap_or(0) as f64;
    let cac = cached.unwrap_or(0) as f64;
    if inp <= 0.0 && out <= 0.0 && cac <= 0.0 {
        return None;
    }
    let is_flash = model
        .map(|m| m.to_ascii_lowercase().contains("flash"))
        .unwrap_or(false);
    let (in_rate, out_rate, cache_rate) = if is_flash {
        (0.15 / 1_000_000.0, 0.60 / 1_000_000.0, 0.0375 / 1_000_000.0)
    } else {
        (1.25 / 1_000_000.0, 5.00 / 1_000_000.0, 0.3125 / 1_000_000.0)
    };
    Some(inp * in_rate + out * out_rate + cac * cache_rate)
}
