use std::collections::HashMap;

use tauri::{AppHandle, Manager, State};

use crate::backlog::{BacklogItem, BacklogSnapshot, ItemPlanInput};
use crate::claude::CliProblem;
use crate::connectivity;
use crate::db::{AgentPause, DbError, DbProblem, DbState, Project};
use crate::events::{Agent, VokiaEvent};
use crate::orchestrator::{self, RequestOutcome};
use crate::permission_level::PermissionLevel;
use crate::permissions::PermissionError;
use crate::projects::{
    self, FolderInfo, GithubRepoInfo, NewProjectInput, ProjectEdit, ProjectError,
};
use crate::state::{AppState, RequestContext};

const ERASE_BUSY_MESSAGE: &str = "Há um pedido em andamento; aguarde terminar.";

#[tauri::command]
pub fn cli_problem(state: State<'_, AppState>) -> Option<CliProblem> {
    state.cli.as_ref().err().cloned()
}

#[tauri::command]
pub async fn check_connection() -> bool {
    connectivity::is_reachable().await
}

#[tauri::command]
pub async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
    to: Agent,
    project_id: i64,
    run_id: Option<u64>,
    item_id: Option<String>,
) -> Result<RequestOutcome, String> {
    let context = RequestContext::new(run_id, item_id);
    match orchestrator::handle_request(&app, &state, &text, to, project_id, context).await {
        Ok(outcome) => Ok(outcome),
        Err(error) => {
            orchestrator::report_failure(&app, project_id, &error)
                .map_err(|emit_error| emit_error.to_string())?;
            Ok(RequestOutcome::Failed)
        }
    }
}

