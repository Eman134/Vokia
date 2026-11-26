pub(crate) mod acp;
#[cfg(test)]
pub(crate) mod antigravity;
#[cfg(not(test))]
mod antigravity;
mod claude;
#[cfg(test)]
pub(crate) mod codex;
#[cfg(not(test))]
mod codex;
pub(crate) mod codex_app_server;
mod generic;
mod opencode;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::agents::Agent;
use crate::claude::{
    classify_provider_error, cli_message, kill_process_tree, timeout_for, Access, ClaudeError,
    Reply,
};
use crate::mcp::McpSession;
use crate::permissions::{PermissionGate, PermissionLevel};
use crate::providers::{ProviderConfig, ProviderProtocol, ProviderRegistry, ToolTransport};
use crate::session_key::SessionKey;
use crate::usage::model::ProviderPlanUsage;

use super::agent_session::{
    decode_provider_session, encode_provider_session, AgentCall, AgentSession, SessionStore,
};
use super::console::ConsoleSink;
use super::error::FlowError;
use super::prompts::StepPrompts;
use super::session_pool::NO_PID;

/// Pool genérico de runtimes persistentes dos adaptadores de provedores.
/// Mantém detalhes específicos de cada protocolo encapsulados atrás da camada de adaptadores.
#[derive(Default)]
pub(crate) struct AdapterRuntimePool {
    codex: codex_app_server::CodexIdleSessions,
    acp: acp::AcpRuntimePool,
}

impl AdapterRuntimePool {
    pub(crate) async fn reap_idle(&self, is_externally_busy: impl Fn(&SessionKey) -> bool) {
        let _ = self
            .codex
            .reap_idle(Instant::now(), &is_externally_busy)
            .await;
        let _ = self
            .acp
            .reap_idle(Instant::now(), is_externally_busy)
            .await;
    }

    pub(crate) async fn archive_project(&self, project_id: i64) {
        let _ = self.codex.archive_project(project_id).await;
        self.acp.archive_project(project_id).await;
    }

    pub(crate) async fn archive_agent(&self, agent: &Agent) {
        let _ = self.codex.archive_agent(agent).await;
        self.acp.archive_agent(agent).await;
    }

    pub(crate) async fn archive_worktree(
        &self,
        project_id: i64,
        item_id: &str,
        worktree_path: Option<&Path>,
    ) {
        let _ = self
            .codex
            .archive_worktree(project_id, item_id, worktree_path)
            .await;
        self.acp
            .archive_worktree(project_id, item_id, worktree_path)
            .await;
    }

    pub(crate) async fn archive_all(&self) {
        let _ = self.codex.archive_all().await;
        self.acp.archive_all().await;
    }

    pub(crate) fn shutdown_all(&self) {
        self.codex.shutdown_all_sync();
        self.acp.shutdown_all_sync();
    }

