use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::extract::State;
use axum::http::header::{AUTHORIZATION, HOST, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use tauri::AppHandle;
use tokio::net::TcpListener;

use super::protocol::{handle, Reply};
use super::registry::{random_hex, Binding, RandomnessUnavailable, Sessions, Token, ToolRegistry};
use super::slot::ExchangeSlot;
use super::tools::{self, ToolScope};
use super::{McpError, SERVER_NAME};

const MCP_PATH: &str = "/mcp";
const TOOL_NAMESPACE: &str = "mcp";
const CONFIG_NAME_BYTES: usize = 16;
const BEARER_PREFIX: &str = "Bearer ";
const MAX_BODY_BYTES: usize = 64 * 1024;
const BIND_TIMEOUT: Duration = Duration::from_secs(5);

struct Shared {
    port: u16,
    sessions: Sessions,
    tools: ToolRegistry,
    app: OnceLock<AppHandle>,
}

#[derive(Clone)]
pub struct McpServer {
    shared: Arc<Shared>,
}

pub struct McpConfigFile {
    path: PathBuf,
    allowed_tools: Vec<String>,
}

impl McpConfigFile {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn allowed_tools(&self) -> &[String] {
        &self.allowed_tools
    }
}

// Vive tanto quanto a sessão da CLI: ao cair, revoga o token e apaga o arquivo que o contém.
pub struct McpSession {
    shared: Arc<Shared>,
    token: Token,
    url: String,
    config: Option<McpConfigFile>,
    slot: ExchangeSlot,
}

impl McpSession {
    pub fn config(&self) -> Option<&McpConfigFile> {
        self.config.as_ref()
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn token(&self) -> &str {
        self.token.as_str()
    }

    pub fn slot(&self) -> &ExchangeSlot {
        &self.slot
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        self.shared.sessions.revoke(&self.token);
        // O arquivo pode nunca ter sido gravado (sessão headless ou falha no meio); ausente é o resultado desejado.
        if let Some(config) = &self.config {
            let _ = fs::remove_file(&config.path);
        }
    }
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
            app: OnceLock::new(),
        });
        let router = Router::new()
            .route(MCP_PATH, post(post_mcp))
            .with_state(shared.clone());
        tauri::async_runtime::spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        Ok(McpServer { shared })
    }

    // As ferramentas precisam do app (banco e eventos); sem isto o servidor responde com a lista vazia.
    pub fn attach(&self, app: AppHandle) {
        if self.shared.app.set(app).is_ok() {
            self.shared.tools.install(tools::definitions());
        }
    }

    #[cfg_attr(not(test), allow(dead_code))] // usado só nos testes
    pub fn register(&self, binding: Binding) -> Result<Token, RandomnessUnavailable> {
        self.shared.sessions.register(binding)
    }

    #[cfg_attr(not(test), allow(dead_code))] // usado só nos testes
    pub fn revoke(&self, token: &Token) {
        self.shared.sessions.revoke(token);
    }

    pub fn url(&self) -> String {
        format!("http://{}{MCP_PATH}", self.shared.host())
    }

    pub fn tool_names(&self, scope: ToolScope) -> Vec<String> {
        self.shared
            .tools
            .names(scope)
            .map(|name| format!("{TOOL_NAMESPACE}__{SERVER_NAME}__{name}"))
            .collect()
    }

    pub fn open_session(&self, dir: &Path, binding: Binding) -> Result<McpSession, McpError> {
        let file_name = random_hex(CONFIG_NAME_BYTES).map_err(McpError::Randomness)?;
        let slot = binding.slot.clone();
        let allowed_tools = self.tool_names(binding.scope);
        let token = self
            .shared
            .sessions
            .register(binding)
            .map_err(McpError::Randomness)?;
        let url = self.url();
        let config = McpConfigFile {
            path: dir.join(format!("{file_name}.json")),
            allowed_tools,
        };
        let session = McpSession {
            shared: self.shared.clone(),
            url: url.clone(),
            config: Some(config),
            token,
            slot,
        };
        session.write_config(&url)?;
        Ok(session)
    }

    pub fn open_headless(&self, binding: Binding) -> Result<McpSession, McpError> {
        let slot = binding.slot.clone();
        let token = self
            .shared
            .sessions
            .register(binding)
            .map_err(McpError::Randomness)?;
        Ok(McpSession {
            shared: self.shared.clone(),
            token,
            url: self.url(),
            config: None,
            slot,
        })
    }
}

impl McpSession {
    fn write_config(&self, url: &str) -> Result<(), McpError> {
        let Some(config_file) = &self.config else {
            return Ok(());
        };
        let config = json!({
            "mcpServers": {
                SERVER_NAME: {
                    "type": "http",
                    "url": url,
                    "headers": { "Authorization": format!("{BEARER_PREFIX}{}", self.token.as_str()) },
                }
            }
        });
        if let Some(dir) = config_file.path.parent() {
            fs::create_dir_all(dir).map_err(McpError::ConfigFile)?;
        }
        fs::write(&config_file.path, config.to_string()).map_err(McpError::ConfigFile)
    }
}

impl Shared {
    fn host(&self) -> String {
        format!("{}:{}", Ipv4Addr::LOCALHOST, self.port)
    }

    // Host fixo protege de DNS rebinding; Origin só existe em requisição de navegador.
    fn authorize(&self, headers: &HeaderMap) -> Result<Binding, StatusCode> {
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
        token
            .and_then(|token| self.sessions.find(token))
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

async fn post_mcp(State(shared): State<Arc<Shared>>, headers: HeaderMap, body: Body) -> Response {
    let binding = match shared.authorize(&headers) {
        Ok(binding) => binding,
        Err(status) => return status.into_response(),
    };
    let Ok(bytes) = to_bytes(body, MAX_BODY_BYTES).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };
    match handle(&bytes, &shared.tools, &binding, shared.app.get()).await {
        Reply::Json(value) => Json(value).into_response(),
        Reply::NoContent => StatusCode::NO_CONTENT.into_response(),
    }
}
