use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, PoisonError};
use std::time::{Duration, Instant};

use serde_json::{json, Value as Json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};
use tokio::time::timeout;

use crate::agents::{is_agent_or_replica, Agent};
use crate::claude::{
    base_command_for_provider, classify_provider_error, kill_process_tree, timeout_for, Access, ClaudeError,
    ConsoleKind, ConsoleLine, PermissionRequest, PermissionVerdict, Reply, ToolActivity,
    ToolCategory, ToolStatus, TurnActivity, TurnUsage,
};
use crate::mcp::McpSession;
use crate::orchestrator::session_pool::NO_PID;
use crate::orchestrator::step_labels::label_for_codex_item;
use crate::orchestrator::turn_plan::{plan_from_codex_notification, TurnPlan};
use crate::permissions::{PermissionGate, PermissionLevel};
use crate::providers::ProviderConfig;
use crate::session_key::SessionKey;
use crate::usage::model::ProviderPlanUsage;
use crate::usage::now_millis as usage_now_millis;
use crate::usage::readers::codex::parse_rate_limits_rpc_message;

use super::ConsoleSink;

pub const THREAD_IDLE_UNSUBSCRIBE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const SERVER_IDLE_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub const INITIAL_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(60);
pub const RPC_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

const FEATURE_TOOL_SUGGEST: &str = "features.tool_suggest";
const FEATURE_RECOMMENDED_PLUGINS: &str = "features.recommended_plugins";
const MCP_TOKEN_ENV: &str = "VOKIA_MCP_TOKEN";
const STDERR_TAIL_LINES: usize = 16;
const COMMAND_OUTPUT_TAIL_CHARS: usize = 600;
#[cfg(not(windows))]
const UNIX_SIGTERM_GRACE_MS: u64 = 500;

