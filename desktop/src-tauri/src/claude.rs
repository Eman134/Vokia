use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::task::JoinHandle;
use tokio::time::timeout;

const USER_MESSAGE_KIND: &str = "user";
const USER_ROLE: &str = "user";
const PIPE_MISSING: &str = "o pipe do processo não ficou disponível";
const STDERR_TAIL_LINES: usize = 5;
const STDERR_DRAIN_TIMEOUT: Duration = Duration::from_secs(2);
const SESSION_EOF: &str = "o processo encerrou antes de enviar o resultado";
const CLI_NAME: &str = "claude";
const MODEL: &str = "haiku";
const TEXT_ONLY_MAX_TURNS: &str = "3";
const PROJECT_MAX_TURNS: &str = "40";
const CLI_TIMEOUT: Duration = Duration::from_secs(120);
const PROJECT_CLI_TIMEOUT: Duration = Duration::from_secs(600);
const PERMISSION_MODE: &str = "manual";
// Ignora hooks e regras do usuário (um hook que reescreve o comando faz a regra não casar).
const NO_SETTING_SOURCES: &str = "";
const PERMISSION_PROMPT_TOOL: &str = "stdio";
// Ferramentas de arquivo com padrão de caminho ficam na pasta do projeto (cwd); a regra sem padrão libera qualquer caminho, inclusive Glob e Grep.
const FILE_TOOLS: [&str; 4] = ["Edit(./**)", "Write(./**)", "Glob(./**)", "Grep(./**)"];
// Regra por prefixo não enxerga opções que gravam (`git diff --output`, `git branch novo`): esses comandos entram só na forma exata.
const BASH_ALLOWLIST: [&str; 26] = [
    "Bash(ls)",
    "Bash(ls *)",
    "Bash(cat *)",
    "Bash(head *)",
    "Bash(tail *)",
    "Bash(pwd)",
    "Bash(git status)",
    "Bash(git status *)",
    "Bash(git log)",
    "Bash(git log --oneline)",
    "Bash(git diff)",
    "Bash(git diff --stat)",
    "Bash(git show)",
    "Bash(git branch)",
    "Bash(git branch --list)",
    "Bash(git rev-parse *)",
    "Bash(npm test *)",
    "Bash(npm run test *)",
    "Bash(npm run build *)",
    "Bash(npm run typecheck *)",
    "Bash(npm run lint *)",
    "Bash(cargo test *)",
    "Bash(cargo build *)",
    "Bash(cargo check *)",
    "Bash(cargo fmt --check)",
    "Bash(cargo clippy *)",
];
const CONTROL_REQUEST_KIND: &str = "control_request";
const CAN_USE_TOOL_SUBTYPE: &str = "can_use_tool";
const CONTROL_RESPONSE_KIND: &str = "control_response";
const DENIED_MESSAGE: &str = "O usuário negou este comando.";
const RESULT_KIND: &str = "result";
const RATE_LIMIT_KIND: &str = "rate_limit_event";
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
    MissingResult,
    Reported(String),
    Timeout(Duration),
    SessionClosed(String),
}

impl ClaudeError {
    pub fn is_session_loss(&self) -> bool {
        matches!(
            self,
            ClaudeError::SessionClosed(_) | ClaudeError::Timeout(_)
        )
    }
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
            ClaudeError::MissingResult => write!(
                f,
                "A CLI claude terminou sem a linha de resultado do stream-json."
            ),
            ClaudeError::Timeout(limit) => {
                write!(
                    f,
                    "A CLI claude não respondeu em {} s e foi encerrada.",
                    limit.as_secs()
                )
            }
            ClaudeError::Reported(message) => write!(f, "A CLI claude devolveu um erro: {message}"),
            ClaudeError::SessionClosed(reason) => {
                write!(f, "A sessão da CLI claude foi encerrada: {reason}")
            }
        }
    }
}

pub struct Reply {
    pub text: String,
    pub limits: Option<RateLimits>,
}

#[derive(Clone, Copy, Debug)]
pub struct RateLimits {
    pub five_hour: LimitWindow,
    pub seven_day: LimitWindow,
}

#[derive(Clone, Copy, Debug)]
pub struct LimitWindow {
    pub utilization: f64,
    pub resets_at: u64,
}

