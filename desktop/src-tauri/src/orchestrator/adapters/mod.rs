mod antigravity;
mod claude;
mod codex;
mod generic;
mod opencode;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::claude::{
    classify_provider_error, cli_message, kill_process_tree, timeout_for, Access, ClaudeError,
    Reply,
};
use crate::mcp::McpSession;
use crate::permissions::{PermissionGate, PermissionLevel};
use crate::providers::{ProviderConfig, ProviderProtocol, ProviderRegistry, ToolTransport};

use super::agent_session::{AgentCall, AgentSession, SessionStore};
use super::console::ConsoleSink;
use super::error::FlowError;
use super::prompts::StepPrompts;
use super::session_pool::NO_PID;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::orchestrator) enum ProviderAdapter {
    Claude(ProviderConfig),
    Codex(ProviderConfig),
    OpenCode(ProviderConfig),
    Antigravity(ProviderConfig),
    Generic(Box<ProviderConfig>),
}

impl ProviderAdapter {
    pub(in crate::orchestrator) fn from_config(config: ProviderConfig) -> Self {
        match config.protocol {
            ProviderProtocol::ClaudeLive => Self::Claude(config),
            ProviderProtocol::CodexJsonl => Self::Codex(config),
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
            | Self::Codex(config)
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

    #[allow(dead_code)]
    pub(in crate::orchestrator) fn tool_transport(&self) -> ToolTransport {
        self.config().effective_tool_transport()
    }

    pub(in crate::orchestrator) fn has_tools(&self) -> bool {
        self.config().has_tools()
    }

    pub(in crate::orchestrator) fn supports_live_session(&self) -> bool {
        matches!(self, Self::Claude(_))
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
    ) -> Result<Option<Reply>, FlowError> {
        match (self, access) {
            (Self::Claude(_), Access::Project(project)) => {
                held.reply(project, gate, store, call, prompts, first, console)
                    .await
            }
            _ => Ok(None),
        }
    }

    pub(in crate::orchestrator) async fn run_fresh(
        &self,
        access: Access<'_>,
        gate: PermissionGate<'_>,
        pid: &AtomicU32,
        call: &AgentCall,
        message: &str,
        console: &ConsoleSink<'_>,
    ) -> Result<Reply, ClaudeError> {
        match self {
            Self::Claude(_) => claude::run_fresh(access, gate, pid, call, message, console).await,
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
                stdin.write_all(input.as_bytes()).await?;
                stdin.shutdown().await?;
            }
        }
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let stdout_fut = async {
            let mut buf = Vec::new();
            if let Some(out) = stdout {
                let mut lines = BufReader::new(out).lines();
                while let Some(line) = lines.next_line().await? {
                    on_line(&line);
                    buf.extend_from_slice(line.as_bytes());
                    buf.push(b'\n');
                }
            }
            Ok::<Vec<u8>, std::io::Error>(buf)
        };

        let stderr_fut = async {
            let mut buf = Vec::new();
            if let Some(mut err) = stderr {
                err.read_to_end(&mut buf).await?;
            }
            Ok::<Vec<u8>, std::io::Error>(buf)
        };

        let (stdout_res, stderr_res) = tokio::join!(stdout_fut, stderr_fut);
        let status = child.wait().await?;
        Ok::<std::process::Output, std::io::Error>(std::process::Output {
            status,
            stdout: stdout_res?,
            stderr: stderr_res?,
        })
    };

    let output = match timeout(limit, io_future).await {
        Ok(Ok(output)) => {
            pid.store(NO_PID, Ordering::SeqCst);
            output
        }
        Ok(Err(error)) => {
            if let Some(id) = child_pid {
                kill_process_tree(id);
            }
            pid.store(NO_PID, Ordering::SeqCst);
            return Err(ClaudeError::Spawn(error));
        }
        Err(_) => {
            if let Some(id) = child_pid {
                kill_process_tree(id);
            }
            pid.store(NO_PID, Ordering::SeqCst);
            return Err(ClaudeError::Timeout(limit));
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
