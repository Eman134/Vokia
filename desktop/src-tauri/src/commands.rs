use tauri::{AppHandle, State};

use crate::claude::CliProblem;
use crate::connectivity;
use crate::db::{DbProblem, DbState, Project};
use crate::events::{Agent, VokiaEvent};
use crate::orchestrator;
use crate::state::AppState;

#[tauri::command]
pub fn cli_problem(state: State<'_, AppState>) -> Option<CliProblem> {
    state.cli.as_ref().err().cloned()
}

#[tauri::command]
pub async fn check_connection() -> bool {
    connectivity::is_reachable().await
}

#[tauri::command]
pub async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    to: Agent,
) -> Result<(), String> {
    if let Err(error) = orchestrator::handle_request(&app, &state, &text, to).await {
        orchestrator::report_failure(&app, &error).map_err(|emit_error| emit_error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn db_problem(state: State<'_, DbState>) -> Option<DbProblem> {
    state.problem()
}

#[tauri::command]
pub fn list_projects(state: State<'_, DbState>) -> Result<Vec<Project>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_projects().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn select_project(state: State<'_, DbState>, id: i64) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.select_project(id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn add_project(
    state: State<'_, DbState>,
    name: String,
    path: String,
) -> Result<Project, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.add_project(&name, &path)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn load_history(state: State<'_, DbState>, project_id: i64) -> Result<Vec<VokiaEvent>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.load_history(project_id)
        .map_err(|error| error.to_string())
}
