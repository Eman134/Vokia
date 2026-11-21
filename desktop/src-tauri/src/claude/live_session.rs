use std::collections::VecDeque;
use std::future::Future;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::activity::{ActivityParser, TurnActivity};
use super::command::{agent_command, timeout_for, Access};
use super::console::{tool_summary, ConsoleKind, ConsoleLine, ConsoleParser};
use super::control::{parse_control_request, ControlRequest, PermissionRequest, PermissionVerdict};
use super::error::ClaudeError;
use super::stream::{into_reply, CliResponse, RateLimits, Reply, StreamAccumulator};
use crate::mcp::{ExchangeSlot, McpSession};

const USER_MESSAGE_KIND: &str = "user";
const USER_ROLE: &str = "user";
const PIPE_MISSING: &str = "o pipe do processo não ficou disponível";
const STDERR_TAIL_LINES: usize = 5;
const STDERR_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_EOF: &str = "o processo encerrou antes de enviar o resultado";
const PERMISSION_PROMPT_TOOL: &str = "stdio";
const CONTROL_RESPONSE_KIND: &str = "control_response";
const DENIED_MESSAGE: &str = "O usuário negou este comando.";
const PERMISSION_WAIT_PREFIX: &str = "aguardando sua permissão: ";

pub struct LiveSession {
    child: Child,
    stdin: ChildStdin,
    lines: Lines<BufReader<ChildStdout>>,
    session_id: Option<String>,
    timeout: Duration,
    stderr_tail: Option<JoinHandle<String>>,
    mcp: Option<McpSession>,
}

impl LiveSession {
    pub fn open(
        program: &Path,
        user_args: &[String],
        role: &str,
        model: &str,
        access: Access<'_>,
        resume: Option<&str>,
        mcp: Option<McpSession>,
    ) -> Result<Self, ClaudeError> {
        let mut command = agent_command(
            program,
            user_args,
            &["-p", "--input-format", "stream-json"],
            role,
            model,
            access,
            mcp.as_ref().and_then(McpSession::config),
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
            mcp,
        })
    }

    pub fn exchange_slot(&self) -> Option<&ExchangeSlot> {
        self.mcp.as_ref().map(McpSession::slot)
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }

    pub async fn send<D, F, O, A>(
        &mut self,
        message: &str,
        decide: D,
        mut observe: O,
        mut on_activity: A,
    ) -> Result<Reply, ClaudeError>
    where
        D: Fn(PermissionRequest) -> F,
        F: Future<Output = PermissionVerdict>,
        O: FnMut(ConsoleLine),
        A: FnMut(TurnActivity),
    {
        let (response, limits, activity) = self
            .exchange(message, decide, &mut observe, &mut on_activity)
            .await?;
        if !response.session_id.is_empty() {
            self.session_id = Some(response.session_id.clone());
        }
        let mut reply = into_reply(response, limits)?;
        reply.activity = activity;
        Ok(reply)
    }

    async fn exchange<D, F, O, A>(
        &mut self,
        message: &str,
        decide: D,
        observe: &mut O,
        on_activity: &mut A,
    ) -> Result<(CliResponse, Option<RateLimits>, TurnActivity), ClaudeError>
    where
        D: Fn(PermissionRequest) -> F,
        F: Future<Output = PermissionVerdict>,
        O: FnMut(ConsoleLine),
        A: FnMut(TurnActivity),
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
        let mut console = ConsoleParser;
        let mut activity = ActivityParser::new(Instant::now());
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
                    observe(ConsoleLine::new(
                        ConsoleKind::Permission,
                        &format!(
                            "{PERMISSION_WAIT_PREFIX}{}",
                            tool_summary(&request.tool, &request.input)
                        ),
                    ));
                    let verdict = decide(request).await;
                    self.answer_permission(&id, permission_response(verdict, input))
                        .await?;
                }
                Some(ControlRequest::Malformed(id)) => {
                    self.answer_permission(&id, permission_response(PermissionVerdict::Deny(None), serde_json::Value::Null))
                        .await?;
                }
                None => {
                    for console_line in console.lines(line.as_bytes()) {
                        observe(console_line);
                    }
                    if activity.push_line(line.as_bytes(), Instant::now()) {
                        on_activity(activity.snapshot());
                    }
                    if accumulator.push_line(line.as_bytes())? {
                        let (response, limits) = accumulator.finish()?;
                        return Ok((response, limits, activity.finish()));
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
        decision: serde_json::Value,
    ) -> Result<(), ClaudeError> {
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

fn permission_response(verdict: PermissionVerdict, input: serde_json::Value) -> serde_json::Value {
    match verdict {
        PermissionVerdict::Allow => serde_json::json!({ "behavior": "allow", "updatedInput": input }),
        PermissionVerdict::Deny(Some(reason)) => {
            serde_json::json!({ "behavior": "deny", "message": format!("{DENIED_MESSAGE} Motivo: {reason}") })
        }
        PermissionVerdict::Deny(None) => serde_json::json!({ "behavior": "deny", "message": DENIED_MESSAGE }),
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
