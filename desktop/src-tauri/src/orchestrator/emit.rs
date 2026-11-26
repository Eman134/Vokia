use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::agents::Agent;
use crate::claude::{LimitWindow, RateLimits, TurnActivity, TurnUsage};
use crate::db::DbState;
use crate::events::{AgentState, RequestProposal, Speaker, UsageWindow, VokiaEvent, EVENT_NAME};
use crate::session_key::SessionKey;

use super::error::FlowError;
use super::flow::drain_all_waiting;
use crate::state::AppState;

const PERCENT_SCALE: f64 = 100.0;

// Onde a mensagem pertence: o canal de onde o pedido veio e a task (card) que a gerou.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MessageScope<'a> {
    pub(crate) channel_id: Option<i64>,
    pub(crate) item_id: Option<&'a str>,
}

pub(super) fn report_limits(app: &AppHandle, limits: Option<RateLimits>) -> Result<(), FlowError> {
    let Some(limits) = limits else {
        return Ok(());
    };
    let updated_at_ms = now_millis()?;
    let provider_usage = crate::usage::ProviderPlanUsage::from_claude_rate_limits(limits, updated_at_ms);
    if let Some(state) = app.try_state::<AppState>() {
        state.usage.record_provider_usage(provider_usage.clone());
        let exhausted = state
            .usage
            .pause_threshold()
            .is_some_and(|threshold| provider_usage.is_exhausted(threshold));
        if !exhausted {
            let released = state.usage.confirm_provider_renewed("claude", None);
            if !released.returns.is_empty() {
                let mut overrides = state
                    .temporary_overrides
                    .lock()
                    .unwrap_or_else(|e| e.into_inner());
                let mut changed = false;
                for ret in released.returns {
                    let should_remove = overrides.get(&ret.agent).is_some_and(|ov| {
                        ov.reason == crate::orchestrator::fallback::OverrideReason::UsageLimit
                            && ov.original_cli == ret.original_provider_id
                    });
                    if should_remove {
                        overrides.remove(&ret.agent);
                        changed = true;
                    }
                }
                drop(overrides);
                if changed {
                    crate::orchestrator::fallback::emit_overrides_changed(app, &state);
                }
            }
        }
    }
    emit(
        app,
        VokiaEvent::ProviderUsageUpdated {
            usage: provider_usage,
        },
    )?;
    emit(
        app,
        VokiaEvent::PlanUsage {
            five_hour: usage_window(limits.five_hour),
            seven_day: usage_window(limits.seven_day),
        },
    )
}

fn usage_window(window: LimitWindow) -> UsageWindow {
    UsageWindow {
        percent: (window.utilization * PERCENT_SCALE)
            .round()
            .clamp(0.0, PERCENT_SCALE) as u8,
        resets_at: window.resets_at,
    }
}

// Aviso já gravado no banco pela operação que o gerou: só leva o evento ao feed aberto.
pub(crate) fn emit_feed_note(
    app: &AppHandle,
    project_id: i64,
    from: Speaker,
    text: &str,
    time: u64,
) -> Result<(), FlowError> {
    emit(
        app,
        VokiaEvent::Message {
            from,
            to: Speaker::User,
            text: text.to_string(),
            time,
            project_id: Some(project_id),
            channel_id: None,
            item_id: None,
            activity: None,
            request_proposal: None,
            usage: None,
            is_error: false,
            foreign_instance: false,
        },
    )
}

#[allow(clippy::too_many_arguments)] // cada argumento é um campo do evento de mensagem
pub(crate) fn emit_message(
    app: &AppHandle,
    project_id: i64,
    from: Speaker,
    to: Speaker,
    text: &str,
    scope: MessageScope<'_>,
    activity: Option<TurnActivity>,
    request_proposal: Option<RequestProposal>,
    usage: Option<TurnUsage>,
) -> Result<(), FlowError> {
    let MessageScope { channel_id, item_id } = scope;
    let time = now_millis()?;
    if let Some(db) = app.state::<DbState>().db() {
        db.insert_message(
            project_id,
            from.clone(),
            to.clone(),
            text,
            time,
            channel_id,
            item_id,
            activity.as_ref(),
            request_proposal.as_ref(),
            usage.as_ref(),
        )?;
    }
    emit(
        app,
        VokiaEvent::Message {
            from,
            to,
            text: text.to_string(),
            time,
            project_id: Some(project_id),
            channel_id,
            item_id: item_id.map(str::to_string),
            activity,
            request_proposal,
            usage,
            is_error: false,
            foreign_instance: false,
        },
    )
}

