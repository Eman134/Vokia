use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use tauri::{AppHandle, Manager};

use crate::agents::replica_of;
use crate::attachments;
use crate::agents::{Agent, AgentError, AgentProfile, AgentRegistry, Team};
use crate::claude::ProjectAccess;
use crate::db::{
    DbError, DbState, FlowEdge, OrchestrationSnapshot, FLOW_END, GATEWAY_PARALLEL, LINK_FLOW,
};
use crate::events::{AgentState, DecisionKind, Speaker, VokiaEvent};
use crate::mcp::{McpState, SlotDecision};
use crate::orchestration::counters::TaskCounters;
use crate::orchestration::instances;
use crate::orchestration::policy::{
    self, placed_participants, Passage, Verdict, HUMAN_GATE_END, HUMAN_GATE_NEVER,
    REVIEW_BATCH_TITLE, REVIEW_TASK_TITLE,
};
use crate::orchestration::worktree_policy::{leader_limit, should_use};
use crate::projects::worktree;
use crate::session_key::SessionKey;
use crate::state::{
    AppState, Batch, PendingDecision, QueuedJob, RequestContext, WaitingDelegation,
};
use crate::task::{
    long_request_text, parse_definition, ActiveTask, TaskDefinition, TaskOutcome, Variable,
};

use super::agent_session::ask_step;
use super::archive::archive_task;
use super::emit::{
    emit, emit_backlog_changed, emit_feed_note, emit_message, emit_state, now_millis, report_limits,
};
use super::error::{FlowError, RequestOutcome};
use super::project_context::{
    load_project, project_context, project_dir, project_id_from_choice, scan_structure,
};
use super::prompts::NO_NAMES;
use super::prompts::{channel_note, interpreter_role, StepPrompts};
use super::runs::{begin_run, end_run, pause_run, RESUME_NOTE};
use super::session_pool::ExchangeSessions;
use super::text_fallback::{read_reply, TextContext};
use super::turns::{
    count_passage, edge_passages, first_speaker, open_decision, record_passage, run_turns,
    with_cache, Exchange, USER_LABEL,
};
use super::validation::{self, VALIDATION_RETURN};
use crate::orchestration::snapshot::{flow_target, leader_of, node_id, placement_of};
use crate::task::Change;

// Sem configuração de gate, a junção segue o padrão do projeto: revisão a cada lote.
const HUMAN_GATE_LOTE: &str = "lote";

pub async fn handle_request(
    app: &AppHandle,
    state: &AppState,
    request: &str,
    recipient: Agent,
    project_id: i64,
    context: RequestContext,
) -> Result<RequestOutcome, FlowError> {
    handle_request_from(
        app,
        state,
        request,
        Speaker::User,
        recipient,
        project_id,
        context,
    )
    .await
}

// O pedido pode vir de quem não é o usuário (o líder devolvendo uma task reprovada pela validação automática).
pub(super) async fn handle_request_from(
    app: &AppHandle,
    state: &AppState,
    request: &str,
    from: Speaker,
    recipient: Agent,
    project_id: i64,
    context: RequestContext,
) -> Result<RequestOutcome, FlowError> {
    if let Some(problem) = app.state::<DbState>().problem() {
        return Err(FlowError::DatabaseUnavailable(problem.reason));
    }
    let cli = state
        .cli
        .as_ref()
        .map_err(|problem| FlowError::CliUnavailable(problem.reason.clone()))?;
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(FlowError::Team)?;
    project_dir(app, project_id)?;
    // Retomada não repete o pedido no feed: o pedido original já está gravado; só avisa quem continua.
    if context.resumed_run.is_some() {
        emit_message(
            app,
            project_id,
            Speaker::from(recipient.clone()),
            Speaker::User,
            RESUME_NOTE,
            None,
            None,
            None,
            None,
        )?;
    } else {
        emit_message(
            app,
            project_id,
            from,
            Speaker::from(recipient.clone()),
            request,
            context.channel_id,
            None,
            None,
            None,
        )?;
        // Só depois da mensagem gravada: antes disso o anexo ainda é rascunho e pode ser descartado.
        if !context.attachments.is_empty() {
            app.state::<DbState>()
                .require()?
                .mark_attachments_sent(project_id, &context.attachments)?;
        }
    }
    let key = SessionKey {
        project_id,
        agent: recipient,
    };
    if !start_or_enqueue(state, key.clone(), request, context.clone())? {
        return Ok(RequestOutcome::Queued);
    }
    run_flow(app, state, cli, key, request, context, &team).await
}

pub async fn answer_decision(
    app: &AppHandle,
    state: &AppState,
    id: u64,
    answer: &str,
) -> Result<(), FlowError> {
    let decision = {
        let mut decisions = state
            .decisions
            .lock()
            .map_err(|_| FlowError::DecisionsPoisoned)?;
        // Devolver sem motivo é recusado com a decisão ainda aberta: só some depois de aceita.
        let pending = decisions.get(&id).ok_or(FlowError::UnknownDecision)?;
        if pending.kind == DecisionKind::Validacao && returns_without_motive(answer) {
            return Err(DbError::ReturnMotiveRequired.into());
        }
        decisions.remove(&id).ok_or(FlowError::UnknownDecision)?
    };
    if decision.kind == DecisionKind::Validacao {
        return answer_validation(app, state, &decision, answer).await;
    }
    let text = format!(
        "Decisão \"{}\" (sobre o pedido: {}): {answer}",
        decision.title, decision.request
    );
    let (project_id, item_id) = match chosen_project(app, &decision, answer)? {
        Some(chosen) if chosen != decision.project_id => (chosen, None),
        _ => (decision.project_id, decision.item_id),
    };
    let context = RequestContext::new(None, item_id, None);
    handle_request(app, state, &text, decision.agent, project_id, context)
        .await
        .map(|_| ())
}

