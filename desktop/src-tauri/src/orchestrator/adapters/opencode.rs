use std::path::Path;

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
) -> Result<Command, ClaudeError> {
    build_command_with_resume(program, user_args, role, model, access, message, None)
}

pub(crate) fn build_command_with_resume(
    program: &Path,
    user_args: &[String],
    _role: &str,
    model: &str,
    access: Access<'_>,
    _message: &str,
    resume_session: Option<&str>,
) -> Result<Command, ClaudeError> {
    let mut command = base_command(program);
    command.kill_on_drop(true);
    command.args(user_args);
    command.args(["run", "--pure", "--format", "json"]);
    command.args(["-m", model]);
    if let Some(sid) = resume_session.map(str::trim).filter(|s| !s.is_empty()) {
        command.args(["-s", sid]);
    }
    if let Access::Project(project) = access {
        if project.level == PermissionLevel::Liberado {
            return Err(ClaudeError::Reported(
                "OpenCode não oferece sandbox nem controle de aprovação equivalente para o nível Liberado."
                    .to_string(),
            ));
        }
        command.current_dir(project.dir);
        command.arg("--dir").arg(project.dir);
    }
    Ok(command)
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn run_turn(
    program: &Path,
    user_args: &[String],
    role: &str,
    model: &str,
    access: Access<'_>,
    message: &str,
    pid: &std::sync::atomic::AtomicU32,
    provider: &ProviderConfig,
    vokia_cli: Option<(&McpSession, &Path)>,
    resume_session: Option<&str>,
    console: &super::ConsoleSink<'_>,
) -> Result<Reply, ClaudeError> {
    let mut command =
        build_command_with_resume(program, user_args, role, model, access, message, resume_session)?;
    if let Some((session, cli_path)) = vokia_cli {
        super::inject_vokia_cli_env(&mut command, session, cli_path);
    }
    let prompt = format!("{role}\n\n{message}");
    let mut activity = TurnActivity::default();
    let mut plan_state = crate::orchestrator::turn_plan::ClaudePlanState::default();
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
            if let Some(plan) = crate::orchestrator::turn_plan::plan_from_acp_notification(&value) {
                activity.plan = (!plan.is_empty()).then_some(plan.clone());
                console.turn_plan(plan);
                console.live_activity(activity.clone());
                return;
            }
            let kind = value.get("type").and_then(Json::as_str).unwrap_or("");
            let part_type = value["part"].get("type").and_then(Json::as_str).unwrap_or("");
            if matches!(kind, "reasoning" | "thinking") || part_type == "reasoning" {
                let text = value["part"]["text"]
                    .as_str()
                    .or_else(|| value["text"].as_str())
                    .or_else(|| value["content"].as_str());
                if let Some(t) = text.filter(|s| !s.is_empty()) {
                    match &mut activity.thinking_text {
                        Some(existing) => {
                            existing.push('\n');
                            existing.push_str(t);
                        }
                        None => activity.thinking_text = Some(t.to_string()),
                    }
                }
                activity.thinking_ms = Some(activity.thinking_ms.unwrap_or(0).saturating_add(elapsed_ms));
                activity.current_step_label = None;
                console.live_activity(activity.clone());
            } else if kind == "tool_use" || part_type == "tool_use" || part_type == "tool" {
                if let Some(plan) = extract_opencode_tool_plan(&mut plan_state, &value) {
                    activity.plan = (!plan.is_empty()).then_some(plan.clone());
                    console.turn_plan(plan);
                }
                activity.current_step_label =
                    crate::orchestrator::step_labels::label_for_opencode_event(&value);
                console.live_activity(activity.clone());
            }
        },
    )
    .await?;
    let mut reply = parse_json_output_with_provider(&stdout, Some(provider), model)?;
    activity.current_step_label = None;
    reply.activity = activity;
    Ok(reply)
}

fn extract_opencode_tool_plan(
    state: &mut crate::orchestrator::turn_plan::ClaudePlanState,
    value: &Json,
) -> Option<crate::orchestrator::turn_plan::TurnPlan> {
    let part = &value["part"];
    let name = part
        .get("tool")
        .or_else(|| part.get("name"))
        .or_else(|| value.get("tool"))
        .or_else(|| value.get("name"))
        .and_then(Json::as_str)?;
    let call_id = part
        .get("callID")
        .or_else(|| part.get("id"))
        .or_else(|| value.get("id"))
        .and_then(Json::as_str)
        .unwrap_or("");
    let raw_input = part
        .pointer("/state/input")
        .or_else(|| part.get("input"))
        .or_else(|| part.get("arguments"))
        .or_else(|| value.get("input"))
        .or_else(|| value.get("arguments"));
    let parsed_string = raw_input
        .and_then(Json::as_str)
        .and_then(|s| serde_json::from_str::<Json>(s).ok());
    let input = parsed_string
        .as_ref()
        .or(raw_input)
        .unwrap_or(&Json::Null);
    crate::orchestrator::turn_plan::fold_claude_plan_call(state, call_id, name, input)
}

