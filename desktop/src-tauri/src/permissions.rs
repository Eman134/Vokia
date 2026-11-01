use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::claude::PermissionRequest;
use crate::db::DbState;
use crate::events::{Agent, AgentState, PermissionOutcome, VokiaEvent, EVENT_NAME};
use crate::orchestrator::{event_project_id, now_millis};
use crate::state::AppState;

pub const PERMISSION_WAIT_LIMIT: Duration = Duration::from_secs(300);
const COMMAND_FIELD: &str = "command";
const PATH_FIELD: &str = "file_path";

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
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

#[derive(Clone, Copy)]
pub struct PermissionGate<'a> {
    pub app: &'a AppHandle,
    pub broker: &'a PermissionBroker,
    pub agent: Agent,
}

impl PermissionGate<'_> {
    pub async fn decide(self, request: PermissionRequest) -> bool {
        self.broker.request(self.app, self.agent, request).await
    }
}

impl PermissionBroker {
    // Sem resposta no limite ou com falha ao avisar a tela, o resultado é negar.
    async fn request(&self, app: &AppHandle, agent: Agent, request: PermissionRequest) -> bool {
        let (sender, receiver) = oneshot::channel();
        if self.register(&request.id, sender).is_err() {
            return false;
        }
        let Ok(project_id) = event_project_id(&app.state::<AppState>()) else {
            self.forget(&request.id);
            return false;
        };
        let Ok(now) = now_millis() else {
            self.forget(&request.id);
            return false;
        };
        let command = detail(&request);
        let db_state = app.state::<DbState>();
        let db = db_state.db();
        // Falha ao ler o contador não pode travar o pedido.
        let approved_before = project_id
            .zip(db)
            .and_then(|(id, db)| db.approved_count(id, &request.tool, &command).ok())
            .unwrap_or(0);
        let event = VokiaEvent::Permission {
            id: request.id.clone(),
            agent,
            tool: request.tool.clone(),
            detail: command.clone(),
            project_id,
            expires_at: now + PERMISSION_WAIT_LIMIT.as_millis() as u64,
            approved_before,
        };
        if app.emit(EVENT_NAME, event).is_err() {
            self.forget(&request.id);
            return false;
        }
        emit_agent_state(app, agent, AgentState::Waiting, project_id);
        let outcome = match timeout(PERMISSION_WAIT_LIMIT, receiver).await {
            Ok(Ok(true)) => PermissionOutcome::Allowed,
            Ok(Ok(false)) | Ok(Err(_)) => PermissionOutcome::Denied,
            Err(_) => PermissionOutcome::Expired,
        };
        self.forget(&request.id);
        if outcome == PermissionOutcome::Allowed {
            if let Some((id, db)) = project_id.zip(db) {
                let recorded = now_millis()
                    .map_err(|error| error.to_string())
                    .and_then(|time| {
                        db.record_approval(id, &request.tool, &command, time)
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
                project_id,
                outcome,
            },
        );
        emit_agent_state(app, agent, AgentState::Thinking, project_id);
        outcome == PermissionOutcome::Allowed
    }

    pub fn answer(&self, id: &str, allow: bool) -> Result<(), PermissionError> {
        let sender = self
            .pending
            .lock()
            .map_err(|_: PoisonError<_>| PermissionError::Poisoned)?
            .remove(id)
            .ok_or(PermissionError::Unknown)?;
        // O receptor some quando o tempo esgota; a resposta tardia vira "não existe mais".
        sender.send(allow).map_err(|_| PermissionError::Unknown)
    }

    fn register(&self, id: &str, sender: oneshot::Sender<bool>) -> Result<(), PermissionError> {
        self.pending
            .lock()
            .map_err(|_: PoisonError<_>| PermissionError::Poisoned)?
            .insert(id.to_string(), sender);
        Ok(())
    }

    fn forget(&self, id: &str) {
        if let Ok(mut pending) = self.pending.lock() {
            pending.remove(id);
        }
    }
}

fn emit_agent_state(app: &AppHandle, agent: Agent, state: AgentState, project_id: Option<i64>) {
    let _ = app.emit(
        EVENT_NAME,
        VokiaEvent::AgentState {
            agent,
            state,
            project_id,
        },
    );
}

fn detail(request: &PermissionRequest) -> String {
    [COMMAND_FIELD, PATH_FIELD]
        .iter()
        .find_map(|field| request.input.get(field).and_then(|value| value.as_str()))
        .map_or_else(|| request.input.to_string(), str::to_string)
}
