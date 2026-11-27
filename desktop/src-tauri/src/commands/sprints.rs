use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::db::{DbState, NewSprint, Sprint, SprintPatch};

use super::backlog::notify_backlog_changed;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListSprintsArgs {
    pub project_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateSprintArgs {
    pub project_id: i64,
    #[serde(flatten)]
    pub sprint: NewSprint,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateSprintArgs {
    pub project_id: i64,
    pub id: String,
    #[serde(flatten)]
    pub patch: SprintPatch,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloseSprintArgs {
    pub project_id: i64,
    pub id: String,
    #[serde(default)]
    pub move_pending_to_sprint_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteSprintArgs {
    pub project_id: i64,
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetItemSprintArgs {
    pub project_id: i64,
    pub item_id: String,
    #[serde(default)]
    pub sprint_id: Option<String>,
}

pub(crate) fn list_sprints_exec(
    app: &AppHandle,
    args: ListSprintsArgs,
) -> Result<Vec<Sprint>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_sprints(args.project_id)
        .map_err(|error| error.to_string())
}

pub(crate) fn create_sprint_exec(
    app: &AppHandle,
    args: CreateSprintArgs,
) -> Result<Sprint, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let sprint = db
        .create_sprint(args.project_id, args.sprint)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(sprint)
}

pub(crate) fn update_sprint_exec(
    app: &AppHandle,
    args: UpdateSprintArgs,
) -> Result<Sprint, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let sprint = db
        .update_sprint(args.project_id, &args.id, args.patch)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(sprint)
}

pub(crate) fn close_sprint_exec(
    app: &AppHandle,
    args: CloseSprintArgs,
) -> Result<Sprint, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let sprint = db
        .close_sprint(
            args.project_id,
            &args.id,
            args.move_pending_to_sprint_id.as_deref(),
        )
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(sprint)
}

pub(crate) fn delete_sprint_exec(app: &AppHandle, args: DeleteSprintArgs) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_sprint(args.project_id, &args.id)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn set_item_sprint_exec(
    app: &AppHandle,
    args: SetItemSprintArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_item_sprint(args.project_id, &args.item_id, args.sprint_id.as_deref())
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

#[tauri::command]
pub fn list_sprints(app: AppHandle, project_id: i64) -> Result<Vec<Sprint>, String> {
    list_sprints_exec(&app, ListSprintsArgs { project_id })
}

#[tauri::command]
pub fn create_sprint(
    app: AppHandle,
    project_id: i64,
    sprint: NewSprint,
) -> Result<Sprint, String> {
    create_sprint_exec(&app, CreateSprintArgs { project_id, sprint })
}

#[tauri::command]
pub fn update_sprint(
    app: AppHandle,
    project_id: i64,
    id: String,
    patch: SprintPatch,
) -> Result<Sprint, String> {
    update_sprint_exec(&app, UpdateSprintArgs { project_id, id, patch })
}

#[tauri::command]
pub fn close_sprint(
    app: AppHandle,
    project_id: i64,
    id: String,
    move_pending_to_sprint_id: Option<String>,
) -> Result<Sprint, String> {
    close_sprint_exec(
        &app,
        CloseSprintArgs {
            project_id,
            id,
            move_pending_to_sprint_id,
        },
    )
}

#[tauri::command]
pub fn delete_sprint(app: AppHandle, project_id: i64, id: String) -> Result<(), String> {
    delete_sprint_exec(&app, DeleteSprintArgs { project_id, id })
}

#[tauri::command]
pub fn set_item_sprint(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    sprint_id: Option<String>,
) -> Result<(), String> {
    set_item_sprint_exec(
        &app,
        SetItemSprintArgs {
            project_id,
            item_id,
            sprint_id,
        },
    )
}
