use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::agents::{Agent, AgentError, AgentProfile, AgentRegistry, Team};
use crate::cache::{Cache, Change};
use crate::claude::{
    self, kill_process_tree, Access, ClaudeError, LiveSession, ProjectAccess, Reply,
};
use crate::db::{Db, DbError, DbState, GRANT_SESSION, SETTING_PERMISSION_GRANT};
use crate::events::{AgentState, Speaker, VokiaEvent};
use crate::permissions::PermissionGate;
use crate::session_key::SessionKey;
use crate::state::{AppState, PendingDecision, QueuedJob, RequestContext};
use crate::task::MAX_LIVE_SESSIONS;
use crate::task::{
    call_mode, degrades_to_fresh, long_request_text, parse_definition, ActiveTask, CallMode,
    TaskDefinition, TaskOutcome, TaskRecord, SESSION_CLOSE_TIMEOUT_SECS, SESSION_IDLE_LIMIT_SECS,
    SESSION_RECOVERY_LIMIT, SESSION_ROTATE_MESSAGES,
};

use self::emit::{emit, emit_message, emit_state, report_limits};
use self::markers::{parse_decision, parse_reply, ParsedDecision, ParsedReply};
use self::project_context::{load_project, project_context, project_dir, scan_structure};
use self::prompts::{
    role, StepPrompts, ALTERNATE_INSTRUCTION, INTERPRETER_ROLE, NO_NAMES, REJECTION_PREFIX,
    REPLY_PREFIX,
};

mod emit;
mod error;
mod markers;
mod project_context;
mod prompts;

pub use emit::now_millis;
pub use error::{FlowError, RequestOutcome};

const SESSION_CLOSE_GRACE: Duration = Duration::from_secs(SESSION_CLOSE_TIMEOUT_SECS);
const SESSION_IDLE_LIMIT: Duration = Duration::from_secs(SESSION_IDLE_LIMIT_SECS);
const NO_CONVERSATION: usize = 0;
const LOOP_REPEAT_LIMIT: u32 = 3;
const LOOP_MESSAGE: &str = "A conversa entre os agentes parou: a mesma mensagem se repetiu em turnos seguidos sem avançar a tarefa.";
const STALL_LIMIT: u32 = 3;
const USER_LABEL: &str = "Usuário";
const ARCHIVE_LIMIT: usize = 20;
const ARCHIVE_FILE: &str = "tasks-archive.json";

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
    let context = RequestContext::new(None, decision.item_id);
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
            team,
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
    team: &Team,
) -> Result<TaskOutcome, FlowError> {
    let mut sessions = ExchangeSessions::acquire(&state.sessions, key.project_id, team).await;
    let result = exchange_turns(app, state, cli, key, request, item_id, &mut sessions, team).await;
    sessions.release().await;
    result
}

#[allow(clippy::too_many_arguments)]
async fn exchange_turns(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    key: SessionKey,
    request: &str,
    item_id: Option<&str>,
    sessions: &mut ExchangeSessions,
    team: &Team,
) -> Result<TaskOutcome, FlowError> {
    emit_state(app, key.project_id, key.agent.clone(), AgentState::Sending)?;
    let project_dir = project_dir(app, key.project_id)?;
    let project = load_project(app, key.project_id)?;
    // Sem cache: a pasta é relida a cada exchange para refletir o estado atual.
    let tree = scan_structure(&project_dir).await;
    let context = project_context(&project, tree.as_ref());
    let definition = interpret_task(app, state, cli, request, key.clone(), team).await?;
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
        key: key.clone(),
        definition: &definition,
        team,
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

async fn interpret_task(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    request: &str,
    key: SessionKey,
    team: &Team,
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
    let reply = claude::ask(cli, &message, INTERPRETER_ROLE, &profile.model).await?;
    report_limits(app, reply.limits)?;
    Ok(parse_definition(&reply.text, &origin, request, team))
}

fn profile_of<'a>(team: &'a Team, agent: &Agent) -> Result<&'a AgentProfile, FlowError> {
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

