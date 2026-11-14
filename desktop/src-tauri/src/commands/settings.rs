use std::collections::HashMap;

use tauri::{AppHandle, Manager, State};

use crate::db::{DbProblem, DbState};
use crate::orchestrator;
use crate::state::AppState;

use super::projects::stop_project_work;

const ERASE_BUSY_MESSAGE: &str = "Há um pedido em andamento; aguarde terminar.";

#[tauri::command]
pub fn db_problem(state: State<'_, DbState>) -> Option<DbProblem> {
    state.problem()
}

#[tauri::command]
pub async fn erase_all_data(app: AppHandle, state: State<'_, DbState>) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let app_state = app.state::<AppState>();
    if orchestrator::has_flow(&app_state, None).map_err(|error| error.to_string())? {
        return Err(ERASE_BUSY_MESSAGE.to_string());
    }
    let existing = db.list_projects().map_err(|error| error.to_string())?;
    for project in &existing {
        stop_project_work(&app_state, project.id).await;
    }
    db.erase_all().map_err(|error| error.to_string())?;
    for project in &existing {
        orchestrator::forget_project(&app_state, project.id);
    }
    orchestrator::remove_archive(&app, &app_state).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, DbState>) -> Result<HashMap<String, String>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.load_settings().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_setting(state: State<'_, DbState>, key: String, value: String) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.save_setting(&key, &value)
        .map_err(|error| error.to_string())
}
