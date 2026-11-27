use std::collections::HashSet;

use tauri::{AppHandle, Manager};

use crate::agents::{replica_of, Agent, AgentRegistry, Team};
use crate::db::{FlowEdge, OrchestrationSnapshot, FLOW_END, GATEWAY_PARALLEL, LINK_FLOW};
use crate::events::{AgentState, DecisionKind, Speaker, VokiaEvent};
use crate::mcp::SlotDecision;
use crate::orchestration::instances;
use crate::orchestration::policy::{
    HUMAN_GATE_END, HUMAN_GATE_NEVER, REVIEW_BATCH_TITLE, REVIEW_TASK_TITLE,
};
use crate::orchestration::snapshot::{flow_target, leader_of, node_id, placement_of};
use crate::session_key::SessionKey;
use crate::state::{AppState, Batch, RequestContext};
use crate::task::Variable;

use super::super::emit::{emit, emit_feed_note, emit_message, emit_state, now_millis, reconcile_waiting, MessageScope};
use super::super::error::FlowError;
use super::super::turns::open_decision;
use super::super::validation;
use super::{dispatch_request, load_orchestration, report_failure, HUMAN_GATE_LOTE};

// Abre o lote de um turno do líder; o id devolvido vai nas trocas paralelas.
pub(in crate::orchestrator) fn open_batch(state: &AppState, batch: Batch) -> Result<u64, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    let id = flow.next_batch;
    flow.next_batch += 1;
    flow.batches.insert(id, batch);
    Ok(id)
}

pub(in crate::orchestrator) fn is_batch_open(state: &AppState, id: u64) -> bool {
    state
        .flow
        .lock()
        .is_ok_and(|flow| flow.batches.contains_key(&id))
}

// Objetivo e variáveis da tarefa do líder: a réplica usa o mesmo registro que o líder.
pub(in crate::orchestrator) fn batch_task(state: &AppState, batch: Option<u64>) -> Option<(String, Vec<Variable>)> {
    let id = batch?;
    let flow = state.flow.lock().ok()?;
    let open = flow.batches.get(&id)?;
    Some((open.objective.clone(), open.variables.clone()))
}

// Guarda o resultado de uma troca do lote; a última fecha o lote e vai para a junção.
pub(in crate::orchestrator) fn finish_item(
    app: &AppHandle,
    state: &AppState,
    team: &Team,
    batch: Option<u64>,
    agent: &Agent,
    text: String,
) {
    if let Some(id) = batch {
        let result = (agent.clone(), instances::display_name(team, agent), text);
        settle_batch(app, state, id, Some(result));
    }
}

// Fim do despacho do líder: solta a contagem própria do lote.
pub(in crate::orchestrator) fn seal_batch(app: &AppHandle, state: &AppState, batch: u64) {
    settle_batch(app, state, batch, None);
}

// Uma troca (ou o despacho) sai do lote; quando nada mais falta, a junção roda.
fn settle_batch(
    app: &AppHandle,
    state: &AppState,
    id: u64,
    result: Option<(Agent, String, String)>,
) {
    let (done, progress) = {
        let Ok(mut flow) = state.flow.lock() else {
            return;
        };
        let Some(open) = flow.batches.get_mut(&id) else {
            return;
        };
        if let Some((agent, name, text)) = result {
            open.returned.push(agent);
            open.results.push((name, text));
        }
        open.pending = open.pending.saturating_sub(1);
        if open.pending == 0 {
            let closed = flow.batches.remove(&id);
            let progress = closed.as_ref().map(|batch| progress_event(batch, true));
            (closed, progress)
        } else {
            (None, Some(progress_event(open, false)))
        }
    };
    if let Some(progress) = progress {
        let _ = emit(app, progress);
    }
    if let Some(done) = done {
        let project_id = done.project_id;
        if let Err(error) = join_batch(app, state, done) {
            let _ = report_failure(app, project_id, &error);
        }
        reconcile_waiting(app, state, project_id);
    }
}

// Placar do lote: quantas trocas voltaram, quantas foram despachadas e quais ainda rodam.
fn progress_event(batch: &Batch, done: bool) -> VokiaEvent {
    let mut running = batch.started.clone();
    for returned in &batch.returned {
        if let Some(position) = running.iter().position(|agent| agent == returned) {
            running.remove(position);
        }
    }
    VokiaEvent::BatchProgress {
        project_id: batch.project_id,
        leader: batch.leader.clone(),
        returned: batch.returned.len(),
        total: batch.dispatched.len(),
        running,
        done,
    }
}

