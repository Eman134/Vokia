use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager};

use crate::agents::{Agent, AgentProfile, Team};
use crate::claude::{Access, ClaudeError, LiveSession, ProjectAccess, Reply, TurnActivity, TurnUsage};
use crate::db::{
    Db, DbError, DbState, OrchestrationSnapshot, GRANT_SESSION, SETTING_PERMISSION_GRANT,
};
use crate::events::{AgentState, VokiaEvent};
use crate::mcp::{
    tool_definitions, Binding, ExchangeOutcome, ExchangeSlot, McpServer, McpSession, McpState,
    CONFIG_DIR,
};
use crate::orchestration::instances::display_name;
use crate::orchestration::snapshot::{agent_prompt, leader_of};
use crate::permissions::{PermissionGate, PermissionLevel};
use crate::providers::ProviderRegistry;
use crate::session_key::SessionKey;
use crate::state::AppState;

use super::adapters::ProviderAdapter;
use super::console::ConsoleSink;
use super::emit::{emit, emit_feed_note, emit_state, now_millis, report_limits};
use super::error::FlowError;
use super::fallback::{
    emit_overrides_changed, load_global_active_llm, load_global_fallback, mark_cooldown,
    now_millis_u64, resolve_candidates, resolve_program_for, LlmTarget, OverrideReason,
    ResolvedCandidate, TemporaryOverride, DEFAULT_COOLDOWN_SECS,
};
use super::flow::profile_of;
use super::project_context::project_choices;
use super::prompts::{maestro_role, role, StepPrompts, ToolPromptMode};
use super::session_policy::{
    call_mode, CallMode, SESSION_CLOSE_TIMEOUT_SECS, SESSION_IDLE_LIMIT_SECS,
    SESSION_RECOVERY_LIMIT, SESSION_ROTATE_MESSAGES,
};
use super::session_pool::{ExchangeSessions, NO_PID};
use super::turns::Exchange;

pub(super) const SESSION_CLOSE_GRACE: Duration = Duration::from_secs(SESSION_CLOSE_TIMEOUT_SECS);
const SESSION_IDLE_LIMIT: Duration = Duration::from_secs(SESSION_IDLE_LIMIT_SECS);

