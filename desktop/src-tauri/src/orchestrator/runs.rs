use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager};

use crate::agents::{replica_of, Agent, AgentError, AgentRegistry};
use crate::claude::UsageLimit;
use crate::db::{DbState, InterruptedRun, RunHandoff, RunMessage, RunProgress};
use crate::events::{AgentState, VokiaEvent};
use crate::session_key::SessionKey;
use crate::state::{AppState, RequestContext};
use crate::task::truncate;

use super::emit::{emit, emit_state, now_millis};
use super::error::{FlowError, RequestOutcome};
use super::flow::handle_request;

const JOURNAL_ERROR_PREFIX: &str = "Não foi possível registrar o pedido: ";
const JOURNAL_CLOSE_ERROR_PREFIX: &str = "Não foi possível fechar o registro do pedido: ";
const JOURNAL_PAUSE_ERROR_PREFIX: &str = "Não foi possível registrar a pausa do pedido: ";
pub(super) const RESUME_NOTE: &str = "Retomando o pedido interrompido de onde parou.";
const RESUME_HEADING: &str = "Retomada de pedido interrompido: o app fechou antes do fim. Continue de onde parou: não refaça o que já foi feito nem repita respostas já dadas.";
const ORIGINAL_REQUEST_LABEL: &str = "Pedido original:";
const ORIGINAL_AGENT_LABEL: &str = "Agente que atendia:";
const CARD_LABEL: &str = "Card:";
const HANDOFFS_HEADING: &str = "Delegações já feitas:";
const MESSAGES_HEADING: &str = "Mensagens já trocadas (mais antigas primeiro):";
const TOOLS_LABEL: &str = "ferramentas:";
const NOTHING_RECORDED: &str = "Nada foi registrado antes da interrupção: comece pelo pedido original.";
const EMBER_COORDINATION: &str = "Coordene a continuação: confira o estado dos cards e das delegações acima e delegue de novo só o que ficou pela metade.";
const PROGRESS_TEXT_MAX_CHARS: usize = 1500;

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

const AUTO_RESUME_BUFFER_SECS: u64 = 2;

pub(super) fn auto_pause_on_limit(app: &AppHandle) -> bool {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return false;
    };
    db.auto_pause_on_limit().unwrap_or(false)
}

pub(super) fn is_agent_paused(app: &AppHandle, project_id: i64, agent: &Agent) -> bool {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return false;
    };
    db.is_agent_paused(project_id, agent).unwrap_or(false)
}

pub(crate) fn emit_agent_pauses_changed(app: &AppHandle) -> Result<(), FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(());
    };
    let pauses = db.agent_pauses().map_err(FlowError::Runs)?;
    emit(app, VokiaEvent::AgentPausesChanged { pauses })
}

fn project_team_agents(app: &AppHandle) -> Vec<Agent> {
    match app.state::<AgentRegistry>().team() {
        Ok(team) => team.ids().cloned().collect(),
        Err(_) => vec![Agent::ember(), Agent::axiom(), Agent::vexa()],
    }
}

fn interrupt_in_db(app: &AppHandle, id: Option<i64>) -> Result<(), FlowError> {
    let Some(id) = id else {
        return Ok(());
    };
    let state = app.state::<DbState>();
    if let Err(error) = state.require().and_then(|db| db.interrupt_run(id)) {
        emit(
            app,
            VokiaEvent::Error {
                message: format!("{JOURNAL_PAUSE_ERROR_PREFIX}{error}."),
            },
        )?;
    }
    Ok(())
}

// Pausa um pedido segurado por pausa manual ou do projeto: interrompido (retomável) e agente ocioso.
pub(super) fn interrupt_paused_run(
    app: &AppHandle,
    key: &SessionKey,
    id: Option<i64>,
    item_id: Option<&str>,
) -> Result<(), FlowError> {
    interrupt_in_db(app, id)?;
    emit_state(app, key.project_id, key.agent.clone(), AgentState::Idle, item_id)
}

fn schedule_auto_resume(app: AppHandle, project_id: i64, run_id: i64, resets_at: u64) {
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let wait_secs = resets_at
        .saturating_sub(now_secs)
        .saturating_add(AUTO_RESUME_BUFFER_SECS);
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(wait_secs)).await;
        if !auto_pause_on_limit(&app) {
            return;
        }
        let still_interrupted = app
            .state::<DbState>()
            .db()
            .and_then(|db| db.interrupted_runs().ok())
            .is_some_and(|runs| {
                runs.iter()
                    .any(|run| run.id == run_id && run.project_id == project_id)
            });
        if !still_interrupted {
            return;
        }
        let _ = resume_run(&app, &app.state::<AppState>(), run_id).await;
    });
}

fn base_agent_obj(agent: &Agent) -> Agent {
    replica_of(agent.as_str())
        .and_then(|(base, _)| Agent::from_stored(base))
        .unwrap_or_else(|| agent.clone())
}

