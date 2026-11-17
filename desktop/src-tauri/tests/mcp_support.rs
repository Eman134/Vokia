use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::{Binding, ExchangeSlot, McpServer, Token};
use crate::events::Agent;

pub const IO_TIMEOUT: Duration = Duration::from_secs(5);

pub struct Reply {
    pub status: u16,
    pub body: String,
}

impl Reply {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|error| panic!("corpo não é JSON ({error}): {:?}", self.body))
    }

    pub fn error_code(&self) -> i64 {
        self.json()["error"]["code"]
            .as_i64()
            .unwrap_or_else(|| panic!("a resposta não tem error.code: {}", self.body))
    }
}

pub struct Request<'a> {
    method: &'a str,
    host: Option<String>,
    token: Option<&'a str>,
    authorization: Option<String>,
    origin: Option<&'a str>,
    pub body: Vec<u8>,
}

impl<'a> Request<'a> {
    pub fn post(port: u16, token: &'a Token, body: Value) -> Request<'a> {
        Request {
            method: "POST",
            host: Some(format!("127.0.0.1:{port}")),
            token: Some(token.as_str()),
            authorization: None,
            origin: None,
            body: body.to_string().into_bytes(),
        }
    }

    pub fn raw_authorization(mut self, value: &str) -> Request<'a> {
        self.token = None;
        self.authorization = Some(value.to_string());
        self
    }

    pub fn without_authorization(mut self) -> Request<'a> {
        self.token = None;
        self
    }

    pub fn with_host(mut self, host: &str) -> Request<'a> {
        self.host = Some(host.to_string());
        self
    }

    pub fn with_origin(mut self, origin: &'a str) -> Request<'a> {
        self.origin = Some(origin);
        self
    }

    pub fn with_method(mut self, method: &'a str) -> Request<'a> {
        self.method = method;
        self
    }

    pub fn with_body(mut self, body: Vec<u8>) -> Request<'a> {
        self.body = body;
        self
    }

    pub fn render(&self) -> Vec<u8> {
        let mut head = format!("{} /mcp HTTP/1.1\r\n", self.method);
        if let Some(host) = &self.host {
            head.push_str(&format!("Host: {host}\r\n"));
        }
        if let Some(token) = self.token {
            head.push_str(&format!("Authorization: Bearer {token}\r\n"));
        }
        if let Some(value) = &self.authorization {
            head.push_str(&format!("Authorization: {value}\r\n"));
        }
        if let Some(origin) = self.origin {
            head.push_str(&format!("Origin: {origin}\r\n"));
        }
        head.push_str("Content-Type: application/json\r\n");
        head.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        head.push_str("Connection: close\r\n\r\n");
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

pub fn parse_reply(raw: &[u8]) -> Reply {
    let text = String::from_utf8_lossy(raw).into_owned();
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or_else(|| panic!("resposta HTTP sem status: {text:?}"));
    let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
    let body = if head
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        dechunk(body)
    } else {
        body.to_string()
    };
    Reply { status, body }
}

pub fn dechunk(body: &str) -> String {
    let mut rest = body;
    let mut out = String::new();
    while let Some((size_line, after)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size_line.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        out.push_str(&after[..size]);
        rest = &after[size + 2..];
    }
    out
}

pub async fn send(port: u16, request: &Request<'_>) -> Reply {
    let mut stream = tokio::time::timeout(IO_TIMEOUT, TcpStream::connect(("127.0.0.1", port)))
        .await
        .expect("conectar demorou")
        .expect("o servidor deveria aceitar conexão em 127.0.0.1");
    // O servidor pode responder 413 e fechar antes de consumir o corpo inteiro: o erro de escrita não importa.
    let _ = stream.write_all(&request.render()).await;
    let mut raw = Vec::new();
    let _ = tokio::time::timeout(IO_TIMEOUT, stream.read_to_end(&mut raw)).await;
    parse_reply(&raw)
}

pub fn port_of(server: &McpServer) -> u16 {
    server
        .url()
        .rsplit(':')
        .next()
        .and_then(|tail| tail.strip_suffix("/mcp"))
        .and_then(|port| port.parse().ok())
        .unwrap_or_else(|| panic!("url sem porta: {}", server.url()))
}

pub fn binding(agent: &str, name: &str) -> Binding {
    Binding {
        project_id: 1,
        agent: Agent::from_stored(agent).expect("id de agente válido no teste"),
        agent_name: name.to_string(),
        slot: ExchangeSlot::default(),
    }
}

pub fn rpc(id: i64, method: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method })
}

pub fn run<T>(scenario: impl std::future::Future<Output = T>) -> T {
    tauri::async_runtime::block_on(scenario)
}

// App Tauri real (runtime Wry, sem janelas) só para o servidor MCP ter AppHandle, banco e eventos.
pub fn build_app(data_dir: &std::path::Path) -> tauri::App {
    use tauri::Manager;
    let mut context = tauri::generate_context!();
    context.config_mut().app.windows.clear();
    let app = tauri::Builder::default()
        .any_thread()
        .build(context)
        .expect("o app de teste deveria abrir");
    app.manage(crate::db::DbState::open(data_dir));
    app
}
