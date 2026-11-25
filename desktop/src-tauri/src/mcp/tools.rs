use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::agents::{Agent, AgentRegistry, Team};
use crate::commands::artifacts::notify_artifacts_changed;
use crate::commands::channels::post_agent_message;
use crate::commands::dispatcher::list_commands_filtered;
use crate::commands::orchestration::{read_movements, read_orchestration};
use crate::db::{
    artifact_id, board_column, plan_stage_number, AgentActor, BacklogItem, CardOperation, CardOutcome,
    CardResponsible, Db, DbError, DbState, ItemPlanInput, OrchestrationSnapshot, AGENT_FORBIDDEN_COLUMNS, ARTIFACT_KINDS,
    ArtifactDocumentPatch, BACKLOG_LABEL_MAX_CHARS, BOARD_COLUMNS, NewArtifactDocument,
    REQUEST_KIND_VALUES, WHEN_EVENT_VALUES,
};
use crate::events::{DecisionKind, RequestProposal};
use crate::orchestration::policy::check_intent;
use crate::orchestration::snapshot::{find_team, team_leader, team_leaders, TeamLeader};
use crate::orchestrator::{emit_backlog_changed, start_approved_plan, variables_listing, with_cache};
use crate::state::{AppState, MAX_DELEGATION_PING_SECS, MIN_DELEGATION_PING_SECS};
use crate::task::{Change, VarValue};

use super::registry::Binding;
use super::slot::{Delegation, DelegationIntent, SlotDecision};

const LIST_BACKLOG: &str = "list_backlog";
const READ_CARD: &str = "read_card";
const CREATE_CARD: &str = "create_card";
const MOVE_CARD: &str = "move_card";
const UPDATE_CARD: &str = "update_card";
const BACKLOG_ADD_LABEL: &str = "backlog_add_label";
const BACKLOG_REMOVE_LABEL: &str = "backlog_remove_label";
const BACKLOG_SET_RESPONSIBLE: &str = "backlog_set_responsible";
const BACKLOG_ARCHIVE_ITEM: &str = "backlog_archive_item";
const BACKLOG_RESTORE_ITEM: &str = "backlog_restore_item";
const BACKLOG_DELETE_ITEM: &str = "backlog_delete_item";
const REQUEST_DECISION: &str = "request_decision";
const PROPOSE_REQUEST: &str = "propose_request";
const DELEGATE: &str = "delegate";
const GET_VARIABLES: &str = "get_variables";
const SET_VARIABLE: &str = "set_variable";
const GET_ORCHESTRATION: &str = "get_orchestration";
const LIST_AGENTS: &str = "list_agents";
const SAVE_ITEM_PLAN: &str = "save_item_plan";
const ARTIFACT_SET_CRITERION: &str = "artifact_set_criterion";
const ARTIFACT: &str = "artifact";
const ARTIFACT_OP_LIST: &str = "list";
const ARTIFACT_OP_READ: &str = "read";
const ARTIFACT_OP_CREATE: &str = "create";
const ARTIFACT_OP_UPDATE: &str = "update";
const ARTIFACT_OP_CHECK: &str = "check";
const ARTIFACT_OPS: [&str; 5] = [
    ARTIFACT_OP_LIST,
    ARTIFACT_OP_READ,
    ARTIFACT_OP_CREATE,
    ARTIFACT_OP_UPDATE,
    ARTIFACT_OP_CHECK,
];
const LIST_MOVEMENTS: &str = "list_movements";
const POST_CHANNEL_MESSAGE: &str = "post_channel_message";
const APP_COMMANDS_LIST: &str = "app_commands_list";
pub(super) const APP_COMMAND_CALL: &str = "app_command_call";
const COMMAND_DOMAINS: [&str; 12] = [
    "team",
    "projects",
    "channels",
    "backlog",
    "artifacts",
    "attachments",
    "agents",
    "roles",
    "orchestration",
    "providers",
    "settings",
    "search",
];
// Administração do Vokia e manutenção do mapa ficam com quem coordena: cada definição a menos sai do contexto
// de toda chamada à API dos demais agentes.
const COORDINATION_TOOLS: [&str; 8] = [
    BACKLOG_ARCHIVE_ITEM,
    BACKLOG_RESTORE_ITEM,
    BACKLOG_DELETE_ITEM,
    GET_ORCHESTRATION,
    LIST_AGENTS,
    LIST_MOVEMENTS,
    APP_COMMANDS_LIST,
    APP_COMMAND_CALL,
];
const OP_SET: &str = "set";
const OP_APPEND: &str = "append";

const MAX_TITLE_CHARS: usize = 200;
const LIST_LIMIT: usize = 50;
const RECENT_ACTIVITY_COUNT: usize = 5;
const DEFAULT_COLUMN: &str = "Backlog";

const TITLE_TOO_LONG: &str = "O título do card passa de 200 caracteres.";
const SYSTEM_FAILURE: &str = "Não foi possível acessar o backlog agora. Tente de novo mais tarde.";
const EMPTY_BACKLOG: &str = "Nenhum card.";
const NO_DESCRIPTION: &str = "sem descrição";
const UPDATE_WITHOUT_FIELDS: &str =
    "Informe o novo título ou a nova descrição: sem nenhum dos dois não há o que atualizar.";

const VARIABLES_FAILURE: &str =
    "Não foi possível acessar as variáveis agora. Tente de novo mais tarde.";
const NO_VARIABLES_IN_EXCHANGE: &str =
    "Esta tarefa não usa variáveis: não há o que registrar. Responda só com texto.";
const INVALID_OP: &str = "Operação inválida. Use set ou append.";
const EMPTY_VALUE: &str = "O valor não pode ser vazio.";
const NON_INTEGER_VALUE: &str = "O valor numérico precisa ser um inteiro.";
const UNSUPPORTED_VALUE: &str = "O valor precisa ser um texto ou um número inteiro.";

const MAX_DELEGATIONS_PER_REPLY: usize = 5;
const DELEGATION_REGISTERED: &str = "Delegação registrada.";
const DELEGATION_CONTINUE: &str =
    "Pode registrar outra delegação; quando não houver mais pedidos, termine a resposta.";
const DELEGATION_DUPLICATE: &str =
    "Delegação repetida ignorada: esse agente já recebeu este tipo de pedido neste item nesta resposta.";
const DELEGATION_LIMIT: &str = "Limite de delegações por resposta atingido.";
const DELEGATION_GENERAL_KIND: &str = "geral";
const DELEGATION_NO_ITEM: &str = "sem item";
const DELEGATION_NOTICE_STATUS: &str = "aviso enviado";
const DELEGATION_PENDING_STATUS: &str = "pendente";
const DELEGATION_EMPTY_REQUEST: &str = "O pedido da delegação não pode ser vazio.";
const DELEGATION_TO_SELF: &str = "Você não pode delegar a si mesmo.";

