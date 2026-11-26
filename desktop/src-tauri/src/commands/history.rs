use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::db::history::{
    HistoryReadQuery, HistoryReadResponse, HistorySearchQuery, HistorySearchResponse,
    HistoryWorkStepEntry,
};
use crate::db::DbState;

const DEFAULT_WORK_STEPS_LIMIT: usize = 10;
const MAX_WORK_STEPS_LIMIT: usize = 50;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistorySearchArgs {
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub channel_id: Option<i64>,
    #[serde(default, alias = "itemId")]
    pub card_id: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryReadArgs {
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub message_id: Option<i64>,
    #[serde(default)]
    pub before_message_id: Option<i64>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub channel_id: Option<i64>,
    #[serde(default, alias = "itemId")]
    pub card_id: Option<String>,
    #[serde(default)]
    pub include_work_steps: bool,
    #[serde(default)]
    pub work_steps_offset: Option<usize>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListTurnWorkStepsArgs {
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default, alias = "itemId")]
    pub card_id: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
    #[serde(default)]
    pub offset: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListTurnWorkStepsResponse {
    pub project_id: i64,
    pub work_steps: Vec<HistoryWorkStepEntry>,
    pub has_more: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_offset: Option<usize>,
}

fn resolve_scoped_project_id(
    requested: Option<i64>,
    bound_project_id: Option<i64>,
) -> Result<i64, String> {
    match (requested, bound_project_id) {
        (Some(req), Some(bound)) if req != bound => Err(format!(
            "Acesso negado: o projeto {req} não pertence ao projeto atual ({bound})."
        )),
        (Some(req), _) => Ok(req),
        (None, Some(bound)) => Ok(bound),
        (None, None) => Err("Informe projectId para consultar o histórico.".to_string()),
    }
}

fn resolve_before_message_id(
    before_message_id: Option<i64>,
    cursor: Option<&str>,
) -> Result<Option<i64>, String> {
    if before_message_id.is_some() {
        return Ok(before_message_id);
    }
    let Some(raw) = cursor.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    raw.parse::<i64>()
        .map(Some)
        .map_err(|_| format!("Cursor de histórico inválido: {raw}"))
}

pub(crate) fn history_search_exec(
    app: &AppHandle,
    bound_project_id: Option<i64>,
    args: HistorySearchArgs,
) -> Result<HistorySearchResponse, String> {
    let project_id = resolve_scoped_project_id(args.project_id, bound_project_id)?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.history_search(&HistorySearchQuery {
        project_id,
        query: args.query,
        agent: args.agent,
        channel_id: args.channel_id,
        card_id: args.card_id,
        limit: args.limit,
    })
    .map_err(|error| error.to_string())
}

pub(crate) fn history_read_exec(
    app: &AppHandle,
    bound_project_id: Option<i64>,
    args: HistoryReadArgs,
) -> Result<HistoryReadResponse, String> {
    let project_id = resolve_scoped_project_id(args.project_id, bound_project_id)?;
    let before_message_id =
        resolve_before_message_id(args.before_message_id, args.cursor.as_deref())?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.history_read(&HistoryReadQuery {
        project_id,
        message_id: args.message_id,
        before_message_id,
        limit: args.limit,
        offset: args.offset.unwrap_or(0),
        agent: args.agent,
        channel_id: args.channel_id,
        card_id: args.card_id,
        include_work_steps: args.include_work_steps,
        work_steps_offset: args.work_steps_offset.unwrap_or(0),
    })
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn history_search(
    app: AppHandle,
    project_id: i64,
    query: String,
    agent: Option<String>,
    channel_id: Option<i64>,
    card_id: Option<String>,
    limit: Option<usize>,
) -> Result<HistorySearchResponse, String> {
    history_search_exec(
        &app,
        None,
        HistorySearchArgs {
            project_id: Some(project_id),
            query,
            agent,
            channel_id,
            card_id,
            limit,
        },
    )
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn history_read(
    app: AppHandle,
    project_id: i64,
    message_id: Option<i64>,
    before_message_id: Option<i64>,
    cursor: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    agent: Option<String>,
    channel_id: Option<i64>,
    card_id: Option<String>,
    include_work_steps: Option<bool>,
    work_steps_offset: Option<usize>,
) -> Result<HistoryReadResponse, String> {
    history_read_exec(
        &app,
        None,
        HistoryReadArgs {
            project_id: Some(project_id),
            message_id,
            before_message_id,
            cursor,
            limit,
            offset,
            agent,
            channel_id,
            card_id,
            include_work_steps: include_work_steps.unwrap_or(false),
            work_steps_offset,
        },
    )
}

pub(crate) fn list_turn_work_steps_exec(
    app: &AppHandle,
    bound_project_id: Option<i64>,
    args: ListTurnWorkStepsArgs,
) -> Result<ListTurnWorkStepsResponse, String> {
    let project_id = resolve_scoped_project_id(args.project_id, bound_project_id)?;
    let limit = args
        .limit
        .unwrap_or(DEFAULT_WORK_STEPS_LIMIT)
        .clamp(1, MAX_WORK_STEPS_LIMIT);
    let offset = args.offset.unwrap_or(0);
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let (records, has_more) = db
        .list_history_work_steps(
            project_id,
            args.agent.as_deref(),
            args.card_id.as_deref(),
            limit,
            offset,
        )
        .map_err(|error| error.to_string())?;
    let work_steps: Vec<HistoryWorkStepEntry> = records.into_iter().map(Into::into).collect();
    let next_offset = has_more.then_some(offset + work_steps.len());
    Ok(ListTurnWorkStepsResponse {
        project_id,
        work_steps,
        has_more,
        next_offset,
    })
}

#[tauri::command]
pub fn list_turn_work_steps(
    app: AppHandle,
    project_id: i64,
    agent: Option<String>,
    card_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<ListTurnWorkStepsResponse, String> {
    list_turn_work_steps_exec(
        &app,
        None,
        ListTurnWorkStepsArgs {
            project_id: Some(project_id),
            agent,
            card_id,
            limit,
            offset,
        },
    )
}
