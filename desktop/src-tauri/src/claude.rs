use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

use serde::{Deserialize, Serialize};
use tokio::process::Command;

const CLI_NAME: &str = "claude";
const MODEL: &str = "haiku";
const MAX_TURNS: &str = "1";
const INSTALL_COMMAND: &str = "npm install -g @anthropic-ai/claude-code";
const LOGIN_COMMAND: &str = "claude auth login";
// Sem isso o executável release (subsistema "windows") abre uma janela de console a cada chamada.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliProblem {
    pub reason: String,
    pub fix_command: String,
}

#[derive(Debug)]
pub enum ClaudeError {
    Spawn(std::io::Error),
    Exit { code: Option<i32>, message: String },
    InvalidJson(serde_json::Error),
    Reported(String),
}

impl fmt::Display for ClaudeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClaudeError::Spawn(error) => {
                write!(f, "Não foi possível executar a CLI claude: {error}")
            }
            ClaudeError::Exit {
                code: Some(code),
                message,
            } => {
                write!(f, "A CLI claude saiu com código {code}: {message}")
            }
            ClaudeError::Exit {
                code: None,
                message,
            } => {
                write!(f, "A CLI claude foi interrompida: {message}")
            }
            ClaudeError::InvalidJson(error) => {
                write!(f, "A CLI claude devolveu um JSON inválido: {error}")
            }
            ClaudeError::Reported(message) => write!(f, "A CLI claude devolveu um erro: {message}"),
        }
    }
}

pub struct Reply {
    pub text: String,
    pub session_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    logged_in: bool,
}

#[derive(Deserialize)]
struct CliResponse {
    #[serde(default)]
    result: String,
    session_id: String,
    is_error: bool,
}

pub async fn locate() -> Result<PathBuf, CliProblem> {
    let cli = which::which(CLI_NAME).map_err(|error| CliProblem {
        reason: format!("A CLI claude não foi encontrada no PATH ({error})."),
        fix_command: INSTALL_COMMAND.to_string(),
    })?;
    ensure_logged_in(&cli).await?;
    Ok(cli)
}

async fn ensure_logged_in(cli: &Path) -> Result<(), CliProblem> {
    let output = base_command(cli)
        .args(["auth", "status", "--json"])
        .output()
        .await
        .map_err(|error| {
            login_problem(format!(
                "Não foi possível executar claude auth status: {error}."
            ))
        })?;
    // Deslogada, a CLI sai com código 1 e ainda imprime o JSON com loggedIn=false.
    let status: AuthStatus = serde_json::from_slice(&output.stdout).map_err(|error| {
        login_problem(format!(
            "Resposta inválida de claude auth status ({error}): {}",
            cli_message(&output)
        ))
    })?;
    if status.logged_in {
        Ok(())
    } else {
        Err(login_problem("A CLI claude não está logada.".to_string()))
    }
}

pub async fn ask(
    cli: &Path,
    message: &str,
    role: &str,
    session_id: Option<&str>,
) -> Result<Reply, ClaudeError> {
    let mut command = base_command(cli);
    command.args([
        "-p",
        message,
        "--output-format",
        "json",
        "--model",
        MODEL,
        "--tools",
        "",
        "--max-turns",
        MAX_TURNS,
        "--append-system-prompt",
        role,
    ]);
    if let Some(id) = session_id {
        command.args(["--resume", id]);
    }
    let output = command.output().await.map_err(ClaudeError::Spawn)?;
    if !output.status.success() {
        return Err(ClaudeError::Exit {
            code: output.status.code(),
            message: cli_message(&output),
        });
    }
    let response: CliResponse =
        serde_json::from_slice(&output.stdout).map_err(ClaudeError::InvalidJson)?;
    if response.is_error {
        return Err(ClaudeError::Reported(response.result));
    }
    Ok(Reply {
        text: response.result,
        session_id: response.session_id,
    })
}

fn base_command(cli: &Path) -> Command {
    let mut command = Command::new(cli);
    // Com stdin aberto a CLI espera 3 s por dados antes de responder.
    command.stdin(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

fn cli_message(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    } else {
        stderr
    }
}

fn login_problem(reason: String) -> CliProblem {
    CliProblem {
        reason,
        fix_command: LOGIN_COMMAND.to_string(),
    }
}
