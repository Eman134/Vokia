use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU32;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::agents::{Agent, AgentProfile, Team};
use crate::claude::{self, Access, Reply};
use crate::db::{DbState, SETTING_ACTIVE_LLM, SETTING_LLM_FALLBACK};
use crate::events::VokiaEvent;
use crate::permissions::{PermissionBroker, PermissionGate, PermissionLevel};
use crate::providers::{
    ModelTier, ProviderConfig, ProviderProtocol, ProviderRegistry, ToolTransport,
};
use crate::state::AppState;

use super::adapters::ProviderAdapter;
use super::console::ConsoleSink;
use super::emit::emit;
use super::error::FlowError;

pub(crate) const DEFAULT_COOLDOWN_SECS: u64 = 120;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LlmTarget {
    pub cli: String,
    #[serde(default, alias = "tier")]
    pub model_tier: Option<ModelTier>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_cooldown_secs() -> u64 {
    DEFAULT_COOLDOWN_SECS
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LlmFallbackSetting {
    Config {
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_cooldown_secs", rename = "cooldownSecs")]
        cooldown_secs: u64,
        #[serde(default)]
        chain: Vec<LlmTarget>,
    },
    List(Vec<LlmTarget>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverrideReason {
    UsageLimit,
    ModelUnavailable,
    ManualTemp,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporaryOverride {
    pub agent: Agent,
    pub cli: String,
    pub model: String,
    #[serde(default)]
    pub model_tier: Option<ModelTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    pub command: String,
    pub original_cli: String,
    pub original_model: String,
    #[serde(default)]
    pub original_tier: Option<ModelTier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_reasoning_effort: Option<String>,
    pub reason: OverrideReason,
    pub applied_at: u64,
    #[serde(default)]
    pub expires_at: Option<u64>,
}

#[derive(Default)]
pub struct CooldownTracker {
    entries: HashMap<(String, String), Instant>,
}

impl CooldownTracker {
    pub fn mark(&mut self, cli: &str, model: &str, duration: Duration) {
        self.entries
            .insert((cli.to_string(), model.to_string()), Instant::now() + duration);
    }

    pub fn is_cooling(&mut self, cli: &str, model: &str) -> bool {
        let now = Instant::now();
        self.entries.retain(|_, until| *until > now);
        self.entries
            .get(&(cli.to_string(), model.to_string()))
            .is_some_and(|until| *until > now)
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedCandidate {
    pub cli: String,
    pub model: String,
    pub model_tier: Option<ModelTier>,
    pub reasoning_effort: Option<String>,
    pub command: String,
    pub provider: ProviderConfig,
}

pub(crate) fn now_millis_u64() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

pub fn load_global_active_llm(app: &AppHandle) -> Option<LlmTarget> {
    let db_state = app.state::<DbState>();
    let db = db_state.db()?;
    let settings = db.load_settings().ok()?;
    let raw = settings.get(SETTING_ACTIVE_LLM)?;
    serde_json::from_str::<LlmTarget>(raw).ok()
}

pub fn load_global_fallback(app: &AppHandle) -> Vec<LlmTarget> {
    let db_state = app.state::<DbState>();
    let Some(db) = db_state.db() else {
        return Vec::new();
    };
    let Ok(settings) = db.load_settings() else {
        return Vec::new();
    };
    let Some(raw) = settings.get(SETTING_LLM_FALLBACK) else {
        return Vec::new();
    };
    match serde_json::from_str::<LlmFallbackSetting>(raw) {
        Ok(LlmFallbackSetting::Config { enabled, chain, .. }) => {
            if enabled {
                chain
            } else {
                Vec::new()
            }
        }
        Ok(LlmFallbackSetting::List(chain)) => chain,
        Err(_) => Vec::new(),
    }
}

pub(crate) fn resolve_model_for_provider(
    provider: &ProviderConfig,
    tier: Option<ModelTier>,
    explicit_model: &str,
) -> String {
    let trimmed = explicit_model.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }
    if let Some(tier) = tier {
        let from_tier = provider.tiers.get(tier).trim();
        if !from_tier.is_empty() {
            return from_tier.to_string();
        }
    }
    provider.default_model.clone()
}

pub fn resolve_candidates(
    agent: &Agent,
    profile: &AgentProfile,
    providers: &ProviderRegistry,
    state: &AppState,
    global_active: Option<&LlmTarget>,
    global_fallback: &[LlmTarget],
    permission_level: Option<PermissionLevel>,
) -> Vec<ResolvedCandidate> {
    let now_ms = now_millis_u64();
    let released = state.usage.release_expired_limits(now_ms);
    let active_override = {
        let mut overrides = state
            .temporary_overrides
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for ret in &released.returns {
            let should_remove = overrides.get(&ret.agent).is_some_and(|ov| {
                ov.reason == OverrideReason::UsageLimit
                    && ov.original_cli == ret.original_provider_id
            });
            if should_remove {
                overrides.remove(&ret.agent);
            }
        }
        let expired = overrides
            .get(agent)
            .and_then(|ov| ov.expires_at)
            .is_some_and(|exp| now_ms >= exp);
        if expired {
            overrides.remove(agent);
        }
        overrides.get(agent).cloned()
    };

    let mut raw_targets: Vec<(
        String,
        Option<ModelTier>,
        String,
        Option<String>,
        Option<String>,
    )> = Vec::new();

    if let Some(ov) = active_override {
        raw_targets.push((
            ov.cli,
            ov.model_tier,
            ov.model,
            ov.reasoning_effort,
            Some(ov.command),
        ));
    }

    if profile.follow_default_llm {
        if let Some(global) = global_active {
            raw_targets.push((
                global.cli.clone(),
                global.model_tier,
                global.model.clone().unwrap_or_default(),
                global.reasoning_effort.clone(),
                None,
            ));
        }
    }

    raw_targets.push((
        profile.cli.clone(),
        profile.model_tier,
        profile.model.clone(),
        profile.reasoning_effort.clone(),
        Some(profile.command.clone()),
    ));

    for target in &profile.fallback {
        raw_targets.push((
            target.cli.clone(),
            target.model_tier,
            target.model.clone().unwrap_or_default(),
            target.reasoning_effort.clone(),
            None,
        ));
    }
    for target in global_fallback {
        raw_targets.push((
            target.cli.clone(),
            target.model_tier,
            target.model.clone().unwrap_or_default(),
            target.reasoning_effort.clone(),
            None,
        ));
    }

    let is_ember = agent == &Agent::ember();
    let requires_sandbox = permission_level == Some(PermissionLevel::Liberado);
    let mut cooldowns = state.cooldowns.lock().unwrap_or_else(|e| e.into_inner());
    let mut seen: HashSet<(String, String)> = HashSet::new();
    let mut result = Vec::new();

    for (cli, tier, model_raw, reasoning_effort, custom_cmd) in raw_targets {
        let Ok(provider) = providers.get(&cli) else {
            continue;
        };
        if !provider.enabled {
            continue;
        }
        if is_ember && !provider.has_tools() {
            continue;
        }
        if requires_sandbox && !provider.supports_sandbox {
            continue;
        }
        let base_model = resolve_model_for_provider(&provider, tier, &model_raw);
        let model = provider.resolve_model_with_effort(&base_model, reasoning_effort.as_deref());
        if !seen.insert((cli.clone(), model.clone())) {
            continue;
        }
        if cooldowns.is_cooling(&cli, &model) {
            continue;
        }
        if state.usage.is_model_limited(&cli, &model, now_ms)
            && !state.usage.is_provider_using_credits(&cli)
        {
            continue;
        }
        let command = custom_cmd
            .filter(|cmd| !cmd.trim().is_empty())
            .unwrap_or_else(|| provider.default_command.clone());
        result.push(ResolvedCandidate {
            cli,
            model,
            model_tier: tier,
            reasoning_effort,
            command,
            provider,
        });
    }

    result
}

pub(crate) fn effective_tool_transport_for(
    app: &AppHandle,
    state: &AppState,
    team: &Team,
    agent: &Agent,
    permission_level: Option<PermissionLevel>,
) -> ToolTransport {
    let Some(profile) = team.profile(agent) else {
        return ToolTransport::None;
    };
    let providers = app.state::<ProviderRegistry>();
    let global_active = load_global_active_llm(app);
    let global_fallback = load_global_fallback(app);
    let candidates = resolve_candidates(
        agent,
        profile,
        &providers,
        state,
        global_active.as_ref(),
        &global_fallback,
        permission_level,
    );
    if let Some(first) = candidates.first() {
        return first.provider.effective_tool_transport();
    }
    providers
        .get(&profile.cli)
        .map(|provider| provider.effective_tool_transport())
        .unwrap_or_default()
}

pub fn mark_cooldown(state: &AppState, cli: &str, model: &str) {
    let mut cooldowns = state.cooldowns.lock().unwrap_or_else(|e| e.into_inner());
    cooldowns.mark(cli, model, Duration::from_secs(DEFAULT_COOLDOWN_SECS));
}

pub fn emit_overrides_changed(app: &AppHandle, state: &AppState) {
    let overrides: Vec<TemporaryOverride> = state
        .temporary_overrides
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .cloned()
        .collect();
    let _ = emit(app, VokiaEvent::TemporaryOverridesChanged { overrides });
}

/// Desativa automaticamente um provedor com erro de assinatura/pagamento/autenticação,
/// limpa overrides e ativo global que apontavam para ele e notifica a interface.
pub fn handle_provider_auth_or_billing(
    app: &AppHandle,
    state: &AppState,
    provider_id: &str,
    reason: &str,
) {
    let label = if let Some(providers) = app.try_state::<ProviderRegistry>() {
        match providers.set_enabled(provider_id, false, Some(reason)) {
            Ok(updated) => updated.label,
            Err(_) => provider_id.to_string(),
        }
    } else {
        provider_id.to_string()
    };

    let cleared_override = {
        let mut overrides = state
            .temporary_overrides
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let before = overrides.len();
        overrides.retain(|_, ov| ov.cli != provider_id && ov.original_cli != provider_id);
        overrides.len() != before
    };
    if cleared_override {
        emit_overrides_changed(app, state);
    }

    if let Some(db_state) = app.try_state::<DbState>() {
        if let Some(db) = db_state.db() {
            if let Ok(settings) = db.load_settings() {
                if let Some(raw) = settings.get(SETTING_ACTIVE_LLM) {
                    if let Ok(target) = serde_json::from_str::<LlmTarget>(raw) {
                        if target.cli == provider_id {
                            let _ = db.save_setting(SETTING_ACTIVE_LLM, "");
                        }
                    }
                }
            }
        }
    }

    let _ = emit(
        app,
        VokiaEvent::ProviderDisabled {
            provider: provider_id.to_string(),
            label,
            reason: reason.to_string(),
        },
    );
    let _ = emit(app, VokiaEvent::ProvidersChanged);
}

/// Releases expired `(provider_id, model)` limits and restores any agents whose
/// `UsageLimit` temporary overrides were waiting for quota renewal.
pub async fn apply_released_limit_returns(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let now_ms = now_millis_u64();
    let released = state.usage.release_expired_limits(now_ms);
    for (provider_id, _) in &released.released_keys {
        state.usage.invalidate_provider(provider_id);
    }
    let mut restored_agents: Vec<Agent> = Vec::new();
    if !released.returns.is_empty() {
        let mut overrides = state
            .temporary_overrides
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        for ret in released.returns {
            state.usage.invalidate_provider(&ret.original_provider_id);
            let should_remove = overrides.get(&ret.agent).is_some_and(|ov| {
                ov.reason == OverrideReason::UsageLimit
                    && ov.original_cli == ret.original_provider_id
            });
            if should_remove {
                overrides.remove(&ret.agent);
                restored_agents.push(ret.agent);
            }
        }
    }
    for agent in &restored_agents {
        state.sessions.retire_agent(agent).await;
    }
    if !restored_agents.is_empty() {
        emit_overrides_changed(app, &state);
    }
}

/// Schedules an automatic timer to fire at `release_at_ms` (`resetsAt + 30s` or 15 min recheck)
/// so agents return to their original model once the quota window renews.
pub fn schedule_limit_return(app: AppHandle, release_at_ms: u64) {
    let now_ms = now_millis_u64();
    let wait_ms = release_at_ms.saturating_sub(now_ms);
    tauri::async_runtime::spawn(async move {
        if wait_ms > 0 {
            tokio::time::sleep(Duration::from_millis(wait_ms)).await;
        }
        apply_released_limit_returns(&app).await;
    });
}

pub fn resolve_program_for(
    agent: &Agent,
    command: &str,
    default_claude_cli: &Path,
) -> Result<(PathBuf, Vec<String>), FlowError> {
    let parts = shlex::split(command.trim()).ok_or_else(|| {
        FlowError::CliUnavailable(format!("Comando inválido para {agent}: {command}."))
    })?;
    let Some((bin, args)) = parts.split_first() else {
        return Err(FlowError::CliUnavailable(format!(
            "Comando vazio para {agent}."
        )));
    };
    if agent == &Agent::ember() && bin == "claude" && args.is_empty() {
        return Ok((default_claude_cli.to_path_buf(), Vec::new()));
    }
    let program = which::which(bin).map_err(|_| {
        FlowError::CliUnavailable(format!("CLI {bin} não encontrada (comando: {command})."))
    })?;
    Ok((program, args.to_vec()))
}

pub(crate) async fn ask_with_fallback(
    app: &AppHandle,
    state: &AppState,
    default_cli: &Path,
    agent: &Agent,
    profile: &AgentProfile,
    message: &str,
    role: &str,
) -> Result<Reply, FlowError> {
    ask_with_fallback_in_access(
        app,
        state,
        default_cli,
        agent,
        profile,
        message,
        role,
        Access::TextOnly,
        0,
    )
    .await
}

pub(crate) async fn ask_project_readonly_with_fallback(
    app: &AppHandle,
    state: &AppState,
    default_cli: &Path,
    agent: &Agent,
    profile: &AgentProfile,
    project_id: i64,
    project_dir: &Path,
    message: &str,
    role: &str,
) -> Result<Reply, FlowError> {
    let access = Access::Project(crate::claude::ProjectAccess {
        dir: project_dir,
        attachments: project_dir,
        extra_dirs: &[],
        level: crate::permissions::PermissionLevel::Leitura,
        builtin_tools: true,
    });
    ask_with_fallback_in_access(
        app,
        state,
        default_cli,
        agent,
        profile,
        message,
        role,
        access,
        project_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn ask_with_fallback_in_access(
    app: &AppHandle,
    state: &AppState,
    default_cli: &Path,
    agent: &Agent,
    profile: &AgentProfile,
    message: &str,
    role: &str,
    access: Access<'_>,
    project_id: i64,
) -> Result<Reply, FlowError> {
    let providers = app.state::<ProviderRegistry>();
    let global_active = load_global_active_llm(app);
    let global_fallback = load_global_fallback(app);
    let candidates = resolve_candidates(
        agent,
        profile,
        &providers,
        state,
        global_active.as_ref(),
        &global_fallback,
        None,
    );
    if candidates.is_empty() {
        return Err(FlowError::CliUnavailable(
            "Nenhum provedor ativo disponível para executar esta ação.".to_string(),
        ));
    }

    let mut last_error: Option<FlowError> = None;
    for candidate in candidates {
        let res: Result<Reply, FlowError> = match (candidate.provider.protocol, access) {
            (ProviderProtocol::ClaudeLive, Access::TextOnly) => {
                let (program, _) =
                    resolve_program_for(agent, &candidate.command, default_cli)?;
                claude::ask(&program, message, role, &candidate.model)
                    .await
                    .map_err(FlowError::Claude)
            }
            _ => {
                let (program, mut user_args) =
                    resolve_program_for(agent, &candidate.command, default_cli)?;
                user_args.extend(candidate.provider.resolve_effort_args(
                    &candidate.model,
                    candidate.reasoning_effort.as_deref(),
                ));
                let adapter = ProviderAdapter::from_config(candidate.provider.clone());
                let broker = PermissionBroker::default();
                let granted = std::sync::Arc::new(std::sync::Mutex::new(HashSet::new()));
                let gate = PermissionGate {
                    app,
                    broker: &broker,
                    agent: agent.clone(),
                    project_id,
                    item_id: None,
                    granted: &granted,
                    grant_per_session: false,
                };
                let pid = AtomicU32::new(0);
                let console = ConsoleSink::new(
                    app,
                    crate::session_key::SessionKey {
                        project_id,
                        agent: agent.clone(),
                    },
                    None,
                );
                let call = super::agent_session::AgentCall::for_text_ask(
                    adapter.clone(),
                    program,
                    user_args,
                    role.to_string(),
                    candidate.model.clone(),
                );
                adapter
                    .run_fresh(
                        access,
                        gate,
                        &pid,
                        &call,
                        message,
                        &console,
                        state.sessions.adapters(),
                    )
                    .await
                    .map_err(FlowError::Claude)
            }
        };

        match res {
            Ok(reply) => return Ok(reply),
            Err(error) if error.is_provider_disabled() => {
                let reason = error
                    .auth_or_billing()
                    .map(|(_, r)| r)
                    .unwrap_or_else(|| "Falha de autenticação ou assinatura.".to_string());
                handle_provider_auth_or_billing(app, state, &candidate.cli, &reason);
                mark_cooldown(state, &candidate.cli, &candidate.model);
                last_error = Some(FlowError::ProviderDisabled {
                    provider: candidate.cli,
                    reason,
                });
            }
            Err(error) if error.is_limit() => {
                let resets_at = error.usage_limit().map(|l| l.resets_at).filter(|&r| r > 0);
                let entry = state.usage.mark_limit_reached(
                    &candidate.cli,
                    &candidate.model,
                    resets_at,
                    now_millis_u64(),
                );
                schedule_limit_return(app.clone(), entry.release_at_ms);
                mark_cooldown(state, &candidate.cli, &candidate.model);
                last_error = Some(error);
            }
            Err(error) if error.is_model_unavailable() => {
                mark_cooldown(state, &candidate.cli, &candidate.model);
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        FlowError::CliUnavailable("Nenhum provedor disponível na cadeia de fallback.".to_string())
    }))
}