const DECISION_REGISTERED: &str = "Decisão registrada. Termine a resposta agora.";
const DECISION_ALREADY_REGISTERED: &str =
    "Já há uma decisão registrada nesta resposta. Termine a resposta agora.";
const DECISION_EMPTY_TITLE: &str = "O título da decisão não pode ser vazio.";

const PROPOSAL_REGISTERED: &str = "Proposta registrada. Escreva a resposta normalmente.";
const PROPOSAL_ALREADY_REGISTERED: &str = "Já há um pedido proposto nesta resposta.";
const PROPOSAL_EMPTY_TITLE: &str = "O título do pedido não pode ser vazio.";
const DECISION_CHOICE_WITHOUT_OPTIONS: &str = "Uma decisão do tipo escolha precisa de opções.";
const CHECKED_MARK: &str = "[x]";
const UNCHECKED_MARK: &str = "[ ]";
const CRITERION_DONE: &str = "marcado";
const CRITERION_UNDONE: &str = "desmarcado";

// Falha de negócio volta ao agente com a frase do banco; falha de sistema não expõe detalhe.
type ToolText = Result<String, String>;

#[derive(Deserialize)]
struct ListArgs {
    column: Option<String>,
}

#[derive(Deserialize)]
struct LabelArgs {
    item_id: String,
    label: String,
}

#[derive(Deserialize)]
struct ResponsibleArgs {
    item_id: String,
    agent: String,
}

#[derive(Deserialize)]
struct ItemArgs {
    item_id: String,
}

#[derive(Deserialize)]
struct ReadArgs {
    id: String,
}

#[derive(Deserialize)]
struct CreateArgs {
    title: String,
    description: Option<String>,
    team: Option<String>,
}

