use tauri::{AppHandle, Manager, State};

use crate::db::{DbError, DbState, PermissionRule, Project};
use crate::events::VokiaEvent;
use crate::orchestrator;
use crate::permissions::PermissionLevel;
use crate::projects::{
    self, FolderInfo, GithubRepoInfo, NewProjectInput, ProjectEdit, ProjectError,
};
use crate::state::AppState;

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
pub fn list_permission_rules(
    state: State<'_, DbState>,
    project_id: i64,
) -> Result<Vec<PermissionRule>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_permission_rules(project_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_permission_rule(
    state: State<'_, DbState>,
    project_id: i64,
    tool: String,
    command: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_permission_rule(project_id, &tool, &command)
        .map_err(|error| error.to_string())
}

pub(super) async fn stop_project_work(state: &AppState, project_id: i64) {
    state.permissions.deny_project(project_id);
    state.sessions.close_project(project_id).await;
}

#[tauri::command]
pub fn load_history(state: State<'_, DbState>, project_id: i64) -> Result<Vec<VokiaEvent>, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.load_history(project_id)
        .map_err(|error| error.to_string())
}
