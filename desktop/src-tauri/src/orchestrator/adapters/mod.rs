mod antigravity;
mod claude;
mod codex;
mod opencode;

use std::process::Stdio;
use std::sync::atomic::{AtomicU32, Ordering};

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

use crate::claude::{cli_message, kill_process_tree, timeout_for, Access, ClaudeError, Reply};
use crate::permissions::PermissionGate;

use super::agent_session::{AgentCall, AgentSession, SessionStore};
use super::console::ConsoleSink;
use super::error::FlowError;
use super::prompts::StepPrompts;
use super::session_pool::NO_PID;

const CLAUDE_ID: &str = "claude";
const CODEX_ID: &str = "codex";
const OPENCODE_ID: &str = "opencode";
const ANTIGRAVITY_ID: &str = "antigravity";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::orchestrator) enum ProviderAdapter {
    Claude,
    Codex,
    OpenCode,
    Antigravity,
}

impl ProviderAdapter {
    pub(in crate::orchestrator) fn from_cli(cli: &str) -> Result<Self, FlowError> {
        match cli {
            CLAUDE_ID => Ok(Self::Claude),
            CODEX_ID => Ok(Self::Codex),
            OPENCODE_ID => Ok(Self::OpenCode),
            ANTIGRAVITY_ID => Ok(Self::Antigravity),
            other => Err(FlowError::CliUnavailable(format!(
                "Provedor de CLI desconhecido: {other}."
            ))),
        }
    }

    pub(in crate::orchestrator) fn supports_mcp(self) -> bool {
        matches!(self, Self::Claude)
    }

    pub(in crate::orchestrator) fn supports_live_session(self) -> bool {
        matches!(self, Self::Claude)
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::orchestrator) async fn run_live(
        self,
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
            (Self::Claude, Access::Project(project)) => {
                held.reply(project, gate, store, call, prompts, first, console)
                    .await
            }
            _ => Ok(None),
        }
    }

    pub(in crate::orchestrator) async fn run_fresh(
        self,
        access: Access<'_>,
        gate: PermissionGate<'_>,
        pid: &AtomicU32,
        call: &AgentCall,
        message: &str,
        console: &ConsoleSink<'_>,
    ) -> Result<Reply, ClaudeError> {
        match self {
            Self::Claude => claude::run_fresh(access, gate, pid, call, message, console).await,
            Self::Codex => {
                codex::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                )
                .await
            }
            Self::OpenCode => {
                opencode::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                )
                .await
            }
            Self::Antigravity => {
                antigravity::run_turn(
                    &call.program,
                    &call.user_args,
                    &call.role,
                    &call.model,
                    access,
                    message,
                    pid,
                )
                .await
            }
        }
    }
}

pub(crate) fn cli_supports_mcp(cli: &str) -> bool {
    ProviderAdapter::from_cli(cli).is_ok_and(ProviderAdapter::supports_mcp)
}

pub(super) async fn run_cli_process(
    mut command: Command,
    access: Access<'_>,
    pid: &AtomicU32,
    stdin_input: Option<String>,
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
        return Err(ClaudeError::Exit {
            code: output.status.code(),
            message: cli_message(&output),
        });
    }

    Ok(output.stdout)
}

#[cfg(test)]
#[path = "../../../tests/adapters_tests.rs"]
mod adapters_tests;
