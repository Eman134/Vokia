use tauri::{AppHandle, Emitter, Manager, State};

use crate::agents::AgentRegistry;
use crate::db::{
    Db, DbError, DbState, Handoff, Layer, LayerDraft, OrchestrationSettings, OrchestrationSnapshot,
    Placement, PlacementDraft, Rule, RuleDraft, Team, TeamDraft,
};
use crate::events::{VokiaEvent, EVENT_NAME};
use crate::orchestration::seed;
use crate::state::AppState;

const MOVEMENTS_LIMIT: i64 = 20;
const NOTIFY_FAILED_MESSAGE: &str = "A alteração foi gravada, mas a tela não foi avisada: ";

fn load_team_and_seed(
    db: &Db,
    registry: &AgentRegistry,
    project_id: i64,
) -> Result<crate::agents::Team, String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    db.seed_orchestration(project_id, &seed::build(&team))
        .map_err(|error| error.to_string())?;
    Ok(team)
}

// Antes de cada escrita: semente se preciso, poda das posições de agentes fora do elenco, escrita e aviso.
fn write<T>(
    app: &AppHandle,
    state: &DbState,
    registry: &AgentRegistry,
    project_id: i64,
    operation: impl FnOnce(&Db) -> Result<T, DbError>,
) -> Result<T, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let team = load_team_and_seed(db, registry, project_id)?;
    let agent_ids: Vec<String> = team.ids().map(|id| id.as_str().to_string()).collect();
    db.delete_placements_outside(project_id, &agent_ids)
        .map_err(|error| error.to_string())?;
    let value = operation(db).map_err(|error| error.to_string())?;
    close_project_sessions(app, project_id);
    app.emit(EVENT_NAME, VokiaEvent::OrchestrationChanged { project_id })
        .map_err(|error| format!("{NOTIFY_FAILED_MESSAGE}{error}."))?;
    Ok(value)
}

// Sessões em uso terminam a troca atual e fecham no release; as ociosas fecham agora. A próxima mensagem dá --resume com o prompt novo.
fn close_project_sessions(app: &AppHandle, project_id: i64) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        handle
            .state::<AppState>()
            .sessions
            .close_project(project_id)
            .await;
    });
}

#[tauri::command]
pub fn get_orchestration(
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
) -> Result<OrchestrationSnapshot, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let team = load_team_and_seed(db, &registry, project_id)?;
    let mut snapshot = db
        .orchestration(project_id)
        .map_err(|error| error.to_string())?;
    snapshot
        .placements
        .retain(|placement| team.ids().any(|id| id.as_str() == placement.agent_id));
    Ok(snapshot)
}

#[tauri::command]
pub fn save_layer(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    draft: LayerDraft,
) -> Result<Layer, String> {
    write(&app, &state, &registry, project_id, |db| {
        db.save_layer(project_id, &draft)
    })
}

#[tauri::command]
pub fn delete_layer(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    id: String,
) -> Result<(), String> {
    write(&app, &state, &registry, project_id, |db| {
        db.delete_layer(project_id, &id)
    })
}

#[tauri::command]
pub fn reorder_layers(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    ids: Vec<String>,
) -> Result<(), String> {
    write(&app, &state, &registry, project_id, |db| {
        db.reorder_layers(project_id, &ids)
    })
}

#[tauri::command]
pub fn save_team(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    draft: TeamDraft,
) -> Result<Team, String> {
    write(&app, &state, &registry, project_id, |db| {
        db.save_team(project_id, &draft)
    })
}

#[tauri::command]
pub fn delete_team(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    id: String,
) -> Result<(), String> {
    write(&app, &state, &registry, project_id, |db| {
        db.delete_team(project_id, &id)
    })
}

#[tauri::command]
pub fn place_agent(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    draft: PlacementDraft,
) -> Result<Placement, String> {
    write(&app, &state, &registry, project_id, |db| {
        db.place_agent(project_id, &draft)
    })
}

#[tauri::command]
pub fn save_rule(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    draft: RuleDraft,
) -> Result<Rule, String> {
    write(&app, &state, &registry, project_id, |db| {
        db.save_rule(project_id, &draft)
    })
}

#[tauri::command]
pub fn delete_rule(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    id: i64,
) -> Result<(), String> {
    write(&app, &state, &registry, project_id, |db| {
        db.delete_rule(project_id, id)
    })
}

#[tauri::command]
pub fn save_orchestration_settings(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    settings: OrchestrationSettings,
) -> Result<(), String> {
    write(&app, &state, &registry, project_id, |db| {
        db.save_settings(project_id, &settings)
    })
}

#[tauri::command]
pub fn list_movements(state: State<'_, DbState>, project_id: i64) -> Result<Vec<Handoff>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_movements(project_id, MOVEMENTS_LIMIT)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "../../tests/orchestration_commands_tests.rs"]
mod tests;
