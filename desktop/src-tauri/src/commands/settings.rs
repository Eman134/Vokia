use std::collections::HashMap;

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::agents::AgentRegistry;
use crate::db::{DbProblem, DbState, SETTING_ACTIVE_LLM};
use crate::orchestrator;
use crate::state::AppState;

use super::projects::stop_project_work;

const ERASE_CONFIRM_MESSAGE: &str = "Confirmação obrigatória: informe { \"confirm\": true }.";
const ERASE_BUSY_MESSAGE: &str = "Há um pedido em andamento; aguarde terminar.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetSettingArgs {
    pub key: String,
    pub value: String,
}

pub(crate) fn db_problem_exec(app: &AppHandle) -> Option<DbProblem> {
    app.state::<DbState>().problem()
}

pub(crate) async fn erase_all_data_exec(app: &AppHandle, confirm: bool) -> Result<(), String> {
    if !confirm {
        return Err(ERASE_CONFIRM_MESSAGE.to_string());
    }
    let state = app.state::<DbState>();
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
    orchestrator::remove_archive(app, &app_state).map_err(|error| error.to_string())
}

pub(crate) fn get_settings_exec(app: &AppHandle) -> Result<HashMap<String, String>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.load_settings().map_err(|error| error.to_string())
}

pub(crate) async fn set_setting_exec(app: &AppHandle, args: SetSettingArgs) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.save_setting(&args.key, &args.value)
        .map_err(|error| error.to_string())?;
    if args.key == SETTING_ACTIVE_LLM {
        if let Ok(team) = app.state::<AgentRegistry>().team() {
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

#[tauri::command]
pub fn db_problem(app: AppHandle) -> Option<DbProblem> {
    db_problem_exec(&app)
}

#[tauri::command]
pub async fn erase_all_data(app: AppHandle) -> Result<(), String> {
    erase_all_data_exec(&app, true).await
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<HashMap<String, String>, String> {
    get_settings_exec(&app)
}

#[tauri::command]
pub async fn set_setting(app: AppHandle, key: String, value: String) -> Result<(), String> {
    set_setting_exec(&app, SetSettingArgs { key, value }).await
}
