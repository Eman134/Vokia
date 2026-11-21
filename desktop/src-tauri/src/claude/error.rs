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
    ForCli {
        cli: String,
        inner: Box<ClaudeError>,
    },
}

impl ClaudeError {
    pub fn with_cli(self, cli: impl Into<String>) -> Self {
        let cli = cli.into();
        match self {
            ClaudeError::ForCli { inner, .. } => ClaudeError::ForCli { cli, inner },
            other => ClaudeError::ForCli {
                cli,
                inner: Box::new(other),
            },
        }
    }

    pub fn is_session_loss(&self) -> bool {
        match self {
            ClaudeError::ForCli { inner, .. } => inner.is_session_loss(),
            ClaudeError::SessionClosed(_) | ClaudeError::Timeout(_) => true,
            _ => false,
        }
    }

    pub fn usage_limit(&self) -> Option<UsageLimit> {
        match self {
            ClaudeError::ForCli { inner, .. } => inner.usage_limit(),
            ClaudeError::UsageLimit(limit) => Some(*limit),
            _ => None,
        }
    }

    pub fn is_limit(&self) -> bool {
        match self {
            ClaudeError::ForCli { inner, .. } => inner.is_limit(),
            ClaudeError::UsageLimit(_) | ClaudeError::LimitExhausted => true,
            ClaudeError::Reported(message) | ClaudeError::Exit { message, .. } => {
                is_limit_message(message)
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn is_model_unavailable(&self) -> bool {
        match self {
            ClaudeError::ForCli { inner, .. } => inner.is_model_unavailable(),
            ClaudeError::ModelUnavailable { .. } => true,
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn model_unavailable(&self) -> Option<(&str, &str, &str)> {
        match self {
            ClaudeError::ForCli { inner, .. } => inner.model_unavailable(),
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

impl ClaudeError {
    fn fmt_with_cli(&self, cli: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClaudeError::ForCli { cli, inner } => inner.fmt_with_cli(cli, f),
            ClaudeError::Spawn(error) => {
                write!(f, "Não foi possível executar a CLI {cli}: {error}")
            }
            ClaudeError::Exit { message, .. } if is_limit_message(message) => {
                f.write_str("Limite esgotado")
            }
            ClaudeError::Exit {
                code: Some(code),
                message,
            } => {
                write!(f, "A CLI {cli} saiu com código {code}: {message}")
            }
            ClaudeError::Exit {
                code: None,
                message,
            } => {
                write!(f, "A CLI {cli} foi interrompida: {message}")
            }
            ClaudeError::InvalidJson(error) => {
                write!(f, "A CLI {cli} devolveu um JSON inválido: {error}")
            }
            ClaudeError::MissingResult => write!(
                f,
                "A CLI {cli} terminou sem a linha de resultado do stream-json."
            ),
            ClaudeError::Timeout(limit) => {
                write!(
                    f,
                    "A CLI {cli} não respondeu em {} s e foi encerrada.",
                    limit.as_secs()
                )
            }
            ClaudeError::Reported(message) if is_limit_message(message) => {
                f.write_str("Limite esgotado")
            }
            ClaudeError::Reported(message) => {
                write!(f, "A CLI {cli} devolveu um erro: {message}")
            }
            ClaudeError::UsageLimit(_) | ClaudeError::LimitExhausted => {
                f.write_str("Limite esgotado")
            }
            ClaudeError::ModelUnavailable {
                provider,
                model,
                detail,
            } => write!(f, "Modelo {model} indisponível em {provider}: {detail}"),
            ClaudeError::SessionClosed(reason) => {
                write!(f, "A sessão da CLI {cli} foi encerrada: {reason}")
            }
            ClaudeError::Mcp(reason) => {
                write!(f, "Não foi possível preparar o MCP do Vokia: {reason}")
            }
        }
    }
}

impl fmt::Display for ClaudeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_cli("claude", f)
    }
}
