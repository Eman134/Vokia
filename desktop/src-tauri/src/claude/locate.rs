use std::path::{Path, PathBuf};
use std::process::Output;

use serde::{Deserialize, Serialize};

use super::command::base_command;

const CLI_NAME: &str = "claude";
const INSTALL_COMMAND: &str = "npm install -g @anthropic-ai/claude-code";
const LOGIN_COMMAND: &str = "claude auth login";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliProblem {
    pub reason: String,
    pub fix_command: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    logged_in: bool,
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

pub(crate) fn cli_message(output: &Output) -> String {
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
