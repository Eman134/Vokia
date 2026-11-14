use tauri::State;

use crate::agents::{Agent, AgentDraft, AgentProfile, AgentRegistry, TeamListing};

#[tauri::command]
pub fn list_agents(registry: State<'_, AgentRegistry>) -> Result<TeamListing, String> {
    registry.listing().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_agent(
    registry: State<'_, AgentRegistry>,
    draft: AgentDraft,
) -> Result<AgentProfile, String> {
    registry.create(&draft).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_agent(
    registry: State<'_, AgentRegistry>,
    id: Agent,
    draft: AgentDraft,
) -> Result<AgentProfile, String> {
    registry
        .update(&id, &draft)
        .map_err(|error| error.to_string())
}