    pub(crate) async fn steer_turn(
        &self,
        key: &SessionKey,
        message: &str,
    ) -> Result<bool, FlowError> {
        if self
            .codex
            .steer_turn(key, message)
            .await
            .map_err(FlowError::Claude)?
        {
            return Ok(true);
        }
        self.acp
            .steer_turn(key, message)
            .await
            .map_err(FlowError::Claude)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn interrupt_turn(&self, key: &SessionKey) -> Result<bool, FlowError> {
        if self
            .codex
            .interrupt_turn(key)
            .await
            .map_err(FlowError::Claude)?
        {
            return Ok(true);
        }
        self.acp
            .interrupt_turn(key)
            .await
            .map_err(FlowError::Claude)
    }

    #[allow(dead_code)]
    pub(crate) fn acp_session_status(&self, key: &SessionKey) -> acp::AcpSessionStatus {
        self.acp.session_status(key)
    }

    #[allow(dead_code)]
    pub(crate) async fn restart_acp_session(&self, key: &SessionKey) -> bool {
        self.acp.restart_session(key).await
    }

    #[allow(dead_code)]
    pub(crate) async fn discover_models(
        &self,
        protocol: ProviderProtocol,
        program: &Path,
        user_args: &[String],
    ) -> Result<Option<Vec<String>>, String> {
        self.discover_models_for_provider(protocol, program, user_args, None)
            .await
    }

    pub(crate) async fn discover_models_for_provider(
        &self,
        protocol: ProviderProtocol,
        program: &Path,
        user_args: &[String],
        provider: Option<&ProviderConfig>,
    ) -> Result<Option<Vec<String>>, String> {
        match protocol {
            ProviderProtocol::CodexAppServer => self
                .codex
                .list_models(program, user_args)
                .await
                .map(Some)
                .map_err(|err| err.to_string()),
            ProviderProtocol::Acp => self
                .acp
                .discover_models(program, user_args, provider)
                .await
                .map(Some)
                .map_err(|err| err.to_string()),
            _ => Ok(None),
        }
    }

    pub(crate) async fn read_rate_limits(
        &self,
        protocol: ProviderProtocol,
        program: &Path,
        user_args: &[String],
        now_ms: u64,
    ) -> Result<Option<ProviderPlanUsage>, String> {
        match protocol {
            ProviderProtocol::CodexAppServer => self
                .codex
                .read_rate_limits(program, user_args, now_ms)
                .await
                .map_err(|err| err.to_string()),
            _ => Ok(None),
        }
    }
}

pub(crate) async fn test_persistent_provider(
    config: &ProviderConfig,
    program: &Path,
    user_args: &[String],
    model: &str,
    limit: std::time::Duration,
) -> Option<Result<String, ClaudeError>> {
    match config.protocol {
        ProviderProtocol::CodexAppServer => Some(
            codex_app_server::test_connection(program, user_args, model, config, limit).await,
        ),
        ProviderProtocol::Acp => {
            let pool = acp::AcpRuntimePool::default();
            Some(pool.test_connection(program, user_args, model, config, limit).await)
        }
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::orchestrator) enum ProviderAdapter {
    Claude(ProviderConfig),
    CodexAppServer(ProviderConfig),
    Codex(ProviderConfig),
    Acp(ProviderConfig),
    OpenCode(ProviderConfig),
    Antigravity(ProviderConfig),
    Generic(Box<ProviderConfig>),
}

impl ProviderAdapter {
    pub(in crate::orchestrator) fn from_config(config: ProviderConfig) -> Self {
        match config.protocol {
            ProviderProtocol::ClaudeLive => Self::Claude(config),
            ProviderProtocol::CodexAppServer => Self::CodexAppServer(config),
            ProviderProtocol::CodexJsonl => Self::Codex(config),
            ProviderProtocol::Acp => Self::Acp(config),
            ProviderProtocol::OpenCodeJson => Self::OpenCode(config),
            ProviderProtocol::AntigravityStream => Self::Antigravity(config),
            ProviderProtocol::GenericCli => Self::Generic(Box::new(config)),
        }
    }

    pub(in crate::orchestrator) fn from_cli(
        cli: &str,
        providers: &ProviderRegistry,
    ) -> Result<Self, FlowError> {
        let config = providers.get(cli).map_err(|_| {
            FlowError::CliUnavailable(format!("Provedor de CLI desconhecido: {cli}."))
        })?;
        Ok(Self::from_config(config))
    }

    pub(in crate::orchestrator) fn config(&self) -> &ProviderConfig {
        match self {
            Self::Claude(config)
            | Self::CodexAppServer(config)
            | Self::Codex(config)
            | Self::Acp(config)
            | Self::OpenCode(config)
            | Self::Antigravity(config) => config,
            Self::Generic(config) => config,
        }
    }

    pub(in crate::orchestrator) fn is_claude(&self) -> bool {
        matches!(self, Self::Claude(_))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::orchestrator) fn supports_mcp(&self) -> bool {
        self.config().supports_mcp
    }

    pub(in crate::orchestrator) fn tool_transport(&self) -> ToolTransport {
        self.config().effective_tool_transport()
    }

    pub(in crate::orchestrator) fn has_tools(&self) -> bool {
        self.config().has_tools()
    }

    pub(in crate::orchestrator) fn supports_live_session(&self) -> bool {
        matches!(self, Self::Claude(_) | Self::CodexAppServer(_) | Self::Acp(_))
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::orchestrator) async fn run_live(
        &self,
        held: &mut AgentSession,
        access: Access<'_>,
        gate: PermissionGate<'_>,
        store: &SessionStore<'_>,
        call: &AgentCall,
        prompts: &StepPrompts,
        first: bool,
        console: &ConsoleSink<'_>,
        runtimes: &AdapterRuntimePool,
    ) -> Result<Option<Reply>, FlowError> {
        match (self, access) {
            (Self::Claude(_), Access::Project(project)) => {
                held.reply(project, gate, store, call, prompts, first, console)
                    .await
            }
            (Self::CodexAppServer(provider), _) => {
                let stored = store.load().ok().flatten();
                let persisted_thread_id = stored.as_deref().and_then(|raw| {
                    decode_provider_session(raw, &provider.id, &call.model, call.item_id())
                });
                let mcp_session = super::agent_session::open_mcp_headless(call)
                    .map_err(|err| FlowError::Claude(err.with_cli(&provider.id)))?;
                if let Some(session) = &mcp_session {
                    call.begin_mcp_slot(session);
                }
                let result = runtimes
                    .codex
                    .run_persistent_turn(
                        console.session_key().clone(),
                        call.item_id().map(str::to_string),
                        &call.program,
                        &call.user_args,
                        &call.role,
                        &call.model,
                        access,
                        &prompts.opening,
                        &prompts.follow_up,
                        persisted_thread_id.as_deref(),
                        Some(provider),
                        mcp_session.as_ref(),
                        Some(&gate),
                        &held.pid,
                        Some(console),
                    )
                    .await;
                if let Some(session) = &mcp_session {
                    call.finish_mcp_slot(session);
                }
                let reply = result.map_err(|err| FlowError::Claude(err.with_cli(&provider.id)))?;
                if let Some(thread_id) = reply.session_id.as_deref() {
                    store.save(&encode_provider_session(
                        &provider.id,
                        &call.model,
                        call.item_id(),
                        thread_id,
                    ))?;
                }
                held.provider_key = Some((provider.id.clone(), call.model.clone()));
                Ok(Some(reply))
            }
            (Self::Acp(provider), _) => {
                let stored = store.load().ok().flatten();
                let persisted_session_id = stored.as_deref().and_then(|raw| {
                    decode_provider_session(raw, &provider.id, &call.model, call.item_id())
                });
                let mcp_session = super::agent_session::open_mcp_headless(call)
                    .map_err(|err| FlowError::Claude(err.with_cli(&provider.id)))?;
                if let Some(session) = &mcp_session {
                    call.begin_mcp_slot(session);
                }
                let result = runtimes
                    .acp
                    .run_persistent_turn(
                        console.session_key().clone(),
                        call.item_id().map(str::to_string),
                        &call.program,
                        &call.user_args,
                        &call.role,
                        &call.model,
                        access,
                        &prompts.opening,
                        &prompts.follow_up,
                        persisted_session_id.as_deref(),
                        Some(provider),
                        mcp_session.as_ref(),
                        Vec::new(),
                        Some(&gate),
                        &held.pid,
                        Some(console),
                    )
                    .await;
                if let Some(session) = &mcp_session {
                    call.finish_mcp_slot(session);
                }
                let reply = result.map_err(|err| FlowError::Claude(err.with_cli(&provider.id)))?;
                if let Some(session_id) = reply.session_id.as_deref() {
                    store.save(&encode_provider_session(
                        &provider.id,
                        &call.model,
                        call.item_id(),
                        session_id,
                    ))?;
                }
                held.provider_key = Some((provider.id.clone(), call.model.clone()));
                Ok(Some(reply))
            }
            _ => Ok(None),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::orchestrator) async fn run_fresh(
        &self,
        access: Access<'_>,
        gate: PermissionGate<'_>,
        pid: &AtomicU32,
        call: &AgentCall,
        message: &str,
        console: &ConsoleSink<'_>,
        runtimes: &AdapterRuntimePool,
    ) -> Result<Reply, ClaudeError> {
        match self {
            Self::Claude(_) => claude::run_fresh(access, gate, pid, call, message, console).await,
            Self::CodexAppServer(provider) => {
                let mcp_session = super::agent_session::open_mcp_headless(call)?;
                if let Some(session) = &mcp_session {
                    call.begin_mcp_slot(session);
                }
                let result = runtimes
                    .codex
                    .run_ephemeral_turn(
                        &call.program,
                        &call.user_args,
                        &call.role,
                        &call.model,
                        access,
                        message,
                        Some(provider),
                        mcp_session.as_ref(),
                        Some(&gate),
                        pid,
                        Some(console),
                    )
                    .await;
                if let Some(session) = &mcp_session {
                    call.finish_mcp_slot(session);
                }
                result.map_err(|err| err.with_cli(&provider.id))
            }
            Self::Acp(provider) => {
                let mcp_session = super::agent_session::open_mcp_headless(call)?;
                if let Some(session) = &mcp_session {
                    call.begin_mcp_slot(session);
                }
                let result = runtimes
                    .acp
                    .run_ephemeral_turn(
                        &call.program,
                        &call.user_args,
                        &call.role,
                        &call.model,
                        access,
                        message,
                        Some(provider),
                        mcp_session.as_ref(),
                        Vec::new(),
                        Some(&gate),
                        pid,
                        Some(console),
                    )
                    .await;
                if let Some(session) = &mcp_session {
                    call.finish_mcp_slot(session);
                }
                result.map_err(|err| err.with_cli(&provider.id))
            }
            Self::Codex(provider) => {
                let mcp_session = super::agent_session::open_mcp_headless(call)?;
                if let Some(session) = &mcp_session {
                    call.begin_mcp_slot(session);
                }
                let result = codex::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                    provider,
                    mcp_session.as_ref(),
                    console,
                )
                .await;
                if let Some(session) = &mcp_session {
                    call.finish_mcp_slot(session);
                }
                result.map_err(|err| err.with_cli(&provider.id))
            }
            Self::OpenCode(provider) => {
                let vokia_cli = prepare_vokia_cli(self.tool_transport(), call)?;
                let vokia_ref = vokia_cli
                    .as_ref()
                    .map(|(session, path)| (session, path.as_path()));
                let result = opencode::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                    provider,
                    vokia_ref,
                    call.resume_session.as_deref(),
                    console,
                )
                .await;
                finalize_vokia_cli(result, access, call, vokia_cli)
                    .map_err(|err| err.with_cli(&provider.id))
            }
            Self::Antigravity(provider) => {
                let vokia_cli = prepare_vokia_cli(self.tool_transport(), call)?;
                let vokia_ref = vokia_cli
                    .as_ref()
                    .map(|(session, path)| (session, path.as_path()));
                let result = antigravity::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                    provider,
                    vokia_ref,
                    call.resume_session.as_deref(),
                    console,
                )
                .await;
                finalize_vokia_cli(result, access, call, vokia_cli)
                    .map_err(|err| err.with_cli(&provider.id))
            }
            Self::Generic(provider) => {
                let vokia_cli = prepare_vokia_cli(self.tool_transport(), call)?;
                let vokia_ref = vokia_cli
                    .as_ref()
                    .map(|(session, path)| (session, path.as_path()));
                let result = generic::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                    provider,
                    vokia_ref,
                )
                .await;
                finalize_vokia_cli(result, access, call, vokia_cli)
                    .map_err(|err| err.with_cli(&provider.id))
            }
        }
    }
}