#[derive(Deserialize)]
struct UpdateArgs {
    id: String,
    title: Option<String>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct MoveArgs {
    id: String,
    column: String,
}

#[derive(Deserialize)]
struct SetVariableArgs {
    name: String,
    op: String,
    value: Value,
    share: Option<bool>,
}

#[derive(Deserialize)]
struct DelegateArgs {
    agent: String,
    request: String,
    kind: Option<String>,
    event: Option<String>,
    item_id: Option<String>,
    ping_seconds: Option<u64>,
    notify: Option<bool>,
}

#[derive(Deserialize)]
struct DecisionArgs {
    kind: String,
    title: String,
    detail: Option<String>,
    options: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ProposalArgs {
    title: String,
    detail: Option<String>,
}

#[derive(Deserialize)]
struct CommandsListArgs {
    domain: Option<String>,
}

#[derive(Deserialize)]
struct PostChannelArgs {
    channel: String,
    text: String,
}

#[derive(Deserialize)]
struct SavePlanArgs {
    item_id: String,
    #[serde(flatten)]
    plan: ItemPlanInput,
}

#[derive(Deserialize)]
struct CriterionArgs {
    artifact_id: String,
    criterion_id: String,
    done: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolScope {
    Coordinator,
    Worker,
}

impl ToolScope {
    pub fn allows(self, name: &str) -> bool {
        self == ToolScope::Coordinator || !COORDINATION_TOOLS.contains(&name)
    }
}

// Teto explícito do payload JSON de `tools/list` (baseline atual: ~10.5 KB Coordinator / ~8.5 KB Worker).
// Impede inflação acidental de tokens fixos pagos a cada turno; novos comandos devem entrar em `commands/dispatcher.rs`.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const TOOLS_LIST_COORDINATOR_MAX_BYTES: usize = 11_200;
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) const TOOLS_LIST_WORKER_MAX_BYTES: usize = 9_200;

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn definitions_for_scope(scope: ToolScope) -> Vec<Value> {
    definitions()
        .into_iter()
        .filter(|definition| {
            let name = definition
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            scope.allows(name)
        })
        .collect()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn tools_list_payload_bytes(definitions: &[Value]) -> usize {
    serde_json::to_vec(&json!({ "tools": definitions }))
        .map(|bytes| bytes.len())
        .unwrap_or(usize::MAX)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn check_tools_list_budget(
    scope_label: &str,
    definitions: &[Value],
    max_bytes: usize,
) -> Result<usize, String> {
    let bytes = tools_list_payload_bytes(definitions);
    if bytes > max_bytes {
        Err(format!(
            "Orçamento de contexto de tools/list ({scope_label}) excedido: {bytes} bytes > teto de {max_bytes} bytes. \
             Evite adicionar texto excessivo ou novas ferramentas diretas em tools/list (aumenta tokens fixos por sessão); \
             exponha comandos sob demanda em commands/dispatcher.rs."
        ))
    } else {
        Ok(bytes)
    }
}

pub(crate) fn is_tool(name: &str) -> bool {
    definitions()
        .iter()
        .any(|definition| definition["name"].as_str() == Some(name))
}

pub fn definitions() -> Vec<Value> {
    let movable: Vec<&str> = BOARD_COLUMNS
        .iter()
        .copied()
        .filter(|column| !AGENT_FORBIDDEN_COLUMNS.contains(column))
        .collect();
    let decision_kinds = decision_kind_names();
    let artifact_kinds: Vec<&str> = ARTIFACT_KINDS.iter().map(|kind| kind.kind).collect();
    vec![
        tool(
            LIST_BACKLOG,
            "Lista os cards do backlog do projeto (até 50, os mais recentes).",
            json!({ "column": { "type": "string", "enum": BOARD_COLUMNS } }),
            &[],
        ),
        tool(
            READ_CARD,
            "Lê um card do backlog: coluna, dono, origem, descrição e atividades recentes.",
            json!({ "id": { "type": "string" } }),
            &["id"],
        ),
        tool(
            CREATE_CARD,
            "Cria um card no backlog já em triagem (leia o código, pergunte via request_decision se faltar definição e grave o plano via save_item_plan antes de delegar a implementação); o título não pode repetir o de outro card.",
            json!({
                "title": { "type": "string", "maxLength": MAX_TITLE_CHARS },
                "description": { "type": "string" },
                "team": { "type": "string", "description": "Id ou nome do time responsável; só o Ember usa, e o líder disponível do time passa a ser o responsável do card." },
            }),
            &["title"],
        ),
        tool(
            MOVE_CARD,
            "Move um card seu, ou o da conversa atual, para outra coluna.",
            json!({
                "id": { "type": "string" },
                "column": { "type": "string", "enum": movable },
            }),
            &["id", "column"],
        ),
        tool(
            UPDATE_CARD,
            "Edita o título ou a descrição de um card seu, ou do da conversa atual; descrição vazia não apaga a atual.",
            json!({
                "id": { "type": "string" },
                "title": { "type": "string", "maxLength": MAX_TITLE_CHARS },
                "description": { "type": "string" },
            }),
            &["id"],
        ),
        tool(
            BACKLOG_ADD_LABEL,
            "Adiciona uma etiqueta a um card do backlog; a etiqueta não pode repetir uma que o card já tem, sem diferença de maiúsculas.",
            json!({
                "item_id": { "type": "string" },
                "label": { "type": "string", "maxLength": BACKLOG_LABEL_MAX_CHARS },
            }),
            &["item_id", "label"],
        ),
        tool(
            BACKLOG_REMOVE_LABEL,
            "Remove uma etiqueta de um card do backlog.",
            json!({
                "item_id": { "type": "string" },
                "label": { "type": "string" },
            }),
            &["item_id", "label"],
        ),
        tool(
            BACKLOG_SET_RESPONSIBLE,
            "Define ou troca o agente responsável por um card do backlog; trocar um responsável já definido cabe ao Ember ou ao responsável atual, e indicar o próprio responsável não faz nada.",
            json!({
                "item_id": { "type": "string" },
                "agent": { "type": "string" },
            }),
            &["item_id", "agent"],
        ),
        tool(
            BACKLOG_ARCHIVE_ITEM,
            "Arquiva um card do backlog: ele sai do quadro e das listagens, mas pode ser restaurado.",
            json!({ "item_id": { "type": "string" } }),
            &["item_id"],
        ),
        tool(
            BACKLOG_RESTORE_ITEM,
            "Restaura um card arquivado do backlog.",
            json!({ "item_id": { "type": "string" } }),
            &["item_id"],
        ),
        tool(
            BACKLOG_DELETE_ITEM,
            "Remove um card do backlog de vez. Não remove card em andamento nem card com plano.",
            json!({ "item_id": { "type": "string" } }),
            &["item_id"],
        ),
        tool(
            REQUEST_DECISION,
            "Pede uma decisão ao usuário e não espera a resposta: depois de chamar, termine a resposta.",
            json!({
                "kind": { "type": "string", "enum": decision_kinds },
                "title": { "type": "string" },
                "detail": { "type": "string" },
                "options": { "type": "array", "items": { "type": "string" } },
            }),
            &["kind", "title"],
        ),
        tool(
            PROPOSE_REQUEST,
            "Indica que esta resposta merece virar um pedido no backlog; o usuário decide com um botão. Não espera resposta: continue e escreva o texto normalmente.",
            json!({
                "title": { "type": "string", "maxLength": MAX_TITLE_CHARS },
                "detail": { "type": "string" },
            }),
            &["title"],
        ),
        tool(
            DELEGATE,
            "Delega um pedido a outro agente do time sem esperar a resposta. Pode chamar de novo para outros pedidos da mesma resposta (até 5); com notify true só avisa o agente, sem dar trabalho. Quando não houver mais pedidos, termine a resposta.",
            json!({
                "agent": { "type": "string" },
                "request": { "type": "string" },
                "kind": { "type": "string", "enum": REQUEST_KIND_VALUES },
                "event": { "type": "string", "enum": WHEN_EVENT_VALUES },
                "item_id": { "type": "string" },
                "ping_seconds": { "type": "integer", "minimum": MIN_DELEGATION_PING_SECS, "maximum": MAX_DELEGATION_PING_SECS },
                "notify": { "type": "boolean" },
            }),
            &["agent", "request"],
        ),
        tool(
            GET_VARIABLES,
            "Lista as variáveis da tarefa que você pode ver, com tipo, escopo e valor.",
            json!({}),
            &[],
        ),
        tool(
            SET_VARIABLE,
            "Grava uma variável da tarefa. Variáveis da tarefa são compartilhadas por padrão: todos os agentes leem e alteram. Use share false só para rascunho pessoal, visível apenas a você; share só vale ao criar a variável. op set troca o valor; op append acrescenta um item a uma lista.",
            json!({
                "name": { "type": "string" },
                "op": { "type": "string", "enum": [OP_SET, OP_APPEND] },
                "value": { "type": ["string", "integer"] },
                "share": { "type": "boolean" },
            }),
            &["name", "op", "value"],
        ),
        tool(
            GET_ORCHESTRATION,
            "Lê os times, as camadas e as regras de passagem do Vokia, com a posição de cada agente.",
            json!({}),
            &[],
        ),
        tool(
            LIST_AGENTS,
            "Lista os agentes do elenco, com nome, papel e instruções.",
            json!({}),
            &[],
        ),
        tool(
            SAVE_ITEM_PLAN,
            &format!(
                "Grava o plano de um card do backlog. state vazio limpa o plano; proposto exige proposal; vinculado exige plan com id (ID público curto), name (título bem resumido) e meta (estimativa para execução por LLM em minutos, ex: '~5min', '~15min'). {}",
                crate::plan_proposal::PLAN_PROTOCOL_HINT
            ),
            json!({
                "item_id": { "type": "string" },
                "state": { "type": "string" },
                "proposal": { "type": "string", "description": crate::plan_proposal::PLAN_PROTOCOL_HINT },
                "plan": {
                    "type": "object",
                    "properties": {
                        "id": { "type": "string" },
                        "name": { "type": "string" },
                        "meta": { "type": "string" },
                    },
                },
                "agent": { "type": "string" },
            }),
            &["item_id", "state"],
        ),
        tool(
            ARTIFACT_SET_CRITERION,
            "Marca ou desmarca um critério do plano de um card. artifact_id é plano-<id do card>; criterion_id é c<n>, onde n é o número da etapa do plano.",
            json!({
                "artifact_id": { "type": "string" },
                "criterion_id": { "type": "string" },
                "done": { "type": "boolean" },
            }),
            &["artifact_id", "criterion_id", "done"],
        ),
        tool(
            ARTIFACT,
            "Gerencia artefatos do projeto (documentos, especificações, checklists, decisões, arquiteturas). Opções de op: list, read, create, update, check.",
            json!({
                "op": { "type": "string", "enum": ARTIFACT_OPS },
                "id": { "type": "string" },
                "kind": { "type": "string", "enum": artifact_kinds },
                "title": { "type": "string" },
                "summary": { "type": "string" },
                "body": { "type": "string" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "index": { "type": "integer" },
                "done": { "type": "boolean" },
                "expected_updated_at": { "type": "integer" },
            }),
            &["op"],
        ),
        tool(
            LIST_MOVEMENTS,
            "Lista as passagens de trabalho recentes entre agentes do projeto.",
            json!({}),
            &[],
        ),
        tool(
            POST_CHANNEL_MESSAGE,
            "Escreve uma mensagem num canal do projeto do qual você é membro.",
            json!({
                "channel": { "type": "string" },
                "text": { "type": "string" },
            }),
            &["channel", "text"],
        ),
        tool(
            APP_COMMANDS_LIST,
            "Lista os comandos administrativos do Vokia (papéis, agentes, orquestração, provedores, canais, projetos, configurações) com o schema de cada um.",
            json!({ "domain": { "type": "string", "enum": COMMAND_DOMAINS } }),
            &[],
        ),
        tool(
            APP_COMMAND_CALL,
            "Executa um comando administrativo do Vokia pelo nome, com os argumentos do schema listado por app_commands_list.",
            json!({
                "name": { "type": "string" },
                "arguments": { "type": "object" },
            }),
            &["name"],
        ),
    ]
}

fn decision_kind_names() -> Vec<&'static str> {
    DecisionKind::ALL
        .into_iter()
        .map(DecisionKind::as_str)
        .collect()
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false,
        },
    })
}

// None quando o nome não é uma ferramenta deste módulo.
pub fn call(app: &AppHandle, binding: &Binding, name: &str, arguments: &Value) -> Option<Value> {
    let outcome = match name {
        LIST_BACKLOG => parse(name, arguments).and_then(|args| list_backlog(app, binding, args)),
        READ_CARD => parse(name, arguments).and_then(|args| read_card(app, binding, args)),
        CREATE_CARD => parse(name, arguments).and_then(|args| create_card(app, binding, args)),
        MOVE_CARD => parse(name, arguments).and_then(|args| move_card(app, binding, args)),
        UPDATE_CARD => parse(name, arguments).and_then(|args| update_card(app, binding, args)),
        BACKLOG_ADD_LABEL => parse(name, arguments).and_then(|args| add_backlog_label(app, binding, args)),
        BACKLOG_REMOVE_LABEL => {
            parse(name, arguments).and_then(|args| remove_backlog_label(app, binding, args))
        }
        BACKLOG_SET_RESPONSIBLE => {
            parse(name, arguments).and_then(|args| set_backlog_responsible(app, binding, args))
        }
        BACKLOG_ARCHIVE_ITEM => parse(name, arguments).and_then(|args| archive_backlog_item(app, binding, args)),
        BACKLOG_RESTORE_ITEM => parse(name, arguments).and_then(|args| restore_backlog_item(app, binding, args)),
        BACKLOG_DELETE_ITEM => parse(name, arguments).and_then(|args| delete_backlog_item(app, binding, args)),
        REQUEST_DECISION => parse(name, arguments).and_then(|args| request_decision(binding, args)),
        PROPOSE_REQUEST => parse(name, arguments).and_then(|args| propose_request(binding, args)),
        DELEGATE => parse(name, arguments).and_then(|args| delegate(app, binding, args)),
        GET_VARIABLES => get_variables(app, binding),
        SET_VARIABLE => parse(name, arguments).and_then(|args| set_variable(app, binding, args)),
        GET_ORCHESTRATION => get_orchestration(app),
        LIST_AGENTS => list_agents(app),
        SAVE_ITEM_PLAN => parse(name, arguments).and_then(|args| save_item_plan(app, binding, args)),
        ARTIFACT_SET_CRITERION => {
            parse(name, arguments).and_then(|args| set_artifact_criterion(app, binding, args))
        }
        ARTIFACT => parse(name, arguments).and_then(|args| artifact(app, binding, args)),
        LIST_MOVEMENTS => list_movements(app, binding),
        POST_CHANNEL_MESSAGE => {
            parse(name, arguments).and_then(|args| post_channel_message(app, binding, args))
        }
        APP_COMMANDS_LIST => parse(name, arguments).and_then(app_commands_list),
        _ => return None,
    };
    Some(match outcome {
        Ok(text) => result(&text, false),
        Err(text) => result(&text, true),
    })
}

fn result(text: &str, is_error: bool) -> Value {
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

// Campos desconhecidos (inclusive project_id e agent) são ignorados: o projeto e o agente vêm só do Binding.
fn parse<T: for<'de> Deserialize<'de>>(tool: &str, arguments: &Value) -> Result<T, String> {
    let arguments = if arguments.is_null() {
        json!({})
    } else {
        arguments.clone()
    };
    serde_json::from_value(arguments)
        .map_err(|_| format!("Argumentos inválidos para {tool}: confira o schema da ferramenta."))
}

fn refusal_or_system(error: &DbError) -> String {
    if error.is_card_refusal() {
        error.to_string()
    } else {
        SYSTEM_FAILURE.to_string()
    }
}

fn list_backlog(app: &AppHandle, binding: &Binding, args: ListArgs) -> ToolText {
    let filter = args
        .column
        .as_deref()
        .map(|requested| board_column(requested).map_err(|error| error.to_string()))
        .transpose()?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    let snapshot = db
        .list_backlog(binding.project_id)
        .map_err(|error| refusal_or_system(&error))?;
    let matching: Vec<&BacklogItem> = snapshot
        .items
        .iter()
        .filter(|item| filter.is_none_or(|column| column_of(item) == column))
        .collect();
    if matching.is_empty() {
        return Ok(EMPTY_BACKLOG.to_string());
    }
    let skipped = matching.len().saturating_sub(LIST_LIMIT);
    let mut lines: Vec<String> = matching[skipped..]
        .iter()
        .rev()
        .map(|item| {
            format!(
                "{} | {} | {} | {} | {}",
                item.id,
                item.title,
                column_of(item),
                item.who,
                item.origin
            )
        })
        .collect();
    if skipped > 0 {
        lines.push(format!("(mais {skipped} cards antigos não listados)"));
    }
    Ok(lines.join("\n"))
}

fn read_card(app: &AppHandle, binding: &Binding, args: ReadArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    card_text(db, binding.project_id, &args.id)
}

pub(crate) fn card_text(db: &Db, project_id: i64, id: &str) -> ToolText {
    let card = db
        .read_card(project_id, id)
        .map_err(|error| refusal_or_system(&error))?;
    let recent = card.activity.len().saturating_sub(RECENT_ACTIVITY_COUNT);
    let mut lines = vec![
        format!("Card {}: {}", card.id, card.title),
        format!("Coluna: {}", column_of(&card)),
        format!("Dono: {}", card.who),
        format!("Origem: {}", card.origin),
        format!(
            "Descrição: {}",
            card.description.as_deref().unwrap_or(NO_DESCRIPTION)
        ),
    ];
    lines.extend(progress_lines(db, project_id, &card)?);
    lines.extend(
        card.activity[recent..]
            .iter()
            .map(|activity| format!("- {} {}", activity.at, activity.text)),
    );
    Ok(lines.join("\n"))
}

// Progresso por etapa: a etapa diz qual critério do plano marcar; o plano lista as etapas e os critérios marcados.
fn progress_lines(db: &Db, project_id: i64, card: &BacklogItem) -> Result<Vec<String>, String> {
    let mut lines = Vec::new();
    if let Some(parent) = card.parent.as_deref() {
        lines.push(format!("Plano: {parent}"));
        if let Some(number) = plan_stage_number(parent, &card.id) {
            lines.push(format!(
                "Critério desta etapa: {} c{number}",
                artifact_id(parent)
            ));
        }
    }
    let Some(children) = &card.children else {
        return Ok(lines);
    };
    lines.push(format!(
        "Etapas concluídas: {} de {}",
        children.done, children.total
    ));
    for stage in db
        .plan_stages(project_id, &card.id)
        .map_err(|error| refusal_or_system(&error))?
    {
        lines.push(format!("- {}: {}", stage.id, column_of(&stage)));
    }
    for criterion in db
        .plan_criteria(project_id, &card.id)
        .map_err(|error| refusal_or_system(&error))?
    {
        let mark = if criterion.done { CHECKED_MARK } else { UNCHECKED_MARK };
        lines.push(format!("- {mark} {}: {}", criterion.id, criterion.text));
    }
    Ok(lines)
}

fn ensure_title_length(title: &str) -> Result<(), String> {
    if title.trim().chars().count() > MAX_TITLE_CHARS {
        return Err(TITLE_TOO_LONG.to_string());
    }
    Ok(())
}

fn create_card(app: &AppHandle, binding: &Binding, args: CreateArgs) -> ToolText {
    ensure_title_length(&args.title)?;
    let responsible = if binding.agent == Agent::ember() {
        ember_card_responsible(app, binding.project_id, args.team.as_deref())?
    } else {
        None
    };
    apply(
        app,
        binding,
        CardOperation::Create {
            title: args.title,
            description: args.description,
            responsible,
        },
    )
}

fn leaders_text(leaders: &[TeamLeader], team: &Team) -> String {
    leaders
        .iter()
        .map(|entry| {
            format!(
                "{} (líder {})",
                entry.team_name,
                team.name_of(&entry.leader)
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn leaders_or_none(context: &LeaderContext) -> String {
    let leaders = context.leaders();
    if leaders.is_empty() {
        "nenhum".to_string()
    } else {
        leaders_text(&leaders, &context.team)
    }
}

// Sem nenhum time com líder disponível o Ember cria o card para si, como antes dos times.
fn ember_card_responsible(
    app: &AppHandle,
    project_id: i64,
    requested: Option<&str>,
) -> Result<Option<CardResponsible>, String> {
    let Some(context) = LeaderContext::load(app, project_id)? else {
        return Ok(None);
    };
    let requested = requested.map(str::trim).filter(|team| !team.is_empty());
    let Some(requested) = requested else {
        if context.leaders().is_empty() {
            return Ok(None);
        }
        return Err(format!(
            "Informe o time do card no campo team. Times com líder disponível: {}.",
            leaders_or_none(&context)
        ));
    };
    let Some(orch_team) = find_team(&context.snapshot, requested) else {
        let teams: Vec<String> = context
            .snapshot
            .teams
            .iter()
            .map(|team| format!("{} ({})", team.id, team.name))
            .collect();
        return Err(format!(
            "Não existe o time {requested}. Times: {}.",
            teams.join(", ")
        ));
    };
    let Some(entry) = team_leader(&context.snapshot, orch_team, &context.team, &context.paused)
    else {
        return Err(format!(
            "O time {} não tem líder disponível. Times com líder disponível: {}.",
            orch_team.name,
            leaders_or_none(&context)
        ));
    };
    Ok(Some(CardResponsible {
        leader_name: context.team.name_of(&entry.leader).to_string(),
        team_name: entry.team_name,
        leader: entry.leader,
    }))
}

// Mapa global, elenco e pausas do projeto da troca: tudo o que a regra de líder disponível lê.
struct LeaderContext {
    snapshot: OrchestrationSnapshot,
    team: Team,
    paused: Vec<Agent>,
}

impl LeaderContext {
    // Sem banco ou sem times cadastrados não há líder a exigir.
    fn load(app: &AppHandle, project_id: i64) -> Result<Option<Self>, String> {
        let Some(snapshot) = orchestration_snapshot(app)? else {
            return Ok(None);
        };
        if snapshot.teams.is_empty() {
            return Ok(None);
        }
        let team = app
            .state::<AgentRegistry>()
            .team()
            .map_err(|_| SYSTEM_FAILURE.to_string())?;
        let state = app.state::<DbState>();
        let db = state.require().map_err(|_| SYSTEM_FAILURE.to_string())?;
        let paused = db
            .project_paused_agents(project_id)
            .map_err(|_| SYSTEM_FAILURE.to_string())?;
        Ok(Some(Self {
            snapshot,
            team,
            paused,
        }))
    }

    fn leaders(&self) -> Vec<TeamLeader> {
        team_leaders(&self.snapshot, &self.team, &self.paused)
    }
}

// O Ember só delega a líderes; o card sem responsável de verdade passa ao líder escolhido.
fn ember_delegation_to_leader(
    app: &AppHandle,
    binding: &Binding,
    helper: &Agent,
    item_id: Option<&str>,
) -> Result<(), String> {
    let Some(context) = LeaderContext::load(app, binding.project_id)? else {
        return Ok(());
    };
    let leaders = context.leaders();
    if leaders.is_empty() {
        return Ok(());
    }
    if !leaders.iter().any(|entry| entry.leader == *helper) {
        return Err(format!(
            "{} não é líder de time. Delegue ao líder do time: {}.",
            context.team.name_of(helper),
            leaders_text(&leaders, &context.team)
        ));
    }
    let Some(item) = item_id else {
        return Ok(());
    };
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.set_backlog_responsible(binding.project_id, item, helper, &context.team, Some(&binding.agent))
        .map(|_| ())
        .map_err(|error| refusal_or_system(&error))
}

fn update_card(app: &AppHandle, binding: &Binding, args: UpdateArgs) -> ToolText {
    let has_description = args
        .description
        .as_deref()
        .is_some_and(|description| !description.trim().is_empty());
    if args.title.is_none() && !has_description {
        return Err(UPDATE_WITHOUT_FIELDS.to_string());
    }
    if let Some(title) = &args.title {
        ensure_title_length(title)?;
    }
    apply(
        app,
        binding,
        CardOperation::Update {
            id: args.id,
            title: args.title,
            description: args.description,
        },
    )
}

fn move_card(app: &AppHandle, binding: &Binding, args: MoveArgs) -> ToolText {
    apply(
        app,
        binding,
        CardOperation::Move {
            id: args.id,
            column: args.column,
        },
    )
}

fn get_variables(app: &AppHandle, binding: &Binding) -> ToolText {
    let state = app.state::<AppState>();
    with_cache(&state, binding.project_id, |cache| {
        Ok(variables_listing(cache, &binding.agent))
    })
    .map_err(|_| VARIABLES_FAILURE.to_string())
}

fn set_variable(app: &AppHandle, binding: &Binding, args: SetVariableArgs) -> ToolText {
    if !binding.slot.lock().variables_enabled {
        return Err(NO_VARIABLES_IN_EXCHANGE.to_string());
    }
    let name = args.name.trim().to_string();
    let changes = variable_changes(&name, &args)?;
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|_| VARIABLES_FAILURE.to_string())?;
    let state = app.state::<AppState>();
    let applied = with_cache(&state, binding.project_id, |cache| {
        Ok(cache.apply_changes(&binding.agent, changes, &team))
    })
    .map_err(|_| VARIABLES_FAILURE.to_string())?;
    if !applied.rejected.is_empty() {
        return Err(format!(
            "Variável recusada: {}.",
            applied.rejected.join("; ")
        ));
    }
    let mut slot = binding.slot.lock();
    slot.variables.created_variable |= applied.created_variable;
    slot.variables.changed_value |= applied.changed_value;
    slot.variables.accepted.extend(applied.accepted);
    Ok(format!("Variável {name} registrada."))
}

fn variable_changes(name: &str, args: &SetVariableArgs) -> Result<Vec<Change>, String> {
    let share = args.share.unwrap_or(true);
    match args.op.as_str() {
        OP_SET => Ok(vec![Change::Set {
            name: name.to_string(),
            value: variable_value(&args.value)?,
            share,
        }]),
        OP_APPEND => Ok(vec![Change::Append {
            name: name.to_string(),
            item: item_text(&args.value)?,
            share,
        }]),
        _ => Err(INVALID_OP.to_string()),
    }
}

// Texto numérico vira número, como na linha `$nome = valor`.
fn variable_value(value: &Value) -> Result<VarValue, String> {
    match value {
        Value::Number(number) => number
            .as_i64()
            .map(VarValue::Number)
            .ok_or_else(|| NON_INTEGER_VALUE.to_string()),
        Value::String(text) if !text.trim().is_empty() => Ok(VarValue::from_text(text.trim())),
        Value::String(_) => Err(EMPTY_VALUE.to_string()),
        _ => Err(UNSUPPORTED_VALUE.to_string()),
    }
}

fn item_text(value: &Value) -> Result<String, String> {
    match value {
        Value::Number(number) => Ok(number.to_string()),
        Value::String(text) if !text.trim().is_empty() => Ok(text.trim().to_string()),
        Value::String(_) => Err(EMPTY_VALUE.to_string()),
        _ => Err(UNSUPPORTED_VALUE.to_string()),
    }
}

fn delegate(app: &AppHandle, binding: &Binding, args: DelegateArgs) -> ToolText {
    let request = args.request.trim();
    if request.is_empty() {
        return Err(DELEGATION_EMPTY_REQUEST.to_string());
    }
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|_| SYSTEM_FAILURE.to_string())?;
    let name = args.agent.trim();
    let Some(helper) = team.named(name) else {
        let colleagues: Vec<&str> = team
            .profiles()
            .iter()
            .filter(|profile| profile.id != binding.agent)
            .map(|profile| profile.name.as_str())
            .collect();
        return Err(format!(
            "Não existe agente chamado {name}. Colegas: {}.",
            colleagues.join(", ")
        ));
    };
    if helper == binding.agent {
        return Err(DELEGATION_TO_SELF.to_string());
    }
    let kind = optional_choice(args.kind.as_deref(), REQUEST_KIND_VALUES, KIND_INVALID)?;
    let event = optional_choice(args.event.as_deref(), WHEN_EVENT_VALUES, EVENT_INVALID)?;
    let notify = args.notify.unwrap_or(false);
    let item_id = args
        .item_id
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .or_else(|| binding.slot.lock().item_id.clone());
    if item_id.is_some() && !notify {
        let providers = app.state::<crate::providers::ProviderRegistry>();
        let has_tools = team
            .profile(&helper)
            .is_some_and(|profile| crate::orchestrator::cli_has_tools(&profile.cli, &providers));
        if !has_tools {
            return Err(format!(
                "{} usa uma CLI sem suporte a ferramentas (MCP ou Vokia CLI) e não pode receber delegação de card.",
                team.name_of(&helper)
            ));
        }
    }
    check_item(app, binding.project_id, item_id.as_deref())?;
    let intent = DelegationIntent {
        kind: kind.map(str::to_string),
        event: event.map(str::to_string),
        item_id,
        ping_seconds: args.ping_seconds,
        notify,
    };
    match delegation_room(&binding.slot.lock().delegations, &helper, &intent)? {
        Room::Repeated => return Ok(DELEGATION_DUPLICATE.to_string()),
        Room::Open => {}
    }
    if !notify {
        if binding.agent == Agent::ember() {
            ember_delegation_to_leader(app, binding, &helper, intent.item_id.as_deref())?;
        }
        let snapshot = orchestration_snapshot(app)?;
        check_intent(snapshot.as_ref(), &team, &binding.agent, &helper, kind, event)?;
    }
    let mut slot = binding.slot.lock();
    slot.delegations.push(Delegation {
        agent: helper,
        request: request.to_string(),
        intent,
    });
    Ok(delegation_registered(&slot.delegations, &team))
}

enum Room {
    Open,
    Repeated,
}

// Repetido não ocupa vaga; o teto é só de pedidos de trabalho, para um aviso não tomar a vaga de uma delegação.
fn delegation_room(
    registered: &[Delegation],
    helper: &Agent,
    intent: &DelegationIntent,
) -> Result<Room, String> {
    let repeated = registered.iter().any(|delegation| {
        delegation.agent == *helper
            && delegation.intent.item_id == intent.item_id
            && delegation.intent.kind == intent.kind
    });
    if repeated {
        return Ok(Room::Repeated);
    }
    let work = registered
        .iter()
        .filter(|delegation| !delegation.intent.notify)
        .count();
    if !intent.notify && work >= MAX_DELEGATIONS_PER_REPLY {
        return Err(DELEGATION_LIMIT.to_string());
    }
    Ok(Room::Open)
}

fn delegation_registered(delegations: &[Delegation], team: &Team) -> String {
    let lines: Vec<String> = delegations
        .iter()
        .map(|delegation| {
            let status = if delegation.intent.notify {
                DELEGATION_NOTICE_STATUS
            } else {
                DELEGATION_PENDING_STATUS
            };
            format!(
                "- {} · {} · {} · {status}",
                team.name_of(&delegation.agent),
                delegation.intent.kind.as_deref().unwrap_or(DELEGATION_GENERAL_KIND),
                delegation.intent.item_id.as_deref().unwrap_or(DELEGATION_NO_ITEM),
            )
        })
        .collect();
    format!(
        "{DELEGATION_REGISTERED}
Delegações nesta resposta:
{}
{DELEGATION_CONTINUE}",
        lines.join("
")
    )
}

const KIND_INVALID: &str = "Tipo de pedido inválido.";
const EVENT_INVALID: &str = "Evento inválido.";

fn optional_choice<'a>(
    value: Option<&'a str>,
    allowed: &[&str],
    invalid: &str,
) -> Result<Option<&'a str>, String> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if allowed.contains(&value) {
        Ok(Some(value))
    } else {
        Err(format!("{invalid} Use um destes: {}.", allowed.join(", ")))
    }
}

// Item de outro projeto não existe aqui: a conferência é sempre no backlog do projeto da troca.
fn check_item(app: &AppHandle, project_id: i64, item: Option<&str>) -> Result<(), String> {
    let Some(item) = item.map(str::trim).filter(|item| !item.is_empty()) else {
        return Ok(());
    };
    let state = app.state::<DbState>();
    let db = state.require().map_err(|_| SYSTEM_FAILURE.to_string())?;
    let backlog = db
        .list_backlog(project_id)
        .map_err(|_| SYSTEM_FAILURE.to_string())?;
    if backlog.items.iter().any(|candidate| candidate.id == item) {
        Ok(())
    } else {
        Err(format!(
            "O item {item} não existe no backlog deste projeto."
        ))
    }
}

fn orchestration_snapshot(app: &AppHandle) -> Result<Option<OrchestrationSnapshot>, String> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(None);
    };
    db.orchestration()
        .map(Some)
        .map_err(|_| SYSTEM_FAILURE.to_string())
}