#[derive(Deserialize)]
struct StreamLine {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct RateLimitLine {
    rate_limit_info: RateLimitInfo,
}

#[derive(Deserialize)]
struct RateLimitInfo {
    #[serde(rename = "unifiedWindows")]
    unified_windows: UnifiedWindows,
}

#[derive(Deserialize)]
struct UnifiedWindows {
    five_hour: WindowInfo,
    seven_day: WindowInfo,
}

#[derive(Deserialize)]
struct WindowInfo {
    utilization: f64,
    #[serde(rename = "resetsAt")]
    resets_at: u64,
}

impl From<UnifiedWindows> for RateLimits {
    fn from(windows: UnifiedWindows) -> Self {
        Self {
            five_hour: windows.five_hour.into(),
            seven_day: windows.seven_day.into(),
        }
    }
}

impl From<WindowInfo> for LimitWindow {
    fn from(window: WindowInfo) -> Self {
        Self {
            utilization: window.utilization,
            resets_at: window.resets_at,
        }
    }
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
    #[serde(default)]
    subtype: String,
    #[serde(default)]
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

#[derive(Clone, Copy)]
pub enum Access<'a> {
    TextOnly,
    Project(&'a Path),
}

fn timeout_for(access: Access<'_>) -> Duration {
    match access {
        Access::TextOnly => CLI_TIMEOUT,
        Access::Project(_) => PROJECT_CLI_TIMEOUT,
    }
}

pub async fn ask(cli: &Path, message: &str, role: &str) -> Result<Reply, ClaudeError> {
    let mut command = agent_command(cli, &["-p", message], role, Access::TextOnly);
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

fn into_reply(response: CliResponse, limits: Option<RateLimits>) -> Result<Reply, ClaudeError> {
    if response.is_error {
        let message = if response.result.is_empty() {
            format!("sem texto (subtype: {})", response.subtype)
        } else {
            response.result
        };
        return Err(ClaudeError::Reported(message));
    }
    Ok(Reply {
        text: response.result,
        limits,
    })
}

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
        access: Access<'_>,
        resume: Option<&str>,
    ) -> Result<Self, ClaudeError> {
        let mut command =
            agent_command(cli, &["-p", "--input-format", "stream-json"], role, access);
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

// Encerra o processo e seus filhos (as ferramentas que o claude lança) sem depender do drop do LiveSession.
pub fn kill_process_tree(pid: u32) {
    #[cfg(windows)]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("taskkill");
        command
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(CREATE_NO_WINDOW);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = std::process::Command::new("kill");
        command.args(["-KILL", &pid.to_string()]);
        command
    };
    // Falha quando o processo já saiu, que é o resultado desejado.
    let _ = command.output();
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

#[derive(Clone)]
pub struct PermissionRequest {
    pub id: String,
    pub tool: String,
    pub input: serde_json::Value,
}

enum ControlRequest {
    Permission(PermissionRequest),
    Malformed(String),
}

#[derive(Deserialize)]
struct ControlLine {
    #[serde(rename = "type")]
    kind: String,
    request_id: Option<String>,
    request: Option<ControlBody>,
}

#[derive(Deserialize)]
struct ControlBody {
    subtype: String,
    tool_name: Option<String>,
    input: Option<serde_json::Value>,
}

// Pedido sem id é ignorado (não há a quem responder); com id mas incompleto, a resposta é negar, nunca permitir.
fn parse_control_request(line: &[u8]) -> Option<ControlRequest> {
    let parsed: ControlLine = serde_json::from_slice(line).ok()?;
    if parsed.kind != CONTROL_REQUEST_KIND {
        return None;
    }
    let id = parsed.request_id?;
    let Some(body) = parsed
        .request
        .filter(|body| body.subtype == CAN_USE_TOOL_SUBTYPE)
    else {
        return Some(ControlRequest::Malformed(id));
    };
    match (body.tool_name, body.input) {
        (Some(tool), Some(input)) => Some(ControlRequest::Permission(PermissionRequest {
            id,
            tool,
            input,
        })),
        _ => Some(ControlRequest::Malformed(id)),
    }
}

#[derive(Default)]
struct StreamAccumulator {
    response: Option<CliResponse>,
    limits: Option<RateLimits>,
}

impl StreamAccumulator {
    fn push_line(&mut self, line: &[u8]) -> Result<bool, ClaudeError> {
        if line.iter().all(u8::is_ascii_whitespace) {
            return Ok(false);
        }
        let stream_line: StreamLine =
            serde_json::from_slice(line).map_err(ClaudeError::InvalidJson)?;
        match stream_line.kind.as_str() {
            RESULT_KIND => {
                self.response =
                    Some(serde_json::from_slice(line).map_err(ClaudeError::InvalidJson)?);
                return Ok(true);
            }
            // Um formato novo de limite não pode derrubar a conversa: a linha é ignorada.
            RATE_LIMIT_KIND => {
                if let Ok(rate_limit_line) = serde_json::from_slice::<RateLimitLine>(line) {
                    self.limits = Some(rate_limit_line.rate_limit_info.unified_windows.into());
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn finish(self) -> Result<(CliResponse, Option<RateLimits>), ClaudeError> {
        let response = self.response.ok_or(ClaudeError::MissingResult)?;
        Ok((response, self.limits))
    }
}

fn parse_stream(stdout: &[u8]) -> Result<(CliResponse, Option<RateLimits>), ClaudeError> {
    let mut accumulator = StreamAccumulator::default();
    for line in stdout.split(|byte| *byte == b'\n') {
        accumulator.push_line(line)?;
    }
    accumulator.finish()
}

fn agent_command(cli: &Path, leading: &[&str], role: &str, access: Access<'_>) -> Command {
    let mut command = base_command(cli);
    command.kill_on_drop(true);
    // A mensagem de `-p` vem antes das opções variádicas (`--tools`, `--allowedTools`),
    // que a engoliriam; `--append-system-prompt` fica por último pelo mesmo motivo.
    command.args(leading);
    command.args([
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        MODEL,
    ]);
    match access {
        Access::TextOnly => {
            command.args(["--tools", "", "--max-turns", TEXT_ONLY_MAX_TURNS]);
        }
        Access::Project(dir) => {
            command.current_dir(dir);
            command
                .arg("--add-dir")
                .arg(dir)
                .args(["--permission-mode", PERMISSION_MODE])
                .args(["--setting-sources", NO_SETTING_SOURCES])
                .args(["--allowedTools", &allowed_tools()])
                .args(["--max-turns", PROJECT_MAX_TURNS]);
        }
    }
    command.args(["--append-system-prompt", role]);
    command
}

fn allowed_tools() -> String {
    FILE_TOOLS
        .iter()
        .chain(BASH_ALLOWLIST.iter())
        .copied()
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn base_command(cli: &Path) -> Command {
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

#[cfg(test)]
mod tests {
    use super::*;

    const SYSTEM_LINE: &str = r#"{"type":"system","subtype":"init","session_id":"sess-1"}"#;
    const RESULT_LINE: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"ok","session_id":"sess-1"}"#;
    const RATE_LIMIT_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.33,"resetsAt":1793083800,"status":"allowed"},"seven_day":{"utilization":0.52,"resetsAt":1793426400,"status":"allowed"}}},"uuid":"u-1","session_id":"sess-1"}"#;
    const RATE_LIMIT_LATER_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.91,"resetsAt":1793083800,"status":"allowed"},"seven_day":{"utilization":0.55,"resetsAt":1793426400,"status":"allowed"}}},"uuid":"u-2","session_id":"sess-1"}"#;
    const RATE_LIMIT_EXPIRED_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","rateLimitType":"five_hour","unifiedWindows":{"five_hour":{"utilization":0.91,"resetsAt":1700000000,"status":"allowed"},"seven_day":{"utilization":0.55,"resetsAt":1793426400,"status":"allowed"}}},"uuid":"u-3","session_id":"sess-1"}"#;
    const RATE_LIMIT_MALFORMED_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":"alto","resetsAt":1793083800},"seven_day":{"utilization":0.52,"resetsAt":1793426400}}}}"#;
    const RATE_LIMIT_TRUNCATED_LINE: &str = r#"{"type":"rate_limit_event","rate_limit_info":{"unifiedWindows":{"five_hour":{"utilization":0.33"#;
    const NOT_JSON_LINE: &str = "isto não é json";

