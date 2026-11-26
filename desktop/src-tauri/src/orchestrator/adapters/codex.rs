use std::path::Path;
use std::sync::atomic::AtomicU32;

use serde_json::Value as Json;
use tokio::process::Command;

use crate::claude::{
    base_command_for_provider, classify_provider_error, Access, ClaudeError, Reply, ToolActivity, ToolCategory,
    ToolStatus, TurnActivity, TurnUsage,
};
use crate::mcp::McpSession;
use crate::permissions::PermissionLevel;
use crate::providers::ProviderConfig;

const MCP_TOKEN_ENV: &str = "VOKIA_MCP_TOKEN";

pub(crate) fn build_command(
    program: &Path,
    user_args: &[String],
    model: &str,
    access: Access<'_>,
    mcp: Option<&McpSession>,
) -> Command {
    let mut command = base_command_for_provider(program, "codex");
    command.kill_on_drop(true);
    command.args(user_args);
    command.args(["exec", "--json", "--skip-git-repo-check"]);
    command.args(["-m", model]);
    match access {
        Access::TextOnly => {
            command.args(["-s", "read-only"]);
        }
        Access::Project(project) => {
            command.current_dir(project.dir);
            command.arg("-C").arg(project.dir);
            command.arg("--add-dir").arg(project.attachments);
            let sandbox = match project.level {
                PermissionLevel::Leitura => "read-only",
                PermissionLevel::Edicao => "workspace-write",
                PermissionLevel::Liberado => "danger-full-access",
            };
            command.args(["-s", sandbox]);
        }
    }
    if let Some(session) = mcp {
        command.env(MCP_TOKEN_ENV, session.token());
        command.args(["-c", &format!("mcp_servers.vokia.url=\"{}\"", session.url())]);
        command.args([
            "-c",
            &format!("mcp_servers.vokia.bearer_token_env_var=\"{MCP_TOKEN_ENV}\""),
        ]);
    }
    command.arg("-");
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
    mcp: Option<&McpSession>,
    console: &super::ConsoleSink<'_>,
) -> Result<Reply, ClaudeError> {
    let command = build_command(program, user_args, model, access, mcp);
    let prompt = format!("{role}\n\n{message}");
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
            if let Some(plan) = crate::orchestrator::turn_plan::plan_from_codex_notification(&value)
            {
                activity.plan = (!plan.is_empty()).then_some(plan.clone());
                console.turn_plan(plan);
                console.live_activity(activity.clone());
                return;
            }
            let kind = value.get("type").and_then(Json::as_str).unwrap_or("");
            if matches!(kind, "item.completed" | "item.updated" | "item.started") {
                let item = &value["item"];
                let item_type = item.get("type").and_then(Json::as_str).unwrap_or("");
                if item_type == "reasoning" {
                    if let Some(text) = item.get("text").and_then(Json::as_str).filter(|s| !s.is_empty()) {
                        match &mut activity.thinking_text {
                            Some(existing) => {
                                existing.push('\n');
                                existing.push_str(text);
                            }
                            None => activity.thinking_text = Some(text.to_string()),
                        }
                    }
                    activity.thinking_ms = Some(activity.thinking_ms.unwrap_or(0).saturating_add(elapsed_ms));
                    activity.current_step_label = None;
                    console.live_activity(activity.clone());
                } else if matches!(
                    item_type,
                    "command_execution"
                        | "mcp_tool_call"
                        | "function_call"
                        | "tool_use"
                        | "file_change"
                        | "patch_apply"
                        | "web_search"
                ) {
                    activity.current_step_label =
                        crate::orchestrator::step_labels::label_for_codex_item(item);
                    console.live_activity(activity.clone());
                }
            }
        },
    )
    .await?;
    let mut reply = parse_jsonl_output_with_provider(&stdout, Some(provider), model)?;
    activity.current_step_label = None;
    reply.activity = activity;
    Ok(reply)
}

#[cfg_attr(not(test), allow(dead_code))] // usado só nos testes
pub(crate) fn parse_jsonl_output(stdout: &[u8]) -> Result<Reply, ClaudeError> {
    parse_jsonl_output_with_provider(stdout, None, "")
}