fn request_decision(binding: &Binding, args: DecisionArgs) -> ToolText {
    let kind = DecisionKind::parse(args.kind.trim()).ok_or_else(|| {
        format!(
            "Tipo de decisão inválido. Use um destes: {}.",
            decision_kind_names().join(", ")
        )
    })?;
    let title = args.title.trim();
    if title.is_empty() {
        return Err(DECISION_EMPTY_TITLE.to_string());
    }
    let options: Vec<String> = args
        .options
        .unwrap_or_default()
        .iter()
        .map(|option| option.trim())
        .filter(|option| !option.is_empty())
        .map(str::to_string)
        .collect();
    if kind == DecisionKind::Escolha && options.is_empty() {
        return Err(DECISION_CHOICE_WITHOUT_OPTIONS.to_string());
    }
    let mut slot = binding.slot.lock();
    if slot.decision.is_some() {
        return Err(DECISION_ALREADY_REGISTERED.to_string());
    }
    slot.decision = Some(SlotDecision {
        kind,
        title: title.to_string(),
        detail: args.detail.unwrap_or_default().trim().to_string(),
        options,
    });
    Ok(DECISION_REGISTERED.to_string())
}

fn propose_request(binding: &Binding, args: ProposalArgs) -> ToolText {
    let title = args.title.trim();
    if title.is_empty() {
        return Err(PROPOSAL_EMPTY_TITLE.to_string());
    }
    ensure_title_length(title)?;
    let mut slot = binding.slot.lock();
    if slot.proposal.is_some() {
        return Err(PROPOSAL_ALREADY_REGISTERED.to_string());
    }
    slot.proposal = Some(RequestProposal {
        title: title.to_string(),
        detail: args.detail.unwrap_or_default().trim().to_string(),
    });
    Ok(PROPOSAL_REGISTERED.to_string())
}