// Emite o placar do lote que ainda está aberto (usado depois do despacho do líder e da resposta de réplica).
pub(in crate::orchestrator) fn announce_batch(app: &AppHandle, state: &AppState, id: u64) {
    let progress = {
        let Ok(flow) = state.flow.lock() else {
            return;
        };
        flow.batches
            .get(&id)
            .map(|batch| progress_event(batch, false))
    };
    if let Some(progress) = progress {
        let _ = emit(app, progress);
    }
}

// Acesso ao lote aberto; None quando ele já fechou.
pub(in crate::orchestrator) fn with_batch<T>(state: &AppState, id: u64, action: impl FnOnce(&mut Batch) -> T) -> Option<T> {
    let mut flow = state.flow.lock().ok()?;
    flow.batches.get_mut(&id).map(action)
}

// Réplica usa o perfil do base: a política e os limites contam a delegação pelo agente base.
pub(in crate::orchestrator) fn base_agent(agent: &Agent) -> Agent {
    replica_of(agent.as_str())
        .and_then(|(base, _)| Agent::from_stored(base))
        .unwrap_or_else(|| agent.clone())
}

// Junção: uma entrada do líder com o resultado de cada troca, e o human_gate dispara com ela.
fn join_batch(app: &AppHandle, state: &AppState, batch: Batch) -> Result<(), FlowError> {
    let count = batch.results.len();
    let text = batch
        .results
        .iter()
        .map(|(name, text)| format!("{name}: {text}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    emit_message(
        app,
        batch.project_id,
        Speaker::from(batch.leader.clone()),
        Speaker::User,
        &text,
        MessageScope {
            channel_id: batch.channel_id,
            item_id: batch.item_id.as_deref(),
        },
        None,
        None,
        None,
    )?;
    let snapshot = load_orchestration(app)?;
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(FlowError::Team)?;
    let join = snapshot
        .as_ref()
        .and_then(|snapshot| join_target(snapshot, &team, &batch));
    // A junção sai para o fim: os ramos paralelos não passam pelo laço de turnos (que marca o fim), então a junção
    // marca; a validação final abre em release_branch, quando o último ramo do card termina.
    if let (Some(JoinExit::End), Some(item)) = (&join, batch.item_id.as_deref()) {
        let last = batch.returned.last().unwrap_or(&batch.leader);
        mark_flow_end(state, batch.project_id, item, last)?;
        return emit_state(
            app,
            batch.project_id,
            batch.leader,
            AgentState::Idle,
            batch.item_id.as_deref(),
        );
    }
    if let Some(JoinExit::Agent(target)) = join {
        let max_instances = snapshot
            .as_ref()
            .and_then(|snapshot| placement_of(snapshot, &target))
            .map_or(1, |placement| placement.max_instances);
        let max_parallel = snapshot
            .as_ref()
            .map_or(1, |snapshot| snapshot.settings.max_parallel);
        let context = RequestContext::new(None, batch.item_id.clone(), batch.channel_id);
        count_branch(state, batch.project_id, batch.item_id.as_deref())?;
        dispatch_request(
            app,
            state,
            batch.project_id,
            &target,
            &format!("Resultados dos ramos:\n\n{text}"),
            context,
            max_instances,
            max_parallel,
        )?;
        return emit_state(
            app,
            batch.project_id,
            batch.leader,
            AgentState::Idle,
            batch.item_id.as_deref(),
        );
    }
    let gate = snapshot.as_ref().map_or(HUMAN_GATE_LOTE, |snapshot| {
        snapshot.settings.human_gate.as_str()
    });
    if gate == HUMAN_GATE_NEVER {
        return emit_state(
            app,
            batch.project_id,
            batch.leader,
            AgentState::Idle,
            batch.item_id.as_deref(),
        );
    }
    let title = if gate == HUMAN_GATE_END {
        REVIEW_TASK_TITLE
    } else {
        REVIEW_BATCH_TITLE
    };
    open_decision(
        app,
        state,
        &batch.leader,
        batch.project_id,
        batch.item_id.as_deref(),
        &batch.objective,
        SlotDecision {
            kind: DecisionKind::Revisao,
            title: title.to_string(),
            detail: format!("Resultados de {count} trocas em paralelo."),
            options: Vec::new(),
        },
    )?;
    emit_state(
        app,
        batch.project_id,
        batch.leader,
        AgentState::Waiting,
        batch.item_id.as_deref(),
    )
}

enum JoinExit {
    Agent(Agent),
    End,
}

// Gateway paralelo de junção (2 ou mais entradas, uma saída) cujas entradas vêm todas dos ramos do lote: o destino da
// saída (um agente ou o fim do fluxo) recebe os resultados quando o último ramo volta.
fn join_target(snapshot: &OrchestrationSnapshot, team: &Team, batch: &Batch) -> Option<JoinExit> {
    let branches: HashSet<&str> = batch.dispatched.iter().map(node_id).collect();
    let flow_edges = |from: bool, gateway: &str| -> Vec<&FlowEdge> {
        snapshot
            .edges
            .iter()
            .filter(|edge| {
                edge.link == LINK_FLOW
                    && if from {
                        edge.from_node == gateway
                    } else {
                        edge.to_node == gateway
                    }
            })
            .collect()
    };
    snapshot
        .gateways
        .iter()
        .filter(|gateway| gateway.kind == GATEWAY_PARALLEL)
        .find_map(|gateway| {
            let entries = flow_edges(false, &gateway.id);
            let exits = flow_edges(true, &gateway.id);
            let from_branches = entries.len() >= 2
                && entries
                    .iter()
                    .all(|edge| branches.contains(edge.from_node.as_str()));
            match exits.as_slice() {
                [exit] if from_branches && exit.to_node == FLOW_END => Some(JoinExit::End),
                [exit] if from_branches => flow_target(team, &exit.to_node)
                    .cloned()
                    .map(JoinExit::Agent),
                _ => None,
            }
        })
}


// O card chegou ao fim do fluxo por este agente; a validação final espera o último ramo terminar.
pub(in crate::orchestrator) fn mark_flow_end(
    state: &AppState,
    project_id: i64,
    item_id: &str,
    speaker: &Agent,
) -> Result<(), FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    flow.ended
        .insert((project_id, item_id.to_string()), speaker.clone());
    Ok(())
}