fn returns_without_motive(answer: &str) -> bool {
    answer
        .strip_prefix(VALIDATION_RETURN)
        .is_some_and(|rest| rest.trim_start_matches(':').trim().is_empty())
}

// Validação final: "Concluir" fecha o card sem falar com agente; "Devolver: <motivo>" volta a task ao líder.
async fn answer_validation(
    app: &AppHandle,
    state: &AppState,
    decision: &PendingDecision,
    answer: &str,
) -> Result<(), FlowError> {
    let item_id = decision
        .item_id
        .as_deref()
        .ok_or(FlowError::UnknownDecision)?;
    match answer.strip_prefix(VALIDATION_RETURN) {
        Some(rest) => {
            let motive = rest.trim_start_matches(':');
            validation::return_task(app, state, decision.project_id, item_id, motive).await
        }
        None => validation::conclude_card(app, decision.project_id, item_id),
    }
}

// Escolha de projeto feita pelo Ember, pelo id do rótulo. O card de origem pertence ao projeto original e não vai junto.
fn chosen_project(
    app: &AppHandle,
    decision: &PendingDecision,
    answer: &str,
) -> Result<Option<i64>, FlowError> {
    let asks_project =
        decision.agent == Agent::ember() && decision.options.iter().any(|option| option == answer);
    if !asks_project {
        return Ok(None);
    }
    project_id_from_choice(app, answer)
}

// Delegação paralela: uma instância ociosa do base; todas ocupadas, fila da instância base; no teto de max_parallel, espera no projeto (nada se perde).
#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_request(
    app: &AppHandle,
    state: &AppState,
    project_id: i64,
    base: &Agent,
    request: &str,
    context: RequestContext,
    max_instances: i64,
    max_parallel: i64,
) -> Result<(), FlowError> {
    let parallel = RequestContext {
        parallel: true,
        ..context
    };
    // Conta a troca no lote antes de qualquer caminho (inicia, fila ou espera), para a junção não sair cedo.
    if let Some(batch) = parallel.batch {
        let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
        if let Some(open) = flow.batches.get_mut(&batch) {
            open.pending += 1;
            open.dispatched.push(base.clone());
        }
    }
    start_dispatch(
        app,
        state,
        project_id,
        base,
        request,
        parallel,
        max_instances,
        max_parallel,
        true,
    )
}

// Card que ainda não tem worktree e cujo líder já usa o máximo dele: o pedido espera.
struct WorktreeWait {
    leader: String,
    item_id: String,
    max: i64,
}

fn worktree_wait(
    app: &AppHandle,
    project_id: i64,
    context: &RequestContext,
) -> Result<Option<WorktreeWait>, FlowError> {
    let Some(item_id) = context.item_id.as_deref() else {
        return Ok(None);
    };
    let Some(snapshot) = load_orchestration(app)? else {
        return Ok(None);
    };
    if !should_use(&snapshot.settings.worktree_policy, &snapshot, Some(item_id)) {
        return Ok(None);
    }
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return Ok(None);
    };
    let card = match db.read_card(project_id, item_id) {
        Ok(card) => card,
        Err(DbError::BacklogItemNotFound(_)) => return Ok(None),
        Err(error) => return Err(FlowError::Orchestration(error)),
    };
    if card.worktree_path.is_some() {
        return Ok(None);
    }
    let Some(max) = leader_limit(&snapshot, &card.who) else {
        return Ok(None);
    };
    let in_use = db
        .count_team_worktrees(&card.who)
        .map_err(FlowError::Orchestration)?;
    Ok((in_use >= max).then(|| WorktreeWait {
        leader: card.who,
        item_id: item_id.to_string(),
        max,
    }))
}

// Segue o despacho já contado no lote; `announce` avisa o líder só na primeira espera por worktree.
#[allow(clippy::too_many_arguments)]
fn start_dispatch(
    app: &AppHandle,
    state: &AppState,
    project_id: i64,
    base: &Agent,
    request: &str,
    parallel: RequestContext,
    max_instances: i64,
    max_parallel: i64,
    announce: bool,
) -> Result<(), FlowError> {
    let key_of = |agent: &Agent| SessionKey {
        project_id,
        agent: agent.clone(),
    };
    if let Some(wait) = worktree_wait(app, project_id, &parallel)? {
        if announce {
            if let Some(leader) = Agent::from_stored(&wait.leader) {
                let text = format!(
                    "Aguardando worktree livre para {} (limite {}).",
                    wait.item_id, wait.max
                );
                emit_feed_note(app, project_id, Speaker::from(leader), &text, now_millis()?)?;
            }
        }
        let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
        flow.waiting
            .entry(project_id)
            .or_default()
            .push_back(WaitingDelegation {
                base: base.clone(),
                text: request.to_string(),
                context: parallel,
            });
        return Ok(());
    }
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    let Some(agent) = instances::pick(base, max_instances, |agent| {
        flow.pending.contains_key(&key_of(agent))
    }) else {
        flow.pending
            .entry(key_of(base))
            .or_default()
            .push_back(QueuedJob {
                text: request.to_string(),
                context: parallel,
            });
        return Ok(());
    };
    let running = flow
        .pending
        .keys()
        .filter(|key| key.project_id == project_id)
        .count() as i64;
    if running >= max_parallel {
        flow.waiting
            .entry(project_id)
            .or_default()
            .push_back(WaitingDelegation {
                base: base.clone(),
                text: request.to_string(),
                context: parallel,
            });
        return Ok(());
    }
    let key = key_of(&agent);
    flow.pending.insert(key.clone(), VecDeque::new());
    drop(flow);
    spawn_flow(app, key, request.to_string(), parallel);
    Ok(())
}

