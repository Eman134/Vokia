use std::path::Path;

use serde_json::Value as Json;
use tokio::process::Command;

use crate::claude::{base_command, Access, ClaudeError, Reply, TurnActivity};
use crate::permissions::PermissionLevel;

pub(crate) fn build_command(
    program: &Path,
    user_args: &[String],
    role: &str,
    model: &str,
    access: Access<'_>,
    message: &str,
) -> Result<Command, ClaudeError> {
    let mut command = base_command(program);
    command.kill_on_drop(true);
    command.args(user_args);
    command.args(["run", "--format", "json"]);
    command.args(["-m", model]);
    if let Access::Project(project) = access {
        if project.level == PermissionLevel::Liberado {
            return Err(ClaudeError::Reported(
                "OpenCode não oferece sandbox nem controle de aprovação equivalente para o nível Liberado."
                    .to_string(),
            ));
        }
        command.current_dir(project.dir);
        command.arg("--dir").arg(project.dir);
    }
    let prompt = format!("{role}\n\n{message}");
    command.arg("--").arg(prompt);
    Ok(command)
}

pub(super) async fn run_turn(
    program: &Path,
    user_args: &[String],
    role: &str,
    model: &str,
    access: Access<'_>,
    message: &str,
    pid: &std::sync::atomic::AtomicU32,
) -> Result<Reply, ClaudeError> {
    let command = build_command(program, user_args, role, model, access, message)?;
    let stdout = super::run_cli_process(command, access, pid, None).await?;
    parse_json_output(&stdout)
}

pub(crate) fn parse_json_output(stdout: &[u8]) -> Result<Reply, ClaudeError> {
    let raw = String::from_utf8_lossy(stdout);
    let mut texts: Vec<String> = Vec::new();
    let mut saw_json = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Json>(trimmed) else {
            continue;
        };
        saw_json = true;
        let Some(kind) = value.get("type").and_then(Json::as_str) else {
            continue;
        };
        match kind {
            "text" => {
                let text = value["part"]["text"]
                    .as_str()
                    .or_else(|| value["text"].as_str())
                    .or_else(|| value["content"].as_str());
                if let Some(text) = text {
                    if !text.trim().is_empty() {
                        texts.push(text.to_string());
                    }
                }
            }
            "error" => {
                let message = value["error"]["data"]["message"]
                    .as_str()
                    .or_else(|| value["error"]["message"].as_str())
                    .or_else(|| value["message"].as_str())
                    .unwrap_or("erro desconhecido no OpenCode");
                return Err(ClaudeError::Reported(message.to_string()));
            }
            _ => {}
        }
    }

    let text = if !texts.is_empty() {
        texts.join("\n\n")
    } else if !saw_json {
        raw.trim().to_string()
    } else {
        String::new()
    };

    Ok(Reply {
        text,
        limits: None,
        activity: TurnActivity::default(),
    })
}
