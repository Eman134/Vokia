use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::PoisonError;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::agents::replica_of;
use crate::attachments;
use crate::agents::{Agent, AgentError, AgentProfile, AgentRegistry, Team};
use crate::claude::{LimitName, ProjectAccess, UsageLimit};
use crate::db::{
    DbError, DbState, FlowEdge, OrchestrationSnapshot, FLOW_END, GATEWAY_PARALLEL, LINK_FLOW,
};
use crate::events::{AgentState, DecisionKind, Speaker, VokiaEvent};
use crate::mcp::{McpState, SlotDecision};
use crate::orchestration::counters::TaskCounters;
use crate::orchestration::instances;
use crate::orchestration::stages::{self, StallAction};
use crate::orchestration::policy::{
    self, placed_participants, Passage, Verdict, HUMAN_GATE_END, HUMAN_GATE_NEVER,
    REVIEW_BATCH_TITLE, REVIEW_TASK_TITLE,
};
use crate::orchestration::worktree_policy::{leader_limit, should_use};
use crate::projects::worktree;
use crate::roles::RoleRegistry;
use crate::session_key::SessionKey;
use crate::state::{
    ActiveDelegation, AppState, Batch, PendingDecision, QueuedJob, RequestContext, WaitingDelegation,
    DEFAULT_DELEGATION_PING_SECS, MAX_DELEGATION_PING_SECS, MAX_WATCHDOG_RETRIES, MIN_DELEGATION_PING_SECS,
};
use crate::task::{
    fast_definition, long_request_text, needs_llm_interpreter, parse_definition, ActiveTask, TaskDefinition,
    TaskKind, TaskOutcome, Variable, CONVERSATION_VAR,
};

