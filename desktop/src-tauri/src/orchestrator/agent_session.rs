use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::agents::Agent;
use crate::claude::{Access, ClaudeError, LiveSession, ProjectAccess, Reply};
use crate::db::{Db, DbError, DbState, GRANT_SESSION, SETTING_PERMISSION_GRANT};
use crate::events::{AgentState, VokiaEvent};
use crate::permissions::PermissionGate;
use crate::session_key::SessionKey;
use crate::task::{
    call_mode, CallMode, SESSION_CLOSE_TIMEOUT_SECS, SESSION_IDLE_LIMIT_SECS,
    SESSION_RECOVERY_LIMIT, SESSION_ROTATE_MESSAGES,
};

use super::emit::{emit, emit_state, report_limits};
use super::error::FlowError;
use super::flow::profile_of;
use super::prompts::{role, StepPrompts};
use super::session_pool::{ExchangeSessions, NO_PID};
use super::turns::Exchange;

const SESSION_CLOSE_GRACE: Duration = Duration::from_secs(SESSION_CLOSE_TIMEOUT_SECS);
const SESSION_IDLE_LIMIT: Duration = Duration::from_secs(SESSION_IDLE_LIMIT_SECS);

pub(super) async fn ask_step(
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
}