#[tauri::command]
pub fn answer_permission(
    state: State<'_, AppState>,
    id: String,
    allow: bool,
) -> Result<(), String> {
    match state.permissions.answer(&id, allow) {
        // A outra superfície já decidiu ou o pedido expirou: nada a mostrar.
        Ok(()) | Err(PermissionError::Unknown) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[tauri::command]
pub async fn answer_decision(
    app: AppHandle,
    state: State<'_, AppState>,
    id: u64,
    answer: String,
) -> Result<(), String> {
    orchestrator::answer_decision(&app, &state, id, &answer)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn db_problem(state: State<'_, DbState>) -> Option<DbProblem> {
    state.problem()
}

#[tauri::command]
pub fn list_projects(state: State<'_, DbState>) -> Result<Vec<Project>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_projects().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn select_project(app: AppHandle, state: State<'_, DbState>, id: i64) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let previous = match db.selected_project() {
        Ok(project) => Some(project.id),
        Err(DbError::NoSelectedProject) => None,
        Err(error) => return Err(error.to_string()),
    };
    db.select_project(id).map_err(|error| error.to_string())?;
    if let Some(previous) = previous.filter(|previous| *previous != id) {
        app.state::<AppState>().permissions.deny_project(previous);
        // Fechar espera a mensagem em andamento do projeto anterior; a troca de seleção não deve esperar junto.
        tauri::async_runtime::spawn(async move {
            app.state::<AppState>()
                .sessions
                .close_project(previous)
                .await;
        });
    }
    Ok(())
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Option<String> {
    projects::pick_folder(&app)
}

#[tauri::command]
pub async fn inspect_folder(path: String) -> Result<FolderInfo, ProjectError> {
    projects::inspect_folder(&path).await
}

#[tauri::command]
pub async fn inspect_github_repo(repo: String) -> Result<GithubRepoInfo, ProjectError> {
    projects::inspect_github_repo(&repo).await
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, DbState>,
    input: NewProjectInput,
) -> Result<Project, ProjectError> {
    let db = state.require()?;
    projects::create_project(db, input).await
}

#[tauri::command]
pub fn update_project(
    state: State<'_, DbState>,
    id: i64,
    input: ProjectEdit,
) -> Result<Project, ProjectError> {
    let db = state.require()?;
    projects::update_project(db, id, input)
}

#[tauri::command]
pub fn pin_project(state: State<'_, DbState>, id: Option<i64>) -> Result<(), ProjectError> {
    let db = state.require()?;
    projects::pin_project(db, id)
}

#[tauri::command]
pub async fn set_permission_level(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    level: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let permission_level = PermissionLevel::parse(&level)
        .ok_or_else(|| DbError::InvalidPermissionLevel(level).to_string())?;
    let app_state = app.state::<AppState>();
    if orchestrator::has_flow(&app_state, Some(project_id)).map_err(|error| error.to_string())? {
        return Err(DbError::ProjectBusy.to_string());
    }
    db.set_permission_level(project_id, permission_level)
        .map_err(|error| error.to_string())?;
    stop_project_work(&app_state, project_id).await;
    Ok(())
}

#[tauri::command]
pub async fn delete_project(
    app: AppHandle,
    state: State<'_, DbState>,
    id: i64,
) -> Result<(), ProjectError> {
    let db = state.require()?;
    let app_state = app.state::<AppState>();
    if orchestrator::has_flow(&app_state, Some(id))? {
        return Err(DbError::ProjectBusy.into());
    }
    stop_project_work(&app_state, id).await;
    db.delete_project(id)?;
    orchestrator::forget_project(&app_state, id);
    Ok(())
}

#[tauri::command]
pub async fn clear_conversation(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let app_state = app.state::<AppState>();
    if orchestrator::has_flow(&app_state, Some(project_id)).map_err(|error| error.to_string())? {
        return Err(DbError::ProjectBusy.to_string());
    }
    stop_project_work(&app_state, project_id).await;
    db.clear_messages(project_id)
        .map_err(|error| error.to_string())?;
    orchestrator::forget_project(&app_state, project_id);
    Ok(())
}

#[tauri::command]
pub async fn erase_all_data(app: AppHandle, state: State<'_, DbState>) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    let app_state = app.state::<AppState>();
    if orchestrator::has_flow(&app_state, None).map_err(|error| error.to_string())? {
        return Err(ERASE_BUSY_MESSAGE.to_string());
    }
    let existing = db.list_projects().map_err(|error| error.to_string())?;
    for project in &existing {
        stop_project_work(&app_state, project.id).await;
    }
    db.erase_all().map_err(|error| error.to_string())?;
    for project in &existing {
        orchestrator::forget_project(&app_state, project.id);
    }
    orchestrator::remove_archive(&app, &app_state).map_err(|error| error.to_string())
}

async fn stop_project_work(state: &AppState, project_id: i64) {
    state.permissions.deny_project(project_id);
    state.sessions.close_project(project_id).await;
}

#[tauri::command]
pub fn load_history(state: State<'_, DbState>, project_id: i64) -> Result<Vec<VokiaEvent>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.load_history(project_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_agent_pauses(state: State<'_, DbState>) -> Result<Vec<AgentPause>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.agent_pauses().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_agent_paused(
    state: State<'_, DbState>,
    project_id: i64,
    agent: Agent,
    paused: bool,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_agent_paused(project_id, agent, paused)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_settings(state: State<'_, DbState>) -> Result<HashMap<String, String>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.load_settings().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_setting(state: State<'_, DbState>, key: String, value: String) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.save_setting(&key, &value)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn list_backlog(state: State<'_, DbState>, project_id: i64) -> Result<BacklogSnapshot, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_backlog(project_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_backlog_item(
    state: State<'_, DbState>,
    project_id: i64,
    item: BacklogItem,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.save_backlog_item(project_id, &item)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn move_backlog_item(
    state: State<'_, DbState>,
    project_id: i64,
    id: String,
    column: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.move_backlog_item(project_id, &id, &column)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_item_plan(
    state: State<'_, DbState>,
    project_id: i64,
    item_id: String,
    plan: ItemPlanInput,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.save_item_plan(project_id, &item_id, &plan)
        .map_err(|error| error.to_string())
}