use super::agent_session::ask_step;
use super::archive::archive_task;
use super::emit::{
    emit, emit_artifacts_changed, emit_backlog_changed, emit_feed_note, emit_message, emit_state,
    now_millis, reconcile_waiting, report_limits, MessageScope,
};
use super::error::{FlowError, RequestOutcome};
use super::project_context::{
    load_project, project_context, project_context_without_tree, project_dir, project_dir_of, project_id_from_choice,
    scan_structure,
};
use super::prompts::NO_NAMES;
use super::prompts::{channel_note, interpreter_role, StepPrompts};
use super::runs::{
    auto_pause_on_limit, begin_run, end_run, interrupt_paused_run, is_agent_paused, pause_run,
    RESUME_NOTE,
};
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
    // Nova mensagem do usuário cancela imediatamente qualquer continuação automática em curso.
    if from == Speaker::User {
        state.continuations.cancel_agent(project_id, &recipient);
    }
    // Retomada não repete o pedido no feed: o pedido original já está gravado; só avisa quem continua.
    if context.resumed_run.is_some() {
        emit_message(
            app,
            project_id,
            Speaker::from(recipient.clone()),
            Speaker::User,
            RESUME_NOTE,
            MessageScope {
                channel_id: None,
                item_id: context.item_id.as_deref(),
            },
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
            MessageScope {
                channel_id: context.channel_id,
                item_id: context.item_id.as_deref(),
            },
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
    if context.resumed_run.is_none() && is_agent_paused(app, project_id, &key.agent) {
        return Ok(RequestOutcome::Paused);
    }
    if context.steer {
        if let Ok(true) = state.sessions.steer_active_turn(&key, request).await {
            return Ok(RequestOutcome::Steered);
        }
    }
    if (key.agent != Agent::ember() || context.resumed_run.is_some())
        && !start_or_enqueue(app, state, key.clone(), request, context.clone())?
    {
        return Ok(RequestOutcome::Queued);
    }
    let item_id = context.item_id.clone();
    let result = run_flow(app, state, cli, key.clone(), request, context, &team).await;
    if result.is_err() {
        abandon_flow(app, state, &key, item_id.as_deref());
    }
    result
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
        let result = answer_validation(app, state, &decision, answer).await;
        reconcile_waiting(app, state, decision.project_id);
        return result;
    }
    let text = format!(
        "Decisão \"{}\" (sobre o pedido: {}): {answer}",
        decision.title, decision.request
    );
    let (project_id, item_id) = match chosen_project(app, &decision, answer) {
        Ok(Some(chosen)) if chosen != decision.project_id => (chosen, None),
        Ok(_) => (decision.project_id, decision.item_id),
        Err(error) => {
            reconcile_waiting(app, state, decision.project_id);
            return Err(error);
        }
    };
    if project_id != decision.project_id {
        reconcile_waiting(app, state, decision.project_id);
    }
    let context = RequestContext::new(None, item_id, None);
    let result = handle_request(app, state, &text, decision.agent, project_id, context).await;
    if !matches!(result, Ok(RequestOutcome::Finished)) {
        reconcile_waiting(app, state, project_id);
    }
    result.map(|_| ())
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

// Delegação do orquestrador: o líder roda a cadeia inteira no próprio fluxo (é ele quem espera os ajudantes),
// enquanto quem delegou fica livre. Mesmas regras de instância, teto e espera de `dispatch_request`.
#[allow(clippy::too_many_arguments)]
pub(super) fn dispatch_flow(
    app: &AppHandle,
    state: &AppState,
    project_id: i64,
    base: &Agent,
    request: &str,
    context: RequestContext,
    max_instances: i64,
    max_parallel: i64,
) -> Result<(), FlowError> {
    start_dispatch(
        app,
        state,
        project_id,
        base,
        request,
        context,
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
    if is_agent_paused(app, project_id, base) {
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
        let key = key_of(base);
        let pending = flow.pending.entry(key.clone()).or_default();
        push_pending_job(app, pending, &key, request.to_string(), parallel);
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
    flow.active_delegations.insert(
        (project_id, agent.clone(), parallel.item_id.clone()),
        new_delegation(project_id, &agent, &parallel),
    );
    drop(flow);
    spawn_flow(app, key, request.to_string(), parallel);
    Ok(())
}

fn new_delegation(project_id: i64, to_agent: &Agent, context: &RequestContext) -> ActiveDelegation {
    ActiveDelegation {
        project_id,
        to_agent: to_agent.clone(),
        item_id: context.item_id.clone(),
        created_at: Instant::now(),
        ping_interval: Duration::from_secs(
            context
                .ping_seconds
                .unwrap_or(DEFAULT_DELEGATION_PING_SECS)
                .clamp(MIN_DELEGATION_PING_SECS, MAX_DELEGATION_PING_SECS),
        ),
        activated: false,
        retries: 0,
    }
}

// Abre o lote de um turno do líder; o id devolvido vai nas trocas paralelas.
pub(super) fn open_batch(state: &AppState, batch: Batch) -> Result<u64, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    let id = flow.next_batch;
    flow.next_batch += 1;
    flow.batches.insert(id, batch);
    Ok(id)
}

pub(super) fn is_batch_open(state: &AppState, id: u64) -> bool {
    state
        .flow
        .lock()
        .is_ok_and(|flow| flow.batches.contains_key(&id))
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
        if let Err(error) = run_flow(&app, &state, &cli, key.clone(), &request, context, &team).await {
            let _ = report_failure(&app, project_id, &error);
            abandon_flow(&app, &state, &key, item_id.as_deref());
        }
    });
}

// O fluxo não chegou a rodar: solta a reserva da sessão e o ramo do card, para não prender open_branches nem pending.
fn abandon_flow(app: &AppHandle, state: &AppState, key: &SessionKey, item_id: Option<&str>) {
    if let Ok(mut flow) = state.flow.lock() {
        flow.pending.remove(key);
        flow.running.remove(key);
        flow.active_delegations
            .remove(&(key.project_id, key.agent.clone(), item_id.map(str::to_string)));
    }
    if let Some(db) = app.state::<DbState>().db() {
        let _ = db.clear_flow_queue_for_session(key.project_id, &key.agent);
    }
    if let Some(item) = item_id {
        close_branch(state, key.project_id, item, true);
    }
    reconcile_waiting(app, state, key.project_id);
    drain_waiting(app, state, key.project_id);
}

fn push_pending_job(
    app: &AppHandle,
    pending: &mut VecDeque<QueuedJob>,
    key: &SessionKey,
    text: String,
    context: RequestContext,
) {
    let db_id = app
        .state::<DbState>()
        .db()
        .and_then(|db| db.enqueue_flow_job(key.project_id, &key.agent, &text, &context).ok());
    pending.push_back(QueuedJob {
        db_id,
        text,
        context,
    });
}