// Abre o lote de um turno do líder; o id devolvido vai nas trocas paralelas.
pub(super) fn open_batch(state: &AppState, batch: Batch) -> Result<u64, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    let id = flow.next_batch;
    flow.next_batch += 1;
    flow.batches.insert(id, batch);
    Ok(id)
}

// Objetivo e variáveis da tarefa do líder: a réplica usa o mesmo registro que o líder.
fn batch_task(state: &AppState, batch: Option<u64>) -> Option<(String, Vec<Variable>)> {
    let id = batch?;
    let flow = state.flow.lock().ok()?;
    let open = flow.batches.get(&id)?;
    Some((open.objective.clone(), open.variables.clone()))
}

// Guarda o resultado de uma troca do lote; a última fecha o lote e vai para a junção.
fn finish_item(
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
pub(super) fn seal_batch(app: &AppHandle, state: &AppState, batch: u64) {
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
pub(super) fn announce_batch(app: &AppHandle, state: &AppState, id: u64) {
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
fn with_batch<T>(state: &AppState, id: u64, action: impl FnOnce(&mut Batch) -> T) -> Option<T> {
    let mut flow = state.flow.lock().ok()?;
    flow.batches.get_mut(&id).map(action)
}

// Réplica usa o perfil do base: a política e os limites contam a delegação pelo agente base.
fn base_agent(agent: &Agent) -> Agent {
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
        batch.channel_id,
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
        return emit_state(app, batch.project_id, batch.leader, AgentState::Idle);
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
        return emit_state(app, batch.project_id, batch.leader, AgentState::Idle);
    }
    let gate = snapshot.as_ref().map_or(HUMAN_GATE_LOTE, |snapshot| {
        snapshot.settings.human_gate.as_str()
    });
    if gate == HUMAN_GATE_NEVER {
        return emit_state(app, batch.project_id, batch.leader, AgentState::Idle);
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
    emit_state(app, batch.project_id, batch.leader, AgentState::Waiting)
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

fn spawn_flow(app: &AppHandle, key: SessionKey, request: String, context: RequestContext) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let project_id = key.project_id;
        let state = app.state::<AppState>();
        let item_id = context.item_id.clone();
        let cli = match state.cli.as_ref() {
            Ok(cli) => cli.clone(),
            Err(problem) => {
                let _ = stop_with_error(&app, project_id, problem.reason.clone());
                abandon_flow(&app, &state, &key, item_id.as_deref());
                return;
            }
        };
        let team = match app.state::<AgentRegistry>().team() {
            Ok(team) => team,
            Err(error) => {
                let _ = report_failure(&app, project_id, &FlowError::Team(error));
                abandon_flow(&app, &state, &key, item_id.as_deref());
                return;
            }
        };
        if let Err(error) = run_flow(&app, &state, &cli, key, &request, context, &team).await {
            let _ = report_failure(&app, project_id, &error);
        }
    });
}

// O fluxo não chegou a rodar: solta a reserva da sessão e o ramo do card, para não prender open_branches nem pending.
fn abandon_flow(app: &AppHandle, state: &AppState, key: &SessionKey, item_id: Option<&str>) {
    if let Ok(mut flow) = state.flow.lock() {
        flow.pending.remove(key);
        flow.running.remove(key);
    }
    if let Some(item) = item_id {
        close_branch(state, key.project_id, item, true);
    }
    drain_waiting(app, state, key.project_id);
}

fn start_or_enqueue(
    state: &AppState,
    key: SessionKey,
    request: &str,
    context: RequestContext,
) -> Result<bool, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    if let Some(pending) = flow.pending.get_mut(&key) {
        pending.push_back(QueuedJob {
            text: request.to_string(),
            context,
        });
        return Ok(false);
    }
    flow.pending.insert(key, VecDeque::new());
    Ok(true)
}

fn next_job(state: &AppState, key: SessionKey) -> Result<Option<QueuedJob>, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    let next = flow.pending.get_mut(&key).and_then(VecDeque::pop_front);
    if next.is_none() {
        flow.pending.remove(&key);
        flow.running.remove(&key);
    }
    Ok(next)
}

