use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::agents::AgentRegistry;
use crate::db::{
    board_column, AgentActor, BacklogItem, CardOperation, CardOutcome, DbError, DbState,
    AGENT_FORBIDDEN_COLUMN, BOARD_COLUMNS,
};
use crate::events::DecisionKind;
use crate::orchestrator::{emit_backlog_changed, variables_listing, with_cache};
use crate::state::AppState;
use crate::task::{Change, VarValue};

use super::registry::Binding;
use super::slot::SlotDecision;

const LIST_BACKLOG: &str = "list_backlog";
const READ_CARD: &str = "read_card";
const CREATE_CARD: &str = "create_card";
const MOVE_CARD: &str = "move_card";
const REQUEST_DECISION: &str = "request_decision";
const DELEGATE: &str = "delegate";
const GET_VARIABLES: &str = "get_variables";
const SET_VARIABLE: &str = "set_variable";
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
const DECISION_CHOICE_WITHOUT_OPTIONS: &str = "Uma decisão do tipo escolha precisa de opções.";

// Falha de negócio volta ao agente com a frase do banco; falha de sistema não expõe detalhe.
type ToolText = Result<String, String>;

#[derive(Deserialize)]
struct ListArgs {
    column: Option<String>,
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
}

#[derive(Deserialize)]
struct DecisionArgs {
    kind: String,
    title: String,
    detail: Option<String>,
    options: Option<Vec<String>>,
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
            DELEGATE,
            "Delega um pedido a outro agente do time e não espera a resposta: depois de chamar, termine a resposta.",
            json!({
                "agent": { "type": "string" },
                "request": { "type": "string" },
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
            "Grava uma variável da tarefa: op set troca o valor; op append acrescenta um item a uma lista; share compartilha com o time.",
            json!({
                "name": { "type": "string" },
                "op": { "type": "string", "enum": [OP_SET, OP_APPEND] },
                "value": { "type": ["string", "integer"] },
                "share": { "type": "boolean" },
            }),
            &["name", "op", "value"],
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
        REQUEST_DECISION => parse(name, arguments).and_then(|args| request_decision(binding, args)),
        DELEGATE => parse(name, arguments).and_then(|args| delegate(app, binding, args)),
        GET_VARIABLES => get_variables(app, binding),
        SET_VARIABLE => parse(name, arguments).and_then(|args| set_variable(app, binding, args)),
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

fn create_card(app: &AppHandle, binding: &Binding, args: CreateArgs) -> ToolText {
    if args.title.trim().chars().count() > MAX_TITLE_CHARS {
        return Err(TITLE_TOO_LONG.to_string());
    }
    apply(
        app,
        binding,
        CardOperation::Create {
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
        let updates = changes.into_iter().map(Ok).collect();
        Ok(cache.apply_reply(&binding.agent, updates, &team))
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
    let share = args.share.unwrap_or(false);
    match args.op.as_str() {
        OP_SET => Ok(vec![Change::Set {
            name: name.to_string(),
            value: variable_value(&args.value)?,
            share,
        }]),
        OP_APPEND => {
            let mut changes = vec![Change::Append {
                name: name.to_string(),
                item: item_text(&args.value)?,
            }];
            if share {
                changes.push(Change::Share {
                    name: name.to_string(),
                });
            }
            Ok(changes)
        }
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
    let mut slot = binding.slot.lock();
    if slot.delegation.is_some() {
        return Err(DELEGATION_ALREADY_REGISTERED.to_string());
    }
    slot.delegation = Some((helper, request.to_string()));
    Ok(DELEGATION_REGISTERED.to_string())
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

fn apply(app: &AppHandle, binding: &Binding, operation: CardOperation) -> ToolText {
    let actor = AgentActor {
        id: binding.agent.clone(),
        name: binding.agent_name.clone(),
        current_card: binding.slot.lock().item_id.clone(),
    };
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
            CardOutcome::Moved => {
                changed = true;
                messages.push(format!("Card {} movido para {}.", card.id, card.column));
            }
            CardOutcome::Unchanged => {
                messages.push(format!("Card {} já está em {}.", card.id, card.column));
            }
        }
    }
    if changed {
        {
            let mut slot = binding.slot.lock();
            slot.confirmations.extend(messages.iter().cloned());
            slot.changed = true;
        }
        // O card já está gravado; falha ao avisar o front não pode virar erro para o agente, que repetiria a criação.
        let _ = emit_backlog_changed(app, binding.project_id);
    }
    Ok(messages.join("\n"))
}

fn column_of(item: &BacklogItem) -> &str {
    item.column.as_deref().unwrap_or(DEFAULT_COLUMN)
}
