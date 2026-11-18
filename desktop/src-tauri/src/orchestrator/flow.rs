use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use tauri::{AppHandle, Manager};

use crate::agents::replica_of;
use crate::agents::{Agent, AgentError, AgentProfile, AgentRegistry, Team};
use crate::claude::{self, ProjectAccess};
use crate::db::{DbState, OrchestrationSnapshot};
use crate::events::{AgentState, DecisionKind, Speaker, VokiaEvent};
use crate::mcp::{McpState, SlotDecision};
use crate::orchestration::counters::TaskCounters;
use crate::orchestration::instances;
use crate::orchestration::policy::{
    self, placed_participants, Verdict, HUMAN_GATE_END, HUMAN_GATE_NEVER, REVIEW_BATCH_TITLE,
    REVIEW_TASK_TITLE,
};
use crate::orchestration::worktree_policy::should_use;
use crate::projects::worktree;
use crate::session_key::SessionKey;
use crate::state::{AppState, Batch, QueuedJob, RequestContext, WaitingDelegation};
use crate::task::{
    long_request_text, parse_definition, ActiveTask, TaskDefinition, TaskOutcome, Variable,
};

use super::agent_session::ask_step;
use super::archive::archive_task;
use super::emit::{emit, emit_backlog_changed, emit_message, emit_state, report_limits};
use super::error::{FlowError, RequestOutcome};
use super::project_context::{load_project, project_context, project_dir, scan_structure};
use super::prompts::NO_NAMES;
use super::prompts::{channel_note, interpreter_role, StepPrompts};
use super::runs::{begin_run, end_run};
use super::session_pool::ExchangeSessions;
use super::text_fallback::{read_reply, TextContext};
use super::turns::{
    first_speaker, open_decision, record_passage, run_turns, with_cache, Exchange, USER_LABEL,
};
use crate::orchestration::snapshot::{placement_of, ACTION_RETURN};
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
    emit_message(
        app,
        project_id,
        Speaker::User,
        Speaker::from(recipient.clone()),
        request,
        context.channel_id,
        None,
        None,
    )?;
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
    let decision = state
        .decisions
        .lock()
        .map_err(|_| FlowError::DecisionsPoisoned)?
        .remove(&id)
        .ok_or(FlowError::UnknownDecision)?;
    let text = format!(
        "Decisão \"{}\" (sobre o pedido: {}): {answer}",
        decision.title, decision.request
    );
    let context = RequestContext::new(None, decision.item_id, None);
    handle_request(
        app,
        state,
        &text,
        decision.agent,
        decision.project_id,
        context,
    )
    .await
    .map(|_| ())
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
    let key_of = |agent: &Agent| SessionKey {
        project_id,
        agent: agent.clone(),
    };
    let parallel = RequestContext {
        parallel: true,
        ..context
    };
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    // Conta a troca no lote antes de qualquer caminho (inicia, fila ou espera), para a junção não sair cedo.
    if let Some(batch) = parallel.batch {
        if let Some(open) = flow.batches.get_mut(&batch) {
            open.pending += 1;
            open.dispatched.push(base.clone());
        }
    }
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
    )?;
    let snapshot = load_orchestration(app, batch.project_id)?;
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

fn spawn_flow(app: &AppHandle, key: SessionKey, request: String, context: RequestContext) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let project_id = key.project_id;
        let state = app.state::<AppState>();
        let cli = match state.cli.as_ref() {
            Ok(cli) => cli.clone(),
            Err(problem) => {
                let _ = stop_with_error(&app, project_id, problem.reason.clone());
                return;
            }
        };
        let team = match app.state::<AgentRegistry>().team() {
            Ok(team) => team,
            Err(error) => {
                let _ = report_failure(&app, project_id, &FlowError::Team(error));
                return;
            }
        };
        if let Err(error) = run_flow(&app, &state, &cli, key, &request, context, &team).await {
            let _ = report_failure(&app, project_id, &error);
        }
    });
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
    pub(crate) agent: Agent,
    pub(crate) item_id: Option<String>,
}

