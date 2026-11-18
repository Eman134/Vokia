use std::collections::VecDeque;
use std::path::Path;
use std::sync::PoisonError;

use tauri::{AppHandle, Manager};

use crate::agents::{Agent, AgentError, AgentProfile, AgentRegistry, Team};
use crate::claude::{self, ProjectAccess};
use crate::db::{DbState, OrchestrationSnapshot};
use crate::events::{AgentState, Speaker, VokiaEvent};
use crate::orchestration::instances;
use crate::orchestration::policy::placed_participants;
use crate::session_key::SessionKey;
use crate::state::{AppState, QueuedJob, RequestContext};
use crate::task::{long_request_text, parse_definition, ActiveTask, TaskDefinition, TaskOutcome};

use super::archive::archive_task;
use super::emit::{emit, emit_message, emit_state, report_limits};
use super::error::{FlowError, RequestOutcome};
use super::project_context::{load_project, project_context, project_dir, scan_structure};
use super::prompts::NO_NAMES;
use super::prompts::{channel_note, interpreter_role};
use super::session_pool::ExchangeSessions;
use super::turns::{first_speaker, run_turns, with_cache, Exchange, USER_LABEL};

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

pub(super) enum Dispatch {
    Started,
    Queued,
    Full,
}

// Pedido delegado em paralelo: uma instância ociosa do agente base, ou a fila do base quando todas estão ocupadas; no teto de max_parallel não inicia nada.
pub(super) fn dispatch_request(
    app: &AppHandle,
    state: &AppState,
    project_id: i64,
    base: &Agent,
    request: &str,
    context: RequestContext,
    max_instances: i64,
    max_parallel: i64,
) -> Result<Dispatch, FlowError> {
    let key_of = |agent: &Agent| SessionKey {
        project_id,
        agent: agent.clone(),
    };
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    let Some(agent) = instances::pick(base, max_instances, |agent| {
        flow.pending.contains_key(&key_of(agent))
    }) else {
        flow.pending
            .entry(key_of(base))
            .or_default()
            .push_back(QueuedJob {
                text: request.to_string(),
                context,
            });
        return Ok(Dispatch::Queued);
    };
    let running = flow
        .pending
        .keys()
        .filter(|key| key.project_id == project_id)
        .count() as i64;
    if running >= max_parallel {
        return Ok(Dispatch::Full);
    }
    let key = key_of(&agent);
    flow.pending.insert(key.clone(), VecDeque::new());
    drop(flow);
    spawn_flow(app, key, request.to_string(), context);
    Ok(Dispatch::Started)
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
    }
    Ok(next)
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
    let mut outcome: Result<(), FlowError> = Ok(());
    // Só o primeiro exchange é o pedido de quem chamou; os seguintes saem da fila interna.
    let mut first: Option<RequestOutcome> = None;
    let mut current_run: Option<u64> = None;
    loop {
        let exchange = run_exchange(
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
        .await;
        let this = match exchange {
            Ok(TaskOutcome::Finished) => RequestOutcome::Finished,
            Ok(_) => RequestOutcome::Failed,
            Err(error) => {
                outcome = outcome.and(report_failure(app, key.project_id, &error));
                RequestOutcome::Failed
            }
        };
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
            }
            None => return outcome.map(|()| first.unwrap_or(RequestOutcome::Failed)),
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
    // A réplica reserva a própria sessão além das do time; a ordem fixa da lista evita impasse.
    let mut agents: Vec<Agent> = team.ids().cloned().collect();
    if !agents.contains(&key.agent) {
        agents.push(key.agent.clone());
    }
    let mut sessions = ExchangeSessions::acquire(&state.sessions, key.project_id, &agents).await;
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
    sessions: &mut ExchangeSessions,
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
        dir: &project_dir,
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
