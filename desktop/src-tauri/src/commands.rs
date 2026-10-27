use tauri::{AppHandle, State};

use crate::claude::CliProblem;
use crate::connectivity;
use crate::events::Agent;
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
