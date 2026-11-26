use std::path::Path;
use std::sync::atomic::AtomicU32;

use serde_json::Value as Json;
use tokio::process::Command;

use crate::claude::{
    base_command_for_provider, classify_provider_error, Access, ClaudeError, Reply, ToolActivity,
    ToolCategory, ToolStatus, TurnActivity, TurnUsage,
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
    let mut command = base_command_for_provider(program, "antigravity");
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
            for extra_dir in project.extra_dirs {
                command.arg("--add-dir").arg(extra_dir);
            }
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
                    activity.current_step_label = None;
                    console.live_activity(activity.clone());
                } else if matches!(step_type, "tool_use" | "tool_call" | "command" | "web_search") {
                    if let Some(plan) = extract_antigravity_step_plan(&mut plan_state, step) {
                        activity.plan = (!plan.is_empty()).then_some(plan.clone());
                        console.turn_plan(plan);
                    }
                    record_antigravity_step_tool(&mut activity, step, step_type);
                    activity.current_step_label =
                        crate::orchestrator::step_labels::label_for_antigravity_step(step);
                    console.live_activity(activity.clone());
                }
            } else if value.get("type").and_then(Json::as_str) == Some("assistant") {
                if let Some(blocks) = value.pointer("/message/content").and_then(Json::as_array) {
                    let mut updated = false;
                    for block in blocks {
                        if block.get("type").and_then(Json::as_str) == Some("tool_use") {
                            let call_id = block.get("id").and_then(Json::as_str).unwrap_or("");
                            let name = block.get("name").and_then(Json::as_str).unwrap_or("");
                            if let Some(plan) = extract_tool_plan(
                                &mut plan_state,
                                call_id,
                                name,
                                &block["input"],
                            ) {
                                activity.plan = (!plan.is_empty()).then_some(plan.clone());
                                console.turn_plan(plan);
                            }
                            upsert_antigravity_tool(
                                &mut activity,
                                call_id,
                                name,
                                &block["input"],
                                ToolStatus::Running,
                                None,
                            );
                            activity.current_step_label =
                                crate::orchestrator::step_labels::label_for_tool(name, &block["input"]);
                            updated = true;
                        }
                    }
                    if updated {
                        console.live_activity(activity.clone());
                    }
                }
            } else if value.get("type").and_then(Json::as_str) == Some("user") {
                if let Some(blocks) = value.pointer("/message/content").and_then(Json::as_array) {
                    let outcome = value.get("tool_use_result");
                    for block in blocks {
                        if block.get("type").and_then(Json::as_str) == Some("tool_result") {
                            if let Some(id) = block.get("tool_use_id").and_then(Json::as_str) {
                                let is_error =
                                    block.get("is_error").and_then(Json::as_bool) == Some(true);
                                let result_text = block
                                    .get("content")
                                    .and_then(Json::as_str)
                                    .map(str::to_string)
                                    .or_else(|| block.get("content").map(|v| v.to_string()));
                                if let Some(existing) =
                                    activity.tools.iter_mut().find(|tool| tool.call_id == id)
                                {
                                    existing.status = if is_error {
                                        ToolStatus::Failed
                                    } else {
                                        ToolStatus::Ok
                                    };
                                    if result_text.is_some() {
                                        existing.result = result_text;
                                    }
                                    console.live_activity(activity.clone());
                                }
                                if let Some(plan) =
                                    crate::orchestrator::turn_plan::fold_claude_plan_result(
                                        &mut plan_state,
                                        id,
                                        block.get("content"),
                                        outcome,
                                        is_error,
                                    )
                                {
                                    activity.plan = (!plan.is_empty()).then_some(plan.clone());
                                    console.turn_plan(plan);
                                    console.live_activity(activity.clone());
                                }
                            }
                        }
                    }
                }
            }
        },
    )
    .await?;
    let mut reply = parse_stream_output_with_provider(&stdout, Some(provider), model)?;
    activity.current_step_label = None;
    reply.activity = activity;
    Ok(reply)
}

fn extract_antigravity_step_plan(
    state: &mut crate::orchestrator::turn_plan::ClaudePlanState,
    step: &Json,
) -> Option<crate::orchestrator::turn_plan::TurnPlan> {
    let (call_id, name, input) = extract_antigravity_step_tool(step)?;
    extract_tool_plan(state, call_id, name, &input)
}

