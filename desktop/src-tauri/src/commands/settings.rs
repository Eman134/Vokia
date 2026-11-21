use std::collections::HashMap;

use tauri::{AppHandle, Manager, State};

use crate::agents::AgentRegistry;
use crate::db::{DbProblem, DbState, SETTING_ACTIVE_LLM};
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
pub async fn set_setting(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    key: String,
    value: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.save_setting(&key, &value)
        .map_err(|error| error.to_string())?;
    if key == SETTING_ACTIVE_LLM {
        if let Ok(team) = registry.team() {
            let app_state = app.state::<AppState>();
            for profile in team.profiles() {
                if profile.follow_default_llm {
                    app_state.sessions.retire_agent(&profile.id).await;
                }
            }
        }
    }
    Ok(())
}