struct Exchange<'a> {
    app: &'a AppHandle,
    state: &'a AppState,
    cli: &'a Path,
    project: ProjectAccess<'a>,
    project_context: &'a str,
    request: &'a str,
    item_id: Option<&'a str>,
    key: SessionKey,
    definition: &'a TaskDefinition,
    team: &'a Team,
}

async fn run_turns(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions,
    transcript: &mut Vec<String>,
) -> Result<TaskOutcome, FlowError> {
    let Exchange {
        app,
        state,
        request,
        key,
        definition,
        team,
        ..
    } = exchange;
    let project_id = key.project_id;
    let origin = key.agent.clone();
    let mut turn = Turn::first(definition, &origin, request);
    let mut loop_guard = LoopGuard::default();
    let mut stalled = 0;
    loop {
        let instruction = turn.instruction();
        let prompts = with_cache(state, project_id, |cache| {
            Ok(StepPrompts::build(
                cache,
                definition,
                &turn.speaker,
                &instruction,
            ))
        })?;
        let reply = ask_step(exchange, sessions, &turn.speaker, &prompts).await?;
        if let Some(parsed) = parse_decision(&reply) {
            return raise_decision(exchange, transcript, &turn, parsed);
        }
        let ParsedReply {
            visible,
            delegation,
            updates,
        } = parse_reply(&reply, &turn.speaker, team);
        let applied = with_cache(state, project_id, |cache| {
            Ok(cache.apply_reply(&turn.speaker, updates, team))
        })?;
        let delegation = delegation.filter(|_| !definition.alternate);
        if !applied.rejected.is_empty() {
            if !visible.is_empty() {
                let to = next_turn(definition, &turn, &visible, None, team)
                    .map_or(Speaker::User, |next| Speaker::from(next.speaker));
                emit_message(
                    app,
                    project_id,
                    Speaker::from(turn.speaker.clone()),
                    to,
                    &visible,
                )?;
            }
            stalled += 1;
            if stalled == STALL_LIMIT {
                stop_with_error(app, project_id, stall_message())?;
                return Ok(TaskOutcome::Stalled);
            }
            sessions.review(&turn.speaker, stalled, 0, team)?.await;
            turn.rejection = Some(applied.rejected.join("; "));
            continue;
        }

        let progressed = !applied.accepted.is_empty();
        let feed = feed_text(&visible, &applied.accepted);
        let finished = with_cache(state, project_id, |cache| {
            Ok(definition
                .stop
                .as_ref()
                .is_some_and(|stop| stop.reached(cache)))
        })?;
        if finished {
            if let Some(text) = &feed {
                say(
                    app,
                    project_id,
                    transcript,
                    &turn.speaker,
                    Speaker::User,
                    text,
                    team,
                )?;
            }
            emit_state(app, project_id, turn.speaker.clone(), AgentState::Idle)?;
            let message = format!("Tarefa concluída: {}.", definition.objetivo);
            say(
                app,
                project_id,
                transcript,
                &origin,
                Speaker::User,
                &message,
                team,
            )?;
            return Ok(TaskOutcome::Finished);
        }

        if progressed {
            stalled = 0;
        } else if definition.alternate || delegation.is_some() {
            stalled += 1;
            if stalled == STALL_LIMIT {
                stop_with_error(app, project_id, stall_message())?;
                return Ok(TaskOutcome::Stalled);
            }
            sessions.review(&turn.speaker, stalled, 0, team)?.await;
        }

        let next = next_turn(
            definition,
            &turn,
            feed.as_deref().unwrap_or_default(),
            delegation.as_ref(),
            team,
        );
        let to = next
            .as_ref()
            .map_or(Speaker::User, |next| Speaker::from(next.speaker.clone()));
        if let Some(text) = &feed {
            say(app, project_id, transcript, &turn.speaker, to, text, team)?;
        }
        if let Some((helper, helper_request)) = &delegation {
            say(
                app,
                project_id,
                transcript,
                &turn.speaker,
                Speaker::from(helper.clone()),
                helper_request,
                team,
            )?;
        }
        let speaker_state = if delegation.is_some() {
            AgentState::Waiting
        } else {
            AgentState::Idle
        };
        emit_state(app, project_id, turn.speaker.clone(), speaker_state)?;
        match next {
            Some(next) => {
                let repeats = loop_guard.register(next.speaker.clone(), &next.received);
                if repeats >= LOOP_REPEAT_LIMIT {
                    stop_with_error(app, project_id, LOOP_MESSAGE.to_string())?;
                    return Ok(TaskOutcome::Stalled);
                }
                sessions.review(&next.speaker, 0, repeats, team)?.await;
                turn = next;
            }
            None => return Ok(TaskOutcome::Finished),
        }
    }
}

