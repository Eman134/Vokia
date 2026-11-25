use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::agents::{Agent, AgentProfile, Team};
use crate::claude::{Access, ClaudeError, LiveSession, ProjectAccess, Reply, TurnActivity, TurnUsage};
use crate::db::{
    load_recent_work_steps_safe, work_steps_session_key, Db, DbError, DbState,
    OrchestrationSnapshot, GRANT_SESSION, SETTING_AUTO_TRIAGE, SETTING_PERMISSION_GRANT,
};
use crate::events::{AgentState, Speaker, VokiaEvent};
use crate::mcp::{
    Binding, ExchangeOutcome, ExchangeSlot, McpServer, McpSession, McpState, ToolScope,
    CONFIG_DIR,
};
use crate::orchestration::instances::display_name;
use crate::orchestration::snapshot::{agent_prompt, leader_of};
use crate::permissions::{PermissionGate, PermissionLevel};
use crate::providers::{ModelTier, ProviderProtocol, ProviderRegistry};
use crate::session_key::SessionKey;
use crate::state::AppState;
use crate::usage::{UsageLimitGate, UsageLimitReturn};

use super::adapters::ProviderAdapter;
use super::console::ConsoleSink;
use super::emit::{emit, emit_state, report_limits};
use super::error::FlowError;
use super::fallback::{
    emit_overrides_changed, handle_provider_auth_or_billing, load_global_active_llm,
    load_global_fallback, mark_cooldown, now_millis_u64, resolve_candidates, resolve_program_for,
    schedule_limit_return, LlmTarget, OverrideReason, ResolvedCandidate, TemporaryOverride,
    DEFAULT_COOLDOWN_SECS,
};
use super::flow::profile_of;
use super::project_context::project_choices;
use super::prompts::{maestro_role, role, StepPrompts, ToolPromptMode};
use crate::roles::RoleRegistry;
use super::session_policy::{
    call_mode, CallMode, SESSION_CLOSE_TIMEOUT_SECS, SESSION_IDLE_LIMIT_SECS,
    SESSION_RECOVERY_LIMIT, SESSION_ROTATE_MESSAGES,
};
use super::session_pool::{ExchangeSessions, NO_PID};
use super::turns::Exchange;
use super::work_steps::{build_handoff_packet, HandoffInput, HandoffMessage};

const GLOBAL_INSTRUCTIONS_PATH: &str = ".claude/CLAUDE.md";

pub(super) const SESSION_CLOSE_GRACE: Duration = Duration::from_secs(SESSION_CLOSE_TIMEOUT_SECS);
pub(super) const SESSION_IDLE_LIMIT: Duration = Duration::from_secs(SESSION_IDLE_LIMIT_SECS);

pub(super) struct StepReply {
    pub(super) text: String,
    pub(super) outcome: ExchangeOutcome,
    pub(super) activity: Option<TurnActivity>,
    pub(super) usage: Option<TurnUsage>,
}

pub(super) async fn ask_step(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions<'_>,
    agent: &Agent,
    prompts: &StepPrompts,
) -> Result<StepReply, FlowError> {
    let Exchange {
        app, key, item_id, ..
    } = exchange;
    emit_state(app, key.project_id, agent.clone(), AgentState::Thinking, *item_id)?;
    let console = ConsoleSink::new(
        app,
        SessionKey {
            project_id: key.project_id,
            agent: agent.clone(),
        },
        item_id.map(str::to_string),
    );
    console.started();
    let result = run_step(exchange, sessions, agent, prompts, &console).await;
    if result.is_err() {
        if let Some(partial) = console.take_last_activity() {
            super::turns::record_turn_work_steps(exchange, agent, Some(&partial), None);
        }
    }
    console.finished(&result);
    result
}