// O card chegou ao fim do fluxo por este agente; a validação final espera o último ramo terminar.
pub(super) fn mark_flow_end(
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
fn register_main_branch(
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
pub(super) fn count_branch(
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
async fn release_branch(
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
fn close_branch(
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

fn set_running_item(
    state: &AppState,
    key: &SessionKey,
    item_id: Option<&str>,
) -> Result<(), FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    match item_id {
        Some(id) => flow.running.insert(key.clone(), id.to_string()),
        None => flow.running.remove(key),
    };
    Ok(())
}

pub(crate) struct ActiveInstance {
    pub(crate) project_id: i64,
    pub(crate) agent: Agent,
    pub(crate) item_id: Option<String>,
}

// Fluxos em andamento em todos os projetos; réplicas são as que têm sufixo -<n> no id.
pub(crate) fn all_active_instances(state: &AppState) -> Result<Vec<ActiveInstance>, FlowError> {
    let flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    Ok(flow
        .pending
        .keys()
        .map(|key| ActiveInstance {
            project_id: key.project_id,
            agent: key.agent.clone(),
            item_id: flow.running.get(key).cloned(),
        })
        .collect())
}

pub(crate) fn active_instances(
    state: &AppState,
    project_id: i64,
) -> Result<Vec<ActiveInstance>, FlowError> {
    Ok(all_active_instances(state)?
        .into_iter()
        .filter(|instance| instance.project_id == project_id)
        .collect())
}

async fn run_flow(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    key: SessionKey,
    request: &str,
    context: RequestContext,
    team: &Team,
) -> Result<RequestOutcome, FlowError> {
    let mut request = request.to_string();
    let mut item_id = context.item_id;
    let mut channel_id = context.channel_id;
    let mut parallel = context.parallel;
    let parallel_run = context.run_id;
    let mut batch = context.batch;
    let mut resumed_run = context.resumed_run;
    let mut outcome: Result<(), FlowError> = Ok(());
    // Só o primeiro exchange é o pedido de quem chamou; os seguintes saem da fila interna.
    let mut first: Option<RequestOutcome> = None;
    let mut current_run: Option<u64> = None;
    loop {
        set_running_item(state, &key, item_id.as_deref())?;
        register_main_branch(state, key.project_id, item_id.as_deref())?;
        let journal_run = if parallel {
            current_run.or(parallel_run)
        } else {
            current_run
        };
        // A retomada continua no registro original: o started_at antigo mantém a janela do que já foi feito.
        let journal = match resumed_run.take() {
            Some(id) => Some(id),
            None => begin_run(app, &key, &request, item_id.as_deref(), journal_run)?,
        };
        let exchange = if parallel {
            let item = run_parallel_item(
                app,
                state,
                cli,
                key.clone(),
                &request,
                item_id.as_deref(),
                channel_id,
                current_run.or(parallel_run),
                batch,
                team,
            )
            .await;
            match item {
                Ok((outcome, text)) => {
                    finish_item(app, state, team, batch, &key.agent, text);
                    Ok(outcome)
                }
                Err(error) => {
                    finish_item(app, state, team, batch, &key.agent, error.to_string());
                    Err(error)
                }
            }
        } else {
            run_exchange(
                app,
                state,
                cli,
                key.clone(),
                &request,
                item_id.as_deref(),
                channel_id,
                team,
                current_run,
            )
            .await
        };
        let stopped = match &exchange {
            Ok(TaskOutcome::Finished) => None,
            Ok(_) => Some("a troca terminou sem concluir".to_string()),
            Err(error) => Some(error.to_string()),
        };
        let (this, limit) = match exchange {
            Ok(TaskOutcome::Finished) => (RequestOutcome::Finished, None),
            Ok(_) => (RequestOutcome::Failed, None),
            Err(error) => match error.usage_limit() {
                Some(limit) => (RequestOutcome::Paused, Some(limit)),
                None if error.is_limit() => {
                    outcome = outcome.and(set_agents_idle(app, key.project_id));
                    (RequestOutcome::Failed, None)
                }
                None => {
                    outcome = outcome.and(report_failure(app, key.project_id, &error));
                    (RequestOutcome::Failed, None)
                }
            },
        };
        match limit {
            Some(limit) => pause_run(app, &key, journal, limit)?,
            None => end_run(app, journal)?,
        }
        if let Some(run_id) = current_run {
            emit(
                app,
                VokiaEvent::RequestSettled {
                    run_id,
                    project_id: key.project_id,
                    outcome: this,
                },
            )?;
        }
        first.get_or_insert(this);
        release_branch(app, state, team, &key, item_id.as_deref(), stopped).await;
        match next_job(state, key.clone())? {
            Some(job) => {
                request = job.text;
                current_run = job.context.run_id;
                item_id = job.context.item_id;
                channel_id = job.context.channel_id;
                parallel = job.context.parallel;
                batch = job.context.batch;
                resumed_run = job.context.resumed_run;
            }
            None => {
                drain_waiting(app, state, key.project_id);
                return outcome.map(|()| first.unwrap_or(RequestOutcome::Failed));
            }
        }
    }
}

// As sessões do projeto ficam reservadas até o fim da mensagem; a ordem fixa evita impasse entre fluxos.
#[allow(clippy::too_many_arguments)] // contexto da troca repassado ao laço de turnos
async fn run_exchange(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    key: SessionKey,
    request: &str,
    item_id: Option<&str>,
    channel_id: Option<i64>,
    team: &Team,
    run_id: Option<u64>,
) -> Result<TaskOutcome, FlowError> {
    // Uma tarefa por projeto: a troca de líder segura o lock da tarefa; cada passo reserva só a instância que fala.
    let task = state.sessions.task_lock(key.project_id);
    let _task_guard = task.lock_owned().await;
    let mut sessions = ExchangeSessions::new(&state.sessions, key.project_id);
    let result = exchange_turns(
        app,
        state,
        cli,
        key,
        request,
        item_id,
        channel_id,
        &mut sessions,
        team,
        run_id,
    )
    .await;
    sessions.release().await;
    result
}

const WORKTREES_DIR: &str = "worktrees";

// Pedido com card roda no worktree do card quando a política manda; sem card, na pasta do projeto.
async fn working_dir(
    app: &AppHandle,
    project_id: i64,
    item_id: Option<&str>,
    snapshot: Option<&OrchestrationSnapshot>,
    project_dir: &Path,
) -> Result<PathBuf, FlowError> {
    let (Some(snapshot), Some(item)) = (snapshot, item_id) else {
        return Ok(project_dir.to_path_buf());
    };
    if !should_use(&snapshot.settings.worktree_policy, snapshot, Some(item)) {
        return Ok(project_dir.to_path_buf());
    }
    let project = load_project(app, project_id)?;
    let root = app
        .path()
        .app_data_dir()
        .map_err(FlowError::ArchivePath)?
        .join(WORKTREES_DIR)
        .join(project_id.to_string());
    let worktree = worktree::ensure(project_dir, &root, item, project.setup_command.as_deref())
        .await
        .map_err(FlowError::Worktree)?;
    Ok(worktree.path)
}

fn attachments_dir(app: &AppHandle, project_id: i64) -> Result<PathBuf, FlowError> {
    let data_dir = app.path().app_data_dir().map_err(FlowError::ArchivePath)?;
    attachments::project_dir(&data_dir, project_id).map_err(FlowError::AttachmentsDir)
}

// Delegação paralela: a instância responde ao pedido uma vez. Não interpreta nem abre tarefa do projeto, então a tarefa do líder fica intacta.
#[allow(clippy::too_many_arguments)]
async fn run_parallel_item(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    key: SessionKey,
    request: &str,
    item_id: Option<&str>,
    channel_id: Option<i64>,
    run_id: Option<u64>,
    batch: Option<u64>,
    team: &Team,
) -> Result<(TaskOutcome, String), FlowError> {
    emit(
        app,
        VokiaEvent::AgentNamed {
            project_id: key.project_id,
            agent: key.agent.clone(),
            name: instances::display_name(team, &key.agent),
        },
    )?;
    emit_state(app, key.project_id, key.agent.clone(), AgentState::Sending)?;
    if let Some(id) = batch {
        with_batch(state, id, |open| open.started.push(key.agent.clone()));
        announce_batch(app, state, id);
    }
    let project_dir = project_dir(app, key.project_id)?;
    let attachments_dir = attachments_dir(app, key.project_id)?;
    let project = load_project(app, key.project_id)?;
    let tree = scan_structure(&project_dir).await;
    let context = project_context(&project, tree.as_ref());
    let orchestration = load_orchestration(app)?;
    let working = working_dir(
        app,
        key.project_id,
        item_id,
        orchestration.as_ref(),
        &project_dir,
    )
    .await?;
    let (objective, variables) =
        batch_task(state, batch).unwrap_or_else(|| (request.to_string(), Vec::new()));
    let definition = parallel_definition(&key.agent, objective, variables);
    let exchange = Exchange {
        app,
        state,
        cli,
        project: ProjectAccess {
            dir: &working,
            attachments: &attachments_dir,
            level: project.permission_level,
        },
        project_context: &context,
        request,
        item_id,
        channel_id,
        key: key.clone(),
        definition: &definition,
        team,
        orchestration: orchestration.as_ref(),
        run_id,
    };
    let mut sessions = ExchangeSessions::new(&state.sessions, key.project_id);
    let result = answer_once(&exchange, &mut sessions, batch).await;
    sessions.release().await;
    result.map(|text| (TaskOutcome::Finished, text))
}

// Resposta da réplica: as delegações dela entram no mesmo lote (a junção espera por elas); o texto vai para a junção.
async fn answer_once(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions<'_>,
    batch: Option<u64>,
) -> Result<String, FlowError> {
    let agent = exchange.key.agent.clone();
    let project_id = exchange.key.project_id;
    let has_mcp_server = exchange.app.state::<McpState>().server().is_some();
    let transport = super::fallback::effective_tool_transport_for(
        exchange.app,
        exchange.state,
        exchange.team,
        &agent,
        Some(exchange.project.level),
    );
    let tool_mode = if !has_mcp_server {
        super::prompts::ToolPromptMode::None
    } else {
        match transport {
            crate::providers::ToolTransport::McpNative => super::prompts::ToolPromptMode::Mcp,
            crate::providers::ToolTransport::VokiaCli => {
                if super::adapters::resolve_vokia_cli_path().is_some() {
                    super::prompts::ToolPromptMode::VokiaCli
                } else {
                    super::prompts::ToolPromptMode::None
                }
            }
            crate::providers::ToolTransport::None => super::prompts::ToolPromptMode::None,
        }
    };
    let with_tools = tool_mode != super::prompts::ToolPromptMode::None;
    let prompts = with_cache(exchange.state, project_id, |cache| {
        Ok(StepPrompts::build(
            cache,
            exchange.definition,
            &agent,
            exchange.request,
            tool_mode,
        ))
    })?;
    let mut reply = match ask_step(exchange, sessions, &agent, &prompts).await {
        Ok(reply) => reply,
        Err(error) => {
            if error.is_limit() || error.is_cli_failure() {
                let _ = super::emit::emit_error_message(
                    exchange.app,
                    project_id,
                    Speaker::from(agent.clone()),
                    exchange.channel_id,
                    &error.to_string(),
                );
            }
            return Err(error);
        }
    };
    let read = read_reply(
        with_tools,
        &TextContext {
            app: exchange.app,
            state: exchange.state,
            project_id,
            speaker: &agent,
            item_id: exchange.item_id,
            team: exchange.team,
        },
        &reply.text,
    )?;
    let base = base_agent(&agent);
    let snapshot = exchange.orchestration;
    let team = exchange.team;
    if read.cards.changed {
        emit_backlog_changed(exchange.app, project_id)?;
    }
    // Decisões, propostas e confirmações da réplica entram no registro da tarefa, como no líder.
    let decision = reply
        .outcome
        .decision
        .or_else(|| read.decision.map(|parsed| parsed.split().0));
    if let Some(decision) = decision {
        open_decision(
            exchange.app,
            exchange.state,
            &agent,
            project_id,
            exchange.item_id,
            exchange.request,
            decision,
        )?;
    }
    let mut shown = read.visible.clone();
    if let Some(proposal) = reply.outcome.proposal {
        emit_message(
            exchange.app,
            project_id,
            Speaker::from(agent.clone()),
            Speaker::User,
            &read.visible,
            exchange.channel_id,
            None,
            Some(proposal),
            reply.usage.take(),
        )?;
        shown = String::new();
    }
    // A réplica também conta um turno da tarefa: o limite do líder vale para ela.
    let turn_limit = batch.and_then(|id| {
        with_batch(exchange.state, id, |open| {
            open.counters.register_turn(&base);
            policy::task_limit_reached(snapshot, &open.counters)
        })
        .flatten()
    });
    let mut notes: Vec<String> = read.rejected;
    // Mesma regra do líder: o que a réplica alterou (MCP ou texto) entra na tarefa e no que volta ao lote.
    let accepted: Vec<Change> = reply
        .outcome
        .variables
        .accepted
        .into_iter()
        .chain(read.applied.accepted)
        .collect();
    let limited = turn_limit.is_some();
    if let Some(motive) = turn_limit {
        open_decision(
            exchange.app,
            exchange.state,
            &agent,
            project_id,
            exchange.item_id,
            exchange.request,
            limit_decision(motive),
        )?;
    }
    let delegations: Vec<(Agent, String)> = reply
        .outcome
        .delegation
        .or(read.delegation)
        .into_iter()
        .collect();
    let max_parallel = snapshot.map_or(1, |snapshot| snapshot.settings.max_parallel);
    for (helper, request) in delegations {
        let name = team.name_of(&helper).to_string();
        if limited {
            notes.push(format!(
                "Delegação para {name} não enviada: limite do mapa atingido."
            ));
            continue;
        }
        // Mesma política e mesmos contadores do líder: sem isso, réplicas poderiam formar ciclo.
        let verdict = batch
            .and_then(|id| {
                with_batch(exchange.state, id, |open| {
                    let passed =
                        edge_passages(exchange.app, project_id, exchange.item_id, &open.counters);
                    policy::check(
                        snapshot,
                        team,
                        &base,
                        &helper,
                        &open.counters,
                        &passed,
                        None,
                    )
                })
            })
            .unwrap_or_else(|| {
                let counters = TaskCounters::default();
                let passed = edge_passages(exchange.app, project_id, exchange.item_id, &counters);
                policy::check(snapshot, team, &base, &helper, &counters, &passed, None)
            });
        let deliveries: Vec<(Agent, Passage)> = match verdict {
            Verdict::Deliver(passage) => {
                let target = passage
                    .routed
                    .as_ref()
                    .map_or_else(|| helper.clone(), |routed| routed.to.clone());
                vec![(target, passage)]
            }
            Verdict::Fork(branches) => branches
                .into_iter()
                .filter_map(|branch| Some((branch.routed.as_ref()?.to.clone(), branch)))
                .collect(),
            Verdict::Refuse(motive) => {
                notes.push(format!("Delegação para {name} recusada: {motive}"));
                continue;
            }
            Verdict::Escalate(motive) => {
                open_decision(
                    exchange.app,
                    exchange.state,
                    &agent,
                    project_id,
                    exchange.item_id,
                    exchange.request,
                    limit_decision(motive),
                )?;
                continue;
            }
        };
        for (target, passage) in deliveries {
            if let Some(id) = batch {
                with_batch(exchange.state, id, |open| {
                    count_passage(&mut open.counters, &base, &target, &passage);
                });
            }
            record_passage(
                exchange.app,
                exchange.run_id,
                project_id,
                exchange.item_id,
                None,
                &agent,
                &target,
                &passage,
            )?;
            let max_instances = snapshot
                .and_then(|snapshot| placement_of(snapshot, &target))
                .map_or(1, |placement| placement.max_instances);
            let mut context = RequestContext::new(
                exchange.run_id,
                exchange.item_id.map(str::to_string),
                exchange.channel_id,
            );
            context.batch = batch;
            count_branch(exchange.state, project_id, exchange.item_id)?;
            dispatch_request(
                exchange.app,
                exchange.state,
                project_id,
                &target,
                &request,
                context,
                max_instances,
                max_parallel,
            )?;
            emit_message(
                exchange.app,
                project_id,
                Speaker::from(agent.clone()),
                Speaker::from(target),
                &request,
                exchange.channel_id,
                None,
                None,
                reply.usage.take(),
            )?;
        }
    }
    let mut parts = vec![shown];
    parts.extend(accepted.iter().filter_map(Change::feed));
    parts.extend(reply.outcome.confirmations);
    parts.extend(read.cards.confirmations);
    parts.extend(notes);
    let text = parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if let Some(id) = batch {
        announce_batch(exchange.app, exchange.state, id);
    }
    emit_state(exchange.app, project_id, agent, AgentState::Idle)?;
    Ok(text)
}

// Estouro de limite do mapa vira decisão do usuário, com o motivo da política.
fn limit_decision(motive: String) -> SlotDecision {
    SlotDecision {
        kind: DecisionKind::Pergunta,
        title: policy::LIMIT_TITLE.to_string(),
        detail: motive,
        options: Vec::new(),
    }
}

fn parallel_definition(
    agent: &Agent,
    objective: String,
    variables: Vec<Variable>,
) -> TaskDefinition {
    TaskDefinition {
        nova_tarefa: false,
        objetivo: objective,
        criterion: String::new(),
        verify: false,
        participants: vec![agent.clone()],
        alternate: false,
        variables,
        stop: None,
    }
}

// Ao fim de um fluxo, as delegações que esperavam o teto (ou worktree livre) entram enquanto houver vaga.
// Cada espera é tentada uma vez por chamada: a que continua bloqueada volta ao fim da fila.
fn drain_waiting(app: &AppHandle, state: &AppState, project_id: i64) {
    let snapshot = match load_orchestration(app) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let _ = report_failure(app, project_id, &error);
            return;
        }
    };
    let max_parallel = snapshot
        .as_ref()
        .map_or(1, |snapshot| snapshot.settings.max_parallel);
    let queued = state
        .flow
        .lock()
        .ok()
        .and_then(|flow| flow.waiting.get(&project_id).map(VecDeque::len))
        .unwrap_or(0);
    for _ in 0..queued {
        if !has_room(state, project_id, max_parallel) {
            return;
        }
        let Some(waiting) = take_waiting(state, project_id) else {
            return;
        };
        let max_instances = snapshot
            .as_ref()
            .and_then(|snapshot| placement_of(snapshot, &waiting.base))
            .map_or(1, |placement| placement.max_instances);
        if let Err(error) = start_dispatch(
            app,
            state,
            project_id,
            &waiting.base,
            &waiting.text,
            waiting.context,
            max_instances,
            max_parallel,
            false,
        ) {
            let _ = report_failure(app, project_id, &error);
            return;
        }
    }
}

// Um card saiu de In progress, possivelmente em outro projeto: toda fila de espera tenta de novo.
pub(super) fn drain_all_waiting(app: &AppHandle, state: &AppState) {
    let projects: Vec<i64> = state
        .flow
        .lock()
        .map(|flow| {
            flow.waiting
                .iter()
                .filter(|(_, queue)| !queue.is_empty())
                .map(|(project_id, _)| *project_id)
                .collect()
        })
        .unwrap_or_default();
    for project_id in projects {
        drain_waiting(app, state, project_id);
    }
}

fn has_room(state: &AppState, project_id: i64, max_parallel: i64) -> bool {
    let Ok(flow) = state.flow.lock() else {
        return false;
    };
    let running = flow
        .pending
        .keys()
        .filter(|key| key.project_id == project_id)
        .count() as i64;
    running < max_parallel
}

fn take_waiting(state: &AppState, project_id: i64) -> Option<WaitingDelegation> {
    let mut flow = state.flow.lock().ok()?;
    flow.waiting.get_mut(&project_id)?.pop_front()
}

// Banco ausente vira None (prompt de antes); erro de leitura do banco interrompe a troca.
pub(super) fn load_orchestration(
    app: &AppHandle,
) -> Result<Option<OrchestrationSnapshot>, FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(None);
    };
    db.orchestration()
        .map(Some)
        .map_err(FlowError::Orchestration)
}