// Cada troca com card é um ramo dele; a primeira registra o ramo principal.
pub(in crate::orchestrator) fn register_main_branch(
    state: &AppState,
    project_id: i64,
    item_id: Option<&str>,
) -> Result<(), FlowError> {
    let Some(item) = item_id else {
        return Ok(());
    };
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    flow.open_branches
        .entry((project_id, item.to_string()))
        .or_insert(1);
    Ok(())
}

// Um ramo novo do card: linha assíncrona, saída de gateway paralelo ou a entrega da junção.
pub(in crate::orchestrator) fn count_branch(
    state: &AppState,
    project_id: i64,
    item_id: Option<&str>,
) -> Result<(), FlowError> {
    let Some(item) = item_id else {
        return Ok(());
    };
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    *flow
        .open_branches
        .entry((project_id, item.to_string()))
        .or_insert(0) += 1;
    Ok(())
}

// O ramo terminou (concluído ou não). Sem chegar ao fim, o líder fica sabendo; o card só segue quando nenhum ramo resta.
pub(in crate::orchestrator) async fn release_branch(
    app: &AppHandle,
    state: &AppState,
    team: &Team,
    key: &SessionKey,
    item_id: Option<&str>,
    stopped: Option<String>,
) {
    let Some(item) = item_id else {
        return;
    };
    let failed = stopped.is_some();
    if let Some(reason) = stopped {
        let leader = load_orchestration(app)
            .ok()
            .flatten()
            .map_or_else(|| leader_of(None), |snapshot| leader_of(Some(&snapshot)));
        let text = format!(
            "Ramo {} de {item} parou: {reason}.",
            team.name_of(&base_agent(&key.agent))
        );
        if let Ok(time) = now_millis() {
            let _ = emit_feed_note(app, key.project_id, Speaker::from(leader), &text, time);
        }
    }
    let Some((ended, finished)) = close_branch(state, key.project_id, item, failed) else {
        return;
    };
    // Sem ramo aberto: só o card que chegou ao fim do fluxo vai para a validação final.
    if let Some(speaker) = ended {
        if let Err(error) =
            validation::on_flow_end(app, state, team, key.project_id, item, &speaker, finished)
                .await
        {
            let _ = report_failure(app, key.project_id, &error);
        }
    }
}

// Fecha um ramo do card. Devolve None enquanto restam ramos abertos; no último, o agente que chegou ao fim (se algum
// chegou) e se todos os ramos terminaram sem parar por erro.
pub(in crate::orchestrator) fn close_branch(
    state: &AppState,
    project_id: i64,
    item: &str,
    failed: bool,
) -> Option<(Option<Agent>, bool)> {
    let mut flow = state.flow.lock().ok()?;
    let branch = (project_id, item.to_string());
    if failed {
        flow.stopped.insert(branch.clone());
    }
    let open = flow.open_branches.get(&branch).copied().unwrap_or(0);
    if open > 1 {
        flow.open_branches.insert(branch, open - 1);
        return None;
    }
    flow.open_branches.remove(&branch);
    let finished = !flow.stopped.remove(&branch);
    Some((flow.ended.remove(&branch), finished))
}