async fn run_step(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions<'_>,
    agent: &Agent,
    prompts: &StepPrompts,
    console: &ConsoleSink<'_>,
) -> Result<StepReply, FlowError> {
    let Exchange {
        app,
        state,
        cli,
        project,
        project_context,
        key,
        team,
        orchestration,
        ..
    } = exchange;
    let profile = profile_of(team, agent)?;
    let providers = app.state::<crate::providers::ProviderRegistry>();
    let global_active = load_global_active_llm(app);
    let global_fallback = load_global_fallback(app);
    let candidates = resolve_candidates(
        agent,
        profile,
        &providers,
        state,
        global_active.as_ref(),
        &global_fallback,
        Some(project.level),
    );
    if candidates.is_empty() {
        return Err(FlowError::CliUnavailable(format!(
            "Nenhum provedor disponível para {agent}."
        )));
    }
    let grant_per_session = grants_per_session(app)?;
    let per_step = sessions.is_per_step();
    let store = SessionStore {
        app,
        key: SessionKey {
            project_id: key.project_id,
            agent: agent.clone(),
        },
    };
    let mut fallback_reason: Option<OverrideReason> = None;
    let mut fallback_expires_at: Option<u64> = None;
    let mut original_limited: Option<(String, String, Option<ModelTier>, u64)> = None;
    let mut failed_from: Option<(String, String)> = None;
    for (idx, candidate) in candidates.iter().enumerate() {
        console.reset_acted();
        let adapter = ProviderAdapter::from_cli(&candidate.cli, &providers)?;
        if agent == &Agent::ember() && !adapter.has_tools() {
            continue;
        }
        let scope = tool_scope(*orchestration, agent);
        let mcp = if adapter.has_tools() {
            McpLaunch::new(
                app,
                key.project_id,
                agent,
                &crate::orchestration::instances::display_name(team, agent),
                scope,
            )?
        } else {
            None
        };
        let tool_mode = match adapter.tool_transport() {
            crate::providers::ToolTransport::McpNative if mcp.is_some() => ToolPromptMode::Mcp,
            crate::providers::ToolTransport::VokiaCli if mcp.is_some() => {
                if super::adapters::resolve_vokia_cli_path().is_some() {
                    ToolPromptMode::VokiaCli
                } else {
                    ToolPromptMode::None
                }
            }
            _ => ToolPromptMode::None,
        };
        let prompt = agent_prompt(*orchestration, team, agent);
        let registered_role = app
            .state::<RoleRegistry>()
            .find(&profile.role)
            .map_err(FlowError::Roles)?;
        let base = role(
            profile,
            registered_role.as_ref(),
            &prompt.roster,
            prompt.layer.as_ref(),
            project_context,
            tool_mode,
            scope,
            global_instructions(app, adapter.is_claude()).as_deref(),
        );
        let role = if agent == &Agent::ember() {
            let paused = paused_agents(app, key.project_id)?;
            maestro_role(
                &base,
                &[],
                &project_choices(app)?,
                *orchestration,
                team,
                &paused,
                tool_mode,
                auto_triage_enabled(app)?,
            )
        } else {
            base
        };
        let (program, mut user_args) = resolve_program_for(agent, &candidate.command, cli)?;
        user_args.extend(candidate.provider.resolve_effort_args(
            &candidate.model,
            candidate.reasoning_effort.as_deref(),
        ));
        let mut call = AgentCall {
            adapter,
            program,
            user_args,
            role,
            model: candidate.model.clone(),
            resume_session: None,
            item_id: exchange.item_id.map(str::to_string),
            channel_id: exchange.channel_id,
            mcp,
            has_variables: !exchange.definition.variables.is_empty(),
            collected: StdMutex::default(),
        };
        let stored_session = store.load().ok().flatten();
        if !per_step && !call.adapter.supports_live_session() {
            call.resume_session = stored_session.as_deref().and_then(|stored| {
                decode_provider_session(
                    stored,
                    &candidate.cli,
                    &candidate.model,
                    exchange.item_id,
                )
            });
        }
        let first = sessions.opened.insert(agent.clone());
        let mut held = sessions.hold(agent).await;
        if first {
            held.guard.begin_exchange();
        }
        let current_provider = (candidate.cli.clone(), candidate.model.clone());
        let previous_provider = detect_previous_provider(
            failed_from.as_ref(),
            held.guard.provider_key.as_ref(),
            stored_session.as_deref(),
            &current_provider,
        );
        let provider_switched = previous_provider.is_some();
        let rotating_session =
            held.guard.live.is_some() && held.guard.messages >= SESSION_ROTATE_MESSAGES;
        let handoff_prompts = if provider_switched || rotating_session {
            let reason = handoff_reason_label(fallback_reason, provider_switched, rotating_session);
            Some(build_handoff_prompts(
                exchange,
                agent,
                prompts,
                previous_provider.as_ref(),
                &current_provider,
                Some(reason),
            ))
        } else {
            None
        };
        let effective_prompts = handoff_prompts.as_ref().unwrap_or(prompts);
        if provider_switched {
            held.guard.session_id = None;
            call.resume_session = None;
        }
        if idx > 0 {
            held.guard.degrade().await;
        }
        if held.guard.live.is_none() {
            state.sessions.make_room(&held.slot).await;
        }
        if !grant_per_session {
            held.guard.forget_grants();
        }
        let granted = Arc::clone(&held.guard.granted);
        let gate = PermissionGate {
            app,
            broker: &state.permissions,
            agent: agent.clone(),
            project_id: key.project_id,
            granted: &granted,
            grant_per_session,
            item_id: exchange.item_id.map(str::to_string),
        };
        let access = if call.adapter.has_tools() {
            Access::Project(ProjectAccess {
                builtin_tools: has_builtin_tools(agent),
                ..*project
            })
        } else {
            Access::TextOnly
        };
        let attempt: Result<Reply, FlowError> = async {
            let live = if call.adapter.supports_live_session() {
                let mode = if per_step {
                    CallMode::Fresh
                } else {
                    call_mode(held.guard.fresh_only)
                };
                match mode {
                    CallMode::Live => {
                        call.adapter
                            .run_live(
                                &mut held.guard,
                                access,
                                gate.clone(),
                                &store,
                                &call,
                                effective_prompts,
                                first,
                                console,
                                state.sessions.adapters(),
                            )
                            .await?
                    }
                    CallMode::Fresh => None,
                }
            } else {
                None
            };
            match live {
                Some(reply) => Ok(reply),
                None => {
                    // Sem sessão viva não há onde acumular aprovações: cada comando pergunta de novo.
                    let gate = PermissionGate {
                        grant_per_session: false,
                        ..gate
                    };
                    let message = if call.resume_session.is_some() {
                        &effective_prompts.follow_up
                    } else {
                        &effective_prompts.opening
                    };
                    match fresh_reply(
                        access,
                        gate.clone(),
                        &held.slot.pid,
                        &call,
                        message,
                        console,
                        state.sessions.adapters(),
                    )
                    .await
                    .map_err(FlowError::Claude)
                    {
                        Err(error)
                            if call.resume_session.is_some()
                                && !error.is_limit()
                                && !error.is_model_unavailable() =>
                        {
                            store.clear()?;
                            call.resume_session = None;
                            fresh_reply(
                                access,
                                gate,
                                &held.slot.pid,
                                &call,
                                &effective_prompts.opening,
                                console,
                                state.sessions.adapters(),
                            )
                            .await
                            .map_err(FlowError::Claude)
                        }
                        result => result,
                    }
                }
            }
        }
        .await;
        match attempt {
            Ok(reply) => {
                if let Some(reason) = fallback_reason {
                    let (orig_cli, orig_model, orig_tier) = original_limited
                        .as_ref()
                        .map(|(cli, model, tier, _)| (cli.clone(), model.clone(), *tier))
                        .unwrap_or_else(|| {
                            (profile.cli.clone(), profile.model.clone(), profile.model_tier)
                        });
                    let ov = TemporaryOverride {
                        agent: agent.clone(),
                        cli: candidate.cli.clone(),
                        model: candidate.model.clone(),
                        model_tier: candidate.model_tier,
                        reasoning_effort: candidate.reasoning_effort.clone(),
                        command: candidate.command.clone(),
                        original_cli: orig_cli.clone(),
                        original_model: orig_model.clone(),
                        original_tier: orig_tier,
                        original_reasoning_effort: profile.reasoning_effort.clone(),
                        reason,
                        applied_at: now_millis_u64(),
                        expires_at: fallback_expires_at,
                    };
                    state
                        .temporary_overrides
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(agent.clone(), ov);
                    if reason == OverrideReason::UsageLimit {
                        if let Some((_, _, _, release_at_ms)) = original_limited {
                            state.usage.register_limit_return(UsageLimitReturn {
                                agent: agent.clone(),
                                original_provider_id: orig_cli,
                                original_model: orig_model,
                                original_tier: orig_tier,
                                fallback_provider_id: candidate.cli.clone(),
                                fallback_model: candidate.model.clone(),
                                release_at_ms,
                            });
                            schedule_limit_return((*app).clone(), release_at_ms);
                        }
                    }
                    emit_overrides_changed(app, state);
                }
                if let (Some(prev), Some(item_id)) = (previous_provider.as_ref(), exchange.item_id) {
                    record_provider_switch_card_activity(
                        app,
                        key.project_id,
                        item_id,
                        prev,
                        &current_provider,
                        fallback_reason,
                    );
                }
                if !per_step && !call.adapter.supports_live_session() {
                    if let Some(session_id) = reply.session_id.as_deref() {
                        store.save(&encode_provider_session(
                            &candidate.cli,
                            &candidate.model,
                            exchange.item_id,
                            session_id,
                        ))?;
                    }
                }
                if !call.adapter.supports_live_session() {
                    held.guard.provider_key = Some(current_provider);
                }
                report_limits(app, reply.limits)?;
                let outcome = call
                    .collected
                    .lock()
                    .map(|mut collected| std::mem::take(&mut *collected))
                    .unwrap_or_default();
                let mut normalized_usage = reply.usage.unwrap_or_default();
                normalized_usage.provider = Some(candidate.cli.clone());
                if candidate.cli != profile.cli || candidate.model != profile.model {
                    normalized_usage.is_fallback = Some(true);
                }
                if normalized_usage
                    .model
                    .as_deref()
                    .is_none_or(|m| m.trim().is_empty())
                {
                    normalized_usage.model = Some(candidate.model.clone());
                }
                return Ok(StepReply {
                    text: reply.text,
                    outcome,
                    activity: Some(reply.activity).filter(|activity| {
                        activity.thinking_ms.is_some()
                            || !activity.tools.is_empty()
                            || activity.plan.is_some()
                    }),
                    usage: Some(normalized_usage),
                });
            }
            Err(err) => {
                if let Some((err_provider, reason)) = err.auth_or_billing() {
                    let target_provider = err_provider.unwrap_or(&candidate.cli).to_string();
                    let acted = console.has_acted() || call.has_collected_actions();
                    let has_backup = idx + 1 < candidates.len();
                    handle_provider_auth_or_billing(app, state, &target_provider, &reason);
                    mark_cooldown(state, &candidate.cli, &candidate.model);
                    if UsageLimitGate::can_failover(acted, has_backup) {
                        held.guard.degrade().await;
                        continue;
                    }
                    return Err(FlowError::ProviderDisabled {
                        provider: target_provider,
                        reason,
                    });
                }
                let is_limit = err.is_limit();
                let is_unavail = err.is_model_unavailable();
                let acted = console.has_acted() || call.has_collected_actions();
                let has_backup = idx + 1 < candidates.len();
                let now_ms = now_millis_u64();

                if is_limit {
                    let resets_at = err.usage_limit().map(|l| l.resets_at).filter(|&r| r > 0);
                    let limit_entry = state.usage.mark_limit_reached(
                        &candidate.cli,
                        &candidate.model,
                        resets_at,
                        now_ms,
                    );
                    schedule_limit_return((*app).clone(), limit_entry.release_at_ms);
                    if UsageLimitGate::can_failover(acted, has_backup) {
                        mark_cooldown(state, &candidate.cli, &candidate.model);
                        held.guard.degrade().await;
                        fallback_expires_at = Some(limit_entry.release_at_ms);
                        fallback_reason = Some(OverrideReason::UsageLimit);
                        failed_from
                            .get_or_insert_with(|| (candidate.cli.clone(), candidate.model.clone()));
                        if original_limited.is_none() {
                            original_limited = Some((
                                candidate.cli.clone(),
                                candidate.model.clone(),
                                candidate.model_tier,
                                limit_entry.release_at_ms,
                            ));
                        }
                        continue;
                    }
                } else if is_unavail && UsageLimitGate::can_failover(acted, has_backup) {
                    mark_cooldown(state, &candidate.cli, &candidate.model);
                    held.guard.degrade().await;
                    fallback_expires_at =
                        Some(now_ms.saturating_add(DEFAULT_COOLDOWN_SECS * 1000));
                    fallback_reason = Some(OverrideReason::ModelUnavailable);
                    failed_from
                        .get_or_insert_with(|| (candidate.cli.clone(), candidate.model.clone()));
                    continue;
                }
                if let FlowError::Claude(ClaudeError::ModelUnavailable {
                    provider,
                    model,
                    detail,
                }) = &err
                {
                    let _ = emit(
                        app,
                        VokiaEvent::ModelUnavailable {
                            agent: agent.clone(),
                            provider: provider.clone(),
                            model: model.clone(),
                            detail: detail.clone(),
                        },
                    );
                }
                return Err(err);
            }
        }
    }
    Err(FlowError::CliUnavailable(format!(
        "Nenhum provedor disponível para {agent}."
    )))
}

