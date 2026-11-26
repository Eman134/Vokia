use std::collections::{BTreeMap, HashMap};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, PoisonError};
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::extract::{Path as AxumPath, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, HOST, ORIGIN};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::process::ChildStdin;
use tokio::sync::{oneshot, Mutex as AsyncMutex};

use crate::claude::{base_command, kill_process_tree};

use super::registry::{constant_time_eq, random_hex, Token};
use super::{McpError, SERVER_NAME};

const BEARER_PREFIX: &str = "Bearer ";
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const BIND_TIMEOUT: Duration = Duration::from_secs(5);
const STDIO_CALL_TIMEOUT: Duration = Duration::from_secs(60);
const TOKEN_BYTES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StdioMcpServerConfig {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct VokiaMcpBridge {
    pub url: String,
    pub token: String,
}

#[derive(Clone, Debug)]
pub struct RelayEndpoint {
    #[allow(dead_code)]
    pub vokia_url: String,
    pub token: String,
    pub server_urls: BTreeMap<String, String>,
}

struct RelayState {
    port: u16,
    token: Token,
    vokia_bridge: StdMutex<Option<VokiaMcpBridge>>,
    stdio_configs: StdMutex<HashMap<String, StdioMcpServerConfig>>,
    stdio_clients: AsyncMutex<HashMap<String, Arc<StdioMcpProcess>>>,
    http_client: reqwest::Client,
    closed: AtomicBool,
}

impl RelayState {
    fn host(&self) -> String {
        format!("{}:{}", Ipv4Addr::LOCALHOST, self.port)
    }

    fn authorize(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        let host_matches = headers
            .get(HOST)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|host| host == self.host());
        if !host_matches || headers.contains_key(ORIGIN) {
            return Err(StatusCode::FORBIDDEN);
        }
        let presented = headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix(BEARER_PREFIX));
        match presented {
            Some(tok) if constant_time_eq(tok.as_bytes(), self.token.as_str().as_bytes()) => Ok(()),
            _ => Err(StatusCode::UNAUTHORIZED),
        }
    }

    fn update_vokia_bridge(&self, bridge: Option<VokiaMcpBridge>) {
        if let Some(next) = bridge {
            let mut guard = self.vokia_bridge.lock().unwrap_or_else(PoisonError::into_inner);
            *guard = Some(next);
        }
    }

    fn merge_stdio_configs(&self, configs: Vec<StdioMcpServerConfig>) {
        let mut guard = self.stdio_configs.lock().unwrap_or_else(PoisonError::into_inner);
        for cfg in configs {
            if !cfg.name.trim().is_empty() && cfg.name != SERVER_NAME {
                guard.entry(cfg.name.clone()).or_insert(cfg);
            }
        }
    }

    async fn shutdown(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        let clients: Vec<Arc<StdioMcpProcess>> = {
            let mut guard = self.stdio_clients.lock().await;
            guard.drain().map(|(_, v)| v).collect()
        };
        for client in clients {
            client.terminate();
        }
    }
}

struct StdioMcpProcess {
    stdin: AsyncMutex<ChildStdin>,
    pending: Arc<StdMutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>>,
    pid: AtomicU32,
}

impl StdioMcpProcess {
    fn spawn(cfg: &StdioMcpServerConfig) -> Result<Arc<Self>, String> {
        let mut cmd = base_command(Path::new(&cfg.command));
        cmd.kill_on_drop(true);
        cmd.args(&cfg.args);
        for (k, v) in &cfg.env {
            cmd.env(k, v);
        }
        if let Some(cwd) = &cfg.cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(unix)]
        {
            cmd.process_group(0);
        }

