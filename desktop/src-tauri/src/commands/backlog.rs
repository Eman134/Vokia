use tauri::State;

use crate::db::{BacklogItem, BacklogSnapshot, DbState, ItemPlanInput};

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
