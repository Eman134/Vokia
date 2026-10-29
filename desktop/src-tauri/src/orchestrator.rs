use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, SystemTimeError, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::cache::{
    parse_update_line, Cache, Change, Rule, Scope, Variable, CONVERSATION_ITEMS, CONVERSATION_VAR,
    RECOVERY_CONVERSATION_ITEMS,
};
use crate::claude::{self, ClaudeError, LimitWindow, LiveSession, RateLimits, Reply};
use crate::events::{Agent, AgentState, Speaker, UsageWindow, VokiaEvent, EVENT_NAME};
use crate::state::AppState;
use crate::task::{
    agent_named, call_mode, degrades_to_fresh, is_live_task, long_request_text, parse_definition,
    ActiveTask, CallMode, TaskDefinition, TaskOutcome, TaskRecord, SESSION_CLOSE_TIMEOUT_SECS,
    SESSION_IDLE_LIMIT_SECS, SESSION_RECOVERY_LIMIT, SESSION_ROTATE_MESSAGES,
};

const PERCENT_SCALE: f64 = 100.0;
const SESSION_CLOSE_GRACE: Duration = Duration::from_secs(SESSION_CLOSE_TIMEOUT_SECS);
const SESSION_IDLE_LIMIT: Duration = Duration::from_secs(SESSION_IDLE_LIMIT_SECS);
const NO_CONVERSATION: usize = 0;
const TEAM_ROSTER: &str = "Time desta sessão: Iara (tech lead) e Davi (dev frontend).";
const PROMPT_RULES: &str = "Para registrar progresso, escreva uma linha `$nome = valor` por variável alterada. Use `@time` ao criar uma variável que o outro agente deve ler e `$nome += item` para listas. Execute a instrução agora: não se apresente nem comente o processo.";
const DELEGATION_RULE: &str = "Delegue só quando o pedido exigir algo que você não sabe fazer ou quando o usuário pediu outro agente pelo nome; nesse caso termine a resposta com uma linha exatamente assim: @<Nome>: <pedido>. Nunca delegue para confirmar o que você já respondeu, nem uma resposta que você já deu, e nunca escreva uma delegação vazia como \"@<Nome>: nenhum pedido adicional\". Quando a tarefa estiver concluída, responda sem essa linha.";
const DELEGATION_MARK: char = '@';
const DELEGATION_SEPARATOR: char = ':';
const REPLY_PREFIX: &str = " respondeu: ";
const ALTERNATE_INSTRUCTION: &str = "É a sua vez. Atualize as variáveis conforme o objetivo.";
const REJECTION_PREFIX: &str = "Sua resposta anterior foi rejeitada: ";
const LOOP_REPEAT_LIMIT: u32 = 3;
const LOOP_MESSAGE: &str = "A conversa entre os agentes parou: a mesma mensagem se repetiu em turnos seguidos sem avançar a tarefa.";
const STALL_LIMIT: u32 = 3;
const USER_LABEL: &str = "Usuário";
const ARCHIVE_LIMIT: usize = 20;
const ARCHIVE_FILE: &str = "tasks-archive.json";
const NO_NAMES: &str = "nenhuma";
const NO_VARIABLES: &str = "- nenhuma";
const INTERPRETER_ROLE: &str = "Você extrai a definição de uma tarefa e não conversa. Responda somente com um objeto JSON, sem texto fora dele, com os campos: nova_tarefa (true se o pedido inicia um assunto diferente do anterior, false se continua o anterior); objetivo (frase curta); participantes (só quem precisa agir: por padrão o destinatário, ou o agente que o pedido cita; inclua os dois apenas quando o pedido mandar os dois agirem); alternar (true somente se o pedido pedir revezamento explícito entre os dois, como alternadamente, um de cada vez ou revezando; sem esse pedido é false, e então variaveis é [] e parada é null); variaveis (lista de {\"nome\", \"tipo\": \"numero\" | \"texto\" | \"lista\", \"valor\", \"regra\": \"livre\" | \"mais_um\" | \"so_leitura\"}); parada ({\"variavel\", \"alcanca\": nome de uma variável numérica ou inteiro}, ou null). Exemplos: \"contem até 20 alternadamente\" → {\"nova_tarefa\":true,\"objetivo\":\"contar até 20 alternadamente\",\"participantes\":[\"Iara\",\"Davi\"],\"alternar\":true,\"variaveis\":[{\"nome\":\"contagem\",\"tipo\":\"numero\",\"valor\":0,\"regra\":\"mais_um\"},{\"nome\":\"limite\",\"tipo\":\"numero\",\"valor\":20,\"regra\":\"so_leitura\"}],\"parada\":{\"variavel\":\"contagem\",\"alcanca\":\"limite\"}}. \"fale um animal alternadamente, você e o davi\" → {\"nova_tarefa\":true,\"objetivo\":\"alternar animais\",\"participantes\":[\"Davi\",\"Iara\"],\"alternar\":true,\"variaveis\":[{\"nome\":\"animais\",\"tipo\":\"lista\",\"valor\":[],\"regra\":\"livre\"}],\"parada\":null}. \"quanto é 1+1?\" → {\"nova_tarefa\":true,\"objetivo\":\"responder 1+1\",\"participantes\":[\"Iara\"],\"alternar\":false,\"variaveis\":[],\"parada\":null}. \"conte até 20\" para o Davi, sem pedir revezamento → {\"nova_tarefa\":true,\"objetivo\":\"contar até 20\",\"participantes\":[\"Davi\"],\"alternar\":false,\"variaveis\":[],\"parada\":null}.";