// Banco ausente vira lista vazia, como nas demais leituras do prompt; só as pausas do projeto da troca contam.
fn paused_agents(app: &AppHandle, project_id: i64) -> Result<Vec<Agent>, FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(Vec::new());
    };
    db.project_paused_agents(project_id)
        .map_err(FlowError::Orchestration)
}

const AUTO_TRIAGE_ENABLED: &str = "true";

fn setting_equals(app: &AppHandle, key: &str, expected: &str) -> Result<bool, FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(false);
    };
    let settings = db.load_settings().map_err(FlowError::Settings)?;
    Ok(settings.get(key).is_some_and(|value| value == expected))
}

fn grants_per_session(app: &AppHandle) -> Result<bool, FlowError> {
    setting_equals(app, SETTING_PERMISSION_GRANT, GRANT_SESSION)
}

fn auto_triage_enabled(app: &AppHandle) -> Result<bool, FlowError> {
    setting_equals(app, SETTING_AUTO_TRIAGE, AUTO_TRIAGE_ENABLED)
}

// Contexto novo a cada chamada, mas pela sessão viva: só ela tem canal para o usuário decidir comandos fora da allowlist.
async fn fresh_reply(
    access: Access<'_>,
    gate: PermissionGate<'_>,
    pid: &AtomicU32,
    call: &AgentCall,
    message: &str,
    console: &ConsoleSink<'_>,
    runtimes: &super::adapters::AdapterRuntimePool,
) -> Result<Reply, ClaudeError> {
    call.adapter
        .run_fresh(access, gate, pid, call, message, console, runtimes)
        .await
}