// Pausa por limite de uso: o pedido fica interrompido (retomável) e todo o time do projeto entra em pausa real.
pub(super) fn pause_run(
    app: &AppHandle,
    key: &SessionKey,
    id: Option<i64>,
    limit: UsageLimit,
    item_id: Option<&str>,
) -> Result<(), FlowError> {
    interrupt_in_db(app, id)?;
    let agents = project_team_agents(app);
    if let Some(db) = app.state::<DbState>().db() {
        if let Err(error) = db.pause_project_agents(key.project_id, &agents) {
            emit(
                app,
                VokiaEvent::Error {
                    message: format!("{JOURNAL_PAUSE_ERROR_PREFIX}{error}."),
                },
            )?;
        }
    }
    emit_agent_pauses_changed(app)?;
    for agent in agents {
        let (state, agent_item) = if agent == key.agent {
            (AgentState::Waiting, item_id)
        } else {
            (AgentState::Idle, None)
        };
        emit_state(app, key.project_id, agent, state, agent_item)?;
    }
    if key.agent != base_agent_obj(&key.agent) {
        emit_state(
            app,
            key.project_id,
            key.agent.clone(),
            AgentState::Waiting,
            item_id,
        )?;
    }
    emit(
        app,
        VokiaEvent::UsageLimit {
            agent: key.agent.clone(),
            project_id: key.project_id,
            window: limit.window,
            resets_at: limit.resets_at,
            run_id: id,
        },
    )?;
    if limit.resets_at > 0 {
        if let Some(run_id) = id {
            schedule_auto_resume(app.clone(), key.project_id, run_id, limit.resets_at);
        }
    }
    Ok(())
}

pub async fn resume_run(
    app: &AppHandle,
    state: &AppState,
    id: i64,
) -> Result<RequestOutcome, FlowError> {
    let run = claim(app, id)?;
    if let Some(db) = app.state::<DbState>().db() {
        let _ = db.resume_project_agents(run.project_id);
        let _ = emit_agent_pauses_changed(app);
    }
    match continue_run(app, state, &run).await {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            release(app, id)?;
            Err(error)
        }
    }
}

async fn continue_run(
    app: &AppHandle,
    state: &AppState,
    run: &InterruptedRun,
) -> Result<RequestOutcome, FlowError> {
    let agent = Agent::from_stored(&run.agent)
        .ok_or_else(|| FlowError::Team(AgentError::UnknownAgent(run.agent.clone())))?;
    let db_state = app.state::<DbState>();
    let progress = db_state
        .require()
        .and_then(|db| db.run_progress(run))
        .map_err(FlowError::Runs)?;
    // Fluxo que já delegou não cabe a um agente só: o Ember recebe o estado parcial e coordena a continuação.
    let to_ember = !progress.handoffs.is_empty() && agent != Agent::ember();
    let (recipient, item_id) = if to_ember {
        (Agent::ember(), None)
    } else {
        (agent, run.item_id.clone())
    };
    let text = continuation(run, &progress, to_ember);
    let mut context = RequestContext::new(
        run.run_id.and_then(|id| u64::try_from(id).ok()),
        item_id,
        None,
    );
    context.resumed_run = Some(run.id);
    handle_request(app, state, &text, recipient, run.project_id, context).await
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

fn continuation(run: &InterruptedRun, progress: &RunProgress, to_ember: bool) -> String {
    let mut lines = vec![
        RESUME_HEADING.to_string(),
        format!("{ORIGINAL_REQUEST_LABEL} {}", run.request),
        format!("{ORIGINAL_AGENT_LABEL} {}", run.agent),
    ];
    if let Some(item) = &run.item_id {
        lines.push(format!("{CARD_LABEL} {item}"));
    }
    if progress.is_empty() {
        lines.push(NOTHING_RECORDED.to_string());
    }
    if !progress.handoffs.is_empty() {
        lines.push(HANDOFFS_HEADING.to_string());
        lines.extend(progress.handoffs.iter().map(handoff_line));
    }
    if !progress.messages.is_empty() {
        lines.push(MESSAGES_HEADING.to_string());
        lines.extend(progress.messages.iter().flat_map(message_lines));
    }
    if to_ember {
        lines.push(EMBER_COORDINATION.to_string());
    }
    lines.join("\n")
}

fn handoff_line(handoff: &RunHandoff) -> String {
    let route = format!("{} → {}", handoff.from_agent, handoff.to_agent);
    match &handoff.item_id {
        Some(item) => format!("- {route} ({} de {item})", handoff.kind),
        None => format!("- {route} ({})", handoff.kind),
    }
}

fn message_lines(message: &RunMessage) -> Vec<String> {
    let mut lines = vec![format!(
        "- {} → {}: {}",
        message.from_agent,
        message.to_agent,
        truncate(&message.text, PROGRESS_TEXT_MAX_CHARS)
    )];
    let tools: Vec<String> = message
        .activity
        .iter()
        .flat_map(|activity| activity.tools.iter())
        .map(|tool| format!("{} {}", tool.name, tool.target).trim_end().to_string())
        .collect();
    if !tools.is_empty() {
        lines.push(format!("  {TOOLS_LABEL} {}", tools.join("; ")));
    }
    lines
}

#[cfg(test)]
#[path = "../../tests/resume_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/auto_pause_limit_tests.rs"]
mod auto_pause_limit_tests;