pub(super) struct StepReply {
    pub(super) text: String,
    pub(super) outcome: ExchangeOutcome,
    pub(super) activity: Option<TurnActivity>,
    pub(super) usage: Option<TurnUsage>,
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
    let providers = app.state::<crate::providers::ProviderRegistry>();
    let global_active = load_global_active_llm(app);
    let global_fallback = load_global_fallback(app);
    let candidates = resolve_candidates(
        agent,
        profile,
        &providers,
        state,
        global_active.as_ref(),
        &global_fallback,
        Some(project.level),
    );
    if candidates.is_empty() {
        return Err(FlowError::CliUnavailable(format!(
            "Nenhum provedor disponível para {agent}."
        )));
    }
    let grant_per_session = grants_per_session(app)?;
    let store = SessionStore {
        app,
        key: SessionKey {
            project_id: key.project_id,
            agent: agent.clone(),
        },
    };
    let mut fallback_reason: Option<OverrideReason> = None;
    let mut fallback_expires_at: Option<u64> = None;
    for (idx, candidate) in candidates.iter().enumerate() {
        let adapter = ProviderAdapter::from_cli(&candidate.cli, &providers)?;
        if agent == &Agent::ember() && !adapter.has_tools() {
            continue;
        }
        let mcp = if adapter.has_tools() {
            McpLaunch::new(
                app,
                key.project_id,
                agent,
                &crate::orchestration::instances::display_name(team, agent),
            )?
        } else {
            None
        };
        let tool_mode = match adapter.tool_transport() {
            crate::providers::ToolTransport::McpNative if mcp.is_some() => ToolPromptMode::Mcp,
            crate::providers::ToolTransport::VokiaCli if mcp.is_some() => {
                if super::adapters::resolve_vokia_cli_path().is_some() {
                    ToolPromptMode::VokiaCli
                } else {
                    ToolPromptMode::None
                }
            }
            _ => ToolPromptMode::None,
        };
        let prompt = agent_prompt(*orchestration, team, agent);
        let base = role(
            profile,
            &prompt.roster,
            prompt.layer.as_ref(),
            project_context,
            tool_mode,
        );
        let role = if agent == &Agent::ember() {
            let tools = if tool_mode != ToolPromptMode::None {
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
                tool_mode,
            )
        } else {
            base
        };
        let (program, user_args) = resolve_program_for(agent, &candidate.command, cli)?;
        let mut call = AgentCall {
            adapter,
            program,
            user_args,
            role,
            model: candidate.model.clone(),
            resume_session: None,
            item_id: exchange.item_id.map(str::to_string),
            channel_id: exchange.channel_id,
            mcp,
            has_variables: !exchange.definition.variables.is_empty(),
            collected: StdMutex::default(),
        };
        if !call.adapter.supports_live_session() {
            call.resume_session = store.load()?.and_then(|stored| {
                decode_provider_session(
                    &stored,
                    &candidate.cli,
                    &candidate.model,
                    exchange.item_id,
                )
            });
        }
        let first = sessions.opened.insert(agent.clone());
        let mut held = sessions.hold(agent).await;
        if first {
            held.guard.begin_exchange();
        }
        if idx > 0 {
            held.guard.degrade().await;
        }
        if held.guard.live.is_none() {
            state.sessions.make_room(&held.slot).await;
        }
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
        let access = if call.adapter.has_tools() {
            Access::Project(*project)
        } else {
            Access::TextOnly
        };
        let attempt: Result<Reply, FlowError> = async {
            let live = if call.adapter.supports_live_session() {
                match call_mode(held.guard.fresh_only) {
                    CallMode::Live => {
                        call.adapter
                            .run_live(
                                &mut held.guard,
                                access,
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
                }
            } else {
                None
            };
            match live {
                Some(reply) => Ok(reply),
                None => {
                    // Sem sessão viva não há onde acumular aprovações: cada comando pergunta de novo.
                    let gate = PermissionGate {
                        grant_per_session: false,
                        ..gate
                    };
                    let message = if call.resume_session.is_some() {
                        &prompts.follow_up
                    } else {
                        &prompts.opening
                    };
                    match fresh_reply(
                        access,
                        gate.clone(),
                        &held.slot.pid,
                        &call,
                        message,
                        console,
                    )
                    .await
                    .map_err(FlowError::Claude)
                    {
                        Err(error)
                            if call.resume_session.is_some()
                                && !error.is_limit()
                                && !error.is_model_unavailable() =>
                        {
                            store.clear()?;
                            call.resume_session = None;
                            fresh_reply(
                                access,
                                gate,
                                &held.slot.pid,
                                &call,
                                &prompts.opening,
                                console,
                            )
                            .await
                            .map_err(FlowError::Claude)
                        }
                        result => result,
                    }
                }
            }
        }
        .await;
        match attempt {
            Ok(reply) => {
                if let Some(reason) = fallback_reason {
                    let ov = TemporaryOverride {
                        agent: agent.clone(),
                        cli: candidate.cli.clone(),
                        model: candidate.model.clone(),
                        model_tier: candidate.model_tier,
                        command: candidate.command.clone(),
                        original_cli: profile.cli.clone(),
                        original_model: profile.model.clone(),
                        original_tier: profile.model_tier,
                        reason,
                        applied_at: now_millis_u64(),
                        expires_at: fallback_expires_at,
                    };
                    state
                        .temporary_overrides
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(agent.clone(), ov);
                    emit_overrides_changed(app, state);
                    let _ = emit_feed_note(
                        app,
                        key.project_id,
                        crate::events::Speaker::Agent(agent.clone()),
                        &format!(
                            "Fallback automático para {} ({})",
                            candidate.cli, candidate.model
                        ),
                        now_millis().unwrap_or_else(|_| now_millis_u64()),
                    );
                }
                if !call.adapter.supports_live_session() {
                    if let Some(session_id) = reply.session_id.as_deref() {
                        store.save(&encode_provider_session(
                            &candidate.cli,
                            &candidate.model,
                            exchange.item_id,
                            session_id,
                        ))?;
                    }
                }
                report_limits(app, reply.limits)?;
                let outcome = call
                    .collected
                    .lock()
                    .map(|mut collected| std::mem::take(&mut *collected))
                    .unwrap_or_default();
                let mut normalized_usage = reply.usage.unwrap_or_default();
                normalized_usage.provider = Some(candidate.cli.clone());
                if normalized_usage
                    .model
                    .as_deref()
                    .is_none_or(|m| m.trim().is_empty())
                {
                    normalized_usage.model = Some(candidate.model.clone());
                }
                return Ok(StepReply {
                    text: reply.text,
                    outcome,
                    activity: Some(reply.activity)
                        .filter(|activity| activity.thinking_ms.is_some() || !activity.tools.is_empty()),
                    usage: Some(normalized_usage),
                });
            }
            Err(err) => {
                let is_limit = err.is_limit();
                let is_unavail = err.is_model_unavailable();
                if (is_limit || is_unavail) && idx + 1 < candidates.len() {
                    mark_cooldown(state, &candidate.cli, &candidate.model);
                    held.guard.degrade().await;
                    let now_ms = now_millis_u64();
                    let default_exp = now_ms.saturating_add(DEFAULT_COOLDOWN_SECS * 1000);
                    fallback_expires_at = Some(match &err {
                        FlowError::Claude(ClaudeError::UsageLimit(limit)) if limit.resets_at > 0 => {
                            let reset_ms = limit.resets_at.saturating_mul(1000);
                            if reset_ms > now_ms { reset_ms } else { default_exp }
                        }
                        _ => default_exp,
                    });
                    fallback_reason = Some(if is_limit {
                        OverrideReason::UsageLimit
                    } else {
                        OverrideReason::ModelUnavailable
                    });
                    continue;
                }
                if let FlowError::Claude(ClaudeError::ModelUnavailable {
                    provider,
                    model,
                    detail,
                }) = &err
                {
                    let _ = emit(
                        app,
                        VokiaEvent::ModelUnavailable {
                            agent: agent.clone(),
                            provider: provider.clone(),
                            model: model.clone(),
                            detail: detail.clone(),
                        },
                    );
                }
                return Err(err);
            }
        }
    }
    Err(FlowError::CliUnavailable(format!(
        "Nenhum provedor disponível para {agent}."
    )))
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
    access: Access<'_>,
    gate: PermissionGate<'_>,
    pid: &AtomicU32,
    call: &AgentCall,
    message: &str,
    console: &ConsoleSink<'_>,
) -> Result<Reply, ClaudeError> {
    call.adapter
        .run_fresh(access, gate, pid, call, message, console)
        .await
}

fn encode_provider_session(
    cli: &str,
    model: &str,
    item_id: Option<&str>,
    session_id: &str,
) -> String {
    format!("{}:{}:{}:{session_id}", cli, model, item_id.unwrap_or(""))
}

fn decode_provider_session(
    stored: &str,
    cli: &str,
    model: &str,
    item_id: Option<&str>,
) -> Option<String> {
    let prefix = format!("{}:{}:{}:", cli, model, item_id.unwrap_or(""));
    stored
        .strip_prefix(&prefix)
        .map(str::trim)
        .filter(|session_id| !session_id.is_empty())
        .map(str::to_string)
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

    pub(super) fn for_preload(dir: PathBuf) -> Self {
        Self { dir, item_id: None }
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
    pub(super) provider_key: Option<(String, String)>,
    pub(super) preloaded_at: Option<Instant>,
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
        project: ProjectAccess<'_>,
        gate: PermissionGate<'_>,
        store: &SessionStore<'_>,
        call: &AgentCall,
        prompts: &StepPrompts,
        first: bool,
        console: &ConsoleSink<'_>,
    ) -> Result<Option<Reply>, FlowError> {
        let place = Place::of(project, call);
        let provider_key = (call.adapter.config().id.clone(), call.model.clone());
        let mut next = self.first_attempt(store, &place, &provider_key).await?;
        while let Some(attempt) = next {
            let was_unused_preload = self.is_unused_preload();
            match self
                .attempt(
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
                Err(error)
                    if error.is_session_loss()
                        && (attempt != Attempt::Reuse || was_unused_preload) =>
                {
                    if attempt == Attempt::Resume || was_unused_preload {
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
        provider_key: &(String, String),
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
        if self
            .provider_key
            .as_ref()
            .is_some_and(|known| known != provider_key)
        {
            self.close_live().await;
        }
        if self.live.is_none() {
            if self.session_id.is_none() {
                self.session_id = store.load()?.filter(|id| !id.contains(':'));
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
                    &call.program,
                    &call.user_args,
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
                self.preloaded_at = None;
                self.provider_key = Some((call.adapter.config().id.clone(), call.model.clone()));
                Ok(reply)
            }
            Err(error) => {
                live.close(SESSION_CLOSE_GRACE).await;
                self.pid.store(NO_PID, Ordering::SeqCst);
                self.provider_key = None;
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

    pub(super) async fn close_live(&mut self) {
        if let Some(live) = self.live.take() {
            live.close(SESSION_CLOSE_GRACE).await;
            self.pid.store(NO_PID, Ordering::SeqCst);
            self.provider_key = None;
            self.preloaded_at = None;
        }
    }

    pub(super) fn adopt_preload(
        &mut self,
        live: LiveSession,
        session_id: Option<String>,
        place: Place,
        provider_key: (String, String),
    ) {
        self.pid
            .store(live.pid().unwrap_or(NO_PID), Ordering::SeqCst);
        self.live = Some(live);
        self.session_id = session_id;
        self.messages = 0;
        self.last_used = None;
        self.preloaded_at = Some(Instant::now());
        self.place = Some(place);
        self.provider_key = Some(provider_key);
    }

    pub(super) fn is_unused_preload(&self) -> bool {
        self.live.is_some() && self.last_used.is_none()
    }
}

pub(super) fn preload_targets(
    orchestration: Option<&OrchestrationSnapshot>,
    team: &Team,
    paused: &[Agent],
) -> Vec<Agent> {
    let mut raw = vec![Agent::ember(), leader_of(orchestration)];
    for t in orchestration.map_or(&[][..], |s| s.teams.as_slice()) {
        if let Some(leader) = t.leader_agent.as_deref().and_then(Agent::from_stored) {
            raw.push(leader);
        }
    }
    let mut seen = HashSet::new();
    raw.into_iter()
        .filter(|agent| {
            team.profile(agent).is_some() && !paused.contains(agent) && seen.insert(agent.clone())
        })
        .collect()
}

pub(super) fn preload_candidate(
    agent: &Agent,
    profile: &AgentProfile,
    providers: &ProviderRegistry,
    state: &AppState,
    global_active: Option<&LlmTarget>,
    global_fallback: &[LlmTarget],
    permission_level: PermissionLevel,
) -> Option<ResolvedCandidate> {
    resolve_candidates(
        agent,
        profile,
        providers,
        state,
        global_active,
        global_fallback,
        Some(permission_level),
    )
    .into_iter()
    .next()
}

#[cfg(test)]
mod provider_session_tests {
    use super::*;
    use crate::agents::Agent;
    use crate::db::{DbState, NewProject};
    use crate::mcp::support;
    use tauri::Manager;

    #[test]
    fn tok4_provider_sessions_are_scoped_and_prefixed_ids_are_not_claude_resumes() {
        let session_id = "sid:with:colon";
        let stored = encode_provider_session("opencode", "model-a", Some("item-1"), session_id);

        assert_eq!(stored, "opencode:model-a:item-1:sid:with:colon");
        assert_eq!(
            decode_provider_session(&stored, "opencode", "model-a", Some("item-1")),
            Some(session_id.to_string())
        );
        assert_eq!(decode_provider_session(&stored, "opencode", "model-a", Some("item-2")), None);
        assert_eq!(decode_provider_session(&stored, "opencode", "model-a", None), None);
        assert_eq!(decode_provider_session(&stored, "antigravity", "model-a", Some("item-1")), None);
        assert_eq!(decode_provider_session(&stored, "opencode", "model-b", Some("item-1")), None);

        let data_dir = std::env::temp_dir().join(format!(
            "vokia-claude-session-filter-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&data_dir).unwrap();
        let app = support::build_app(&data_dir);
        let project_id = app
            .state::<DbState>()
            .db()
            .expect("banco de teste")
            .insert_project(&NewProject {
                name: "sessao",
                symbol: "folder",
                description: "",
                path: "C:/test/project",
                origin_kind: "folder",
                origin_repo: None,
            })
            .expect("projeto de teste")
            .id;
        let store = SessionStore {
            app: app.handle(),
            key: SessionKey { project_id, agent: Agent::ember() },
        };
        store.save(&stored).expect("sessão prefixada deve ser gravada");
        let mut session = AgentSession::default();
        let place = Place::for_preload(PathBuf::from("C:/test/project"));
        let provider_key = ("claude".to_string(), "model-a".to_string());

        let attempt = support::run(session.first_attempt(&store, &place, &provider_key))
            .expect("o filtro de sessão Claude deve ser avaliado");

        assert!(matches!(attempt, Some(Attempt::Open)));
        assert_eq!(session.session_id, None);
        drop(store);
        drop(app);
        let _ = std::fs::remove_dir_all(&data_dir);
    }
}

pub(super) type PreloadedSession = (LiveSession, Option<String>, Place, (String, String));

#[allow(clippy::too_many_arguments)]
pub(super) fn open_preloaded_session(
    app: &AppHandle,
    project_id: i64,
    project_dir: &Path,
    attachments_dir: &Path,
    permission_level: PermissionLevel,
    agent: &Agent,
    profile: &AgentProfile,
    team: &Team,
    orchestration: Option<&OrchestrationSnapshot>,
    paused: &[Agent],
    project_context: &str,
    candidate: &ResolvedCandidate,
    cli: &Path,
) -> Result<PreloadedSession, FlowError> {
    let mcp = if candidate.provider.has_tools() {
        McpLaunch::new(app, project_id, agent, &display_name(team, agent))?
    } else {
        None
    };
    let tool_mode = if mcp.is_some() {
        ToolPromptMode::Mcp
    } else {
        ToolPromptMode::None
    };
    let prompt = agent_prompt(orchestration, team, agent);
    let base = role(
        profile,
        &prompt.roster,
        prompt.layer.as_ref(),
        project_context,
        tool_mode,
    );
    let role_text = if agent == &Agent::ember() {
        let tools = if tool_mode != ToolPromptMode::None {
            tool_definitions()
        } else {
            Vec::new()
        };
        maestro_role(
            &base,
            &tools,
            &project_choices(app)?,
            None,
            orchestration,
            team,
            paused,
            tool_mode,
        )
    } else {
        base
    };
    let (program, user_args) = resolve_program_for(agent, &candidate.command, cli)?;
    let store = SessionStore {
        app,
        key: SessionKey {
            project_id,
            agent: agent.clone(),
        },
    };
    let resume = store.load()?;
    let mcp_session = mcp.as_ref().map(McpLaunch::open).transpose()?;
    let access = Access::Project(ProjectAccess {
        dir: project_dir,
        attachments: attachments_dir,
        level: permission_level,
    });
    let live = LiveSession::open(
        &program,
        &user_args,
        &role_text,
        &candidate.model,
        access,
        resume.as_deref(),
        mcp_session,
    )?;
    let place = Place::for_preload(project_dir.to_path_buf());
    let provider_key = (candidate.cli.clone(), candidate.model.clone());
    Ok((live, resume, place, provider_key))
}

pub(super) struct AgentCall {
    pub(super) adapter: ProviderAdapter,
    pub(super) program: PathBuf,
    pub(super) user_args: Vec<String>,
    pub(super) role: String,
    pub(super) model: String,
    pub(super) resume_session: Option<String>,
    item_id: Option<String>,
    channel_id: Option<i64>,
    mcp: Option<McpLaunch>,
    has_variables: bool,
    collected: StdMutex<ExchangeOutcome>,
}

impl AgentCall {
    pub(super) fn for_text_ask(
        adapter: ProviderAdapter,
        program: PathBuf,
        user_args: Vec<String>,
        role: String,
        model: String,
    ) -> Self {
        Self {
            adapter,
            program,
            user_args,
            role,
            model,
            resume_session: None,
            item_id: None,
            channel_id: None,
            mcp: None,
            has_variables: false,
            collected: StdMutex::default(),
        }
    }

    pub(super) fn has_mcp(&self) -> bool {
        self.mcp.is_some()
    }

    pub(super) fn begin_mcp_slot(&self, session: &McpSession) {
        let slot = session.slot();
        slot.begin(self.item_id.as_deref(), self.channel_id);
        if self.has_variables {
            slot.enable_variables();
        }
    }

    pub(super) fn finish_mcp_slot(&self, session: &McpSession) {
        let outcome = session.slot().end();
        if let Ok(mut collected) = self.collected.lock() {
            *collected = outcome;
        }
    }
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

    fn open_headless(&self) -> Result<McpSession, ClaudeError> {
        let binding = Binding {
            project_id: self.project_id,
            agent: self.agent.clone(),
            agent_name: self.agent_name.clone(),
            slot: ExchangeSlot::default(),
        };
        self.server
            .open_headless(binding)
            .map_err(|error| ClaudeError::Mcp(error.to_string()))
    }
}

pub(super) fn open_mcp(call: &AgentCall) -> Result<Option<McpSession>, ClaudeError> {
    call.mcp.as_ref().map(McpLaunch::open).transpose()
}

pub(super) fn open_mcp_headless(call: &AgentCall) -> Result<Option<McpSession>, ClaudeError> {
    call.mcp.as_ref().map(McpLaunch::open_headless).transpose()
}

pub(super) async fn send_in_slot(
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
