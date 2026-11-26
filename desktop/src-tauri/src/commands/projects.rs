use serde::Deserialize;
use tauri::{AppHandle, Manager};

use super::attachments;
use crate::db::{DbError, DbState, PermissionRule, Project};
use crate::events::VokiaEvent;
use crate::orchestrator;
use crate::permissions::PermissionLevel;
use crate::projects::{
    self, FolderInfo, GithubRepoInfo, NewProjectInput, ProjectEdit, ProjectError,
};
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SelectProjectArgs {
    pub id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InspectFolderArgs {
    pub path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InspectGithubRepoArgs {
    pub repo: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateProjectArgs {
    pub input: NewProjectInput,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateProjectArgs {
    pub id: i64,
    pub input: ProjectEdit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PinProjectArgs {
    pub id: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SetPermissionLevelArgs {
    pub project_id: i64,
    pub level: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeleteProjectArgs {
    pub id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ProjectIdArgs {
    pub project_id: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DeletePermissionRuleArgs {
    pub project_id: i64,
    pub tool: String,
    pub command: String,
}

pub(crate) fn list_projects_exec(app: &AppHandle) -> Result<Vec<Project>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_projects().map_err(|error| error.to_string())
}

pub(crate) fn select_project_exec(app: &AppHandle, args: SelectProjectArgs) -> Result<(), String> {
    let id = args.id;
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let previous = match db.selected_project() {
        Ok(project) => Some(project.id),
        Err(DbError::NoSelectedProject) => None,
        Err(error) => return Err(error.to_string()),
    };
    db.select_project(id).map_err(|error| error.to_string())?;
    if previous != Some(id) {
        orchestrator::schedule_preload(app.clone(), id);
    }
    if let Some(previous) = previous.filter(|previous| *previous != id) {
        app.state::<AppState>().permissions.deny_project(previous);
        // Fechar espera a mensagem em andamento do projeto anterior; a troca de seleção não deve esperar junto.
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            let app_state = handle.state::<AppState>();
            app_state.sessions.close_project(previous).await;
            orchestrator::reconcile_waiting(&handle, &app_state, previous);
        });
    }
    Ok(())
}

pub(crate) async fn inspect_folder_exec(
    args: InspectFolderArgs,
) -> Result<FolderInfo, ProjectError> {
    projects::inspect_folder(&args.path).await
}

pub(crate) async fn inspect_github_repo_exec(
    args: InspectGithubRepoArgs,
) -> Result<GithubRepoInfo, ProjectError> {
    projects::inspect_github_repo(&args.repo).await
}

pub(crate) async fn create_project_exec(
    app: &AppHandle,
    args: CreateProjectArgs,
) -> Result<Project, ProjectError> {
    let state = app.state::<DbState>();
    let db = state.require()?;
    projects::create_project(db, args.input).await
}

pub(crate) fn update_project_exec(
    app: &AppHandle,
    args: UpdateProjectArgs,
) -> Result<Project, ProjectError> {
    let state = app.state::<DbState>();
    let db = state.require()?;
    projects::update_project(db, args.id, args.input)
}

pub(crate) fn pin_project_exec(app: &AppHandle, args: PinProjectArgs) -> Result<(), ProjectError> {
    let state = app.state::<DbState>();
    let db = state.require()?;
    projects::pin_project(db, args.id)
}

pub(crate) async fn set_permission_level_exec(
    app: &AppHandle,
    args: SetPermissionLevelArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let permission_level = PermissionLevel::parse(&args.level)
        .ok_or_else(|| DbError::InvalidPermissionLevel(args.level).to_string())?;
    let app_state = app.state::<AppState>();
    if orchestrator::has_flow(&app_state, Some(args.project_id))
        .map_err(|error| error.to_string())?
    {
        return Err(DbError::ProjectBusy.to_string());
    }
    db.set_permission_level(args.project_id, permission_level)
        .map_err(|error| error.to_string())?;
    stop_project_work(&app_state, args.project_id).await;
    Ok(())
}

pub(crate) async fn delete_project_exec(
    app: &AppHandle,
    args: DeleteProjectArgs,
) -> Result<(), ProjectError> {
    let state = app.state::<DbState>();
    let db = state.require()?;
    let app_state = app.state::<AppState>();
    if orchestrator::has_flow(&app_state, Some(args.id))? {
        return Err(DbError::ProjectBusy.into());
    }
    stop_project_work(&app_state, args.id).await;
    db.delete_project(args.id)?;
    attachments::purge_project_files(app, db, args.id);
    orchestrator::forget_project(&app_state, args.id);
    Ok(())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClearConversationArgs {
    pub project_id: i64,
    #[serde(default)]
    pub scope: Option<crate::db::ClearConversationScope>,
}

pub(crate) async fn clear_conversation_exec(
    app: &AppHandle,
    args: ClearConversationArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let app_state = app.state::<AppState>();
    let is_all = args
        .scope
        .as_ref()
        .and_then(|s| s.kind.as_deref())
        .is_none_or(|k| k == "all");
    if is_all
        && orchestrator::has_flow(&app_state, Some(args.project_id))
            .map_err(|error| error.to_string())?
    {
        return Err(DbError::ProjectBusy.to_string());
    }
    if is_all {
        stop_project_work(&app_state, args.project_id).await;
    }
    let review_snapshot = orchestrator::capture_review_snapshot(db, args.project_id);
    db.clear_messages_scoped(args.project_id, args.scope.as_ref())
        .map_err(|error| error.to_string())?;
    if !review_snapshot.messages.is_empty() || !review_snapshot.work_steps.is_empty() {
        orchestrator::schedule_memory_review(
            app,
            args.project_id,
            orchestrator::ReviewTrigger::ClearConversation,
            None,
            None,
            Some(review_snapshot),
        );
    }
    if is_all {
        attachments::purge_sent_files(app, db, args.project_id);
        orchestrator::forget_project(&app_state, args.project_id);
    }
    Ok(())
}

pub(crate) fn list_permission_rules_exec(
    app: &AppHandle,
    args: ProjectIdArgs,
) -> Result<Vec<PermissionRule>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.list_permission_rules(args.project_id)
        .map_err(|error| error.to_string())
}

pub(crate) fn delete_permission_rule_exec(
    app: &AppHandle,
    args: DeletePermissionRuleArgs,
) -> Result<(), String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_permission_rule(args.project_id, &args.tool, &args.command)
        .map_err(|error| error.to_string())
}

pub(crate) fn load_history_exec(
    app: &AppHandle,
    args: ProjectIdArgs,
) -> Result<Vec<VokiaEvent>, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    db.load_history(args.project_id)
        .map_err(|error| error.to_string())
}

pub(super) async fn stop_project_work(state: &AppState, project_id: i64) {
    state.continuations.cancel_project(project_id);
    state.permissions.deny_project(project_id);
    state.sessions.close_project(project_id).await;
}

#[tauri::command]
pub fn list_projects(app: AppHandle) -> Result<Vec<Project>, String> {
    list_projects_exec(&app)
}

#[tauri::command]
pub fn select_project(app: AppHandle, id: i64) -> Result<(), String> {
    select_project_exec(&app, SelectProjectArgs { id })
}

#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Option<String> {
    projects::pick_folder(&app)
}

#[tauri::command]
pub async fn inspect_folder(path: String) -> Result<FolderInfo, ProjectError> {
    inspect_folder_exec(InspectFolderArgs { path }).await
}

#[tauri::command]
pub async fn inspect_github_repo(repo: String) -> Result<GithubRepoInfo, ProjectError> {
    inspect_github_repo_exec(InspectGithubRepoArgs { repo }).await
}

#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    input: NewProjectInput,
) -> Result<Project, ProjectError> {
    create_project_exec(&app, CreateProjectArgs { input }).await
}

#[tauri::command]
pub fn update_project(
    app: AppHandle,
    id: i64,
    input: ProjectEdit,
) -> Result<Project, ProjectError> {
    update_project_exec(&app, UpdateProjectArgs { id, input })
}

#[tauri::command]
pub fn pin_project(app: AppHandle, id: Option<i64>) -> Result<(), ProjectError> {
    pin_project_exec(&app, PinProjectArgs { id })
}

#[tauri::command]
pub async fn set_permission_level(
    app: AppHandle,
    project_id: i64,
    level: String,
) -> Result<(), String> {
    set_permission_level_exec(&app, SetPermissionLevelArgs { project_id, level }).await
}

#[tauri::command]
pub async fn delete_project(app: AppHandle, id: i64) -> Result<(), ProjectError> {
    delete_project_exec(&app, DeleteProjectArgs { id }).await
}

#[tauri::command]
pub async fn clear_conversation(
    app: AppHandle,
    project_id: i64,
    scope: Option<crate::db::ClearConversationScope>,
) -> Result<(), String> {
    clear_conversation_exec(&app, ClearConversationArgs { project_id, scope }).await
}

#[tauri::command]
pub fn list_permission_rules(
    app: AppHandle,
    project_id: i64,
) -> Result<Vec<PermissionRule>, String> {
    list_permission_rules_exec(&app, ProjectIdArgs { project_id })
}

#[tauri::command]
pub fn delete_permission_rule(
    app: AppHandle,
    project_id: i64,
    tool: String,
    command: String,
) -> Result<(), String> {
    delete_permission_rule_exec(
        &app,
        DeletePermissionRuleArgs {
            project_id,
            tool,
            command,
        },
    )
}

#[tauri::command]
pub fn load_history(app: AppHandle, project_id: i64) -> Result<Vec<VokiaEvent>, String> {
    load_history_exec(&app, ProjectIdArgs { project_id })
}
