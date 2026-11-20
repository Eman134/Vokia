use std::path::Path;
use std::sync::atomic::AtomicU32;

use serde_json::Value as Json;
use tokio::process::Command;

use crate::claude::{base_command, Access, ClaudeError, Reply, TurnActivity};
use crate::permissions::PermissionLevel;

pub(crate) fn build_command(
    program: &Path,
    user_args: &[String],
    model: &str,
    access: Access<'_>,
) -> Command {
    let mut command = base_command(program);
    command.kill_on_drop(true);
    command.args(user_args);
    command.args(["exec", "--json", "--skip-git-repo-check"]);
    command.args(["-m", model]);
    match access {
        Access::TextOnly => {
            command.args(["-s", "read-only"]);
        }
        Access::Project(project) => {
            command.current_dir(project.dir);
            command.arg("-C").arg(project.dir);
            command.arg("--add-dir").arg(project.attachments);
            let sandbox = match project.level {
                PermissionLevel::Leitura => "read-only",
                PermissionLevel::Edicao => "workspace-write",
                PermissionLevel::Liberado => "danger-full-access",
            };
            command.args(["-s", sandbox]);
        }
    }
    command.arg("-");
    command
}

pub(super) async fn run_turn(
    program: &Path,
    user_args: &[String],
    role: &str,
    model: &str,
    access: Access<'_>,
    message: &str,
    pid: &AtomicU32,
) -> Result<Reply, ClaudeError> {
    let command = build_command(program, user_args, model, access);
    let prompt = format!("{role}\n\n{message}");
    let stdout = super::run_cli_process(command, access, pid, Some(prompt)).await?;
    parse_jsonl_output(&stdout)
}

pub(crate) fn parse_jsonl_output(stdout: &[u8]) -> Result<Reply, ClaudeError> {
    let raw = String::from_utf8_lossy(stdout);
    let mut agent_messages: Vec<String> = Vec::new();
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
            "item.completed" => {
                let item = &value["item"];
                if item.get("type").and_then(Json::as_str) == Some("agent_message") {
                    if let Some(text) = item.get("text").and_then(Json::as_str) {
                        if !text.trim().is_empty() {
                            agent_messages.push(text.to_string());
                        }
                    }
                }
            }
            "turn.failed" | "error" => {
                let message = value["error"]["message"]
                    .as_str()
                    .or_else(|| value["message"].as_str())
                    .unwrap_or("erro desconhecido no Codex");
                return Err(ClaudeError::Reported(message.to_string()));
            }
            _ => {}
        }
    }

    let text = if !agent_messages.is_empty() {
        agent_messages.join("\n\n")
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
