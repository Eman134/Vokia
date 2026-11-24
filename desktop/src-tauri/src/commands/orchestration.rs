use tauri::{AppHandle, Emitter, Manager, State};

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveLayerArgs {
    pub draft: LayerDraft,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteLayerArgs {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReorderLayersArgs {
    pub ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveTeamArgs {
    pub draft: TeamDraft,
    pub leader_agent: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteTeamArgs {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaceAgentArgs {
    pub draft: PlacementDraft,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnplaceAgentArgs {
    pub agent_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveTeamFlowArgs {
    pub team_id: String,
    pub draft: TeamFlowDraft,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetFinalValidationArgs {
    pub team_id: String,
    pub mode: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetJudgeFullFlowArgs {
    pub team_id: String,
    pub enabled: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveOrchestrationSettingsArgs {
    pub settings: OrchestrationSettings,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListAgentCopiesArgs {
    pub project_id: i64,
}

fn get_orchestration_impl(
    state: &DbState,
    registry: &AgentRegistry,
) -> Result<OrchestrationSnapshot, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    read_orchestration(db, registry)
}

fn save_layer_impl(
    app: &AppHandle,
    state: &DbState,
    registry: &AgentRegistry,
    draft: &LayerDraft,
) -> Result<Layer, String> {
    write(app, state, registry, |db| db.save_layer(draft))
}

fn place_agent_impl(
    app: &AppHandle,
    state: &DbState,
    registry: &AgentRegistry,
    draft: &PlacementDraft,
) -> Result<Placement, String> {
    write(app, state, registry, |db| db.place_agent(draft))
}

fn save_orchestration_settings_impl(
    app: &AppHandle,
    state: &DbState,
    registry: &AgentRegistry,
    settings: &OrchestrationSettings,
) -> Result<(), String> {
    write(app, state, registry, |db| db.save_settings(settings))
}

pub(crate) fn get_orchestration_exec(app: &AppHandle) -> Result<OrchestrationSnapshot, String> {
    get_orchestration_impl(&app.state::<DbState>(), &app.state::<AgentRegistry>())
}

pub(crate) fn save_layer_exec(app: &AppHandle, args: SaveLayerArgs) -> Result<Layer, String> {
    save_layer_impl(
        app,
        &app.state::<DbState>(),
        &app.state::<AgentRegistry>(),
        &args.draft,
    )
}

pub(crate) fn delete_layer_exec(app: &AppHandle, args: DeleteLayerArgs) -> Result<(), String> {
    write(
        app,
        &app.state::<DbState>(),
        &app.state::<AgentRegistry>(),
        |db| db.delete_layer(&args.id),
    )
}

pub(crate) fn reorder_layers_exec(app: &AppHandle, args: ReorderLayersArgs) -> Result<(), String> {
    write(
        app,
        &app.state::<DbState>(),
        &app.state::<AgentRegistry>(),
        |db| db.reorder_layers(&args.ids),
    )
}

pub(crate) fn save_team_exec(app: &AppHandle, args: SaveTeamArgs) -> Result<Team, String> {
    let registry = app.state::<AgentRegistry>();
    let leader = leader_in_cast(&registry, args.leader_agent)?;
    let draft = args.draft;
    write(app, &app.state::<DbState>(), &registry, |db| {
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
pub(crate) fn delete_team_exec(app: &AppHandle, args: DeleteTeamArgs) -> Result<(), String> {
    let registry = app.state::<AgentRegistry>();
    let cast = registry.team().map_err(|error| error.to_string())?;
    let successor = Agent::ember();
    let successor_name = cast.name_of(&successor).to_string();
    let time = orchestrator::now_millis().map_err(|error| error.to_string())?;
    let notes = write(app, &app.state::<DbState>(), &registry, |db| {
        db.delete_team(
            &args.id,
            &successor,
            &successor_name,
            |agent| cast.name_of(agent).to_string(),
            time,
        )
    })?;
    for note in notes {
        // A troca já está gravada; falha ao avisar o feed não vira erro para quem removeu.
        let _ = orchestrator::emit_feed_note(
            app,
            note.project_id,
            Speaker::Agent(successor.clone()),
            &note.text,
            time,
        );
        let _ = orchestrator::emit_backlog_changed(app, note.project_id);
    }
    Ok(())
}

pub(crate) fn place_agent_exec(app: &AppHandle, args: PlaceAgentArgs) -> Result<Placement, String> {
    place_agent_impl(
        app,
        &app.state::<DbState>(),
        &app.state::<AgentRegistry>(),
        &args.draft,
    )
}

pub(crate) fn unplace_agent_exec(app: &AppHandle, args: UnplaceAgentArgs) -> Result<(), String> {
    write(
        app,
        &app.state::<DbState>(),
        &app.state::<AgentRegistry>(),
        |db| db.unplace_agent(&args.agent_id),
    )
}

// A estrutura do fluxo (agentes, gateways e ligações) só muda por aqui, validada de uma vez.
pub(crate) fn save_team_flow_exec(
    app: &AppHandle,
    args: SaveTeamFlowArgs,
) -> Result<OrchestrationSnapshot, String> {
    let registry = app.state::<AgentRegistry>();
    let cast = registry.team().map_err(|error| error.to_string())?;
    let name_of = |id: &str| {
        Agent::from_stored(id)
            .map_or_else(|| id.to_string(), |agent| cast.name_of(&agent).to_string())
    };
    write(app, &app.state::<DbState>(), &registry, |db| {
        db.save_team_flow(&args.team_id, &args.draft, &name_of)
    })
}

pub(crate) fn set_final_validation_exec(
    app: &AppHandle,
    args: SetFinalValidationArgs,
) -> Result<(), String> {
    write(
        app,
        &app.state::<DbState>(),
        &app.state::<AgentRegistry>(),
        |db| db.set_final_validation(&args.team_id, &args.mode),
    )
}

pub(crate) fn set_judge_full_flow_exec(
    app: &AppHandle,
    args: SetJudgeFullFlowArgs,
) -> Result<(), String> {
    write(
        app,
        &app.state::<DbState>(),
        &app.state::<AgentRegistry>(),
        |db| db.set_judge_full_flow(&args.team_id, args.enabled),
    )
}

pub(crate) fn save_orchestration_settings_exec(
    app: &AppHandle,
    args: SaveOrchestrationSettingsArgs,
) -> Result<(), String> {
    save_orchestration_settings_impl(
        app,
        &app.state::<DbState>(),
        &app.state::<AgentRegistry>(),
        &args.settings,
    )
}

#[tauri::command]
pub fn get_orchestration(
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
) -> Result<OrchestrationSnapshot, String> {
    get_orchestration_impl(&state, &registry)
}

#[tauri::command]
pub fn save_layer(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    draft: LayerDraft,
) -> Result<Layer, String> {
    save_layer_impl(&app, &state, &registry, &draft)
}

#[tauri::command]
pub fn delete_layer(app: AppHandle, id: String) -> Result<(), String> {
    delete_layer_exec(&app, DeleteLayerArgs { id })
}

#[tauri::command]
pub fn reorder_layers(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    reorder_layers_exec(&app, ReorderLayersArgs { ids })
}

#[tauri::command]
pub fn save_team(
    app: AppHandle,
    draft: TeamDraft,
    leader_agent: Option<String>,
) -> Result<Team, String> {
    save_team_exec(
        &app,
        SaveTeamArgs {
            draft,
            leader_agent,
        },
    )
}

#[tauri::command]
pub fn delete_team(app: AppHandle, id: String) -> Result<(), String> {
    delete_team_exec(&app, DeleteTeamArgs { id })
}

#[tauri::command]
pub fn place_agent(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    draft: PlacementDraft,
) -> Result<Placement, String> {
    place_agent_impl(&app, &state, &registry, &draft)
}

#[tauri::command]
pub fn unplace_agent(app: AppHandle, agent_id: String) -> Result<(), String> {
    unplace_agent_exec(&app, UnplaceAgentArgs { agent_id })
}

#[tauri::command]
pub fn save_team_flow(
    app: AppHandle,
    team_id: String,
    draft: TeamFlowDraft,
) -> Result<OrchestrationSnapshot, String> {
    save_team_flow_exec(&app, SaveTeamFlowArgs { team_id, draft })
}

#[tauri::command]
pub fn set_final_validation(app: AppHandle, team_id: String, mode: String) -> Result<(), String> {
    set_final_validation_exec(&app, SetFinalValidationArgs { team_id, mode })
}

#[tauri::command]
pub fn set_judge_full_flow(app: AppHandle, team_id: String, enabled: bool) -> Result<(), String> {
    set_judge_full_flow_exec(&app, SetJudgeFullFlowArgs { team_id, enabled })
}

#[tauri::command]
pub fn save_orchestration_settings(
    app: AppHandle,
    state: State<'_, DbState>,
    registry: State<'_, AgentRegistry>,
    settings: OrchestrationSettings,
) -> Result<(), String> {
    save_orchestration_settings_impl(&app, &state, &registry, &settings)
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
pub(crate) fn list_agent_copies_exec(
    app: &AppHandle,
    args: ListAgentCopiesArgs,
) -> Result<Vec<AgentCopies>, String> {
    let project_id = args.project_id;
    let state = app.state::<DbState>();
    let app_state = app.state::<AppState>();
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
pub(crate) fn list_agent_activity_exec(app: &AppHandle) -> Result<Vec<AgentActivity>, String> {
    let state = app.state::<DbState>();
    let app_state = app.state::<AppState>();
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

#[tauri::command]
pub fn list_agent_copies(app: AppHandle, project_id: i64) -> Result<Vec<AgentCopies>, String> {
    list_agent_copies_exec(&app, ListAgentCopiesArgs { project_id })
}

#[tauri::command]
pub fn list_agent_activity(app: AppHandle) -> Result<Vec<AgentActivity>, String> {
    list_agent_activity_exec(&app)
}

#[cfg(test)]
#[path = "../../tests/orchestration_commands_tests.rs"]
mod tests;
