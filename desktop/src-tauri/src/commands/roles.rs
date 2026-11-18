use tauri::State;

use crate::agents::AgentRegistry;
use crate::roles::{Role, RoleDraft, RoleEdit, RoleRegistry};

#[tauri::command]
pub fn role_list(roles: State<'_, RoleRegistry>) -> Result<Vec<Role>, String> {
    roles.list().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn role_create(roles: State<'_, RoleRegistry>, draft: RoleDraft) -> Result<Role, String> {
    roles.create(&draft).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn role_update(
    roles: State<'_, RoleRegistry>,
    id: String,
    edit: RoleEdit,
) -> Result<Role, String> {
    roles.update(&id, &edit).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn role_delete(
    roles: State<'_, RoleRegistry>,
    agents: State<'_, AgentRegistry>,
    id: String,
) -> Result<(), String> {
    let team = agents.team().map_err(|error| error.to_string())?;
    roles.delete(&id, &team).map_err(|error| error.to_string())
}
