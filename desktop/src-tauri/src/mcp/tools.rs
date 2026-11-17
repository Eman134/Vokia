use serde::Deserialize;
use serde_json::{json, Value};
use tauri::{AppHandle, Manager};

use crate::db::{
    board_column, AgentActor, BacklogItem, CardOperation, CardOutcome, DbError, DbState,
    AGENT_FORBIDDEN_COLUMN, BOARD_COLUMNS,
};
use crate::orchestrator::emit_backlog_changed;

use super::registry::Binding;

const LIST_BACKLOG: &str = "list_backlog";
const READ_CARD: &str = "read_card";
const CREATE_CARD: &str = "create_card";
const MOVE_CARD: &str = "move_card";

const MAX_TITLE_CHARS: usize = 200;
const LIST_LIMIT: usize = 50;
const RECENT_ACTIVITY_COUNT: usize = 5;
const DEFAULT_COLUMN: &str = "Backlog";

const TITLE_TOO_LONG: &str = "O título do card passa de 200 caracteres.";
const SYSTEM_FAILURE: &str = "Não foi possível acessar o backlog agora. Tente de novo mais tarde.";
const EMPTY_BACKLOG: &str = "Nenhum card.";
const NO_DESCRIPTION: &str = "sem descrição";

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

pub fn definitions() -> Vec<Value> {
    let movable: Vec<&str> = BOARD_COLUMNS
        .iter()
        .copied()
        .filter(|column| *column != AGENT_FORBIDDEN_COLUMN)
        .collect();
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
    ]
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
