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
    MessageProposalData(String),
    EmptyCardTitle,
    DuplicateCardTitle(String),
    InvalidBacklogOrigin(String),
    AgentOriginNotAllowed,
    AgentForbiddenColumn(String),
    CardClosed(String),
    CardNotOwned(String),
    CardInReady(String),
    InvalidOrchestrationField {
        field: &'static str,
        value: String,
    },
    OrchestrationOutOfRange {
        field: &'static str,
        min: i64,
        max: i64,
    },
    OrchestrationInstructionsTooLong {
        max: usize,
    },
    OrchestrationNameEmpty,
    ReservedLayerName(String),
    LayerNameTaken(String),
    LayerNotFound(String),
    LayerHasAgents(String),
    LayerOrderInvalid,
    TeamNameTaken(String),
    TeamNotFound(String),
    TeamHasAgents(String),
    RuleNotFound(i64),
    RuleDuplicated {
        from: String,
        to: String,
    },
    RuleSelfLoop,
    RuleEscalateTarget,
    RuleVoceNeedsEscalate,
    RuleReturnNotBackward,
}

impl DbError {
    // Recusa que o agente pode corrigir; item corrompido (BacklogItemData) é erro de sistema.
    pub fn is_card_refusal(&self) -> bool {
        matches!(
            self,
            DbError::EmptyCardTitle
                | DbError::DuplicateCardTitle(_)
                | DbError::InvalidBacklogColumn(_)
                | DbError::AgentForbiddenColumn(_)
                | DbError::CardClosed(_)
                | DbError::CardNotOwned(_)
                | DbError::CardInReady(_)
                | DbError::BacklogItemNotFound(_)
        )
    }
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
            DbError::MessageProposalData(reason) => {
                write!(f, "O pedido proposto na mensagem é inválido: {reason}")
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
            DbError::CardInReady(id) => {
                write!(f, "O card {id} está em Ready: só o usuário altera.")
            }
            DbError::InvalidOrchestrationField { field, value } => write!(
                f,
                "O campo {field} da orquestração não aceita o valor {value:?}."
            ),
            DbError::OrchestrationOutOfRange { field, min, max } => write!(
                f,
                "O campo {field} da orquestração deve ficar entre {min} e {max}."
            ),
            DbError::OrchestrationInstructionsTooLong { max } => {
                write!(f, "As instruções aceitam até {max} caracteres.")
            }
            DbError::OrchestrationNameEmpty => {
                write!(f, "O nome precisa ter letras ou números.")
            }
            DbError::ReservedLayerName(name) => write!(
                f,
                "{name:?} é o nó Você e não pode ser o nome de uma camada."
            ),
            DbError::LayerNameTaken(id) => {
                write!(
                    f,
                    "Já existe uma camada com o identificador {id} neste projeto."
                )
            }
            DbError::LayerNotFound(id) => write!(f, "A camada {id} não existe neste projeto."),
            DbError::LayerHasAgents(id) => write!(
                f,
                "A camada {id} tem agentes posicionados: mova os agentes antes de excluí-la."
            ),
            DbError::LayerOrderInvalid => write!(
                f,
                "A nova ordem precisa listar cada camada do projeto uma única vez."
            ),
            DbError::TeamNameTaken(id) => {
                write!(
                    f,
                    "Já existe um time com o identificador {id} neste projeto."
                )
            }
            DbError::TeamNotFound(id) => write!(f, "O time {id} não existe neste projeto."),
            DbError::TeamHasAgents(id) => write!(
                f,
                "O time {id} tem agentes posicionados: mova os agentes antes de excluí-lo."
            ),
            DbError::RuleNotFound(id) => write!(f, "A seta {id} não existe neste projeto."),
            DbError::RuleDuplicated { from, to } => {
                write!(f, "Já existe uma seta de {from} para {to}.")
            }
            DbError::RuleSelfLoop => {
                write!(f, "Uma camada não pode enviar trabalho para ela mesma.")
            }
            DbError::RuleEscalateTarget => {
                write!(f, "A regra escalar só pode apontar para Você.")
            }
            DbError::RuleVoceNeedsEscalate => {
                write!(f, "Você só recebe trabalho pela regra escalar.")
            }
            DbError::RuleReturnNotBackward => {
                write!(f, "Devolver só vai para uma camada anterior.")
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
