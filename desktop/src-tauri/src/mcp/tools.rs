use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::agents::AgentRegistry;
use crate::commands::channels::post_agent_message;
use crate::commands::orchestration::{read_movements, read_orchestration};
use crate::db::{
    board_column, AgentActor, BacklogItem, CardOperation, CardOutcome, DbError, DbState,
    ItemPlanInput, OrchestrationSnapshot, AGENT_FORBIDDEN_COLUMN, BACKLOG_LABEL_MAX_CHARS,
    BOARD_COLUMNS, REQUEST_KIND_VALUES, WHEN_EVENT_VALUES,
};
use crate::events::{DecisionKind, RequestProposal};
use crate::orchestration::policy::{check_intent, parallel_pair};
use crate::orchestrator::{emit_backlog_changed, variables_listing, with_cache};
use crate::state::AppState;
use crate::task::{Change, VarValue};

use super::registry::Binding;
use super::slot::{DelegationIntent, SlotDecision};

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
const LIST_MOVEMENTS: &str = "list_movements";
const POST_CHANNEL_MESSAGE: &str = "post_channel_message";
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

const DELEGATION_REGISTERED: &str = "Delegação registrada. Termine a resposta agora.";
const DELEGATION_ALREADY_REGISTERED: &str =
    "Já há uma delegação registrada nesta resposta. Termine a resposta agora.";
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

pub fn definitions() -> Vec<Value> {
    let movable: Vec<&str> = BOARD_COLUMNS
        .iter()
        .copied()
        .filter(|column| *column != AGENT_FORBIDDEN_COLUMN)
        .collect();
    let decision_kinds = decision_kind_names();
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
            "Cria um card no backlog; o título não pode repetir o de outro card.",
            json!({
                "title": { "type": "string", "maxLength": MAX_TITLE_CHARS },
                "description": { "type": "string" },
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
            "Define o agente responsável por um card do backlog; o card só aceita um novo responsável se ainda não tiver um, ou se o responsável atual for o próprio agente indicado.",
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
            "Delega um pedido a outro agente do time e não espera a resposta: depois de chamar, termine a resposta.",
            json!({
                "agent": { "type": "string" },
                "request": { "type": "string" },
                "kind": { "type": "string", "enum": REQUEST_KIND_VALUES },
                "event": { "type": "string", "enum": WHEN_EVENT_VALUES },
                "item_id": { "type": "string" },
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
            "Lê os times, as camadas e as regras de passagem do projeto, com a posição de cada agente.",
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
            "Grava o plano de um card do backlog. state vazio limpa o plano; proposto exige proposal; vinculado exige plan com id, name e meta.",
            json!({
                "item_id": { "type": "string" },
                "state": { "type": "string" },
                "proposal": { "type": "string" },
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
        GET_ORCHESTRATION => get_orchestration(app, binding),
        LIST_AGENTS => list_agents(app),
        SAVE_ITEM_PLAN => parse(name, arguments).and_then(|args| save_item_plan(app, binding, args)),
        LIST_MOVEMENTS => list_movements(app, binding),
        POST_CHANNEL_MESSAGE => {
            parse(name, arguments).and_then(|args| post_channel_message(app, binding, args))
        }
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
    let card = db
        .read_card(binding.project_id, &args.id)
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
    lines.extend(
        card.activity[recent..]
            .iter()
            .map(|activity| format!("- {} {}", activity.at, activity.text)),
    );
    Ok(lines.join("\n"))
}

fn ensure_title_length(title: &str) -> Result<(), String> {
    if title.trim().chars().count() > MAX_TITLE_CHARS {
        return Err(TITLE_TOO_LONG.to_string());
    }
    Ok(())
}

fn create_card(app: &AppHandle, binding: &Binding, args: CreateArgs) -> ToolText {
    ensure_title_length(&args.title)?;
    apply(
        app,
        binding,
        CardOperation::Create {
            title: args.title,
            description: args.description,
        },
    )
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
    let item_id = args
        .item_id
        .as_deref()
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string);
    check_item(app, binding.project_id, item_id.as_deref())?;
    let snapshot = orchestration_snapshot(app, binding.project_id)?;
    check_intent(
        snapshot.as_ref(),
        &team,
        &binding.agent,
        &helper,
        kind,
        event,
    )?;
    let mut slot = binding.slot.lock();
    if parallel_pair(snapshot.as_ref(), &binding.agent, &helper) {
        slot.parallel_delegations
            .push((helper, request.to_string()));
        return Ok(DELEGATION_REGISTERED.to_string());
    }
    if slot.delegation.is_some() {
        return Err(DELEGATION_ALREADY_REGISTERED.to_string());
    }
    slot.delegation = Some((helper, request.to_string()));
    slot.delegation_intent = Some(DelegationIntent {
        kind: kind.map(str::to_string),
        event: event.map(str::to_string),
        item_id,
    });
    Ok(DELEGATION_REGISTERED.to_string())
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

fn orchestration_snapshot(
    app: &AppHandle,
    project_id: i64,
) -> Result<Option<OrchestrationSnapshot>, String> {
    let state = app.state::<DbState>();
    let Some(db) = state.db() else {
        return Ok(None);
    };
    db.orchestration(project_id)
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

fn get_orchestration(app: &AppHandle, binding: &Binding) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    let registry = app.state::<AgentRegistry>();
    let snapshot = read_orchestration(db, &registry, binding.project_id)
        .map_err(|_| SYSTEM_FAILURE.to_string())?;
    to_json(&snapshot)
}

fn list_agents(app: &AppHandle) -> ToolText {
    let listing = app
        .state::<AgentRegistry>()
        .listing()
        .map_err(|_| SYSTEM_FAILURE.to_string())?;
    to_json(&listing)
}

fn save_item_plan(app: &AppHandle, binding: &Binding, args: SavePlanArgs) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    db.save_item_plan(binding.project_id, &args.item_id, &args.plan)
        .map_err(|error| refusal_or_system(&error))?;
    Ok(format!("Plano do card {} gravado.", args.item_id))
}

fn list_movements(app: &AppHandle, binding: &Binding) -> ToolText {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| refusal_or_system(&error))?;
    let movements =
        read_movements(db, binding.project_id).map_err(|_| SYSTEM_FAILURE.to_string())?;
    to_json(&movements)
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
    db.set_backlog_responsible(binding.project_id, &args.item_id, &agent, &team)
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
