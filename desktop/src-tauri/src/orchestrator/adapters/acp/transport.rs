use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex, PoisonError};
use std::time::Duration;

use serde_json::{json, Value as Json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::{mpsc, oneshot, Mutex as AsyncMutex};

use crate::claude::{kill_process_tree, ClaudeError};
use crate::orchestrator::session_pool::NO_PID;

use super::flavor::AcpSpawnSpec;

const STDERR_TAIL_LINES: usize = 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcpRpcError {
    pub code: i64,
    pub message: String,
}

impl std::fmt::Display for AcpRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.code != 0 {
            write!(f, "{} (código RPC {})", self.message, self.code)
        } else {
            f.write_str(&self.message)
        }
    }
}

#[derive(Debug)]
pub enum TransportError {
    Rpc(AcpRpcError),
    Io(String),
    Timeout(Duration),
}

impl From<TransportError> for ClaudeError {
    fn from(err: TransportError) -> Self {
        match err {
            TransportError::Rpc(rpc) => ClaudeError::Reported(rpc.to_string()),
            TransportError::Io(msg) => ClaudeError::Reported(msg),
            TransportError::Timeout(dur) => ClaudeError::Timeout(dur),
        }
    }
}

#[derive(Clone, Debug)]
pub enum IncomingRpc {
    Notification {
        method: String,
        params: Json,
    },
    Request {
        id: Json,
        method: String,
        params: Json,
    },
}

pub struct AcpTransport {
    stdin: AsyncMutex<ChildStdin>,
    pending: Arc<StdMutex<HashMap<u64, oneshot::Sender<Result<Json, AcpRpcError>>>>>,
    incoming_rx: AsyncMutex<mpsc::UnboundedReceiver<IncomingRpc>>,
    next_id: AtomicU64,
    pid: AtomicU32,
    alive: Arc<AtomicBool>,
    stderr_tail: Arc<StdMutex<VecDeque<String>>>,
    temp_files: Vec<PathBuf>,
}

impl AcpTransport {
    pub fn spawn(
        mut spec: AcpSpawnSpec,
        pid_sink: Option<&AtomicU32>,
    ) -> Result<Self, ClaudeError> {
        let mut child = spec.command.spawn().map_err(ClaudeError::Spawn)?;
        let pid = child.id().unwrap_or(NO_PID);
        if let Some(sink) = pid_sink {
            sink.store(pid, Ordering::SeqCst);
        }

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ClaudeError::Reported("stdin indisponível no processo ACP".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ClaudeError::Reported("stdout indisponível no processo ACP".to_string()))?;
        let stderr = child.stderr.take();

        let pending: Arc<StdMutex<HashMap<u64, oneshot::Sender<Result<Json, AcpRpcError>>>>> =
            Arc::new(StdMutex::new(HashMap::new()));
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
        let alive = Arc::new(AtomicBool::new(true));
        let stderr_tail = Arc::new(StdMutex::new(VecDeque::with_capacity(STDERR_TAIL_LINES)));

        if let Some(err_stream) = stderr {
            let tail_ref = Arc::clone(&stderr_tail);
            tokio::spawn(async move {
                let mut lines = BufReader::new(err_stream).lines();
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

        let pending_reader = Arc::clone(&pending);
        let alive_reader = Arc::clone(&alive);
        let tail_for_reader = Arc::clone(&stderr_tail);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(value) = serde_json::from_str::<Json>(trimmed) else {
                    continue;
                };
                if let Some(method) = value.get("method").and_then(Json::as_str) {
                    let params = value.get("params").cloned().unwrap_or(Json::Null);
                    if let Some(id) = value.get("id").filter(|v| !v.is_null()) {
                        let _ = incoming_tx.send(IncomingRpc::Request {
                            id: id.clone(),
                            method: method.to_string(),
                            params,
                        });
                    } else {
                        let _ = incoming_tx.send(IncomingRpc::Notification {
                            method: method.to_string(),
                            params,
                        });
                    }
                    continue;
                }

                if let Some(id_num) = value.get("id").and_then(Json::as_u64) {
                    let tx = {
                        let mut map = pending_reader.lock().unwrap_or_else(PoisonError::into_inner);
                        map.remove(&id_num)
                    };
                    if let Some(tx) = tx {
                        if let Some(err_obj) = value.get("error").filter(|v| !v.is_null()) {
                            let code = err_obj.get("code").and_then(Json::as_i64).unwrap_or(-1);
                            let message = err_obj
                                .get("message")
                                .and_then(Json::as_str)
                                .unwrap_or("erro RPC desconhecido")
                                .to_string();
                            let _ = tx.send(Err(AcpRpcError { code, message }));
                        } else {
                            let result = value.get("result").cloned().unwrap_or(Json::Null);
                            let _ = tx.send(Ok(result));
                        }
                    }
                }
            }
            let _ = child.wait().await;
            alive_reader.store(false, Ordering::SeqCst);
            let stderr_msg = {
                let g = tail_for_reader.lock().unwrap_or_else(PoisonError::into_inner);
                g.iter().cloned().collect::<Vec<_>>().join(" | ")
            };
            let detail = if stderr_msg.is_empty() {
                "processo ACP encerrou inesperadamente".to_string()
            } else {
                format!("processo ACP encerrou inesperadamente: {stderr_msg}")
            };
            let mut map = pending_reader.lock().unwrap_or_else(PoisonError::into_inner);
            for (_, tx) in map.drain() {
                let _ = tx.send(Err(AcpRpcError {
                    code: -32000,
                    message: detail.clone(),
                }));
            }
        });

        Ok(Self {
            stdin: AsyncMutex::new(stdin),
            pending,
            incoming_rx: AsyncMutex::new(incoming_rx),
            next_id: AtomicU64::new(1),
            pid: AtomicU32::new(pid),
            alive,
            stderr_tail,
            temp_files: spec.temp_files,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid.load(Ordering::SeqCst)
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    pub fn stderr_tail(&self) -> String {
        let guard = self.stderr_tail.lock().unwrap_or_else(PoisonError::into_inner);
        guard.iter().cloned().collect::<Vec<_>>().join("\n")
    }

    pub async fn request(
        &self,
        method: &str,
        params: Json,
        timeout_dur: Duration,
    ) -> Result<Json, TransportError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            map.insert(id, tx);
        }

        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        if let Err(err) = self.write_json(&payload).await {
            let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            map.remove(&id);
            return Err(err);
        }

        match tokio::time::timeout(timeout_dur, rx).await {
            Ok(Ok(Ok(result))) => Ok(result),
            Ok(Ok(Err(rpc))) => Err(TransportError::Rpc(rpc)),
            Ok(Err(_)) => Err(TransportError::Io(self.with_stderr("canal RPC ACP fechado"))),
            Err(_) => {
                let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
                map.remove(&id);
                Err(TransportError::Timeout(timeout_dur))
            }
        }
    }

    pub async fn request_unbounded(
        &self,
        method: &str,
        params: Json,
    ) -> Result<Json, TransportError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            map.insert(id, tx);
        }

        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        if let Err(err) = self.write_json(&payload).await {
            let mut map = self.pending.lock().unwrap_or_else(PoisonError::into_inner);
            map.remove(&id);
            return Err(err);
        }

