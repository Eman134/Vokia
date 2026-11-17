use serde::Deserialize;
use serde_json::{json, Value};

use tauri::AppHandle;

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

pub fn handle(
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
        "tools/list" => success(id, json!({ "tools": tools.definitions() })),
        "tools/call" => call_tool(id, request.params.as_ref(), binding, app),
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

fn call_tool(
    id: Value,
    params: Option<&Value>,
    binding: &Binding,
    app: Option<&AppHandle>,
) -> Reply {
    let name = params
        .and_then(|params| params.get("name"))
        .and_then(Value::as_str);
    let Some(name) = name else {
        return failure(id, INVALID_PARAMS, "falta o nome da ferramenta");
    };
    let arguments = params
        .and_then(|params| params.get("arguments"))
        .unwrap_or(&Value::Null);
    match app.and_then(|app| tools::call(app, binding, name, arguments)) {
        Some(result) => success(id, result),
        None => failure(
            id,
            INVALID_PARAMS,
            &format!("ferramenta desconhecida: {name}"),
        ),
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
