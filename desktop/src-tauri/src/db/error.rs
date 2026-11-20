use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::ErrorCode;
use serde::Serialize;

use super::backlog::BACKLOG_LABEL_MAX_CHARS;
use super::orchestration::FlowProblem;

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
    BacklogLabelLength,
    DuplicateBacklogLabel(String),
    BacklogLabelNotFound(String),
    ResponsibleAlreadySet(String),
    ResponsibleOutsideTeam(String),
    AgentHasPosition(String),
    BacklogItemArchived,
    BacklogItemNotArchived,
    BacklogDeleteInProgress,
    BacklogDeleteWithPlan,
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
    OrchestratorNotPlaceable,
    FlowNodeOutsideTeam(String),
    ReturnMotiveRequired,
    InvalidClosedBy(String),
    FlowInvalid(Vec<FlowProblem>),
    OrchestratorNotLeader,
    RunNotInterrupted(i64),
    ArtifactNotFound(String),
    CriterionNotFound(String),
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
                | DbError::ReturnMotiveRequired
                | DbError::InvalidClosedBy(_)
                | DbError::CardClosed(_)
                | DbError::CardNotOwned(_)
                | DbError::CardInReady(_)
                | DbError::BacklogItemNotFound(_)
                | DbError::BacklogLabelLength
                | DbError::DuplicateBacklogLabel(_)
                | DbError::BacklogLabelNotFound(_)
                | DbError::ResponsibleAlreadySet(_)
                | DbError::ResponsibleOutsideTeam(_)
                | DbError::BacklogItemArchived
                | DbError::BacklogItemNotArchived
                | DbError::BacklogDeleteInProgress
                | DbError::BacklogDeleteWithPlan
                | DbError::InvalidPlanState(_)
                | DbError::InvalidPlanAgent(_)
                | DbError::InvalidBacklogPlan(_)
                | DbError::ArtifactNotFound(_)
                | DbError::CriterionNotFound(_)
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
            DbError::BacklogLabelLength => write!(
                f,
                "A etiqueta precisa ter de 1 a {BACKLOG_LABEL_MAX_CHARS} caracteres."
            ),
            DbError::DuplicateBacklogLabel(label) => {
                write!(f, "A etiqueta {label} já existe neste item.")
            }
            DbError::BacklogLabelNotFound(label) => {
                write!(f, "O item não tem a etiqueta {label}.")
            }
            DbError::ResponsibleAlreadySet(who) => {
                write!(f, "O item já tem o responsável {who}.")
            }
            DbError::ResponsibleOutsideTeam(agent) => {
                write!(f, "O agente {agent} não faz parte do elenco.")
            }
            DbError::AgentHasPosition(message) => f.write_str(message),
            DbError::BacklogItemArchived => write!(f, "O item já está arquivado."),
            DbError::BacklogItemNotArchived => write!(f, "O item não está arquivado."),
            DbError::BacklogDeleteInProgress => {
                write!(f, "Um item em andamento não pode ser removido.")
            }
            DbError::BacklogDeleteWithPlan => write!(
                f,
                "O item tem plano; remova o plano antes de excluir o item."
            ),
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
            DbError::FlowInvalid(problems) => {
                let messages: Vec<&str> = problems
                    .iter()
                    .map(|problem| problem.message.as_str())
                    .collect();
                f.write_str(&messages.join(" "))
            }
            DbError::ReturnMotiveRequired => write!(f, "Informe o motivo da devolução."),
            DbError::InvalidClosedBy(closer) => {
                write!(
                    f,
                    "Quem concluiu o card precisa ser usuario ou automatico, não {closer}."
                )
            }
            DbError::FlowNodeOutsideTeam(agent) => {
                write!(f, "O agente {agent} não está posicionado neste time.")
            }
            DbError::OrchestratorNotPlaceable => write!(
                f,
                "O orquestrador geral não pode ser posicionado em time ou camada."
            ),
            DbError::OrchestratorNotLeader => {
                write!(f, "O orquestrador geral não pode conduzir um time.")
            }
            DbError::RunNotInterrupted(id) => {
                write!(f, "O pedido {id} não está mais interrompido.")
            }
            DbError::ArtifactNotFound(id) => {
                write!(f, "O plano {id} não existe neste projeto.")
            }
            DbError::CriterionNotFound(id) => {
                write!(f, "O critério {id} não existe neste plano.")
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
