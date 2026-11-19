use tauri::{AppHandle, Emitter, Manager, State};

use std::collections::BTreeMap;

use serde::Serialize;

use crate::agents::{replica_of, Agent, AgentError, AgentRegistry};
use crate::db::{
    Db, DbError, DbState, Handoff, Layer, LayerDraft, OrchestrationSettings, OrchestrationSnapshot,
    Placement, PlacementDraft, Team, TeamDraft, TeamFlowDraft,
};
use crate::events::{Speaker, VokiaEvent, EVENT_NAME};
use crate::orchestration::seed;
use crate::orchestrator;
use crate::state::AppState;

const MOVEMENTS_LIMIT: i64 = 20;
const NOTIFY_FAILED_MESSAGE: &str = "A alteração foi gravada, mas a tela não foi avisada: ";

fn load_team_and_seed(db: &Db, registry: &AgentRegistry) -> Result<crate::agents::Team, String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    db.seed_orchestration(&seed::build(&team))
        .map_err(|error| error.to_string())?;
    Ok(team)
}

// Líder fora do elenco é recusado antes da escrita; líder None é válido e limpa o líder do time.
fn leader_in_cast(
    registry: &AgentRegistry,
    leader: Option<String>,
) -> Result<Option<String>, String> {
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

// Fluxo recusado volta como a lista de problemas em JSON (código, nó e mensagem) para a tela marcar cada um.
fn write_error_text(error: &DbError) -> String {
    match error {
        DbError::FlowInvalid(problems) => serde_json::to_string(problems)
            .expect("os problemas do fluxo sempre serializam para JSON"),
        _ => error.to_string(),
    }
}

// Antes de cada escrita: semente se preciso, poda das posições de agentes fora do elenco, escrita e aviso.
fn write<T>(
    app: &AppHandle,
    state: &DbState,
    registry: &AgentRegistry,
    operation: impl FnOnce(&Db) -> Result<T, DbError>,
) -> Result<T, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let team = load_team_and_seed(db, registry)?;
    let agent_ids: Vec<String> = team.ids().map(|id| id.as_str().to_string()).collect();
    db.delete_placements_outside(&agent_ids)
        .map_err(|error| error.to_string())?;
    let value = operation(db).map_err(|error| write_error_text(&error))?;
    close_all_sessions(app);
    app.emit(EVENT_NAME, VokiaEvent::OrchestrationChanged)
        .map_err(|error| format!("{NOTIFY_FAILED_MESSAGE}{error}."))?;
    Ok(value)
}

// Sessões em uso terminam a troca atual e fecham no release; as ociosas fecham agora. A próxima mensagem dá --resume com o prompt novo.
fn close_all_sessions(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        handle.state::<AppState>().sessions.close_everything().await;
    });
}

pub(crate) fn read_orchestration(
    db: &Db,
    registry: &AgentRegistry,
) -> Result<OrchestrationSnapshot, String> {
    let team = load_team_and_seed(db, registry)?;
    let mut snapshot = db.orchestration().map_err(|error| error.to_string())?;
    snapshot
        .placements
        .retain(|placement| team.ids().any(|id| id.as_str() == placement.agent_id));
    Ok(snapshot)
}

#[tauri::command]
pub fn get_orchestration(
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
) -> Result<OrchestrationSnapshot, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    read_orchestration(db, &registry)
}

#[tauri::command]
pub fn save_layer(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    draft: LayerDraft,
) -> Result<Layer, String> {
    write(&app, &state, &registry, |db| db.save_layer(&draft))
}

#[tauri::command]
pub fn delete_layer(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    id: String,
) -> Result<(), String> {
    write(&app, &state, &registry, |db| db.delete_layer(&id))
}

#[tauri::command]
pub fn reorder_layers(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    ids: Vec<String>,
) -> Result<(), String> {
    write(&app, &state, &registry, |db| db.reorder_layers(&ids))
}

