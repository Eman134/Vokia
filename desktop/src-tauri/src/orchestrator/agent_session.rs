use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::agents::Agent;
use crate::claude::{Access, ClaudeError, LiveSession, ProjectAccess, Reply, TurnActivity};
use crate::db::{Db, DbError, DbState, GRANT_SESSION, SETTING_PERMISSION_GRANT};
use crate::events::{AgentState, VokiaEvent};
use crate::mcp::{
    tool_definitions, Binding, ExchangeOutcome, ExchangeSlot, McpServer, McpSession, McpState,
    CONFIG_DIR,
};
use crate::orchestration::snapshot::agent_prompt;
use crate::permissions::PermissionGate;
use crate::session_key::SessionKey;

use super::console::ConsoleSink;
use super::emit::{emit, emit_state, report_limits};
use super::error::FlowError;
use super::flow::profile_of;
use super::project_context::project_choices;
use super::prompts::{maestro_role, role, StepPrompts};
use super::session_policy::{
    call_mode, CallMode, SESSION_CLOSE_TIMEOUT_SECS, SESSION_IDLE_LIMIT_SECS,
    SESSION_RECOVERY_LIMIT, SESSION_ROTATE_MESSAGES,
};
use super::session_pool::{ExchangeSessions, NO_PID};
use super::turns::Exchange;

const SESSION_CLOSE_GRACE: Duration = Duration::from_secs(SESSION_CLOSE_TIMEOUT_SECS);
const SESSION_IDLE_LIMIT: Duration = Duration::from_secs(SESSION_IDLE_LIMIT_SECS);

pub(super) struct StepReply {
    pub(super) text: String,
    pub(super) outcome: ExchangeOutcome,
    pub(super) activity: Option<TurnActivity>,
}

pub(super) async fn ask_step(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions<'_>,
    agent: &Agent,
    prompts: &StepPrompts,
) -> Result<StepReply, FlowError> {
    let Exchange { app, key, .. } = exchange;
    emit_state(app, key.project_id, agent.clone(), AgentState::Thinking)?;
    let console = ConsoleSink::new(
        app,
        SessionKey {
            project_id: key.project_id,
            agent: agent.clone(),
        },
    );
    console.started();
    let result = run_step(exchange, sessions, agent, prompts, &console).await;
    console.finished(&result);
    result
}

async fn run_step(
    exchange: &Exchange<'_>,
    sessions: &mut ExchangeSessions<'_>,
    agent: &Agent,
    prompts: &StepPrompts,
    console: &ConsoleSink<'_>,
) -> Result<StepReply, FlowError> {
    let Exchange {
        app,
        state,
        cli,
        project,
        project_context,
        key,
        team,
        orchestration,
        ..
    } = exchange;
    let profile = profile_of(team, agent)?;
    let mcp = McpLaunch::new(
        app,
        key.project_id,
        agent,
        &crate::orchestration::instances::display_name(team, agent),
    )?;
    let prompt = agent_prompt(*orchestration, team, agent);
    let base = role(
        profile,
        &prompt.roster,
        prompt.layer.as_ref(),
        project_context,
        mcp.is_some(),
    );
    let role = if agent == &Agent::ember() {
        let tools = if mcp.is_some() {
            tool_definitions()
        } else {
            Vec::new()
        };
        // Instruções extras do Ember entram aqui quando o campo do agente existir (Hefesto 28).
        let paused = paused_agents(app, key.project_id)?;
        maestro_role(
            &base,
            &tools,
            &project_choices(app)?,
            None,
            *orchestration,
            team,
            &paused,
        )
    } else {
        base
    };
    let call = AgentCall {
        role,
        model: profile.model.clone(),
        item_id: exchange.item_id.map(str::to_string),
        channel_id: exchange.channel_id,
        mcp,
        has_variables: !exchange.definition.variables.is_empty(),
        collected: StdMutex::default(),
    };
    let first = sessions.opened.insert(agent.clone());
    let mut held = sessions.hold(agent).await;
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
                .reply(
                    cli,
                    *project,
                    gate.clone(),
                    &store,
                    &call,
                    prompts,
                    first,
                    console,
                )
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
            fresh_reply(
                exchange,
                gate,
                &held.slot.pid,
                &call,
                &prompts.opening,
                console,
            )
            .await?
        }
    };
    report_limits(app, reply.limits)?;
    let outcome = call
        .collected
        .lock()
        .map(|mut collected| std::mem::take(&mut *collected))
        .unwrap_or_default();
    Ok(StepReply {
        text: reply.text,
        outcome,
        activity: Some(reply.activity)
            .filter(|activity| activity.thinking_ms.is_some() || !activity.tools.is_empty()),
    })
}

// Banco ausente vira lista vazia, como nas demais leituras do prompt; só as pausas do projeto da troca contam.
fn paused_agents(app: &AppHandle, project_id: i64) -> Result<Vec<Agent>, FlowError> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(Vec::new());
    };
    db.project_paused_agents(project_id)
        .map_err(FlowError::Orchestration)
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
    console: &ConsoleSink<'_>,
) -> Result<Reply, ClaudeError> {
    let mut live = LiveSession::open(
        exchange.cli,
        &call.role,
        &call.model,
        Access::Project(exchange.project),
        None,
        open_mcp(call)?,
    )?;
    pid.store(live.pid().unwrap_or(NO_PID), Ordering::SeqCst);
    let reply = send_in_slot(&mut live, call, message, &gate, console).await;
    live.close(SESSION_CLOSE_GRACE).await;
    pid.store(NO_PID, Ordering::SeqCst);
    reply
}