fn start_or_enqueue(
    app: &AppHandle,
    state: &AppState,
    key: SessionKey,
    request: &str,
    context: RequestContext,
) -> Result<bool, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    if let Some(pending) = flow.pending.get_mut(&key) {
        if context.steer {
            let db_id = app
                .state::<DbState>()
                .db()
                .and_then(|db| db.enqueue_flow_job(key.project_id, &key.agent, request, &context).ok());
            pending.push_front(QueuedJob {
                db_id,
                text: request.to_string(),
                context,
            });
        } else {
            push_pending_job(app, pending, &key, request.to_string(), context);
        }
        return Ok(false);
    }
    flow.pending.insert(key, VecDeque::new());
    Ok(true)
}

fn next_job_with_db(
    db: Option<&crate::db::Db>,
    state: &AppState,
    key: SessionKey,
) -> Result<Option<QueuedJob>, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    let next = flow.pending.get_mut(&key).and_then(VecDeque::pop_front);
    if let Some(ref job) = next {
        if let Some(db_id) = job.db_id {
            if let Some(db) = db {
                let _ = db.remove_flow_job(db_id);
            }
        }
    } else {
        flow.pending.remove(&key);
        flow.running.remove(&key);
        flow.active_delegations
            .retain(|(project_id, agent, _), _| !(*project_id == key.project_id && agent == &key.agent));
    }
    Ok(next)
}