fn to_json(value: &impl serde::Serialize) -> ToolText {
    serde_json::to_string(value).map_err(|_| SYSTEM_FAILURE.to_string())
}

fn get_orchestration(app: &AppHandle) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    let registry = app.state::<AgentRegistry>();
    let snapshot = read_orchestration(db, &registry)
        .map_err(|_| SYSTEM_FAILURE.to_string())?;
    to_json(&snapshot)
}

fn list_agents(app: &AppHandle) -> ToolText {
    let listing = app
        .state::<AgentRegistry>()
        .listing(&app.state::<crate::providers::ProviderRegistry>())
        .map_err(|_| SYSTEM_FAILURE.to_string())?;
    to_json(&listing)
}

fn save_item_plan(app: &AppHandle, binding: &Binding, args: SavePlanArgs) -> ToolText {
    let approved = {
        let state = app.state::<DbState>();
        let db = state.require().map_err(|error| refusal_or_system(&error))?;
        match db.save_item_plan(binding.project_id, &args.item_id, &args.plan) {
            Ok(approved) => approved,
            Err(error) => return Err(refusal_or_system(&error)),
        }
    };
    if approved {
        start_approved_plan(app, binding.project_id, &args.item_id)
            .map_err(|error| error.to_string())?;
    }
    let message = format!("Plano do card {} gravado.", args.item_id);
    record_backlog_change(app, binding, vec![message.clone()]);
    Ok(message)
}