const METHOD_INITIALIZE: &str = "initialize";
const METHOD_INITIALIZED: &str = "initialized";
const METHOD_THREAD_START: &str = "thread/start";
const METHOD_THREAD_RESUME: &str = "thread/resume";
const METHOD_THREAD_UNSUBSCRIBE: &str = "thread/unsubscribe";
const METHOD_THREAD_ARCHIVE: &str = "thread/archive";
const METHOD_TURN_START: &str = "turn/start";
const METHOD_TURN_STEER: &str = "turn/steer";
const METHOD_TURN_INTERRUPT: &str = "turn/interrupt";
const METHOD_MODEL_LIST: &str = "model/list";
const METHOD_RATE_LIMITS_READ: &str = "account/rateLimits/read";
const METHOD_RATE_LIMITS_UPDATED: &str = "account/rateLimits/updated";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerLifecycleState {
    Stopped,
    Starting,
    Ready,
    Idle,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreadLifecycleState {
    New,
    Active,
    Idle,
    Unloaded,
    Resumed,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ServerKey {
    pub program: PathBuf,
    pub user_args: Vec<String>,
}

impl ServerKey {
    pub fn new(program: &Path, user_args: &[String]) -> Self {
        Self {
            program: program.to_path_buf(),
            user_args: user_args.to_vec(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CodexIdleConfig {
    pub thread_idle_timeout: Duration,
    pub server_idle_timeout: Duration,
    pub handshake_timeout: Duration,
    pub rpc_timeout: Duration,
}

impl Default for CodexIdleConfig {
    fn default() -> Self {
        Self {
            thread_idle_timeout: THREAD_IDLE_UNSUBSCRIBE_TIMEOUT,
            server_idle_timeout: SERVER_IDLE_SHUTDOWN_TIMEOUT,
            handshake_timeout: INITIAL_HANDSHAKE_TIMEOUT,
            rpc_timeout: RPC_REQUEST_TIMEOUT,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReapReport {
    pub unsubscribed_threads: Vec<String>,
    pub closed_servers: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.code != 0 {
            write!(f, "{} (código RPC {})", self.message, self.code)
        } else {
            f.write_str(&self.message)
        }
    }
}

impl std::error::Error for RpcError {}

// ==========================================
// 1. Flags de economia de tokens e comando
// ==========================================

pub fn has_user_config_override(user_args: &[String], key: &str) -> bool {
    let mut i = 0;
    while i < user_args.len() {
        let arg = user_args[i].as_str();
        let candidate = if arg == "-c" || arg == "--config" {
            i += 1;
            user_args.get(i).map(String::as_str)
        } else if let Some(rest) = arg.strip_prefix("--config=") {
            Some(rest)
        } else if let Some(rest) = arg.strip_prefix("-c") {
            if rest.is_empty() {
                None
            } else {
                Some(rest)
            }
        } else {
            None
        };

        if let Some(raw) = candidate {
            let lhs = raw
                .split_once('=')
                .map(|(k, _)| k)
                .unwrap_or(raw)
                .trim()
                .trim_matches(['"', '\'']);
            if lhs.eq_ignore_ascii_case(key) {
                return true;
            }
        }
        i += 1;
    }
    false
}

pub fn token_saving_config_args(user_args: &[String]) -> Vec<String> {
    let mut extra = Vec::new();
    if !has_user_config_override(user_args, FEATURE_TOOL_SUGGEST) {
        extra.push("-c".to_string());
        extra.push(format!("{FEATURE_TOOL_SUGGEST}=false"));
    }
    if !has_user_config_override(user_args, FEATURE_RECOMMENDED_PLUGINS) {
        extra.push("-c".to_string());
        extra.push(format!("{FEATURE_RECOMMENDED_PLUGINS}=false"));
    }
    extra
}

pub fn build_app_server_command(
    program: &Path,
    user_args: &[String],
    mcp: Option<&McpSession>,
) -> Command {
    let mut command = base_command_for_provider(program, "codex");
    command.kill_on_drop(true);
    command.args(user_args);
    command.args(token_saving_config_args(user_args));
    if let Some(session) = mcp {
        command.env(MCP_TOKEN_ENV, session.token());
        command.args([
            "-c",
            &format!("mcp_servers.vokia.url=\"{}\"", session.url()),
        ]);
        command.args([
            "-c",
            &format!("mcp_servers.vokia.bearer_token_env_var=\"{MCP_TOKEN_ENV}\""),
        ]);
    }
    command.arg("app-server");
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(unix)]
    {
        // Coloca o codex app-server em seu próprio process group para encerrar toda a árvore com SIGTERM -> SIGKILL.
        command.process_group(0);
    }

    command
}

/// Encerra a árvore de processos do `codex app-server`:
/// - Windows: `taskkill /PID <pid> /T /F` (sem janela de console).
/// - Unix: `SIGTERM` no process group (`-<pid>`), breve espera e `SIGKILL` no process group.
pub fn terminate_process_tree(pid: u32) {
    if pid == NO_PID {
        return;
    }
    #[cfg(windows)]
    {
        kill_process_tree(pid);
    }
    #[cfg(not(windows))]
    {
        let pgid = format!("-{pid}");
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pgid])
            .output();
        std::thread::sleep(Duration::from_millis(UNIX_SIGTERM_GRACE_MS));
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &pgid])
            .output();
        kill_process_tree(pid);
    }
}

// ==========================================
// 2. Resiliência e diagnósticos em PT-BR
// ==========================================

pub fn is_code_mode_model(model: &str) -> bool {
    let lower = model.trim().to_ascii_lowercase();
    lower.contains("gpt-6") || lower.contains("gpt6") || lower.contains("code-mode")
}

/// Verifica se uma instalação nativa do Codex possui o binário auxiliar `codex-code-mode-host`
/// quando o modelo selecionado exige code-mode (ex.: família GPT-6).
pub fn detect_missing_code_mode_host(program_dir: &Path, path_has_host: bool, model: &str) -> Option<String> {
    if !is_code_mode_model(model) || path_has_host {
        return None;
    }
    let host_bin = if cfg!(windows) {
        "codex-code-mode-host.exe"
    } else {
        "codex-code-mode-host"
    };
    let sibling = program_dir.join(host_bin);
    if sibling.is_file() {
        return None;
    }
    Some(missing_code_mode_host_message(model))
}

fn enrich_code_mode_error(err: ClaudeError, program: &Path, model: &str) -> ClaudeError {
    if !is_code_mode_model(model) {
        return err;
    }
    let lower = err.to_string().to_ascii_lowercase();
    let looks_like_host_failure = lower.contains("code-mode")
        || lower.contains("code_mode")
        || lower.contains("enoent")
        || lower.contains("not found");
    if !looks_like_host_failure {
        return err;
    }
    let path_has_host = which::which("codex-code-mode-host").is_ok();
    if let Some(dir) = program.parent() {
        if let Some(msg) = detect_missing_code_mode_host(dir, path_has_host, model) {
            return ClaudeError::Reported(msg);
        }
    }
    err
}

fn missing_code_mode_host_message(model: &str) -> String {
    let target = if model.trim().is_empty() {
        "o modelo selecionado"
    } else {
        model.trim()
    };
    format!(
        "Instalação incompleta do Codex CLI: o binário auxiliar 'codex-code-mode-host' exigido por {target} não foi encontrado. Reinstale ou atualize o Codex CLI completo (ou escolha outro modelo / altere o protocolo do provedor para 'codex-jsonl')."
    )
}

fn incompatible_binary_message(detail: &str) -> String {
    let suffix = detail.trim();
    if suffix.is_empty() {
        "A versão instalada do Codex CLI é antiga ou incompatível com 'codex app-server'. Atualize o Codex CLI (ex.: npm i -g @openai/codex) ou altere o protocolo do provedor 'codex' para 'codex-jsonl' nas configurações de provedores.".to_string()
    } else {
        format!(
            "A versão instalada do Codex CLI é antiga ou incompatível com 'codex app-server' ({suffix}). Atualize o Codex CLI (ex.: npm i -g @openai/codex) ou altere o protocolo do provedor 'codex' para 'codex-jsonl' nas configurações de provedores."
        )
    }
}

fn state_db_backfill_message(detail: &str) -> String {
    format!(
        "O Codex excedeu o tempo limite ao sincronizar o banco local de estado ({detail}). Aguarde a migração inicial terminar e tente novamente; se persistir, execute 'codex' uma vez no terminal para concluir o backfill ou altere temporariamente o protocolo do provedor para 'codex-jsonl'."
    )
}

pub fn classify_app_server_error(
    message: &str,
    model: &str,
    provider: Option<&ProviderConfig>,
) -> ClaudeError {
    let lower = message.to_ascii_lowercase();

    if let Some(provider) = provider {
        if let Some(classified) = classify_provider_error(
            &provider.id,
            model,
            message,
            &provider.rate_limit_patterns,
            &provider.model_unavailable_patterns,
        ) {
            return classified;
        }
    }

    if lower.contains("state db backfill")
        || (lower.contains("timed out") && lower.contains("backfill"))
    {
        return ClaudeError::Reported(state_db_backfill_message(message.trim()));
    }

    if lower.contains("codex-code-mode-host")
        || lower.contains("code-mode-host")
        || lower.contains("code_mode_host")
        || (lower.contains("code mode") && lower.contains("host"))
    {
        return ClaudeError::Reported(missing_code_mode_host_message(model));
    }

    if lower.contains("unrecognized subcommand")
        || lower.contains("unknown subcommand")
        || (lower.contains("unexpected argument") && lower.contains("app-server"))
        || lower.contains("method not found")
        || lower.contains("-32601")
    {
        return ClaudeError::Reported(incompatible_binary_message(message));
    }

    ClaudeError::Reported(message.to_string())
}

fn is_thread_not_found_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("thread not found")
        || lower.contains("unknown thread")
        || lower.contains("invalid thread")
        || lower.contains("no such thread")
        || lower.contains("session not found")
}

fn is_no_active_turn_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("no active turn")
        || lower.contains("turn not active")
        || lower.contains("turn already completed")
        || lower.contains("not running")
        || lower.contains("expectedturnid")
        || lower.contains("expected_turn_id")
}

// ==========================================
// 3. Cliente JSON-RPC multiplexado sobre stdio
// ==========================================

#[derive(Debug)]
pub enum ServerEvent {
    Notification(Json),
    ServerRequest {
        id: Json,
        method: String,
        params: Json,
    },
    Closed(String),
}

struct PendingRequests {
    map: HashMap<u64, oneshot::Sender<Result<Json, RpcError>>>,
}

impl Default for PendingRequests {
    fn default() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

struct TurnListeners {
    by_thread: HashMap<String, mpsc::UnboundedSender<ServerEvent>>,
}

impl Default for TurnListeners {
    fn default() -> Self {
        Self {
            by_thread: HashMap::new(),
        }
    }
}

pub type UsagePushCallback = Arc<dyn Fn(ProviderPlanUsage) + Send + Sync>;

pub struct CodexRpcClient {
    writer_tx: mpsc::UnboundedSender<String>,
    next_id: AtomicU64,
    pending: Arc<StdMutex<PendingRequests>>,
    listeners: Arc<StdMutex<TurnListeners>>,
    alive: Arc<AtomicBool>,
    pid: Option<u32>,
    child: Arc<StdMutex<Option<Child>>>,
    stderr_tail: Arc<StdMutex<VecDeque<String>>>,
    global_usage_push: Arc<StdMutex<Option<UsagePushCallback>>>,
}

impl CodexRpcClient {
    pub fn from_io<R, W>(
        reader: R,
        mut writer: W,
        child: Option<Child>,
        pid: Option<u32>,
        stderr_tail: Arc<StdMutex<VecDeque<String>>>,
    ) -> Arc<Self>
    where
        R: AsyncBufRead + Unpin + Send + 'static,
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (writer_tx, mut writer_rx) = mpsc::unbounded_channel::<String>();
        let pending = Arc::new(StdMutex::new(PendingRequests::default()));
        let listeners = Arc::new(StdMutex::new(TurnListeners::default()));
        let alive = Arc::new(AtomicBool::new(true));
        let global_usage_push: Arc<StdMutex<Option<UsagePushCallback>>> =
            Arc::new(StdMutex::new(None));

        let alive_writer = Arc::clone(&alive);
        tokio::spawn(async move {
            while let Some(line) = writer_rx.recv().await {
                if writer.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if writer.flush().await.is_err() {
                    break;
                }
            }
            alive_writer.store(false, Ordering::SeqCst);
        });

        let pending_reader = Arc::clone(&pending);
        let listeners_reader = Arc::clone(&listeners);
        let alive_reader = Arc::clone(&alive);
        let stderr_reader = Arc::clone(&stderr_tail);
        let usage_push_reader = Arc::clone(&global_usage_push);

        tokio::spawn(async move {
            let mut lines = reader.lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let Ok(value) = serde_json::from_str::<Json>(trimmed) else {
                            continue;
                        };
                        dispatch_incoming_message(
                            value,
                            &pending_reader,
                            &listeners_reader,
                            &usage_push_reader,
                        );
                    }
                    Ok(None) | Err(_) => break,
                }
            }
            alive_reader.store(false, Ordering::SeqCst);
            let reason = format_stderr_reason(&stderr_reader);
            fail_all_pending(&pending_reader, &reason);
            notify_all_listeners_closed(&listeners_reader, &reason);
        });

        Arc::new(Self {
            writer_tx,
            next_id: AtomicU64::new(1),
            pending,
            listeners,
            alive,
            pid,
            child: Arc::new(StdMutex::new(child)),
            stderr_tail,
            global_usage_push,
        })
    }

    pub fn spawn(
        program: &Path,
        user_args: &[String],
        mcp: Option<&McpSession>,
    ) -> Result<Arc<Self>, ClaudeError> {
        let mut command = build_app_server_command(program, user_args, mcp);
        let mut child = command.spawn().map_err(ClaudeError::Spawn)?;
        let pid = child.id();
        let stdin = child.stdin.take().ok_or_else(|| {
            if let Some(pid) = pid {
                terminate_process_tree(pid);
            }
            ClaudeError::SessionClosed("codex app-server abriu sem stdin disponível.".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            if let Some(pid) = pid {
                terminate_process_tree(pid);
            }
            ClaudeError::SessionClosed("codex app-server abriu sem stdout disponível.".to_string())
        })?;
        let stderr_tail = Arc::new(StdMutex::new(VecDeque::with_capacity(STDERR_TAIL_LINES)));
        if let Some(stderr) = child.stderr.take() {
            let tail_ref = Arc::clone(&stderr_tail);
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let mut guard = tail_ref.lock().unwrap_or_else(PoisonError::into_inner);
                    if guard.len() >= STDERR_TAIL_LINES {
                        guard.pop_front();
                    }
                    guard.push_back(trimmed.to_string());
                }
            });
        }
        Ok(Self::from_io(
            BufReader::new(stdout),
            stdin,
            Some(child),
            pid,
            stderr_tail,
        ))
    }

    pub fn set_global_usage_callback(&self, cb: UsagePushCallback) {
        let mut guard = self
            .global_usage_push
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        *guard = Some(cb);
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    pub fn stderr_snapshot(&self) -> String {
        format_stderr_reason(&self.stderr_tail)
    }

    pub fn register_thread_listener(
        &self,
        thread_id: &str,
    ) -> mpsc::UnboundedReceiver<ServerEvent> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut guard = self
            .listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        guard.by_thread.insert(thread_id.to_string(), tx);
        rx
    }

    pub fn unregister_thread_listener(&self, thread_id: &str) {
        let mut guard = self
            .listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        guard.by_thread.remove(thread_id);
    }

    pub async fn request(
        &self,
        method: &str,
        params: Json,
        limit: Duration,
    ) -> Result<Json, RpcError> {
        if !self.is_alive() {
            return Err(RpcError {
                code: 0,
                message: self.stderr_snapshot(),
            });
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        {
            let mut guard = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            guard.map.insert(id, tx);
        }
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&payload).map_err(|err| RpcError {
            code: 0,
            message: format!("Falha ao serializar requisição JSON-RPC ({method}): {err}"),
        })?;
        line.push('\n');
        if self.writer_tx.send(line).is_err() {
            let mut guard = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            guard.map.remove(&id);
            return Err(RpcError {
                code: 0,
                message: self.stderr_snapshot(),
            });
        }
        match timeout(limit, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(RpcError {
                code: 0,
                message: self.stderr_snapshot(),
            }),
            Err(_) => {
                let mut guard = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
                guard.map.remove(&id);
                let stderr = self.stderr_snapshot();
                let detail = if stderr.to_ascii_lowercase().contains("backfill") {
                    stderr
                } else {
                    format!(
                        "tempo limite ({}s) aguardando resposta de '{method}'",
                        limit.as_secs()
                    )
                };
                Err(RpcError {
                    code: 0,
                    message: detail,
                })
            }
        }
    }

    pub fn notify(&self, method: &str, params: Json) -> Result<(), RpcError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let mut line = serde_json::to_string(&payload).map_err(|err| RpcError {
            code: 0,
            message: format!("Falha ao serializar notificação JSON-RPC ({method}): {err}"),
        })?;
        line.push('\n');
        self.writer_tx.send(line).map_err(|_| RpcError {
            code: 0,
            message: self.stderr_snapshot(),
        })
    }

    pub fn respond(&self, id: Json, result: Json) -> Result<(), RpcError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });
        let mut line = serde_json::to_string(&payload).map_err(|err| RpcError {
            code: 0,
            message: format!("Falha ao serializar resposta JSON-RPC: {err}"),
        })?;
        line.push('\n');
        self.writer_tx.send(line).map_err(|_| RpcError {
            code: 0,
            message: self.stderr_snapshot(),
        })
    }

    pub async fn initialize(&self, handshake_timeout: Duration) -> Result<(), ClaudeError> {
        let params = json!({
            "clientInfo": {
                "name": "vokia",
                "version": env!("CARGO_PKG_VERSION"),
            },
            "capabilities": {
                "experimentalApi": true
            }
        });
        self.request(METHOD_INITIALIZE, params, handshake_timeout)
            .await
            .map_err(|err| classify_app_server_error(&err.to_string(), "", None))?;
        self.notify(METHOD_INITIALIZED, json!({}))
            .map_err(|err| ClaudeError::SessionClosed(err.to_string()))?;
        Ok(())
    }

    pub async fn thread_start(
        &self,
        params: &ThreadParams<'_>,
        rpc_timeout: Duration,
    ) -> Result<String, ClaudeError> {
        let body = build_thread_start_params(params);
        let result = self
            .request(METHOD_THREAD_START, body, rpc_timeout)
            .await
            .map_err(|err| classify_app_server_error(&err.to_string(), params.model, params.provider))?;
        extract_thread_id(&result).ok_or_else(|| {
            ClaudeError::Reported("codex app-server respondeu thread/start sem threadId.".to_string())
        })
    }

    pub async fn thread_resume(
        &self,
        thread_id: &str,
        params: &ThreadParams<'_>,
        rpc_timeout: Duration,
    ) -> Result<String, ClaudeError> {
        let body = build_thread_resume_params(thread_id, params);
        let result = self
            .request(METHOD_THREAD_RESUME, body, rpc_timeout)
            .await
            .map_err(|err| classify_app_server_error(&err.to_string(), params.model, params.provider))?;
        Ok(extract_thread_id(&result).unwrap_or_else(|| thread_id.to_string()))
    }

    pub async fn thread_unsubscribe(
        &self,
        thread_id: &str,
        rpc_timeout: Duration,
    ) -> Result<(), ClaudeError> {
        self.request(
            METHOD_THREAD_UNSUBSCRIBE,
            json!({ "threadId": thread_id }),
            rpc_timeout,
        )
        .await
        .map(|_| ())
        .map_err(|err| ClaudeError::Reported(err.to_string()))
    }

    pub async fn thread_archive(
        &self,
        thread_id: &str,
        rpc_timeout: Duration,
    ) -> Result<(), ClaudeError> {
        self.request(
            METHOD_THREAD_ARCHIVE,
            json!({ "threadId": thread_id }),
            rpc_timeout,
        )
        .await
        .map(|_| ())
        .map_err(|err| ClaudeError::Reported(err.to_string()))
    }

    pub async fn turn_start(
        &self,
        thread_id: &str,
        message: &str,
        model: &str,
        access: Access<'_>,
        provider: Option<&ProviderConfig>,
        rpc_timeout: Duration,
    ) -> Result<String, ClaudeError> {
        let mut body = json!({
            "threadId": thread_id,
            "input": [{ "type": "text", "text": message }],
        });
        if let Some(obj) = body.as_object_mut() {
            if !model.trim().is_empty() {
                obj.insert("model".to_string(), json!(model));
            }
            if let Access::Project(project) = access {
                obj.insert(
                    "cwd".to_string(),
                    json!(project.dir.to_string_lossy().as_ref()),
                );
                obj.insert("approvalPolicy".to_string(), json!(approval_policy_for(project.level)));
            }
        }
        let result = self
            .request(METHOD_TURN_START, body, rpc_timeout)
            .await
            .map_err(|err| classify_app_server_error(&err.to_string(), model, provider))?;
        Ok(extract_turn_id(&result).unwrap_or_else(|| format!("turn-{}", self.next_id.load(Ordering::Relaxed))))
    }

    pub async fn turn_steer(
        &self,
        thread_id: &str,
        expected_turn_id: Option<&str>,
        message: &str,
        rpc_timeout: Duration,
    ) -> Result<bool, ClaudeError> {
        let mut body = json!({
            "threadId": thread_id,
            "input": [{ "type": "text", "text": message }],
        });
        if let (Some(obj), Some(turn_id)) = (body.as_object_mut(), expected_turn_id) {
            obj.insert("expectedTurnId".to_string(), json!(turn_id));
        }
        match self.request(METHOD_TURN_STEER, body, rpc_timeout).await {
            Ok(_) => Ok(true),
            Err(err) if is_no_active_turn_error(&err.message) => Ok(false),
            Err(err) => Err(classify_app_server_error(&err.to_string(), "", None)),
        }
    }

    pub async fn turn_interrupt(
        &self,
        thread_id: &str,
        turn_id: Option<&str>,
        rpc_timeout: Duration,
    ) -> Result<(), ClaudeError> {
        let mut body = json!({ "threadId": thread_id });
        if let (Some(obj), Some(tid)) = (body.as_object_mut(), turn_id) {
            obj.insert("turnId".to_string(), json!(tid));
        }
        self.request(METHOD_TURN_INTERRUPT, body, rpc_timeout)
            .await
            .map(|_| ())
            .map_err(|err| ClaudeError::Reported(err.to_string()))
    }

    pub async fn model_list(&self, rpc_timeout: Duration) -> Result<Vec<String>, ClaudeError> {
        let result = self
            .request(METHOD_MODEL_LIST, json!({ "includeHidden": false }), rpc_timeout)
            .await
            .map_err(|err| classify_app_server_error(&err.to_string(), "", None))?;
        Ok(parse_model_list_result(&result))
    }

    pub async fn read_rate_limits(
        &self,
        now_ms: u64,
        rpc_timeout: Duration,
    ) -> Result<Option<ProviderPlanUsage>, ClaudeError> {
        let result = self
            .request(METHOD_RATE_LIMITS_READ, json!({}), rpc_timeout)
            .await
            .map_err(|err| classify_app_server_error(&err.to_string(), "", None))?;
        let wrapped = json!({ "result": result });
        parse_rate_limits_rpc_message(&wrapped, now_ms)
            .map_err(|err| ClaudeError::Reported(err.to_string()))
    }

    pub fn shutdown_sync(&self) {
        self.alive.store(false, Ordering::SeqCst);
        if let Some(pid) = self.pid {
            terminate_process_tree(pid);
        }
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                let _ = child.start_kill();
            }
        }
    }
}