#[derive(Debug)]
pub enum FlowError {
    CliUnavailable(String),
    Claude(ClaudeError),
    Emit(tauri::Error),
    CachePoisoned,
    Cache(String),
    QueuePoisoned,
    ArchivePoisoned,
    ArchivePath(tauri::Error),
    ArchiveIo(std::io::Error),
    ArchiveJson(serde_json::Error),
    Clock(SystemTimeError),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::CliUnavailable(reason) => write!(f, "{reason}"),
            FlowError::Claude(error) => write!(f, "{error}"),
            FlowError::Emit(error) => write!(f, "Não foi possível emitir o evento: {error}"),
            FlowError::CachePoisoned => write!(
                f,
                "O cache de variáveis ficou inconsistente após uma falha anterior."
            ),
            FlowError::Cache(reason) => write!(f, "{reason}"),
            FlowError::QueuePoisoned => write!(
                f,
                "A fila de pedidos ficou inconsistente após uma falha anterior."
            ),
            FlowError::ArchivePoisoned => write!(
                f,
                "O arquivo de tarefas ficou inconsistente após uma falha anterior."
            ),
            FlowError::ArchivePath(error) => write!(
                f,
                "Não foi possível localizar a pasta de dados do app: {error}"
            ),
            FlowError::ArchiveIo(error) => {
                write!(f, "Não foi possível gravar o arquivo de tarefas: {error}")
            }
            FlowError::ArchiveJson(error) => {
                write!(
                    f,
                    "Não foi possível serializar o arquivo de tarefas: {error}"
                )
            }
            FlowError::Clock(error) => {
                write!(f, "O relógio do sistema está antes de 1970: {error}")
            }
        }
    }
}

impl From<ClaudeError> for FlowError {
    fn from(error: ClaudeError) -> Self {
        FlowError::Claude(error)
    }
}

impl From<tauri::Error> for FlowError {
    fn from(error: tauri::Error) -> Self {
        FlowError::Emit(error)
    }
}

pub async fn handle_request(
    app: &AppHandle,
    state: &AppState,
    request: &str,
    recipient: Agent,
) -> Result<(), FlowError> {
    let cli = state
        .cli
        .as_ref()
        .map_err(|problem| FlowError::CliUnavailable(problem.reason.clone()))?;
    emit_message(app, Speaker::User, Speaker::from(recipient), request)?;
    if !start_or_enqueue(state, request, recipient)? {
        return Ok(());
    }
    run_flow(app, state, cli, request, recipient).await
}

fn start_or_enqueue(state: &AppState, request: &str, recipient: Agent) -> Result<bool, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    if flow.active {
        flow.pending.push_back((request.to_string(), recipient));
        return Ok(false);
    }
    flow.active = true;
    Ok(true)
}

fn next_job(state: &AppState) -> Result<Option<(String, Agent)>, FlowError> {
    let mut flow = state.flow.lock().map_err(|_| FlowError::QueuePoisoned)?;
    let next = flow.pending.pop_front();
    flow.active = next.is_some();
    Ok(next)
}

async fn run_flow(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    request: &str,
    agent: Agent,
) -> Result<(), FlowError> {
    let mut job = (request.to_string(), agent);
    let mut outcome: Result<(), FlowError> = Ok(());
    loop {
        if let Err(error) = run_exchange(app, state, cli, &job.0, job.1).await {
            outcome = outcome.and(report_failure(app, &error));
        }
        match next_job(state)? {
            Some(next) => job = next,
            None => return outcome,
        }
    }
}