#[allow(clippy::too_many_arguments)]
async fn exchange_turns(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    key: SessionKey,
    request: &str,
    item_id: Option<&str>,
    channel_id: Option<i64>,
    sessions: &mut ExchangeSessions<'_>,
    team: &Team,
    run_id: Option<u64>,
) -> Result<TaskOutcome, FlowError> {
    emit_state(app, key.project_id, key.agent.clone(), AgentState::Sending)?;
    let channel = find_channel(app, key.project_id, channel_id, team);
    let request = with_channel_note(request, channel.as_ref());
    let request = request.as_str();
    let project_dir = project_dir(app, key.project_id)?;
    let attachments_dir = attachments_dir(app, key.project_id)?;
    let project = load_project(app, key.project_id)?;
    // Sem cache: a pasta é relida a cada exchange para refletir o estado atual.
    let tree = scan_structure(&project_dir).await;
    let context = project_context(&project, tree.as_ref());
    // Snapshot único da troca: edições feitas durante ela só valem na próxima.
    let orchestration = load_orchestration(app)?;
    let working = working_dir(
        app,
        key.project_id,
        item_id,
        orchestration.as_ref(),
        &project_dir,
    )
    .await?;
    let mut definition = match interpret_task(
        app,
        state,
        cli,
        request,
        key.clone(),
        team,
        orchestration.as_ref(),
    )
    .await
    {
        Ok(definition) => definition,
        Err(error) => {
            if error.is_limit() || error.is_cli_failure() {
                let _ = super::emit::emit_error_message(
                    app,
                    key.project_id,
                    Speaker::from(key.agent.clone()),
                    channel.as_ref().map(|found| found.id),
                    &error.to_string(),
                );
            }
            return Err(error);
        }
    };
    definition.participants = placed_participants(
        orchestration.as_ref(),
        std::mem::take(&mut definition.participants),
        &key.agent,
    );
    if first_speaker(&definition, &key.agent) != key.agent {
        emit_state(app, key.project_id, key.agent.clone(), AgentState::Idle)?;
    }
    begin_task(app, state, key.project_id, &definition, request)?;
    let mut transcript = vec![format!("{USER_LABEL}: {request}")];
    let access = ProjectAccess {
        dir: &working,
        attachments: &attachments_dir,
        level: project.permission_level,
    };
    let exchange = Exchange {
        app,
        state,
        cli,
        project: access,
        project_context: &context,
        request,
        item_id,
        channel_id: channel.as_ref().map(|found| found.id),
        key: key.clone(),
        definition: &definition,
        team,
        orchestration: orchestration.as_ref(),
        run_id,
    };
    let result = run_turns(&exchange, sessions, &mut transcript).await;
    let outcome = match &result {
        Ok(outcome) => *outcome,
        Err(error) if error.usage_limit().is_some() => TaskOutcome::Paused,
        Err(_) => TaskOutcome::Failed,
    };
    let closed = close_task(state, key.project_id, outcome, transcript);
    result?;
    closed?;
    Ok(outcome)
}

