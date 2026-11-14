use std::fmt;
use std::time::Duration;

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