#[cfg(test)]
fn next_job(state: &AppState, key: SessionKey) -> Result<Option<QueuedJob>, FlowError> {
    next_job_with_db(None, state, key)
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

// Começar a executar um card ou etapa o põe em In progress com as etiquetas de andamento; o quadro é avisado na hora.
fn start_item_work(app: &AppHandle, project_id: i64, item_id: Option<&str>) -> Result<(), FlowError> {
    let Some(item_id) = item_id else {
        return Ok(());
    };
    let state = app.state::<DbState>();
    let db = state.require().map_err(|source| FlowError::CardStart {
        item_id: item_id.to_string(),
        source,
    })?;
    db.start_card(project_id, item_id).map_err(|source| FlowError::CardStart {
        item_id: item_id.to_string(),
        source,
    })?;
    emit_backlog_changed(app, project_id)
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
    let mut reply_to = context.reply_to;
    let mut outcome: Result<(), FlowError> = Ok(());
    // Só o primeiro exchange é o pedido de quem chamou; os seguintes saem da fila interna.
    let mut first: Option<RequestOutcome> = None;
    let mut current_run: Option<u64> = None;
    loop {
        start_item_work(app, key.project_id, item_id.as_deref())?;
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
                reply_to.as_ref(),
            )
            .await
        };
        let stopped = match &exchange {
            Ok(TaskOutcome::Finished) => None,
            Ok(_) => Some("a troca terminou sem concluir".to_string()),
            Err(error) => Some(error.to_string()),
        };
        let auto_pause = auto_pause_on_limit(app);
        let (this, limit, paused_mid_flow) = match exchange {
            Ok(TaskOutcome::Finished) => (RequestOutcome::Finished, None, false),
            Ok(TaskOutcome::Paused) => (RequestOutcome::Paused, None, true),
            Ok(_) => (RequestOutcome::Failed, None, false),
            Err(error) if error.is_limit() => {
                if auto_pause {
                    let limit = error.usage_limit().unwrap_or(UsageLimit {
                        window: LimitName::FiveHour,
                        resets_at: 0,
                    });
                    (RequestOutcome::Paused, Some(limit), false)
                } else {
                    outcome = outcome.and(set_agents_idle(app, key.project_id));
                    let limit_info = error.usage_limit();
                    let _ = emit(
                        app,
                        VokiaEvent::UsageLimitFailed {
                            agent: key.agent.clone(),
                            project_id: key.project_id,
                            window: limit_info.map(|l| l.window),
                            resets_at: limit_info.map(|l| l.resets_at).filter(|&r| r > 0),
                            message: error.to_string(),
                            channel_id,
                            item_id: item_id.clone(),
                        },
                    );
                    (RequestOutcome::Failed, None, false)
                }
            }
            Err(error) => {
                outcome = outcome.and(report_failure(app, key.project_id, &error));
                (RequestOutcome::Failed, None, false)
            }
        };
        let is_limit_pause = limit.is_some();
        if let Some(limit) = limit {
            pause_run(app, &key, journal, limit, item_id.as_deref())?;
        } else if paused_mid_flow {
            interrupt_paused_run(app, &key, journal, item_id.as_deref())?;
        } else {
            end_run(app, journal)?;
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
        match next_job_with_db(app.state::<DbState>().db(), state, key.clone())? {
            Some(job) => {
                request = job.text;
                current_run = job.context.run_id;
                item_id = job.context.item_id;
                channel_id = job.context.channel_id;
                parallel = job.context.parallel;
                batch = job.context.batch;
                resumed_run = job.context.resumed_run;
                reply_to = job.context.reply_to;
            }
            None => {
                if !is_limit_pause {
                    reconcile_waiting(app, state, key.project_id);
                }
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
    reply_to: Option<&Agent>,
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
        reply_to,
    )
    .await;
    sessions.release().await;
    result
}

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
    let root = worktree::project_worktrees_dir(
        &app.path().app_data_dir().map_err(FlowError::ArchivePath)?,
        project_id,
    );
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
    emit_state(app, key.project_id, key.agent.clone(), AgentState::Sending, item_id)?;
    if let Some(id) = batch {
        with_batch(state, id, |open| open.started.push(key.agent.clone()));
        announce_batch(app, state, id);
    }
    let project = load_project(app, key.project_id)?;
    let project_dir = project_dir_of(&project)?;
    let attachments_dir = attachments_dir(app, key.project_id)?;
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
            extra_dirs: &[],
            level: project.permission_level,
            builtin_tools: true,
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
        reply_to: None,
    };
    let mut sessions = ExchangeSessions::per_step(&state.sessions, key.project_id);
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
    let card = exchange.card_context();
    let prompts = with_cache(exchange.state, project_id, |cache| {
        Ok(StepPrompts::build(
            cache,
            exchange.definition,
            &agent,
            exchange.request,
            tool_mode,
            card.as_deref(),
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
                    exchange.scope(),
                    &error.to_string(),
                );
            }
            return Err(error);
        }
    };
    super::turns::record_turn_work_steps(
        exchange,
        &agent,
        reply.activity.as_ref(),
        reply.usage.as_ref().and_then(|u| u.provider.as_deref()),
    );
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
            exchange.scope(),
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
    let mut delegations: Vec<(Agent, String)> = Vec::new();
    if reply.outcome.delegations.is_empty() {
        delegations.extend(read.delegation);
    } else {
        for delegation in reply.outcome.delegations {
            if delegation.intent.notify {
                emit_message(
                    exchange.app,
                    project_id,
                    Speaker::from(agent.clone()),
                    Speaker::from(delegation.agent),
                    &delegation.request,
                    exchange.scope(),
                    None,
                    None,
                    None,
                )?;
            } else {
                delegations.push((delegation.agent, delegation.request));
            }
        }
    }
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
                exchange.scope(),
                reply.activity.take(),
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
    } else {
        if !text.is_empty() {
            let _ = emit_message(
                exchange.app,
                project_id,
                Speaker::from(agent.clone()),
                Speaker::User,
                &text,
                exchange.scope(),
                reply.activity.take(),
                None,
                reply.usage.take(),
            );
        }
        if let Some(item_id) = exchange.item_id {
            let _ = mark_flow_end(exchange.state, project_id, item_id, &agent);
        }
    }
    emit_state(
        exchange.app,
        project_id,
        agent,
        AgentState::Idle,
        exchange.item_id,
    )?;
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
        kind: TaskKind::Demanda,
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

pub(crate) fn spawn_delegation_watchdog(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(MIN_DELEGATION_PING_SECS));
        loop {
            ticker.tick().await;
            sweep_silent_delegations(&app);
            sweep_stalled_stages(&app);
        }
    });
}

