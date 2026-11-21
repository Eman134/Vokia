use std::fmt;
use std::time::Duration;

use super::stream::UsageLimit;

#[derive(Debug)]
pub enum ClaudeError {
    Spawn(std::io::Error),
    Exit { code: Option<i32>, message: String },
    InvalidJson(serde_json::Error),
    MissingResult,
    Reported(String),
    UsageLimit(UsageLimit),
    LimitExhausted,
    ModelUnavailable {
        provider: String,
        model: String,
        detail: String,
    },
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

    pub fn is_limit(&self) -> bool {
        match self {
            ClaudeError::UsageLimit(_) | ClaudeError::LimitExhausted => true,
            ClaudeError::Reported(message) | ClaudeError::Exit { message, .. } => {
                is_limit_message(message)
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn is_model_unavailable(&self) -> bool {
        matches!(self, ClaudeError::ModelUnavailable { .. })
    }

    #[allow(dead_code)]
    pub fn model_unavailable(&self) -> Option<(&str, &str, &str)> {
        match self {
            ClaudeError::ModelUnavailable {
                provider,
                model,
                detail,
            } => Some((provider.as_str(), model.as_str(), detail.as_str())),
            _ => None,
        }
    }
}

pub fn classify_provider_error(
    provider: &str,
    model: &str,
    message: &str,
    limit_patterns: &[String],
    unavailable_patterns: &[String],
) -> Option<ClaudeError> {
    let lower = message.to_ascii_lowercase();
    if unavailable_patterns
        .iter()
        .any(|pattern| !pattern.is_empty() && lower.contains(&pattern.to_ascii_lowercase()))
    {
        return Some(ClaudeError::ModelUnavailable {
            provider: provider.to_string(),
            model: model.to_string(),
            detail: message.to_string(),
        });
    }
    if is_limit_message(message)
        || limit_patterns
            .iter()
            .any(|pattern| !pattern.is_empty() && lower.contains(&pattern.to_ascii_lowercase()))
    {
        return Some(ClaudeError::LimitExhausted);
    }
    None
}

fn is_limit_message(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("usage_limit_reached")
        || lower.contains("hit your limit")
        || lower.contains("out of extra usage")
        || lower.contains("rate_limit_error")
        || lower.contains("limite esgotado")
}

impl fmt::Display for ClaudeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClaudeError::Spawn(error) => {
                write!(f, "Não foi possível executar a CLI claude: {error}")
            }
            ClaudeError::Exit { message, .. } if is_limit_message(message) => {
                f.write_str("Limite esgotado")
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
            ClaudeError::Reported(message) if is_limit_message(message) => {
                f.write_str("Limite esgotado")
            }
            ClaudeError::Reported(message) => write!(f, "A CLI claude devolveu um erro: {message}"),
            ClaudeError::UsageLimit(_) | ClaudeError::LimitExhausted => {
                f.write_str("Limite esgotado")
            }
            ClaudeError::ModelUnavailable {
                provider,
                model,
                detail,
            } => write!(f, "Modelo {model} indisponível em {provider}: {detail}"),
            ClaudeError::SessionClosed(reason) => {
                write!(f, "A sessão da CLI claude foi encerrada: {reason}")
            }
            ClaudeError::Mcp(reason) => {
                write!(f, "Não foi possível preparar o MCP do Vokia: {reason}")
            }
        }
    }
}