pub(super) struct SessionStore<'a> {
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

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Place {
    dir: PathBuf,
    item_id: Option<String>,
}

impl Place {
    fn of(project: ProjectAccess<'_>, call: &AgentCall) -> Self {
        Self {
            dir: project.dir.to_path_buf(),
            item_id: call.item_id.clone(),
        }
    }
}

#[derive(Default)]
pub(super) struct AgentSession {
    pub(super) live: Option<LiveSession>,
    pub(super) session_id: Option<String>,
    pub(super) messages: u32,
    pub(super) recoveries: u32,
    pub(super) fresh_only: bool,
    pub(super) last_used: Option<Instant>,
    pub(super) granted: Arc<StdMutex<HashSet<String>>>,
    pub(super) pid: Arc<AtomicU32>,
    pub(super) place: Option<Place>,
}

impl AgentSession {
    // A degradação e as falhas contam por mensagem; a sessão, ao contrário do pool antigo, atravessa várias.
    pub(super) fn begin_exchange(&mut self) {
        self.recoveries = 0;
        self.fresh_only = false;
    }

    pub(super) fn forget_grants(&self) {
        if let Ok(mut granted) = self.granted.lock() {
            granted.clear();
        }
    }

    // Fecha só o processo: o session_id fica para o próximo --resume.
    pub(super) async fn end(&mut self) {
        self.close_live().await;
        self.forget_grants();
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) async fn reply(
        &mut self,
        cli: &Path,
        project: ProjectAccess<'_>,
        gate: PermissionGate<'_>,
        store: &SessionStore<'_>,
        call: &AgentCall,
        prompts: &StepPrompts,
        first: bool,
        console: &ConsoleSink<'_>,
    ) -> Result<Option<Reply>, FlowError> {
        let place = Place::of(project, call);
        let mut next = self.first_attempt(store, &place).await?;
        while let Some(attempt) = next {
            match self
                .attempt(
                    cli,
                    project,
                    gate.clone(),
                    call,
                    attempt,
                    prompts,
                    first,
                    console,
                )
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
        place: &Place,
    ) -> Result<Option<Attempt>, FlowError> {
        // Sessão aberta em outro diretório ou de outro item não atende este pedido; o --resume só vale para o mesmo item.
        if let Some(known) = self.place.replace(place.clone()) {
            if known != *place {
                self.close_live().await;
                if known.item_id != place.item_id {
                    self.session_id = None;
                    store.clear()?;
                }
            }
        }
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
        console: &ConsoleSink<'_>,
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
                    open_mcp(call)?,
                )?
            }
        };
        self.pid
            .store(live.pid().unwrap_or(NO_PID), Ordering::SeqCst);
        match send_in_slot(&mut live, call, message, &gate, console).await {
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

    pub(super) async fn degrade(&mut self) {
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

pub(super) struct AgentCall {
    role: String,
    model: String,
    item_id: Option<String>,
    channel_id: Option<i64>,
    mcp: Option<McpLaunch>,
    has_variables: bool,
    collected: StdMutex<ExchangeOutcome>,
}

// Cada abertura de sessão da CLI ganha Binding, token e arquivo próprios; `--resume` nunca reaproveita o token antigo.
struct McpLaunch {
    server: McpServer,
    dir: PathBuf,
    project_id: i64,
    agent: Agent,
    agent_name: String,
}

impl McpLaunch {
    // Servidor fora do ar: o agente sobe sem MCP e sem regra de ferramentas.
    fn new(
        app: &AppHandle,
        project_id: i64,
        agent: &Agent,
        agent_name: &str,
    ) -> Result<Option<McpLaunch>, FlowError> {
        let state = app.state::<McpState>();
        let Some(server) = state.server() else {
            return Ok(None);
        };
        let data_dir = app.path().app_data_dir().map_err(FlowError::ArchivePath)?;
        Ok(Some(McpLaunch {
            server: server.clone(),
            dir: data_dir.join(CONFIG_DIR),
            project_id,
            agent: agent.clone(),
            agent_name: agent_name.to_string(),
        }))
    }

    fn open(&self) -> Result<McpSession, ClaudeError> {
        let binding = Binding {
            project_id: self.project_id,
            agent: self.agent.clone(),
            agent_name: self.agent_name.clone(),
            slot: ExchangeSlot::default(),
        };
        self.server
            .open_session(&self.dir, binding)
            .map_err(|error| ClaudeError::Mcp(error.to_string()))
    }
}

fn open_mcp(call: &AgentCall) -> Result<Option<McpSession>, ClaudeError> {
    call.mcp.as_ref().map(McpLaunch::open).transpose()
}

async fn send_in_slot(
    live: &mut LiveSession,
    call: &AgentCall,
    message: &str,
    gate: &PermissionGate<'_>,
    console: &ConsoleSink<'_>,
) -> Result<Reply, ClaudeError> {
    let slot = live.exchange_slot().cloned();
    if let Some(slot) = &slot {
        slot.begin(call.item_id.as_deref(), call.channel_id);
        if call.has_variables {
            slot.enable_variables();
        }
    }
    let reply = live
        .send(
            message,
            |request| gate.clone().decide(request),
            |line| console.record(line),
            |activity| console.live_activity(activity),
        )
        .await;
    if let Some(slot) = &slot {
        // Também na falha: o card já gravado numa tentativa que caiu continua precisando de confirmação.
        let outcome = slot.end();
        if let Ok(mut collected) = call.collected.lock() {
            collected.absorb(outcome);
        }
    }
    reply
}
