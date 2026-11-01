use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::claude::PermissionRequest;
use crate::events::{Agent, VokiaEvent, EVENT_NAME};
use crate::orchestrator::event_project_id;
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
        let event = VokiaEvent::Permission {
            id: request.id.clone(),
            agent,
            tool: request.tool.clone(),
            detail: detail(&request),
            project_id,
        };
        if app.emit(EVENT_NAME, event).is_err() {
            self.forget(&request.id);
            return false;
        }
        let allowed = matches!(timeout(PERMISSION_WAIT_LIMIT, receiver).await, Ok(Ok(true)));
        self.forget(&request.id);
        // Sem este aviso o balão ficaria aberto depois de negado por tempo.
        let _ = app.emit(EVENT_NAME, VokiaEvent::PermissionClosed { id: request.id });
        allowed
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

fn detail(request: &PermissionRequest) -> String {
    [COMMAND_FIELD, PATH_FIELD]
        .iter()
        .find_map(|field| request.input.get(field).and_then(|value| value.as_str()))
        .map_or_else(|| request.input.to_string(), str::to_string)
}