// Quem decide fica Waiting até a resposta virar um novo pedido; quem delegou a ele volta a Idle.
fn raise_decision(
    exchange: &Exchange<'_>,
    transcript: &mut Vec<String>,
    turn: &Turn,
    parsed: ParsedDecision,
) -> Result<TaskOutcome, FlowError> {
    let Exchange {
        app,
        state,
        request,
        item_id,
        key,
        team,
        ..
    } = exchange;
    let project_id = key.project_id;
    let ParsedDecision {
        kind,
        title,
        detail,
        options,
        visible,
    } = parsed;
    let agent = turn.speaker.clone();
    let visible = parse_reply(&visible, &agent, team).visible;
    if !visible.is_empty() {
        say(
            app,
            project_id,
            transcript,
            &agent,
            Speaker::User,
            &visible,
            team,
        )?;
    }
    let id = state.next_decision_id.fetch_add(1, Ordering::Relaxed);
    state
        .decisions
        .lock()
        .map_err(|_| FlowError::DecisionsPoisoned)?
        .insert(
            id,
            PendingDecision {
                agent: agent.clone(),
                project_id,
                title: title.clone(),
                request: request.to_string(),
                item_id: item_id.map(str::to_string),
            },
        );
    emit(
        app,
        VokiaEvent::Decision {
            id,
            agent: agent.clone(),
            kind,
            title,
            detail,
            options,
            project_id: Some(project_id),
            item_id: item_id.map(str::to_string),
        },
    )?;
    for requester in &turn.requesters {
        emit_state(app, project_id, requester.clone(), AgentState::Idle)?;
    }
    emit_state(app, project_id, agent, AgentState::Waiting)?;
    Ok(TaskOutcome::Finished)
}

fn first_speaker(definition: &TaskDefinition, origin: &Agent) -> Agent {
    definition
        .participants
        .first()
        .cloned()
        .unwrap_or_else(|| origin.clone())
}

struct Turn {
    speaker: Agent,
    base: String,
    rejection: Option<String>,
    received: String,
    requesters: Vec<Agent>,
}

impl Turn {
    fn new(speaker: Agent, base: &str, received: &str, requesters: Vec<Agent>) -> Self {
        Self {
            speaker,
            base: base.to_string(),
            rejection: None,
            received: received.to_string(),
            requesters,
        }
    }

    fn first(definition: &TaskDefinition, origin: &Agent, request: &str) -> Self {
        let speaker = first_speaker(definition, origin);
        Self::new(
            speaker,
            &format!("Pedido do usuário: {request}"),
            request,
            Vec::new(),
        )
    }

    fn instruction(&self) -> String {
        match &self.rejection {
            Some(motive) => format!("{REJECTION_PREFIX}{motive}. {}", self.base),
            None => self.base.clone(),
        }
    }
}

fn next_turn(
    definition: &TaskDefinition,
    turn: &Turn,
    feed: &str,
    delegation: Option<&(Agent, String)>,
    team: &Team,
) -> Option<Turn> {
    let speaker = turn.speaker.clone();
    if definition.alternate {
        let agent = next_participant(&definition.participants, &speaker);
        return Some(Turn::new(agent, ALTERNATE_INSTRUCTION, feed, Vec::new()));
    }
    if let Some((helper, request)) = delegation {
        let mut requesters = turn.requesters.clone();
        requesters.push(speaker);
        return Some(Turn::new(helper.clone(), request, request, requesters));
    }
    let (requester, remaining) = turn.requesters.split_last()?;
    let message = format!("{}{REPLY_PREFIX}{feed}", team.name_of(&speaker));
    Some(Turn::new(
        requester.clone(),
        &message,
        &message,
        remaining.to_vec(),
    ))
}

