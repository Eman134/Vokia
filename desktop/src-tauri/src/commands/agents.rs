use tauri::State;

use crate::agents::{Agent, AgentDraft, AgentProfile, AgentRegistry, TeamListing};
use crate::roles::RoleRegistry;

#[tauri::command]
pub fn list_agents(registry: State<'_, AgentRegistry>) -> Result<TeamListing, String> {
    registry.listing().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_agent(
    registry: State<'_, AgentRegistry>,
    roles: State<'_, RoleRegistry>,
    draft: AgentDraft,
) -> Result<AgentProfile, String> {
    roles
        .ensure_exists(&draft.role)
        .map_err(|error| error.to_string())?;
    registry.create(&draft).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_agent(
    registry: State<'_, AgentRegistry>,
    roles: State<'_, RoleRegistry>,
    id: Agent,
    draft: AgentDraft,
) -> Result<AgentProfile, String> {
    roles
        .ensure_exists(&draft.role)
        .map_err(|error| error.to_string())?;
    registry
        .update(&id, &draft)
        .map_err(|error| error.to_string())
}