        let mut child = cmd
            .spawn()
            .map_err(|err| format!("falha ao iniciar servidor MCP stdio '{}': {err}", cfg.name))?;
        let pid = child.id().unwrap_or(0);
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| format!("stdin indisponível para servidor MCP '{}'", cfg.name))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("stdout indisponível para servidor MCP '{}'", cfg.name))?;

        let pending: Arc<StdMutex<HashMap<String, oneshot::Sender<Result<Value, String>>>>> =
            Arc::new(StdMutex::new(HashMap::new()));
        let pending_reader = Arc::clone(&pending);

        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
                    continue;
                };
                if let Some(id_key) = value.get("id").and_then(json_id_key) {
                    let tx = {
                        let mut map = pending_reader.lock().unwrap_or_else(PoisonError::into_inner);
                        map.remove(&id_key)
                    };
                    if let Some(tx) = tx {
                        let _ = tx.send(Ok(value));
                    }
                }
            }
            let _ = child.wait().await;
            let mut map = pending_reader.lock().unwrap_or_else(PoisonError::into_inner);
            for (_, tx) in map.drain() {
                let _ = tx.send(Err("servidor MCP stdio encerrou inesperadamente".to_string()));
            }
        });

        Ok(Arc::new(Self {
            stdin: AsyncMutex::new(stdin),
            pending,
            pid: AtomicU32::new(pid),
        }))
    }

    async fn send(&self, payload: &Value) -> Result<Option<Value>, String> {
        let id_key = payload.get("id").and_then(json_id_key);
        let rx = if let Some(ref key) = id_key {
            let (tx, rx) = oneshot::channel();
            let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            map.insert(key.clone(), tx);
            Some(rx)
        } else {
            None
        };

        let mut line = serde_json::to_string(payload).map_err(|e| e.to_string())?;
        line.push('\n');
        {
            let mut stdin = self.stdin.lock().await;
            if let Err(err) = stdin.write_all(line.as_bytes()).await {
                if let Some(key) = id_key {
                    let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
                    map.remove(&key);
                }
                return Err(format!("falha ao escrever no servidor MCP stdio: {err}"));
            }
            let _ = stdin.flush().await;
        }

        let Some(rx) = rx else {
            return Ok(None);
        };
        match tokio::time::timeout(STDIO_CALL_TIMEOUT, rx).await {
            Ok(Ok(Ok(value))) => Ok(Some(value)),
            Ok(Ok(Err(err))) => Err(err),
            Ok(Err(_)) => Err("canal de resposta MCP stdio fechado".to_string()),
            Err(_) => {
                if let Some(key) = id_key {
                    let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
                    map.remove(&key);
                }
                Err("tempo limite ao aguardar resposta do servidor MCP stdio".to_string())
            }
        }
    }

    fn terminate(&self) {
        let pid = self.pid.swap(0, Ordering::SeqCst);
        if pid != 0 {
            #[cfg(not(windows))]
            {
                let pgid = format!("-{pid}");
                let _ = std::process::Command::new("kill")
                    .args(["-TERM", &pgid])
                    .output();
            }
            kill_process_tree(pid);
        }
    }
}

fn json_id_key(id: &Value) -> Option<String> {
    match id {
        Value::String(s) => Some(format!("s:{s}")),
        Value::Number(n) => Some(format!("n:{n}")),
        _ => None,
    }
}

struct RelayEntry {
    state: Arc<RelayState>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    session_count: usize,
}

#[derive(Default)]
pub struct McpRelayManager {
    entries: StdMutex<HashMap<PathBuf, RelayEntry>>,
}

pub struct RelaySessionGuard {
    manager: Arc<McpRelayManager>,
    dir: PathBuf,
    state: Arc<RelayState>,
}

impl RelaySessionGuard {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn port(&self) -> u16 {
        self.state.port
    }

    pub fn token(&self) -> &str {
        self.state.token.as_str()
    }

    #[allow(dead_code)]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn vokia_url(&self) -> String {
        self.url_for(SERVER_NAME)
    }

    pub fn url_for(&self, server_name: &str) -> String {
        format!("http://{}/mcp/{server_name}", self.state.host())
    }

    pub fn endpoint(&self) -> RelayEndpoint {
        let configs = self
            .state
            .stdio_configs
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let mut server_urls = BTreeMap::new();
        server_urls.insert(SERVER_NAME.to_string(), self.vokia_url());
        for name in configs.keys() {
            server_urls.insert(name.clone(), self.url_for(name));
        }
        RelayEndpoint {
            vokia_url: self.vokia_url(),
            token: self.token().to_string(),
            server_urls,
        }
    }
}