    fn stream(lines: &[&str]) -> Vec<u8> {
        lines.join("\n").into_bytes()
    }

    #[test]
    fn linha_de_limite_valida_vira_janelas_de_uso_e_reset() {
        let stdout = stream(&[SYSTEM_LINE, RATE_LIMIT_LINE, RESULT_LINE]);

        let (response, limits) = parse_stream(&stdout).unwrap();
        let limits = limits.expect("a linha rate_limit_event deveria virar limites");

        assert_eq!(response.result, "ok");
        assert_eq!(limits.five_hour.utilization, 0.33);
        assert_eq!(limits.five_hour.resets_at, 1793083800);
        assert_eq!(limits.seven_day.utilization, 0.52);
        assert_eq!(limits.seven_day.resets_at, 1793426400);
    }

    #[test]
    fn ultima_linha_de_limite_valida_prevalece() {
        let stdout = stream(&[RATE_LIMIT_LINE, RATE_LIMIT_LATER_LINE, RESULT_LINE]);

        let (_, limits) = parse_stream(&stdout).unwrap();

        assert_eq!(limits.unwrap().five_hour.utilization, 0.91);
    }

    #[test]
    fn linha_de_limite_malformada_mantem_o_ultimo_valor_valido() {
        let stdout = stream(&[RATE_LIMIT_LINE, RATE_LIMIT_MALFORMED_LINE, RESULT_LINE]);

        let (response, limits) = parse_stream(&stdout).unwrap();

        assert_eq!(response.result, "ok");
        assert_eq!(limits.unwrap().five_hour.utilization, 0.33);
    }