fn extract_antigravity_step_tool(step: &Json) -> Option<(&str, &str, Json)> {
    let name = step
        .get("tool_name")
        .or_else(|| step.get("name"))
        .or_else(|| step.get("tool"))
        .or_else(|| step.pointer("/tool_call/name"))
        .or_else(|| step.pointer("/tool_use/name"))
        .and_then(Json::as_str)?;
    let call_id = step
        .get("tool_call_id")
        .or_else(|| step.get("id"))
        .and_then(Json::as_str)
        .unwrap_or("");
    let raw_input = step
        .get("tool_input")
        .or_else(|| step.get("input"))
        .or_else(|| step.get("arguments"))
        .or_else(|| step.get("args"))
        .or_else(|| step.pointer("/tool_call/input"))
        .or_else(|| step.pointer("/tool_call/arguments"))
        .or_else(|| step.pointer("/tool_use/input"));
    let parsed_string = raw_input
        .and_then(Json::as_str)
        .and_then(|s| serde_json::from_str::<Json>(s).ok());
    let input = parsed_string
        .or_else(|| raw_input.cloned())
        .unwrap_or(Json::Null);
    Some((call_id, name, input))
}

fn record_antigravity_step_tool(activity: &mut TurnActivity, step: &Json, step_type: &str) {
    let status = match step
        .get("status")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "completed" | "done" | "ok" | "success" => ToolStatus::Ok,
        "failed" | "error" => ToolStatus::Failed,
        _ => ToolStatus::Ok,
    };
    let result = step
        .get("output")
        .or_else(|| step.get("result"))
        .and_then(|v| v.as_str().map(str::to_string).or_else(|| Some(v.to_string())));

    if let Some((call_id, name, input)) = extract_antigravity_step_tool(step) {
        upsert_antigravity_tool(activity, call_id, name, &input, status, result);
        return;
    }
    if step_type == "command" {
        if let Some(cmd) = step.get("command").and_then(Json::as_str) {
            let call_id = step.get("id").and_then(Json::as_str).unwrap_or("");
            upsert_antigravity_tool(
                activity,
                call_id,
                "run_command",
                &serde_json::json!({ "command": cmd }),
                status,
                result,
            );
        }
    } else if step_type == "web_search" {
        if let Some(query) = step.get("query").and_then(Json::as_str) {
            let call_id = step.get("id").and_then(Json::as_str).unwrap_or("");
            upsert_antigravity_tool(
                activity,
                call_id,
                "web_search",
                &serde_json::json!({ "query": query }),
                status,
                result,
            );
        }
    }
}

fn upsert_antigravity_tool(
    activity: &mut TurnActivity,
    call_id: &str,
    name: &str,
    input: &Json,
    status: ToolStatus,
    result: Option<String>,
) {
    let lower = name.to_ascii_lowercase();
    let category = if lower.starts_with("mcp__vokia__") {
        ToolCategory::App
    } else if lower.starts_with("mcp__") {
        ToolCategory::Mcp
    } else if lower.contains("command") || lower == "bash" || lower == "shell" {
        ToolCategory::Command
    } else if lower.contains("edit") || lower.contains("write") {
        ToolCategory::Edit
    } else if lower.contains("read") {
        ToolCategory::Read
    } else if lower.contains("list") || lower == "glob" {
        ToolCategory::List
    } else if lower.contains("web") {
        ToolCategory::Web
    } else if lower.contains("grep") || lower.contains("search") {
        ToolCategory::Search
    } else {
        ToolCategory::Other
    };
    let target = input
        .get("command")
        .or_else(|| input.get("CommandLine"))
        .or_else(|| input.get("file_path"))
        .or_else(|| input.get("path"))
        .or_else(|| input.get("Path"))
        .or_else(|| input.get("TargetFile"))
        .or_else(|| input.get("query"))
        .or_else(|| input.get("url"))
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(name)
        .to_string();

    if !call_id.is_empty() {
        if let Some(existing) = activity.tools.iter_mut().find(|t| t.call_id == call_id) {
            existing.status = status;
            if result.is_some() {
                existing.result = result;
            }
            return;
        }
    }
    activity.tools.push(ToolActivity {
        call_id: if call_id.is_empty() {
            format!("agy-{}", activity.tools.len() + 1)
        } else {
            call_id.to_string()
        },
        name: name.to_string(),
        category,
        target,
        status,
        description: None,
        added: None,
        removed: None,
        change: None,
        duration_ms: None,
        detail: None,
        result,
        exit_code: None,
    });
}