fn set_artifact_criterion(app: &AppHandle, binding: &Binding, args: CriterionArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.set_artifact_criterion(binding.project_id, &args.artifact_id, &args.criterion_id, args.done)
        .map_err(|error| refusal_or_system(&error))?;
    let mark = if args.done { CRITERION_DONE } else { CRITERION_UNDONE };
    let message = format!("Critério {} do plano {} {mark}.", args.criterion_id, args.artifact_id);
    record_backlog_change(app, binding, vec![message.clone()]);
    Ok(message)
}

#[derive(Deserialize)]
struct ArtifactArgs {
    op: String,
    id: Option<String>,
    kind: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    body: Option<String>,
    tags: Option<Vec<String>>,
    index: Option<usize>,
    done: Option<bool>,
    expected_updated_at: Option<i64>,
}

fn missing_artifact_field(op: &str, field: &str) -> String {
    format!("A operação '{op}' de artefato exige o campo '{field}'.")
}

fn artifact(app: &AppHandle, binding: &Binding, args: ArtifactArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    let project_id = binding.project_id;
    let op = args.op.as_str();
    let id = || args.id.clone().ok_or_else(|| missing_artifact_field(op, "id"));
    match op {
        ARTIFACT_OP_LIST => to_json(
            &db.list_artifact_summaries(project_id)
                .map_err(|error| refusal_or_system(&error))?,
        ),
        ARTIFACT_OP_READ => to_json(
            &db.get_artifact(project_id, &id()?)
                .map_err(|error| refusal_or_system(&error))?,
        ),
        ARTIFACT_OP_CREATE => {
            let kind = args.kind.clone().ok_or_else(|| missing_artifact_field(op, "kind"))?;
            let title = args.title.clone().ok_or_else(|| missing_artifact_field(op, "title"))?;
            let document = db
                .create_artifact_document(
                    project_id,
                    NewArtifactDocument {
                        kind,
                        title,
                        summary: args.summary,
                        body: args.body,
                        tags: args.tags.unwrap_or_default(),
                        author: Some(binding.agent.as_str().to_string()),
                    },
                )
                .map_err(|error| refusal_or_system(&error))?;
            notify_artifacts_changed(app, project_id);
            to_json(&document)
        }
        ARTIFACT_OP_UPDATE => {
            let document = db
                .update_artifact_document(
                    project_id,
                    &id()?,
                    ArtifactDocumentPatch {
                        title: args.title,
                        summary: args.summary,
                        body: args.body,
                        tags: args.tags,
                        expected_updated_at: args.expected_updated_at,
                    },
                )
                .map_err(|error| refusal_or_system(&error))?;
            notify_artifacts_changed(app, project_id);
            to_json(&document)
        }
        ARTIFACT_OP_CHECK => {
            let index = args.index.ok_or_else(|| missing_artifact_field(op, "index"))?;
            let done = args.done.ok_or_else(|| missing_artifact_field(op, "done"))?;
            let document = db
                .set_artifact_check(project_id, &id()?, index, done)
                .map_err(|error| refusal_or_system(&error))?;
            notify_artifacts_changed(app, project_id);
            to_json(&document)
        }
        other => Err(format!(
            "Operação de artefato desconhecida: {other}. Use {}.",
            ARTIFACT_OPS.join(", ")
        )),
    }
}

