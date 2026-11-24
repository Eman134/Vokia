use serde::Deserialize;
use serde_json::{json, Value};

use tauri::AppHandle;

use crate::commands::dispatcher;

use super::registry::{Binding, ToolRegistry};
use super::tools;
use super::SERVER_NAME;

const JSONRPC_VERSION: &str = "2.0";
const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

pub enum Reply {
    Json(Value),
    NoContent,
}

#[derive(Deserialize)]
struct Request {
    method: String,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    params: Option<Value>,
}

pub async fn handle(
    body: &[u8],
    tools: &ToolRegistry,
    binding: &Binding,
    app: Option<&AppHandle>,
) -> Reply {
    let Ok(message) = serde_json::from_slice::<Value>(body) else {
        return failure(Value::Null, PARSE_ERROR, "JSON inválido");
    };
    let Ok(request) = serde_json::from_value::<Request>(message) else {
        return failure(Value::Null, INVALID_REQUEST, "requisição JSON-RPC inválida");
    };
    let id = request.id.unwrap_or(Value::Null);
    match request.method.as_str() {
        "initialize" => success(id, initialize_result(request.params.as_ref())),
        "notifications/initialized" => Reply::NoContent,
        "ping" => success(id, json!({})),
        "tools/list" => success(id, json!({ "tools": tools.definitions_for(binding.scope) })),
        "tools/call" => call_tool(id, request.params.as_ref(), binding, app).await,
        "commands/list" => {
            let domain = request
                .params
                .as_ref()
                .and_then(|params| params.get("domain"))
                .and_then(Value::as_str);
            success(
                id,
                json!({ "commands": dispatcher::list_commands_filtered(domain) }),
            )
        }
        "commands/call" => call_app_command(id, request.params.as_ref(), binding, app).await,
        other => failure(
            id,
            METHOD_NOT_FOUND,
            &format!("método desconhecido: {other}"),
        ),
    }
}

fn initialize_result(params: Option<&Value>) -> Value {
    let requested = params
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str);
    let version = requested
        .filter(|version| SUPPORTED_PROTOCOL_VERSIONS.contains(version))
        .unwrap_or(LATEST_PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
    })
}

fn call_target(params: Option<&Value>) -> Option<(&str, &Value)> {
    let name = params?.get("name")?.as_str()?;
    let arguments = params
        .and_then(|params| params.get("arguments"))
        .unwrap_or(&Value::Null);
    Some((name, arguments))
}

async fn call_tool(
    id: Value,
    params: Option<&Value>,
    binding: &Binding,
    app: Option<&AppHandle>,
) -> Reply {
    let Some((name, arguments)) = call_target(params) else {
        return failure(id, INVALID_PARAMS, "falta o nome da ferramenta");
    };
    let Some(app) = app else {
        return failure(id, INVALID_PARAMS, &format!("ferramenta desconhecida: {name}"));
    };
    if name == tools::APP_COMMAND_CALL {
        return call_meta_app_command(id, arguments, binding, app).await;
    }
    if let Some(result) = tools::call(app, binding, name, arguments) {
        return success(id, result);
    }
    match dispatcher::dispatch(app, Some(&binding.agent), name, arguments.clone()).await {
        Some(outcome) => success(id, command_reply(outcome)),
        None => failure(
            id,
            INVALID_PARAMS,
            &format!("ferramenta desconhecida: {name}"),
        ),
    }
}

async fn call_meta_app_command(
    id: Value,
    arguments: &Value,
    binding: &Binding,
    app: &AppHandle,
) -> Reply {
    let Some(target_name) = arguments.get("name").and_then(Value::as_str) else {
        return failure(
            id,
            INVALID_PARAMS,
            "falta o nome do comando em app_command_call",
        );
    };
    let target_args = arguments
        .get("arguments")
        .filter(|value| !value.is_null())
        .cloned()
        .unwrap_or_else(|| json!({}));
    match dispatcher::dispatch(app, Some(&binding.agent), target_name, target_args).await {
        Some(outcome) => success(id, command_reply(outcome)),
        None => success(
            id,
            command_reply(Err(format!("comando desconhecido: {target_name}"))),
        ),
    }
}

async fn call_app_command(
    id: Value,
    params: Option<&Value>,
    binding: &Binding,
    app: Option<&AppHandle>,
) -> Reply {
    let Some((name, arguments)) = call_target(params) else {
        return failure(id, INVALID_PARAMS, "falta o nome do comando");
    };
    let Some(app) = app else {
        return failure(id, INVALID_PARAMS, &format!("comando desconhecido: {name}"));
    };
    match dispatcher::dispatch(app, Some(&binding.agent), name, arguments.clone()).await {
        Some(outcome) => success(id, command_reply(outcome)),
        None => failure(id, INVALID_PARAMS, &format!("comando desconhecido: {name}")),
    }
}

fn command_reply(outcome: Result<Value, String>) -> Value {
    match outcome {
        Ok(value) => {
            let text = match value {
                Value::Null => "OK".to_string(),
                Value::String(text) => text,
                other => serde_json::to_string_pretty(&other).unwrap_or_else(|_| other.to_string()),
            };
            json!({ "content": [{ "type": "text", "text": text }], "isError": false })
        }
        Err(message) => {
            json!({ "content": [{ "type": "text", "text": message }], "isError": true })
        }
    }
}

fn success(id: Value, result: Value) -> Reply {
    Reply::Json(json!({ "jsonrpc": JSONRPC_VERSION, "id": id, "result": result }))
}

fn failure(id: Value, code: i64, message: &str) -> Reply {
    Reply::Json(json!({
        "jsonrpc": JSONRPC_VERSION,
        "id": id,
        "error": { "code": code, "message": message },
    }))
}