async fn run_exchange(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    request: &str,
    origin: Agent,
) -> Result<(), FlowError> {
    let definition = interpret_task(app, state, cli, request, origin).await?;
    begin_task(app, state, &definition, request)?;
    let mut transcript = vec![format!("{USER_LABEL}: {request}")];
    let exchange = Exchange {
        app,
        state,
        cli,
        request,
        origin,
        definition: &definition,
        live_task: is_live_task(&definition, request),
    };
    let mut pool = SessionPool::default();
    let result = run_turns(&exchange, &mut pool, &mut transcript).await;
    pool.close_all().await;
    let outcome = match &result {
        Ok(outcome) => *outcome,
        Err(_) => TaskOutcome::Failed,
    };
    let closed = close_task(state, outcome, transcript);
    result?;
    closed
}

async fn interpret_task(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    request: &str,
    origin: Agent,
) -> Result<TaskDefinition, FlowError> {
    let names = with_cache(state, |cache| Ok(cache.names().join(", ")))?;
    let names = if names.is_empty() {
        NO_NAMES.to_string()
    } else {
        names
    };
    let message = format!(
        "{TEAM_ROSTER} Destinatário: {}. Variáveis existentes: {names}. Pedido: {request}",
        origin.name()
    );
    let reply = claude::ask(cli, &message, INTERPRETER_ROLE).await?;
    report_limits(app, reply.limits)?;
    Ok(parse_definition(&reply.text, origin, request))
}