fn list_movements(app: &AppHandle, binding: &Binding) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    let movements =
        read_movements(db, binding.project_id).map_err(|_| SYSTEM_FAILURE.to_string())?;
    to_json(&movements)
}

fn app_commands_list(args: CommandsListArgs) -> ToolText {
    to_json(&list_commands_filtered(args.domain.as_deref()))
}

fn post_channel_message(app: &AppHandle, binding: &Binding, args: PostChannelArgs) -> ToolText {
    post_agent_message(
        app,
        binding.project_id,
        &binding.agent,
        &args.channel,
        &args.text,
    )?;
    Ok(format!("Mensagem gravada no canal {}.", args.channel.trim()))
}

fn apply(app: &AppHandle, binding: &Binding, operation: CardOperation) -> ToolText {
    let actor = AgentActor {
        id: binding.agent.clone(),
        name: binding.agent_name.clone(),
        current_card: binding.slot.lock().item_id.clone(),
    };
    let is_update = matches!(operation, CardOperation::Update { .. });
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    let results = db
        .apply_card_operations(binding.project_id, &actor, &[operation])
        .map_err(|error| refusal_or_system(&error))?;
    let mut messages = Vec::new();
    let mut changed = false;
    for card in results {
        match card.outcome {
            CardOutcome::Created => {
                changed = true;
                messages.push(format!("Card {} criado: {}.", card.id, card.title));
            }
            CardOutcome::Updated => {
                changed = true;
                messages.push(format!("Card {} atualizado: {}.", card.id, card.title));
            }
            CardOutcome::Moved => {
                changed = true;
                messages.push(format!("Card {} movido para {}.", card.id, card.column));
            }
            CardOutcome::Unchanged if is_update => {
                messages.push(format!("Card {} já está assim: nada mudou.", card.id));
            }
            CardOutcome::Unchanged => {
                messages.push(format!("Card {} já está em {}.", card.id, card.column));
            }
        }
    }
    let text = messages.join("\n");
    if changed {
        record_backlog_change(app, binding, messages);
    }
    Ok(text)
}

