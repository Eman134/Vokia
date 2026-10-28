use std::fmt;
use std::path::Path;
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};

use crate::claude::{self, ClaudeError, LimitWindow};
use crate::events::{Agent, AgentState, Speaker, UsageWindow, VokiaEvent, EVENT_NAME};
use crate::state::AppState;

const PERCENT_SCALE: f64 = 100.0;
const IARA_ROLE: &str = "Você é Iara, tech lead do time. Responda em português, em no máximo 3 frases. Ao receber um cumprimento, apresente-se como tech lead.";
const DAVI_ROLE: &str = "Você é Davi, dev frontend. Responda em português, em no máximo 3 frases.";
const TEAM_ROSTER: &str = "Time desta sessão: Iara (tech lead) e Davi (dev frontend).";
const DELEGATION_RULE: &str = "Quando precisar de outro agente do time, termine a resposta com uma linha exatamente assim: @<Nome>: <pedido>.";
const DELEGATION_MARK: char = '@';
const DELEGATION_SEPARATOR: char = ':';
const REPLY_PREFIX: &str = " respondeu: ";

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
    handle_agent_request(app, state, cli, recipient, request).await
}

async fn handle_agent_request(
    app: &AppHandle,
    state: &AppState,
    cli: &Path,
    agent: Agent,
    request: &str,
) -> Result<(), FlowError> {
    emit_message(app, Speaker::User, Speaker::from(agent), request)?;
    let reply = ask_agent(app, state, cli, agent, request, true).await?;
    let Some((helper, helper_request)) = delegation(&reply, agent) else {
        emit_message(app, Speaker::from(agent), Speaker::User, &reply)?;
        return emit_state(app, agent, AgentState::Idle);
    };
    let visible_reply = without_delegation(&reply, agent);
    if !visible_reply.is_empty() {
        emit_message(app, Speaker::from(agent), Speaker::User, &visible_reply)?;
    }
    emit_state(app, agent, AgentState::Waiting)?;
    emit_message(
        app,
        Speaker::from(agent),
        Speaker::from(helper),
        &helper_request,
    )?;
    let helper_reply = ask_agent(app, state, cli, helper, &helper_request, false).await?;
    emit_state(app, helper, AgentState::Idle)?;
    emit_message(
        app,
        Speaker::from(helper),
        Speaker::from(agent),
        &helper_reply,
    )?;
    let summary_request = format!("{}{REPLY_PREFIX}{helper_reply}", helper.name());
    let summary = ask_agent(app, state, cli, agent, &summary_request, false).await?;
    emit_state(app, agent, AgentState::Idle)?;
    emit_message(app, Speaker::from(agent), Speaker::User, &summary)
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
    can_delegate: bool,
) -> Result<String, FlowError> {
    emit_state(app, agent, AgentState::Thinking)?;
    let session_id = stored_session(state, agent)?;
    let reply = claude::ask(
        cli,
        message,
        &role(agent, can_delegate),
        session_id.as_deref(),
    )
    .await?;
    if let Some(limits) = reply.limits {
        emit(
            app,
            VokiaEvent::PlanUsage {
                five_hour: usage_window(limits.five_hour),
                seven_day: usage_window(limits.seven_day),
            },
        )?;
    }
    state
        .sessions
        .lock()
        .map_err(|_| FlowError::SessionsPoisoned)?
        .insert(agent, reply.session_id);
    Ok(reply.text)
}

fn usage_window(window: LimitWindow) -> UsageWindow {
    UsageWindow {
        percent: (window.utilization * PERCENT_SCALE)
            .round()
            .clamp(0.0, PERCENT_SCALE) as u8,
        resets_at: window.resets_at,
    }
}

fn stored_session(state: &AppState, agent: Agent) -> Result<Option<String>, FlowError> {
    let sessions = state
        .sessions
        .lock()
        .map_err(|_| FlowError::SessionsPoisoned)?;
    Ok(sessions.get(&agent).cloned())
}

fn role(agent: Agent, can_delegate: bool) -> String {
    let base = match agent {
        Agent::Iara => IARA_ROLE,
        Agent::Davi => DAVI_ROLE,
    };
    if can_delegate {
        format!("{base} {TEAM_ROSTER} {DELEGATION_RULE}")
    } else {
        format!("{base} {TEAM_ROSTER}")
    }
}

fn delegation_request(line: &str, from: Agent) -> Option<(Agent, &str)> {
    let rest = line.trim().strip_prefix(DELEGATION_MARK)?;
    let (name, request) = rest.split_once(DELEGATION_SEPARATOR)?;
    let agent = Agent::ALL
        .into_iter()
        .find(|agent| agent.name().eq_ignore_ascii_case(name.trim()))?;
    let request = request.trim();
    (agent != from && !request.is_empty()).then_some((agent, request))
}

fn delegation(reply: &str, from: Agent) -> Option<(Agent, String)> {
    reply
        .lines()
        .rev()
        .find_map(|line| delegation_request(line, from))
        .map(|(agent, request)| (agent, request.to_string()))
}

fn without_delegation(reply: &str, from: Agent) -> String {
    reply
        .lines()
        .filter(|line| delegation_request(line, from).is_none())
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