fn next_participant(participants: &[Agent], speaker: &Agent) -> Agent {
    let index = participants
        .iter()
        .position(|agent| agent == speaker)
        .map_or(0, |index| index + 1);
    participants[index % participants.len()].clone()
}

fn feed_text(visible: &str, accepted: &[Change]) -> Option<String> {
    if !visible.is_empty() {
        return Some(visible.to_string());
    }
    match accepted {
        [only] => only.feed(),
        _ => None,
    }
}

fn say(
    app: &AppHandle,
    project_id: i64,
    transcript: &mut Vec<String>,
    from: &Agent,
    to: Speaker,
    text: &str,
    team: &Team,
) -> Result<(), FlowError> {
    emit_message(app, project_id, Speaker::from(from.clone()), to, text)?;
    transcript.push(format!("{}: {text}", team.name_of(from)));
    Ok(())
}

fn with_cache<T>(
    state: &AppState,
    project_id: i64,
    action: impl FnOnce(&mut Cache) -> Result<T, String>,
) -> Result<T, FlowError> {
    let mut caches = state.cache.lock().map_err(|_| FlowError::CachePoisoned)?;
    action(caches.entry(project_id).or_default()).map_err(FlowError::Cache)
}

fn archive_task(app: &AppHandle, state: &AppState, record: TaskRecord) -> Result<(), FlowError> {
    let mut archive = state
        .archive
        .lock()
        .map_err(|_| FlowError::ArchivePoisoned)?;
    archive.push(record);
    if archive.len() > ARCHIVE_LIMIT {
        archive.remove(0);
    }
    write_archive(&archive_path(app)?, &archive)
}

fn archive_path(app: &AppHandle) -> Result<PathBuf, FlowError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(FlowError::ArchivePath)?
        .join(ARCHIVE_FILE))
}

fn write_archive(path: &Path, records: &[TaskRecord]) -> Result<(), FlowError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(FlowError::ArchiveIo)?;
    }
    let json = serde_json::to_vec(records).map_err(FlowError::ArchiveJson)?;
    fs::write(path, json).map_err(FlowError::ArchiveIo)
}

pub(crate) fn remove_archive(app: &AppHandle, state: &AppState) -> Result<(), FlowError> {
    state
        .archive
        .lock()
        .map_err(|_| FlowError::ArchivePoisoned)?
        .clear();
    match fs::remove_file(archive_path(app)?) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FlowError::ArchiveIo(error)),
    }
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
}

#[derive(Default)]
struct LoopGuard {
    last: HashMap<Agent, (String, u32)>,
}

impl LoopGuard {
    fn register(&mut self, agent: Agent, message: &str) -> u32 {
        let normalized = normalize_message(message);
        let (last, repeats) = self
            .last
            .entry(agent)
            .or_insert_with(|| (normalized.clone(), 0));
        if *last == normalized {
            *repeats += 1;
        } else {
            *last = normalized;
            *repeats = 1;
        }
        *repeats
    }
}

