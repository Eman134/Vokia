use tauri::{AppHandle, Emitter, Manager, State};

use std::collections::BTreeMap;

use serde::Serialize;

use crate::agents::{replica_of, Agent, AgentError, AgentRegistry};
use crate::db::{
    Db, DbError, DbState, Handoff, Layer, LayerDraft, OrchestrationSettings, OrchestrationSnapshot,
    Placement, PlacementDraft, Rule, RuleDraft, Team, TeamDraft,
};
use crate::events::{Speaker, VokiaEvent, EVENT_NAME};
use crate::orchestrator;
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

// Líder fora do elenco é recusado antes da escrita; líder None é válido e limpa o líder do time.
fn leader_in_cast(registry: &AgentRegistry, leader: Option<String>) -> Result<Option<String>, String> {
    let Some(id) = leader else {
        return Ok(None);
    };
    let team = registry.team().map_err(|error| error.to_string())?;
    let in_cast = Agent::from_stored(&id).is_some_and(|agent| team.profile(&agent).is_some());
    if in_cast {
        Ok(Some(id))
    } else {
        Err(AgentError::UnknownAgent(id).to_string())
    }
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

pub(crate) fn read_orchestration(
    db: &Db,
    registry: &AgentRegistry,
    project_id: i64,
) -> Result<OrchestrationSnapshot, String> {
    let team = load_team_and_seed(db, registry, project_id)?;
    let mut snapshot = db
        .orchestration(project_id)
        .map_err(|error| error.to_string())?;
    snapshot
        .placements
        .retain(|placement| team.ids().any(|id| id.as_str() == placement.agent_id));
    Ok(snapshot)
}

#[tauri::command]
pub fn get_orchestration(
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
) -> Result<OrchestrationSnapshot, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    read_orchestration(db, &registry, project_id)
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
    leader_agent: Option<String>,
) -> Result<Team, String> {
    let leader = leader_in_cast(&registry, leader_agent)?;
    write(&app, &state, &registry, project_id, |db| {
        let team = db.save_team(project_id, &draft)?;
        match leader.as_deref() {
            Some(leader) => db.set_team_leader(project_id, &team.id, leader),
            None => Ok(team),
        }
    })
}

// Os agentes do time ficam sem posição; o Ember assume os cards deles, com aviso no feed.
#[tauri::command]
pub fn delete_team(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    id: String,
) -> Result<(), String> {
    let cast = registry.team().map_err(|error| error.to_string())?;
    let successor = Agent::ember();
    let successor_name = cast.name_of(&successor).to_string();
    let time = orchestrator::now_millis().map_err(|error| error.to_string())?;
    let notes = write(&app, &state, &registry, project_id, |db| {
        db.delete_team(
            project_id,
            &id,
            &successor,
            &successor_name,
            |agent| cast.name_of(agent).to_string(),
            time,
        )
    })?;
    for note in notes {
        // A troca já está gravada; falha ao avisar o feed não vira erro para quem removeu.
        let _ = orchestrator::emit_feed_note(
            &app,
            note.project_id,
            Speaker::Agent(successor.clone()),
            &note.text,
            time,
        );
        let _ = orchestrator::emit_backlog_changed(&app, note.project_id);
    }
    Ok(())
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
pub fn unplace_agent(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    project_id: i64,
    agent_id: String,
) -> Result<(), String> {
    write(&app, &state, &registry, project_id, |db| {
        db.unplace_agent(project_id, &agent_id)
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

pub(crate) fn read_movements(db: &Db, project_id: i64) -> Result<Vec<Handoff>, String> {
    db.list_movements(project_id, MOVEMENTS_LIMIT)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_movements(state: State<'_, DbState>, project_id: i64) -> Result<Vec<Handoff>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    read_movements(db, project_id)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCopy {
    id: String,
    number: u32,
    item_id: Option<String>,
    item_title: Option<String>,
    worktree_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCopies {
    agent: String,
    copies: Vec<AgentCopy>,
}

// Agrupa só as réplicas em execução (id <base>-<n>) sob o agente base; o grupo some quando não há réplica. O item vem do turno em andamento e o worktree do próprio item.
#[tauri::command]
pub fn list_agent_copies(
    app_state: State<'_, AppState>,
    state: State<'_, DbState>,
    project_id: i64,
) -> Result<Vec<AgentCopies>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let instances =
        orchestrator::active_instances(&app_state, project_id).map_err(|error| error.to_string())?;
    let mut groups: BTreeMap<String, Vec<AgentCopy>> = BTreeMap::new();
    for instance in instances {
        let Some((base, number)) = replica_of(instance.agent.as_str()) else {
            continue;
        };
        let (item_id, item_title, worktree_path) = match instance.item_id {
            Some(id) => match db.read_card(project_id, &id) {
                Ok(item) => (Some(item.id), Some(item.title), item.worktree_path),
                Err(DbError::BacklogItemNotFound(_)) => (Some(id), None, None),
                Err(error) => return Err(error.to_string()),
            },
            None => (None, None, None),
        };
        groups.entry(base.to_string()).or_default().push(AgentCopy {
            id: instance.agent.to_string(),
            number,
            item_id,
            item_title,
            worktree_path,
        });
    }
    Ok(groups
        .into_iter()
        .map(|(agent, mut copies)| {
            copies.sort_by_key(|copy| copy.number);
            AgentCopies { agent, copies }
        })
        .collect())
}

#[cfg(test)]
#[path = "../../tests/orchestration_commands_tests.rs"]
mod tests;