fn begin_task(
    app: &AppHandle,
    state: &AppState,
    definition: &TaskDefinition,
    request: &str,
) -> Result<(), FlowError> {
    let archived = with_cache(state, |cache| {
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
    outcome: TaskOutcome,
    transcript: Vec<String>,
) -> Result<(), FlowError> {
    with_cache(state, |cache| {
        cache.finish_task(outcome);
        cache.push_conversation(transcript);
        Ok(())
    })
}

#[derive(Clone, Copy)]
struct Exchange<'a> {
    app: &'a AppHandle,
    state: &'a AppState,
    cli: &'a Path,
    request: &'a str,
    origin: Agent,
    definition: &'a TaskDefinition,
    live_task: bool,
}

async fn run_turns(
    exchange: &Exchange<'_>,
    pool: &mut SessionPool,
    transcript: &mut Vec<String>,
) -> Result<TaskOutcome, FlowError> {
    let Exchange {
        app,
        state,
        request,
        origin,
        definition,
        ..
    } = *exchange;
    let mut turn = Turn::first(definition, origin, request);
    let mut loop_guard = LoopGuard::default();
    let mut stalled = 0;
    loop {
        let instruction = turn.instruction();
        let prompts = with_cache(state, |cache| {
            Ok(StepPrompts::build(
                cache,
                definition,
                turn.speaker,
                &instruction,
            ))
        })?;
        let reply = ask_step(exchange, pool, turn.speaker, &prompts).await?;
        let ParsedReply {
            visible,
            delegation,
            updates,
        } = parse_reply(&reply, turn.speaker);
        let applied = with_cache(state, |cache| Ok(cache.apply_reply(turn.speaker, updates)))?;
        let delegation = delegation.filter(|_| !definition.alternate);
        if !applied.rejected.is_empty() {
            if !visible.is_empty() {
                let to = next_turn(definition, &turn, &visible, None)
                    .map_or(Speaker::User, |next| Speaker::from(next.speaker));
                emit_message(app, Speaker::from(turn.speaker), to, &visible)?;
            }
            stalled += 1;
            if stalled == STALL_LIMIT {
                stop_with_error(app, stall_message())?;
                return Ok(TaskOutcome::Stalled);
            }
            pool.review(turn.speaker, stalled, 0).await;
            turn.rejection = Some(applied.rejected.join("; "));
            continue;
        }

        let progressed = !applied.accepted.is_empty();
        let feed = feed_text(&visible, &applied.accepted);
        let finished = with_cache(state, |cache| {
            Ok(definition
                .stop
                .as_ref()
                .is_some_and(|stop| stop.reached(cache)))
        })?;
        if finished {
            if let Some(text) = &feed {
                say(app, transcript, turn.speaker, Speaker::User, text)?;
            }
            emit_state(app, turn.speaker, AgentState::Idle)?;
            let message = format!("Tarefa concluída: {}.", definition.objetivo);
            say(app, transcript, origin, Speaker::User, &message)?;
            return Ok(TaskOutcome::Finished);
        }

        if progressed {
            stalled = 0;
        } else if definition.alternate || delegation.is_some() {
            stalled += 1;
            if stalled == STALL_LIMIT {
                stop_with_error(app, stall_message())?;
                return Ok(TaskOutcome::Stalled);
            }
            pool.review(turn.speaker, stalled, 0).await;
        }

        let next = next_turn(
            definition,
            &turn,
            feed.as_deref().unwrap_or_default(),
            delegation.as_ref(),
        );
        let to = next
            .as_ref()
            .map_or(Speaker::User, |next| Speaker::from(next.speaker));
        if let Some(text) = &feed {
            say(app, transcript, turn.speaker, to, text)?;
        }
        if let Some((helper, helper_request)) = &delegation {
            say(
                app,
                transcript,
                turn.speaker,
                Speaker::from(*helper),
                helper_request,
            )?;
        }
        let speaker_state = if delegation.is_some() {
            AgentState::Waiting
        } else {
            AgentState::Idle
        };
        emit_state(app, turn.speaker, speaker_state)?;
        match next {
            Some(next) => {
                let repeats = loop_guard.register(next.speaker, &next.received);
                if repeats >= LOOP_REPEAT_LIMIT {
                    stop_with_error(app, LOOP_MESSAGE.to_string())?;
                    return Ok(TaskOutcome::Stalled);
                }
                pool.review(next.speaker, 0, repeats).await;
                turn = next;
            }
            None => return Ok(TaskOutcome::Finished),
        }
    }
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

    fn first(definition: &TaskDefinition, origin: Agent, request: &str) -> Self {
        let speaker = definition.participants.first().copied().unwrap_or(origin);
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
) -> Option<Turn> {
    let speaker = turn.speaker;
    if definition.alternate {
        let agent = next_participant(&definition.participants, speaker);
        return Some(Turn::new(agent, ALTERNATE_INSTRUCTION, feed, Vec::new()));
    }
    if let Some((helper, request)) = delegation {
        let mut requesters = turn.requesters.clone();
        requesters.push(speaker);
        return Some(Turn::new(*helper, request, request, requesters));
    }
    let (requester, remaining) = turn.requesters.split_last()?;
    let message = format!("{}{REPLY_PREFIX}{feed}", speaker.name());
    Some(Turn::new(
        *requester,
        &message,
        &message,
        remaining.to_vec(),
    ))
}

fn next_participant(participants: &[Agent], speaker: Agent) -> Agent {
    let index = participants
        .iter()
        .position(|agent| *agent == speaker)
        .map_or(0, |index| index + 1);
    participants[index % participants.len()]
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
    transcript: &mut Vec<String>,
    from: Agent,
    to: Speaker,
    text: &str,
) -> Result<(), FlowError> {
    emit_message(app, Speaker::from(from), to, text)?;
    transcript.push(format!("{}: {text}", from.name()));
    Ok(())
}

fn with_cache<T>(
    state: &AppState,
    action: impl FnOnce(&mut Cache) -> Result<T, String>,
) -> Result<T, FlowError> {
    let mut cache = state.cache.lock().map_err(|_| FlowError::CachePoisoned)?;
    action(&mut cache).map_err(FlowError::Cache)
}

struct StepPrompts {
    opening: String,
    follow_up: String,
    minimal: String,
}

impl StepPrompts {
    fn build(cache: &Cache, definition: &TaskDefinition, agent: Agent, instruction: &str) -> Self {
        let prompt = |conversation_items, with_request| {
            step_prompt(
                cache,
                definition,
                agent,
                instruction,
                conversation_items,
                with_request,
            )
        };
        Self {
            opening: prompt(CONVERSATION_ITEMS, true),
            follow_up: prompt(NO_CONVERSATION, false),
            minimal: prompt(RECOVERY_CONVERSATION_ITEMS, true),
        }
    }
}

// O estado compartilhado só entra quando há revezamento; fora dele o agente recebe o pedido puro.
fn step_prompt(
    cache: &Cache,
    definition: &TaskDefinition,
    agent: Agent,
    instruction: &str,
    conversation_items: usize,
    with_request: bool,
) -> String {
    let mut sections = vec![format!("Objetivo: {}", definition.objetivo)];
    if let Some(request) = cache.long_request().filter(|_| with_request) {
        sections.push(format!(
            "Pedido original:
{request}"
        ));
    }
    if definition.alternate {
        sections.push(format!(
            "Variáveis visíveis:\n{}",
            variables_listing(cache, agent)
        ));
    }
    let conversation = cache.recent_conversation(conversation_items);
    if !conversation.is_empty() {
        let entries: Vec<String> = conversation
            .into_iter()
            .map(|entry| format!("- {entry}"))
            .collect();
        sections.push(format!("Conversa recente:\n{}", entries.join("\n")));
    }
    sections.push(format!("Instrução: {instruction}"));
    if definition.alternate {
        sections.push(PROMPT_RULES.to_string());
    }
    sections.join("\n")
}

fn variables_listing(cache: &Cache, agent: Agent) -> String {
    let lines: Vec<String> = cache
        .visible_to(agent)
        .into_iter()
        .filter(|variable| variable.name != CONVERSATION_VAR)
        .map(describe_variable)
        .collect();
    if lines.is_empty() {
        NO_VARIABLES.to_string()
    } else {
        lines.join("\n")
    }
}

fn describe_variable(variable: &Variable) -> String {
    let read_only = if variable.rule == Rule::ReadOnly {
        ", somente leitura"
    } else {
        ""
    };
    format!(
        "- {} ({}, {}{read_only}) = {}",
        variable.name,
        variable.value.type_label(),
        scope_label(variable.scope),
        variable.value.display()
    )
}

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::Private => "privada",
        Scope::Team => "compartilhada",
    }
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
    let path = app
        .path()
        .app_data_dir()
        .map_err(FlowError::ArchivePath)?
        .join(ARCHIVE_FILE);
    write_archive(&path, &archive)
}

