use tauri::{AppHandle, Manager};

use crate::agents::Agent;
use crate::db::{DbState, OrchestrationSnapshot, Placement, Team as OrchTeam, FLOW_START, START_EDGE_EVENT};
use crate::orchestration::snapshot::placement_of;
use crate::orchestration::stages::TASK_DONE_EVENT;

use super::error::FlowError;
use super::flow::{load_orchestration, publish_stage, spawn_stage_request};

const ITEM_PLACEHOLDER: &str = "{item}";
const STAGE_PLACEHOLDER: &str = "{stage}";
const START_STAGE_TEXT: &str =
    "O plano {item} foi aprovado. Execute a etapa {stage} conforme a descrição dela no card.";
const NEXT_STAGE_TEXT: &str =
    "A etapa anterior do plano {item} foi concluída. Execute a etapa {stage} conforme a descrição dela no card.";
const FIRST_STAGE_NUMBER: u32 = 1;

// Plano aprovado: a primeira etapa sai pela aresta de início do fluxo do time do card e segue para o responsável.
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
    let stages = db.plan_stages(project_id, item_id).map_err(FlowError::Orchestration)?;
    let Some(first) = stages.first() else {
        return Ok(());
    };
    let owner = stage_agent(&first.who)?;
    let start = snapshot.edges.iter().find(|edge| {
        edge.team_id == team.id && edge.from_node == FLOW_START && edge.when_event == START_EDGE_EVENT
    });
    if let Some(start) = start.filter(|start| start.to_node != first.who) {
        return Err(FlowError::PlanStage(format!(
            "A etapa {FIRST_STAGE_NUMBER} é de {}, mas o fluxo do time {} começa em {}.",
            first.who, team.name, start.to_node
        )));
    }
    let placement = member_placement(&snapshot, &team.id, &first.who)?;
    db.start_plan_stage(
        project_id,
        &first.id,
        &first.who,
        &placement.layer_id,
        start.map(|edge| edge.id),
    )
    .map_err(FlowError::Orchestration)?;
    publish_stage(app, project_id, &first.id)?;
    let text = stage_text(START_STAGE_TEXT, item_id, &first.id);
    spawn_stage_request(app, project_id, &first.id, owner, &text);
    Ok(())
}

// Etapa concluída: a próxima do plano é liberada pela aresta `tarefa_concluida` de quem concluiu para quem recebe.
// Última etapa não faz nada aqui; o card pai acompanha pela transição da etapa no banco.
pub(crate) fn advance_plan(app: &AppHandle, project_id: i64, stage_id: &str) -> Result<(), FlowError> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(FlowError::Orchestration)?;
    let stage = db.read_card(project_id, stage_id).map_err(FlowError::Orchestration)?;
    let Some(parent) = stage.parent.as_deref() else {
        return Ok(());
    };
    let stages = db.plan_stages(project_id, parent).map_err(FlowError::Orchestration)?;
    let Some(position) = stages.iter().position(|candidate| candidate.id == stage.id) else {
        return Ok(());
    };
    let Some(next) = stages.get(position + 1) else {
        return Ok(());
    };
    let Some(snapshot) = load_orchestration(app)? else {
        return Ok(());
    };
    let from = stage_agent(&stage.who)?;
    let from_placement = placement_of(&snapshot, &from).ok_or_else(|| {
        FlowError::PlanStage(format!("{} não está em nenhum time do fluxo.", stage.who))
    })?;
    let edge_id = handoff_edge(&snapshot, &from_placement.team_id, &stage.who, &next.who)?;
    let placement = member_placement(&snapshot, &from_placement.team_id, &next.who)?;
    db.start_plan_stage(project_id, &next.id, &next.who, &placement.layer_id, edge_id)
        .map_err(FlowError::Orchestration)?;
    publish_stage(app, project_id, &next.id)?;
    let text = stage_text(NEXT_STAGE_TEXT, parent, &next.id);
    spawn_stage_request(app, project_id, &next.id, stage_agent(&next.who)?, &text);
    Ok(())
}

fn team_led_by<'a>(snapshot: &'a OrchestrationSnapshot, leader: &str) -> Option<&'a OrchTeam> {
    snapshot
        .teams
        .iter()
        .find(|team| team.leader_agent.as_deref() == Some(leader))
}

// Mesmo agente em etapas seguidas não tem aresta entre eles: a etapa segue sem aresta.
fn handoff_edge(
    snapshot: &OrchestrationSnapshot,
    team_id: &str,
    from: &str,
    to: &str,
) -> Result<Option<i64>, FlowError> {
    if from == to {
        return Ok(None);
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
        .map(|edge| Some(edge.id))
        .ok_or_else(|| {
            FlowError::PlanStage(format!(
                "Não há aresta de {from} para {to} quando a tarefa é concluída no time."
            ))
        })
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