    #[test]
    fn linha_de_limite_malformada_sem_valor_anterior_devolve_sem_limites() {
        let stdout = stream(&[RATE_LIMIT_MALFORMED_LINE, RESULT_LINE]);

        let (response, limits) = parse_stream(&stdout).unwrap();

        assert_eq!(response.result, "ok");
        assert!(limits.is_none());
    }

    #[test]
    fn linha_de_limite_truncada_falha_como_json_invalido() {
        let stdout = stream(&[RATE_LIMIT_TRUNCATED_LINE, RESULT_LINE]);

        assert!(matches!(
            parse_stream(&stdout),
            Err(ClaudeError::InvalidJson(_))
        ));
    }

    #[test]
    fn linha_que_nao_e_json_continua_falhando_como_antes() {
        let stdout = stream(&[NOT_JSON_LINE, RESULT_LINE]);

        assert!(matches!(
            parse_stream(&stdout),
            Err(ClaudeError::InvalidJson(_))
        ));
    }

    #[test]
    fn sem_linha_de_limite_devolve_resposta_sem_limites() {
        let stdout = stream(&[SYSTEM_LINE, RESULT_LINE]);

        let (response, limits) = parse_stream(&stdout).unwrap();

        assert_eq!(response.result, "ok");
        assert!(limits.is_none());
    }

    #[test]
    fn reset_ja_passado_chega_sem_alteracao() {
        // A expiração é decidida no front (useUsage.visible); o parser só repassa o dado.
        let stdout = stream(&[RATE_LIMIT_EXPIRED_LINE, RESULT_LINE]);

        let (_, limits) = parse_stream(&stdout).unwrap();

        assert_eq!(limits.unwrap().five_hour.resets_at, 1700000000);
    }

    #[test]
    fn quebras_de_linha_crlf_nao_quebram_a_leitura() {
        let stdout = format!("{RATE_LIMIT_LINE}\r\n{RESULT_LINE}\r\n").into_bytes();

        let (response, limits) = parse_stream(&stdout).unwrap();

        assert_eq!(response.result, "ok");
        assert_eq!(limits.unwrap().seven_day.utilization, 0.52);
    }

    #[test]
    fn stream_sem_linha_result_falha_com_missing_result() {
        let stdout = stream(&[RATE_LIMIT_LINE]);

        assert!(matches!(
            parse_stream(&stdout),
            Err(ClaudeError::MissingResult)
        ));
    }
}