fn prepare_vokia_cli(
    transport: ToolTransport,
    call: &AgentCall,
) -> Result<Option<(McpSession, PathBuf)>, ClaudeError> {
    if transport != ToolTransport::VokiaCli || !call.has_mcp() {
        return Ok(None);
    }
    let Some(cli_path) = resolve_vokia_cli_path() else {
        return Ok(None);
    };
    let Some(session) = super::agent_session::open_mcp_headless(call)? else {
        return Ok(None);
    };
    call.begin_mcp_slot(&session);
    Ok(Some((session, cli_path)))
}

fn finalize_vokia_cli(
    result: Result<Reply, ClaudeError>,
    access: Access<'_>,
    call: &AgentCall,
    vokia_cli: Option<(McpSession, PathBuf)>,
) -> Result<Reply, ClaudeError> {
    let used_vokia_cli = vokia_cli.is_some();
    if let Some((session, _)) = vokia_cli {
        call.finish_mcp_slot(&session);
    }
    if used_vokia_cli && matches!(access, Access::Project(p) if p.level == PermissionLevel::Leitura) {
        if let Err(err) = result {
            let msg = err.to_string();
            let lower = msg.to_ascii_lowercase();
            if lower.contains("127.0.0.1")
                || lower.contains("connection refused")
                || lower.contains("permission denied")
                || lower.contains("sandbox")
                || lower.contains("network")
            {
                return Err(ClaudeError::Reported(format!(
                    "{msg} (Em modo Leitura o sandbox pode bloquear a conexão local do vokia-cli; tente modo Edição ou um provedor com MCP nativo.)"
                )));
            }
            return Err(err);
        }
    }
    result
}

