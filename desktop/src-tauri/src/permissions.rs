use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::claude::PermissionRequest;
use crate::db::DbState;
use crate::events::{Agent, AgentState, PermissionOutcome, VokiaEvent, EVENT_NAME};
use crate::orchestrator::now_millis;

pub const PERMISSION_WAIT_LIMIT: Duration = Duration::from_secs(300);
const COMMAND_FIELD: &str = "command";
const PATH_FIELD: &str = "file_path";
const APPROVAL_SEPARATOR: char = '\u{1f}';

#[derive(Debug)]
pub enum PermissionError {
    Unknown,
    Poisoned,
}

impl fmt::Display for PermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PermissionError::Unknown => write!(f, "Este pedido de permissão não existe mais."),
            PermissionError::Poisoned => write!(f, "O controle de permissões ficou inconsistente."),
        }
    }
}

#[derive(Default)]
pub struct PermissionBroker {
    pending: Mutex<HashMap<String, PendingPermission>>,
}

struct PendingPermission {
    project_id: i64,
    sender: oneshot::Sender<bool>,
}

#[derive(Clone)]
pub struct PermissionGate<'a> {
    pub app: &'a AppHandle,
    pub broker: &'a PermissionBroker,
    pub agent: Agent,
    pub project_id: i64,
    pub granted: &'a Mutex<HashSet<String>>,
    pub grant_per_session: bool,
}

impl PermissionGate<'_> {
    pub async fn decide(self, request: PermissionRequest) -> bool {
        let approval = approval_key(&request);
        if self.grant_per_session && self.is_granted(&approval) {
            return true;
        }
        let allowed = self
            .broker
            .request(self.app, self.agent.clone(), self.project_id, request)
            .await;
        if allowed && self.grant_per_session {
            self.remember(approval);
        }
        allowed
    }

    // Lock envenenado vale como "não aprovado": o comando volta a ser perguntado.
    fn is_granted(&self, approval: &str) -> bool {
        self.granted
            .lock()
            .is_ok_and(|granted| granted.contains(approval))
    }

    fn remember(&self, approval: String) {
        if let Ok(mut granted) = self.granted.lock() {
            granted.insert(approval);
        }
    }
}

impl PermissionBroker {
    // Sem resposta no limite ou com falha ao avisar a tela, o resultado é negar.
    async fn request(
        &self,
        app: &AppHandle,
        agent: Agent,
        project_id: i64,
        request: PermissionRequest,
    ) -> bool {
        let (sender, receiver) = oneshot::channel();
        if self.register(&request.id, project_id, sender).is_err() {
            return false;
        }
        let Ok(now) = now_millis() else {
            self.forget(&request.id);
            return false;
        };
        let command = detail(&request);
        let db_state = app.state::<DbState>();
        let db = db_state.db();
        // Falha ao ler o contador não pode travar o pedido.
        let approved_before = db
            .and_then(|db| db.approved_count(project_id, &request.tool, &command).ok())
            .unwrap_or(0);
        let event = VokiaEvent::Permission {
            id: request.id.clone(),
            agent: agent.clone(),
            tool: request.tool.clone(),
            detail: command.clone(),
            project_id: Some(project_id),
            expires_at: now + PERMISSION_WAIT_LIMIT.as_millis() as u64,
            approved_before,
        };
        if app.emit(EVENT_NAME, event).is_err() {
            self.forget(&request.id);
            return false;
        }
        emit_agent_state(app, &agent, AgentState::Waiting, project_id);
        let outcome = match timeout(PERMISSION_WAIT_LIMIT, receiver).await {
            Ok(Ok(true)) => PermissionOutcome::Allowed,
            Ok(Ok(false)) | Ok(Err(_)) => PermissionOutcome::Denied,
            Err(_) => PermissionOutcome::Expired,
        };
        self.forget(&request.id);
        if outcome == PermissionOutcome::Allowed {
            if let Some(db) = db {
                let recorded = now_millis()
                    .map_err(|error| error.to_string())
                    .and_then(|time| {
                        db.record_approval(project_id, &request.tool, &command, time)
                            .map_err(|error| error.to_string())
                    });
                if let Err(message) = recorded {
                    let _ = app.emit(EVENT_NAME, VokiaEvent::Error { message });
                }
            }
        }
        // Sem este aviso o balão ficaria aberto depois de negado por tempo.
        let _ = app.emit(
            EVENT_NAME,
            VokiaEvent::PermissionClosed {
                id: request.id,
                project_id: Some(project_id),
                outcome,
            },
        );
        emit_agent_state(app, &agent, AgentState::Thinking, project_id);
        outcome == PermissionOutcome::Allowed
    }

    pub fn answer(&self, id: &str, allow: bool) -> Result<(), PermissionError> {
        let sender = self
            .pending
            .lock()
            .map_err(|_: PoisonError<_>| PermissionError::Poisoned)?
            .remove(id)
            .ok_or(PermissionError::Unknown)?
            .sender;
        // O receptor some quando o tempo esgota; a resposta tardia vira "não existe mais".
        sender.send(allow).map_err(|_| PermissionError::Unknown)
    }

    // Fechar as sessões de um projeto encerra na hora o que elas ainda perguntam ao usuário: negado, sem esperar o limite.
    pub fn deny_project(&self, project_id: i64) {
        let Ok(mut pending) = self.pending.lock() else {
            return;
        };
        let ids: Vec<String> = pending
            .iter()
            .filter(|(_, entry)| entry.project_id == project_id)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(entry) = pending.remove(&id) {
                // O receptor some quando o tempo já esgotou; nesse caso não há o que negar.
                let _ = entry.sender.send(false);
            }
        }
    }

    fn register(
        &self,
        id: &str,
        project_id: i64,
        sender: oneshot::Sender<bool>,
    ) -> Result<(), PermissionError> {
        self.pending
            .lock()
            .map_err(|_: PoisonError<_>| PermissionError::Poisoned)?
            .insert(id.to_string(), PendingPermission { project_id, sender });
        Ok(())
    }

    fn forget(&self, id: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(id);
        }
    }
}

fn emit_agent_state(app: &AppHandle, agent: &Agent, state: AgentState, project_id: i64) {
    let _ = app.emit(
        EVENT_NAME,
        VokiaEvent::AgentState {
            agent: agent.clone(),
            state,
            project_id: Some(project_id),
        },
    );
}

fn approval_key(request: &PermissionRequest) -> String {
    format!("{}{APPROVAL_SEPARATOR}{}", request.tool, detail(request))
}

fn detail(request: &PermissionRequest) -> String {
    [COMMAND_FIELD, PATH_FIELD]
        .iter()
        .find_map(|field| request.input.get(field).and_then(|value| value.as_str()))
        .map_or_else(|| request.input.to_string(), str::to_string)
}
