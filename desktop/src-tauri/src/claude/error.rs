use std::fmt;
use std::time::Duration;

use super::stream::{LimitName, UsageLimit};

#[derive(Debug)]
pub enum ClaudeError {
    Spawn(std::io::Error),
    Exit { code: Option<i32>, message: String },
    InvalidJson(serde_json::Error),
    MissingResult,
    Reported(String),
    UsageLimit(UsageLimit),
    Timeout(Duration),
    SessionClosed(String),
    Mcp(String),
}

impl ClaudeError {
    pub fn is_session_loss(&self) -> bool {
        matches!(
            self,
            ClaudeError::SessionClosed(_) | ClaudeError::Timeout(_)
        )
    }

    pub fn usage_limit(&self) -> Option<UsageLimit> {
        match self {
            ClaudeError::UsageLimit(limit) => Some(*limit),
            _ => None,
        }
    }
}

impl LimitName {
    pub fn label(self) -> &'static str {
        match self {
            LimitName::FiveHour => "de 5 horas",
            LimitName::SevenDay => "semanal",
        }
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
            ClaudeError::UsageLimit(limit) => write!(
                f,
                "O limite de uso da janela {} foi atingido.",
                limit.window.label()
            ),
            ClaudeError::SessionClosed(reason) => {
                write!(f, "A sessão da CLI claude foi encerrada: {reason}")
            }
            ClaudeError::Mcp(reason) => {
                write!(f, "Não foi possível preparar o MCP do Vokia: {reason}")
            }
        }
    }
}