        match rx.await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(rpc)) => Err(TransportError::Rpc(rpc)),
            Err(_) => Err(TransportError::Io(self.with_stderr("canal RPC ACP fechado"))),
        }
    }

    pub async fn notify(&self, method: &str, params: Json) -> Result<(), TransportError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        self.write_json(&payload).await
    }

    pub async fn respond(&self, id: Json, result: Json) -> Result<(), TransportError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });
        self.write_json(&payload).await
    }

    pub async fn respond_error(
        &self,
        id: Json,
        code: i64,
        message: &str,
    ) -> Result<(), TransportError> {
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        });
        self.write_json(&payload).await
    }

    pub async fn recv_incoming(&self) -> Option<IncomingRpc> {
        let mut rx = self.incoming_rx.lock().await;
        rx.recv().await
    }

    pub fn try_recv_incoming(&self) -> Option<IncomingRpc> {
        let Ok(mut rx) = self.incoming_rx.try_lock() else {
            return None;
        };
        rx.try_recv().ok()
    }

    async fn write_json(&self, value: &Json) -> Result<(), TransportError> {
        let mut line = serde_json::to_string(value)
            .map_err(|err| TransportError::Io(err.to_string()))?;
        line.push('\n');
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(line.as_bytes())
            .await
            .map_err(|err| TransportError::Io(self.with_stderr(&format!("falha ao escrever no ACP: {err}"))))?;
        stdin
            .flush()
            .await
            .map_err(|err| TransportError::Io(self.with_stderr(&format!("falha ao enviar flush no ACP: {err}"))))?;
        Ok(())
    }

    fn with_stderr(&self, msg: &str) -> String {
        let tail = self.stderr_tail();
        if tail.is_empty() {
            msg.to_string()
        } else {
            format!("{msg} ({tail})")
        }
    }

    pub fn terminate(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let pid = self.pid.swap(NO_PID, Ordering::SeqCst);
        if pid != NO_PID {
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

impl Drop for AcpTransport {
    fn drop(&mut self) {
        self.terminate();
        for file in &self.temp_files {
            let _ = std::fs::remove_file(file);
        }
    }
}
