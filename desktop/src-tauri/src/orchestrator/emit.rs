use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::agents::Agent;
use crate::claude::{LimitWindow, RateLimits, TurnActivity};
use crate::db::DbState;
use crate::events::{AgentState, RequestProposal, Speaker, UsageWindow, VokiaEvent, EVENT_NAME};

use super::error::FlowError;

const PERCENT_SCALE: f64 = 100.0;

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

pub(super) fn emit_message(
    app: &AppHandle,
    project_id: i64,
    from: Speaker,
    to: Speaker,
    text: &str,
    channel_id: Option<i64>,
    activity: Option<TurnActivity>,
    request_proposal: Option<RequestProposal>,
) -> Result<(), FlowError> {
    let channel_id = channel_id.filter(|_| from == Speaker::User || to == Speaker::User);
    let time = now_millis()?;
    if let Some(db) = app.state::<DbState>().db() {
        db.insert_message(
            project_id,
            from.clone(),
            to.clone(),
            text,
            time,
            channel_id,
            activity.as_ref(),
            request_proposal.as_ref(),
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
            activity,
            request_proposal,
        },
    )
}

pub(crate) fn emit_backlog_changed(app: &AppHandle, project_id: i64) -> Result<(), FlowError> {
    emit(app, VokiaEvent::BacklogChanged { project_id })
}

pub(super) fn emit_state(
    app: &AppHandle,
    project_id: i64,
    agent: Agent,
    state: AgentState,
) -> Result<(), FlowError> {
    emit(
        app,
        VokiaEvent::AgentState {
            agent,
            state,
            project_id: Some(project_id),
        },
    )
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
