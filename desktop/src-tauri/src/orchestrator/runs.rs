use tauri::{AppHandle, Manager};

use crate::agents::{replica_of, Agent, AgentError};
use crate::claude::UsageLimit;
use crate::db::{DbState, InterruptedRun, RunHandoff};
use crate::events::{AgentState, VokiaEvent};
use crate::session_key::SessionKey;
use crate::state::{AppState, RequestContext};

use super::emit::{emit, emit_state, now_millis};
use super::error::{FlowError, RequestOutcome};
use super::flow::handle_request;

const JOURNAL_ERROR_PREFIX: &str = "Não foi possível registrar o pedido: ";
const JOURNAL_CLOSE_ERROR_PREFIX: &str = "Não foi possível fechar o registro do pedido: ";
const JOURNAL_PAUSE_ERROR_PREFIX: &str = "Não foi possível registrar a pausa do pedido: ";
const RUN_CONTEXT_HEADING: &str = "Contexto do run interrompido (delegações já feitas):";

// Réplica grava o registro no nome do agente base: é ele que existe no time e pode ser retomado.
fn base_agent(agent: &Agent) -> &str {
    replica_of(agent.as_str()).map_or(agent.as_str(), |(base, _)| base)
}

fn millis_now() -> Result<i64, FlowError> {
    Ok(i64::try_from(now_millis()?).expect("milissegundos desde 1970 cabem em i64"))
}

// Sem registro o pedido segue (o aviso vai ao feed); o que se perde é só a chance de retomá-lo.
pub(super) fn begin_run(
    app: &AppHandle,
    key: &SessionKey,
    request: &str,
    item_id: Option<&str>,
    run_id: Option<u64>,
) -> Result<Option<i64>, FlowError> {
    let started_at = millis_now()?;
    let run_id = run_id.and_then(|id| i64::try_from(id).ok());
    let state = app.state::<DbState>();
    let result = state.require().and_then(|db| {
        db.start_run(
            key.project_id,
            base_agent(&key.agent),
            request,
            run_id,
            item_id,
            started_at,
        )
    });
    match result {
        Ok(id) => Ok(Some(id)),
        Err(error) => {
            emit(
                app,
                VokiaEvent::Error {
                    message: format!("{JOURNAL_ERROR_PREFIX}{error}."),
                },
            )?;
            Ok(None)
        }
    }
}

pub(super) fn end_run(app: &AppHandle, id: Option<i64>) -> Result<(), FlowError> {
    let Some(id) = id else {
        return Ok(());
    };
    let ended_at = millis_now()?;
    let state = app.state::<DbState>();
    match state.require().and_then(|db| db.finish_run(id, ended_at)) {
        Ok(()) => Ok(()),
        Err(error) => emit(
            app,
            VokiaEvent::Error {
                message: format!("{JOURNAL_CLOSE_ERROR_PREFIX}{error}."),
            },
        ),
    }
}

// Pausa por limite de uso: o pedido fica interrompido (retomável) e o agente aguarda, sem erro no feed.
pub(super) fn pause_run(
    app: &AppHandle,
    key: &SessionKey,
    id: Option<i64>,
    limit: UsageLimit,
) -> Result<(), FlowError> {
    if let Some(id) = id {
        let state = app.state::<DbState>();
        if let Err(error) = state.require().and_then(|db| db.interrupt_run(id)) {
            emit(
                app,
                VokiaEvent::Error {
                    message: format!("{JOURNAL_PAUSE_ERROR_PREFIX}{error}."),
                },
            )?;
        }
    }
    emit_state(app, key.project_id, key.agent.clone(), AgentState::Waiting)?;
    emit(
        app,
        VokiaEvent::UsageLimit {
            agent: key.agent.clone(),
            project_id: key.project_id,
            window: limit.window,
            resets_at: limit.resets_at,
            run_id: id,
        },
    )
}

pub async fn resume_run(
    app: &AppHandle,
    state: &AppState,
    id: i64,
) -> Result<RequestOutcome, FlowError> {
    let run = claim(app, id)?;
    let agent = Agent::from_stored(&run.agent)
        .ok_or_else(|| FlowError::Team(AgentError::UnknownAgent(run.agent.clone())))?;
    let text = if run.leader {
        with_run_context(app, &run)?
    } else {
        run.request.clone()
    };
    let context = RequestContext::new(
        run.run_id.and_then(|id| u64::try_from(id).ok()),
        run.item_id.clone(),
        None,
    );
    match handle_request(app, state, &text, agent, run.project_id, context).await {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            release(app, id)?;
            Err(error)
        }
    }
}

fn claim(app: &AppHandle, id: i64) -> Result<InterruptedRun, FlowError> {
    let state = app.state::<DbState>();
    state
        .require()
        .and_then(|db| db.claim_interrupted_run(id))
        .map_err(FlowError::Runs)
}

fn release(app: &AppHandle, id: i64) -> Result<(), FlowError> {
    let state = app.state::<DbState>();
    state
        .require()
        .and_then(|db| db.release_run(id))
        .map_err(FlowError::Runs)
}

fn with_run_context(app: &AppHandle, run: &InterruptedRun) -> Result<String, FlowError> {
    let state = app.state::<DbState>();
    let handoffs = state
        .require()
        .and_then(|db| db.run_handoffs(run))
        .map_err(FlowError::Runs)?;
    if handoffs.is_empty() {
        return Ok(run.request.clone());
    }
    let lines: Vec<String> = handoffs.iter().map(handoff_line).collect();
    Ok(format!(
        "{}\n\n{RUN_CONTEXT_HEADING}\n{}",
        run.request,
        lines.join("\n")
    ))
}

fn handoff_line(handoff: &RunHandoff) -> String {
    let route = format!("{} → {}", handoff.from_agent, handoff.to_agent);
    match &handoff.item_id {
        Some(item) => format!("- {route} ({} de {item})", handoff.kind),
        None => format!("- {route} ({})", handoff.kind),
    }
}
