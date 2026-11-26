use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::db::{DbState, LearningSuggestion, ProjectMemory, SkillScope, SuggestionStatus};
use crate::events::VokiaEvent;
use crate::orchestrator::memory_review::{
    apply_learning_suggestion_on_db, dismiss_learning_suggestion_on_db,
    run_memory_review_for_project, ReviewOutcome, ReviewTrigger,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunMemoryReviewArgs {
    pub project_id: i64,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub card_id: Option<String>,
    #[serde(default)]
    pub trigger: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListLearningSuggestionsArgs {
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyLearningSuggestionArgs {
    pub id: String,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DismissLearningSuggestionArgs {
    pub id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListProjectMemoriesArgs {
    pub project_id: i64,
}

fn now_millis_i64() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

fn emit_learning_changed(app: &AppHandle, project_id: i64) {
    let db_state = app.state::<DbState>();
    let pending_count = db_state
        .db()
        .and_then(|db| db.count_pending_learning_suggestions(Some(project_id)).ok())
        .unwrap_or(0);
    let _ = crate::orchestrator::emit(
        app,
        VokiaEvent::LearningSuggestionsChanged {
            project_id,
            pending_count,
            new_count: 0,
        },
    );
}

pub(crate) async fn run_memory_review_exec(
    app: &AppHandle,
    args: RunMemoryReviewArgs,
) -> Result<ReviewOutcome, String> {
    let trigger = ReviewTrigger::parse(args.trigger.as_deref());
    run_memory_review_for_project(
        app,
        args.project_id,
        trigger,
        args.agent.as_deref(),
        args.card_id.as_deref(),
        None,
    )
    .await
}

pub(crate) fn list_learning_suggestions_exec(
    app: &AppHandle,
    args: ListLearningSuggestionsArgs,
) -> Result<Vec<LearningSuggestion>, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    let status = args
        .status
        .as_deref()
        .and_then(SuggestionStatus::parse);
    db.list_learning_suggestions(args.project_id, status)
        .map_err(|e| e.to_string())
}

pub(crate) fn apply_learning_suggestion_exec(
    app: &AppHandle,
    args: ApplyLearningSuggestionArgs,
) -> Result<LearningSuggestion, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    let existing = db
        .get_learning_suggestion(&args.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("Sugestão não encontrada: {}", args.id))?;
    let project_path = db
        .project(existing.project_id)
        .ok()
        .map(|p| PathBuf::from(p.path));
    let app_data_dir = app.path().app_data_dir().ok();
    let scope_override = args.scope.as_deref().map(SkillScope::parse);
    let updated = apply_learning_suggestion_on_db(
        db,
        &args.id,
        scope_override,
        project_path.as_deref(),
        app_data_dir.as_deref(),
        now_millis_i64(),
    )?;
    emit_learning_changed(app, updated.project_id);
    Ok(updated)
}

pub(crate) fn dismiss_learning_suggestion_exec(
    app: &AppHandle,
    args: DismissLearningSuggestionArgs,
) -> Result<LearningSuggestion, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    let updated = dismiss_learning_suggestion_on_db(db, &args.id, now_millis_i64())?;
    emit_learning_changed(app, updated.project_id);
    Ok(updated)
}

pub(crate) fn list_project_memories_exec(
    app: &AppHandle,
    args: ListProjectMemoriesArgs,
) -> Result<Vec<ProjectMemory>, String> {
    let db_state = app.state::<DbState>();
    let db = db_state.require().map_err(|e| e.to_string())?;
    db.list_project_memories(args.project_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_memory_review(
    app: AppHandle,
    args: RunMemoryReviewArgs,
) -> Result<ReviewOutcome, String> {
    run_memory_review_exec(&app, args).await
}

#[tauri::command]
pub fn list_learning_suggestions(
    app: AppHandle,
    args: Option<ListLearningSuggestionsArgs>,
) -> Result<Vec<LearningSuggestion>, String> {
    list_learning_suggestions_exec(&app, args.unwrap_or_default())
}

#[tauri::command]
pub fn apply_learning_suggestion(
    app: AppHandle,
    args: ApplyLearningSuggestionArgs,
) -> Result<LearningSuggestion, String> {
    apply_learning_suggestion_exec(&app, args)
}

#[tauri::command]
pub fn dismiss_learning_suggestion(
    app: AppHandle,
    args: DismissLearningSuggestionArgs,
) -> Result<LearningSuggestion, String> {
    dismiss_learning_suggestion_exec(&app, args)
}

#[tauri::command]
pub fn list_project_memories(
    app: AppHandle,
    args: ListProjectMemoriesArgs,
) -> Result<Vec<ProjectMemory>, String> {
    list_project_memories_exec(&app, args)
}
