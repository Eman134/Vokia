use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::agents::Agent;
use crate::claude::{LimitWindow, RateLimits, TurnActivity, TurnUsage};
use crate::db::DbState;
use crate::events::{AgentState, RequestProposal, Speaker, UsageWindow, VokiaEvent, EVENT_NAME};

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

pub(super) fn emit_state(
    app: &AppHandle,
    project_id: i64,
    agent: Agent,
    state: AgentState,
    item_id: Option<&str>,
) -> Result<(), FlowError> {
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

pub(super) fn emit(app: &AppHandle, event: VokiaEvent) -> Result<(), FlowError> {
    Ok(app.emit(EVENT_NAME, event)?)
}

pub fn now_millis() -> Result<u64, FlowError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(FlowError::Clock)?;
    Ok(elapsed.as_millis() as u64)
}