fn normalize_message(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn stall_message() -> String {
    format!("A tarefa parou: o estado não avançou em {STALL_LIMIT} turnos seguidos.")
}

pub fn report_failure(
    app: &AppHandle,
    project_id: i64,
    error: &FlowError,
) -> Result<(), FlowError> {
    stop_with_error(app, project_id, error.to_string())
}

fn stop_with_error(app: &AppHandle, project_id: i64, message: String) -> Result<(), FlowError> {
    emit(app, VokiaEvent::Error { message })?;
    let agents = match app.state::<AgentRegistry>().team() {
        Ok(team) => team.ids().cloned().collect(),
        Err(_) => vec![Agent::iara(), Agent::davi()],
    };
    for agent in agents {
        emit_state(app, project_id, agent, AgentState::Idle)?;
    }
    Ok(())
}

async fn ask_step(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions,
    agent: &Agent,
    prompts: &StepPrompts,
) -> Result<String, FlowError> {
    let Exchange {
        app,
        state,
        cli,
        project,
        project_context,
        key,
        team,
        ..
    } = exchange;
    emit_state(app, key.project_id, agent.clone(), AgentState::Thinking)?;
    let profile = profile_of(team, agent)?;
    let call = AgentCall {
        role: role(profile, &team.roster_line(), project_context),
        model: profile.model.clone(),
    };
    let first = sessions.opened.insert(agent.clone());
    let held = sessions.held(agent, team)?;
    if first {
        held.guard.begin_exchange();
    }
    if held.guard.live.is_none() {
        state.sessions.make_room(&held.slot).await;
    }
    let grant_per_session = grants_per_session(app)?;
    if !grant_per_session {
        held.guard.forget_grants();
    }
    let granted = Arc::clone(&held.guard.granted);
    let gate = PermissionGate {
        app,
        broker: &state.permissions,
        agent: agent.clone(),
        project_id: key.project_id,
        granted: &granted,
        grant_per_session,
    };
    let store = SessionStore {
        app,
        key: SessionKey {
            project_id: key.project_id,
            agent: agent.clone(),
        },
    };
    let live = match call_mode(held.guard.fresh_only) {
        CallMode::Live => {
            held.guard
                .reply(cli, *project, gate.clone(), &store, &call, prompts, first)
                .await?
        }
        CallMode::Fresh => None,
    };
    let reply = match live {
        Some(reply) => reply,
        None => {
            // Sem sessão viva não há onde acumular aprovações: cada comando pergunta de novo.
            let gate = PermissionGate {
                grant_per_session: false,
                ..gate
            };
            fresh_reply(exchange, gate, &held.slot.pid, &call, &prompts.opening).await?
        }
    };
    report_limits(app, reply.limits)?;
    Ok(reply.text)
}

fn grants_per_session(app: &AppHandle) -> Result<bool, FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(false);
    };
    let settings = db.load_settings().map_err(FlowError::Settings)?;
    Ok(settings
        .get(SETTING_PERMISSION_GRANT)
        .is_some_and(|grant| grant == GRANT_SESSION))
}

// Contexto novo a cada chamada, mas pela sessão viva: só ela tem canal para o usuário decidir comandos fora da allowlist.
async fn fresh_reply(
    exchange: &Exchange<'_>,
    gate: PermissionGate<'_>,
    pid: &AtomicU32,
    call: &AgentCall,
    message: &str,
) -> Result<Reply, ClaudeError> {
    let mut live = LiveSession::open(
        exchange.cli,
        &call.role,
        &call.model,
        Access::Project(exchange.project),
        None,
    )?;
    pid.store(live.pid().unwrap_or(NO_PID), Ordering::SeqCst);
    let reply = live
        .send(message, |request| gate.clone().decide(request))
        .await;
    live.close(SESSION_CLOSE_GRACE).await;
    pid.store(NO_PID, Ordering::SeqCst);
    reply
}

const NO_PID: u32 = 0;

// `closing` e `pid` ficam fora do lock da sessão: o fechamento pode ser marcado e o processo encerrado com uma mensagem em andamento.
struct SessionSlot {
    closing: AtomicBool,
    pid: Arc<AtomicU32>,
    session: Arc<Mutex<AgentSession>>,
}

impl SessionSlot {
    fn new() -> Self {
        let pid = Arc::new(AtomicU32::new(NO_PID));
        let session = AgentSession {
            pid: Arc::clone(&pid),
            ..AgentSession::default()
        };
        Self {
            closing: AtomicBool::new(false),
            pid,
            session: Arc::new(Mutex::new(session)),
        }
    }

    fn is_live(&self) -> bool {
        self.pid.load(Ordering::SeqCst) != NO_PID
    }
}

#[derive(Default)]
pub(crate) struct SessionPool {
    slots: StdMutex<HashMap<SessionKey, Arc<SessionSlot>>>,
}