pub(super) fn encode_provider_session(
    cli: &str,
    model: &str,
    item_id: Option<&str>,
    session_id: &str,
) -> String {
    format!("{}:{}:{}:{session_id}", cli, model, item_id.unwrap_or(""))
}

pub(super) fn decode_provider_session(
    stored: &str,
    cli: &str,
    model: &str,
    item_id: Option<&str>,
) -> Option<String> {
    let prefix = format!("{}:{}:{}:", cli, model, item_id.unwrap_or(""));
    stored
        .strip_prefix(&prefix)
        .map(str::trim)
        .filter(|session_id| !session_id.is_empty())
        .map(str::to_string)
}

const HANDOFF_WORK_STEPS_LIMIT: usize = 20;

fn detect_previous_provider(
    failed_from: Option<&(String, String)>,
    live_provider: Option<&(String, String)>,
    stored_session: Option<&str>,
    current: &(String, String),
) -> Option<(String, String)> {
    if let Some(prev) = failed_from {
        if prev != current {
            return Some(prev.clone());
        }
    }
    if let Some(prev) = live_provider {
        if prev != current {
            return Some(prev.clone());
        }
    }
    if let Some(stored) = stored_session.map(str::trim).filter(|s| !s.is_empty()) {
        if let Some((cli, rest)) = stored.split_once(':') {
            if let Some((model, _)) = rest.split_once(':') {
                if !cli.is_empty() && (cli != current.0 || (!model.is_empty() && model != current.1)) {
                    return Some((cli.to_string(), model.to_string()));
                }
            }
        } else if current.0 != "claude" {
            return Some(("claude".to_string(), String::new()));
        }
    }
    None
}

fn handoff_reason_label(
    fallback_reason: Option<OverrideReason>,
    provider_switched: bool,
    rotating_session: bool,
) -> &'static str {
    match fallback_reason {
        Some(OverrideReason::UsageLimit) => "limite de uso",
        Some(OverrideReason::ModelUnavailable) => "modelo indisponível",
        Some(OverrideReason::ManualTemp) => "troca manual",
        None if provider_switched => "troca de provedor",
        None if rotating_session => "rotação de sessão",
        None => "continuidade",
    }
}

fn format_provider_label(provider: &(String, String)) -> String {
    if provider.1.trim().is_empty() {
        provider.0.clone()
    } else {
        format!("{} ({})", provider.0, provider.1)
    }
}

fn build_handoff_prompts(
    exchange: &Exchange<'_>,
    agent: &Agent,
    prompts: &StepPrompts,
    previous_provider: Option<&(String, String)>,
    current_provider: &(String, String),
    reason: Option<&str>,
) -> StepPrompts {
    let db_state = exchange.app.state::<DbState>();
    let db = db_state.db();
    let project_id = exchange.key.project_id;
    let session_key = work_steps_session_key(project_id, agent.as_str(), exchange.item_id);

    let work_steps_blocks: Vec<String> = load_recent_work_steps_safe(
        db,
        project_id,
        agent.as_str(),
        &session_key,
        HANDOFF_WORK_STEPS_LIMIT,
    )
    .into_iter()
    .map(|record| record.block)
    .collect();

    let mut recent_messages = load_handoff_messages(
        db,
        project_id,
        agent,
        exchange.item_id,
        exchange.channel_id,
    );
    if recent_messages.is_empty() {
        if let Ok(lines) = super::turns::with_cache(exchange.state, project_id, |cache| {
            Ok(cache.recent_conversation(20))
        }) {
            recent_messages = lines
                .into_iter()
                .map(|line| HandoffMessage {
                    author: String::new(),
                    text: line,
                })
                .collect();
        }
    }

    let open_plan_steps = load_open_plan_steps(db, project_id, exchange.item_id);
    let card_summary = exchange.card_context();

    let packet = build_handoff_packet(&HandoffInput {
        from_provider: previous_provider.map(format_provider_label),
        to_provider: Some(format_provider_label(current_provider)),
        reason: reason.map(str::to_string),
        objective: (!exchange.definition.objetivo.trim().is_empty())
            .then(|| exchange.definition.objetivo.clone()),
        current_request: (!exchange.request.trim().is_empty())
            .then(|| exchange.request.to_string()),
        card_summary,
        open_plan_steps,
        recent_messages,
        older_messages: Vec::new(),
        work_steps_blocks,
        history_pointers: Vec::new(),
    });

    StepPrompts {
        opening: format!("{}\n\n{}", packet.text, prompts.opening),
        follow_up: prompts.follow_up.clone(),
        minimal: format!("{}\n\n{}", packet.text, prompts.minimal),
    }
}

