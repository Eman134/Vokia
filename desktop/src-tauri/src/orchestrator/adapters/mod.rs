mod antigravity;
mod claude;
mod codex;
mod generic;
mod opencode;

use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::claude::{
    classify_provider_error, cli_message, kill_process_tree, timeout_for, Access, ClaudeError,
    Reply,
};
use crate::permissions::PermissionGate;
use crate::providers::{ProviderConfig, ProviderProtocol, ProviderRegistry};

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

    pub(in crate::orchestrator) fn supports_mcp(&self) -> bool {
        self.config().supports_mcp
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
                codex::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                    provider,
                )
                .await
            }
            Self::OpenCode(provider) => {
                opencode::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                    provider,
                )
                .await
            }
            Self::Antigravity(provider) => {
                antigravity::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                    provider,
                )
                .await
            }
            Self::Generic(provider) => {
                generic::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                    provider,
                )
                .await
            }
        }
    }
}

pub(crate) fn cli_supports_mcp(cli: &str) -> bool {
    crate::providers::initial_providers()
        .into_iter()
        .find(|provider| provider.id == cli)
        .is_some_and(|provider| provider.supports_mcp)
}

pub(super) async fn run_cli_process(
    mut command: Command,
    access: Access<'_>,
    pid: &AtomicU32,
    stdin_input: Option<String>,
    classify: Option<(&ProviderConfig, &str)>,
) -> Result<Vec<u8>, ClaudeError> {
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
        child.wait_with_output().await
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
