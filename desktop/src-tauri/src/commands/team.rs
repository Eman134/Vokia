use tauri::{AppHandle, Manager, State};

use crate::agents::{Agent, AgentError, AgentRegistry};
use crate::claude::{self, CliProblem};
use crate::console_log::ConsoleEntry;
use crate::db::{AgentPause, DbState};
use crate::orchestrator::{self, RequestOutcome};
use crate::permissions::PermissionError;
use crate::session_key::SessionKey;
use crate::state::{AppState, RequestContext};

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
pub async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    to: Agent,
    project_id: i64,
    run_id: Option<u64>,
    item_id: Option<String>,
    channel_id: Option<i64>,
) -> Result<RequestOutcome, String> {
    ensure_agent_exists(&app.state::<AgentRegistry>(), &to)?;
    let context = RequestContext::new(run_id, item_id, channel_id);
    match orchestrator::handle_request(&app, &state, &text, to, project_id, context).await {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            orchestrator::report_failure(&app, project_id, &error)
                .map_err(|emit_error| emit_error.to_string())?;
            Ok(RequestOutcome::Failed)
        }
    }
}

#[tauri::command]
pub fn answer_permission(
    state: State<'_, AppState>,
    id: String,
    allow: bool,
) -> Result<(), String> {
    match state.permissions.answer(&id, allow) {
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
