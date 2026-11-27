use serde::Deserialize;
use tauri::{AppHandle, Manager, State};

use crate::agents::{Agent, AgentError, AgentRegistry};
use crate::db::{
    AgentActor, BacklogItem, BacklogSnapshot, CardOperation, CardResult, DbState, GitRefLinkInput,
    ItemPlanInput, Link, NewCard,
};
use crate::orchestrator::{self, emit_backlog_changed};
use crate::projects::{self, GitRefs};
use crate::state::AppState;

const MISSING_RESULT: &str = "O banco não devolveu o card criado.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectIdArgs {
    pub project_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveBacklogItemArgs {
    pub project_id: i64,
    pub item: BacklogItem,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateBacklogItemArgs {
    pub project_id: i64,
    pub request: NewCard,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateRequestFromProposalArgs {
    pub project_id: i64,
    pub agent: Agent,
    pub title: String,
    pub detail: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MoveBacklogItemArgs {
    pub project_id: i64,
    pub id: String,
    pub column: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReorderBacklogColumnArgs {
    pub project_id: i64,
    pub column: String,
    pub item_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetItemPriorityArgs {
    pub project_id: i64,
    pub item_id: String,
    pub priority: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BacklogLabelArgs {
    pub project_id: i64,
    pub item_id: String,
    pub label: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetBacklogResponsibleArgs {
    pub project_id: i64,
    pub item_id: String,
    pub agent: Agent,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BacklogItemArgs {
    pub project_id: i64,
    pub item_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SaveItemPlanArgs {
    pub project_id: i64,
    pub item_id: String,
    pub plan: ItemPlanInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReturnTaskArgs {
    pub project_id: i64,
    pub item_id: String,
    pub motive: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LinkGitRefArgs {
    pub project_id: i64,
    pub item_id: String,
    pub kind: String,
    pub ref_id: String,
    pub label: Option<String>,
    pub url: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UnlinkGitRefArgs {
    pub project_id: i64,
    pub item_id: String,
    pub link_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListGitRefsArgs {
    pub project_id: i64,
    pub limit: Option<usize>,
}

pub(crate) fn list_backlog_exec(
    app: &AppHandle,
    args: ProjectIdArgs,
) -> Result<BacklogSnapshot, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_backlog(args.project_id)
        .map_err(|error| error.to_string())
}

pub(crate) fn save_backlog_item_exec(
    app: &AppHandle,
    args: SaveBacklogItemArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.save_backlog_item(args.project_id, &args.item)
        .map_err(|error| error.to_string())
}

pub(crate) fn create_backlog_item_exec(
    app: &AppHandle,
    args: CreateBacklogItemArgs,
) -> Result<BacklogItem, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.create_card(args.project_id, &args.request)
        .map_err(|error| error.to_string())
}

pub(crate) fn create_request_from_proposal_exec(
    app: &AppHandle,
    args: CreateRequestFromProposalArgs,
) -> Result<CardResult, String> {
    create_request_from_proposal_impl(app, &app.state::<DbState>(), args)
}

// Mesmo caminho do `create_card` do MCP: origem agente, título repetido e acima de 200 recusados pelo banco.
fn create_request_from_proposal_impl(
    app: &AppHandle,
    state: &DbState,
    args: CreateRequestFromProposalArgs,
) -> Result<CardResult, String> {
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    if team.profile(&args.agent).is_none() {
        return Err(AgentError::UnknownAgent(args.agent.to_string()).to_string());
    }
    let actor = AgentActor {
        name: team.name_of(&args.agent).to_string(),
        id: args.agent,
        current_card: None,
    };
    let db = state.require().map_err(|error| error.to_string())?;
    let operation = CardOperation::Create {
        title: args.title,
        description: args.detail,
        responsible: None,
    };
    db.apply_card_operations(args.project_id, &actor, &[operation])
        .map_err(|error| error.to_string())?
        .pop()
        .ok_or_else(|| MISSING_RESULT.to_string())
}

pub(crate) fn move_backlog_item_exec(
    app: &AppHandle,
    args: MoveBacklogItemArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.move_backlog_item(args.project_id, &args.id, &args.column)
        .map_err(|error| error.to_string())?;
    if args.column.eq_ignore_ascii_case("done") {
        orchestrator::schedule_done_cleanup(app, args.project_id, &args.id);
    }
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn reorder_backlog_column_exec(
    app: &AppHandle,
    args: ReorderBacklogColumnArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.reorder_column_items(args.project_id, &args.column, &args.item_ids)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn set_item_priority_exec(
    app: &AppHandle,
    args: SetItemPriorityArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_item_priority(args.project_id, &args.item_id, &args.priority)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn add_backlog_label_exec(
    app: &AppHandle,
    args: BacklogLabelArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.add_backlog_label(args.project_id, &args.item_id, &args.label)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn remove_backlog_label_exec(
    app: &AppHandle,
    args: BacklogLabelArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.remove_backlog_label(args.project_id, &args.item_id, &args.label)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn link_git_ref_exec(
    app: &AppHandle,
    args: LinkGitRefArgs,
) -> Result<Link, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let input = GitRefLinkInput {
        kind: args.kind,
        ref_id: args.ref_id,
        label: args.label,
        url: args.url,
    };
    let link = db
        .link_git_ref(args.project_id, &args.item_id, &input)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(link)
}

pub(crate) fn unlink_git_ref_exec(
    app: &AppHandle,
    args: UnlinkGitRefArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.unlink_git_ref(args.project_id, &args.item_id, &args.link_id)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) async fn list_git_refs_exec(
    app: &AppHandle,
    args: ListGitRefsArgs,
) -> Result<GitRefs, String> {
    let project_path = {
        let state = app.state::<DbState>();
        let db = state.require().map_err(|error| error.to_string())?;
        db.project(args.project_id)
            .map_err(|error| error.to_string())?
            .path
    };
    projects::list_git_refs(&project_path, args.limit)
        .await
        .map_err(|error| error.message().to_string())
}

pub(crate) fn set_backlog_responsible_exec(
    app: &AppHandle,
    args: SetBacklogResponsibleArgs,
    requester: Option<&Agent>,
) -> Result<(), String> {
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_backlog_responsible(args.project_id, &args.item_id, &args.agent, &team, requester)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn archive_backlog_item_exec(
    app: &AppHandle,
    args: BacklogItemArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.archive_backlog_item(args.project_id, &args.item_id)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn restore_backlog_item_exec(
    app: &AppHandle,
    args: BacklogItemArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.restore_backlog_item(args.project_id, &args.item_id)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

pub(crate) fn delete_backlog_item_exec(
    app: &AppHandle,
    args: BacklogItemArgs,
) -> Result<(), String> {
    delete_backlog_item_with_cascade_exec(app, args, false)
}

pub(crate) fn delete_backlog_item_with_cascade_exec(
    app: &AppHandle,
    args: BacklogItemArgs,
    cascade: bool,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_backlog_item_with_cascade(args.project_id, &args.item_id, cascade)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(app, args.project_id);
    Ok(())
}

// A gravação já está no banco; falha ao avisar o front não vira erro para quem chamou.
pub(crate) fn notify_backlog_changed(app: &AppHandle, project_id: i64) {
    let _ = emit_backlog_changed(app, project_id);
}

pub(crate) fn save_item_plan_exec(app: &AppHandle, args: SaveItemPlanArgs) -> Result<(), String> {
    let approved = {
        let state = app.state::<DbState>();
        let db = state.require().map_err(|error| error.to_string())?;
        db.save_item_plan(args.project_id, &args.item_id, &args.plan)
            .map_err(|error| error.to_string())?
    };
    notify_backlog_changed(app, args.project_id);
    if approved {
        orchestrator::start_approved_plan(app, args.project_id, &args.item_id)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

// Devolve a task concluída ao líder: o motivo é obrigatório e o card volta a In progress.
pub(crate) async fn return_task_exec(app: &AppHandle, args: ReturnTaskArgs) -> Result<(), String> {
    orchestrator::return_task(
        app,
        &app.state::<AppState>(),
        args.project_id,
        &args.item_id,
        &args.motive,
    )
    .await
    .map_err(|error| error.to_string())
}

// Interrompe qualquer execução ou fila em andamento de um card e o devolve para Ready.
pub(crate) async fn stop_task_run_exec(
    app: &AppHandle,
    args: BacklogItemArgs,
) -> Result<bool, String> {
    orchestrator::stop_task_flow(app, &app.state::<AppState>(), args.project_id, &args.item_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_backlog(app: AppHandle, project_id: i64) -> Result<BacklogSnapshot, String> {
    list_backlog_exec(&app, ProjectIdArgs { project_id })
}

#[tauri::command]
pub fn save_backlog_item(app: AppHandle, project_id: i64, item: BacklogItem) -> Result<(), String> {
    save_backlog_item_exec(&app, SaveBacklogItemArgs { project_id, item })
}

#[tauri::command]
pub fn create_backlog_item(
    app: AppHandle,
    project_id: i64,
    request: NewCard,
) -> Result<BacklogItem, String> {
    create_backlog_item_exec(
        &app,
        CreateBacklogItemArgs {
            project_id,
            request,
        },
    )
}

#[tauri::command]
pub fn create_request_from_proposal(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    agent: Agent,
    title: String,
    detail: Option<String>,
) -> Result<CardResult, String> {
    create_request_from_proposal_impl(
        &app,
        &state,
        CreateRequestFromProposalArgs {
            project_id,
            agent,
            title,
            detail,
        },
    )
}

#[tauri::command]
pub fn move_backlog_item(
    app: AppHandle,
    project_id: i64,
    id: String,
    column: String,
) -> Result<(), String> {
    move_backlog_item_exec(
        &app,
        MoveBacklogItemArgs {
            project_id,
            id,
            column,
        },
    )
}

#[tauri::command]
pub fn reorder_backlog_column(
    app: AppHandle,
    project_id: i64,
    column: String,
    item_ids: Vec<String>,
) -> Result<(), String> {
    reorder_backlog_column_exec(
        &app,
        ReorderBacklogColumnArgs {
            project_id,
            column,
            item_ids,
        },
    )
}

#[tauri::command]
pub fn set_item_priority(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    priority: String,
) -> Result<(), String> {
    set_item_priority_exec(
        &app,
        SetItemPriorityArgs {
            project_id,
            item_id,
            priority,
        },
    )
}

#[tauri::command]
pub fn add_backlog_label(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    label: String,
) -> Result<(), String> {
    add_backlog_label_exec(
        &app,
        BacklogLabelArgs {
            project_id,
            item_id,
            label,
        },
    )
}

#[tauri::command]
pub fn remove_backlog_label(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    label: String,
) -> Result<(), String> {
    remove_backlog_label_exec(
        &app,
        BacklogLabelArgs {
            project_id,
            item_id,
            label,
        },
    )
}

#[tauri::command]
pub fn set_backlog_responsible(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    agent: Agent,
) -> Result<(), String> {
    set_backlog_responsible_exec(
        &app,
        SetBacklogResponsibleArgs {
            project_id,
            item_id,
            agent,
        },
        None,
    )
}

#[tauri::command]
pub fn archive_backlog_item(
    app: AppHandle,
    project_id: i64,
    item_id: String,
) -> Result<(), String> {
    archive_backlog_item_exec(
        &app,
        BacklogItemArgs {
            project_id,
            item_id,
        },
    )
}

#[tauri::command]
pub fn restore_backlog_item(
    app: AppHandle,
    project_id: i64,
    item_id: String,
) -> Result<(), String> {
    restore_backlog_item_exec(
        &app,
        BacklogItemArgs {
            project_id,
            item_id,
        },
    )
}

#[tauri::command]
pub fn delete_backlog_item(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    cascade: Option<bool>,
) -> Result<(), String> {
    delete_backlog_item_with_cascade_exec(
        &app,
        BacklogItemArgs {
            project_id,
            item_id,
        },
        cascade.unwrap_or(false),
    )
}

#[tauri::command]
pub fn save_item_plan(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    plan: ItemPlanInput,
) -> Result<(), String> {
    save_item_plan_exec(
        &app,
        SaveItemPlanArgs {
            project_id,
            item_id,
            plan,
        },
    )
}

#[tauri::command]
pub async fn return_task(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    motive: String,
) -> Result<(), String> {
    return_task_exec(
        &app,
        ReturnTaskArgs {
            project_id,
            item_id,
            motive,
        },
    )
    .await
}

#[tauri::command]
pub async fn stop_task_run(
    app: AppHandle,
    project_id: i64,
    item_id: String,
) -> Result<bool, String> {
    stop_task_run_exec(
        &app,
        BacklogItemArgs {
            project_id,
            item_id,
        },
    )
    .await
}

#[tauri::command]
pub fn link_git_ref(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    kind: String,
    ref_id: String,
    label: Option<String>,
    url: Option<String>,
) -> Result<Link, String> {
    link_git_ref_exec(
        &app,
        LinkGitRefArgs {
            project_id,
            item_id,
            kind,
            ref_id,
            label,
            url,
        },
    )
}

#[tauri::command]
pub fn unlink_git_ref(
    app: AppHandle,
    project_id: i64,
    item_id: String,
    link_id: String,
) -> Result<(), String> {
    unlink_git_ref_exec(
        &app,
        UnlinkGitRefArgs {
            project_id,
            item_id,
            link_id,
        },
    )
}

#[tauri::command]
pub async fn list_git_refs(
    app: AppHandle,
    project_id: i64,
    limit: Option<usize>,
) -> Result<GitRefs, String> {
    list_git_refs_exec(&app, ListGitRefsArgs { project_id, limit }).await
}
