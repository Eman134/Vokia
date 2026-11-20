use tauri::{AppHandle, State};

use crate::db::{Artifact, DbState};

use super::backlog::notify_backlog_changed;

#[tauri::command]
pub fn list_artifacts(state: State<'_, DbState>, project_id: i64) -> Result<Vec<Artifact>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_artifacts(project_id).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_artifact_criterion(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    artifact_id: String,
    criterion_id: String,
    done: bool,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_artifact_criterion(project_id, &artifact_id, &criterion_id, done)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}