fn dispatch_incoming_message(
    value: Json,
    pending: &StdMutex<PendingRequests>,
    listeners: &StdMutex<TurnListeners>,
    global_usage_push: &StdMutex<Option<UsagePushCallback>>,
) {
    let has_id = value.get("id").is_some_and(|v| !v.is_null());
    let method = value.get("method").and_then(Json::as_str).map(str::to_string);

    match (has_id, method) {
        // Resposta JSON-RPC para uma requisição pendente do cliente.
        (true, None) => {
            let Some(id) = value.get("id").and_then(Json::as_u64) else {
                return;
            };
            let tx = {
                let mut guard = pending.lock().unwrap_or_else(PoisonError::into_inner);
                guard.map.remove(&id)
            };
            if let Some(tx) = tx {
                let outcome = if let Some(err) = value.get("error") {
                    let code = err.get("code").and_then(Json::as_i64).unwrap_or(0);
                    let message = err
                        .get("message")
                        .and_then(Json::as_str)
                        .unwrap_or("erro RPC desconhecido no Codex")
                        .to_string();
                    Err(RpcError { code, message })
                } else {
                    Ok(value.get("result").cloned().unwrap_or(Json::Null))
                };
                let _ = tx.send(outcome);
            }
        }
        // Requisição server -> client (ex.: aprovação de comando ou edição de arquivo).
        (true, Some(method)) => {
            let id = value.get("id").cloned().unwrap_or(Json::Null);
            let params = value.get("params").cloned().unwrap_or(Json::Null);
            let thread_id = extract_notification_thread_id(&params);
            let event = ServerEvent::ServerRequest { id, method, params };
            route_event_to_listener(listeners, thread_id.as_deref(), event);
        }
        // Notificação server -> client.
        (false, Some(method)) => {
            if method == METHOD_RATE_LIMITS_UPDATED {
                if let Ok(Some(usage)) = parse_rate_limits_rpc_message(&value, usage_now_millis()) {
                    let callback = global_usage_push
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .clone();
                    if let Some(cb) = callback {
                        cb(usage);
                    }
                }
            }
            let thread_id = value
                .get("params")
                .and_then(extract_notification_thread_id);
            route_event_to_listener(
                listeners,
                thread_id.as_deref(),
                ServerEvent::Notification(value),
            );
        }
        (false, None) => {}
    }
}

fn route_event_to_listener(
    listeners: &StdMutex<TurnListeners>,
    thread_id: Option<&str>,
    event: ServerEvent,
) {
    let guard = listeners.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(tid) = thread_id {
        if let Some(tx) = guard.by_thread.get(tid) {
            let _ = tx.send(event);
            return;
        }
    }
    // Notificações globais (ex.: account/rateLimits/updated sem threadId) vão a todos os turnos ativos.
    if thread_id.is_none() {
        match event {
            ServerEvent::Notification(val) => {
                for tx in guard.by_thread.values() {
                    let _ = tx.send(ServerEvent::Notification(val.clone()));
                }
            }
            ServerEvent::ServerRequest { id, method, params } => {
                if guard.by_thread.len() == 1 {
                    if let Some(tx) = guard.by_thread.values().next() {
                        let _ = tx.send(ServerEvent::ServerRequest { id, method, params });
                    }
                }
            }
            ServerEvent::Closed(reason) => {
                for tx in guard.by_thread.values() {
                    let _ = tx.send(ServerEvent::Closed(reason.clone()));
                }
            }
        }
    }
}

fn extract_notification_thread_id(params: &Json) -> Option<String> {
    params
        .get("threadId")
        .or_else(|| params.get("thread_id"))
        .or_else(|| params.pointer("/thread/id"))
        .or_else(|| params.pointer("/turn/threadId"))
        .or_else(|| params.pointer("/turn/thread_id"))
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn format_stderr_reason(stderr_tail: &StdMutex<VecDeque<String>>) -> String {
    let guard = stderr_tail.lock().unwrap_or_else(PoisonError::into_inner);
    if guard.is_empty() {
        "o processo codex app-server encerrou inesperadamente.".to_string()
    } else {
        guard.iter().cloned().collect::<Vec<_>>().join("\n")
    }
}

fn fail_all_pending(pending: &StdMutex<PendingRequests>, reason: &str) {
    let drained: Vec<_> = {
        let mut guard = pending.lock().unwrap_or_else(PoisonError::into_inner);
        guard.map.drain().collect()
    };
    for (_, tx) in drained {
        let _ = tx.send(Err(RpcError {
            code: 0,
            message: reason.to_string(),
        }));
    }
}

fn notify_all_listeners_closed(listeners: &StdMutex<TurnListeners>, reason: &str) {
    let guard = listeners.lock().unwrap_or_else(PoisonError::into_inner);
    for tx in guard.by_thread.values() {
        let _ = tx.send(ServerEvent::Closed(reason.to_string()));
    }
}

pub struct ThreadParams<'a> {
    pub model: &'a str,
    pub role: &'a str,
    pub access: Access<'a>,
    pub provider: Option<&'a ProviderConfig>,
    pub mcp: Option<&'a McpSession>,
}

fn sandbox_mode_for(access: Access<'_>) -> &'static str {
    match access {
        Access::TextOnly => "read-only",
        Access::Project(project) => match project.level {
            PermissionLevel::Leitura => "read-only",
            PermissionLevel::Edicao => "workspace-write",
            PermissionLevel::Liberado => "danger-full-access",
        },
    }
}

fn approval_policy_for(level: PermissionLevel) -> &'static str {
    match level {
        PermissionLevel::Liberado => "never",
        PermissionLevel::Leitura | PermissionLevel::Edicao => "on-request",
    }
}

