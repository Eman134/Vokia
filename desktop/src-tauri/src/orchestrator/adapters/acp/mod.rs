pub mod client;
pub mod flavor;
pub mod transport;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::Mutex as AsyncMutex;

use crate::agents::{is_agent_or_replica, Agent};
use crate::claude::{Access, ClaudeError, Reply};
use crate::mcp::{McpRelayManager, McpSession, StdioMcpServerConfig};
use crate::orchestrator::adapters::ConsoleSink;
use crate::orchestrator::session_pool::NO_PID;
use crate::permissions::PermissionGate;
use crate::providers::ProviderConfig;
use crate::session_key::SessionKey;

use self::client::{
    discover_acp_models, interrupt_acp_session, open_acp_session, run_acp_prompt,
    steer_acp_session, test_acp_connection, SharedAcpSession,
};

pub const ACP_DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const WORK_HOLD_IDLE_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub const WORK_HOLD_MAX_DURATION: Duration = Duration::from_secs(4 * 60 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AcpSessionStatus {
    Stopped,
    Warm,
    Idle,
    Busy,
    WorkHold,
}

#[derive(Clone, Copy, Debug)]
pub struct WorkHoldState {
    pub started_at: Instant,
    pub last_activity: Instant,
}

impl WorkHoldState {
    pub fn new(now: Instant) -> Self {
        Self {
            started_at: now,
            last_activity: now,
        }
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last_activity) >= WORK_HOLD_IDLE_TIMEOUT
            || now.saturating_duration_since(self.started_at) >= WORK_HOLD_MAX_DURATION
    }
}

struct PooledAcpSession {
    handle: SharedAcpSession,
    program: PathBuf,
    user_args: Vec<String>,
    model: String,
    item_id: Option<String>,
    cwd: PathBuf,
    busy: bool,
    last_used: Instant,
    work_hold: Option<WorkHoldState>,
}

pub struct AcpRuntimePool {
    sessions: StdMutex<HashMap<SessionKey, PooledAcpSession>>,
    relay_manager: Arc<McpRelayManager>,
    idle_timeout: Duration,
}

impl Default for AcpRuntimePool {
    fn default() -> Self {
        Self::new(ACP_DEFAULT_IDLE_TIMEOUT)
    }
}

impl AcpRuntimePool {
    pub fn new(idle_timeout: Duration) -> Self {
        Self {
            sessions: StdMutex::new(HashMap::new()),
            relay_manager: McpRelayManager::new(),
            idle_timeout,
        }
    }

    #[allow(dead_code)]
    pub fn relay_manager(&self) -> &Arc<McpRelayManager> {
        &self.relay_manager
    }

    pub fn session_status(&self, key: &SessionKey) -> AcpSessionStatus {
        let guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(entry) = guard.get(key) else {
            return AcpSessionStatus::Stopped;
        };
        if entry.busy {
            return AcpSessionStatus::Busy;
        }
        if entry.work_hold.is_some() {
            return AcpSessionStatus::WorkHold;
        }
        if Instant::now().saturating_duration_since(entry.last_used) < Duration::from_secs(30) {
            AcpSessionStatus::Warm
        } else {
            AcpSessionStatus::Idle
        }
    }

    #[allow(dead_code)]
    pub fn set_work_hold(&self, key: &SessionKey, active: bool, now: Instant) {
        let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = guard.get_mut(key) {
            if active {
                match &mut entry.work_hold {
                    Some(hold) => hold.last_activity = now,
                    None => entry.work_hold = Some(WorkHoldState::new(now)),
                }
            } else {
                entry.work_hold = None;
            }
        }
    }

