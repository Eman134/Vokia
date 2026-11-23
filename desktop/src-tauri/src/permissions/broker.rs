use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::oneshot;
use tokio::time::timeout;

use crate::claude::{PermissionRequest, PermissionVerdict};
use crate::db::{root_command, ApprovalHistory, Db, DbError, DbState};
use crate::events::{Agent, AgentState, PermissionOutcome, VokiaEvent, EVENT_NAME};
use crate::orchestrator::now_millis;
use crate::projects::worktree::project_worktrees_dir;
use super::bypass::is_destructive;

pub const PERMISSION_WAIT_LIMIT: Duration = Duration::from_secs(300);
const COMMAND_FIELD: &str = "command";
const PATH_FIELD: &str = "file_path";
const APPROVAL_SEPARATOR: char = '\u{1f}';
const ROOT_GRANT_PREFIX: &str = "root:";
const ROOT_RULE_SUFFIX: &str = " *";
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
    Allow { always: bool, root: bool },
    // O motivo só segue ao agente; nunca vai ao banco, ao console nem a evento.
    Deny { reason: Option<String> },
    // Sessão encerrada pelo app: negado, mas não é uma recusa do usuário.
    Closed,
}

#[derive(Default)]
pub struct PermissionBroker {
    pending: Mutex<HashMap<String, PendingPermission>>,
    // Modo bypass por projeto: só em memória, então volta a desligado a cada abertura do app.
    bypass: Mutex<HashSet<i64>>,
}

struct PendingPermission {
    project_id: i64,
    agent: Agent,
    // Ação destrutiva continua pedindo aprovação mesmo quando o bypass é ligado com pedidos pendentes.
    destructive: bool,
    sender: oneshot::Sender<Answer>,
}

#[derive(Clone)]
pub struct PermissionGate<'a> {
    pub app: &'a AppHandle,
    pub broker: &'a PermissionBroker,
    pub agent: Agent,
    pub project_id: i64,
    pub item_id: Option<String>,
    pub granted: &'a Mutex<HashSet<String>>,
    pub grant_per_session: bool,
}

