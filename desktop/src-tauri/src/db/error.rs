use std::fmt;
use std::path::{Path, PathBuf};

use rusqlite::ErrorCode;
use serde::Serialize;

use super::artifact_kinds::ARTIFACT_KINDS;
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
    InvalidAgent(String),
    InvalidSetting(String, String),
    ChannelNameTaken(String),
    ChannelNotFound,
    InvalidPermissionLevel(String),
    InvalidBacklogColumn(String),
    ColumnNotMovable(String),
    InvalidBacklogPriority(String),
    InvalidBacklogReorder(String),
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
    InvalidGitRefKind(String),
    EmptyGitRef,
    DuplicateGitRef(String),
    GitRefNotFound(String),
    ResponsibleNotAllowed(String),
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
    UnknownArtifactKind(String),
    ArtifactReadOnly(String),
    ArtifactConflict(String),
    ArtifactCheckNotFound(usize),
    ArtifactFieldTooLong { field: &'static str, max: usize },
    EmptyArtifactTitle,
    EmptyArtifactPatch,
    PlanNotFound,
    PlanStageStarted(String),
    PlanFileTooLarge { max: usize },
    PlanFileNotText,
    InvalidUsageQuery(String),
    CrossProjectAccess(String),
    MessageNotFound(i64),
    SprintNotFound(String),
    EmptySprintName,
    SprintFieldTooLong { field: &'static str, max: usize },
    InvalidSprintStatus(String),
    InvalidSprintPeriod,
    ActiveSprintExists(String),
    SprintClosed(String),
}

impl DbError {
    // Recusa que o agente pode corrigir; item corrompido (BacklogItemData) é erro de sistema.
    pub fn is_card_refusal(&self) -> bool {
        matches!(
            self,
            DbError::EmptyCardTitle
                | DbError::DuplicateCardTitle(_)
                | DbError::InvalidBacklogColumn(_)
                | DbError::InvalidBacklogPriority(_)
                | DbError::InvalidBacklogReorder(_)
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
                | DbError::InvalidGitRefKind(_)
                | DbError::EmptyGitRef
                | DbError::DuplicateGitRef(_)
                | DbError::GitRefNotFound(_)
                | DbError::ResponsibleNotAllowed(_)
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
                | DbError::UnknownArtifactKind(_)
                | DbError::ArtifactReadOnly(_)
                | DbError::ArtifactConflict(_)
                | DbError::ArtifactCheckNotFound(_)
                | DbError::ArtifactFieldTooLong { .. }
                | DbError::EmptyArtifactTitle
                | DbError::EmptyArtifactPatch
                | DbError::PlanNotFound
                | DbError::PlanStageStarted(_)
                | DbError::PlanFileTooLarge { .. }
                | DbError::PlanFileNotText
                | DbError::SprintNotFound(_)
                | DbError::EmptySprintName
                | DbError::SprintFieldTooLong { .. }
                | DbError::InvalidSprintStatus(_)
                | DbError::InvalidSprintPeriod
                | DbError::ActiveSprintExists(_)
                | DbError::SprintClosed(_)
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
            DbError::InvalidBacklogPriority(value) => {
                write!(
                    f,
                    "A prioridade {value:?} não é válida. Use Baixa, Média, Alta, Urgente ou vazio para limpar."
                )
            }
            DbError::InvalidBacklogReorder(reason) => f.write_str(reason),
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
            DbError::InvalidGitRefKind(kind) => {
                write!(f, "Tipo de referência Git inválido: {kind} (use commit, branch ou pr).")
            }
            DbError::EmptyGitRef => {
                write!(f, "A referência Git não pode ser vazia.")
            }
            DbError::DuplicateGitRef(id) => {
                write!(f, "A referência {id} já está vinculada a este item.")
            }
            DbError::GitRefNotFound(id) => {
                write!(f, "A referência {id} não está vinculada a este item.")
            }
            DbError::ResponsibleNotAllowed(item_id) => write!(
                f,
                "Só o Ember ou o responsável atual pode trocar o responsável do card {item_id}."
            ),
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
                write!(f, "O artefato {id} não existe neste projeto.")
            }
            DbError::CriterionNotFound(id) => {
                write!(f, "O critério {id} não existe neste plano.")
            }
            DbError::UnknownArtifactKind(kind) => {
                let kinds: Vec<&str> = ARTIFACT_KINDS.iter().map(|known| known.kind).collect();
                write!(
                    f,
                    "Tipo de artefato desconhecido: {kind}. Use {}.",
                    kinds.join(", ")
                )
            }
            DbError::ArtifactReadOnly(id) => {
                write!(f, "O artefato {id} é um plano e não pode ser alterado aqui; edite o plano pelo card.")
            }
            DbError::ArtifactConflict(id) => {
                write!(f, "O artefato {id} foi alterado depois da sua leitura; recarregue e tente de novo.")
            }
            DbError::ArtifactCheckNotFound(index) => {
                write!(f, "O artefato não tem o item de checklist de índice {index}.")
            }
            DbError::ArtifactFieldTooLong { field, max } => {
                write!(f, "O campo {field} do artefato passa de {max} caracteres.")
            }
            DbError::EmptyArtifactTitle => write!(f, "O título do artefato não pode ficar vazio."),
            DbError::EmptyArtifactPatch => {
                write!(f, "Informe ao menos um campo para atualizar o artefato.")
            }
            DbError::PlanNotFound => write!(f, "Este item não tem um plano próprio para editar."),
            DbError::PlanStageStarted(id) => {
                write!(f, "A etapa {id} já foi iniciada e não pode ser removida do plano.")
            }
            DbError::PlanFileTooLarge { max } => {
                write!(f, "O arquivo passa de {max} bytes; escolha um arquivo menor.")
            }
            DbError::PlanFileNotText => write!(f, "O arquivo não é um texto UTF-8 legível."),
            DbError::InvalidUsageQuery(reason) => f.write_str(reason),
            DbError::CrossProjectAccess(reason) => f.write_str(reason),
            DbError::MessageNotFound(id) => {
                write!(f, "A mensagem {id} não existe neste projeto.")
            }
            DbError::SprintNotFound(id) => {
                write!(f, "A sprint {id} não existe neste projeto.")
            }
            DbError::EmptySprintName => write!(f, "O nome da sprint não pode ficar vazio."),
            DbError::SprintFieldTooLong { field, max } => {
                write!(f, "O campo {field} da sprint passa de {max} caracteres.")
            }
            DbError::InvalidSprintStatus(status) => {
                write!(
                    f,
                    "A sprint não aceita o estado {status:?}. Use planejada, ativa ou encerrada."
                )
            }
            DbError::InvalidSprintPeriod => {
                write!(
                    f,
                    "A data de término da sprint não pode ser anterior à data de início."
                )
            }
            DbError::ActiveSprintExists(id) => {
                write!(
                    f,
                    "Já existe a sprint ativa {id} neste projeto; encerre-a antes de ativar outra."
                )
            }
            DbError::SprintClosed(id) => {
                write!(f, "A sprint {id} está encerrada e não aceita novos vínculos.")
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