fn build_thread_common_params(params: &ThreadParams<'_>) -> serde_json::Map<String, Json> {
    let mut map = serde_json::Map::new();
    if !params.model.trim().is_empty() {
        map.insert("model".to_string(), json!(params.model));
    }
    map.insert("sandbox".to_string(), json!(sandbox_mode_for(params.access)));
    if !params.role.trim().is_empty() {
        map.insert("developerInstructions".to_string(), json!(params.role));
    }
    let mut config_obj = serde_json::Map::new();
    match params.access {
        Access::TextOnly => {
            map.insert("approvalPolicy".to_string(), json!("never"));
        }
        Access::Project(project) => {
            map.insert(
                "cwd".to_string(),
                json!(project.dir.to_string_lossy().as_ref()),
            );
            map.insert(
                "approvalPolicy".to_string(),
                json!(approval_policy_for(project.level)),
            );
            if project.level == PermissionLevel::Edicao {
                config_obj.insert(
                    "sandbox_workspace_write.writable_roots".to_string(),
                    json!([project.attachments.to_string_lossy().as_ref()]),
                );
            }
        }
    }
    if let Some(session) = params.mcp {
        config_obj.insert(
            "mcp_servers.vokia.url".to_string(),
            json!(session.url()),
        );
        config_obj.insert(
            "mcp_servers.vokia.http_headers".to_string(),
            json!({ "Authorization": format!("Bearer {}", session.token()) }),
        );
    }
    if !config_obj.is_empty() {
        map.insert("config".to_string(), Json::Object(config_obj));
    }
    map
}

pub fn build_thread_start_params(params: &ThreadParams<'_>) -> Json {
    Json::Object(build_thread_common_params(params))
}

pub fn build_thread_resume_params(thread_id: &str, params: &ThreadParams<'_>) -> Json {
    let mut map = build_thread_common_params(params);
    map.insert("threadId".to_string(), json!(thread_id));
    Json::Object(map)
}

