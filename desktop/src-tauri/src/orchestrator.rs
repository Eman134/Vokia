use std::fmt;
use std::path::Path;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};

use crate::claude::{self, ClaudeError};
use crate::events::{Agent, AgentState, Speaker, VokiaEvent, EVENT_NAME};
use crate::state::AppState;

const IARA_ROLE: &str = "Você é Iara, tech lead do time. Responda em português, em no máximo 3 frases. Quando precisar do Davi, termine a resposta com uma linha exatamente assim: @Davi: <pedido>.";
const DAVI_ROLE: &str = "Você é Davi, dev frontend. Responda em português, em no máximo 3 frases.";
const DELEGATION_PREFIX: &str = "@Davi:";
const DAVI_REPLY_PREFIX: &str = "O Davi respondeu: ";

#[derive(Debug)]
pub enum FlowError {
    CliUnavailable(String),
    Claude(ClaudeError),
    Emit(tauri::Error),
    SessionsPoisoned,
    Clock(SystemTimeError),
}

impl fmt::Display for FlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowError::CliUnavailable(reason) => write!(f, "{reason}"),
            FlowError::Claude(error) => write!(f, "{error}"),
            FlowError::Emit(error) => write!(f, "Não foi possível emitir o evento: {error}"),
            FlowError::SessionsPoisoned => write!(
                f,
                "O registro de sessões ficou inconsistente após uma falha anterior."
            ),
            FlowError::Clock(error) => {
                write!(f, "O relógio do sistema está antes de 1970: {error}")
            }
        }
    }
}

impl From<ClaudeError> for FlowError {
    fn from(error: ClaudeError) -> Self {
        FlowError::Claude(error)
    }
}

impl From<tauri::Error> for FlowError {
    fn from(error: tauri::Error) -> Self {
        FlowError::Emit(error)
    }
}

pub async fn handle_request(
    app: &AppHandle,
    state: &AppState,
    request: &str,
    recipient: Agent,
) -> Result<(), FlowError> {
    let cli = state
        .cli
        .as_ref()
        .map_err(|problem| FlowError::CliUnavailable(problem.reason.clone()))?;
    match recipient {
        Agent::Iara => handle_iara_request(app, state, cli, request).await,
        Agent::Davi => handle_davi_request(app, state, cli, request).await,
    }
}

async fn handle_davi_request(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    request: &str,
) -> Result<(), FlowError> {
    emit_message(app, Speaker::User, Speaker::Davi, request)?;
    let davi_reply = ask_agent(app, state, cli, Agent::Davi, request).await?;
    emit_state(app, Agent::Davi, AgentState::Idle)?;
    emit_message(app, Speaker::Davi, Speaker::User, &davi_reply)
}

async fn handle_iara_request(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    request: &str,
) -> Result<(), FlowError> {
    emit_message(app, Speaker::User, Speaker::Iara, request)?;
    let iara_reply = ask_agent(app, state, cli, Agent::Iara, request).await?;
    let Some(davi_request) = delegation(&iara_reply) else {
        emit_message(app, Speaker::Iara, Speaker::User, &iara_reply)?;
        return emit_state(app, Agent::Iara, AgentState::Idle);
    };
    let visible_reply = without_delegation(&iara_reply);
    if !visible_reply.is_empty() {
        emit_message(app, Speaker::Iara, Speaker::User, &visible_reply)?;
    }
    emit_state(app, Agent::Iara, AgentState::Waiting)?;
    emit_message(app, Speaker::Iara, Speaker::Davi, &davi_request)?;
    let davi_reply = ask_agent(app, state, cli, Agent::Davi, &davi_request).await?;
    emit_state(app, Agent::Davi, AgentState::Idle)?;
    emit_message(app, Speaker::Davi, Speaker::Iara, &davi_reply)?;
    let summary_request = format!("{DAVI_REPLY_PREFIX}{davi_reply}");
    let summary = ask_agent(app, state, cli, Agent::Iara, &summary_request).await?;
    emit_state(app, Agent::Iara, AgentState::Idle)?;
    emit_message(app, Speaker::Iara, Speaker::User, &summary)
}

pub fn report_failure(app: &AppHandle, error: &FlowError) -> Result<(), FlowError> {
    emit(
        app,
        VokiaEvent::Error {
            message: error.to_string(),
        },
    )?;
    emit_state(app, Agent::Iara, AgentState::Idle)?;
    emit_state(app, Agent::Davi, AgentState::Idle)
}

async fn ask_agent(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    agent: Agent,
    message: &str,
) -> Result<String, FlowError> {
    emit_state(app, agent, AgentState::Thinking)?;
    let session_id = stored_session(state, agent)?;
    let reply = claude::ask(cli, message, role(agent), session_id.as_deref()).await?;
    state
        .sessions
        .lock()
        .map_err(|_| FlowError::SessionsPoisoned)?
        .insert(agent, reply.session_id);
    Ok(reply.text)
}

fn stored_session(state: &AppState, agent: Agent) -> Result<Option<String>, FlowError> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| FlowError::SessionsPoisoned)?;
    Ok(sessions.get(&agent).cloned())
}

fn role(agent: Agent) -> &'static str {
    match agent {
        Agent::Iara => IARA_ROLE,
        Agent::Davi => DAVI_ROLE,
    }
}

fn delegation_request(line: &str) -> Option<&str> {
    line.trim().strip_prefix(DELEGATION_PREFIX)
}

fn delegation(reply: &str) -> Option<String> {
    reply
        .lines()
        .rev()
        .filter_map(delegation_request)
        .map(str::trim)
        .find(|request| !request.is_empty())
        .map(str::to_string)
}

fn without_delegation(reply: &str) -> String {
    reply
        .lines()
        .filter(|line| delegation_request(line).is_none())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn emit_message(app: &AppHandle, from: Speaker, to: Speaker, text: &str) -> Result<(), FlowError> {
    emit(
        app,
        VokiaEvent::Message {
            from,
            to,
            text: text.to_string(),
            time: now_millis()?,
        },
    )
}

fn emit_state(app: &AppHandle, agent: Agent, state: AgentState) -> Result<(), FlowError> {
    emit(app, VokiaEvent::AgentState { agent, state })
}

fn emit(app: &AppHandle, event: VokiaEvent) -> Result<(), FlowError> {
    Ok(app.emit(EVENT_NAME, event)?)
}

fn now_millis() -> Result<u64, FlowError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(FlowError::Clock)?;
    Ok(elapsed.as_millis() as u64)
}