fn write_archive(path: &Path, records: &[TaskRecord]) -> Result<(), FlowError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(FlowError::ArchiveIo)?;
    }
    let json = serde_json::to_vec(records).map_err(FlowError::ArchiveJson)?;
    fs::write(path, json).map_err(FlowError::ArchiveIo)
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

pub fn report_failure(app: &AppHandle, error: &FlowError) -> Result<(), FlowError> {
    stop_with_error(app, error.to_string())
}

fn stop_with_error(app: &AppHandle, message: String) -> Result<(), FlowError> {
    emit(app, VokiaEvent::Error { message })?;
    emit_state(app, Agent::Iara, AgentState::Idle)?;
    emit_state(app, Agent::Davi, AgentState::Idle)
}

async fn ask_step(
    exchange: &Exchange<'_>,
    pool: &mut SessionPool,
    agent: Agent,
    prompts: &StepPrompts,
) -> Result<String, FlowError> {
    emit_state(exchange.app, agent, AgentState::Thinking)?;
    let agent_role = role(agent);
    let live = match call_mode(exchange.live_task, pool.is_fresh_only(agent)) {
        CallMode::Live => {
            pool.live_reply(exchange.cli, agent, &agent_role, prompts)
                .await?
        }
        CallMode::Fresh => None,
    };
    let reply = match live {
        Some(reply) => reply,
        None => claude::ask(exchange.cli, &prompts.opening, &agent_role).await?,
    };
    report_limits(exchange.app, reply.limits)?;
    Ok(reply.text)
}

#[derive(Default)]
struct SessionPool {
    agents: HashMap<Agent, AgentSession>,
}

impl SessionPool {
    fn is_fresh_only(&self, agent: Agent) -> bool {
        self.agents
            .get(&agent)
            .is_some_and(|session| session.fresh_only)
    }

    async fn live_reply(
        &mut self,
        cli: &Path,
        agent: Agent,
        role: &str,
        prompts: &StepPrompts,
    ) -> Result<Option<Reply>, FlowError> {
        self.agents
            .entry(agent)
            .or_default()
            .reply(cli, role, prompts)
            .await
    }

    async fn review(&mut self, agent: Agent, stalls: u32, repeats: u32) {
        let Some(session) = self.agents.get_mut(&agent) else {
            return;
        };
        if degrades_to_fresh(stalls, repeats, session.recoveries) {
            session.degrade().await;
        }
    }

