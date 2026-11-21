use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::db::{
    ArtifactDetail, ArtifactDocument, ArtifactDocumentPatch, ArtifactKind, ArtifactSummary, DbState,
    NewArtifactDocument, ARTIFACT_KINDS,
};
use crate::orchestrator::emit_artifacts_changed;

use super::backlog::notify_backlog_changed;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListArtifactsArgs {
    pub project_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArtifactRefArgs {
    pub project_id: i64,
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateArtifactArgs {
    pub project_id: i64,
    #[serde(flatten)]
    pub document: NewArtifactDocument,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateArtifactArgs {
    pub project_id: i64,
    pub id: String,
    #[serde(flatten)]
    pub patch: ArtifactDocumentPatch,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetArtifactCheckArgs {
    pub project_id: i64,
    pub id: String,
    pub index: usize,
    pub done: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetArtifactCriterionArgs {
    pub project_id: i64,
    pub artifact_id: String,
    pub criterion_id: String,
    pub done: bool,
}

// A gravação já está no banco; falha ao avisar o front não vira erro para quem chamou.
pub(crate) fn notify_artifacts_changed(app: &AppHandle, project_id: i64) {
    let _ = emit_artifacts_changed(app, project_id);
}

pub(crate) fn list_artifact_kinds_exec() -> &'static [ArtifactKind] {
    ARTIFACT_KINDS
}

pub(crate) fn list_artifacts_exec(
    app: &AppHandle,
    args: ListArtifactsArgs,
) -> Result<Vec<ArtifactSummary>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_artifact_summaries(args.project_id)
        .map_err(|error| error.to_string())
}

pub(crate) fn get_artifact_exec(
    app: &AppHandle,
    args: ArtifactRefArgs,
) -> Result<ArtifactDetail, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.get_artifact(args.project_id, &args.id)
        .map_err(|error| error.to_string())
}

pub(crate) fn create_artifact_exec(
    app: &AppHandle,
    args: CreateArtifactArgs,
) -> Result<ArtifactDocument, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let document = db
        .create_artifact_document(args.project_id, args.document)
        .map_err(|error| error.to_string())?;
    notify_artifacts_changed(app, args.project_id);
    Ok(document)
}

pub(crate) fn update_artifact_exec(
    app: &AppHandle,
    args: UpdateArtifactArgs,
) -> Result<ArtifactDocument, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let document = db
        .update_artifact_document(args.project_id, &args.id, args.patch)
        .map_err(|error| error.to_string())?;
    notify_artifacts_changed(app, args.project_id);
    Ok(document)
}

pub(crate) fn delete_artifact_exec(app: &AppHandle, args: ArtifactRefArgs) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_artifact_document(args.project_id, &args.id)
        .map_err(|error| error.to_string())?;
    notify_artifacts_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn set_artifact_check_exec(
    app: &AppHandle,
    args: SetArtifactCheckArgs,
) -> Result<ArtifactDocument, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let document = db
        .set_artifact_check(args.project_id, &args.id, args.index, args.done)
        .map_err(|error| error.to_string())?;
    notify_artifacts_changed(app, args.project_id);
    Ok(document)
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
pub fn list_artifact_kinds() -> &'static [ArtifactKind] {
    list_artifact_kinds_exec()
}

#[tauri::command]
pub fn list_artifacts(app: AppHandle, project_id: i64) -> Result<Vec<ArtifactSummary>, String> {
    list_artifacts_exec(&app, ListArtifactsArgs { project_id })
}

#[tauri::command]
pub fn get_artifact(app: AppHandle, project_id: i64, id: String) -> Result<ArtifactDetail, String> {
    get_artifact_exec(&app, ArtifactRefArgs { project_id, id })
}

#[tauri::command]
pub fn create_artifact(
    app: AppHandle,
    project_id: i64,
    document: NewArtifactDocument,
) -> Result<ArtifactDocument, String> {
    create_artifact_exec(&app, CreateArtifactArgs { project_id, document })
}

#[tauri::command]
pub fn update_artifact(
    app: AppHandle,
    project_id: i64,
    id: String,
    patch: ArtifactDocumentPatch,
) -> Result<ArtifactDocument, String> {
    update_artifact_exec(&app, UpdateArtifactArgs { project_id, id, patch })
}

#[tauri::command]
pub fn delete_artifact(app: AppHandle, project_id: i64, id: String) -> Result<(), String> {
    delete_artifact_exec(&app, ArtifactRefArgs { project_id, id })
}

#[tauri::command]
pub fn set_artifact_check(
    app: AppHandle,
    project_id: i64,
    id: String,
    index: usize,
    done: bool,
) -> Result<ArtifactDocument, String> {
    set_artifact_check_exec(
        &app,
        SetArtifactCheckArgs {
            project_id,
            id,
            index,
            done,
        },
    )
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
