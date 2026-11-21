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

pub(crate) fn build_command(
    program: &Path,
    user_args: &[String],
    _role: &str,
    model: &str,
    access: Access<'_>,
    _message: &str,
) -> Command {
    let mut command = base_command(program);
    command.kill_on_drop(true);
    command.args(user_args);
    command.args([
        "-p",
        "--input-format",
        "text",
        "--output-format",
        "stream-json",
        "--model",
        model,
        "--disable-slash-commands",
    ]);
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
) -> Result<Reply, ClaudeError> {
    let mut command = build_command(program, user_args, role, model, access, message);
    if let Some((session, cli_path)) = vokia_cli {
        super::inject_vokia_cli_env(&mut command, session, cli_path);
    }
    let prompt = format!("{role}\n\n{message}");
    let stdout =
        super::run_cli_process(command, access, pid, Some(prompt), Some((provider, model))).await?;
    parse_stream_output_with_provider(&stdout, Some(provider), model)
}

#[allow(dead_code)]
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
    let mut saw_json = false;
    let mut assistant_model: Option<String> = None;
    let mut usage: Option<TurnUsage> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Json>(trimmed) else {
            continue;
        };
        saw_json = true;
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
                if parsed_model.is_some()
                    || input_tokens.is_some()
                    || output_tokens.is_some()
                    || cached_input_tokens.is_some()
                    || cost_usd.is_some()
                {
                    usage = Some(TurnUsage {
                        provider: provider.map(|p| p.id.clone()),
                        model: parsed_model.or_else(|| {
                            (!model.trim().is_empty()).then(|| model.to_string())
                        }),
                        input_tokens,
                        output_tokens,
                        cached_input_tokens,
                        cost_usd,
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
    })
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
