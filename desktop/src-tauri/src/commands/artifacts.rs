use std::fs;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::db::{
    Artifact, ArtifactDetail, ArtifactDocument, ArtifactDocumentPatch, ArtifactKind, ArtifactSummary,
    BacklogItem, DbError, DbState, NewArtifactDocument, ARTIFACT_KINDS,
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

const PLAN_FILE_FILTER_NAME: &str = "Markdown";
const PLAN_SAVE_EXTENSIONS: [&str; 1] = ["md"];
const PLAN_PICK_EXTENSIONS: [&str; 2] = ["md", "txt"];
pub(crate) const PLAN_IMPORT_MAX_BYTES: u64 = 200_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateItemPlanArgs {
    pub project_id: i64,
    pub item_id: String,
    pub proposal: String,
    pub expected_updated_at: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ImportPlanArgs {
    pub project_id: i64,
    pub title: String,
    pub proposal: String,
    pub author: Option<String>,
}

#[derive(Serialize)]
pub struct PlanFile {
    pub name: String,
    pub text: String,
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

pub(crate) fn update_item_plan_exec(
    app: &AppHandle,
    args: UpdateItemPlanArgs,
) -> Result<Artifact, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let artifact = db
        .update_item_plan(args.project_id, &args.item_id, &args.proposal, args.expected_updated_at)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(artifact)
}

pub(crate) fn import_plan_exec(app: &AppHandle, args: ImportPlanArgs) -> Result<BacklogItem, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let item = db
        .import_plan(args.project_id, &args.title, &args.proposal, args.author.as_deref())
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(item)
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
pub fn update_item_plan(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    proposal: String,
    expected_updated_at: Option<i64>,
) -> Result<Artifact, String> {
    update_item_plan_exec(
        &app,
        UpdateItemPlanArgs {
            project_id,
            item_id,
            proposal,
            expected_updated_at,
        },
    )
}

#[tauri::command]
pub fn import_plan(
    app: AppHandle,
    project_id: i64,
    title: String,
    proposal: String,
    author: Option<String>,
) -> Result<BacklogItem, String> {
    import_plan_exec(
        &app,
        ImportPlanArgs {
            project_id,
            title,
            proposal,
            author,
        },
    )
}

// None quando o usuário cancela o diálogo.
#[tauri::command]
pub async fn save_plan_markdown(
    app: AppHandle,
    suggested_name: String,
    content: String,
) -> Result<Option<String>, String> {
    let Some(chosen) = app
        .dialog()
        .file()
        .add_filter(PLAN_FILE_FILTER_NAME, &PLAN_SAVE_EXTENSIONS)
        .set_file_name(&suggested_name)
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(|error| error.to_string())?;
    fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
pub async fn pick_plan_file(app: AppHandle) -> Result<Option<PlanFile>, String> {
    let Some(chosen) = app
        .dialog()
        .file()
        .add_filter(PLAN_FILE_FILTER_NAME, &PLAN_PICK_EXTENSIONS)
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(|error| error.to_string())?;
    let size = fs::metadata(&path).map_err(|error| error.to_string())?.len();
    if size > PLAN_IMPORT_MAX_BYTES {
        return Err(DbError::PlanFileTooLarge {
            max: PLAN_IMPORT_MAX_BYTES as usize,
        }
        .to_string());
    }
    let bytes = fs::read(&path).map_err(|error| error.to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| DbError::PlanFileNotText.to_string())?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(Some(PlanFile { name, text }))
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