// Task parada: a etapa ficou sem movimento além do intervalo de ping, sem delegação pendente nem fluxo rodando no projeto.
// Reenvia ao nó da etapa enquanto houver tentativas; esgotadas, sobe ao líder do time desse nó.
// Bem acima do ping de delegação: cada reenvio reabre o contexto inteiro do agente, então só vale para etapa parada de fato.
const STALL_AFTER_SECS: i64 = 180;
const STALL_AFTER_MS: i64 = STALL_AFTER_SECS * 1000;
const MAX_STAGE_RESENDS: i64 = MAX_WATCHDOG_RETRIES as i64;
const STALL_RESEND_TEXT: &str = "A task {item} parou na sua etapa, sem movimento. Retome o trabalho desta etapa de onde ele parou.";
const STALL_ESCALATE_TEXT: &str = "A task {item} está parada na etapa de {agent}, sem movimento depois de {retries} reenvios. Decida como seguir: retome com quem tem o contexto, reenvie ao nó certo ou conclua a task.";
const STALL_NO_LEADER_TEXT: &str = "A task {item} parou na etapa de {agent} e não há líder para retomá-la.";

fn sweep_stalled_stages(app: &AppHandle) {
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return;
    };
    let Ok(stalled) = db.stalled_task_stages(STALL_AFTER_MS) else {
        return;
    };
    let state = app.state::<AppState>();
    for stage in stalled {
        let Some(agent) = Agent::from_stored(&stage.agent) else {
            continue;
        };
        if stage_is_moving(&state, stage.project_id, &stage.item_id, &agent) {
            continue;
        }
        let item = stage.item_id.as_str();
        match stages::stall_action(stage.retries, MAX_STAGE_RESENDS) {
            StallAction::Resend => {
                if db.bump_task_stage_retry(stage.project_id, item).is_err() {
                    continue;
                }
                let text = STALL_RESEND_TEXT.replace("{item}", item);
                spawn_stage_request(app, stage.project_id, item, agent, &text);
                let _ = emit_stage(app, stage.project_id, item);
            }
            StallAction::Escalate => {
                if set_stage_state(app, stage.project_id, item, stages::STAGE_ESCALATED).is_err() {
                    continue;
                }
                let leader = load_orchestration(app)
                    .ok()
                    .flatten()
                    .and_then(|snapshot| stage_leader(&snapshot, &agent));
                match leader {
                    Some(leader) if leader != agent => {
                        let text = STALL_ESCALATE_TEXT
                            .replace("{item}", item)
                            .replace("{agent}", agent.as_str())
                            .replace("{retries}", &stage.retries.to_string());
                        spawn_stage_request(app, stage.project_id, item, leader, &text);
                    }
                    _ => {
                        let message = STALL_NO_LEADER_TEXT
                            .replace("{item}", item)
                            .replace("{agent}", agent.as_str());
                        record_on_card(app, stage.project_id, item, &message);
                        let _ = emit(app, VokiaEvent::Error { message });
                    }
                }
            }
        }
    }
}

// Líder do time em que o agente está posicionado.
fn stage_leader(snapshot: &OrchestrationSnapshot, agent: &Agent) -> Option<Agent> {
    let placement = snapshot
        .placements
        .iter()
        .find(|placement| placement.agent_id == agent.as_str())?;
    let team = snapshot
        .teams
        .iter()
        .find(|team| team.id == placement.team_id)?;
    Agent::from_stored(team.leader_agent.as_deref()?)
}

// Só o próprio card, o agente da etapa (ou réplica dele) e as delegações do card seguram a etapa: outra task
// rodando no mesmo projeto não pode impedir que a parada seja vista.
fn stage_is_moving(state: &AppState, project_id: i64, item_id: &str, agent: &Agent) -> bool {
    if has_pending_delegation(state, project_id, item_id) {
        return true;
    }
    let Ok(flow) = state.flow.lock() else {
        return true;
    };
    let base = base_agent(agent);
    let item_running = flow
        .running
        .iter()
        .any(|(key, running)| key.project_id == project_id && running == item_id);
    let agent_busy = flow
        .pending
        .keys()
        .any(|key| key.project_id == project_id && base_agent(&key.agent) == base);
    item_running || agent_busy
}

fn has_pending_delegation(state: &AppState, project_id: i64, item_id: &str) -> bool {
    let Ok(flow) = state.flow.lock() else {
        return true;
    };
    let active = flow
        .active_delegations
        .keys()
        .any(|(project, _, item)| *project == project_id && item.as_deref() == Some(item_id));
    let waiting = flow.waiting.get(&project_id).is_some_and(|queue| {
        queue
            .iter()
            .any(|entry| entry.context.item_id.as_deref() == Some(item_id))
    });
    active || waiting
}

