use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::agents::Agent;
use crate::db::{is_stage_finished, is_unstarted, DbState, OrchestrationSnapshot, Placement, Team as OrchTeam, FLOW_START, START_EDGE_EVENT};
use crate::orchestration::snapshot::placement_of;
use crate::db::plan_stage_number;
use crate::orchestration::stages::TASK_DONE_EVENT;

use super::error::FlowError;
use super::flow::{load_orchestration, publish_stage, spawn_stage_request};

const ITEM_PLACEHOLDER: &str = "{item}";
const STAGE_PLACEHOLDER: &str = "{stage}";
const START_STAGE_TEXT: &str =
    "O plano {item} foi aprovado. Execute a etapa {stage} conforme a descrição dela no card.";
const NEXT_STAGE_TEXT: &str =
    "A etapa anterior do plano {item} foi concluída. Execute a etapa {stage} conforme a descrição dela no card.";
const RELEASE_LOCK_POISONED: &str = "O controle de liberação das etapas do plano falhou.";

// Evita que duas etapas concluídas ao mesmo tempo liberem a mesma etapa duas vezes.
static RELEASE_LOCK: Mutex<()> = Mutex::new(());

// Plano aprovado: as etapas sem dependência (a primeira e as com `apos:0`) começam juntas; o teto de max_parallel
// fica por conta da fila de pedidos do projeto.
#[allow(dead_code)]
pub(crate) fn start_approved_plan(app: &AppHandle, project_id: i64, item_id: &str) -> Result<(), FlowError> {
    let Some(snapshot) = load_orchestration(app)? else {
        return Ok(());
    };
    let state = app.state::<DbState>();
    let db = state.require().map_err(FlowError::Orchestration)?;
    let card = db.read_card(project_id, item_id).map_err(FlowError::Orchestration)?;
    let Some(team) = team_led_by(&snapshot, &card.who) else {
        return Ok(());
    };
    release_ready_stages(app, project_id, item_id, &snapshot, &team.id, None)
}

// Etapa concluída: libera todas as etapas ainda não iniciadas cujas dependências já terminaram, cada uma pela aresta
// `tarefa_concluida` de quem concluiu. Quando não sobra etapa, o card pai acompanha pela transição no banco.
pub(crate) fn advance_plan(app: &AppHandle, project_id: i64, stage_id: &str) -> Result<(), FlowError> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(FlowError::Orchestration)?;
    let stage = db.read_card(project_id, stage_id).map_err(FlowError::Orchestration)?;
    let Some(parent) = stage.parent.as_deref() else {
        return Ok(());
    };
    let Some(snapshot) = load_orchestration(app)? else {
        return Ok(());
    };
    let from = stage_agent(&stage.who)?;
    let from_placement = placement_of(&snapshot, &from).ok_or_else(|| {
        FlowError::PlanStage(format!("{} não está em nenhum time do fluxo.", stage.who))
    })?;
    release_ready_stages(
        app,
        project_id,
        parent,
        &snapshot,
        &from_placement.team_id,
        Some(&stage.who),
    )
}

// `completed_by` é quem acabou de concluir; sem ele (início do plano) a aresta é a de início do fluxo.
fn release_ready_stages(
    app: &AppHandle,
    project_id: i64,
    item_id: &str,
    snapshot: &OrchestrationSnapshot,
    team_id: &str,
    completed_by: Option<&str>,
) -> Result<(), FlowError> {
    let _guard = RELEASE_LOCK.lock().map_err(|_| FlowError::PlanStage(RELEASE_LOCK_POISONED.to_string()))?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(FlowError::Orchestration)?;
    let stages = db.plan_stages(project_id, item_id).map_err(FlowError::Orchestration)?;
    let dependencies = db.plan_dependencies(project_id, item_id).map_err(FlowError::Orchestration)?;
    let finished: Vec<u32> = stages
        .iter()
        .filter(|stage| stage.column.as_deref().is_some_and(is_stage_finished))
        .filter_map(|stage| plan_stage_number(item_id, &stage.id))
        .collect();
    for stage in &stages {
        if !stage.column.as_deref().is_none_or(is_unstarted) {
            continue;
        }
        let Some(number) = plan_stage_number(item_id, &stage.id) else {
            continue;
        };
        let ready = dependencies
            .iter()
            .find(|(n, _)| *n == number)
            .is_some_and(|(_, after)| after.iter().all(|dep| finished.contains(dep)));
        if !ready {
            continue;
        }
        let owner = stage_agent(&stage.who)?;
        let edge_id = match completed_by {
            Some(from) => handoff_edge(snapshot, team_id, from, &stage.who),
            None => start_edge(snapshot, team_id, &stage.who),
        };
        let placement = member_placement(snapshot, team_id, &stage.who)?;
        db.start_plan_stage(project_id, &stage.id, &stage.who, &placement.layer_id, edge_id)
            .map_err(FlowError::Orchestration)?;
        publish_stage(app, project_id, &stage.id)?;
        let template = if completed_by.is_some() { NEXT_STAGE_TEXT } else { START_STAGE_TEXT };
        let text = stage_text(template, item_id, &stage.id);
        spawn_stage_request(app, project_id, &stage.id, owner, &text);
    }
    Ok(())
}

// A aresta de início só entra quando aponta para quem recebe a etapa; as demais etapas sem dependência começam sem aresta.
fn start_edge(snapshot: &OrchestrationSnapshot, team_id: &str, agent: &str) -> Option<i64> {
    snapshot
        .edges
        .iter()
        .find(|edge| {
            edge.team_id == team_id
                && edge.from_node == FLOW_START
                && edge.when_event == START_EDGE_EVENT
                && edge.to_node == agent
        })
        .map(|edge| edge.id)
}

#[allow(dead_code)]
fn team_led_by<'a>(snapshot: &'a OrchestrationSnapshot, leader: &str) -> Option<&'a OrchTeam> {
    snapshot
        .teams
        .iter()
        .find(|team| team.leader_agent.as_deref() == Some(leader))
}

// Sem aresta entre os agentes (mesmo agente em etapas seguidas ou passagem fora do desenho do fluxo), a etapa segue sem aresta.
fn handoff_edge(
    snapshot: &OrchestrationSnapshot,
    team_id: &str,
    from: &str,
    to: &str,
) -> Option<i64> {
    if from == to {
        return None;
    }
    snapshot
        .edges
        .iter()
        .find(|edge| {
            edge.team_id == team_id
                && edge.from_node == from
                && edge.to_node == to
                && edge.when_event == TASK_DONE_EVENT
        })
        .map(|edge| edge.id)
}

fn member_placement<'a>(
    snapshot: &'a OrchestrationSnapshot,
    team_id: &str,
    agent: &str,
) -> Result<&'a Placement, FlowError> {
    snapshot
        .placements
        .iter()
        .find(|placement| placement.team_id == team_id && placement.agent_id == agent)
        .ok_or_else(|| FlowError::PlanStage(format!("{agent} não está no time do plano.")))
}

fn stage_agent(who: &str) -> Result<Agent, FlowError> {
    Agent::from_stored(who).ok_or_else(|| FlowError::PlanStage(format!("{who} não é um agente válido.")))
}

fn stage_text(template: &str, item_id: &str, stage_id: &str) -> String {
    template
        .replace(ITEM_PLACEHOLDER, item_id)
        .replace(STAGE_PLACEHOLDER, stage_id)
}

#[cfg(test)]
#[path = "../../tests/plan_stages_tests.rs"]
mod tests;
