use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value as Json};
use tokio::sync::Mutex as AsyncMutex;

use crate::claude::{
    classify_provider_error, timeout_for, Access, ClaudeError, PermissionRequest,
    PermissionVerdict, Reply, ToolActivity, ToolCategory, ToolStatus, TurnActivity, TurnUsage,
};
use crate::mcp::{
    McpRelayManager, McpSession, RelaySessionGuard, StdioMcpServerConfig, VokiaMcpBridge,
};
use crate::orchestrator::adapters::ConsoleSink;
use crate::orchestrator::step_labels::label_for_tool;
use crate::orchestrator::turn_plan::plan_from_acp_notification;
use crate::orchestrator::work_steps::{build_handoff_packet, HandoffInput, WorkStepCollector};
use crate::permissions::{PermissionGate, PermissionLevel};
use crate::providers::ProviderConfig;

use super::flavor::{
    build_acp_mcp_servers, resolve_flavor, AcpFlavor, AcpSteeringMode, DiscoveredModel,
};
use super::transport::{AcpRpcError, AcpTransport, IncomingRpc, TransportError};

pub const ACP_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);
pub const ACP_RPC_TIMEOUT: Duration = Duration::from_secs(30);

pub struct AcpSessionHandle {
    pub transport: AcpTransport,
    pub flavor: Box<dyn AcpFlavor>,
    pub provider: Option<ProviderConfig>,
    pub model: String,
    pub session_id: String,
    pub cwd: PathBuf,
    pub relay_guard: Option<RelaySessionGuard>,
    pub unread_steers: Vec<String>,
    pub unsent_background_turns: Vec<String>,
    pub wakes_after_prompt_ends: bool,
    pub context_tokens_used: u64,
    pub context_tokens_limit: Option<u64>,
    pub last_work_steps: Vec<String>,
}

impl AcpSessionHandle {
    pub fn is_alive(&self) -> bool {
        self.transport.is_alive()
    }

    pub fn terminate(&self) {
        self.transport.terminate();
    }

    pub fn needs_context_rotation(&self) -> bool {
        if !self.flavor.supports_context_rotation() {
            return false;
        }
        let Some(limit) = self.context_tokens_limit else {
            return false;
        };
        limit > 0 && self.context_tokens_used.saturating_mul(100) >= limit.saturating_mul(85)
    }
}

