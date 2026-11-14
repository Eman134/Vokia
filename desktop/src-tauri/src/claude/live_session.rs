use std::collections::VecDeque;
use std::future::Future;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::command::{agent_command, timeout_for, Access};
use super::control::{parse_control_request, ControlRequest, PermissionRequest};
use super::error::ClaudeError;
use super::stream::{into_reply, CliResponse, RateLimits, Reply, StreamAccumulator};

const USER_MESSAGE_KIND: &str = "user";
const USER_ROLE: &str = "user";
const PIPE_MISSING: &str = "o pipe do processo não ficou disponível";
const STDERR_TAIL_LINES: usize = 5;
const STDERR_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_EOF: &str = "o processo encerrou antes de enviar o resultado";
const PERMISSION_PROMPT_TOOL: &str = "stdio";
const CONTROL_RESPONSE_KIND: &str = "control_response";
const DENIED_MESSAGE: &str = "O usuário negou este comando.";

pub struct LiveSession {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    session_id: Option<String>,
    timeout: Duration,
    stderr_tail: Option<JoinHandle<String>>,
}

impl LiveSession {
    pub fn open(
        cli: &Path,
        role: &str,
        model: &str,
        access: Access<'_>,
        resume: Option<&str>,
    ) -> Result<Self, ClaudeError> {
        let mut command = agent_command(
            cli,
            &["-p", "--input-format", "stream-json"],
            role,
            model,
            access,
        );
        command.args([
            "--permission-prompts",
            "host",
            "--permission-prompt-tool",
            PERMISSION_PROMPT_TOOL,
        ]);
        if let Some(session_id) = resume {
            command.args(["--resume", session_id]);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(ClaudeError::Spawn)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ClaudeError::SessionClosed(PIPE_MISSING.to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ClaudeError::SessionClosed(PIPE_MISSING.to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ClaudeError::SessionClosed(PIPE_MISSING.to_string()))?;
        Ok(Self {
            child,
            stdin,
            lines: BufReader::new(stdout).lines(),
            session_id: resume.map(str::to_string),
            timeout: timeout_for(access),
            stderr_tail: Some(tokio::spawn(read_stderr_tail(stderr))),
        })
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    pub async fn send<D, F>(&mut self, message: &str, decide: D) -> Result<Reply, ClaudeError>
    where
        D: Fn(PermissionRequest) -> F,
        F: Future<Output = bool>,
    {
        let (response, limits) = self.exchange(message, decide).await?;
        if !response.session_id.is_empty() {
            self.session_id = Some(response.session_id.clone());
        }
        into_reply(response, limits)
    }

    async fn exchange<D, F>(
        &mut self,
        message: &str,
        decide: D,
    ) -> Result<(CliResponse, Option<RateLimits>), ClaudeError>
    where
        D: Fn(PermissionRequest) -> F,
        F: Future<Output = bool>,
    {
        let payload = serde_json::json!({
            "type": USER_MESSAGE_KIND,
            "message": { "role": USER_ROLE, "content": message },
        });
        self.stdin
            .write_all(format!("{payload}\n").as_bytes())
            .await
            .map_err(|error| ClaudeError::SessionClosed(error.to_string()))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| ClaudeError::SessionClosed(error.to_string()))?;
        let mut accumulator = StreamAccumulator::default();
        loop {
            // O limite vale entre linhas: a espera pela decisão do usuário não conta, e um agente que segue produzindo não estoura.
            let line = timeout(self.timeout, self.lines.next_line())
                .await
                .map_err(|_| ClaudeError::Timeout(self.timeout))?
                .map_err(|error| ClaudeError::SessionClosed(error.to_string()))?;
            let Some(line) = line else {
                return Err(ClaudeError::SessionClosed(self.closing_reason().await));
            };
            match parse_control_request(line.as_bytes()) {
                Some(ControlRequest::Permission(request)) => {
                    let id = request.id.clone();
                    let input = request.input.clone();
                    let allowed = decide(request).await;
                    self.answer_permission(&id, allowed.then_some(input))
                        .await?;
                }
                Some(ControlRequest::Malformed(id)) => {
                    self.answer_permission(&id, None).await?;
                }
                None => {
                    if accumulator.push_line(line.as_bytes())? {
                        return accumulator.finish();
                    }
                }
            }
        }
    }

    async fn closing_reason(&mut self) -> String {
        let text = self.stderr_text().await;
        match timeout(STDERR_DRAIN_TIMEOUT, self.child.wait()).await {
            Ok(Ok(status)) => match status.code() {
                Some(code) => format!("{text} (código {code})"),
                None => text,
            },
            _ => text,
        }
    }

    async fn stderr_text(&mut self) -> String {
        let Some(tail) = self.stderr_tail.take() else {
            return SESSION_EOF.to_string();
        };
        match timeout(STDERR_DRAIN_TIMEOUT, tail).await {
            Ok(Ok(text)) if !text.is_empty() => text,
            _ => SESSION_EOF.to_string(),
        }
    }

    async fn answer_permission(
        &mut self,
        request_id: &str,
        allowed_input: Option<serde_json::Value>,
    ) -> Result<(), ClaudeError> {
        let decision = match allowed_input {
            Some(input) => serde_json::json!({ "behavior": "allow", "updatedInput": input }),
            None => serde_json::json!({ "behavior": "deny", "message": DENIED_MESSAGE }),
        };
        let payload = serde_json::json!({
            "type": CONTROL_RESPONSE_KIND,
            "response": { "subtype": "success", "request_id": request_id, "response": decision },
        });
        self.stdin
            .write_all(
                format!(
                    "{payload}
"
                )
                .as_bytes(),
            )
            .await
            .map_err(|error| ClaudeError::SessionClosed(error.to_string()))?;
        self.stdin
            .flush()
            .await
            .map_err(|error| ClaudeError::SessionClosed(error.to_string()))
    }

    pub async fn close(mut self, grace: Duration) {
        drop(self.stdin);
        if let Some(tail) = self.stderr_tail.take() {
            tail.abort();
        }
        if timeout(grace, self.child.wait()).await.is_err() {
            // O kill só falha se o processo já saiu, que é o resultado desejado.
            let _ = self.child.kill().await;
        }
    }
}

// Sem leitor contínuo o stderr cheio travaria o processo no meio de uma resposta.
async fn read_stderr_tail(stderr: ChildStderr) -> String {
    let mut lines = BufReader::new(stderr).lines();
    let mut tail = VecDeque::with_capacity(STDERR_TAIL_LINES);
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        if tail.len() == STDERR_TAIL_LINES {
            tail.pop_front();
        }
        tail.push_back(line);
    }
    Vec::from(tail).join(
        "
",
    )
}