fn add_backlog_label(app: &AppHandle, binding: &Binding, args: LabelArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.add_backlog_label(binding.project_id, &args.item_id, &args.label)
        .map_err(|error| refusal_or_system(&error))?;
    let message = format!("Etiqueta {} adicionada ao card {}.", args.label.trim(), args.item_id);
    record_backlog_change(app, binding, vec![message.clone()]);
    Ok(message)
}

fn remove_backlog_label(app: &AppHandle, binding: &Binding, args: LabelArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.remove_backlog_label(binding.project_id, &args.item_id, &args.label)
        .map_err(|error| refusal_or_system(&error))?;
    let message = format!("Etiqueta {} removida do card {}.", args.label.trim(), args.item_id);
    record_backlog_change(app, binding, vec![message.clone()]);
    Ok(message)
}

fn set_backlog_responsible(app: &AppHandle, binding: &Binding, args: ResponsibleArgs) -> ToolText {
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|_| SYSTEM_FAILURE.to_string())?;
    let name = args.agent.trim();
    let agent = team
        .named(name)
        .ok_or_else(|| DbError::ResponsibleOutsideTeam(name.to_string()).to_string())?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.set_backlog_responsible(binding.project_id, &args.item_id, &agent, &team, Some(&binding.agent))
        .map_err(|error| refusal_or_system(&error))?;
    let message = format!(
        "Responsável do card {} definido: {}.",
        args.item_id,
        team.name_of(&agent)
    );
    record_backlog_change(app, binding, vec![message.clone()]);
    Ok(message)
}

fn archive_backlog_item(app: &AppHandle, binding: &Binding, args: ItemArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.archive_backlog_item(binding.project_id, &args.item_id)
        .map_err(|error| refusal_or_system(&error))?;
    let message = format!("Card {} arquivado.", args.item_id);
    record_backlog_change(app, binding, vec![message.clone()]);
    Ok(message)
}

fn restore_backlog_item(app: &AppHandle, binding: &Binding, args: ItemArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.restore_backlog_item(binding.project_id, &args.item_id)
        .map_err(|error| refusal_or_system(&error))?;
    let message = format!("Card {} restaurado.", args.item_id);
    record_backlog_change(app, binding, vec![message.clone()]);
    Ok(message)
}

fn delete_backlog_item(app: &AppHandle, binding: &Binding, args: ItemArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.delete_backlog_item(binding.project_id, &args.item_id)
        .map_err(|error| refusal_or_system(&error))?;
    let message = format!("Card {} removido.", args.item_id);
    record_backlog_change(app, binding, vec![message.clone()]);
    Ok(message)
}

fn record_backlog_change(app: &AppHandle, binding: &Binding, messages: Vec<String>) {
    {
        let mut slot = binding.slot.lock();
        slot.confirmations.extend(messages);
        slot.changed = true;
    }
    // O banco já gravou; falha ao avisar o front não pode virar erro para o agente, que repetiria a escrita.
    let _ = emit_backlog_changed(app, binding.project_id);
}

fn column_of(item: &BacklogItem) -> &str {
    item.column.as_deref().unwrap_or(DEFAULT_COLUMN)
}
