use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::claude::{PermissionRequest, PermissionVerdict};
use crate::db::{ApprovalHistory, Db, DbError, DbState};
use crate::events::{Agent, AgentState, PermissionOutcome, VokiaEvent, EVENT_NAME};
use crate::orchestrator::now_millis;

pub const PERMISSION_WAIT_LIMIT: Duration = Duration::from_secs(300);
const COMMAND_FIELD: &str = "command";
const PATH_FIELD: &str = "file_path";
const APPROVAL_SEPARATOR: char = '\u{1f}';
const ALWAYS_SUGGESTION_MIN_APPROVALS: u32 = 3;
const DENIAL_SUGGESTION_WINDOW_MS: u64 = 7 * 24 * 60 * 60 * 1000;

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

#[derive(Clone)]
pub enum Answer {
    Allow { always: bool },
    // O motivo só segue ao agente; nunca vai ao banco, ao console nem a evento.
    Deny { reason: Option<String> },
    // Sessão encerrada pelo app: negado, mas não é uma recusa do usuário.
    Closed,
}

#[derive(Default)]
pub struct PermissionBroker {
    pending: Mutex<HashMap<String, PendingPermission>>,
}

struct PendingPermission {
    project_id: i64,
    sender: oneshot::Sender<Answer>,
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
    pub async fn decide(self, request: PermissionRequest) -> PermissionVerdict {
        if rule_allows(self.app, self.project_id, &request.tool, &target_of(&request)) {
            announce_by_rule(self.app, self.agent.clone(), self.project_id, detail(&request));
            return PermissionVerdict::Allow;
        }
        let approval = approval_key(&request);
        if self.grant_per_session && self.is_granted(&approval) {
            return PermissionVerdict::Allow;
        }
        let verdict = self
            .broker
            .request(self.app, self.agent.clone(), self.project_id, request)
            .await;
        if verdict == PermissionVerdict::Allow && self.grant_per_session {
            self.remember(approval);
        }
        verdict
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
    ) -> PermissionVerdict {
        let (sender, receiver) = oneshot::channel();
        if self.register(&request.id, project_id, sender).is_err() {
            return PermissionVerdict::Deny(None);
        }
        let Ok(now) = now_millis() else {
            self.forget(&request.id);
            return PermissionVerdict::Deny(None);
        };
        let command = detail(&request);
        let target = target_of(&request);
        let db_state = app.state::<DbState>();
        let db = db_state.db();
        // Falha ao ler o histórico não pode travar o pedido.
        let history = db
            .and_then(|db| db.approval_history(project_id, &request.tool, &target).ok())
            .unwrap_or_default();
        let event = VokiaEvent::Permission {
            id: request.id.clone(),
            agent: agent.clone(),
            tool: request.tool.clone(),
            detail: command.clone(),
            project_id: Some(project_id),
            expires_at: now + PERMISSION_WAIT_LIMIT.as_millis() as u64,
            approved_before: history.approved,
            suggest_always: suggests_always(&history, now),
        };
        if app.emit(EVENT_NAME, event).is_err() {
            self.forget(&request.id);
            return PermissionVerdict::Deny(None);
        }
        emit_agent_state(app, &agent, AgentState::Waiting, project_id);
        let (outcome, answer) = match timeout(PERMISSION_WAIT_LIMIT, receiver).await {
            Ok(Ok(answer)) => (answer_outcome(&answer), Some(answer)),
            Ok(Err(_)) => (PermissionOutcome::Denied, None),
            Err(_) => (PermissionOutcome::Expired, None),
        };
        self.forget(&request.id);
        if let Some(db) = db {
            record_answer(app, db, project_id, &request.tool, &target, answer.as_ref());
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
        verdict_of(answer)
    }

    pub fn answer(&self, id: &str, answer: Answer) -> Result<(), PermissionError> {
        let sender = self
            .pending
            .lock()
            .map_err(|_: PoisonError<_>| PermissionError::Poisoned)?
            .remove(id)
            .ok_or(PermissionError::Unknown)?
            .sender;
        // O receptor some quando o tempo esgota; a resposta tardia vira "não existe mais".
        sender.send(answer).map_err(|_| PermissionError::Unknown)
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
                let _ = entry.sender.send(Answer::Closed);
            }
        }
    }

    fn register(
        &self,
        id: &str,
        project_id: i64,
        sender: oneshot::Sender<Answer>,
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

fn answer_outcome(answer: &Answer) -> PermissionOutcome {
    match answer {
        Answer::Allow { .. } => PermissionOutcome::Allowed,
        Answer::Deny { .. } | Answer::Closed => PermissionOutcome::Denied,
    }
}

fn verdict_of(answer: Option<Answer>) -> PermissionVerdict {
    match answer {
        Some(Answer::Allow { .. }) => PermissionVerdict::Allow,
        Some(Answer::Deny { reason }) => PermissionVerdict::Deny(reason),
        Some(Answer::Closed) | None => PermissionVerdict::Deny(None),
    }
}

fn record_answer(
    app: &AppHandle,
    db: &Db,
    project_id: i64,
    tool: &str,
    target: &str,
    answer: Option<&Answer>,
) {
    let recorded = now_millis()
        .map_err(|error| error.to_string())
        .and_then(|time| {
            store_answer(db, project_id, tool, target, time, answer).map_err(|error| error.to_string())
        });
    if let Err(message) = recorded {
        let _ = app.emit(EVENT_NAME, VokiaEvent::Error { message });
    }
}

fn store_answer(
    db: &Db,
    project_id: i64,
    tool: &str,
    target: &str,
    time: u64,
    answer: Option<&Answer>,
) -> Result<(), DbError> {
    match answer {
        Some(Answer::Allow { always }) => {
            db.record_approval(project_id, tool, target, time)?;
            if *always {
                db.save_permission_rule(project_id, tool, target, time)?;
            }
            Ok(())
        }
        Some(Answer::Deny { .. }) => db.record_denial(project_id, tool, target, time),
        Some(Answer::Closed) | None => Ok(()),
    }
}

// Regra do projeto vale só para o projeto em que foi criada: o filtro é por project_id.
fn rule_allows(app: &AppHandle, project_id: i64, tool: &str, target: &str) -> bool {
    let db_state = app.state::<DbState>();
    // Falha ao ler a regra não aprova: o pedido segue para o usuário.
    db_state
        .db()
        .is_some_and(|db| db.has_permission_rule(project_id, tool, target).unwrap_or(false))
}

fn announce_by_rule(app: &AppHandle, agent: Agent, project_id: i64, detail: String) {
    let _ = app.emit(
        EVENT_NAME,
        VokiaEvent::PermissionByRule {
            agent,
            detail,
            project_id: Some(project_id),
        },
    );
}

// Só o que o usuário costuma aceitar e não recusou na janela recente recebe a sugestão.
fn suggests_always(history: &ApprovalHistory, now: u64) -> bool {
    history.approved >= ALWAYS_SUGGESTION_MIN_APPROVALS
        && history
            .last_denied_at
            .is_none_or(|denied| now.saturating_sub(denied) >= DENIAL_SUGGESTION_WINDOW_MS)
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

// Espaços e quebras de linha não mudam o pedido: a contagem e a regra usam a forma colapsada.
fn target_of(request: &PermissionRequest) -> String {
    detail(request).split_whitespace().collect::<Vec<_>>().join(" ")
}

fn detail(request: &PermissionRequest) -> String {
    [COMMAND_FIELD, PATH_FIELD]
        .iter()
        .find_map(|field| request.input.get(field).and_then(|value| value.as_str()))
        .map_or_else(|| request.input.to_string(), str::to_string)
}