    pub async fn restart_session(&self, key: &SessionKey) -> bool {
        let removed = {
            let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            guard.remove(key)
        };
        if let Some(entry) = removed {
            let handle = entry.handle.lock().await;
            handle.terminate();
            true
        } else {
            false
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_persistent_turn(
        &self,
        key: SessionKey,
        item_id: Option<String>,
        program: &Path,
        user_args: &[String],
        role: &str,
        model: &str,
        access: Access<'_>,
        opening_prompt: &str,
        follow_up_prompt: &str,
        persisted_session_id: Option<&str>,
        provider: Option<&ProviderConfig>,
        mcp_session: Option<&McpSession>,
        stdio_servers: Vec<StdioMcpServerConfig>,
        gate: Option<&PermissionGate<'_>>,
        pid_sink: &AtomicU32,
        console: Option<&ConsoleSink<'_>>,
    ) -> Result<Reply, ClaudeError> {
        let existing = {
            let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = guard.get_mut(&key) {
                let compatible = entry.program == program
                    && entry.user_args == user_args
                    && entry.model == model
                    && entry.item_id == item_id;
                if compatible {
                    entry.busy = true;
                    Some(Arc::clone(&entry.handle))
                } else {
                    let old = guard.remove(&key);
                    if let Some(old_entry) = old {
                        if let Ok(h) = old_entry.handle.try_lock() {
                            h.terminate();
                        }
                    }
                    None
                }
            } else {
                None
            }
        };

        let mut is_new_or_rotated = false;
        let mut recovery_handoff: Option<String> = None;

        let shared_handle = if let Some(handle_arc) = existing {
            let mut needs_reopen = false;
            {
                let h = handle_arc.lock().await;
                if !h.is_alive() || h.needs_context_rotation() {
                    h.terminate();
                    needs_reopen = true;
                }
            }
            if needs_reopen {
                let (new_handle, handoff) = open_acp_session(
                    program,
                    user_args,
                    model,
                    access,
                    provider,
                    mcp_session,
                    stdio_servers,
                    &self.relay_manager,
                    persisted_session_id,
                    Some(pid_sink),
                )
                .await?;
                is_new_or_rotated = true;
                recovery_handoff = handoff;
                let mut h = handle_arc.lock().await;
                *h = new_handle;
            }
            handle_arc
        } else {
            let (new_handle, handoff) = open_acp_session(
                program,
                user_args,
                model,
                access,
                provider,
                mcp_session,
                stdio_servers,
                &self.relay_manager,
                persisted_session_id,
                Some(pid_sink),
            )
            .await?;
            is_new_or_rotated = true;
            recovery_handoff = handoff;
            let cwd = new_handle.cwd.clone();
            let handle_arc = Arc::new(AsyncMutex::new(new_handle));
            let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            guard.insert(
                key.clone(),
                PooledAcpSession {
                    handle: Arc::clone(&handle_arc),
                    program: program.to_path_buf(),
                    user_args: user_args.to_vec(),
                    model: model.to_string(),
                    item_id,
                    cwd,
                    busy: true,
                    last_used: Instant::now(),
                    work_hold: None,
                },
            );
            handle_arc
        };

        let base_prompt = if is_new_or_rotated {
            if role.trim().is_empty() {
                opening_prompt.to_string()
            } else {
                format!("{role}\n\n{opening_prompt}")
            }
        } else {
            follow_up_prompt.to_string()
        };

        let prompt_text = if let Some(handoff) = recovery_handoff {
            format!("{handoff}\n\n{base_prompt}")
        } else {
            base_prompt
        };

        let result = {
            let mut h = shared_handle.lock().await;
            pid_sink.store(h.transport.pid(), Ordering::SeqCst);
            let res = run_acp_prompt(&mut h, access, &prompt_text, gate, console).await;
            let has_bg = h.wakes_after_prompt_ends || !h.unsent_background_turns.is_empty();
            pid_sink.store(NO_PID, Ordering::SeqCst);
            (res, has_bg)
        };

        let now = Instant::now();
        {
            let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = guard.get_mut(&key) {
                entry.busy = false;
                entry.last_used = now;
                if result.1 {
                    match &mut entry.work_hold {
                        Some(hold) => hold.last_activity = now,
                        None => entry.work_hold = Some(WorkHoldState::new(now)),
                    }
                } else {
                    entry.work_hold = None;
                }
            }
        }

        result.0
    }

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
        mcp_session: Option<&McpSession>,
        stdio_servers: Vec<StdioMcpServerConfig>,
        gate: Option<&PermissionGate<'_>>,
        pid_sink: &AtomicU32,
        console: Option<&ConsoleSink<'_>>,
    ) -> Result<Reply, ClaudeError> {
        let (mut handle, handoff) = open_acp_session(
            program,
            user_args,
            model,
            access,
            provider,
            mcp_session,
            stdio_servers,
            &self.relay_manager,
            None,
            Some(pid_sink),
        )
        .await?;

        let base = if role.trim().is_empty() {
            message.to_string()
        } else {
            format!("{role}\n\n{message}")
        };
        let prompt = if let Some(h) = handoff {
            format!("{h}\n\n{base}")
        } else {
            base
        };

        let res = run_acp_prompt(&mut handle, access, &prompt, gate, console).await;
        handle.terminate();
        pid_sink.store(NO_PID, Ordering::SeqCst);
        res
    }

    pub async fn steer_turn(&self, key: &SessionKey, message: &str) -> Result<bool, ClaudeError> {
        let handle_arc = {
            let guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            guard.get(key).map(|e| Arc::clone(&e.handle))
        };
        let Some(arc) = handle_arc else {
            return Ok(false);
        };
        let Ok(mut handle) = arc.try_lock() else {
            // Se o prompt está em execução, ainda podemos tentar enviar steering se o transporte permitir;
            // no entanto, `arc` fica travado durante `run_acp_prompt`. Retornamos false se ocupado ou
            // usamos canal futuro.
            return Ok(false);
        };
        steer_acp_session(&mut handle, message).await
    }

    pub async fn interrupt_turn(&self, key: &SessionKey) -> Result<bool, ClaudeError> {
        let handle_arc = {
            let guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            guard.get(key).map(|e| Arc::clone(&e.handle))
        };
        let Some(arc) = handle_arc else {
            return Ok(false);
        };
        if let Ok(handle) = arc.try_lock() {
            interrupt_acp_session(&handle).await?;
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn discover_models(
        &self,
        program: &Path,
        user_args: &[String],
        provider: Option<&ProviderConfig>,
    ) -> Result<Vec<String>, ClaudeError> {
        discover_acp_models(program, user_args, provider, &self.relay_manager).await
    }

    pub async fn test_connection(
        &self,
        program: &Path,
        user_args: &[String],
        model: &str,
        config: &ProviderConfig,
        limit: Duration,
    ) -> Result<String, ClaudeError> {
        test_acp_connection(program, user_args, model, config, limit, &self.relay_manager).await
    }

    pub async fn reap_idle(
        &self,
        now: Instant,
        is_externally_busy: impl Fn(&SessionKey) -> bool,
    ) -> Vec<SessionKey> {
        let to_remove: Vec<(SessionKey, SharedAcpSession)> = {
            let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            let expired_keys: Vec<SessionKey> = guard
                .iter()
                .filter_map(|(key, entry)| {
                    if entry.busy || is_externally_busy(key) {
                        return None;
                    }
                    if let Some(hold) = entry.work_hold {
                        if hold.is_expired(now) {
                            return Some(key.clone());
                        }
                        return None;
                    }
                    if now.saturating_duration_since(entry.last_used) >= self.idle_timeout {
                        Some(key.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let mut removed = Vec::with_capacity(expired_keys.len());
            for key in expired_keys {
                if let Some(entry) = guard.remove(&key) {
                    removed.push((key, entry.handle));
                }
            }
            removed
        };

        let mut reaped = Vec::with_capacity(to_remove.len());
        for (key, handle_arc) in to_remove {
            let handle = handle_arc.lock().await;
            handle.terminate();
            reaped.push(key);
        }
        reaped
    }

    pub async fn archive_project(&self, project_id: i64) {
        self.remove_matching(|k, _| k.project_id == project_id).await;
    }

    pub async fn archive_agent(&self, agent: &Agent) {
        self.remove_matching(|k, _| is_agent_or_replica(k.agent.as_str(), agent))
            .await;
    }

    pub async fn archive_worktree(
        &self,
        project_id: i64,
        item_id: &str,
        worktree_path: Option<&Path>,
    ) {
        self.remove_matching(|k, entry| {
            k.project_id == project_id
                && (entry.item_id.as_deref() == Some(item_id)
                    || worktree_path.is_some_and(|wp| entry.cwd == wp))
        })
        .await;
    }

    pub async fn archive_all(&self) {
        self.remove_matching(|_, _| true).await;
    }

    pub fn shutdown_all_sync(&self) {
        let entries: Vec<PooledAcpSession> = {
            let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            guard.drain().map(|(_, v)| v).collect()
        };
        for entry in entries {
            if let Ok(handle) = entry.handle.try_lock() {
                handle.terminate();
            }
        }
        self.relay_manager.shutdown_all();
    }

    async fn remove_matching(
        &self,
        pred: impl Fn(&SessionKey, &PooledAcpSession) -> bool,
    ) {
        let removed: Vec<SharedAcpSession> = {
            let mut guard = self.sessions.lock().unwrap_or_else(PoisonError::into_inner);
            let keys: Vec<SessionKey> = guard
                .iter()
                .filter(|(k, v)| pred(k, v))
                .map(|(k, _)| k.clone())
                .collect();
            keys.into_iter()
                .filter_map(|k| guard.remove(&k).map(|e| e.handle))
                .collect()
        };
        for arc in removed {
            let handle = arc.lock().await;
            handle.terminate();
        }
    }
}

#[cfg(test)]
#[path = "../../../../tests/acp_adapter_tests.rs"]
mod acp_adapter_tests;