    async fn close_all(self) {
        for mut session in self.agents.into_values() {
            session.close_live().await;
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
}

impl AgentSession {
    async fn reply(
        &mut self,
        cli: &Path,
        role: &str,
        prompts: &StepPrompts,
    ) -> Result<Option<Reply>, FlowError> {
        let mut next = self.first_attempt().await;
        while let Some(attempt) = next {
            match self.attempt(cli, role, attempt, prompts).await {
                Ok(reply) => return Ok(Some(reply)),
                Err(error) if error.is_session_loss() => next = self.after_loss(attempt),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(None)
    }

    async fn first_attempt(&mut self) -> Option<Attempt> {
        if self.live.is_none() {
            return Some(Attempt::Open);
        }
        if self.messages >= SESSION_ROTATE_MESSAGES {
            self.close_live().await;
            self.session_id = None;
            return Some(Attempt::Open);
        }
        if self
            .last_used
            .is_some_and(|used| used.elapsed() >= SESSION_IDLE_LIMIT)
        {
            self.close_live().await;
            return self.after_loss(Attempt::Reuse);
        }
        Some(Attempt::Reuse)
    }

    async fn attempt(
        &mut self,
        cli: &Path,
        role: &str,
        attempt: Attempt,
        prompts: &StepPrompts,
    ) -> Result<Reply, ClaudeError> {
        let message = match attempt {
            Attempt::Open => &prompts.opening,
            Attempt::Reuse | Attempt::Resume => &prompts.follow_up,
            Attempt::Minimal => &prompts.minimal,
        };
        let mut live = match (attempt, self.live.take()) {
            (Attempt::Reuse, Some(live)) => live,
            (_, previous) => {
                if let Some(previous) = previous {
                    previous.close(SESSION_CLOSE_GRACE).await;
                }
                let resume = match attempt {
                    Attempt::Resume => self.session_id.as_deref(),
                    _ => None,
                };
                LiveSession::open(cli, role, resume)?
            }
        };
        match live.send(message).await {
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
        }
    }
}

fn report_limits(app: &AppHandle, limits: Option<RateLimits>) -> Result<(), FlowError> {
    let Some(limits) = limits else {
        return Ok(());
    };
    emit(
        app,
        VokiaEvent::PlanUsage {
            five_hour: usage_window(limits.five_hour),
            seven_day: usage_window(limits.seven_day),
        },
    )
}

fn usage_window(window: LimitWindow) -> UsageWindow {
    UsageWindow {
        percent: (window.utilization * PERCENT_SCALE)
            .round()
            .clamp(0.0, PERCENT_SCALE) as u8,
        resets_at: window.resets_at,
    }
}

fn role(agent: Agent) -> String {
    let title = match agent {
        Agent::Iara => "tech lead",
        Agent::Davi => "dev frontend",
    };
    format!(
        "Você é {}, {title}. {TEAM_ROSTER} Responda o pedido diretamente, em português, em no máximo 3 frases, salvo quando a instrução pedir só um valor. {DELEGATION_RULE}",
        agent.name()
    )
}

struct ParsedReply {
    visible: String,
    delegation: Option<(Agent, String)>,
    updates: Vec<Result<Change, String>>,
}

fn parse_reply(reply: &str, from: Agent) -> ParsedReply {
    let mut visible = Vec::new();
    let mut delegation = None;
    let mut updates = Vec::new();
    for line in reply.lines() {
        if let Some(update) = parse_update_line(line) {
            updates.push(update);
        } else if let Some((agent, request)) = delegation_request(line, from) {
            delegation = Some((agent, request.to_string()));
        } else {
            visible.push(line);
        }
    }
    ParsedReply {
        visible: visible.join("\n").trim().to_string(),
        delegation,
        updates,
    }
}

fn delegation_request(line: &str, from: Agent) -> Option<(Agent, &str)> {
    let rest = line.trim().strip_prefix(DELEGATION_MARK)?;
    let (name, request) = rest.split_once(DELEGATION_SEPARATOR)?;
    let agent = agent_named(name.trim())?;
    let request = request.trim();
    (agent != from && !request.is_empty()).then_some((agent, request))
}

fn emit_message(app: &AppHandle, from: Speaker, to: Speaker, text: &str) -> Result<(), FlowError> {
    emit(
        app,
        VokiaEvent::Message {
            from,
            to,
            text: text.to_string(),
            time: now_millis()?,
        },
    )
}

fn emit_state(app: &AppHandle, agent: Agent, state: AgentState) -> Result<(), FlowError> {
    emit(app, VokiaEvent::AgentState { agent, state })
}

fn emit(app: &AppHandle, event: VokiaEvent) -> Result<(), FlowError> {
    Ok(app.emit(EVENT_NAME, event)?)
}

fn now_millis() -> Result<u64, FlowError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(FlowError::Clock)?;
    Ok(elapsed.as_millis() as u64)
}