// Pedido de retomada ao agente (ou ao líder): entra pelo mesmo caminho de um pedido novo, preso ao card.
pub(super) fn spawn_stage_request(app: &AppHandle, project_id: i64, item_id: &str, recipient: Agent, text: &str) {
    let app = app.clone();
    let text = text.to_string();
    let item_id = item_id.to_string();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let context = RequestContext::new(None, Some(item_id), None);
        let _ = handle_request(&app, &state, &text, recipient, project_id, context).await;
    });
}

// Card que foi para Done: limpa o worktree e a branch dele em segundo plano. Falha ou motivo de manter viram
// registro na atividade do card; a conclusão do card não espera nem depende disso.
pub(crate) fn schedule_done_cleanup(app: &AppHandle, project_id: i64, item_id: &str) {
    super::memory_review::schedule_memory_review(
        app,
        project_id,
        super::memory_review::ReviewTrigger::CardDone,
        None,
        Some(item_id.to_string()),
        None,
    );
    let app = app.clone();
    let item = item_id.to_string();
    tauri::async_runtime::spawn(async move {
        let outcome = done_cleanup(&app, project_id, &item).await;
        let text = match outcome {
            Ok(worktree::CleanupOutcome::Absent) => return,
            Ok(worktree::CleanupOutcome::Removed) => format!(
                "Limpei o worktree e a branch de {item}: estavam integrados à main e sem alterações."
            ),
            Ok(worktree::CleanupOutcome::Kept(reason)) => {
                format!("Mantive o worktree de {item}: {reason}.")
            }
            Err(error) => format!("Não consegui limpar o worktree de {item}: {error}."),
        };
        record_on_card(&app, project_id, &item, &text);
    });
}

// Registro no card que precisa sobreviver ao app fechado; card inexistente ou sem banco não vira erro de quem chama.
fn record_on_card(app: &AppHandle, project_id: i64, item_id: &str, text: &str) {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return;
    };
    if db.add_card_activity(project_id, item_id, text).is_ok() {
        let _ = emit_backlog_changed(app, project_id);
    }
}

async fn done_cleanup(
    app: &AppHandle,
    project_id: i64,
    item_id: &str,
) -> Result<worktree::CleanupOutcome, FlowError> {
    let project = load_project(app, project_id)?;
    let root = worktree::project_worktrees_dir(
        &app.path().app_data_dir().map_err(FlowError::ArchivePath)?,
        project_id,
    );
    let safe_dir_name: String = item_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let worktree_path = root.join(safe_dir_name);
    app.state::<AppState>()
        .sessions
        .release_worktree(project_id, item_id, Some(&worktree_path))
        .await;
    worktree::cleanup_done(Path::new(&project.path), &root, item_id)
        .await
        .map_err(FlowError::Worktree)
}

// Leva a etapa atual da task à tela e aos eventos; sem banco, a etapa simplesmente não sai.
pub(crate) fn emit_stage(app: &AppHandle, project_id: i64, item_id: &str) -> Result<(), FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(());
    };
    match db.task_stage(project_id, item_id) {
        Ok(Some(stage)) => emit(app, VokiaEvent::TaskStage { stage }),
        _ => Ok(()),
    }
}

// Único caminho para mudar a etapa de uma task: o banco grava etapa, card filho, pai e artefato numa transação;
// aqui a tela recebe a etapa, o quadro e os artefatos já atualizados.
pub(crate) fn set_stage_state(
    app: &AppHandle,
    project_id: i64,
    item_id: &str,
    stage_state: &str,
) -> Result<(), FlowError> {
    app.state::<DbState>()
        .require()?
        .set_task_stage_state(project_id, item_id, stage_state)?;
    publish_stage(app, project_id, item_id)
}

// Etapa gravada que mudou: a task, o quadro e os artefatos são avisados juntos.
pub(crate) fn publish_stage(app: &AppHandle, project_id: i64, item_id: &str) -> Result<(), FlowError> {
    emit_stage(app, project_id, item_id)?;
    emit_backlog_changed(app, project_id)?;
    emit_artifacts_changed(app, project_id)
}