struct FoundChannel {
    id: i64,
    note: String,
}

// Id de canal excluído ou de outro projeto vira "sem canal": o pedido segue normalmente.
fn find_channel(
    app: &AppHandle,
    project_id: i64,
    channel_id: Option<i64>,
    team: &Team,
) -> Option<FoundChannel> {
    let wanted = channel_id?;
    let channels = app
        .state::<DbState>()
        .db()?
        .list_channels(project_id)
        .ok()?;
    let channel = channels.into_iter().find(|channel| channel.id == wanted)?;
    let names: Vec<String> = channel
        .members
        .iter()
        .filter_map(|member| Agent::from_stored(member))
        .filter(|agent| team.profile(agent).is_some())
        .map(|agent| team.name_of(&agent).to_string())
        .collect();
    Some(FoundChannel {
        id: channel.id,
        note: channel_note(&channel.name, &names),
    })
}

fn with_channel_note(request: &str, channel: Option<&FoundChannel>) -> String {
    match channel {
        Some(found) => format!("{request}\n{}", found.note),
        None => request.to_string(),
    }
}

async fn interpret_task(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    request: &str,
    key: SessionKey,
    team: &Team,
    orchestration: Option<&OrchestrationSnapshot>,
) -> Result<TaskDefinition, FlowError> {
    let origin = key.agent;
    let profile = profile_of(team, &origin)?;
    let names = with_cache(state, key.project_id, |cache| Ok(cache.names().join(", ")))?;
    let names = if names.is_empty() {
        NO_NAMES.to_string()
    } else {
        names
    };
    let message = format!(
        "{} Destinatário: {}. Variáveis existentes: {names}. Pedido: {request}",
        team.roster_line(),
        team.name_of(&origin)
    );
    let reply = super::fallback::ask_with_fallback(
        app,
        state,
        cli,
        &origin,
        profile,
        &message,
        &interpreter_role(team, orchestration),
    )
    .await?;
    report_limits(app, reply.limits)?;
    Ok(parse_definition(&reply.text, &origin, request, team))
}