fn extract_tool_plan(
    state: &mut crate::orchestrator::turn_plan::ClaudePlanState,
    call_id: &str,
    name: &str,
    input: &Json,
) -> Option<crate::orchestrator::turn_plan::TurnPlan> {
    if name.eq_ignore_ascii_case("update_steps") {
        let steps = input.get("steps").and_then(Json::as_array)?;
        let plan = crate::orchestrator::turn_plan::plan_from_acp_entries(steps);
        return (!plan.is_empty()).then_some(plan);
    }
    crate::orchestrator::turn_plan::fold_claude_plan_call(state, call_id, name, input)
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
                        .or_else(|| result.pointer("/usage/cache_read_input_tokens"))
                        .and_then(Json::as_u64);
                    let cache_write_tokens = result
                        .pointer("/usage/cache_write_tokens")
                        .or_else(|| result.pointer("/usage/cache_creation_input_tokens"))
                        .and_then(Json::as_u64);
                    let thought_tokens = result
                        .pointer("/usage/thought_tokens")
                        .or_else(|| result.pointer("/usage/thoughts_tokens"))
                        .or_else(|| result.pointer("/usage/thoughtsTokenCount"))
                        .or_else(|| result.pointer("/usage/reasoning_tokens"))
                        .and_then(Json::as_u64);
                    if input_tokens.is_some()
                        || output_tokens.is_some()
                        || cached_input_tokens.is_some()
                        || cache_write_tokens.is_some()
                        || thought_tokens.is_some()
                    {
                        let resolved_model = (!model.trim().is_empty()).then(|| model.to_string());
                        let resolved_cost = estimate_antigravity_cost_usd(
                            resolved_model.as_deref(),
                            input_tokens,
                            output_tokens,
                            cached_input_tokens,
                            cache_write_tokens,
                            thought_tokens,
                        );
                        usage = Some(TurnUsage {
                            provider: provider.map(|p| p.id.clone()),
                            model: resolved_model,
                            input_tokens,
                            output_tokens,
                            cached_input_tokens,
                            cache_write_tokens,
                            thought_tokens,
                            cost_usd: resolved_cost,
                            is_fallback: None,
                            ..Default::default()
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
                let cache_write_tokens = value
                    .pointer("/usage/cache_write_tokens")
                    .or_else(|| value.pointer("/usage/cache_creation_input_tokens"))
                    .and_then(Json::as_u64);
                let thought_tokens = value
                    .pointer("/usage/thought_tokens")
                    .or_else(|| value.pointer("/usage/thoughts_tokens"))
                    .or_else(|| value.pointer("/usage/thoughtsTokenCount"))
                    .or_else(|| value.pointer("/usage/reasoning_tokens"))
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
                        cache_write_tokens,
                        thought_tokens,
                    )
                });
                if resolved_model.is_some()
                    || input_tokens.is_some()
                    || output_tokens.is_some()
                    || cached_input_tokens.is_some()
                    || cache_write_tokens.is_some()
                    || thought_tokens.is_some()
                    || resolved_cost.is_some()
                {
                    usage = Some(TurnUsage {
                        provider: provider.map(|p| p.id.clone()),
                        is_fallback: None,
                        model: resolved_model,
                        input_tokens,
                        output_tokens,
                        cached_input_tokens,
                        cache_write_tokens,
                        thought_tokens,
                        cost_usd: resolved_cost,
                        ..Default::default()
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
    cache_write: Option<u64>,
    thought: Option<u64>,
) -> Option<f64> {
    let counts = crate::db::usage_pricing::TokenCounts {
        input_tokens: input.unwrap_or(0),
        cached_input_tokens: cached.unwrap_or(0),
        cache_write_tokens: cache_write.unwrap_or(0),
        output_tokens: output.unwrap_or(0),
        thought_tokens: thought.unwrap_or(0),
    };
    if counts.total() == 0 {
        return None;
    }
    let model = model?;
    crate::db::usage_pricing::estimate_usage_price(model, counts).map(|est| est.cost_usd)
}