impl SessionPool {
    fn slot(&self, key: SessionKey) -> Arc<SessionSlot> {
        Arc::clone(
            self.lock()
                .entry(key)
                .or_insert_with(|| Arc::new(SessionSlot::new())),
        )
    }

    // O mapa só recebe inserções e remoções simples; um lock envenenado não deixa estado pela metade.
    fn lock(&self) -> MutexGuard<'_, HashMap<SessionKey, Arc<SessionSlot>>> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn matching(&self, belongs: impl Fn(&SessionKey) -> bool) -> Vec<Arc<SessionSlot>> {
        self.lock()
            .iter()
            .filter(|(key, _)| belongs(key))
            .map(|(_, slot)| Arc::clone(slot))
            .collect()
    }

    // A mensagem em andamento termina e quem a roda fecha a sessão (ExchangeSessions::release);
    // se nenhuma roda, o lock livre fecha aqui. A marca nunca reabre a sessão.
    pub(crate) async fn close_project(&self, project_id: i64) {
        end_slots(self.matching(|key| key.project_id == project_id)).await;
    }

    // Gancho da remoção do agente (O4): ainda sem chamador.
    #[allow(dead_code)]
    pub(crate) async fn close_agent(&self, db: &Db, agent: Agent) -> Result<(), DbError> {
        let removed: Vec<Arc<SessionSlot>> = {
            let mut slots = self.lock();
            let keys: Vec<SessionKey> = slots
                .keys()
                .filter(|key| key.agent == agent)
                .cloned()
                .collect();
            keys.iter().filter_map(|key| slots.remove(key)).collect()
        };
        end_slots(removed).await;
        db.clear_agent_sessions(&agent)
    }

    // Na saída do app o processo é encerrado pelo pid, sem esperar o lock de um passo em andamento.
    pub(crate) fn close_all(&self) {
        for slot in self.matching(|_| true) {
            let pid = slot.pid.swap(NO_PID, Ordering::SeqCst);
            if pid != NO_PID {
                kill_process_tree(pid);
            }
        }
    }

    // Abre espaço para um novo processo fechando a sessão ociosa menos recente; sessões em uso (lock tomado) ficam.
    async fn make_room(&self, requester: &Arc<SessionSlot>) {
        loop {
            let live: Vec<Arc<SessionSlot>> = self
                .matching(|_| true)
                .into_iter()
                .filter(|slot| !Arc::ptr_eq(slot, requester) && slot.is_live())
                .collect();
            if live.len() < MAX_LIVE_SESSIONS {
                return;
            }
            let mut oldest: Option<OwnedMutexGuard<AgentSession>> = None;
            for slot in &live {
                let Ok(guard) = Arc::clone(&slot.session).try_lock_owned() else {
                    continue;
                };
                if oldest
                    .as_ref()
                    .is_none_or(|current| guard.last_used < current.last_used)
                {
                    oldest = Some(guard);
                }
            }
            let Some(mut idle) = oldest else {
                return;
            };
            idle.end().await;
        }
    }
}

async fn end_slots(slots: Vec<Arc<SessionSlot>>) {
    for slot in &slots {
        slot.closing.store(true, Ordering::SeqCst);
    }
    for slot in slots {
        let mut session = slot.session.lock().await;
        if slot.closing.swap(false, Ordering::SeqCst) {
            session.end().await;
        }
    }
}

struct Held {
    slot: Arc<SessionSlot>,
    guard: OwnedMutexGuard<AgentSession>,
}

struct ExchangeSessions {
    held: Vec<(Agent, Held)>,
    opened: HashSet<Agent>,
}

impl ExchangeSessions {
    async fn acquire(pool: &SessionPool, project_id: i64, team: &Team) -> Self {
        let mut held = Vec::new();
        for agent in team.ids() {
            let slot = pool.slot(SessionKey {
                project_id,
                agent: agent.clone(),
            });
            let guard = Arc::clone(&slot.session).lock_owned().await;
            held.push((agent.clone(), Held { slot, guard }));
        }
        Self {
            held,
            opened: HashSet::new(),
        }
    }