fn extract_thread_id(result: &Json) -> Option<String> {
    result
        .pointer("/thread/id")
        .or_else(|| result.get("threadId"))
        .or_else(|| result.get("thread_id"))
        .or_else(|| result.get("id"))
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn extract_turn_id(result: &Json) -> Option<String> {
    result
        .pointer("/turn/id")
        .or_else(|| result.get("turnId"))
        .or_else(|| result.get("turn_id"))
        .or_else(|| result.get("id"))
        .and_then(Json::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub fn parse_model_list_result(result: &Json) -> Vec<String> {
    let items = result
        .get("data")
        .or_else(|| result.get("models"))
        .or_else(|| result.get("items"))
        .and_then(Json::as_array)
        .or_else(|| result.as_array());
    let Some(items) = items else {
        return Vec::new();
    };
    let mut models = Vec::new();
    for item in items {
        let hidden = item
            .get("hidden")
            .and_then(Json::as_bool)
            .unwrap_or(false);
        if hidden {
            continue;
        }
        let id = item
            .as_str()
            .or_else(|| item.get("model").and_then(Json::as_str))
            .or_else(|| item.get("id").and_then(Json::as_str))
            .or_else(|| item.get("slug").and_then(Json::as_str))
            .map(str::trim)
            .filter(|s| !s.is_empty());
        if let Some(id) = id {
            if !models.iter().any(|existing| existing == id) {
                models.push(id.to_string());
            }
        }
    }
    models
}

// ==========================================
// 4. Processador de eventos estruturados do turno
// ==========================================

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CumulativeTokenCheckpoint {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub thought_tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnCompletion {
    Completed,
    Interrupted,
    Failed(String),
}

pub struct TurnEventCallbacks<'a> {
    pub on_plan: &'a mut dyn FnMut(TurnPlan),
    pub on_activity: &'a mut dyn FnMut(TurnActivity),
    pub on_rate_limits: &'a mut dyn FnMut(ProviderPlanUsage),
    pub on_console: &'a mut dyn FnMut(ConsoleLine),
}

pub struct CodexTurnCollector {
    provider_id: Option<String>,
    model: String,
    activity: TurnActivity,
    last_event_at: Instant,
    item_order: Vec<String>,
    text_by_item: HashMap<String, String>,
    completed_items: HashMap<String, bool>,
    tool_indices: HashMap<String, usize>,
    command_outputs: HashMap<String, String>,
    open_background_commands: HashMap<String, bool>,
    previous_checkpoint: CumulativeTokenCheckpoint,
    latest_checkpoint: CumulativeTokenCheckpoint,
    saw_total_usage: bool,
    last_turn_usage: Option<TurnUsage>,
    context_tokens: Option<u64>,
    context_window: Option<u64>,
    pub latest_rate_limits: Option<ProviderPlanUsage>,
    pub completion: Option<TurnCompletion>,
}

impl CodexTurnCollector {
    pub fn new(
        provider: Option<&ProviderConfig>,
        model: &str,
        previous_checkpoint: CumulativeTokenCheckpoint,
    ) -> Self {
        Self {
            provider_id: provider.map(|p| p.id.clone()),
            model: model.to_string(),
            activity: TurnActivity::default(),
            last_event_at: Instant::now(),
            item_order: Vec::new(),
            text_by_item: HashMap::new(),
            completed_items: HashMap::new(),
            tool_indices: HashMap::new(),
            command_outputs: HashMap::new(),
            open_background_commands: HashMap::new(),
            previous_checkpoint,
            latest_checkpoint: previous_checkpoint,
            saw_total_usage: false,
            last_turn_usage: None,
            context_tokens: None,
            context_window: None,
            latest_rate_limits: None,
            completion: None,
        }
    }

    pub fn active_background_terminals(&self) -> u32 {
        self.open_background_commands
            .values()
            .filter(|active| **active)
            .count() as u32
    }

    pub fn process_notification(
        &mut self,
        msg: &Json,
        now_ms: u64,
        callbacks: &mut TurnEventCallbacks<'_>,
    ) {
        let now = Instant::now();
        let elapsed_ms = (now.saturating_duration_since(self.last_event_at).as_millis() as u64).max(100);
        self.last_event_at = now;

        // 1. Plano vivo (turn/plan/updated) -> alimenta 088
        if let Some(plan) = plan_from_codex_notification(msg) {
            if !plan.is_empty() {
                self.activity.plan = Some(plan.clone());
                (callbacks.on_plan)(plan);
                (callbacks.on_activity)(self.activity.clone());
            }
            return;
        }

        let Some(method) = msg.get("method").and_then(Json::as_str) else {
            return;
        };
        let params = msg.get("params").unwrap_or(&Json::Null);

        match method {
            METHOD_RATE_LIMITS_UPDATED => {
                if let Ok(Some(usage)) = parse_rate_limits_rpc_message(msg, now_ms) {
                    self.latest_rate_limits = Some(usage.clone());
                    (callbacks.on_rate_limits)(usage);
                }
            }
            "item/agentMessage/delta" => {
                let item_id = params
                    .get("itemId")
                    .or_else(|| params.get("item_id"))
                    .and_then(Json::as_str)
                    .unwrap_or("agent-msg-default");
                if let Some(delta) = params.get("delta").and_then(Json::as_str) {
                    if !self.text_by_item.contains_key(item_id) {
                        self.item_order.push(item_id.to_string());
                    }
                    self.text_by_item
                        .entry(item_id.to_string())
                        .or_default()
                        .push_str(delta);
                }
            }
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                if let Some(delta) = params.get("delta").and_then(Json::as_str).filter(|s| !s.is_empty()) {
                    match &mut self.activity.thinking_text {
                        Some(existing) => existing.push_str(delta),
                        None => self.activity.thinking_text = Some(delta.to_string()),
                    }
                }
                self.activity.thinking_ms = Some(
                    self.activity
                        .thinking_ms
                        .unwrap_or(0)
                        .saturating_add(elapsed_ms),
                );
                self.activity.current_step_label = None;
                (callbacks.on_activity)(self.activity.clone());
            }
            "item/commandExecution/outputDelta" => {
                if let (Some(item_id), Some(delta)) = (
                    params
                        .get("itemId")
                        .or_else(|| params.get("item_id"))
                        .and_then(Json::as_str),
                    params.get("delta").and_then(Json::as_str),
                ) {
                    let entry = self.command_outputs.entry(item_id.to_string()).or_default();
                    entry.push_str(delta);
                    if entry.chars().count() > COMMAND_OUTPUT_TAIL_CHARS * 4 {
                        *entry = tail_chars(entry, COMMAND_OUTPUT_TAIL_CHARS * 2);
                    }
                }
            }
            "item/started" | "item/updated" => {
                if let Some(item) = params.get("item") {
                    self.handle_item_event(item, false, elapsed_ms, callbacks);
                }
            }
            "item/completed" => {
                if let Some(item) = params.get("item") {
                    self.handle_item_event(item, true, elapsed_ms, callbacks);
                }
            }
            "thread/tokenUsage/updated" => {
                self.handle_token_usage(params);
            }
            "turn/completed" => {
                let turn = params.get("turn").unwrap_or(params);
                let status = turn
                    .get("status")
                    .and_then(Json::as_str)
                    .unwrap_or("completed")
                    .to_ascii_lowercase();
                if status == "failed" {
                    let message = turn
                        .pointer("/error/message")
                        .or_else(|| turn.get("message"))
                        .or_else(|| params.pointer("/error/message"))
                        .and_then(Json::as_str)
                        .unwrap_or("erro desconhecido no Codex");
                    self.completion = Some(TurnCompletion::Failed(message.to_string()));
                } else if status == "interrupted" {
                    self.completion = Some(TurnCompletion::Interrupted);
                } else {
                    self.completion = Some(TurnCompletion::Completed);
                }
            }
            "error" => {
                let will_retry = params
                    .get("willRetry")
                    .or_else(|| params.get("will_retry"))
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                let message = params
                    .pointer("/error/message")
                    .or_else(|| params.get("message"))
                    .and_then(Json::as_str)
                    .unwrap_or("erro desconhecido no Codex");
                if will_retry {
                    (callbacks.on_console)(ConsoleLine::new(
                        ConsoleKind::Text,
                        &format!("Codex tentando novamente: {message}"),
                    ));
                } else {
                    self.completion = Some(TurnCompletion::Failed(message.to_string()));
                }
            }
            _ => {}
        }
    }

    fn handle_item_event(
        &mut self,
        item: &Json,
        completed: bool,
        elapsed_ms: u64,
        callbacks: &mut TurnEventCallbacks<'_>,
    ) {
        let item_type = item.get("type").and_then(Json::as_str).unwrap_or("");
        let item_id = item
            .get("id")
            .and_then(Json::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("codex-item-{}", self.activity.tools.len() + 1));

        match item_type {
            "agentMessage" | "agent_message" => {
                if completed {
                    if let Some(text) = extract_agent_message_text(item) {
                        if !text.trim().is_empty() {
                            if !self.text_by_item.contains_key(&item_id) {
                                self.item_order.push(item_id.clone());
                            }
                            self.text_by_item.insert(item_id.clone(), text);
                            self.completed_items.insert(item_id, true);
                        }
                    }
                }
            }
            "reasoning" => {
                if let Some(text) = extract_reasoning_text(item) {
                    if !text.is_empty() {
                        match &mut self.activity.thinking_text {
                            Some(existing) if !existing.ends_with(&text) => {
                                if !existing.is_empty() {
                                    existing.push('\n');
                                }
                                existing.push_str(&text);
                            }
                            None => self.activity.thinking_text = Some(text),
                            _ => {}
                        }
                    }
                }
                self.activity.thinking_ms = Some(
                    self.activity
                        .thinking_ms
                        .unwrap_or(0)
                        .saturating_add(elapsed_ms),
                );
                self.activity.current_step_label = None;
                (callbacks.on_activity)(self.activity.clone());
            }
            "commandExecution" | "command_execution" => {
                let is_background = item
                    .get("isBackground")
                    .or_else(|| item.get("background"))
                    .and_then(Json::as_bool)
                    .unwrap_or(false);
                if completed {
                    self.open_background_commands.remove(&item_id);
                } else if is_background {
                    self.open_background_commands.insert(item_id.clone(), true);
                }

                if let Some(label) = label_for_codex_item(item) {
                    self.activity.current_step_label = Some(label.clone());
                    if !completed {
                        (callbacks.on_console)(ConsoleLine::new(ConsoleKind::Tool, &label));
                    }
                }

                self.upsert_command_tool(&item_id, item, completed);
                (callbacks.on_activity)(self.activity.clone());
            }
            "fileChange" | "file_change" | "patchApply" | "patch_apply" => {
                if let Some(label) = label_for_codex_item(item) {
                    self.activity.current_step_label = Some(label.clone());
                    if !completed {
                        (callbacks.on_console)(ConsoleLine::new(ConsoleKind::Tool, &label));
                    }
                }
                if completed {
                    self.record_file_change_tools(&item_id, item);
                }
                (callbacks.on_activity)(self.activity.clone());
            }
            "mcpToolCall" | "mcp_tool_call" | "functionCall" | "function_call" | "toolUse"
            | "tool_use" => {
                if let Some(label) = label_for_codex_item(item) {
                    self.activity.current_step_label = Some(label.clone());
                    if !completed {
                        (callbacks.on_console)(ConsoleLine::new(ConsoleKind::Tool, &label));
                    }
                }
                self.upsert_mcp_tool(&item_id, item, completed);
                (callbacks.on_activity)(self.activity.clone());
            }
            "webSearch" | "web_search" => {
                if let Some(label) = label_for_codex_item(item) {
                    self.activity.current_step_label = Some(label.clone());
                    if !completed {
                        (callbacks.on_console)(ConsoleLine::new(ConsoleKind::Tool, &label));
                    }
                }
                self.upsert_web_search_tool(&item_id, item, completed);
                (callbacks.on_activity)(self.activity.clone());
            }
            _ => {}
        }
    }

    fn upsert_command_tool(&mut self, item_id: &str, item: &Json, completed: bool) {
        let Some(command) = item
            .get("command")
            .or_else(|| item.get("aggregatedCommand"))
            .or_else(|| item.get("aggregated_command"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            return;
        };
        let exit_code = item
            .get("exitCode")
            .or_else(|| item.get("exit_code"))
            .and_then(Json::as_i64)
            .and_then(|code| i32::try_from(code).ok());
        let duration_ms = item
            .get("durationMs")
            .or_else(|| item.get("duration_ms"))
            .and_then(Json::as_u64);
        let status = match exit_code {
            Some(0) => ToolStatus::Ok,
            Some(_) => ToolStatus::Failed,
            None => parse_tool_status(item, completed),
        };
        let output = item
            .get("aggregatedOutput")
            .or_else(|| item.get("aggregated_output"))
            .or_else(|| item.get("output"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| tail_chars(s, COMMAND_OUTPUT_TAIL_CHARS))
            .or_else(|| {
                self.command_outputs
                    .get(item_id)
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(|s| tail_chars(s, COMMAND_OUTPUT_TAIL_CHARS))
            });

        let description = label_for_codex_item(item);
        let entry = ToolActivity {
            call_id: item_id.to_string(),
            name: "Bash".to_string(),
            category: ToolCategory::Command,
            target: command.to_string(),
            status,
            description,
            added: None,
            removed: None,
            change: None,
            duration_ms,
            detail: None,
            result: output,
            exit_code,
        };
        self.upsert_tool(item_id, entry);
    }

    fn record_file_change_tools(&mut self, item_id: &str, item: &Json) {
        let status = parse_tool_status(item, true);
        if let Some(changes) = item.get("changes").and_then(Json::as_array) {
            for (idx, change) in changes.iter().enumerate() {
                let Some(path) = change
                    .get("path")
                    .or_else(|| change.get("file_path"))
                    .and_then(Json::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                else {
                    continue;
                };
                let kind = change
                    .get("kind")
                    .or_else(|| change.get("action"))
                    .and_then(Json::as_str)
                    .unwrap_or("edit")
                    .to_ascii_lowercase();
                let name = match kind.as_str() {
                    "add" | "create" => "Write",
                    "delete" | "remove" => "Delete",
                    _ => "Edit",
                };
                let sub_id = format!("{item_id}-{idx}");
                self.upsert_tool(
                    &sub_id,
                    ToolActivity {
                        call_id: sub_id.clone(),
                        name: name.to_string(),
                        category: ToolCategory::Edit,
                        target: path.to_string(),
                        status,
                        description: None,
                        added: None,
                        removed: (name == "Write").then_some(0),
                        change: None,
                        duration_ms: None,
                        detail: None,
                        result: None,
                        exit_code: None,
                    },
                );
            }
        } else if let Some(path) = item
            .get("path")
            .or_else(|| item.get("file_path"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            self.upsert_tool(
                item_id,
                ToolActivity {
                    call_id: item_id.to_string(),
                    name: "Edit".to_string(),
                    category: ToolCategory::Edit,
                    target: path.to_string(),
                    status,
                    description: None,
                    added: None,
                    removed: None,
                    change: None,
                    duration_ms: None,
                    detail: None,
                    result: None,
                    exit_code: None,
                },
            );
        }
    }

    fn upsert_mcp_tool(&mut self, item_id: &str, item: &Json, completed: bool) {
        let Some(raw_name) = item
            .get("tool")
            .or_else(|| item.get("name"))
            .or_else(|| item.pointer("/function/name"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            return;
        };
        let server = item
            .get("server")
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let name = match server {
            Some(srv) if !raw_name.contains('/') && !raw_name.starts_with("mcp__") => {
                format!("{srv}/{raw_name}")
            }
            _ => raw_name.to_string(),
        };
        let status = parse_tool_status(item, completed);
        let duration_ms = item
            .get("durationMs")
            .or_else(|| item.get("duration_ms"))
            .and_then(Json::as_u64);
        self.upsert_tool(
            item_id,
            ToolActivity {
                call_id: item_id.to_string(),
                name,
                category: ToolCategory::Mcp,
                target: String::new(),
                status,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms,
                detail: None,
                result: None,
                exit_code: None,
            },
        );
    }

    fn upsert_web_search_tool(&mut self, item_id: &str, item: &Json, completed: bool) {
        let Some(query) = item
            .get("query")
            .or_else(|| item.pointer("/action/query"))
            .and_then(Json::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            return;
        };
        let status = parse_tool_status(item, completed);
        self.upsert_tool(
            item_id,
            ToolActivity {
                call_id: item_id.to_string(),
                name: "WebSearch".to_string(),
                category: ToolCategory::Web,
                target: query.to_string(),
                status,
                description: None,
                added: None,
                removed: None,
                change: None,
                duration_ms: None,
                detail: None,
                result: None,
                exit_code: None,
            },
        );
    }

    fn upsert_tool(&mut self, key: &str, entry: ToolActivity) {
        if let Some(&idx) = self.tool_indices.get(key) {
            self.activity.tools[idx] = entry;
        } else {
            let idx = self.activity.tools.len();
            self.tool_indices.insert(key.to_string(), idx);
            self.activity.tools.push(entry);
        }
    }

    fn handle_token_usage(&mut self, params: &Json) {
        let token_usage = params
            .get("tokenUsage")
            .or_else(|| params.get("token_usage"))
            .unwrap_or(params);

        if let Some(window) = token_usage
            .get("modelContextWindow")
            .or_else(|| token_usage.get("model_context_window"))
            .and_then(Json::as_u64)
            .filter(|&n| n > 0)
        {
            self.context_window = Some(window);
        }

        // `tokenUsage.last` reflete a ocupação atual da janela de contexto na última requisição do turno.
        let last = token_usage.get("last");
        if let Some(last_obj) = last {
            let last_input = json_u64_any(last_obj, &["inputTokens", "input_tokens"]);
            let last_output = json_u64_any(last_obj, &["outputTokens", "output_tokens"]);
            let last_total = json_u64_any(last_obj, &["totalTokens", "total_tokens"]).or_else(|| {
                match (last_input, last_output) {
                    (Some(i), Some(o)) => Some(i.saturating_add(o)),
                    (Some(i), None) => Some(i),
                    _ => None,
                }
            });
            if let Some(total) = last_total {
                self.context_tokens = Some(total);
            }
        }

        // Contabilização por delta contra o checkpoint cumulativo da thread para evitar dupla contagem.
        if let Some(total_obj) = token_usage.get("total") {
            let total_input = json_u64_any(total_obj, &["inputTokens", "input_tokens"]).unwrap_or(0);
            let total_output =
                json_u64_any(total_obj, &["outputTokens", "output_tokens"]).unwrap_or(0);
            let total_cached =
                json_u64_any(total_obj, &["cachedInputTokens", "cached_input_tokens"]).unwrap_or(0);
            let total_thought = json_u64_any(
                total_obj,
                &["reasoningOutputTokens", "reasoning_output_tokens"],
            )
            .unwrap_or(0);

            self.saw_total_usage = true;
            self.latest_checkpoint = CumulativeTokenCheckpoint {
                input_tokens: total_input,
                output_tokens: total_output,
                cached_input_tokens: total_cached,
                thought_tokens: total_thought,
            };
        } else if let Some(last_obj) = last {
            let input_tokens = json_u64_any(last_obj, &["inputTokens", "input_tokens"]);
            let output_tokens = json_u64_any(last_obj, &["outputTokens", "output_tokens"]);
            let cached_input_tokens =
                json_u64_any(last_obj, &["cachedInputTokens", "cached_input_tokens"]);
            let thought_tokens = json_u64_any(
                last_obj,
                &["reasoningOutputTokens", "reasoning_output_tokens"],
            );
            self.last_turn_usage = Some(TurnUsage {
                provider: self.provider_id.clone(),
                model: (!self.model.trim().is_empty()).then(|| self.model.clone()),
                input_tokens,
                output_tokens,
                cached_input_tokens,
                cache_write_tokens: None,
                thought_tokens,
                cost_usd: None,
                is_fallback: None,
                context_tokens: self.context_tokens,
                context_window: self.context_window,
                duration_ms: None,
            });
        }
    }

    pub fn into_reply(
        mut self,
        thread_id: String,
        provider: Option<&ProviderConfig>,
    ) -> Result<(Reply, CumulativeTokenCheckpoint), ClaudeError> {
        if let Some(TurnCompletion::Failed(message)) = self.completion {
            return Err(classify_app_server_error(&message, &self.model, provider));
        }

        let mut parts = Vec::new();
        for item_id in &self.item_order {
            if let Some(text) = self.text_by_item.get(item_id) {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            }
        }
        let text = parts.join("\n\n");

        if matches!(self.completion, Some(TurnCompletion::Interrupted)) && text.is_empty() {
            return Err(ClaudeError::Reported("Turno interrompido no Codex.".to_string()));
        }

        let usage = if self.saw_total_usage {
            let d_in = self
                .latest_checkpoint
                .input_tokens
                .saturating_sub(self.previous_checkpoint.input_tokens);
            let d_out = self
                .latest_checkpoint
                .output_tokens
                .saturating_sub(self.previous_checkpoint.output_tokens);
            let d_cached = self
                .latest_checkpoint
                .cached_input_tokens
                .saturating_sub(self.previous_checkpoint.cached_input_tokens);
            let d_thought = self
                .latest_checkpoint
                .thought_tokens
                .saturating_sub(self.previous_checkpoint.thought_tokens);
            Some(TurnUsage {
                provider: self.provider_id.clone(),
                model: (!self.model.trim().is_empty()).then(|| self.model.clone()),
                input_tokens: Some(d_in),
                output_tokens: Some(d_out),
                cached_input_tokens: (d_cached > 0).then_some(d_cached),
                cache_write_tokens: None,
                thought_tokens: (d_thought > 0).then_some(d_thought),
                cost_usd: None,
                is_fallback: None,
                context_tokens: self.context_tokens,
                context_window: self.context_window,
                duration_ms: None,
            })
        } else {
            self.last_turn_usage.map(|mut u| {
                u.context_tokens = self.context_tokens;
                u.context_window = self.context_window;
                u
            })
        };

        self.activity.current_step_label = None;

        Ok((
            Reply {
                text,
                limits: None,
                activity: self.activity,
                usage,
                session_id: Some(thread_id),
            },
            self.latest_checkpoint,
        ))
    }
}

fn json_u64_any(obj: &Json, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| obj.get(*key).and_then(Json::as_u64))
}

fn parse_tool_status(item: &Json, completed: bool) -> ToolStatus {
    match item
        .get("status")
        .and_then(Json::as_str)
        .unwrap_or(if completed { "completed" } else { "in_progress" })
        .to_ascii_lowercase()
        .as_str()
    {
        "failed" | "error" | "declined" => ToolStatus::Failed,
        "in_progress" | "inprogress" | "running" | "pending" => ToolStatus::Running,
        _ => ToolStatus::Ok,
    }
}

fn extract_agent_message_text(item: &Json) -> Option<String> {
    if let Some(text) = item.get("text").and_then(Json::as_str) {
        return Some(text.to_string());
    }
    if let Some(content) = item.get("content").and_then(Json::as_array) {
        let joined: String = content
            .iter()
            .filter_map(|part| part.get("text").and_then(Json::as_str))
            .collect::<Vec<_>>()
            .join("");
        if !joined.is_empty() {
            return Some(joined);
        }
    }
    None
}

fn extract_reasoning_text(item: &Json) -> Option<String> {
    if let Some(text) = item.get("text").and_then(Json::as_str) {
        return Some(text.to_string());
    }
    if let Some(summary) = item.get("summary").and_then(Json::as_array) {
        let joined = summary
            .iter()
            .filter_map(|entry| {
                entry
                    .as_str()
                    .or_else(|| entry.get("text").and_then(Json::as_str))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if !joined.is_empty() {
            return Some(joined);
        }
    }
    None
}

fn tail_chars(text: &str, max_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_chars {
        text.to_string()
    } else {
        text.chars().skip(total - max_chars).collect()
    }
}

// ==========================================
// 5. Pool de sessões ociosas (CodexIdleSessions)
// ==========================================

#[derive(Clone)]
pub struct CodexThreadEntry {
    pub key: SessionKey,
    pub server_key: ServerKey,
    pub thread_id: String,
    pub state: ThreadLifecycleState,
    pub item_id: Option<String>,
    pub cwd: Option<PathBuf>,
    pub model: String,
    pub active_turn_id: Option<String>,
    pub pending_approvals: u32,
    pub open_questions: u32,
    pub background_terminals: u32,
    pub last_used: Instant,
    pub cumulative_usage: CumulativeTokenCheckpoint,
}

impl CodexThreadEntry {
    pub fn is_protected_from_unload(&self) -> bool {
        self.active_turn_id.is_some()
            || self.pending_approvals > 0
            || self.open_questions > 0
            || self.background_terminals > 0
    }

    pub fn can_unsubscribe(&self, now: Instant, idle_limit: Duration, external_busy: bool) -> bool {
        if self.state == ThreadLifecycleState::Unloaded
            || self.is_protected_from_unload()
            || external_busy
        {
            return false;
        }
        now.saturating_duration_since(self.last_used) >= idle_limit
    }
}

struct ServerEntry {
    state: ServerLifecycleState,
    client: Option<Arc<CodexRpcClient>>,
    last_used: Instant,
}

pub struct CodexIdleSessions {
    config: CodexIdleConfig,
    servers: AsyncMutex<HashMap<ServerKey, ServerEntry>>,
    threads: StdMutex<HashMap<SessionKey, CodexThreadEntry>>,
    active_pids: StdMutex<Vec<u32>>,
}

impl Default for CodexIdleSessions {
    fn default() -> Self {
        Self::with_config(CodexIdleConfig::default())
    }
}

impl CodexIdleSessions {
    pub fn with_config(config: CodexIdleConfig) -> Self {
        Self {
            config,
            servers: AsyncMutex::new(HashMap::new()),
            threads: StdMutex::new(HashMap::new()),
            active_pids: StdMutex::new(Vec::new()),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn register_client_for_test(
        &self,
        key: ServerKey,
        client: Arc<CodexRpcClient>,
        now: Instant,
    ) {
        if let Some(pid) = client.pid() {
            self.track_pid(pid);
        }
        let mut servers = self.servers.lock().await;
        servers.insert(
            key,
            ServerEntry {
                state: ServerLifecycleState::Ready,
                client: Some(client),
                last_used: now,
            },
        );
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn server_state(&self, key: &ServerKey) -> ServerLifecycleState {
        let servers = self.servers.lock().await;
        servers
            .get(key)
            .map(|entry| entry.state)
            .unwrap_or(ServerLifecycleState::Stopped)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn thread_snapshot(&self, key: &SessionKey) -> Option<CodexThreadEntry> {
        self.threads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(key)
            .cloned()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn set_thread_guards_for_test(
        &self,
        key: &SessionKey,
        pending_approvals: u32,
        open_questions: u32,
        background_terminals: u32,
    ) {
        let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = threads.get_mut(key) {
            entry.pending_approvals = pending_approvals;
            entry.open_questions = open_questions;
            entry.background_terminals = background_terminals;
        }
    }

    fn track_pid(&self, pid: u32) {
        if pid == NO_PID {
            return;
        }
        let mut pids = self
            .active_pids
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if !pids.contains(&pid) {
            pids.push(pid);
        }
    }

    fn untrack_pid(&self, pid: u32) {
        let mut pids = self
            .active_pids
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        pids.retain(|existing| *existing != pid);
    }

    async fn ensure_server(
        &self,
        server_key: &ServerKey,
        usage_push: Option<UsagePushCallback>,
    ) -> Result<Arc<CodexRpcClient>, ClaudeError> {
        let mut servers = self.servers.lock().await;
        if let Some(entry) = servers.get_mut(server_key) {
            if let Some(client) = &entry.client {
                if client.is_alive() {
                    entry.state = ServerLifecycleState::Ready;
                    entry.last_used = Instant::now();
                    if let Some(cb) = usage_push {
                        client.set_global_usage_callback(cb);
                    }
                    return Ok(Arc::clone(client));
                }
            }
            if let Some(old_pid) = entry.client.as_ref().and_then(|c| c.pid()) {
                self.untrack_pid(old_pid);
            }
        }

        servers.insert(
            server_key.clone(),
            ServerEntry {
                state: ServerLifecycleState::Starting,
                client: None,
                last_used: Instant::now(),
            },
        );
        drop(servers);

        let client = CodexRpcClient::spawn(&server_key.program, &server_key.user_args, None)?;
        if let Some(pid) = client.pid() {
            self.track_pid(pid);
        }
        if let Some(cb) = usage_push {
            client.set_global_usage_callback(cb);
        }
        if let Err(err) = client.initialize(self.config.handshake_timeout).await {
            if let Some(pid) = client.pid() {
                self.untrack_pid(pid);
            }
            client.shutdown_sync();
            let mut servers = self.servers.lock().await;
            if let Some(entry) = servers.get_mut(server_key) {
                entry.state = ServerLifecycleState::Closed;
                entry.client = None;
            }
            return Err(err);
        }

        let mut servers = self.servers.lock().await;
        servers.insert(
            server_key.clone(),
            ServerEntry {
                state: ServerLifecycleState::Ready,
                client: Some(Arc::clone(&client)),
                last_used: Instant::now(),
            },
        );
        Ok(client)
    }

    async fn existing_server(&self, server_key: &ServerKey) -> Option<Arc<CodexRpcClient>> {
        let servers = self.servers.lock().await;
        servers
            .get(server_key)
            .and_then(|entry| entry.client.as_ref())
            .filter(|client| client.is_alive())
            .cloned()
    }

    async fn touch_server(&self, server_key: &ServerKey, now: Instant) {
        let mut servers = self.servers.lock().await;
        if let Some(entry) = servers.get_mut(server_key) {
            entry.last_used = now;
            if entry.state == ServerLifecycleState::Ready {
                entry.state = ServerLifecycleState::Idle;
            }
        }
    }

    /// Executa um turno persistente reutilizando o `codex app-server` compartilhado e
    /// retomando o `thread_id` (em memória ou persistido no SQLite) via `thread/resume`.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_persistent_turn(
        &self,
        session_key: SessionKey,
        item_id: Option<String>,
        program: &Path,
        user_args: &[String],
        role: &str,
        model: &str,
        access: Access<'_>,
        opening_message: &str,
        follow_up_message: &str,
        persisted_thread_id: Option<&str>,
        provider: Option<&ProviderConfig>,
        mcp: Option<&McpSession>,
        gate: Option<&PermissionGate<'_>>,
        pid_slot: &AtomicU32,
        console: Option<&ConsoleSink<'_>>,
    ) -> Result<Reply, ClaudeError> {
        let server_key = ServerKey::new(program, user_args);
        let client = self.ensure_server(&server_key, None).await?;
        if let Some(pid) = client.pid() {
            pid_slot.store(pid, Ordering::SeqCst);
        }

        let cwd = match access {
            Access::Project(project) => Some(project.dir.to_path_buf()),
            Access::TextOnly => None,
        };

        let existing_entry = self
            .threads
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&session_key)
            .cloned();

        let compatible_existing = existing_entry.filter(|entry| {
            entry.server_key == server_key
                && entry.model == model
                && entry.item_id == item_id
                && entry.cwd == cwd
        });

        let candidate_thread_id: Option<String> = compatible_existing
            .as_ref()
            .map(|e| e.thread_id.clone())
            .or_else(|| {
                persisted_thread_id
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            });

        let already_loaded_in_server = compatible_existing
            .as_ref()
            .is_some_and(|e| e.state != ThreadLifecycleState::Unloaded);

        let thread_params = ThreadParams {
            model,
            role,
            access,
            provider,
            mcp,
        };

        let (thread_id, resumed_existing, initial_checkpoint) = if let Some(tid) = candidate_thread_id {
            if already_loaded_in_server {
                let cp = compatible_existing
                    .as_ref()
                    .map(|e| e.cumulative_usage)
                    .unwrap_or_default();
                (tid, true, cp)
            } else {
                match client
                    .thread_resume(&tid, &thread_params, self.config.rpc_timeout)
                    .await
                {
                    Ok(resumed_id) => {
                        let cp = compatible_existing
                            .as_ref()
                            .map(|e| e.cumulative_usage)
                            .unwrap_or_default();
                        (resumed_id, true, cp)
                    }
                    Err(err) if is_thread_not_found_error(&err.to_string()) => {
                        let fresh_id = client
                            .thread_start(&thread_params, self.config.rpc_timeout)
                            .await?;
                        (fresh_id, false, CumulativeTokenCheckpoint::default())
                    }
                    Err(err) => return Err(err),
                }
            }
        } else {
            let fresh_id = client
                .thread_start(&thread_params, self.config.rpc_timeout)
                .await?;
            (fresh_id, false, CumulativeTokenCheckpoint::default())
        };

        let prompt = if resumed_existing {
            follow_up_message
        } else {
            opening_message
        };

        // Registra a thread em estado inicial (New ou Resumed) antes de iniciar o turno.
        {
            let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            threads.insert(
                session_key.clone(),
                CodexThreadEntry {
                    key: session_key.clone(),
                    server_key: server_key.clone(),
                    thread_id: thread_id.clone(),
                    state: if resumed_existing {
                        ThreadLifecycleState::Resumed
                    } else {
                        ThreadLifecycleState::New
                    },
                    item_id: item_id.clone(),
                    cwd: cwd.clone(),
                    model: model.to_string(),
                    active_turn_id: None,
                    pending_approvals: 0,
                    open_questions: 0,
                    background_terminals: 0,
                    last_used: Instant::now(),
                    cumulative_usage: initial_checkpoint,
                },
            );
        }

        let mut events_rx = client.register_thread_listener(&thread_id);
        let turn_id = match client
            .turn_start(
                &thread_id,
                prompt,
                model,
                access,
                provider,
                self.config.rpc_timeout,
            )
            .await
        {
            Ok(id) => id,
            Err(err) => {
                client.unregister_thread_listener(&thread_id);
                return Err(enrich_code_mode_error(err, program, model));
            }
        };

        // Marca thread como ativa (nunca será descarregada enquanto active_turn_id = Some).
        {
            let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = threads.get_mut(&session_key) {
                entry.state = ThreadLifecycleState::Active;
                entry.active_turn_id = Some(turn_id);
                entry.last_used = Instant::now();
            }
        }

        let turn_result = self
            .drive_turn_loop(
                &client,
                &session_key,
                &thread_id,
                &mut events_rx,
                access,
                provider,
                model,
                initial_checkpoint,
                gate,
                console,
            )
            .await;

        client.unregister_thread_listener(&thread_id);
        pid_slot.store(NO_PID, Ordering::SeqCst);

        let finished_at = Instant::now();
        {
            let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = threads.get_mut(&session_key) {
                entry.active_turn_id = None;
                entry.last_used = finished_at;
                entry.state = ThreadLifecycleState::Idle;
                if let Ok((_, checkpoint, bg)) = &turn_result {
                    entry.cumulative_usage = *checkpoint;
                    entry.background_terminals = *bg;
                }
            }
        }
        self.touch_server(&server_key, finished_at).await;

        turn_result.map(|(reply, _, _)| reply)
    }

    /// Executa um turno efêmero (ex.: modo `per_step` ou `TextOnly`) no `codex app-server`
    /// compartilhado e arquiva a thread ao finalizar para não acumular estado nem travar pastas.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_ephemeral_turn(
        &self,
        program: &Path,
        user_args: &[String],
        role: &str,
        model: &str,
        access: Access<'_>,
        message: &str,
        provider: Option<&ProviderConfig>,
        mcp: Option<&McpSession>,
        gate: Option<&PermissionGate<'_>>,
        pid_slot: &AtomicU32,
        console: Option<&ConsoleSink<'_>>,
    ) -> Result<Reply, ClaudeError> {
        let server_key = ServerKey::new(program, user_args);
        let client = self.ensure_server(&server_key, None).await?;
        if let Some(pid) = client.pid() {
            pid_slot.store(pid, Ordering::SeqCst);
        }

        let thread_params = ThreadParams {
            model,
            role,
            access,
            provider,
            mcp,
        };
        let thread_id = client
            .thread_start(&thread_params, self.config.rpc_timeout)
            .await?;

        let mut events_rx = client.register_thread_listener(&thread_id);
        let started = client
            .turn_start(
                &thread_id,
                message,
                model,
                access,
                provider,
                self.config.rpc_timeout,
            )
            .await;

        let ephemeral_key = SessionKey {
            project_id: 0,
            agent: Agent::ember(),
        };

        let result = match started {
            Ok(_) => {
                self.drive_turn_loop(
                    &client,
                    &ephemeral_key,
                    &thread_id,
                    &mut events_rx,
                    access,
                    provider,
                    model,
                    CumulativeTokenCheckpoint::default(),
                    gate,
                    console,
                )
                .await
            }
            Err(err) => Err(err),
        };

        client.unregister_thread_listener(&thread_id);
        let _ = client
            .thread_archive(&thread_id, Duration::from_secs(5))
            .await;
        pid_slot.store(NO_PID, Ordering::SeqCst);
        self.touch_server(&server_key, Instant::now()).await;

        result.map(|(mut reply, _, _)| {
            reply.session_id = None;
            reply
        })
    }

    #[allow(clippy::too_many_arguments)]
    async fn drive_turn_loop(
        &self,
        client: &Arc<CodexRpcClient>,
        session_key: &SessionKey,
        thread_id: &str,
        events_rx: &mut mpsc::UnboundedReceiver<ServerEvent>,
        access: Access<'_>,
        provider: Option<&ProviderConfig>,
        model: &str,
        initial_checkpoint: CumulativeTokenCheckpoint,
        gate: Option<&PermissionGate<'_>>,
        console: Option<&ConsoleSink<'_>>,
    ) -> Result<(Reply, CumulativeTokenCheckpoint, u32), ClaudeError> {
        let mut collector = CodexTurnCollector::new(provider, model, initial_checkpoint);
        let line_timeout = timeout_for(access);

        loop {
            let next_event = timeout(line_timeout, events_rx.recv())
                .await
                .map_err(|_| ClaudeError::Timeout(line_timeout))?;

            let Some(event) = next_event else {
                return Err(ClaudeError::SessionClosed(client.stderr_snapshot()));
            };

            match event {
                ServerEvent::Notification(msg) => {
                    let mut on_plan = |plan: TurnPlan| {
                        if let Some(sink) = console {
                            sink.turn_plan(plan);
                        }
                    };
                    let mut on_activity = |activity: TurnActivity| {
                        if let Some(sink) = console {
                            sink.live_activity(activity);
                        }
                    };
                    let mut on_rate_limits = |usage: ProviderPlanUsage| {
                        if let Some(sink) = console {
                            sink.provider_usage(usage);
                        }
                    };
                    let mut on_console = |line: ConsoleLine| {
                        if let Some(sink) = console {
                            sink.record(line);
                        }
                    };
                    let mut callbacks = TurnEventCallbacks {
                        on_plan: &mut on_plan,
                        on_activity: &mut on_activity,
                        on_rate_limits: &mut on_rate_limits,
                        on_console: &mut on_console,
                    };
                    collector.process_notification(&msg, usage_now_millis(), &mut callbacks);
                    if collector.completion.is_some() {
                        break;
                    }
                }
                ServerEvent::ServerRequest { id, method, params } => {
                    self.handle_server_request(client, session_key, id, &method, &params, gate)
                        .await?;
                }
                ServerEvent::Closed(reason) => {
                    return Err(classify_app_server_error(&reason, model, provider));
                }
            }
        }

        let bg_count = collector.active_background_terminals();
        let (reply, checkpoint) = collector.into_reply(thread_id.to_string(), provider)?;
        Ok((reply, checkpoint, bg_count))
    }

    async fn handle_server_request(
        &self,
        client: &Arc<CodexRpcClient>,
        session_key: &SessionKey,
        id: Json,
        method: &str,
        params: &Json,
        gate: Option<&PermissionGate<'_>>,
    ) -> Result<(), ClaudeError> {
        let is_question = method.eq_ignore_ascii_case("item/tool/requestUserInput");
        {
            let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = threads.get_mut(session_key) {
                if is_question {
                    entry.open_questions = entry.open_questions.saturating_add(1);
                } else {
                    entry.pending_approvals = entry.pending_approvals.saturating_add(1);
                }
            }
        }

        let response_result = if is_question {
            json!({ "answers": {} })
        } else if let Some(gate) = gate {
            let req = permission_request_from_codex(&id, method, params);
            let verdict = gate.clone().decide(req).await;
            match verdict {
                PermissionVerdict::Allow => json!({ "decision": "accept" }),
                PermissionVerdict::Deny(_) => json!({ "decision": "decline" }),
            }
        } else {
            json!({ "decision": "decline" })
        };

        {
            let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = threads.get_mut(session_key) {
                if is_question {
                    entry.open_questions = entry.open_questions.saturating_sub(1);
                } else {
                    entry.pending_approvals = entry.pending_approvals.saturating_sub(1);
                }
            }
        }

        client
            .respond(id, response_result)
            .map_err(|err| ClaudeError::SessionClosed(err.to_string()))
    }

    /// Envia uma correção (steering) diretamente ao turno ativo do Codex sem esperar o fim.
    /// Retorna `Ok(true)` quando entregue ao turno ativo, ou `Ok(false)` se não houver turno ativo.
    pub async fn steer_turn(&self, key: &SessionKey, message: &str) -> Result<bool, ClaudeError> {
        let active = {
            let threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            threads.get(key).and_then(|entry| {
                entry.active_turn_id.as_ref().map(|turn_id| {
                    (
                        entry.server_key.clone(),
                        entry.thread_id.clone(),
                        turn_id.clone(),
                    )
                })
            })
        };
        let Some((server_key, thread_id, turn_id)) = active else {
            return Ok(false);
        };
        let Some(client) = self.existing_server(&server_key).await else {
            return Ok(false);
        };
        client
            .turn_steer(&thread_id, Some(&turn_id), message, self.config.rpc_timeout)
            .await
    }

    pub async fn interrupt_turn(&self, key: &SessionKey) -> Result<bool, ClaudeError> {
        let active = {
            let threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            threads.get(key).and_then(|entry| {
                entry.active_turn_id.as_ref().map(|turn_id| {
                    (
                        entry.server_key.clone(),
                        entry.thread_id.clone(),
                        turn_id.clone(),
                    )
                })
            })
        };
        let Some((server_key, thread_id, turn_id)) = active else {
            return Ok(false);
        };
        let Some(client) = self.existing_server(&server_key).await else {
            return Ok(false);
        };
        client
            .turn_interrupt(&thread_id, Some(&turn_id), self.config.rpc_timeout)
            .await?;
        Ok(true)
    }

    pub async fn list_models(
        &self,
        program: &Path,
        user_args: &[String],
    ) -> Result<Vec<String>, ClaudeError> {
        let server_key = ServerKey::new(program, user_args);
        let client = self.ensure_server(&server_key, None).await?;
        let models = client.model_list(self.config.rpc_timeout).await?;
        self.touch_server(&server_key, Instant::now()).await;
        Ok(models)
    }

    pub async fn read_rate_limits(
        &self,
        program: &Path,
        user_args: &[String],
        now_ms: u64,
    ) -> Result<Option<ProviderPlanUsage>, ClaudeError> {
        let server_key = ServerKey::new(program, user_args);
        let Some(client) = self.existing_server(&server_key).await else {
            return Ok(None);
        };
        let usage = client
            .read_rate_limits(now_ms, self.config.rpc_timeout)
            .await?;
        self.touch_server(&server_key, Instant::now()).await;
        Ok(usage)
    }

    /// Executa coleta de ociosidade:
    /// 1) Envia `thread/unsubscribe` após 5 min sem turno ativo, aprovação pendente, pergunta aberta ou terminal em background (mantendo `thread_id` recuperável).
    /// 2) Encerra `codex app-server` após 30 min sem uso (preservando o cache MCP em memória antes disso).
    pub async fn reap_idle(
        &self,
        now: Instant,
        external_busy: impl Fn(&SessionKey) -> bool,
    ) -> ReapReport {
        let mut report = ReapReport::default();

        // 1. Identifica threads candidatas a unsubscribe (>= 5 min ociosas e sem travas).
        let to_unsubscribe: Vec<(SessionKey, ServerKey, String)> = {
            let threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            threads
                .values()
                .filter(|entry| {
                    entry.can_unsubscribe(
                        now,
                        self.config.thread_idle_timeout,
                        external_busy(&entry.key),
                    )
                })
                .map(|entry| {
                    (
                        entry.key.clone(),
                        entry.server_key.clone(),
                        entry.thread_id.clone(),
                    )
                })
                .collect()
        };

        for (session_key, server_key, thread_id) in to_unsubscribe {
            if let Some(client) = self.existing_server(&server_key).await {
                let _ = client
                    .thread_unsubscribe(&thread_id, Duration::from_secs(5))
                    .await;
            }
            let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = threads.get_mut(&session_key) {
                if !entry.is_protected_from_unload() && !external_busy(&session_key) {
                    entry.state = ThreadLifecycleState::Unloaded;
                    report.unsubscribed_threads.push(thread_id);
                }
            }
        }

        // 2. Identifica servidores ociosos >= 30 min sem nenhuma thread ativa ou protegida.
        let protected_servers: Vec<ServerKey> = {
            let threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            threads
                .values()
                .filter(|entry| {
                    entry.is_protected_from_unload() || external_busy(&entry.key)
                })
                .map(|entry| entry.server_key.clone())
                .collect()
        };

        let mut servers = self.servers.lock().await;
        let mut expired_keys = Vec::new();
        for (server_key, entry) in servers.iter() {
            if entry.client.is_none() || protected_servers.contains(server_key) {
                continue;
            }
            if now.saturating_duration_since(entry.last_used) >= self.config.server_idle_timeout {
                expired_keys.push(server_key.clone());
            }
        }

        for server_key in expired_keys {
            if let Some(entry) = servers.get_mut(&server_key) {
                if let Some(client) = entry.client.take() {
                    if let Some(pid) = client.pid() {
                        self.untrack_pid(pid);
                    }
                    client.shutdown_sync();
                }
                entry.state = ServerLifecycleState::Closed;
                report.closed_servers += 1;
            }
            // Threads deste servidor permanecem recuperáveis via thread/resume quando o servidor reabrir.
            let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            for thread in threads.values_mut() {
                if thread.server_key == server_key {
                    thread.state = ThreadLifecycleState::Unloaded;
                }
            }
        }

        report
    }

    pub async fn archive_matching(&self, predicate: impl Fn(&CodexThreadEntry) -> bool) -> usize {
        let targets: Vec<(SessionKey, ServerKey, String)> = {
            let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
            let keys: Vec<SessionKey> = threads
                .iter()
                .filter(|(_, entry)| predicate(entry))
                .map(|(k, _)| k.clone())
                .collect();
            keys.into_iter()
                .filter_map(|key| {
                    threads
                        .remove(&key)
                        .map(|entry| (key, entry.server_key, entry.thread_id))
                })
                .collect()
        };

        let mut archived = 0usize;
        for (_, server_key, thread_id) in targets {
            if let Some(client) = self.existing_server(&server_key).await {
                let _ = client
                    .thread_archive(&thread_id, Duration::from_secs(5))
                    .await;
                archived += 1;
            }
        }
        archived
    }

    pub async fn archive_project(&self, project_id: i64) -> usize {
        self.archive_matching(|entry| entry.key.project_id == project_id)
            .await
    }

    pub async fn archive_agent(&self, agent: &Agent) -> usize {
        self.archive_matching(|entry| is_agent_or_replica(entry.key.agent.as_str(), agent))
            .await
    }

    pub async fn archive_worktree(
        &self,
        project_id: i64,
        item_id: &str,
        worktree_path: Option<&Path>,
    ) -> usize {
        self.archive_matching(|entry| {
            if entry.key.project_id != project_id {
                return false;
            }
            let same_item = entry.item_id.as_deref() == Some(item_id);
            let same_path = worktree_path.is_some_and(|target| {
                entry
                    .cwd
                    .as_deref()
                    .is_some_and(|cwd| paths_match_or_descend(cwd, target))
            });
            same_item || same_path
        })
        .await
    }

    pub async fn archive_all(&self) -> usize {
        self.archive_matching(|_| true).await
    }

    /// Encerramento síncrono de toda a árvore de processos no shutdown do Vokia.
    pub fn shutdown_all_sync(&self) {
        let pids: Vec<u32> = {
            let mut guard = self
                .active_pids
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            std::mem::take(&mut *guard)
        };
        for pid in pids {
            terminate_process_tree(pid);
        }
        if let Ok(mut servers) = self.servers.try_lock() {
            for entry in servers.values_mut() {
                if let Some(client) = entry.client.take() {
                    client.shutdown_sync();
                }
                entry.state = ServerLifecycleState::Closed;
            }
        }
        let mut threads = self.threads.lock().unwrap_or_else(PoisonError::into_inner);
        threads.clear();
    }
}

fn paths_match_or_descend(cwd: &Path, target: &Path) -> bool {
    let normalize = |p: &Path| {
        p.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    };
    let a = normalize(cwd);
    let b = normalize(target);
    a == b || a.starts_with(&format!("{b}/"))
}

fn permission_request_from_codex(id: &Json, method: &str, params: &Json) -> PermissionRequest {
    let req_id = match id {
        Json::String(s) => s.clone(),
        other => other.to_string(),
    };
    let lower = method.to_ascii_lowercase();
    let tool = if lower.contains("filechange") || lower.contains("applypatch") {
        "Edit".to_string()
    } else if lower.contains("command") {
        "Bash".to_string()
    } else {
        method.to_string()
    };
    PermissionRequest {
        id: req_id,
        tool,
        input: params.clone(),
    }
}

/// Testa conectividade com `codex app-server` executando handshake + `model/list` + turno ping leve.
pub async fn test_connection(
    program: &Path,
    user_args: &[String],
    model: &str,
    provider: &ProviderConfig,
    limit: Duration,
) -> Result<String, ClaudeError> {
    let sessions = CodexIdleSessions::with_config(CodexIdleConfig {
        handshake_timeout: limit,
        rpc_timeout: limit,
        ..CodexIdleConfig::default()
    });
    let pid = AtomicU32::new(NO_PID);
    let result = timeout(
        limit,
        sessions.run_ephemeral_turn(
            program,
            user_args,
            "",
            model,
            Access::TextOnly,
            "Responda apenas OK.",
            Some(provider),
            None,
            None,
            &pid,
            None,
        ),
    )
    .await;
    sessions.shutdown_all_sync();
    match result {
        Ok(Ok(reply)) => Ok(reply.text),
        Ok(Err(err)) => Err(err),
        Err(_) => Err(ClaudeError::Timeout(limit)),
    }
}

#[cfg(test)]
#[path = "../../../tests/codex_app_server_tests.rs"]
mod tests;