pub(super) fn profile_of<'a>(team: &'a Team, agent: &Agent) -> Result<&'a AgentProfile, FlowError> {
    team.profile(agent)
        .ok_or_else(|| FlowError::Team(AgentError::UnknownAgent(agent.to_string())))
}

fn begin_task(
    app: &AppHandle,
    state: &AppState,
    project_id: i64,
    definition: &TaskDefinition,
    request: &str,
) -> Result<(), FlowError> {
    let archived = with_cache(state, project_id, |cache| {
        let archived = if definition.nova_tarefa {
            cache.archive()
        } else {
            None
        };
        let task = ActiveTask {
            objetivo: definition.objetivo.clone(),
            participants: definition.participants.clone(),
            long_request: long_request_text(request),
            outcome: None,
        };
        cache.start_task(task, definition.variables.clone())?;
        Ok(archived)
    })?;
    match archived {
        Some(record) => archive_task(app, state, record),
        None => Ok(()),
    }
}

fn close_task(
    state: &AppState,
    project_id: i64,
    outcome: TaskOutcome,
    transcript: Vec<String>,
) -> Result<(), FlowError> {
    with_cache(state, project_id, |cache| {
        cache.finish_task(outcome);
        cache.push_conversation(transcript);
        Ok(())
    })
}