impl PermissionGate<'_> {
    pub async fn decide(self, request: PermissionRequest) -> PermissionVerdict {
        if rule_allows(self.app, self.project_id, &request.tool, &target_of(&request)) {
            announce_by_rule(self.app, self.agent.clone(), self.project_id, detail(&request), false);
            return PermissionVerdict::Allow;
        }
        let destructive = is_destructive(
            &request.input,
            project_root(self.app, self.project_id).as_deref(),
            worktrees_root(self.app, self.project_id).as_deref(),
        );
        // Bypass ligado no projeto: aprova sem balão nem pergunta, mas ação destrutiva segue para o usuário.
        if !destructive && self.broker.bypass_enabled(self.project_id) {
            announce_by_rule(self.app, self.agent.clone(), self.project_id, detail(&request), true);
            return PermissionVerdict::Allow;
        }
        let approval = approval_key(&request);
        let root_grant = root_grant_key(&request);
        if self.grant_per_session
            && (self.is_granted(&approval) || root_grant.as_deref().is_some_and(|key| self.is_granted(key)))
        {
            return PermissionVerdict::Allow;
        }
        let (verdict, root) = self
            .broker
            .request(
                self.app,
                self.agent.clone(),
                self.project_id,
                self.item_id.as_deref(),
                request,
                destructive,
            )
            .await;
        if verdict == PermissionVerdict::Allow && self.grant_per_session {
            match root_grant.filter(|_| root) {
                Some(key) => self.remember(key),
                None => self.remember(approval),
            }
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
        item_id: Option<&str>,
        request: PermissionRequest,
        destructive: bool,
    ) -> (PermissionVerdict, bool) {
        let (sender, receiver) = oneshot::channel();
        if self
            .register(&request.id, project_id, agent.clone(), destructive, sender)
            .is_err()
        {
            return (PermissionVerdict::Deny(None), false);
        }
        let Ok(now) = now_millis() else {
            self.forget(&request.id);
            return (PermissionVerdict::Deny(None), false);
        };
        let command = detail(&request);
        let target = target_of(&request);
        let root = root_of(&request);
        let db_state = app.state::<DbState>();
        let db = db_state.db();
        // Falha ao ler o histórico não pode travar o pedido.
        let history = db
            .and_then(|db| db.approval_history(project_id, &request.tool, &target, root.as_deref()).ok())
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
            root_command: root.clone(),
        };
        if app.emit(EVENT_NAME, event).is_err() {
            self.forget(&request.id);
            return (PermissionVerdict::Deny(None), false);
        }
        emit_agent_state(app, &agent, AgentState::Waiting, project_id, item_id);
        let (outcome, answer) = match timeout(PERMISSION_WAIT_LIMIT, receiver).await {
            Ok(Ok(answer)) => (answer_outcome(&answer), Some(answer)),
            Ok(Err(_)) => (PermissionOutcome::Denied, None),
            Err(_) => (PermissionOutcome::Expired, None),
        };
        self.forget(&request.id);
        if let Some(db) = db {
            record_answer(app, db, project_id, &request.tool, &target, root.as_deref(), answer.as_ref());
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
        let next_state = if matches!(answer, Some(Answer::Closed)) {
            AgentState::Idle
        } else {
            AgentState::Thinking
        };
        emit_agent_state(app, &agent, next_state, project_id, item_id);
        let granted_root = matches!(answer, Some(Answer::Allow { root: true, .. }));
        (verdict_of(answer), granted_root)
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

    pub fn has_pending(&self, project_id: i64, agent: &Agent) -> bool {
        self.pending.lock().is_ok_and(|pending| {
            pending
                .values()
                .any(|entry| entry.project_id == project_id && &entry.agent == agent)
        })
    }

    fn register(
        &self,
        id: &str,
        project_id: i64,
        agent: Agent,
        destructive: bool,
        sender: oneshot::Sender<Answer>,
    ) -> Result<(), PermissionError> {
        self.pending
            .lock()
            .map_err(|_: PoisonError<_>| PermissionError::Poisoned)?
            .insert(
                id.to_string(),
                PendingPermission {
                    project_id,
                    agent,
                    destructive,
                    sender,
                },
            );
        Ok(())
    }

    // Lock envenenado vale como desligado: o pedido volta a ser perguntado.
    pub fn bypass_enabled(&self, project_id: i64) -> bool {
        self.bypass
            .lock()
            .is_ok_and(|projects| projects.contains(&project_id))
    }

    // Ligar resolve como permitidos os pedidos pendentes do projeto que não são destrutivos; desligar não mexe neles.
    pub fn set_bypass(&self, app: &AppHandle, project_id: i64, enabled: bool) -> Result<(), PermissionError> {
        {
            let mut projects = self.bypass.lock().map_err(|_: PoisonError<_>| PermissionError::Poisoned)?;
            if enabled {
                projects.insert(project_id);
            } else {
                projects.remove(&project_id);
            }
        }
        if enabled {
            self.allow_pending(project_id)?;
        }
        let _ = app.emit(EVENT_NAME, VokiaEvent::BypassChanged { project_id, enabled });
        Ok(())
    }

    fn allow_pending(&self, project_id: i64) -> Result<(), PermissionError> {
        let mut pending = self.pending.lock().map_err(|_: PoisonError<_>| PermissionError::Poisoned)?;
        let ids: Vec<String> = pending
            .iter()
            .filter(|(_, entry)| entry.project_id == project_id && !entry.destructive)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(entry) = pending.remove(&id) {
                // Receptor que já sumiu (tempo esgotado): nada a aprovar.
                let _ = entry.sender.send(Answer::Allow { always: false, root: false });
            }
        }
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
    root: Option<&str>,
    answer: Option<&Answer>,
) {
    let recorded = now_millis()
        .map_err(|error| error.to_string())
        .and_then(|time| {
            store_answer(db, project_id, tool, target, root, time, answer).map_err(|error| error.to_string())
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
    root: Option<&str>,
    time: u64,
    answer: Option<&Answer>,
) -> Result<(), DbError> {
    match answer {
        Some(Answer::Allow { always, root: by_root }) => {
            db.record_approval(project_id, tool, target, root, time)?;
            if *always {
                match root.filter(|_| *by_root) {
                    Some(root) => {
                        db.save_permission_rule(project_id, tool, &format!("{root}{ROOT_RULE_SUFFIX}"), time)?
                    }
                    None => db.save_permission_rule(project_id, tool, target, time)?,
                }
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

// Aprovação automática: por regra salva ou pelo modo bypass do projeto (`bypass` marca o segundo caso).
fn announce_by_rule(app: &AppHandle, agent: Agent, project_id: i64, detail: String, bypass: bool) {
    let _ = app.emit(
        EVENT_NAME,
        VokiaEvent::PermissionByRule {
            agent,
            detail,
            project_id: Some(project_id),
            bypass,
        },
    );
}

// Pasta do projeto, usada para saber se o pedido fica dentro dela. Falha de leitura vira None (pergunta).
fn project_root(app: &AppHandle, project_id: i64) -> Option<String> {
    let db_state = app.state::<DbState>();
    db_state
        .db()
        .and_then(|db| db.project(project_id).ok())
        .map(|project| project.path)
}

// Os agentes com card trabalham em worktrees dentro dos dados do app, fora da pasta do projeto. Sem a pasta, não há escopo.
fn worktrees_root(app: &AppHandle, project_id: i64) -> Option<String> {
    let data_dir = app.path().app_data_dir().ok()?;
    Some(project_worktrees_dir(&data_dir, project_id).to_string_lossy().into_owned())
}

// Só o que o usuário costuma aceitar e não recusou na janela recente recebe a sugestão.
fn suggests_always(history: &ApprovalHistory, now: u64) -> bool {
    history.approved >= ALWAYS_SUGGESTION_MIN_APPROVALS
        && history
            .last_denied_at
            .is_none_or(|denied| now.saturating_sub(denied) >= DENIAL_SUGGESTION_WINDOW_MS)
}

fn emit_agent_state(
    app: &AppHandle,
    agent: &Agent,
    state: AgentState,
    project_id: i64,
    item_id: Option<&str>,
) {
    crate::orchestrator::record_agent_state(app, project_id, agent, state);
    let _ = app.emit(
        EVENT_NAME,
        VokiaEvent::AgentState {
            agent: agent.clone(),
            state,
            project_id: Some(project_id),
            item_id: item_id.map(str::to_string),
        },
    );
}

// Só pedidos com campo de comando têm comando raiz; caminho de arquivo não é comando.
fn root_of(request: &PermissionRequest) -> Option<String> {
    request.input.get(COMMAND_FIELD)?;
    root_command(&target_of(request)).map(str::to_string)
}

fn root_grant_key(request: &PermissionRequest) -> Option<String> {
    root_of(request).map(|root| format!("{ROOT_GRANT_PREFIX}{}{APPROVAL_SEPARATOR}{root}", request.tool))
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

#[cfg(test)]
#[path = "../../tests/permission_broker_tests.rs"]
mod tests;