pub(crate) fn resolve_vokia_cli_path() -> Option<PathBuf> {
    static CLI_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
    CLI_PATH.get_or_init(locate_vokia_cli).clone()
}

fn locate_vokia_cli() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let bin = if cfg!(windows) {
        "vokia-cli.exe"
    } else {
        "vokia-cli"
    };
    let candidate = dir.join(bin);
    candidate.is_file().then_some(candidate)
}

pub(crate) fn inject_vokia_cli_env(
    command: &mut Command,
    session: &McpSession,
    cli_path: &Path,
) {
    command.env("VOKIA_URL", session.url());
    command.env("VOKIA_TOKEN", session.token());
    command.env("VOKIA_CLI", cli_path);
    if let Some(bin_dir) = cli_path.parent() {
        let existing = std::env::var_os("PATH").unwrap_or_default();
        let mut paths = vec![bin_dir.to_path_buf()];
        paths.extend(std::env::split_paths(&existing));
        if let Ok(joined) = std::env::join_paths(paths) {
            command.env("PATH", joined);
        }
    }
}

pub(crate) fn cli_has_tools(cli: &str, providers: &ProviderRegistry) -> bool {
    providers
        .get(cli)
        .is_ok_and(|provider| provider.has_tools())
}

pub(super) async fn run_cli_process<F>(
    mut command: Command,
    access: Access<'_>,
    pid: &AtomicU32,
    stdin_input: Option<String>,
    classify: Option<(&ProviderConfig, &str)>,
    mut on_line: F,
) -> Result<Vec<u8>, ClaudeError>
where
    F: FnMut(&str),
{
    if stdin_input.is_some() {
        command.stdin(Stdio::piped());
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().map_err(ClaudeError::Spawn)?;
    let child_pid = child.id();
    pid.store(child_pid.unwrap_or(NO_PID), Ordering::SeqCst);

    let limit = timeout_for(access);
    let io_future = async {
        if let Some(input) = stdin_input {
            if let Some(mut stdin) = child.stdin.take() {
                stdin
                    .write_all(input.as_bytes())
                    .await
                    .map_err(ClaudeError::Spawn)?;
                stdin.shutdown().await.map_err(ClaudeError::Spawn)?;
            }
        }
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_fut = async {
            let mut buf = Vec::new();
            if let Some(out) = stdout {
                let mut lines = BufReader::new(out).lines();
                loop {
                    match timeout(limit, lines.next_line()).await {
                        Ok(Ok(Some(line))) => {
                            on_line(&line);
                            buf.extend_from_slice(line.as_bytes());
                            buf.push(b'\n');
                        }
                        Ok(Ok(None)) => break,
                        Ok(Err(error)) => return Err(ClaudeError::Spawn(error)),
                        Err(_) => return Err(ClaudeError::Timeout(limit)),
                    }
                }
            }
            Ok::<Vec<u8>, ClaudeError>(buf)
        };

        let stderr_fut = async {
            let mut buf = Vec::new();
            if let Some(mut err) = stderr {
                err.read_to_end(&mut buf)
                    .await
                    .map_err(ClaudeError::Spawn)?;
            }
            Ok::<Vec<u8>, ClaudeError>(buf)
        };

        let (stdout_buf, stderr_buf) = tokio::try_join!(stdout_fut, stderr_fut)?;
        let status = timeout(limit, child.wait())
            .await
            .map_err(|_| ClaudeError::Timeout(limit))?
            .map_err(ClaudeError::Spawn)?;
        Ok::<std::process::Output, ClaudeError>(std::process::Output {
            status,
            stdout: stdout_buf,
            stderr: stderr_buf,
        })
    };

    let output = match io_future.await {
        Ok(output) => {
            pid.store(NO_PID, Ordering::SeqCst);
            output
        }
        Err(error) => {
            if let Some(id) = child_pid {
                kill_process_tree(id);
            }
            pid.store(NO_PID, Ordering::SeqCst);
            return Err(error);
        }
    };

    if !output.status.success() {
        let message = cli_message(&output);
        if let Some((provider, model)) = classify {
            if let Some(classified) = classify_provider_error(
                &provider.id,
                model,
                &message,
                &provider.rate_limit_patterns,
                &provider.model_unavailable_patterns,
            ) {
                return Err(classified);
            }
        }
        return Err(ClaudeError::Exit {
            code: output.status.code(),
            message,
        });
    }

    Ok(output.stdout)
}

#[cfg(test)]
#[path = "../../../tests/adapters_tests.rs"]
mod adapters_tests;
