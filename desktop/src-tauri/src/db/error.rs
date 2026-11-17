use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::ErrorCode;
use serde::Serialize;

#[derive(Debug)]
pub enum DbError {
    Io(PathBuf, std::io::Error),
    Corrupt(PathBuf),
    Inaccessible(PathBuf, rusqlite::Error),
    ReadOnly(PathBuf),
    Sqlite(rusqlite::Error),
    Poisoned,
    Unavailable(String),
    NoSelectedProject,
    ProjectNotFound(i64),
    ProjectBusy,
    ProjectHasNoFolder,
    ProjectPathTaken(String),
    InvalidProject(&'static str),
    InvalidSpeaker(String),
    InvalidAgent(String),
    InvalidSetting(String, String),
    ChannelNameTaken(String),
    ChannelNotFound,
    InvalidPermissionLevel(String),
    InvalidBacklogColumn(String),
    ColumnNotMovable(String),
    InvalidBacklogState(String),
    InvalidBacklogType(String),
    InvalidMicroTaskStatus(String),
    InvalidPlanState(String),
    InvalidPlanAgent(String),
    InvalidBacklogPlan(&'static str),
    BacklogItemNotFound(String),
    BacklogItemData { item_id: String, reason: String },
    MessageActivityData(String),
    EmptyCardTitle,
    DuplicateCardTitle(String),
    InvalidBacklogOrigin(String),
    AgentOriginNotAllowed,
    AgentForbiddenColumn(String),
    CardClosed(String),
    CardNotOwned(String),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DbError::Io(path, error) => write!(
                f,
                "Não foi possível criar a pasta de dados {}: {error}",
                path.display()
            ),
            DbError::Corrupt(path) => write!(
                f,
                "O banco de dados {} está corrompido ou não é um banco válido.",
                path.display()
            ),
            DbError::Inaccessible(path, error) => write!(
                f,
                "Sem acesso ao banco de dados {}: {error}",
                path.display()
            ),
            DbError::ReadOnly(path) => write!(
                f,
                "Sem acesso ao banco de dados {}: o arquivo está somente leitura.",
                path.display()
            ),
            DbError::Sqlite(error) => write!(f, "Erro no banco de dados: {error}"),
            DbError::Poisoned => write!(
                f,
                "O banco de dados ficou inconsistente após uma falha anterior."
            ),
            DbError::Unavailable(reason) => write!(f, "{reason}"),
            DbError::NoSelectedProject => write!(f, "Nenhum projeto está selecionado."),
            DbError::ProjectNotFound(id) => write!(f, "O projeto {id} não existe."),
            DbError::ProjectBusy => write!(
                f,
                "Há um pedido em andamento neste projeto; aguarde terminar."
            ),
            DbError::ProjectHasNoFolder => {
                write!(f, "Escolha a pasta do projeto antes de fixá-lo.")
            }
            DbError::ProjectPathTaken(path) => {
                write!(f, "Já existe um projeto na pasta {path}.")
            }
            DbError::InvalidProject(reason) => write!(f, "{reason}"),
            DbError::InvalidSpeaker(value) => {
                write!(f, "O histórico guarda um remetente desconhecido: {value}.")
            }
            DbError::InvalidAgent(value) => {
                write!(
                    f,
                    "O banco guarda uma pausa de agente desconhecido: {value}."
                )
            }
            DbError::InvalidSetting(key, value) => {
                write!(f, "A configuração {key} não aceita o valor {value:?}.")
            }
            DbError::ChannelNameTaken(name) => {
                write!(f, "Já existe um canal #{name} neste projeto.")
            }
            DbError::ChannelNotFound => write!(f, "Canal não encontrado."),
            DbError::InvalidPermissionLevel(value) => {
                write!(f, "O nível de permissão {value:?} não existe.")
            }
            DbError::InvalidBacklogColumn(value) => {
                write!(f, "O backlog não tem a coluna {value:?}.")
            }
            DbError::ColumnNotMovable(value) => {
                write!(
                    f,
                    "A coluna {value:?} é automática e não aceita movimento manual."
                )
            }
            DbError::InvalidBacklogState(value) => {
                write!(f, "O item do backlog não aceita o estado {value:?}.")
            }
            DbError::InvalidBacklogType(value) => {
                write!(f, "O item do backlog não aceita o tipo {value:?}.")
            }
            DbError::InvalidMicroTaskStatus(value) => {
                write!(f, "A micro-tarefa não aceita o status {value:?}.")
            }
            DbError::InvalidPlanState(value) => {
                write!(f, "O plano do item não aceita o estado {value:?}.")
            }
            DbError::InvalidPlanAgent(value) => {
                write!(f, "O plano do item não aceita o agente {value:?}.")
            }
            DbError::InvalidBacklogPlan(reason) => write!(f, "{reason}"),
            DbError::BacklogItemNotFound(id) => {
                write!(f, "O item {id} não existe no backlog deste projeto.")
            }
            DbError::BacklogItemData { item_id, reason } => write!(
                f,
                "Os dados do item {item_id} do backlog são inválidos: {reason}"
            ),
            DbError::MessageActivityData(reason) => {
                write!(f, "A atividade da mensagem é inválida: {reason}")
            }
            DbError::EmptyCardTitle => write!(f, "O título do card não pode ser vazio."),
            DbError::DuplicateCardTitle(id) => {
                write!(f, "Já existe o card {id} com esse título.")
            }
            DbError::InvalidBacklogOrigin(value) => write!(f, "A origem {value} não existe."),
            DbError::AgentOriginNotAllowed => {
                write!(f, "A origem agente só vale para card criado por agente.")
            }
            DbError::AgentForbiddenColumn(column) => {
                write!(f, "Agente não move card para {column}.")
            }
            DbError::CardClosed(id) => write!(f, "O card {id} já está em Done."),
            DbError::CardNotOwned(id) => {
                write!(f, "O card {id} não é seu nem do pedido atual.")
            }
        }
    }
}

impl From<rusqlite::Error> for DbError {
    fn from(error: rusqlite::Error) -> Self {
        DbError::Sqlite(error)
    }
}

pub(super) fn classify(path: &Path, error: rusqlite::Error) -> DbError {
    match error.sqlite_error_code() {
        Some(ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt) => {
            DbError::Corrupt(path.to_path_buf())
        }
        Some(ErrorCode::CannotOpen | ErrorCode::PermissionDenied | ErrorCode::ReadOnly) => {
            DbError::Inaccessible(path.to_path_buf(), error)
        }
        _ => DbError::Sqlite(error),
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbProblem {
    pub reason: String,
}
