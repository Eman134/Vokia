use tauri::{AppHandle, State};

use crate::agents::{Agent, AgentDraft, AgentProfile, AgentRegistry, TeamListing};
use crate::db::DbState;
use crate::events::Speaker;
use crate::orchestrator::{emit_backlog_changed, emit_feed_note, now_millis};
use crate::roles::RoleRegistry;
use crate::state::AppState;

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

// O Ember assume os cards do agente excluído; o aviso de cada troca entra no feed do projeto.
#[tauri::command]
pub fn delete_agent(
    app: AppHandle,
    registry: State<'_, AgentRegistry>,
    state: State<'_, DbState>,
    id: Agent,
) -> Result<(), String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    let agent_name = team.name_of(&id).to_string();
    let successor = Agent::ember();
    let successor_name = team.name_of(&successor).to_string();
    let db = state.require().map_err(|error| error.to_string())?;
    let time = now_millis().map_err(|error| error.to_string())?;
    let notes = db
        .remove_agent(&id, &agent_name, &successor, &successor_name, time, || {
            registry.remove(&id)
        })
        .map_err(|error| error.to_string())?;
    for note in notes {
        // A troca já está gravada; falha ao avisar o feed não vira erro para quem excluiu.
        let _ = emit_feed_note(
            &app,
            note.project_id,
            Speaker::Agent(successor.clone()),
            &note.text,
            time,
        );
        let _ = emit_backlog_changed(&app, note.project_id);
    }
    Ok(())
}

#[tauri::command]
pub async fn set_agent_instructions(
    registry: State<'_, AgentRegistry>,
    app_state: State<'_, AppState>,
    id: Agent,
    text: String,
) -> Result<AgentProfile, String> {
    let profile = registry
        .set_instructions(&id, &text)
        .map_err(|error| error.to_string())?;
    // Só depois de gravada: a sessão que reabrir já lê o prompt com o texto novo.
    app_state.sessions.retire_agent(&id).await;
    Ok(profile)
}