pub(crate) fn has_flow(state: &AppState, project_id: Option<i64>) -> Result<bool, FlowError> {
    let flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    Ok(flow
        .pending
        .keys()
        .any(|key| project_id.is_none_or(|id| key.project_id == id)))
}

// Os mapas só recebem inserções e remoções simples; um lock envenenado não deixa estado pela metade.
pub(crate) fn forget_project(state: &AppState, project_id: i64) {
    state
        .cache
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&project_id);
    state
        .decisions
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|_, decision| decision.project_id != project_id);
    state.console.forget_project(project_id);
}

pub fn report_failure(
    app: &AppHandle,
    project_id: i64,
    error: &FlowError,
) -> Result<(), FlowError> {
    stop_with_error(app, project_id, error.to_string())
}

pub(super) fn stop_with_error(
    app: &AppHandle,
    project_id: i64,
    message: String,
) -> Result<(), FlowError> {
    emit(app, VokiaEvent::Error { message })?;
    set_agents_idle(app, project_id)
}

fn set_agents_idle(app: &AppHandle, project_id: i64) -> Result<(), FlowError> {
    let agents = match app.state::<AgentRegistry>().team() {
        Ok(team) => team.ids().cloned().collect(),
        Err(_) => vec![Agent::ember(), Agent::axiom(), Agent::vexa()],
    };
    for agent in agents {
        emit_state(app, project_id, agent, AgentState::Idle)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/flow_tests.rs"]
mod tests;
