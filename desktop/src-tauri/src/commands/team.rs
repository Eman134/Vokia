use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::agents::{Agent, AgentError, AgentRegistry};
use crate::attachments;
use crate::claude::{self, CliProblem};
use crate::console_log::ConsoleEntry;
use crate::db::{AgentPause, DbState, InterruptedRun, QueuedRequestRecord};
use crate::orchestrator::{self, RequestOutcome};
use crate::permissions::{Answer, PermissionError};
use crate::session_key::SessionKey;
use crate::state::{AppState, RequestContext};

const PERMISSION_REASON_MAX_CHARS: usize = 500;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentConsoleArgs {
    pub project_id: i64,
    pub agent: Agent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SendRequestArgs {
    pub text: String,
    pub to: Agent,
    pub project_id: i64,
    pub run_id: Option<u64>,
    pub item_id: Option<String>,
    pub channel_id: Option<i64>,
    pub attachments: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnswerPermissionArgs {
    pub id: String,
    pub allow: bool,
    pub always: bool,
    #[serde(default)]
    pub root: bool,
    pub reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnswerDecisionArgs {
    pub id: u64,
    pub answer: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InterruptedRunArgs {
    pub id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetAgentPausedArgs {
    pub project_id: i64,
    pub agent: Agent,
    pub paused: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EnqueueQueuedRequestArgs {
    pub project_id: i64,
    pub to: Agent,
    pub text: String,
    pub channel_id: Option<i64>,
    pub item_id: Option<String>,
    pub attachments: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateQueuedRequestArgs {
    pub id: i64,
    pub text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MoveQueuedRequestArgs {
    pub id: i64,
    pub direction: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoveQueuedRequestArgs {
    pub id: i64,
}

pub(crate) fn cli_problem_exec(app: &AppHandle) -> Option<CliProblem> {
    app.state::<AppState>().cli.as_ref().err().cloned()
}

pub(crate) async fn check_connection_exec() -> bool {
    claude::is_reachable().await
}

pub(crate) fn agent_console_exec(app: &AppHandle, args: AgentConsoleArgs) -> Vec<ConsoleEntry> {
    app.state::<AppState>().console.snapshot(&SessionKey {
        project_id: args.project_id,
        agent: args.agent,
    })
}

pub(crate) async fn send_request_exec(
    app: &AppHandle,
    args: SendRequestArgs,
) -> Result<RequestOutcome, String> {
    let state = app.state::<AppState>();
    ensure_agent_exists(&app.state::<AgentRegistry>(), &args.to)?;
    let attachment_ids = args.attachments.unwrap_or_default();
    let text = if attachment_ids.is_empty() {
        args.text
    } else {
        let db = app.state::<DbState>();
        let db = db.require().map_err(|error| error.to_string())?;
        let taken = attachments::find_for_send(db, args.project_id, &attachment_ids)
            .map_err(|error| error.to_string())?;
        attachments::append_block(&args.text, &taken)
    };
    let mut context = RequestContext::new(args.run_id, args.item_id, args.channel_id);
    context.attachments = attachment_ids;
    match orchestrator::handle_request(app, &state, &text, args.to, args.project_id, context).await
    {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            orchestrator::report_failure(app, args.project_id, &error)
                .map_err(|emit_error| emit_error.to_string())?;
            Ok(RequestOutcome::Failed)
        }
    }
}

// Corta por caracteres, não por bytes, para não partir UTF-8.
fn clean_reason(reason: Option<String>) -> Option<String> {
    let text: String = reason?
        .trim()
        .chars()
        .take(PERMISSION_REASON_MAX_CHARS)
        .collect();
    (!text.is_empty()).then_some(text)
}

pub(crate) fn answer_permission_exec(
    app: &AppHandle,
    args: AnswerPermissionArgs,
) -> Result<(), String> {
    let answer = if args.allow {
        Answer::Allow {
            always: args.always,
            root: args.root,
        }
    } else {
        Answer::Deny {
            reason: clean_reason(args.reason),
        }
    };
    match app.state::<AppState>().permissions.answer(&args.id, answer) {
        // A outra superfície já decidiu ou o pedido expirou: nada a mostrar.
        Ok(()) | Err(PermissionError::Unknown) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) async fn answer_decision_exec(
    app: &AppHandle,
    args: AnswerDecisionArgs,
) -> Result<(), String> {
    orchestrator::answer_decision(app, &app.state::<AppState>(), args.id, &args.answer)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) fn list_agent_pauses_exec(app: &AppHandle) -> Result<Vec<AgentPause>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.agent_pauses().map_err(|error| error.to_string())
}

pub(crate) fn list_interrupted_runs_exec(app: &AppHandle) -> Result<Vec<InterruptedRun>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.interrupted_runs().map_err(|error| error.to_string())
}

pub(crate) async fn resume_interrupted_run_exec(
    app: &AppHandle,
    args: InterruptedRunArgs,
) -> Result<RequestOutcome, String> {
    orchestrator::resume_run(app, &app.state::<AppState>(), args.id)
        .await
        .map_err(|error| error.to_string())
}

pub(crate) fn discard_interrupted_run_exec(
    app: &AppHandle,
    args: InterruptedRunArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.discard_interrupted_run(args.id)
        .map_err(|error| error.to_string())
}

pub(crate) fn set_agent_paused_exec(
    app: &AppHandle,
    args: SetAgentPausedArgs,
) -> Result<(), String> {
    ensure_agent_exists(&app.state::<AgentRegistry>(), &args.agent)?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_agent_paused(args.project_id, args.agent, args.paused)
        .map_err(|error| error.to_string())?;
    orchestrator::emit_agent_pauses_changed(app).map_err(|error| error.to_string())?;
    if !args.paused {
        orchestrator::drain_waiting_for_project(app, &app.state::<AppState>(), args.project_id);
    }
    Ok(())
}

pub(crate) fn list_queued_requests_exec(app: &AppHandle) -> Result<Vec<QueuedRequestRecord>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_queued_requests().map_err(|error| error.to_string())
}

pub(crate) fn enqueue_queued_request_exec(
    app: &AppHandle,
    args: EnqueueQueuedRequestArgs,
) -> Result<QueuedRequestRecord, String> {
    ensure_agent_exists(&app.state::<AgentRegistry>(), &args.to)?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let attachments = args.attachments.unwrap_or_default();
    db.enqueue_queued_request(
        args.project_id,
        &args.to,
        &args.text,
        args.channel_id,
        args.item_id.as_deref(),
        &attachments,
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn update_queued_request_exec(
    app: &AppHandle,
    args: UpdateQueuedRequestArgs,
) -> Result<bool, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.update_queued_request(args.id, &args.text)
        .map_err(|error| error.to_string())
}

pub(crate) fn move_queued_request_exec(
    app: &AppHandle,
    args: MoveQueuedRequestArgs,
) -> Result<bool, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.move_queued_request(args.id, &args.direction)
        .map_err(|error| error.to_string())
}

pub(crate) fn remove_queued_request_exec(
    app: &AppHandle,
    args: RemoveQueuedRequestArgs,
) -> Result<bool, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.remove_queued_request(args.id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn cli_problem(app: AppHandle) -> Option<CliProblem> {
    cli_problem_exec(&app)
}

#[tauri::command]
pub async fn check_connection() -> bool {
    check_connection_exec().await
}

#[tauri::command]
pub fn agent_console(app: AppHandle, project_id: i64, agent: Agent) -> Vec<ConsoleEntry> {
    agent_console_exec(&app, AgentConsoleArgs { project_id, agent })
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // argumentos do comando Tauri vêm do front um a um
pub async fn send_request(
    app: AppHandle,
    text: String,
    to: Agent,
    project_id: i64,
    run_id: Option<u64>,
    item_id: Option<String>,
    channel_id: Option<i64>,
    attachments: Option<Vec<String>>,
) -> Result<RequestOutcome, String> {
    send_request_exec(
        &app,
        SendRequestArgs {
            text,
            to,
            project_id,
            run_id,
            item_id,
            channel_id,
            attachments,
        },
    )
    .await
}

#[tauri::command]
pub fn answer_permission(
    app: AppHandle,
    id: String,
    allow: bool,
    always: bool,
    root: Option<bool>,
    reason: Option<String>,
) -> Result<(), String> {
    answer_permission_exec(
        &app,
        AnswerPermissionArgs {
            id,
            allow,
            always,
            root: root.unwrap_or_default(),
            reason,
        },
    )
}

#[tauri::command]
pub async fn answer_decision(app: AppHandle, id: u64, answer: String) -> Result<(), String> {
    answer_decision_exec(&app, AnswerDecisionArgs { id, answer }).await
}

#[tauri::command]
pub fn list_agent_pauses(app: AppHandle) -> Result<Vec<AgentPause>, String> {
    list_agent_pauses_exec(&app)
}

#[tauri::command]
pub fn list_interrupted_runs(app: AppHandle) -> Result<Vec<InterruptedRun>, String> {
    list_interrupted_runs_exec(&app)
}

#[tauri::command]
pub async fn resume_interrupted_run(app: AppHandle, id: i64) -> Result<RequestOutcome, String> {
    resume_interrupted_run_exec(&app, InterruptedRunArgs { id }).await
}

#[tauri::command]
pub fn discard_interrupted_run(app: AppHandle, id: i64) -> Result<(), String> {
    discard_interrupted_run_exec(&app, InterruptedRunArgs { id })
}

#[tauri::command]
pub fn set_agent_paused(
    app: AppHandle,
    project_id: i64,
    agent: Agent,
    paused: bool,
) -> Result<(), String> {
    set_agent_paused_exec(
        &app,
        SetAgentPausedArgs {
            project_id,
            agent,
            paused,
        },
    )
}

#[tauri::command]
pub fn list_queued_requests(app: AppHandle) -> Result<Vec<QueuedRequestRecord>, String> {
    list_queued_requests_exec(&app)
}

#[tauri::command]
pub fn enqueue_queued_request(
    app: AppHandle,
    project_id: i64,
    to: Agent,
    text: String,
    channel_id: Option<i64>,
    item_id: Option<String>,
    attachments: Option<Vec<String>>,
) -> Result<QueuedRequestRecord, String> {
    enqueue_queued_request_exec(
        &app,
        EnqueueQueuedRequestArgs {
            project_id,
            to,
            text,
            channel_id,
            item_id,
            attachments,
        },
    )
}

#[tauri::command]
pub fn update_queued_request(app: AppHandle, id: i64, text: String) -> Result<bool, String> {
    update_queued_request_exec(&app, UpdateQueuedRequestArgs { id, text })
}

#[tauri::command]
pub fn move_queued_request(app: AppHandle, id: i64, direction: String) -> Result<bool, String> {
    move_queued_request_exec(&app, MoveQueuedRequestArgs { id, direction })
}

#[tauri::command]
pub fn remove_queued_request(app: AppHandle, id: i64) -> Result<bool, String> {
    remove_queued_request_exec(&app, RemoveQueuedRequestArgs { id })
}

fn ensure_agent_exists(registry: &AgentRegistry, agent: &Agent) -> Result<(), String> {
    let team = registry.team().map_err(|error| error.to_string())?;
    if team.profile(agent).is_none() {
        return Err(AgentError::UnknownAgent(agent.to_string()).to_string());
    }
    Ok(())
}