pub async fn open_acp_session(
    program: &Path,
    user_args: &[String],
    model: &str,
    access: Access<'_>,
    provider: Option<&ProviderConfig>,
    mcp_session: Option<&McpSession>,
    stdio_servers: Vec<StdioMcpServerConfig>,
    relay_manager: &Arc<McpRelayManager>,
    persisted_session_id: Option<&str>,
    pid_sink: Option<&AtomicU32>,
) -> Result<(AcpSessionHandle, Option<String>), ClaudeError> {
    let flavor = resolve_flavor(provider, program);
    let cwd = resolve_cwd(access);
    let temp_dir = std::env::temp_dir().join("vokia-acp");

    let vokia_bridge = mcp_session.map(|s| VokiaMcpBridge {
        url: s.url().to_string(),
        token: s.token().to_string(),
    });

    let relay_guard = if vokia_bridge.is_some() || !stdio_servers.is_empty() {
        Some(
            relay_manager
                .acquire_for_directory(&cwd, vokia_bridge, stdio_servers)
                .await
                .map_err(|err| ClaudeError::Mcp(err.to_string()))?,
        )
    } else {
        None
    };

    let relay_endpoint = relay_guard.as_ref().map(RelaySessionGuard::endpoint);
    let spec = flavor.build_spawn_spec(
        program,
        user_args,
        access,
        relay_endpoint.as_ref(),
        &temp_dir,
    )?;

    let transport = AcpTransport::spawn(spec, pid_sink)?;

    // 1. Handshake `initialize`
    let init_params = json!({
        "protocolVersion": 1,
        "clientInfo": {
            "name": "vokia",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "clientCapabilities": {
            "fs": {
                "readTextFile": false,
                "writeTextFile": false
            },
            "terminal": false
        }
    });
    transport
        .request("initialize", init_params, ACP_HANDSHAKE_TIMEOUT)
        .await
        .map_err(ClaudeError::from)?;

    // 2. Open or resume session
    let mcp_servers = build_acp_mcp_servers(relay_endpoint.as_ref());
    let (session_id, recovery_handoff) = open_or_resume_session(
        &transport,
        flavor.as_ref(),
        &cwd,
        &mcp_servers,
        persisted_session_id,
        provider,
        model,
        &[],
    )
    .await?;

    // 3. Set model if non-empty
    if !model.trim().is_empty() {
        let _ = set_session_model(&transport, &session_id, model).await;
    }

    let handle = AcpSessionHandle {
        transport,
        flavor,
        provider: provider.cloned(),
        model: model.to_string(),
        session_id,
        cwd,
        relay_guard,
        unread_steers: Vec::new(),
        unsent_background_turns: Vec::new(),
        wakes_after_prompt_ends: false,
        context_tokens_used: 0,
        context_tokens_limit: None,
        last_work_steps: Vec::new(),
    };

    Ok((handle, recovery_handoff))
}

#[allow(clippy::too_many_arguments)]
async fn open_or_resume_session(
    transport: &AcpTransport,
    flavor: &dyn AcpFlavor,
    cwd: &Path,
    mcp_servers: &[Json],
    persisted_session_id: Option<&str>,
    provider: Option<&ProviderConfig>,
    model: &str,
    work_steps_blocks: &[String],
) -> Result<(String, Option<String>), ClaudeError> {
    let cwd_str = cwd.to_string_lossy().to_string();

    if let Some(existing_id) = persisted_session_id.map(str::trim).filter(|s| !s.is_empty()) {
        let load_params = json!({
            "sessionId": existing_id,
            "cwd": cwd_str,
            "mcpServers": mcp_servers,
        });
        match transport
            .request("session/load", load_params.clone(), ACP_RPC_TIMEOUT)
            .await
        {
            Ok(res) => {
                let sid = extract_session_id(&res).unwrap_or_else(|| existing_id.to_string());
                return Ok((sid, None));
            }
            Err(TransportError::Rpc(rpc)) if rpc.code == -32601 => {
                // `session/load` não suportado: tenta `session/resume`
                match transport
                    .request("session/resume", load_params, ACP_RPC_TIMEOUT)
                    .await
                {
                    Ok(res) => {
                        let sid = extract_session_id(&res).unwrap_or_else(|| existing_id.to_string());
                        return Ok((sid, None));
                    }
                    Err(TransportError::Rpc(resume_rpc))
                        if flavor.is_lost_session_error(resume_rpc.code, &resume_rpc.message) =>
                    {
                        let new_sid = create_new_session(transport, &cwd_str, mcp_servers).await?;
                        let handoff = build_recovery_handoff(
                            provider,
                            model,
                            existing_id,
                            work_steps_blocks,
                        );
                        return Ok((new_sid, Some(handoff)));
                    }
                    Err(_) => {
                        let new_sid = create_new_session(transport, &cwd_str, mcp_servers).await?;
                        return Ok((new_sid, None));
                    }
                }
            }
            Err(TransportError::Rpc(rpc))
                if flavor.is_lost_session_error(rpc.code, &rpc.message) =>
            {
                let new_sid = create_new_session(transport, &cwd_str, mcp_servers).await?;
                let handoff =
                    build_recovery_handoff(provider, model, existing_id, work_steps_blocks);
                return Ok((new_sid, Some(handoff)));
            }
            Err(err) => return Err(ClaudeError::from(err)),
        }
    }

    let new_sid = create_new_session(transport, &cwd_str, mcp_servers).await?;
    Ok((new_sid, None))
}

async fn create_new_session(
    transport: &AcpTransport,
    cwd: &str,
    mcp_servers: &[Json],
) -> Result<String, ClaudeError> {
    let params = json!({
        "cwd": cwd,
        "mcpServers": mcp_servers,
    });
    let res = transport
        .request("session/new", params, ACP_RPC_TIMEOUT)
        .await
        .map_err(ClaudeError::from)?;
    extract_session_id(&res).ok_or_else(|| {
        ClaudeError::Reported("servidor ACP não retornou sessionId em session/new".to_string())
    })
}

async fn set_session_model(
    transport: &AcpTransport,
    session_id: &str,
    model: &str,
) -> Result<(), TransportError> {
    transport
        .request(
            "session/set_model",
            json!({
                "sessionId": session_id,
                "modelId": model,
            }),
            ACP_RPC_TIMEOUT,
        )
        .await
        .map(|_| ())
}

fn build_recovery_handoff(
    provider: Option<&ProviderConfig>,
    model: &str,
    lost_session_id: &str,
    work_steps_blocks: &[String],
) -> String {
    let provider_label = provider
        .map(|p| {
            if model.trim().is_empty() {
                p.id.clone()
            } else {
                format!("{} ({model})", p.id)
            }
        })
        .unwrap_or_else(|| model.to_string());

    let packet = build_handoff_packet(&HandoffInput {
        from_provider: Some(provider_label.clone()),
        to_provider: Some(provider_label),
        reason: Some(format!(
            "sessão ACP anterior ({lost_session_id}) perdida ou expirada"
        )),
        objective: None,
        current_request: None,
        card_summary: None,
        open_plan_steps: Vec::new(),
        recent_messages: Vec::new(),
        older_messages: Vec::new(),
        work_steps_blocks: work_steps_blocks.to_vec(),
        history_pointers: Vec::new(),
    });
    packet.text
}

pub async fn run_acp_prompt(
    handle: &mut AcpSessionHandle,
    access: Access<'_>,
    message: &str,
    gate: Option<&PermissionGate<'_>>,
    console: Option<&ConsoleSink<'_>>,
) -> Result<Reply, ClaudeError> {
    let limit = timeout_for(access);
    let mut prompt_text = message.to_string();

    // Se houver steers não lidos devolvidos à fila, anexa-os ao próximo prompt.
    if !handle.unread_steers.is_empty() {
        let steers = std::mem::take(&mut handle.unread_steers);
        prompt_text = format!("{}\n\n{prompt_text}", steers.join("\n"));
    }

    let mut attempt = 0u8;
    loop {
        match execute_single_prompt(handle, access, &prompt_text, gate, console, limit).await {
            Ok(reply) => return Ok(reply),
            Err(PromptFailure::LostSession(rpc)) if attempt == 0 => {
                attempt += 1;
                let old_sid = handle.session_id.clone();
                let relay_endpoint = handle
                    .relay_guard
                    .as_ref()
                    .map(RelaySessionGuard::endpoint);
                let mcp_servers = build_acp_mcp_servers(relay_endpoint.as_ref());
                let cwd_str = handle.cwd.to_string_lossy().to_string();
                let new_sid =
                    create_new_session(&handle.transport, &cwd_str, &mcp_servers).await?;
                handle.session_id = new_sid;
                if !handle.model.trim().is_empty() {
                    let _ = set_session_model(&handle.transport, &handle.session_id, &handle.model)
                        .await;
                }
                let handoff = build_recovery_handoff(
                    handle.provider.as_ref(),
                    &handle.model,
                    &old_sid,
                    &handle.last_work_steps,
                );
                prompt_text = format!(
                    "{handoff}\n\n[Recuperação automática após {}]\n\n{message}",
                    rpc.message
                );
            }
            Err(PromptFailure::LostSession(rpc)) => {
                return Err(ClaudeError::Reported(rpc.to_string()));
            }
            Err(PromptFailure::Error(err)) => return Err(err),
        }
    }
}

enum PromptFailure {
    LostSession(AcpRpcError),
    Error(ClaudeError),
}

async fn execute_single_prompt(
    handle: &mut AcpSessionHandle,
    access: Access<'_>,
    prompt_text: &str,
    gate: Option<&PermissionGate<'_>>,
    console: Option<&ConsoleSink<'_>>,
    limit: Duration,
) -> Result<Reply, PromptFailure> {
    let session_id = handle.session_id.clone();
    let prompt_params = json!({
        "sessionId": session_id,
        "prompt": [
            {
                "type": "text",
                "text": prompt_text
            }
        ]
    });

    let transport = &handle.transport;
    let prompt_fut = transport.request("session/prompt", prompt_params, limit);
    tokio::pin!(prompt_fut);

    let mut text_buf = String::new();
    let mut activity = TurnActivity::default();
    let mut tool_index: HashMap<String, usize> = HashMap::new();
    let mut work_steps = WorkStepCollector::new();
    let mut last_at = Instant::now();
    let mut usage: Option<TurnUsage> = None;

    let prompt_result = loop {
        tokio::select! {
            res = &mut prompt_fut => {
                break res;
            }
            incoming = transport.recv_incoming() => {
                let Some(msg) = incoming else {
                    continue;
                };
                handle_incoming_rpc(
                    transport,
                    msg,
                    access,
                    gate,
                    console,
                    &mut text_buf,
                    &mut activity,
                    &mut tool_index,
                    &mut work_steps,
                    &mut last_at,
                    &mut usage,
                ).await;
            }
        }
    };

    // Drena notificações que chegaram imediatamente antes da resposta final do prompt.
    while let Some(msg) = transport.try_recv_incoming() {
        handle_incoming_rpc(
            transport,
            msg,
            access,
            gate,
            console,
            &mut text_buf,
            &mut activity,
            &mut tool_index,
            &mut work_steps,
            &mut last_at,
            &mut usage,
        )
        .await;
    }

    let result = match prompt_result {
        Ok(res) => res,
        Err(TransportError::Rpc(rpc)) => {
            if handle.flavor.is_lost_session_error(rpc.code, &rpc.message) {
                return Err(PromptFailure::LostSession(rpc));
            }
            if let Some(provider) = &handle.provider {
                if let Some(classified) = classify_provider_error(
                    &provider.id,
                    &handle.model,
                    &rpc.message,
                    &provider.rate_limit_patterns,
                    &provider.model_unavailable_patterns,
                ) {
                    return Err(PromptFailure::Error(classified));
                }
            }
            return Err(PromptFailure::Error(ClaudeError::Reported(rpc.to_string())));
        }
        Err(err) => return Err(PromptFailure::Error(ClaudeError::from(err))),
    };

    // Verifica uso / rotação de contexto / background turns
    if let Some(res_usage) = extract_usage_from_result(&result, handle.provider.as_ref(), &handle.model) {
        if let Some(total) = res_usage.input_tokens.zip(res_usage.output_tokens).map(|(i, o)| i + o) {
            handle.context_tokens_used = handle.context_tokens_used.saturating_add(total);
        }
        usage = Some(res_usage);
    }
    if let Some(limit_tokens) = result
        .pointer("/usage/contextWindow")
        .or_else(|| result.get("contextWindow"))
        .and_then(Json::as_u64)
    {
        handle.context_tokens_limit = Some(limit_tokens);
    }

    let has_running_tool = activity
        .tools
        .iter()
        .any(|t| t.status == ToolStatus::Running);

    if handle.flavor.wakes_after_prompt_ends() {
        let wakes = result
            .get("wakesAfterPromptEnds")
            .and_then(Json::as_bool)
            .unwrap_or(false)
            || has_running_tool;
        handle.wakes_after_prompt_ends = wakes;
    }

    if handle.flavor.checks_background_via_session_list() && has_running_tool {
        if let Ok(list_res) = transport
            .request("session/list", json!({}), Duration::from_secs(5))
            .await
        {
            if let Some(sessions) = list_res.get("sessions").and_then(Json::as_array) {
                for sess in sessions {
                    let status = sess.get("status").and_then(Json::as_str).unwrap_or("");
                    if status.eq_ignore_ascii_case("running") {
                        if let Some(sid) = extract_session_id(sess) {
                            if !handle.unsent_background_turns.contains(&sid) {
                                handle.unsent_background_turns.push(sid);
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(formatted) = work_steps.format() {
        handle.last_work_steps.push(formatted.block);
        if handle.last_work_steps.len() > 8 {
            handle.last_work_steps.remove(0);
        }
    }

    let final_text = if !text_buf.trim().is_empty() {
        text_buf.trim_end().to_string()
    } else {
        result
            .get("text")
            .or_else(|| result.get("output"))
            .and_then(Json::as_str)
            .unwrap_or("")
            .trim_end()
            .to_string()
    };

    activity.current_step_label = None;

    Ok(Reply {
        text: final_text,
        limits: None,
        activity,
        usage,
        session_id: Some(handle.session_id.clone()),
    })
}

#[allow(clippy::too_many_arguments)]
async fn handle_incoming_rpc(
    transport: &AcpTransport,
    msg: IncomingRpc,
    access: Access<'_>,
    gate: Option<&PermissionGate<'_>>,
    console: Option<&ConsoleSink<'_>>,
    text_buf: &mut String,
    activity: &mut TurnActivity,
    tool_index: &mut HashMap<String, usize>,
    work_steps: &mut WorkStepCollector,
    last_at: &mut Instant,
    usage: &mut Option<TurnUsage>,
) {
    match msg {
        IncomingRpc::Notification { method, params } => {
            if method != "session/update" {
                return;
            }
            let now = Instant::now();
            let elapsed_ms = (now.saturating_duration_since(*last_at).as_millis() as u64).max(50);
            *last_at = now;

            let envelope = json!({ "method": "session/update", "params": params });
            if let Some(plan) = plan_from_acp_notification(&envelope) {
                activity.plan = (!plan.is_empty()).then_some(plan.clone());
                if let Some(c) = console {
                    c.turn_plan(plan);
                    c.live_activity(activity.clone());
                }
                return;
            }

            let update = params.get("update").unwrap_or(&params);
            let kind = update
                .get("sessionUpdate")
                .or_else(|| update.get("session_update"))
                .or_else(|| update.get("type"))
                .and_then(Json::as_str)
                .unwrap_or("");

            match kind {
                "agent_message_chunk" => {
                    if let Some(chunk) = extract_content_text(update) {
                        text_buf.push_str(&chunk);
                    }
                }
                "agent_thought_chunk" => {
                    if let Some(chunk) = extract_content_text(update) {
                        match &mut activity.thinking_text {
                            Some(existing) => existing.push_str(&chunk),
                            None => activity.thinking_text = Some(chunk),
                        }
                    }
                    activity.thinking_ms =
                        Some(activity.thinking_ms.unwrap_or(0).saturating_add(elapsed_ms));
                    activity.current_step_label = None;
                    if let Some(c) = console {
                        c.live_activity(activity.clone());
                    }
                }
                "tool_call" | "tool_call_update" => {
                    apply_tool_call_update(update, activity, tool_index, work_steps);
                    if let Some(c) = console {
                        c.live_activity(activity.clone());
                    }
                }
                "usage_update" => {
                    if let Some(u) = extract_usage_from_result(update, None, "") {
                        *usage = Some(u);
                    }
                }
                _ => {}
            }
        }
        IncomingRpc::Request { id, method, params } => {
            if method == "session/request_permission" {
                let outcome = handle_permission_request(&params, access, gate).await;
                let _ = transport.respond(id, outcome).await;
            } else {
                let _ = transport
                    .respond_error(id, -32601, &format!("método não suportado: {method}"))
                    .await;
            }
        }
    }
}

async fn handle_permission_request(
    params: &Json,
    access: Access<'_>,
    gate: Option<&PermissionGate<'_>>,
) -> Json {
    let tool_call = params.get("toolCall").unwrap_or(params);
    let call_id = tool_call
        .get("toolCallId")
        .or_else(|| tool_call.get("id"))
        .and_then(Json::as_str)
        .unwrap_or("acp-perm")
        .to_string();
    let tool_name = tool_call
        .get("title")
        .or_else(|| tool_call.get("kind"))
        .or_else(|| tool_call.get("name"))
        .and_then(Json::as_str)
        .unwrap_or("Bash")
        .to_string();
    let input = tool_call
        .get("rawInput")
        .or_else(|| tool_call.get("input"))
        .cloned()
        .unwrap_or(Json::Null);

    let options = params
        .get("options")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default();

    let verdict = if let Some(g) = gate {
        g.clone()
            .decide(PermissionRequest {
                id: call_id,
                tool: tool_name,
                input,
            })
            .await
    } else {
        match access {
            Access::Project(p) if p.level == PermissionLevel::Liberado => PermissionVerdict::Allow,
            Access::Project(p) if p.level == PermissionLevel::Edicao => PermissionVerdict::Allow,
            _ => PermissionVerdict::Deny(Some("modo leitura sem aprovação interativa".to_string())),
        }
    };

    match verdict {
        PermissionVerdict::Allow => {
            let allow_id = pick_option_id(&options, &["allow_once", "allow_always", "allow"])
                .unwrap_or_else(|| "allow-once".to_string());
            json!({
                "outcome": {
                    "outcome": "selected",
                    "optionId": allow_id,
                }
            })
        }
        PermissionVerdict::Deny(_) => {
            if let Some(reject_id) = pick_option_id(&options, &["reject_once", "reject_always", "deny"]) {
                json!({
                    "outcome": {
                        "outcome": "selected",
                        "optionId": reject_id,
                    }
                })
            } else {
                json!({
                    "outcome": {
                        "outcome": "cancelled"
                    }
                })
            }
        }
    }
}

fn pick_option_id(options: &[Json], preferred_kinds: &[&str]) -> Option<String> {
    for kind in preferred_kinds {
        for opt in options {
            let opt_kind = opt.get("kind").and_then(Json::as_str).unwrap_or("");
            if opt_kind.eq_ignore_ascii_case(kind) {
                if let Some(id) = opt.get("optionId").and_then(Json::as_str) {
                    return Some(id.to_string());
                }
            }
        }
    }
    None
}

pub async fn steer_acp_session(
    handle: &mut AcpSessionHandle,
    message: &str,
) -> Result<bool, ClaudeError> {
    match handle.flavor.steering_mode() {
        AcpSteeringMode::CustomMethod(method) => {
            let res = handle
                .transport
                .request(
                    method,
                    json!({
                        "sessionId": handle.session_id,
                        "message": message,
                    }),
                    ACP_RPC_TIMEOUT,
                )
                .await
                .map_err(ClaudeError::from)?;
            if let Some(unread) = res.get("unreadSteers").and_then(Json::as_array) {
                for item in unread {
                    if let Some(text) = item.as_str() {
                        handle.unread_steers.push(text.to_string());
                    }
                }
            } else if res
                .get("unreadSteers")
                .and_then(Json::as_u64)
                .is_some_and(|n| n > 0)
            {
                handle.unread_steers.push(message.to_string());
            }
            Ok(true)
        }
        AcpSteeringMode::SecondPrompt => {
            handle
                .transport
                .notify(
                    "session/prompt",
                    json!({
                        "sessionId": handle.session_id,
                        "prompt": [{ "type": "text", "text": message }],
                    }),
                )
                .await
                .map_err(ClaudeError::from)?;
            Ok(true)
        }
    }
}

pub async fn interrupt_acp_session(handle: &AcpSessionHandle) -> Result<(), ClaudeError> {
    handle
        .transport
        .notify(
            "session/cancel",
            json!({ "sessionId": handle.session_id }),
        )
        .await
        .map_err(ClaudeError::from)
}

pub async fn discover_acp_models(
    program: &Path,
    user_args: &[String],
    provider: Option<&ProviderConfig>,
    relay_manager: &Arc<McpRelayManager>,
) -> Result<Vec<String>, ClaudeError> {
    let (handle, _) = open_acp_session(
        program,
        user_args,
        "",
        Access::TextOnly,
        provider,
        None,
        Vec::new(),
        relay_manager,
        None,
        None,
    )
    .await?;

    let mut models = query_model_list(&handle.transport).await?;
    if models.is_empty() {
        if let Some(delay) = handle.flavor.relist_delay() {
            tokio::time::sleep(delay).await;
            models = query_model_list(&handle.transport).await?;
        }
    }
    let sorted = handle.flavor.sort_models(models);
    handle.terminate();
    Ok(sorted)
}

async fn query_model_list(transport: &AcpTransport) -> Result<Vec<DiscoveredModel>, ClaudeError> {
    let res = transport
        .request("model/list", json!({}), ACP_RPC_TIMEOUT)
        .await
        .map_err(ClaudeError::from)?;
    let arr = res
        .get("models")
        .or_else(|| res.get("data"))
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        let Some(id) = item
            .as_str()
            .or_else(|| item.get("id").and_then(Json::as_str))
            .or_else(|| item.get("modelId").and_then(Json::as_str))
            .or_else(|| item.get("name").and_then(Json::as_str))
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let is_free = item.get("isFree").and_then(Json::as_bool) == Some(true)
            || item.get("free").and_then(Json::as_bool) == Some(true)
            || item
                .pointer("/pricing/prompt")
                .and_then(Json::as_f64)
                .is_some_and(|p| p == 0.0);
        out.push(DiscoveredModel {
            id: id.to_string(),
            is_free,
        });
    }
    Ok(out)
}

pub async fn test_acp_connection(
    program: &Path,
    user_args: &[String],
    model: &str,
    config: &ProviderConfig,
    limit: Duration,
    relay_manager: &Arc<McpRelayManager>,
) -> Result<String, ClaudeError> {
    let fut = async {
        let (mut handle, _) = open_acp_session(
            program,
            user_args,
            model,
            Access::TextOnly,
            Some(config),
            None,
            Vec::new(),
            relay_manager,
            None,
            None,
        )
        .await?;
        let reply = run_acp_prompt(
            &mut handle,
            Access::TextOnly,
            "Responda apenas OK.",
            None,
            None,
        )
        .await;
        handle.terminate();
        reply.map(|r| r.text)
    };
    tokio::time::timeout(limit, fut)
        .await
        .map_err(|_| ClaudeError::Timeout(limit))?
}

fn resolve_cwd(access: Access<'_>) -> PathBuf {
    match access {
        Access::Project(p) => p.dir.to_path_buf(),
        Access::TextOnly => std::env::temp_dir(),
    }
}

fn extract_session_id(val: &Json) -> Option<String> {
    val.get("sessionId")
        .or_else(|| val.get("session_id"))
        .or_else(|| val.get("id"))
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn extract_content_text(update: &Json) -> Option<String> {
    if let Some(s) = update.pointer("/content/text").and_then(Json::as_str) {
        return Some(s.to_string());
    }
    if let Some(s) = update.get("text").and_then(Json::as_str) {
        return Some(s.to_string());
    }
    None
}

fn apply_tool_call_update(
    update: &Json,
    activity: &mut TurnActivity,
    tool_index: &mut HashMap<String, usize>,
    work_steps: &mut WorkStepCollector,
) {
    let call_id = update
        .get("toolCallId")
        .or_else(|| update.get("id"))
        .and_then(Json::as_str)
        .unwrap_or("tool")
        .to_string();
    let name = update
        .get("name")
        .or_else(|| update.get("toolName"))
        .or_else(|| update.get("title"))
        .and_then(Json::as_str)
        .unwrap_or("tool");
    let raw_input_val = update
        .get("rawInput")
        .or_else(|| update.get("args"))
        .or_else(|| update.get("input"))
        .or_else(|| update.get("arguments"));
    let parsed_input = raw_input_val
        .and_then(Json::as_str)
        .and_then(|s| serde_json::from_str::<Json>(s).ok());
    let raw_input = parsed_input
        .as_ref()
        .or(raw_input_val)
        .unwrap_or(&Json::Null);
    let status = parse_acp_tool_status(update.get("status").and_then(Json::as_str));
    let category = infer_tool_category(update.get("kind").and_then(Json::as_str), name);
    let location_path = update
        .pointer("/locations/0/path")
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let target = extract_tool_target(raw_input, location_path.unwrap_or(name));

    if let Some(&idx) = tool_index.get(&call_id) {
        if let Some(existing) = activity.tools.get_mut(idx) {
            existing.status = status;
            if existing.name == "tool" && name != "tool" {
                existing.name = name.to_string();
                existing.category = category;
            }
            if (existing.target == existing.name || existing.target == "tool") && target != name {
                existing.target = target.clone();
            }
            if let Some(out) = extract_tool_output(update) {
                existing.result = Some(out);
            }
        }
    } else {
        let idx = activity.tools.len();
        tool_index.insert(call_id, idx);
        activity.tools.push(ToolActivity {
            call_id: update
                .get("toolCallId")
                .or_else(|| update.get("id"))
                .and_then(Json::as_str)
                .unwrap_or("tool")
                .to_string(),
            name: name.to_string(),
            category,
            target: target.clone(),
            status,
            description: None,
            added: None,
            removed: None,
            change: None,
            duration_ms: None,
            detail: None,
            result: extract_tool_output(update),
            exit_code: None,
        });
    }

    if status == ToolStatus::Ok || status == ToolStatus::Failed {
        let status_str = if status == ToolStatus::Ok { "ok" } else { "failed" };
        match category {
            ToolCategory::Command => {
                work_steps.record_command(&target, None, status_str, None);
            }
            _ => {
                work_steps.record_tool(None, name, status_str);
            }
        }
    }

    activity.current_step_label = label_for_tool(name, raw_input)
        .or_else(|| update.get("title").and_then(Json::as_str).map(str::to_string))
        .or_else(|| Some(name.to_string()));
}

fn extract_tool_output(update: &Json) -> Option<String> {
    if let Some(out) = update
        .get("rawOutput")
        .or_else(|| update.get("output"))
        .and_then(|v| {
            v.as_str()
                .map(str::to_string)
                .or_else(|| Some(v.to_string()))
        })
    {
        return Some(out);
    }
    update
        .get("content")
        .and_then(Json::as_array)
        .and_then(|arr| {
            let texts: Vec<&str> = arr
                .iter()
                .filter_map(|item| {
                    item.pointer("/content/text")
                        .or_else(|| item.get("text"))
                        .and_then(Json::as_str)
                })
                .collect();
            (!texts.is_empty()).then(|| texts.join("\n"))
        })
}

fn extract_tool_target(input: &Json, fallback: &str) -> String {
    input
        .get("command")
        .or_else(|| input.get("CommandLine"))
        .or_else(|| input.get("file_path"))
        .or_else(|| input.get("path"))
        .or_else(|| input.get("Path"))
        .or_else(|| input.get("target_file"))
        .or_else(|| input.get("TargetFile"))
        .or_else(|| input.get("AbsolutePath"))
        .or_else(|| input.get("directory"))
        .or_else(|| input.get("DirectoryPath"))
        .or_else(|| input.get("query"))
        .or_else(|| input.get("Query"))
        .or_else(|| input.get("url"))
        .or_else(|| input.get("Url"))
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn parse_acp_tool_status(raw: Option<&str>) -> ToolStatus {
    match raw.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "completed" | "ok" | "done" | "success" => ToolStatus::Ok,
        "failed" | "error" | "cancelled" => ToolStatus::Failed,
        _ => ToolStatus::Running,
    }
}

fn infer_tool_category(kind: Option<&str>, name: &str) -> ToolCategory {
    match kind.unwrap_or("").trim().to_ascii_lowercase().as_str() {
        "read" => ToolCategory::Read,
        "list" => ToolCategory::List,
        "edit" | "delete" | "move" => ToolCategory::Edit,
        "execute" | "command" | "bash" => ToolCategory::Command,
        "search" => ToolCategory::Search,
        "fetch" | "web" => ToolCategory::Web,
        _ => {
            let lower = name.to_ascii_lowercase();
            if lower.starts_with("mcp__vokia__") {
                ToolCategory::App
            } else if lower.starts_with("mcp__") {
                ToolCategory::Mcp
            } else if lower.contains("bash") || lower.contains("exec") || lower.contains("command") {
                ToolCategory::Command
            } else if lower.contains("edit") || lower.contains("write") {
                ToolCategory::Edit
            } else if lower.contains("read") {
                ToolCategory::Read
            } else if lower.contains("list_dir") || lower.contains("find_by_name") || lower == "glob" {
                ToolCategory::List
            } else if lower.contains("grep") || lower.contains("search_web") || lower.contains("web_search") {
                if lower.contains("web") {
                    ToolCategory::Web
                } else {
                    ToolCategory::Search
                }
            } else {
                ToolCategory::Other
            }
        }
    }
}

fn extract_usage_from_result(
    value: &Json,
    provider: Option<&ProviderConfig>,
    model: &str,
) -> Option<TurnUsage> {
    let usage = value.get("usage").unwrap_or(value);
    let input_tokens = usage
        .get("inputTokens")
        .or_else(|| usage.get("input_tokens"))
        .and_then(Json::as_u64);
    let output_tokens = usage
        .get("outputTokens")
        .or_else(|| usage.get("output_tokens"))
        .and_then(Json::as_u64);
    let cached_input_tokens = usage
        .get("cachedInputTokens")
        .or_else(|| usage.get("cachedReadTokens"))
        .or_else(|| usage.get("cache_read_input_tokens"))
        .and_then(Json::as_u64);
    let cache_write_tokens = usage
        .get("cachedWriteTokens")
        .or_else(|| usage.get("cacheWriteTokens"))
        .or_else(|| usage.get("cache_creation_input_tokens"))
        .and_then(Json::as_u64);
    let thought_tokens = usage
        .get("thoughtTokens")
        .or_else(|| usage.get("reasoning_tokens"))
        .and_then(Json::as_u64);
    let raw_cost_usd = usage
        .get("costUsd")
        .or_else(|| usage.get("total_cost_usd"))
        .and_then(Json::as_f64);

    if input_tokens.is_none()
        && output_tokens.is_none()
        && cached_input_tokens.is_none()
        && cache_write_tokens.is_none()
        && thought_tokens.is_none()
        && raw_cost_usd.is_none()
    {
        return None;
    }

    let trimmed_model = model.trim();
    let cost_usd = raw_cost_usd.or_else(|| {
        if trimmed_model.is_empty() {
            return None;
        }
        let counts = crate::db::usage_pricing::TokenCounts {
            input_tokens: input_tokens.unwrap_or(0),
            cached_input_tokens: cached_input_tokens.unwrap_or(0),
            cache_write_tokens: cache_write_tokens.unwrap_or(0),
            output_tokens: output_tokens.unwrap_or(0),
            thought_tokens: thought_tokens.unwrap_or(0),
        };
        (counts.total() > 0)
            .then(|| crate::db::usage_pricing::estimate_usage_price(trimmed_model, counts))
            .flatten()
            .map(|est| est.cost_usd)
    });

    Some(TurnUsage {
        provider: provider.map(|p| p.id.clone()),
        model: (!trimmed_model.is_empty()).then(|| trimmed_model.to_string()),
        input_tokens,
        output_tokens,
        cached_input_tokens,
        cache_write_tokens,
        thought_tokens,
        cost_usd,
        is_fallback: None,
        ..Default::default()
    })
}

pub type SharedAcpSession = Arc<AsyncMutex<AcpSessionHandle>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agy_acp_tool_call_e_update_preenchem_tools_target_e_step_label() {
        let mut activity = TurnActivity::default();
        let mut tool_index = HashMap::new();
        let mut work_steps = WorkStepCollector::new();

        let start = json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "call_1",
            "name": "read_file",
            "args": { "Path": "C:\\repo\\desktop\\package.json", "StartLine": 1, "EndLine": 1 },
            "status": "in_progress",
            "kind": "read",
            "title": "File desktop/package.json"
        });
        apply_tool_call_update(&start, &mut activity, &mut tool_index, &mut work_steps);

        assert_eq!(activity.tools.len(), 1);
        assert_eq!(activity.tools[0].name, "read_file");
        assert_eq!(activity.tools[0].category, ToolCategory::Read);
        assert_eq!(activity.tools[0].target, "C:\\repo\\desktop\\package.json");
        assert_eq!(activity.tools[0].status, ToolStatus::Running);
        assert_eq!(activity.current_step_label.as_deref(), Some("Lendo package.json"));

        let finish = json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "call_1",
            "name": "run_command",
            "args": { "CommandLine": "cargo test" },
            "status": "completed",
            "kind": "execute",
            "rawOutput": "ok"
        });
        apply_tool_call_update(&finish, &mut activity, &mut tool_index, &mut work_steps);
        assert_eq!(activity.tools.len(), 1);
        assert_eq!(activity.tools[0].status, ToolStatus::Ok);
        assert_eq!(activity.tools[0].result.as_deref(), Some("ok"));
    }

    #[test]
    fn extract_usage_from_result_estima_custo_para_gemini_4_hydrogen_com_cached_read_tokens() {
        let payload = json!({
            "stopReason": "end_turn",
            "usage": {
                "inputTokens": 12760,
                "outputTokens": 238,
                "cachedReadTokens": 8393,
                "cachedWriteTokens": 0,
                "thoughtTokens": 145
            }
        });
        let usage = extract_usage_from_result(&payload, None, "gemini-4-hydrogen-medium")
            .expect("usage deve ser extraído");
        assert_eq!(usage.input_tokens, Some(12760));
        assert_eq!(usage.output_tokens, Some(238));
        assert_eq!(usage.cached_input_tokens, Some(8393));
        assert_eq!(usage.cache_write_tokens, Some(0));
        assert_eq!(usage.thought_tokens, Some(145));
        assert!(usage.cost_usd.is_some_and(|c| c > 0.0), "cost_usd={:?}", usage.cost_usd);
    }
}
