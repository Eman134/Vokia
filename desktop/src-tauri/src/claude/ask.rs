use std::path::Path;

use tokio::time::timeout;

use super::command::{agent_command, timeout_for, Access};
use super::error::ClaudeError;
use super::locate::cli_message;
use super::stream::{into_reply, parse_stream, Reply};

pub async fn ask(cli: &Path, message: &str, role: &str, model: &str) -> Result<Reply, ClaudeError> {
    let mut command = agent_command(cli, &["-p", message], role, model, Access::TextOnly, None);
    let limit = timeout_for(Access::TextOnly);
    let output = timeout(limit, command.output())
        .await
        .map_err(|_| ClaudeError::Timeout(limit))?
        .map_err(ClaudeError::Spawn)?;
    if !output.status.success() {
        return Err(ClaudeError::Exit {
            code: output.status.code(),
            message: cli_message(&output),
        });
    }
    let (response, limits) = parse_stream(&output.stdout)?;
    into_reply(response, limits)
}