pub(crate) fn parse_jsonl_output_with_provider(
    stdout: &[u8],
    provider: Option<&ProviderConfig>,
    model: &str,
) -> Result<Reply, ClaudeError> {
    let raw = String::from_utf8_lossy(stdout);
    let mut agent_messages: Vec<String> = Vec::new();
    let mut saw_json = false;
    let mut usage: Option<TurnUsage> = None;
    let mut tools: Vec<ToolActivity> = Vec::new();

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
            "item.completed" => {
                let item = &value["item"];
                match item.get("type").and_then(Json::as_str) {
                    Some("agent_message") => {
                        if let Some(text) = item.get("text").and_then(Json::as_str) {
                            if !text.trim().is_empty() {
                                agent_messages.push(text.to_string());
                            }
                        }
                    }
                    Some(_) => {
                        collect_codex_tools(item, &mut tools);
                    }
                    None => {}
                }
            }
            "turn.completed" => {
                let input_tokens = value.pointer("/usage/input_tokens").and_then(Json::as_u64);
                let cached_input_tokens = value
                    .pointer("/usage/cached_input_tokens")
                    .and_then(Json::as_u64);
                let output_tokens = value.pointer("/usage/output_tokens").and_then(Json::as_u64);
                let thought_tokens = value
                    .pointer("/usage/reasoning_output_tokens")
                    .or_else(|| value.pointer("/usage/output_tokens_details/reasoning_tokens"))
                    .and_then(Json::as_u64);
                let cost_usd = value
                    .get("cost_usd")
                    .or_else(|| value.pointer("/usage/cost_usd"))
                    .and_then(Json::as_f64);
                let parsed_model = value
                    .get("model")
                    .and_then(Json::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .or_else(|| (!model.trim().is_empty()).then(|| model.to_string()));
                if input_tokens.is_some()
                    || output_tokens.is_some()
                    || cached_input_tokens.is_some()
                    || thought_tokens.is_some()
                    || cost_usd.is_some()
                {
                    usage = Some(TurnUsage {
                        provider: provider.map(|p| p.id.clone()),
                        model: parsed_model,
                        input_tokens,
                        output_tokens,
                        cached_input_tokens,
                        cache_write_tokens: None,
                        thought_tokens,
                        cost_usd,
                        is_fallback: None,
                        ..Default::default()
                    });
                }
            }
            "turn.failed" | "error" => {
                let message = value["error"]["message"]
                    .as_str()
                    .or_else(|| value["message"].as_str())
                    .unwrap_or("erro desconhecido no Codex");
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

    let text = if !agent_messages.is_empty() {
        agent_messages.join("\n\n")
    } else if !saw_json {
        raw.trim().to_string()
    } else {
        String::new()
    };

    Ok(Reply {
        text,
        limits: None,
        activity: TurnActivity {
            tools,
            ..TurnActivity::default()
        },
        usage,
        session_id: None,
    })
}

fn collect_codex_tools(item: &Json, tools: &mut Vec<ToolActivity>) {
    let item_type = item.get("type").and_then(Json::as_str).unwrap_or("");
    let call_id = item
        .get("id")
        .and_then(Json::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("codex-{}", tools.len() + 1));
    let status = match item.get("status").and_then(Json::as_str).unwrap_or("completed") {
        "failed" | "error" => ToolStatus::Failed,
        "in_progress" | "running" => ToolStatus::Running,
        _ => ToolStatus::Ok,
    };
    match item_type {
        "command_execution" => {
            let Some(command) = item
                .get("command")
                .or_else(|| item.get("aggregated_command"))
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                return;
            };
            let exit_code = item
                .get("exit_code")
                .or_else(|| item.get("exitCode"))
                .and_then(Json::as_i64)
                .and_then(|code| i32::try_from(code).ok());
            let final_status = match exit_code {
                Some(0) => ToolStatus::Ok,
                Some(_) => ToolStatus::Failed,
                None => status,
            };
            let output = item
                .get("aggregated_output")
                .or_else(|| item.get("output"))
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            tools.push(ToolActivity {
                call_id,
                name: "Bash".to_string(),
                category: ToolCategory::Command,
                target: command.to_string(),
                status: final_status,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: None,
                detail: None,
                result: output,
                exit_code,
            });
        }
        "file_change" | "patch_apply" => {
            if let Some(changes) = item.get("changes").and_then(Json::as_array) {
                for (i, change) in changes.iter().enumerate() {
                    let Some(path) = change
                        .get("path")
                        .or_else(|| change.get("file_path"))
                        .and_then(Json::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    else {
                        continue;
                    };
                    let kind = change
                        .get("kind")
                        .or_else(|| change.get("action"))
                        .and_then(Json::as_str)
                        .unwrap_or("edit")
                        .to_ascii_lowercase();
                    let name = match kind.as_str() {
                        "add" | "create" => "Write",
                        "delete" | "remove" => "Delete",
                        _ => "Edit",
                    };
                    tools.push(ToolActivity {
                        call_id: format!("{call_id}-{i}"),
                        name: name.to_string(),
                        category: ToolCategory::Edit,
                        target: path.to_string(),
                        status,
                        description: None,
                        added: None,
                        removed: (name == "Write").then_some(0),
                        change: None,
                        duration_ms: None,
                        detail: None,
                        result: None,
                        exit_code: None,
                    });
                }
            } else if let Some(path) = item
                .get("path")
                .or_else(|| item.get("file_path"))
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                tools.push(ToolActivity {
                    call_id,
                    name: "Edit".to_string(),
                    category: ToolCategory::Edit,
                    target: path.to_string(),
                    status,
                    description: None,
                    added: None,
                    removed: None,
                    change: None,
                    duration_ms: None,
                    detail: None,
                    result: None,
                    exit_code: None,
                });
            }
        }
        "mcp_tool_call" | "function_call" | "tool_use" => {
            let Some(raw_name) = item
                .get("tool")
                .or_else(|| item.get("name"))
                .or_else(|| item.pointer("/function/name"))
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                return;
            };
            let server = item
                .get("server")
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let name = match server {
                Some(srv) if !raw_name.contains('/') && !raw_name.starts_with("mcp__") => {
                    format!("{srv}/{raw_name}")
                }
                _ => raw_name.to_string(),
            };
            tools.push(ToolActivity {
                call_id,
                name,
                category: ToolCategory::Mcp,
                target: String::new(),
                status,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: None,
                detail: None,
                result: None,
                exit_code: None,
            });
        }
        "web_search" => {
            let Some(query) = item
                .get("query")
                .or_else(|| item.pointer("/action/query"))
                .and_then(Json::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
            else {
                return;
            };
            tools.push(ToolActivity {
                call_id,
                name: "WebSearch".to_string(),
                category: ToolCategory::Web,
                target: query.to_string(),
                status,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: None,
                detail: None,
                result: None,
                exit_code: None,
            });
        }
        _ => {}
    }
}
