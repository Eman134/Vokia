use tauri::{AppHandle, Manager, State};

use crate::agents::{Agent, AgentError, AgentRegistry};
use crate::attachments;
use crate::claude::{self, CliProblem};
use crate::console_log::ConsoleEntry;
use crate::db::{AgentPause, DbState, InterruptedRun};
use crate::orchestrator::{self, RequestOutcome};
use crate::permissions::{Answer, PermissionError};
use crate::session_key::SessionKey;
use crate::state::{AppState, RequestContext};

const PERMISSION_REASON_MAX_CHARS: usize = 500;

#[tauri::command]
pub fn cli_problem(state: State<'_, AppState>) -> Option<CliProblem> {
    state.cli.as_ref().err().cloned()
}

#[tauri::command]
pub async fn check_connection() -> bool {
    claude::is_reachable().await
}

#[tauri::command]
pub fn agent_console(
    state: State<'_, AppState>,
    project_id: i64,
    agent: Agent,
) -> Vec<ConsoleEntry> {
    state.console.snapshot(&SessionKey { project_id, agent })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // argumentos do comando Tauri vêm do front um a um
pub async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    to: Agent,
    project_id: i64,
    run_id: Option<u64>,
    item_id: Option<String>,
    channel_id: Option<i64>,
    attachments: Option<Vec<String>>,
) -> Result<RequestOutcome, String> {
    ensure_agent_exists(&app.state::<AgentRegistry>(), &to)?;
    let attachment_ids = attachments.unwrap_or_default();
    let text = if attachment_ids.is_empty() {
        text
    } else {
        let db = app.state::<DbState>();
        let db = db.require().map_err(|error| error.to_string())?;
        let taken = attachments::find_for_send(db, project_id, &attachment_ids)
            .map_err(|error| error.to_string())?;
        attachments::append_block(&text, &taken)
    };
    let mut context = RequestContext::new(run_id, item_id, channel_id);
    context.attachments = attachment_ids;
    match orchestrator::handle_request(&app, &state, &text, to, project_id, context).await {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            orchestrator::report_failure(&app, project_id, &error)
                .map_err(|emit_error| emit_error.to_string())?;
            Ok(RequestOutcome::Failed)
        }
    }
}

// Corta por caracteres, não por bytes, para não partir UTF-8.
fn clean_reason(reason: Option<String>) -> Option<String> {
    let text: String = reason?
        .trim()
        .chars()
        .take(PERMISSION_REASON_MAX_CHARS)
        .collect();
    (!text.is_empty()).then_some(text)
}

#[tauri::command]
pub fn answer_permission(
    state: State<'_, AppState>,
    id: String,
    allow: bool,
    always: bool,
    reason: Option<String>,
) -> Result<(), String> {
    let answer = if allow {
        Answer::Allow { always }
    } else {
        Answer::Deny {
            reason: clean_reason(reason),
        }
    };
    match state.permissions.answer(&id, answer) {
        // A outra superfície já decidiu ou o pedido expirou: nada a mostrar.
        Ok(()) | Err(PermissionError::Unknown) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn answer_decision(
    app: AppHandle,
    state: State<'_, AppState>,
    id: u64,
    answer: String,
) -> Result<(), String> {
    orchestrator::answer_decision(&app, &state, id, &answer)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_agent_pauses(state: State<'_, DbState>) -> Result<Vec<AgentPause>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.agent_pauses().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_interrupted_runs(state: State<'_, DbState>) -> Result<Vec<InterruptedRun>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.interrupted_runs().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn resume_interrupted_run(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<RequestOutcome, String> {
    orchestrator::resume_run(&app, &state, id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn discard_interrupted_run(state: State<'_, DbState>, id: i64) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.discard_interrupted_run(id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_agent_paused(
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    agent: Agent,
    paused: bool,
) -> Result<(), String> {
    ensure_agent_exists(&registry, &agent)?;
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_agent_paused(project_id, agent, paused)
        .map_err(|error| error.to_string())
}

fn ensure_agent_exists(registry: &AgentRegistry, agent: &Agent) -> Result<(), String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    if team.profile(agent).is_none() {
        return Err(AgentError::UnknownAgent(agent.to_string()).to_string());
    }
    Ok(())
}