impl Drop for RelaySessionGuard {
    fn drop(&mut self) {
        self.manager.release_directory(&self.dir);
    }
}

impl McpRelayManager {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub async fn acquire_for_directory(
        self: &Arc<Self>,
        dir: &Path,
        vokia_bridge: Option<VokiaMcpBridge>,
        stdio_servers: Vec<StdioMcpServerConfig>,
    ) -> Result<RelaySessionGuard, McpError> {
        let key = normalize_dir_key(dir);
        {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = entries.get_mut(&key) {
                if !entry.state.closed.load(Ordering::SeqCst) {
                    entry.session_count += 1;
                    entry.state.update_vokia_bridge(vokia_bridge);
                    entry.state.merge_stdio_configs(stdio_servers);
                    return Ok(RelaySessionGuard {
                        manager: Arc::clone(self),
                        dir: key,
                        state: Arc::clone(&entry.state),
                    });
                }
            }
        }

        let (state, shutdown_tx) = start_relay_server(vokia_bridge, stdio_servers).await?;
        let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(existing) = entries.get_mut(&key) {
            if !existing.state.closed.load(Ordering::SeqCst) {
                existing.session_count += 1;
                let _ = shutdown_tx.send(());
                let state_clone = Arc::clone(&existing.state);
                drop(entries);
                return Ok(RelaySessionGuard {
                    manager: Arc::clone(self),
                    dir: key,
                    state: state_clone,
                });
            }
        }
        entries.insert(
            key.clone(),
            RelayEntry {
                state: Arc::clone(&state),
                shutdown_tx: Some(shutdown_tx),
                session_count: 1,
            },
        );
        Ok(RelaySessionGuard {
            manager: Arc::clone(self),
            dir: key,
            state,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn active_directories(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn session_count_for(&self, dir: &Path) -> usize {
        let key = normalize_dir_key(dir);
        self.entries
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&key)
            .map_or(0, |entry| entry.session_count)
    }

    pub fn shutdown_all(&self) {
        let drained: Vec<RelayEntry> = {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            entries.drain().map(|(_, v)| v).collect()
        };
        for mut entry in drained {
            if let Some(tx) = entry.shutdown_tx.take() {
                let _ = tx.send(());
            }
            let state = Arc::clone(&entry.state);
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    state.shutdown().await;
                });
            } else {
                state.closed.store(true, Ordering::SeqCst);
            }
        }
    }

    fn release_directory(&self, key: &Path) {
        let removed = {
            let mut entries = self.entries.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = entries.get_mut(key) {
                entry.session_count = entry.session_count.saturating_sub(1);
                if entry.session_count == 0 {
                    entries.remove(key)
                } else {
                    None
                }
            } else {
                None
            }
        };
        if let Some(mut entry) = removed {
            if let Some(tx) = entry.shutdown_tx.take() {
                let _ = tx.send(());
            }
            let state = Arc::clone(&entry.state);
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                handle.spawn(async move {
                    state.shutdown().await;
                });
            } else {
                state.closed.store(true, Ordering::SeqCst);
            }
        }
    }
}

fn normalize_dir_key(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

async fn start_relay_server(
    vokia_bridge: Option<VokiaMcpBridge>,
    stdio_servers: Vec<StdioMcpServerConfig>,
) -> Result<(Arc<RelayState>, oneshot::Sender<()>), McpError> {
    let token_raw = random_hex(TOKEN_BYTES).map_err(McpError::Randomness)?;
    let token = Token::from_raw(token_raw);
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    let listener = tokio::time::timeout(BIND_TIMEOUT, TcpListener::bind(address))
        .await
        .map_err(|_| McpError::BindTimeout)?
        .map_err(McpError::Bind)?;
    let port = listener.local_addr().map_err(McpError::Bind)?.port();

    let mut config_map = HashMap::new();
    for cfg in stdio_servers {
        if !cfg.name.trim().is_empty() && cfg.name != SERVER_NAME {
            config_map.insert(cfg.name.clone(), cfg);
        }
    }

    let _ = rustls::crypto::ring::default_provider().install_default();
    let state = Arc::new(RelayState {
        port,
        token,
        vokia_bridge: StdMutex::new(vokia_bridge),
        stdio_configs: StdMutex::new(config_map),
        stdio_clients: AsyncMutex::new(HashMap::new()),
        http_client: reqwest::Client::builder()
            .timeout(STDIO_CALL_TIMEOUT)
            .build()
            .unwrap_or_default(),
        closed: AtomicBool::new(false),
    });

    let router = Router::new()
        .route(
            "/mcp/{server}",
            post(handle_post_mcp)
                .get(handle_get_mcp)
                .delete(handle_delete_mcp),
        )
        .with_state(Arc::clone(&state));

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let state_for_shutdown = Arc::clone(&state);
    tokio::spawn(async move {
        let server = axum::serve(listener, router).with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        });
        let _ = server.await;
        state_for_shutdown.shutdown().await;
    });

    Ok((state, shutdown_tx))
}