// Delegação ativada já cumpriu o papel do registro e sai; a silenciosa recebe novas tentativas de drenar a fila
// e, esgotadas, vira erro para o usuário.
fn sweep_silent_delegations(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (retry_projects, gave_up) = {
        let Ok(mut flow) = state.flow.lock() else {
            return;
        };
        flow.active_delegations.retain(|_, delegation| !delegation.activated);
        let mut retry_projects = HashSet::new();
        let mut gave_up = Vec::new();
        for (key, delegation) in flow.active_delegations.iter_mut() {
            if delegation.created_at.elapsed() < delegation.ping_interval {
                continue;
            }
            if delegation.retries < MAX_WATCHDOG_RETRIES {
                delegation.retries += 1;
                delegation.created_at = Instant::now();
                retry_projects.insert(delegation.project_id);
            } else {
                gave_up.push(key.clone());
            }
        }
        let gave_up: Vec<ActiveDelegation> = gave_up
            .iter()
            .filter_map(|key| flow.active_delegations.remove(key))
            .collect();
        (retry_projects, gave_up)
    };
    for project_id in retry_projects {
        drain_waiting(app, &state, project_id);
    }
    for delegation in gave_up {
        let target = delegation
            .item_id
            .as_ref()
            .map_or_else(String::new, |item| format!(" do card {item}"));
        let message = format!(
            "O agente {} não respondeu à delegação{target}.",
            delegation.to_agent.as_str()
        );
        if let Some(item) = &delegation.item_id {
            record_on_card(app, delegation.project_id, item, &message);
        }
        let _ = emit(app, VokiaEvent::Error { message });
    }
}

