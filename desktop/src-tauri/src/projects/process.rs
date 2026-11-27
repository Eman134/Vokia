use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

use tokio::time::timeout;

use crate::claude::base_command;

use super::error::{ErrorCode, ProjectError};

pub(super) const GIT_PROGRAM: &str = "git";
pub(super) const GIT_MISSING_MESSAGE: &str = "O git não está instalado ou não está no PATH.";
const STDERR_MAX_LINES: usize = 10;

pub(super) enum RunError {
    Spawn(std::io::Error),
    TimedOut,
}

pub(super) fn locate(
    program: &str,
    code: ErrorCode,
    message: &str,
) -> Result<PathBuf, ProjectError> {
    which::which(program).map_err(|_| ProjectError::new(code, message))
}

pub(super) async fn git_output(
    git: &Path,
    dir: &str,
    args: &[&str],
) -> Result<Option<String>, ProjectError> {
    let mut full = vec!["-C", dir];
    full.extend_from_slice(args);
    let output = run(git, &full, None, None)
        .await
        .map_err(|error| match error {
            RunError::Spawn(error) => ProjectError::new(
                ErrorCode::GitNotInstalled,
                format!("Não foi possível executar o git: {error}"),
            ),
            RunError::TimedOut => {
                ProjectError::new(ErrorCode::GitNotInstalled, "O git não respondeu.")
            }
        })?;
    Ok(output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string()))
}

pub(super) async fn run(
    program: &Path,
    args: &[&str],
    dir: Option<&Path>,
    limit: Option<Duration>,
) -> Result<Output, RunError> {
    run_with_env(program, args, dir, limit, &[]).await
}

pub(super) async fn run_with_env(
    program: &Path,
    args: &[&str],
    dir: Option<&Path>,
    limit: Option<Duration>,
    envs: &[(String, String)],
) -> Result<Output, RunError> {
    let mut command = base_command(program);
    command
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .kill_on_drop(true);
    for (key, value) in envs {
        command.env(key, value);
    }
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let output = command.output();
    match limit {
        Some(limit) => timeout(limit, output)
            .await
            .map_err(|_| RunError::TimedOut)?
            .map_err(RunError::Spawn),
        None => output.await.map_err(RunError::Spawn),
    }
}

pub(super) fn tail(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = if stderr.trim().is_empty() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        stderr
    };
    let lines: Vec<&str> = text.trim().lines().collect();
    lines[lines.len().saturating_sub(STDERR_MAX_LINES)..].join("\n")
}