    fn held(&mut self, agent: &Agent, team: &Team) -> Result<&mut Held, FlowError> {
        self.held
            .iter_mut()
            .find(|(held_agent, _)| held_agent == agent)
            .map(|(_, held)| held)
            .ok_or_else(|| FlowError::SessionNotHeld(team.name_of(agent).to_string()))
    }

    fn review(
        &mut self,
        agent: &Agent,
        stalls: u32,
        repeats: u32,
        team: &Team,
    ) -> Result<impl std::future::Future<Output = ()> + '_, FlowError> {
        let session = &mut self.held(agent, team)?.guard;
        Ok(async move {
            if degrades_to_fresh(stalls, repeats, session.recoveries) {
                session.degrade().await;
            }
        })
    }

    // Sessão marcada para fechar durante a mensagem fecha aqui, antes de qualquer outra mensagem usá-la.
    async fn release(self) {
        for (_, mut held) in self.held {
            if held.slot.closing.swap(false, Ordering::SeqCst) {
                held.guard.end().await;
            }
        }
    }
}

struct SessionStore<'a> {
    app: &'a AppHandle,
    key: SessionKey,
}

impl SessionStore<'_> {
    fn load(&self) -> Result<Option<String>, FlowError> {
        let state = self.app.state::<DbState>();
        let Some(db) = state.db() else {
            return Ok(None);
        };
        db.agent_session(&self.key).map_err(FlowError::SessionStore)
    }

    fn save(&self, session_id: &str) -> Result<(), FlowError> {
        self.write(|db| db.save_agent_session(&self.key, session_id))
    }

    fn clear(&self) -> Result<(), FlowError> {
        self.write(|db| db.clear_agent_session(&self.key))
    }

    // Falha ao gravar não derruba a resposta: a sessão segue viva e o usuário é avisado.
    fn write(&self, action: impl FnOnce(&Db) -> Result<(), DbError>) -> Result<(), FlowError> {
        let state = self.app.state::<DbState>();
        let Some(db) = state.db() else {
            return Ok(());
        };
        match action(db) {
            Ok(()) => Ok(()),
            Err(error) => emit(
                self.app,
                VokiaEvent::Error {
                    message: FlowError::SessionStore(error).to_string(),
                },
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Attempt {
    Reuse,
    Open,
    Resume,
    Minimal,
}

#[derive(Default)]
struct AgentSession {
    live: Option<LiveSession>,
    session_id: Option<String>,
    messages: u32,
    recoveries: u32,
    fresh_only: bool,
    last_used: Option<Instant>,
    granted: Arc<StdMutex<HashSet<String>>>,
    pid: Arc<AtomicU32>,
}

impl AgentSession {
    // A degradação e as falhas contam por mensagem; a sessão, ao contrário do pool antigo, atravessa várias.
    fn begin_exchange(&mut self) {
        self.recoveries = 0;
        self.fresh_only = false;
    }

    fn forget_grants(&self) {
        if let Ok(mut granted) = self.granted.lock() {
            granted.clear();
        }
    }

    // Fecha só o processo: o session_id fica para o próximo --resume.
    async fn end(&mut self) {
        self.close_live().await;
        self.forget_grants();
    }

    #[allow(clippy::too_many_arguments)]
    async fn reply(
        &mut self,
        cli: &Path,
        project: ProjectAccess<'_>,
        gate: PermissionGate<'_>,
        store: &SessionStore<'_>,
        call: &AgentCall,
        prompts: &StepPrompts,
        first: bool,
    ) -> Result<Option<Reply>, FlowError> {
        let mut next = self.first_attempt(store).await?;
        while let Some(attempt) = next {
            match self
                .attempt(cli, project, gate.clone(), call, attempt, prompts, first)
                .await
            {
                Ok(reply) => {
                    if let Some(session_id) = &self.session_id {
                        store.save(session_id)?;
                    }
                    return Ok(Some(reply));
                }
                // Morte no meio da resposta de um processo já em uso: reenviar poderia repetir um comando; o erro vai ao feed.
                Err(error) if error.is_session_loss() && attempt != Attempt::Reuse => {
                    if attempt == Attempt::Resume {
                        self.session_id = None;
                        store.clear()?;
                    }
                    next = self.after_loss(attempt);
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
    }

    async fn first_attempt(
        &mut self,
        store: &SessionStore<'_>,
    ) -> Result<Option<Attempt>, FlowError> {
        if self.live.is_none() {
            if self.session_id.is_none() {
                self.session_id = store.load()?;
            }
            let attempt = if self.session_id.is_some() {
                Attempt::Resume
            } else {
                Attempt::Open
            };
            return Ok(Some(attempt));
        }
        if self.messages >= SESSION_ROTATE_MESSAGES {
            self.close_live().await;
            self.session_id = None;
            return Ok(Some(Attempt::Open));
        }
        if self
            .last_used
            .is_some_and(|used| used.elapsed() >= SESSION_IDLE_LIMIT)
        {
            self.close_live().await;
            return Ok(self.after_loss(Attempt::Reuse));
        }
        Ok(Some(Attempt::Reuse))
    }

    #[allow(clippy::too_many_arguments)]
    async fn attempt(
        &mut self,
        cli: &Path,
        project: ProjectAccess<'_>,
        gate: PermissionGate<'_>,
        call: &AgentCall,
        attempt: Attempt,
        prompts: &StepPrompts,
        first: bool,
    ) -> Result<Reply, ClaudeError> {
        let message = match attempt {
            Attempt::Open => &prompts.opening,
            Attempt::Reuse | Attempt::Resume if first => &prompts.opening,
            Attempt::Reuse | Attempt::Resume => &prompts.follow_up,
            Attempt::Minimal => &prompts.minimal,
        };
        let mut live = match (attempt, self.live.take()) {
            (Attempt::Reuse, Some(live)) => live,
            (_, previous) => {
                if let Some(previous) = previous {
                    previous.close(SESSION_CLOSE_GRACE).await;
                    self.pid.store(NO_PID, Ordering::SeqCst);
                }
                let resume = match attempt {
                    Attempt::Resume => self.session_id.as_deref(),
                    _ => None,
                };
                LiveSession::open(
                    cli,
                    &call.role,
                    &call.model,
                    Access::Project(project),
                    resume,
                )?
            }
        };
        self.pid
            .store(live.pid().unwrap_or(NO_PID), Ordering::SeqCst);
        match live
            .send(message, |request| gate.clone().decide(request))
            .await
        {
            Ok(reply) => {
                self.session_id = live.session_id().map(str::to_string);
                self.live = Some(live);
                self.messages = match attempt {
                    Attempt::Open | Attempt::Minimal => 1,
                    Attempt::Reuse | Attempt::Resume => self.messages + 1,
                };
                self.last_used = Some(Instant::now());
                Ok(reply)
            }
            Err(error) => {
                live.close(SESSION_CLOSE_GRACE).await;
                self.pid.store(NO_PID, Ordering::SeqCst);
                Err(error)
            }
        }
    }

    fn after_loss(&mut self, failed: Attempt) -> Option<Attempt> {
        if failed == Attempt::Resume {
            return Some(Attempt::Minimal);
        }
        self.recoveries += 1;
        if self.recoveries >= SESSION_RECOVERY_LIMIT {
            self.fresh_only = true;
            return None;
        }
        match failed {
            Attempt::Minimal => {
                self.fresh_only = true;
                None
            }
            _ if self.session_id.is_some() => Some(Attempt::Resume),
            _ => Some(Attempt::Minimal),
        }
    }

    async fn degrade(&mut self) {
        self.fresh_only = true;
        self.close_live().await;
    }

    async fn close_live(&mut self) {
        if let Some(live) = self.live.take() {
            live.close(SESSION_CLOSE_GRACE).await;
            self.pid.store(NO_PID, Ordering::SeqCst);
        }
    }
}

struct AgentCall {
    role: String,
    model: String,
}