fn load_handoff_messages(
    db: Option<&Db>,
    project_id: i64,
    agent: &Agent,
    item_id: Option<&str>,
    channel_id: Option<i64>,
) -> Vec<HandoffMessage> {
    let Some(db) = db else {
        return Vec::new();
    };
    let Ok(events) = db.load_history(project_id) else {
        return Vec::new();
    };
    let agent_speaker = Speaker::from(agent.clone());
    events
        .into_iter()
        .filter_map(|event| match event {
            VokiaEvent::Message {
                from,
                to,
                text,
                channel_id: msg_channel,
                item_id: msg_item,
                is_error,
                ..
            } => {
                if is_error || text.trim().is_empty() {
                    return None;
                }
                let matches_scope = if let Some(target_item) = item_id {
                    msg_item.as_deref() == Some(target_item)
                } else if let Some(target_channel) = channel_id {
                    msg_channel == Some(target_channel)
                } else {
                    from == agent_speaker || to == agent_speaker
                };
                if !matches_scope {
                    return None;
                }
                Some(HandoffMessage {
                    author: from.as_str().to_string(),
                    text,
                })
            }
            _ => None,
        })
        .collect()
}

fn load_open_plan_steps(db: Option<&Db>, project_id: i64, item_id: Option<&str>) -> Vec<String> {
    let (Some(db), Some(item_id)) = (db, item_id) else {
        return Vec::new();
    };
    let mut steps = Vec::new();
    if let Ok(criteria) = db.plan_criteria(project_id, item_id) {
        for criterion in criteria.into_iter().filter(|c| !c.done) {
            steps.push(format!("[ ] {}: {}", criterion.id, criterion.text));
        }
    }
    if steps.is_empty() {
        if let Ok(stages) = db.plan_stages(project_id, item_id) {
            for stage in stages
                .into_iter()
                .filter(|s| s.column.as_deref() != Some("Done"))
            {
                let col = stage.column.as_deref().unwrap_or("Backlog");
                steps.push(format!("{} ({col}): {}", stage.id, stage.title));
            }
        }
    }
    steps
}

fn record_provider_switch_card_activity(
    app: &AppHandle,
    project_id: i64,
    item_id: &str,
    from: &(String, String),
    to: &(String, String),
    reason: Option<OverrideReason>,
) {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return;
    };
    let reason_text = reason.map(|r| match r {
        OverrideReason::UsageLimit => " (limite de uso)",
        OverrideReason::ModelUnavailable => " (modelo indisponível)",
        OverrideReason::ManualTemp => " (troca manual)",
    });
    let text = format!(
        "Troca de provedor: {} → {}{}",
        format_provider_label(from),
        format_provider_label(to),
        reason_text.unwrap_or("")
    );
    if db.add_card_activity(project_id, item_id, &text).is_ok() {
        let _ = super::emit::emit_backlog_changed(app, project_id);
    }
}

pub(super) struct SessionStore<'a> {
    app: &'a AppHandle,
    key: SessionKey,
}