pub(crate) fn emit_error_message(
    app: &AppHandle,
    project_id: i64,
    from: Speaker,
    scope: MessageScope<'_>,
    text: &str,
) -> Result<(), FlowError> {
    let MessageScope { channel_id, item_id } = scope;
    let time = now_millis()?;
    if let Some(db) = app.state::<DbState>().db() {
        db.insert_message(
            project_id,
            from.clone(),
            Speaker::User,
            text,
            time,
            channel_id,
            item_id,
            None,
            None,
            None,
        )?;
    }
    emit(
        app,
        VokiaEvent::Message {
            from,
            to: Speaker::User,
            text: text.to_string(),
            time,
            project_id: Some(project_id),
            channel_id,
            item_id: item_id.map(str::to_string),
            activity: None,
            request_proposal: None,
            usage: None,
            is_error: true,
            foreign_instance: false,
        },
    )
}

// Um card pode ter saído de In progress: as delegações que esperavam worktree livre tentam de novo.
pub(crate) fn emit_backlog_changed(app: &AppHandle, project_id: i64) -> Result<(), FlowError> {
    emit(app, VokiaEvent::BacklogChanged { project_id })?;
    if let Some(state) = app.try_state::<AppState>() {
        drain_all_waiting(app, &state);
    }
    Ok(())
}

// Só avisa a tela: um artefato novo não libera worktree nem delegação em espera.
pub(crate) fn emit_artifacts_changed(app: &AppHandle, project_id: i64) -> Result<(), FlowError> {
    emit(app, VokiaEvent::ArtifactsChanged { project_id })
}

pub(crate) fn record_agent_state(
    app: &AppHandle,
    project_id: i64,
    agent: &Agent,
    state: AgentState,
) {
    if let Some(app_state) = app.try_state::<AppState>() {
        if let Ok(mut flow) = app_state.flow.lock() {
            if state == AgentState::Idle {
                flow.agent_states.remove(&(project_id, agent.clone()));
            } else {
                flow.agent_states
                    .insert((project_id, agent.clone()), state);
            }
        }
    }
}

// Qualquer agente em Waiting sem execução ativa, batch aberto, decisão pendente nem permissão pendente volta para Idle.
pub(crate) fn reconcile_waiting(app: &AppHandle, state: &AppState, project_id: i64) {
    let candidates: Vec<Agent> = {
        let Ok(flow) = state.flow.lock() else {
            return;
        };
        let decisions = state.decisions.lock().ok();
        flow.agent_states
            .iter()
            .filter(|((pid, _), agent_state)| {
                *pid == project_id && **agent_state == AgentState::Waiting
            })
            .filter(|((_, agent), _)| {
                let has_execution = flow.pending.contains_key(&SessionKey {
                    project_id,
                    agent: agent.clone(),
                }) || flow
                    .turn_waiters
                    .iter()
                    .any(|(key, waiters)| key.project_id == project_id && waiters.contains(agent));
                let has_batch = flow
                    .batches
                    .values()
                    .any(|batch| batch.project_id == project_id && &batch.leader == agent);
                let has_decision = decisions.as_ref().is_some_and(|map| {
                    map.values().any(|decision| {
                        decision.project_id == project_id && &decision.agent == agent
                    })
                });
                let has_permission = state.permissions.has_pending(project_id, agent);
                !has_execution && !has_batch && !has_decision && !has_permission
            })
            .map(|((_, agent), _)| agent.clone())
            .collect()
    };
    for agent in candidates {
        let _ = emit_state(app, project_id, agent, AgentState::Idle, None);
    }
}

pub(super) fn emit_state(
    app: &AppHandle,
    project_id: i64,
    agent: Agent,
    state: AgentState,
    item_id: Option<&str>,
) -> Result<(), FlowError> {
    record_agent_state(app, project_id, &agent, state);
    if matches!(state, AgentState::Sending | AgentState::Thinking) {
        mark_delegations_activated(app, project_id, &agent)?;
    }
    emit(
        app,
        VokiaEvent::AgentState {
            agent,
            state,
            project_id: Some(project_id),
            item_id: item_id.map(str::to_string),
        },
    )
}

fn mark_delegations_activated(app: &AppHandle, project_id: i64, agent: &Agent) -> Result<(), FlowError> {
    let state = app.state::<AppState>();
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    flow.active_delegations
        .values_mut()
        .filter(|delegation| delegation.project_id == project_id && &delegation.to_agent == agent)
        .for_each(|delegation| delegation.activated = true);
    Ok(())
}

pub(crate) fn emit(app: &AppHandle, event: VokiaEvent) -> Result<(), FlowError> {
    Ok(app.emit(EVENT_NAME, event)?)
}

pub fn now_millis() -> Result<u64, FlowError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(FlowError::Clock)?;
    Ok(elapsed.as_millis() as u64)
}