pub(crate) fn drain_waiting_for_project(app: &AppHandle, state: &AppState, project_id: i64) {
    drain_waiting(app, state, project_id);
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
        if is_agent_paused(app, project_id, &waiting.base) {
            if let Ok(mut flow) = state.flow.lock() {
                flow.waiting.entry(project_id).or_default().push_back(waiting);
            }
            continue;
        }
        let max_instances = snapshot
            .as_ref()
            .and_then(|snapshot| placement_of(snapshot, &waiting.base))
            .map_or(1, |placement| placement.max_instances);
        let item_id = waiting.context.item_id.clone();
        let batch_id = waiting.context.batch;
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
            if let Some(item) = item_id.as_deref() {
                close_branch(state, project_id, item, true);
            }
            if let Some(batch) = batch_id {
                seal_batch(app, state, batch);
            }
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
    reply_to: Option<&Agent>,
) -> Result<TaskOutcome, FlowError> {
    emit_state(app, key.project_id, key.agent.clone(), AgentState::Sending, item_id)?;
    let channel = find_channel(app, key.project_id, channel_id, team);
    let request = with_channel_note(request, channel.as_ref());
    let request = request.as_str();
    let project = load_project(app, key.project_id)?;
    let project_dir = project_dir_of(&project)?;
    let attachments_dir = attachments_dir(app, key.project_id)?;
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
                    MessageScope {
                        channel_id: channel.as_ref().map(|found| found.id),
                        item_id,
                    },
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
    // Pedido de um card, ou com participantes e regras de tarefa depois do posicionamento, é trabalho de verdade.
    if item_id.is_some() || definition.has_task_structure() {
        definition.promote_to_demand();
    }
    // Ember só orquestra: sem agente técnico na troca, nem lê a pasta. Sem cache: ela é relida a cada exchange.
    let ember_only = key.agent == Agent::ember()
        && definition.participants.iter().all(|participant| *participant == key.agent);
    let context = if ember_only {
        project_context_without_tree(&project)
    } else {
        let tree = scan_structure(&project_dir).await;
        project_context(&project, tree.as_ref())
    };
    if first_speaker(&definition, &key.agent) != key.agent {
        emit_state(app, key.project_id, key.agent.clone(), AgentState::Idle, item_id)?;
    }
    begin_task(app, state, key.project_id, &definition, request)?;
    let mut transcript = vec![format!("{USER_LABEL}: {request}")];
    let access = ProjectAccess {
        dir: &working,
        attachments: &attachments_dir,
        extra_dirs: &[],
        level: project.permission_level,
        builtin_tools: true,
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
        reply_to,
    };
    let result = run_turns(&exchange, sessions, &mut transcript).await;
    let outcome = match &result {
        Ok(outcome) => *outcome,
        Err(error) if error.is_limit() && auto_pause_on_limit(app) => TaskOutcome::Paused,
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
    let (variable_names, has_running_task) = with_cache(state, key.project_id, |cache| {
        let names: Vec<String> = cache
            .names()
            .into_iter()
            .filter(|name| name != CONVERSATION_VAR)
            .collect();
        Ok((names, cache.has_running_task()))
    })?;
    // Pedido comum não gasta uma chamada ao modelo só para virar definição: o interpretador fica para revezamento e variáveis.
    if !needs_llm_interpreter(request, !variable_names.is_empty()) {
        return Ok(fast_definition(&origin, request, team, !has_running_task));
    }
    let profile = profile_of(team, &origin)?;
    let names = if variable_names.is_empty() {
        NO_NAMES.to_string()
    } else {
        variable_names.join(", ")
    };
    let message = format!(
        "{} Destinatário: {}. Variáveis existentes: {names}. Pedido: {request}",
        team.roster_line(),
        team.name_of(&origin)
    );
    let registered_role = app
        .state::<RoleRegistry>()
        .find(&profile.role)
        .map_err(FlowError::Roles)?;
    let reply = super::fallback::ask_with_fallback(
        app,
        state,
        cli,
        &origin,
        profile,
        &message,
        &interpreter_role(team, orchestration, registered_role.as_ref()),
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
    let mut flow = state
        .flow
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    flow.agent_states
        .retain(|(pid, _), _| *pid != project_id);
    flow.turn_waiters
        .retain(|key, _| key.project_id != project_id);
    state.console.forget_project(project_id);
    state.turn_plans.forget_project(project_id);
    state.continuations.cancel_project(project_id);
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
        emit_state(app, project_id, agent, AgentState::Idle, None)?;
    }
    Ok(())
}

pub(crate) async fn restore_flow_queue(app: AppHandle) {
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return;
    };
    let Ok(jobs) = db.list_flow_queue() else {
        return;
    };
    if jobs.is_empty() {
        return;
    }
    let mut grouped: HashMap<SessionKey, Vec<crate::db::PersistedFlowJob>> = HashMap::new();
    for job in jobs {
        let key = SessionKey {
            project_id: job.project_id,
            agent: job.agent.clone(),
        };
        grouped.entry(key).or_default().push(job);
    }
    for (key, mut session_jobs) in grouped {
        let head = session_jobs.remove(0);
        let head_id = head.id;
        let head_text = head.text;
        let mut head_context = head.context;
        head_context.batch = None;
        let state = app.state::<AppState>();
        let mut head_to_run: Option<(String, RequestContext)> = None;
        {
            let Ok(mut flow) = state.flow.lock() else {
                continue;
            };
            if let Some(pending) = flow.pending.get_mut(&key) {
                pending.push_back(QueuedJob {
                    db_id: Some(head_id),
                    text: head_text,
                    context: head_context,
                });
                for mut job in session_jobs {
                    job.context.batch = None;
                    pending.push_back(QueuedJob {
                        db_id: Some(job.id),
                        text: job.text,
                        context: job.context,
                    });
                }
            } else {
                let mut deque = VecDeque::new();
                for mut job in session_jobs {
                    job.context.batch = None;
                    deque.push_back(QueuedJob {
                        db_id: Some(job.id),
                        text: job.text,
                        context: job.context,
                    });
                }
                flow.pending.insert(key.clone(), deque);
                head_to_run = Some((head_text, head_context));
            }
        }
        let Some((head_text, head_context)) = head_to_run else {
            continue;
        };
        let _ = db.remove_flow_job(head_id);
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let project_id = key.project_id;
            let item_id = head_context.item_id.clone();
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
            if let Err(error) =
                run_flow(&app, &state, &cli, key.clone(), &head_text, head_context, &team).await
            {
                let _ = report_failure(&app, project_id, &error);
                abandon_flow(&app, &state, &key, item_id.as_deref());
            }
        });
    }
}

#[cfg(test)]
#[path = "../../tests/flow_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/delegation_watchdog_tests.rs"]
mod watchdog_tests;
