use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

const CLI_NAME: &str = "claude";
const MODEL: &str = "haiku";
const MAX_TURNS: &str = "3";
const CLI_TIMEOUT: Duration = Duration::from_secs(120);
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

pub async fn ask(cli: &Path, message: &str, role: &str) -> Result<Reply, ClaudeError> {
    let mut command = base_command(cli);
    command.kill_on_drop(true);
    command.args([
        "-p",
        message,
        "--output-format",
        "stream-json",
        "--verbose",
        "--model",
        MODEL,
        "--tools",
        "",
        "--max-turns",
        MAX_TURNS,
        "--append-system-prompt",
        role,
    ]);
    let output = timeout(CLI_TIMEOUT, command.output())
        .await
        .map_err(|_| ClaudeError::Timeout(CLI_TIMEOUT))?
        .map_err(ClaudeError::Spawn)?;
    if !output.status.success() {
        return Err(ClaudeError::Exit {
            code: output.status.code(),
            message: cli_message(&output),
        });
    }
    let (response, limits) = parse_stream(&output.stdout)?;
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

fn parse_stream(stdout: &[u8]) -> Result<(CliResponse, Option<RateLimits>), ClaudeError> {
    let mut response: Option<CliResponse> = None;
    let mut limits: Option<RateLimits> = None;
    for line in stdout
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
    {
        let stream_line: StreamLine =
            serde_json::from_slice(line).map_err(ClaudeError::InvalidJson)?;
        match stream_line.kind.as_str() {
            RESULT_KIND => {
                response = Some(serde_json::from_slice(line).map_err(ClaudeError::InvalidJson)?);
            }
            // Um formato novo de limite não pode derrubar a conversa: a linha é ignorada.
            RATE_LIMIT_KIND => {
                if let Ok(rate_limit_line) = serde_json::from_slice::<RateLimitLine>(line) {
                    limits = Some(rate_limit_line.rate_limit_info.unified_windows.into());
                }
            }
            _ => {}
        }
    }
    let response = response.ok_or(ClaudeError::MissingResult)?;
    Ok((response, limits))
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