#[cfg_attr(not(test), allow(dead_code))] // usado só nos testes
pub(crate) fn parse_json_output(stdout: &[u8]) -> Result<Reply, ClaudeError> {
    parse_json_output_with_provider(stdout, None, "")
}

pub(crate) fn parse_json_output_with_provider(
    stdout: &[u8],
    provider: Option<&ProviderConfig>,
    model: &str,
) -> Result<Reply, ClaudeError> {
    let raw = String::from_utf8_lossy(stdout);
    let mut texts: Vec<String> = Vec::new();
    let mut saw_json = false;
    let mut acc_input: Option<u64> = None;
    let mut acc_output: Option<u64> = None;
    let mut acc_cached: Option<u64> = None;
    let mut acc_cache_write: Option<u64> = None;
    let mut acc_thought: Option<u64> = None;
    let mut acc_cost: Option<f64> = None;
    let mut found_model: Option<String> = None;
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
                .get("sessionID")
                .or_else(|| value.get("session_id"))
                .or_else(|| value.get("session"))
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                found_session_id = Some(sid.to_string());
            }
        }
        let Some(kind) = value.get("type").and_then(Json::as_str) else {
            continue;
        };
        match kind {
            "text" => {
                let text = value["part"]["text"]
                    .as_str()
                    .or_else(|| value["text"].as_str())
                    .or_else(|| value["content"].as_str());
                if let Some(text) = text {
                    if !text.trim().is_empty() {
                        texts.push(text.to_string());
                    }
                }
            }
            "step_finish" | "result" => {
                if let Some(m) = value
                    .pointer("/part/model")
                    .or_else(|| value.get("model"))
                    .and_then(Json::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    found_model = Some(m.to_string());
                }
                if let Some(n) = value
                    .pointer("/part/tokens/input")
                    .or_else(|| value.pointer("/usage/input_tokens"))
                    .and_then(Json::as_u64)
                {
                    acc_input = Some(acc_input.unwrap_or(0) + n);
                }
                if let Some(n) = value
                    .pointer("/part/tokens/output")
                    .or_else(|| value.pointer("/usage/output_tokens"))
                    .and_then(Json::as_u64)
                {
                    acc_output = Some(acc_output.unwrap_or(0) + n);
                }
                if let Some(n) = value
                    .pointer("/part/tokens/cache/read")
                    .or_else(|| value.pointer("/usage/cache_read_input_tokens"))
                    .and_then(Json::as_u64)
                {
                    acc_cached = Some(acc_cached.unwrap_or(0) + n);
                }
                if let Some(n) = value
                    .pointer("/part/tokens/cache/write")
                    .or_else(|| value.pointer("/usage/cache_creation_input_tokens"))
                    .and_then(Json::as_u64)
                {
                    acc_cache_write = Some(acc_cache_write.unwrap_or(0) + n);
                }
                if let Some(n) = value
                    .pointer("/part/tokens/reasoning")
                    .or_else(|| value.pointer("/usage/reasoning_tokens"))
                    .and_then(Json::as_u64)
                {
                    acc_thought = Some(acc_thought.unwrap_or(0) + n);
                }
                if let Some(c) = value
                    .pointer("/part/cost")
                    .or_else(|| value.get("total_cost_usd"))
                    .or_else(|| value.get("cost_usd"))
                    .and_then(Json::as_f64)
                {
                    acc_cost = Some(acc_cost.unwrap_or(0.0) + c);
                }
            }
            "error" => {
                let message = value["error"]["data"]["message"]
                    .as_str()
                    .or_else(|| value["error"]["message"].as_str())
                    .or_else(|| value["message"].as_str())
                    .unwrap_or("erro desconhecido no OpenCode");
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

    let text = if !texts.is_empty() {
        texts.join("\n\n")
    } else if !saw_json {
        raw.trim().to_string()
    } else {
        String::new()
    };

    let usage = if acc_input.is_some()
        || acc_output.is_some()
        || acc_cached.is_some()
        || acc_cache_write.is_some()
        || acc_thought.is_some()
        || acc_cost.is_some()
        || found_model.is_some()
    {
        Some(TurnUsage {
            provider: provider.map(|p| p.id.clone()),
            model: found_model.or_else(|| (!model.trim().is_empty()).then(|| model.to_string())),
            input_tokens: acc_input,
            output_tokens: acc_output,
            cached_input_tokens: acc_cached,
            cache_write_tokens: acc_cache_write,
            thought_tokens: acc_thought,
            cost_usd: acc_cost,
            is_fallback: None,
            ..Default::default()
        })
    } else {
        None
    };

    Ok(Reply {
        text,
        limits: None,
        activity: TurnActivity::default(),
        usage,
        session_id: found_session_id,
    })
}
