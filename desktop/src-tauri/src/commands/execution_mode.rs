use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::permissions::ExecutionMode;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetExecutionModeArgs {
    pub project_id: i64,
    pub mode: ExecutionMode,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExecutionModeProjectArgs {
    pub project_id: i64,
}

// Só a interface muda o modo: agentes nunca afrouxam as próprias permissões.
pub(crate) fn set_execution_mode_exec(app: &AppHandle, args: SetExecutionModeArgs) -> Result<(), String> {
    app.state::<AppState>()
        .permissions
        .set_mode(app, args.project_id, args.mode)
        .map_err(|error| error.to_string())
}

pub(crate) fn get_execution_mode_exec(
    app: &AppHandle,
    args: ExecutionModeProjectArgs,
) -> Result<ExecutionMode, String> {
    Ok(app.state::<AppState>().permissions.mode(args.project_id))
}

#[tauri::command]
pub fn set_execution_mode(app: AppHandle, project_id: i64, mode: ExecutionMode) -> Result<(), String> {
    set_execution_mode_exec(&app, SetExecutionModeArgs { project_id, mode })
}

#[tauri::command]
pub fn get_execution_mode(app: AppHandle, project_id: i64) -> Result<ExecutionMode, String> {
    get_execution_mode_exec(&app, ExecutionModeProjectArgs { project_id })
}