impl SessionStore<'_> {
    pub(super) fn load(&self) -> Result<Option<String>, FlowError> {
        let state = self.app.state::<DbState>();
        let Some(db) = state.db() else {
            return Ok(None);
        };
        db.agent_session(&self.key).map_err(FlowError::SessionStore)
    }

    pub(super) fn save(&self, session_id: &str) -> Result<(), FlowError> {
        self.write(|db| db.save_agent_session(&self.key, session_id))
    }

    pub(super) fn clear(&self) -> Result<(), FlowError> {
        self.write(|db| db.clear_agent_session(&self.key))
    }

    // Falha ao gravar não derruba a resposta: a sessão segue viva e o usuário é avisado.
    fn write(&self, action: impl FnOnce(&Db) -> Result<(), DbError>) -> Result<(), FlowError> {
        let state = self.app.state::<DbState>();
        let Some(db) = state.db() else {
            return Ok(());
        };
        match action(db) {
            Ok(()) => Ok(()),
            Err(error) => emit(
                self.app,
                VokiaEvent::Error {
                    message: FlowError::SessionStore(error).to_string(),
                },
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Attempt {
    Reuse,
    Open,
    Resume,
    Minimal,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Place {
    dir: PathBuf,
    item_id: Option<String>,
}

impl Place {
    fn of(project: ProjectAccess<'_>, call: &AgentCall) -> Self {
        Self {
            dir: project.dir.to_path_buf(),
            item_id: call.item_id.clone(),
        }
    }

    pub(super) fn for_preload(dir: PathBuf) -> Self {
        Self { dir, item_id: None }
    }
}

#[derive(Default)]
pub(super) struct AgentSession {
    pub(super) live: Option<LiveSession>,
    pub(super) session_id: Option<String>,
    pub(super) messages: u32,
    pub(super) recoveries: u32,
    pub(super) fresh_only: bool,
    pub(super) last_used: Option<Instant>,
    pub(super) granted: Arc<StdMutex<HashSet<String>>>,
    pub(super) pid: Arc<AtomicU32>,
    pub(super) place: Option<Place>,
    pub(super) provider_key: Option<(String, String)>,
    pub(super) preloaded_at: Option<Instant>,
}

impl AgentSession {
    // A degradação e as falhas contam por mensagem; a sessão, ao contrário do pool antigo, atravessa várias.
    pub(super) fn begin_exchange(&mut self) {
        self.recoveries = 0;
        self.fresh_only = false;
    }

    pub(super) fn forget_grants(&self) {
        if let Ok(mut granted) = self.granted.lock() {
            granted.clear();
        }
    }

    // Fecha só o processo: o session_id fica para o próximo --resume.
    pub(super) async fn end(&mut self) {
        self.close_live().await;
        self.forget_grants();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn reply(
        &mut self,
        project: ProjectAccess<'_>,
        gate: PermissionGate<'_>,
        store: &SessionStore<'_>,
        call: &AgentCall,
        prompts: &StepPrompts,
        first: bool,
        console: &ConsoleSink<'_>,
    ) -> Result<Option<Reply>, FlowError> {
        let place = Place::of(project, call);
        let provider_key = (call.adapter.config().id.clone(), call.model.clone());
        let mut next = self.first_attempt(store, &place, &provider_key).await?;
        while let Some(attempt) = next {
            let was_unused_preload = self.is_unused_preload();
            match self
                .attempt(
                    project,
                    gate.clone(),
                    call,
                    attempt,
                    prompts,
                    first,
                    console,
                )
                .await
            {
                Ok(reply) => {
                    if let Some(session_id) = &self.session_id {
                        store.save(session_id)?;
                    }
                    return Ok(Some(reply));
                }
                // Morte no meio da resposta de um processo já em uso: reenviar poderia repetir um comando; o erro vai ao feed.
                Err(error)
                    if error.is_session_loss()
                        && (attempt != Attempt::Reuse || was_unused_preload) =>
                {
                    if attempt == Attempt::Resume || was_unused_preload {
                        self.session_id = None;
                        store.clear()?;
                    }
                    next = self.after_loss(attempt);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
    }

    async fn first_attempt(
        &mut self,
        store: &SessionStore<'_>,
        place: &Place,
        provider_key: &(String, String),
    ) -> Result<Option<Attempt>, FlowError> {
        // Sessão aberta em outro diretório ou de outro item não atende este pedido; o --resume só vale para o mesmo item.
        if let Some(known) = self.place.replace(place.clone()) {
            if known != *place {
                self.close_live().await;
                if known.item_id != place.item_id {
                    self.session_id = None;
                    store.clear()?;
                }
            }
        }
        if self
            .provider_key
            .as_ref()
            .is_some_and(|known| known != provider_key)
        {
            self.close_live().await;
        }
        if self.live.is_none() {
            if self.session_id.is_none() {
                self.session_id = store.load()?.filter(|id| !id.contains(':'));
            }
            let attempt = if self.session_id.is_some() {
                Attempt::Resume
            } else {
                Attempt::Open
            };
            return Ok(Some(attempt));
        }
        if self.messages >= SESSION_ROTATE_MESSAGES {
            self.close_live().await;
            self.session_id = None;
            return Ok(Some(Attempt::Open));
        }
        if self
            .last_used
            .is_some_and(|used| used.elapsed() >= SESSION_IDLE_LIMIT)
        {
            self.close_live().await;
            return Ok(self.after_loss(Attempt::Reuse));
        }
        Ok(Some(Attempt::Reuse))
    }

    #[allow(clippy::too_many_arguments)]
    async fn attempt(
        &mut self,
        project: ProjectAccess<'_>,
        gate: PermissionGate<'_>,
        call: &AgentCall,
        attempt: Attempt,
        prompts: &StepPrompts,
        first: bool,
        console: &ConsoleSink<'_>,
    ) -> Result<Reply, ClaudeError> {
        let message = match attempt {
            Attempt::Open => &prompts.opening,
            Attempt::Reuse | Attempt::Resume if first => &prompts.opening,
            Attempt::Reuse | Attempt::Resume => &prompts.follow_up,
            Attempt::Minimal => &prompts.minimal,
        };
        let mut live = match (attempt, self.live.take()) {
            (Attempt::Reuse, Some(live)) => live,
            (_, previous) => {
                if let Some(previous) = previous {
                    previous.close(SESSION_CLOSE_GRACE).await;
                    self.pid.store(NO_PID, Ordering::SeqCst);
                }
                let resume = match attempt {
                    Attempt::Resume => self.session_id.as_deref(),
                    _ => None,
                };
                LiveSession::open(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    Access::Project(project),
                    resume,
                    open_mcp(call)?,
                )?
            }
        };
        self.pid
            .store(live.pid().unwrap_or(NO_PID), Ordering::SeqCst);
        match send_in_slot(&mut live, call, message, &gate, console).await {
            Ok(reply) => {
                self.session_id = live.session_id().map(str::to_string);
                self.live = Some(live);
                self.messages = match attempt {
                    Attempt::Open | Attempt::Minimal => 1,
                    Attempt::Reuse | Attempt::Resume => self.messages + 1,
                };
                self.last_used = Some(Instant::now());
                self.preloaded_at = None;
                self.provider_key = Some((call.adapter.config().id.clone(), call.model.clone()));
                Ok(reply)
            }
            Err(error) => {
                live.close(SESSION_CLOSE_GRACE).await;
                self.pid.store(NO_PID, Ordering::SeqCst);
                self.provider_key = None;
                Err(error)
            }
        }
    }

    fn after_loss(&mut self, failed: Attempt) -> Option<Attempt> {
        if failed == Attempt::Resume {
            return Some(Attempt::Minimal);
        }
        self.recoveries += 1;
        if self.recoveries >= SESSION_RECOVERY_LIMIT {
            self.fresh_only = true;
            return None;
        }
        match failed {
            Attempt::Minimal => {
                self.fresh_only = true;
                None
            }
            _ if self.session_id.is_some() => Some(Attempt::Resume),
            _ => Some(Attempt::Minimal),
        }
    }

    pub(super) async fn degrade(&mut self) {
        self.fresh_only = true;
        self.close_live().await;
    }

    pub(super) async fn close_live(&mut self) {
        if let Some(live) = self.live.take() {
            live.close(SESSION_CLOSE_GRACE).await;
            self.pid.store(NO_PID, Ordering::SeqCst);
            self.provider_key = None;
            self.preloaded_at = None;
        }
    }

    pub(super) fn adopt_preload(
        &mut self,
        live: LiveSession,
        session_id: Option<String>,
        place: Place,
        provider_key: (String, String),
    ) {
        self.pid
            .store(live.pid().unwrap_or(NO_PID), Ordering::SeqCst);
        self.live = Some(live);
        self.session_id = session_id;
        self.messages = 0;
        self.last_used = None;
        self.preloaded_at = Some(Instant::now());
        self.place = Some(place);
        self.provider_key = Some(provider_key);
    }

    pub(super) fn is_unused_preload(&self) -> bool {
        self.live.is_some() && self.last_used.is_none()
    }

    // A sessão usada conta desde a última resposta; a pré-carregada, desde a abertura.
    pub(super) fn is_idle(&self, limit: Duration) -> bool {
        self.live.is_some()
            && self
                .last_used
                .or(self.preloaded_at)
                .is_some_and(|since| since.elapsed() >= limit)
    }
}

// Ember, o líder geral e os líderes de time: quem distribui trabalho e administra o Vokia.
fn coordinators(orchestration: Option<&OrchestrationSnapshot>) -> Vec<Agent> {
    let mut agents = vec![Agent::ember(), leader_of(orchestration)];
    for t in orchestration.map_or(&[][..], |s| s.teams.as_slice()) {
        if let Some(leader) = t.leader_agent.as_deref().and_then(Agent::from_stored) {
            agents.push(leader);
        }
    }
    agents
}

pub(super) fn tool_scope(orchestration: Option<&OrchestrationSnapshot>, agent: &Agent) -> ToolScope {
    if coordinators(orchestration).contains(agent) {
        ToolScope::Coordinator
    } else {
        ToolScope::Worker
    }
}

// O Ember só encaminha: sem Read/Grep/Bash ele não cai em trabalho técnico, que é o que mais consome tokens nele.
fn has_builtin_tools(agent: &Agent) -> bool {
    agent != &Agent::ember()
}

pub(super) fn preload_targets(
    orchestration: Option<&OrchestrationSnapshot>,
    team: &Team,
    paused: &[Agent],
) -> Vec<Agent> {
    let mut seen = HashSet::new();
    coordinators(orchestration)
        .into_iter()
        .filter(|agent| {
            team.profile(agent).is_some() && !paused.contains(agent) && seen.insert(agent.clone())
        })
        .collect()
}

pub(super) fn preload_candidate(
    agent: &Agent,
    profile: &AgentProfile,
    providers: &ProviderRegistry,
    state: &AppState,
    global_active: Option<&LlmTarget>,
    global_fallback: &[LlmTarget],
    permission_level: PermissionLevel,
) -> Option<ResolvedCandidate> {
    resolve_candidates(
        agent,
        profile,
        providers,
        state,
        global_active,
        global_fallback,
        Some(permission_level),
    )
    .into_iter()
    .next()
}

#[cfg(test)]
#[path = "agent_session_tests.rs"]
mod provider_session_tests;

pub(super) type PreloadedSession = (LiveSession, Option<String>, Place, (String, String));

// O Claude roda sem setting sources, então o CLAUDE.md global do usuário só chega pelo prompt.
fn global_instructions(app: &AppHandle, is_claude: bool) -> Option<String> {
    if !is_claude {
        return None;
    }
    let home = match app.path().home_dir() {
        Ok(home) => home,
        Err(err) => {
            eprintln!("Não foi possível localizar a pasta do usuário: {err}");
            return None;
        }
    };
    let path = home.join(GLOBAL_INSTRUCTIONS_PATH);
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            eprintln!("Não foi possível ler {}: {err}", path.display());
            None
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn open_preloaded_session(
    app: &AppHandle,
    project_id: i64,
    project_dir: &Path,
    attachments_dir: &Path,
    permission_level: PermissionLevel,
    agent: &Agent,
    profile: &AgentProfile,
    team: &Team,
    orchestration: Option<&OrchestrationSnapshot>,
    paused: &[Agent],
    project_context: &str,
    candidate: &ResolvedCandidate,
    cli: &Path,
) -> Result<PreloadedSession, FlowError> {
    let scope = tool_scope(orchestration, agent);
    let mcp = if candidate.provider.has_tools() {
        McpLaunch::new(app, project_id, agent, &display_name(team, agent), scope)?
    } else {
        None
    };
    let tool_mode = if mcp.is_some() {
        ToolPromptMode::Mcp
    } else {
        ToolPromptMode::None
    };
    let prompt = agent_prompt(orchestration, team, agent);
    let registered_role = app
        .state::<RoleRegistry>()
        .find(&profile.role)
        .map_err(FlowError::Roles)?;
    let base = role(
        profile,
        registered_role.as_ref(),
        &prompt.roster,
        prompt.layer.as_ref(),
        project_context,
        tool_mode,
        scope,
        global_instructions(app, candidate.provider.protocol == ProviderProtocol::ClaudeLive)
            .as_deref(),
    );
    let role_text = if agent == &Agent::ember() {
        maestro_role(
            &base,
            &[],
            &project_choices(app)?,
            orchestration,
            team,
            paused,
            tool_mode,
            auto_triage_enabled(app)?,
        )
    } else {
        base
    };
    let (program, user_args) = resolve_program_for(agent, &candidate.command, cli)?;
    let store = SessionStore {
        app,
        key: SessionKey {
            project_id,
            agent: agent.clone(),
        },
    };
    let resume = store.load()?;
    let mcp_session = mcp.as_ref().map(McpLaunch::open).transpose()?;
    let access = Access::Project(ProjectAccess {
        dir: project_dir,
        attachments: attachments_dir,
        level: permission_level,
        builtin_tools: has_builtin_tools(agent),
    });
    let live = LiveSession::open(
        &program,
        &user_args,
        &role_text,
        &candidate.model,
        access,
        resume.as_deref(),
        mcp_session,
    )?;
    let place = Place::for_preload(project_dir.to_path_buf());
    let provider_key = (candidate.cli.clone(), candidate.model.clone());
    Ok((live, resume, place, provider_key))
}

pub(super) struct AgentCall {
    pub(super) adapter: ProviderAdapter,
    pub(super) program: PathBuf,
    pub(super) user_args: Vec<String>,
    pub(super) role: String,
    pub(super) model: String,
    pub(super) resume_session: Option<String>,
    item_id: Option<String>,
    channel_id: Option<i64>,
    mcp: Option<McpLaunch>,
    has_variables: bool,
    collected: StdMutex<ExchangeOutcome>,
}

impl AgentCall {
    pub(super) fn for_text_ask(
        adapter: ProviderAdapter,
        program: PathBuf,
        user_args: Vec<String>,
        role: String,
        model: String,
    ) -> Self {
        Self {
            adapter,
            program,
            user_args,
            role,
            model,
            resume_session: None,
            item_id: None,
            channel_id: None,
            mcp: None,
            has_variables: false,
            collected: StdMutex::default(),
        }
    }

    pub(super) fn item_id(&self) -> Option<&str> {
        self.item_id.as_deref()
    }

    pub(super) fn has_mcp(&self) -> bool {
        self.mcp.is_some()
    }

    pub(super) fn has_collected_actions(&self) -> bool {
        self.collected.lock().is_ok_and(|outcome| {
            !outcome.confirmations.is_empty()
                || outcome.decision.is_some()
                || outcome.proposal.is_some()
                || !outcome.delegations.is_empty()
                || !outcome.variables.accepted.is_empty()
        })
    }

    pub(super) fn begin_mcp_slot(&self, session: &McpSession) {
        let slot = session.slot();
        slot.begin(self.item_id.as_deref(), self.channel_id);
        if self.has_variables {
            slot.enable_variables();
        }
    }

    pub(super) fn finish_mcp_slot(&self, session: &McpSession) {
        let outcome = session.slot().end();
        if let Ok(mut collected) = self.collected.lock() {
            *collected = outcome;
        }
    }
}

// Cada abertura de sessão da CLI ganha Binding, token e arquivo próprios; `--resume` nunca reaproveita o token antigo.
struct McpLaunch {
    server: McpServer,
    dir: PathBuf,
    project_id: i64,
    agent: Agent,
    agent_name: String,
    scope: ToolScope,
}

impl McpLaunch {
    // Servidor fora do ar: o agente sobe sem MCP e sem regra de ferramentas.
    fn new(
        app: &AppHandle,
        project_id: i64,
        agent: &Agent,
        agent_name: &str,
        scope: ToolScope,
    ) -> Result<Option<McpLaunch>, FlowError> {
        let state = app.state::<McpState>();
        let Some(server) = state.server() else {
            return Ok(None);
        };
        let data_dir = app.path().app_data_dir().map_err(FlowError::ArchivePath)?;
        Ok(Some(McpLaunch {
            server: server.clone(),
            dir: data_dir.join(CONFIG_DIR),
            project_id,
            agent: agent.clone(),
            agent_name: agent_name.to_string(),
            scope,
        }))
    }

    fn open(&self) -> Result<McpSession, ClaudeError> {
        let binding = Binding {
            project_id: self.project_id,
            agent: self.agent.clone(),
            agent_name: self.agent_name.clone(),
            slot: ExchangeSlot::default(),
            scope: self.scope,
        };
        self.server
            .open_session(&self.dir, binding)
            .map_err(|error| ClaudeError::Mcp(error.to_string()))
    }

    fn open_headless(&self) -> Result<McpSession, ClaudeError> {
        let binding = Binding {
            project_id: self.project_id,
            agent: self.agent.clone(),
            agent_name: self.agent_name.clone(),
            slot: ExchangeSlot::default(),
            scope: self.scope,
        };
        self.server
            .open_headless(binding)
            .map_err(|error| ClaudeError::Mcp(error.to_string()))
    }
}

pub(super) fn open_mcp(call: &AgentCall) -> Result<Option<McpSession>, ClaudeError> {
    call.mcp.as_ref().map(McpLaunch::open).transpose()
}

pub(super) fn open_mcp_headless(call: &AgentCall) -> Result<Option<McpSession>, ClaudeError> {
    call.mcp.as_ref().map(McpLaunch::open_headless).transpose()
}

pub(super) async fn send_in_slot(
    live: &mut LiveSession,
    call: &AgentCall,
    message: &str,
    gate: &PermissionGate<'_>,
    console: &ConsoleSink<'_>,
) -> Result<Reply, ClaudeError> {
    let slot = live.exchange_slot().cloned();
    if let Some(slot) = &slot {
        slot.begin(call.item_id.as_deref(), call.channel_id);
        if call.has_variables {
            slot.enable_variables();
        }
    }
    let reply = live
        .send(
            message,
            |request| gate.clone().decide(request),
            |line| console.record(line),
            |activity| console.live_activity(activity),
        )
        .await;
    if let Some(slot) = &slot {
        // Também na falha: o card já gravado numa tentativa que caiu continua precisando de confirmação.
        let outcome = slot.end();
        if let Ok(mut collected) = call.collected.lock() {
            collected.absorb(outcome);
        }
    }
    reply
}
