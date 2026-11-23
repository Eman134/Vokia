use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetBypassArgs {
    pub project_id: i64,
    pub enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BypassProjectArgs {
    pub project_id: i64,
}

// Só a interface liga o modo bypass: agentes nunca se aprovam sozinhos.
pub(crate) fn set_bypass_mode_exec(app: &AppHandle, args: SetBypassArgs) -> Result<(), String> {
    app.state::<AppState>()
        .permissions
        .set_bypass(app, args.project_id, args.enabled)
        .map_err(|error| error.to_string())
}

pub(crate) fn get_bypass_mode_exec(app: &AppHandle, args: BypassProjectArgs) -> Result<bool, String> {
    Ok(app
        .state::<AppState>()
        .permissions
        .bypass_enabled(args.project_id))
}

#[tauri::command]
pub fn set_bypass_mode(app: AppHandle, project_id: i64, enabled: bool) -> Result<(), String> {
    set_bypass_mode_exec(&app, SetBypassArgs { project_id, enabled })
}

#[tauri::command]
pub fn get_bypass_mode(app: AppHandle, project_id: i64) -> Result<bool, String> {
    get_bypass_mode_exec(&app, BypassProjectArgs { project_id })
}
