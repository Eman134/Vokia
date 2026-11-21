use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::db::{Artifact, DbState};

use super::backlog::notify_backlog_changed;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListArtifactsArgs {
    pub project_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetArtifactCriterionArgs {
    pub project_id: i64,
    pub artifact_id: String,
    pub criterion_id: String,
    pub done: bool,
}

pub(crate) fn list_artifacts_exec(
    app: &AppHandle,
    args: ListArtifactsArgs,
) -> Result<Vec<Artifact>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_artifacts(args.project_id)
        .map_err(|error| error.to_string())
}

pub(crate) fn set_artifact_criterion_exec(
    app: &AppHandle,
    args: SetArtifactCriterionArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_artifact_criterion(
        args.project_id,
        &args.artifact_id,
        &args.criterion_id,
        args.done,
    )
    .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

#[tauri::command]
pub fn list_artifacts(app: AppHandle, project_id: i64) -> Result<Vec<Artifact>, String> {
    list_artifacts_exec(&app, ListArtifactsArgs { project_id })
}

#[tauri::command]
pub fn set_artifact_criterion(
    app: AppHandle,
    project_id: i64,
    artifact_id: String,
    criterion_id: String,
    done: bool,
) -> Result<(), String> {
    set_artifact_criterion_exec(
        &app,
        SetArtifactCriterionArgs {
            project_id,
            artifact_id,
            criterion_id,
            done,
        },
    )
}