#[tauri::command]
pub fn save_team(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    draft: TeamDraft,
    leader_agent: Option<String>,
) -> Result<Team, String> {
    let leader = leader_in_cast(&registry, leader_agent)?;
    write(&app, &state, &registry, |db| {
        let team = db.save_team(&draft)?;
        match leader.as_deref() {
            Some(leader) => db.set_team_leader(&team.id, leader),
            // Time novo nasce sem líder; no existente, nulo explícito (diálogo "Automático") limpa o líder.
            None if draft.id.is_some() => db.clear_team_leader(&team.id),
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
    id: String,
) -> Result<(), String> {
    let cast = registry.team().map_err(|error| error.to_string())?;
    let successor = Agent::ember();
    let successor_name = cast.name_of(&successor).to_string();
    let time = orchestrator::now_millis().map_err(|error| error.to_string())?;
    let notes = write(&app, &state, &registry, |db| {
        db.delete_team(
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
    draft: PlacementDraft,
) -> Result<Placement, String> {
    write(&app, &state, &registry, |db| db.place_agent(&draft))
}

#[tauri::command]
pub fn unplace_agent(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    agent_id: String,
) -> Result<(), String> {
    write(&app, &state, &registry, |db| db.unplace_agent(&agent_id))
}

// A estrutura do fluxo (agentes, gateways e ligações) só muda por aqui, validada de uma vez.
#[tauri::command]
pub fn save_team_flow(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    team_id: String,
    draft: TeamFlowDraft,
) -> Result<OrchestrationSnapshot, String> {
    let cast = registry.team().map_err(|error| error.to_string())?;
    let name_of = |id: &str| {
        Agent::from_stored(id)
            .map_or_else(|| id.to_string(), |agent| cast.name_of(&agent).to_string())
    };
    write(&app, &state, &registry, |db| {
        db.save_team_flow(&team_id, &draft, &name_of)
    })
}

#[tauri::command]
pub fn set_final_validation(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    team_id: String,
    mode: String,
) -> Result<(), String> {
    write(&app, &state, &registry, |db| {
        db.set_final_validation(&team_id, &mode)
    })
}

#[tauri::command]
pub fn save_orchestration_settings(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    settings: OrchestrationSettings,
) -> Result<(), String> {
    write(&app, &state, &registry, |db| db.save_settings(&settings))
}

pub(crate) fn read_movements(db: &Db, project_id: i64) -> Result<Vec<Handoff>, String> {
    db.list_movements(project_id, MOVEMENTS_LIMIT)
        .map_err(|error| error.to_string())
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
    let instances = orchestrator::active_instances(&app_state, project_id)
        .map_err(|error| error.to_string())?;
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    instance_id: String,
    copy_number: u32,
    project_id: i64,
    project_name: String,
    item_id: Option<String>,
    item_title: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentActivity {
    agent: String,
    runs: Vec<AgentRun>,
}

// Execuções em andamento de todos os projetos, por agente base: o próprio agente (cópia 0) e as réplicas (cópia n).
// Ember orquestra e não é posicionado, então não entra.
#[tauri::command]
pub fn list_agent_activity(
    app_state: State<'_, AppState>,
    state: State<'_, DbState>,
) -> Result<Vec<AgentActivity>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let instances =
        orchestrator::all_active_instances(&app_state).map_err(|error| error.to_string())?;
    let projects: BTreeMap<i64, String> = db
        .list_projects()
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|project| (project.id, project.name))
        .collect();
    let mut groups: BTreeMap<String, Vec<AgentRun>> = BTreeMap::new();
    for instance in instances {
        if instance.agent == Agent::ember() {
            continue;
        }
        let (base, copy_number) = replica_of(instance.agent.as_str())
            .map_or((instance.agent.as_str(), 0), |(base, number)| {
                (base, number)
            });
        let (item_id, item_title) = match instance.item_id {
            Some(id) => match db.read_card(instance.project_id, &id) {
                Ok(item) => (Some(item.id), Some(item.title)),
                Err(DbError::BacklogItemNotFound(_)) => (Some(id), None),
                Err(error) => return Err(error.to_string()),
            },
            None => (None, None),
        };
        groups.entry(base.to_string()).or_default().push(AgentRun {
            instance_id: instance.agent.to_string(),
            copy_number,
            project_id: instance.project_id,
            project_name: projects
                .get(&instance.project_id)
                .cloned()
                .unwrap_or_else(|| format!("#{}", instance.project_id)),
            item_id,
            item_title,
        });
    }
    Ok(groups
        .into_iter()
        .map(|(agent, mut runs)| {
            runs.sort_by_key(|run| (run.project_id, run.copy_number));
            AgentActivity { agent, runs }
        })
        .collect())
}

#[cfg(test)]
#[path = "../../tests/orchestration_commands_tests.rs"]
mod tests;