async fn handle_get_mcp(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(status) = state.authorize(&headers) {
        return status.into_response();
    }
    StatusCode::METHOD_NOT_ALLOWED.into_response()
}

async fn handle_delete_mcp(
    State(state): State<Arc<RelayState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(status) = state.authorize(&headers) {
        return status.into_response();
    }
    StatusCode::NO_CONTENT.into_response()
}

async fn handle_post_mcp(
    State(state): State<Arc<RelayState>>,
    AxumPath(server): AxumPath<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    if let Err(status) = state.authorize(&headers) {
        return status.into_response();
    }
    let Ok(bytes) = to_bytes(body, MAX_BODY_BYTES).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };

    if server == SERVER_NAME {
        let bridge = {
            let guard = state.vokia_bridge.lock().unwrap_or_else(PoisonError::into_inner);
            guard.clone()
        };
        let Some(bridge) = bridge else {
            return StatusCode::NOT_FOUND.into_response();
        };
        return forward_to_vokia_bridge(&state.http_client, &bridge, &bytes).await;
    }

    let cfg = {
        let guard = state.stdio_configs.lock().unwrap_or_else(PoisonError::into_inner);
        guard.get(&server).cloned()
    };
    let Some(cfg) = cfg else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let Ok(payload) = serde_json::from_slice::<Value>(&bytes) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": { "code": -32700, "message": "JSON inválido" }
            })),
        )
            .into_response();
    };

    let client = {
        let mut clients = state.stdio_clients.lock().await;
        if let Some(existing) = clients.get(&server) {
            Arc::clone(existing)
        } else {
            match StdioMcpProcess::spawn(&cfg) {
                Ok(created) => {
                    clients.insert(server.clone(), Arc::clone(&created));
                    created
                }
                Err(err) => {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(serde_json::json!({
                            "jsonrpc": "2.0",
                            "id": payload.get("id").cloned().unwrap_or(Value::Null),
                            "error": { "code": -32000, "message": err }
                        })),
                    )
                        .into_response();
                }
            }
        }
    };

    match client.send(&payload).await {
        Ok(Some(reply)) => Json(reply).into_response(),
        Ok(None) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": payload.get("id").cloned().unwrap_or(Value::Null),
                "error": { "code": -32000, "message": err }
            })),
        )
            .into_response(),
    }
}

async fn forward_to_vokia_bridge(
    client: &reqwest::Client,
    bridge: &VokiaMcpBridge,
    body: &[u8],
) -> Response {
    let resp = match client
        .post(&bridge.url)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("{BEARER_PREFIX}{}", bridge.token))
        .body(body.to_vec())
        .send()
        .await
    {
        Ok(r) => r,
        Err(err) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32000, "message": format!("falha na ponte MCP do Vokia: {err}") }
                })),
            )
                .into_response();
        }
    };

    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    if status == StatusCode::NO_CONTENT {
        return StatusCode::NO_CONTENT.into_response();
    }
    let bytes = resp.bytes().await.unwrap_or_default();
    if bytes.is_empty() {
        return status.into_response();
    }
    (
        status,
        [(CONTENT_TYPE, "application/json")],
        bytes,
    )
        .into_response()
}
