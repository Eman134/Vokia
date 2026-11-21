use std::fmt;
use std::time::SystemTimeError;

use serde::Serialize;

use crate::agents::AgentError;
use crate::attachments::AttachmentError;
use crate::claude::{ClaudeError, UsageLimit};
use crate::db::DbError;
use crate::projects::worktree::WorktreeError;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RequestOutcome {
    Finished,
    Failed,
    Queued,
    Paused,
}

#[derive(Debug)]
pub enum FlowError {
    CliUnavailable(String),
    DatabaseUnavailable(String),
    Claude(ClaudeError),
    Emit(tauri::Error),
    CachePoisoned,
    Cache(String),
    QueuePoisoned,
    ArchivePoisoned,
    ArchivePath(tauri::Error),
    AttachmentsDir(AttachmentError),
    ArchiveIo(std::io::Error),
    ArchiveJson(serde_json::Error),
    Clock(SystemTimeError),
    History(DbError),
    ProjectFolder(String),
    DecisionsPoisoned,
    UnknownDecision,
    SessionStore(DbError),
    Settings(DbError),
    Orchestration(DbError),
    Runs(DbError),
    Team(AgentError),
    Worktree(WorktreeError),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::CliUnavailable(reason) => write!(f, "{reason}"),
            FlowError::DatabaseUnavailable(reason) => write!(f, "{reason}"),
            FlowError::Claude(error) => write!(f, "{error}"),
            FlowError::Emit(error) => write!(f, "Não foi possível emitir o evento: {error}"),
            FlowError::CachePoisoned => write!(
                f,
                "O cache de variáveis ficou inconsistente após uma falha anterior."
            ),
            FlowError::Cache(reason) => write!(f, "{reason}"),
            FlowError::QueuePoisoned => write!(
                f,
                "A fila de pedidos ficou inconsistente após uma falha anterior."
            ),
            FlowError::ArchivePoisoned => write!(
                f,
                "O arquivo de tarefas ficou inconsistente após uma falha anterior."
            ),
            FlowError::ArchivePath(error) => write!(
                f,
                "Não foi possível localizar a pasta de dados do app: {error}"
            ),
            FlowError::AttachmentsDir(error) => {
                write!(f, "Não foi possível preparar a pasta de anexos: {error}")
            }
            FlowError::ArchiveIo(error) => {
                write!(f, "Não foi possível gravar o arquivo de tarefas: {error}")
            }
            FlowError::ArchiveJson(error) => {
                write!(
                    f,
                    "Não foi possível serializar o arquivo de tarefas: {error}"
                )
            }
            FlowError::Clock(error) => {
                write!(f, "O relógio do sistema está antes de 1970: {error}")
            }
            FlowError::History(error) => {
                write!(f, "Não foi possível gravar o histórico: {error}")
            }
            FlowError::ProjectFolder(reason) => write!(f, "{reason}"),
            FlowError::DecisionsPoisoned => write!(
                f,
                "As decisões pendentes ficaram inconsistentes após uma falha anterior."
            ),
            FlowError::UnknownDecision => write!(f, "Esta decisão não existe mais."),
            FlowError::SessionStore(error) => {
                write!(f, "Não foi possível guardar a sessão do agente: {error}")
            }
            FlowError::Settings(error) => {
                write!(f, "Não foi possível ler as configurações: {error}")
            }
            FlowError::Orchestration(error) => {
                write!(f, "Não foi possível ler a orquestração: {error}")
            }
            FlowError::Runs(error) => {
                write!(f, "Não foi possível atualizar os pedidos interrompidos: {error}")
            }
            FlowError::Team(error) => write!(f, "{error}"),
            FlowError::Worktree(error) => write!(f, "{error}"),
        }
    }
}

impl FlowError {
    pub fn usage_limit(&self) -> Option<UsageLimit> {
        match self {
            FlowError::Claude(error) => error.usage_limit(),
            _ => None,
        }
    }

    pub fn is_limit(&self) -> bool {
        match self {
            FlowError::Claude(error) => error.is_limit(),
            _ => false,
        }
    }

    pub fn is_cli_failure(&self) -> bool {
        matches!(self, FlowError::Claude(_))
    }

    #[allow(dead_code)]
    pub fn is_model_unavailable(&self) -> bool {
        match self {
            FlowError::Claude(error) => error.is_model_unavailable(),
            _ => false,
        }
    }
}

impl From<ClaudeError> for FlowError {
    fn from(error: ClaudeError) -> Self {
        FlowError::Claude(error)
    }
}

impl From<tauri::Error> for FlowError {
    fn from(error: tauri::Error) -> Self {
        FlowError::Emit(error)
    }
}

impl From<DbError> for FlowError {
    fn from(error: DbError) -> Self {
        FlowError::History(error)
    }
}
