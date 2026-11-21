use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::agents::AgentRegistry;
use crate::roles::{Role, RoleDraft, RoleEdit, RoleRegistry};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoleCreateArgs {
    pub draft: RoleDraft,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoleUpdateArgs {
    pub id: String,
    pub edit: RoleEdit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RoleDeleteArgs {
    pub id: String,
}

pub(crate) fn role_list_exec(app: &AppHandle) -> Result<Vec<Role>, String> {
    app.state::<RoleRegistry>()
        .list()
        .map_err(|error| error.to_string())
}

pub(crate) fn role_create_exec(app: &AppHandle, args: RoleCreateArgs) -> Result<Role, String> {
    app.state::<RoleRegistry>()
        .create(&args.draft)
        .map_err(|error| error.to_string())
}

pub(crate) fn role_update_exec(app: &AppHandle, args: RoleUpdateArgs) -> Result<Role, String> {
    app.state::<RoleRegistry>()
        .update(&args.id, &args.edit)
        .map_err(|error| error.to_string())
}

pub(crate) fn role_delete_exec(app: &AppHandle, args: RoleDeleteArgs) -> Result<(), String> {
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    app.state::<RoleRegistry>()
        .delete(&args.id, &team)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn role_list(app: AppHandle) -> Result<Vec<Role>, String> {
    role_list_exec(&app)
}

#[tauri::command]
pub fn role_create(app: AppHandle, draft: RoleDraft) -> Result<Role, String> {
    role_create_exec(&app, RoleCreateArgs { draft })
}

#[tauri::command]
pub fn role_update(app: AppHandle, id: String, edit: RoleEdit) -> Result<Role, String> {
    role_update_exec(&app, RoleUpdateArgs { id, edit })
}

#[tauri::command]
pub fn role_delete(app: AppHandle, id: String) -> Result<(), String> {
    role_delete_exec(&app, RoleDeleteArgs { id })
}
