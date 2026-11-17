use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::extract::State;
use axum::http::header::{AUTHORIZATION, HOST, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use tokio::net::TcpListener;

use super::protocol::{handle, Reply};
use super::registry::{Binding, RandomnessUnavailable, Sessions, Token, ToolRegistry};
use super::McpError;

const MCP_PATH: &str = "/mcp";
const BEARER_PREFIX: &str = "Bearer ";
const MAX_BODY_BYTES: usize = 64 * 1024;
const BIND_TIMEOUT: Duration = Duration::from_secs(5);

struct Shared {
    port: u16,
    sessions: Sessions,
    tools: ToolRegistry,
}

pub struct McpServer {
    shared: Arc<Shared>,
}

impl McpServer {
    pub async fn start() -> Result<McpServer, McpError> {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        let listener = tokio::time::timeout(BIND_TIMEOUT, TcpListener::bind(address))
            .await
            .map_err(|_| McpError::BindTimeout)?
            .map_err(McpError::Bind)?;
        let port = listener.local_addr().map_err(McpError::Bind)?.port();
        let shared = Arc::new(Shared {
            port,
            sessions: Sessions::default(),
            tools: ToolRegistry::default(),
        });
        let router = Router::new()
            .route(MCP_PATH, post(post_mcp))
            .with_state(shared.clone());
        tauri::async_runtime::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(McpServer { shared })
    }

    pub fn register(&self, binding: Binding) -> Result<Token, RandomnessUnavailable> {
        self.shared.sessions.register(binding)
    }

    pub fn revoke(&self, token: &Token) {
        self.shared.sessions.revoke(token);
    }

    pub fn url(&self) -> String {
        format!("http://{}{MCP_PATH}", self.shared.host())
    }
}

impl Shared {
    fn host(&self) -> String {
        format!("{}:{}", Ipv4Addr::LOCALHOST, self.port)
    }

    // Host fixo protege de DNS rebinding; Origin só existe em requisição de navegador.
    fn authorize(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        let host_matches = headers
            .get(HOST)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|host| host == self.host());
        if !host_matches || headers.contains_key(ORIGIN) {
            return Err(StatusCode::FORBIDDEN);
        }
        let token = headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix(BEARER_PREFIX));
        match token.and_then(|token| self.sessions.find(token)) {
            Some(_) => Ok(()),
            None => Err(StatusCode::UNAUTHORIZED),
        }
    }
}

async fn post_mcp(State(shared): State<Arc<Shared>>, headers: HeaderMap, body: Body) -> Response {
    if let Err(status) = shared.authorize(&headers) {
        return status.into_response();
    }
    let Ok(bytes) = to_bytes(body, MAX_BODY_BYTES).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    match handle(&bytes, &shared.tools) {
        Reply::Json(value) => Json(value).into_response(),
        Reply::NoContent => StatusCode::NO_CONTENT.into_response(),
    }
}