// Fluxos em andamento do projeto; réplicas são as que têm sufixo -<n> no id.
pub(crate) fn active_instances(
    state: &AppState,
    project_id: i64,
) -> Result<Vec<ActiveInstance>, FlowError> {
    let flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    Ok(flow
        .pending
        .keys()
        .filter(|key| key.project_id == project_id)
        .map(|key| ActiveInstance {
            agent: key.agent.clone(),
            item_id: flow.running.get(key).cloned(),
        })
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
    let mut outcome: Result<(), FlowError> = Ok(());
    // Só o primeiro exchange é o pedido de quem chamou; os seguintes saem da fila interna.
    let mut first: Option<RequestOutcome> = None;
    let mut current_run: Option<u64> = None;
    loop {
        set_running_item(state, &key, item_id.as_deref())?;
        let journal_run = if parallel {
            current_run.or(parallel_run)
        } else {
            current_run
        };
        let journal = begin_run(app, &key, &request, item_id.as_deref(), journal_run)?;
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
        let this = match exchange {
            Ok(TaskOutcome::Finished) => RequestOutcome::Finished,
            Ok(_) => RequestOutcome::Failed,
            Err(error) => {
                outcome = outcome.and(report_failure(app, key.project_id, &error));
                RequestOutcome::Failed
            }
        };
        end_run(app, journal)?;
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
        match next_job(state, key.clone())? {
            Some(job) => {
                request = job.text;
                current_run = job.context.run_id;
                item_id = job.context.item_id;
                channel_id = job.context.channel_id;
                parallel = job.context.parallel;
                batch = job.context.batch;
            }
            None => {
                drain_waiting(app, state, key.project_id);
                return outcome.map(|()| first.unwrap_or(RequestOutcome::Failed));
            }
        }
    }
}

// As sessões do projeto ficam reservadas até o fim da mensagem; a ordem fixa evita impasse entre fluxos.
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
    let project = load_project(app, key.project_id)?;
    let tree = scan_structure(&project_dir).await;
    let context = project_context(&project, tree.as_ref());
    let orchestration = load_orchestration(app, key.project_id)?;
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
    let with_tools = exchange.app.state::<McpState>().server().is_some();
    let prompts = with_cache(exchange.state, project_id, |cache| {
        Ok(StepPrompts::build(
            cache,
            exchange.definition,
            &agent,
            exchange.request,
            with_tools,
        ))
    })?;
    let reply = ask_step(exchange, sessions, &agent, &prompts).await?;
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
        .parallel_delegations
        .into_iter()
        .chain(reply.outcome.delegation.or(read.delegation))
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
                    policy::check(snapshot, team, &base, &helper, &open.counters)
                })
            })
            .unwrap_or_else(|| {
                policy::check(snapshot, team, &base, &helper, &TaskCounters::default())
            });
        match verdict {
            Verdict::Deliver(passage) => {
                if let Some(id) = batch {
                    with_batch(exchange.state, id, |open| {
                        open.counters.register_passage(&base, &helper);
                        if passage.action == ACTION_RETURN {
                            open.counters.register_return(&base, &helper);
                        }
                    });
                }
                record_passage(
                    exchange.app,
                    exchange.run_id,
                    project_id,
                    exchange.item_id,
                    None,
                    &agent,
                    &helper,
                    &passage,
                )?;
                let max_instances = snapshot
                    .and_then(|snapshot| placement_of(snapshot, &helper))
                    .map_or(1, |placement| placement.max_instances);
                let mut context = RequestContext::new(
                    exchange.run_id,
                    exchange.item_id.map(str::to_string),
                    exchange.channel_id,
                );
                context.batch = batch;
                dispatch_request(
                    exchange.app,
                    exchange.state,
                    project_id,
                    &helper,
                    &request,
                    context,
                    max_instances,
                    max_parallel,
                )?;
                emit_message(
                    exchange.app,
                    project_id,
                    Speaker::from(agent.clone()),
                    Speaker::from(helper),
                    &request,
                    exchange.channel_id,
                    None,
                    None,
                )?;
            }
            Verdict::Refuse(motive) => {
                notes.push(format!("Delegação para {name} recusada: {motive}"));
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
            }
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

// Ao fim de um fluxo, as delegações que esperavam o teto entram enquanto houver vaga.
fn drain_waiting(app: &AppHandle, state: &AppState, project_id: i64) {
    let snapshot = match load_orchestration(app, project_id) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let _ = report_failure(app, project_id, &error);
            return;
        }
    };
    let max_parallel = snapshot
        .as_ref()
        .map_or(1, |snapshot| snapshot.settings.max_parallel);
    while has_room(state, project_id, max_parallel) {
        let Some(waiting) = take_waiting(state, project_id) else {
            return;
        };
        let max_instances = snapshot
            .as_ref()
            .and_then(|snapshot| placement_of(snapshot, &waiting.base))
            .map_or(1, |placement| placement.max_instances);
        if let Err(error) = dispatch_request(
            app,
            state,
            project_id,
            &waiting.base,
            &waiting.text,
            waiting.context,
            max_instances,
            max_parallel,
        ) {
            let _ = report_failure(app, project_id, &error);
            return;
        }
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
fn load_orchestration(
    app: &AppHandle,
    project_id: i64,
) -> Result<Option<OrchestrationSnapshot>, FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(None);
    };
    db.orchestration(project_id)
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
    let project = load_project(app, key.project_id)?;
    // Sem cache: a pasta é relida a cada exchange para refletir o estado atual.
    let tree = scan_structure(&project_dir).await;
    let context = project_context(&project, tree.as_ref());
    // Snapshot único da troca: edições feitas durante ela só valem na próxima.
    let orchestration = load_orchestration(app, key.project_id)?;
    let working = working_dir(
        app,
        key.project_id,
        item_id,
        orchestration.as_ref(),
        &project_dir,
    )
    .await?;
    let mut definition = interpret_task(
        app,
        state,
        cli,
        request,
        key.clone(),
        team,
        orchestration.as_ref(),
    )
    .await?;
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
    let reply = claude::ask(
        cli,
        &message,
        &interpreter_role(team, orchestration),
        &profile.model,
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
    let agents = match app.state::<AgentRegistry>().team() {
        Ok(team) => team.ids().cloned().collect(),
        Err(_) => vec![Agent::ember(), Agent::davi()],
    };
    for agent in agents {
        emit_state(app, project_id, agent, AgentState::Idle)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/flow_tests.rs"]
mod tests;
