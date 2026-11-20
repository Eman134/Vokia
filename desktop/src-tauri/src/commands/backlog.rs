use tauri::{AppHandle, Manager, State};

use crate::agents::{Agent, AgentError, AgentRegistry};
use crate::db::{
    AgentActor, BacklogItem, BacklogSnapshot, CardOperation, CardResult, DbState, ItemPlanInput,
    NewCard,
};
use crate::orchestrator::{self, emit_backlog_changed};
use crate::state::AppState;

const MISSING_RESULT: &str = "O banco não devolveu o card criado.";

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
pub fn create_backlog_item(
    state: State<'_, DbState>,
    project_id: i64,
    request: NewCard,
) -> Result<BacklogItem, String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.create_card(project_id, &request)
        .map_err(|error| error.to_string())
}

// Mesmo caminho do `create_card` do MCP: origem agente, título repetido e acima de 200 recusados pelo banco.
#[tauri::command]
pub fn create_request_from_proposal(
    app: tauri::AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    agent: Agent,
    title: String,
    detail: Option<String>,
) -> Result<CardResult, String> {
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    if team.profile(&agent).is_none() {
        return Err(AgentError::UnknownAgent(agent.to_string()).to_string());
    }
    let actor = AgentActor {
        name: team.name_of(&agent).to_string(),
        id: agent,
        current_card: None,
    };
    let db = state.require().map_err(|error| error.to_string())?;
    let operation = CardOperation::Create {
        title,
        description: detail,
        responsible: None,
    };
    db.apply_card_operations(project_id, &actor, &[operation])
        .map_err(|error| error.to_string())?
        .pop()
        .ok_or_else(|| MISSING_RESULT.to_string())
}

#[tauri::command]
pub fn move_backlog_item(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    id: String,
    column: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.move_backlog_item(project_id, &id, &column)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
pub fn add_backlog_label(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    item_id: String,
    label: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.add_backlog_label(project_id, &item_id, &label)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
pub fn remove_backlog_label(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    item_id: String,
    label: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.remove_backlog_label(project_id, &item_id, &label)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
pub fn set_backlog_responsible(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    item_id: String,
    agent: Agent,
) -> Result<(), String> {
    let team = app
        .state::<AgentRegistry>()
        .team()
        .map_err(|error| error.to_string())?;
    let db = state.require().map_err(|error| error.to_string())?;
    db.set_backlog_responsible(project_id, &item_id, &agent, &team)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
pub fn archive_backlog_item(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    item_id: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.archive_backlog_item(project_id, &item_id)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
pub fn restore_backlog_item(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    item_id: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.restore_backlog_item(project_id, &item_id)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}

#[tauri::command]
pub fn delete_backlog_item(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    item_id: String,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.delete_backlog_item(project_id, &item_id)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}

// A gravação já está no banco; falha ao avisar o front não vira erro para quem chamou.
pub(crate) fn notify_backlog_changed(app: &AppHandle, project_id: i64) {
    let _ = emit_backlog_changed(app, project_id);
}

#[tauri::command]
pub fn save_item_plan(
    app: AppHandle,
    state: State<'_, DbState>,
    project_id: i64,
    item_id: String,
    plan: ItemPlanInput,
) -> Result<(), String> {
    let db = state.require().map_err(|error| error.to_string())?;
    db.save_item_plan(project_id, &item_id, &plan)
        .map_err(|error| error.to_string())?;
    notify_backlog_changed(&app, project_id);
    Ok(())
}

// Devolve a task concluída ao líder: o motivo é obrigatório e o card volta a In progress.
#[tauri::command]
pub async fn return_task(
    app: AppHandle,
    state: State<'_, AppState>,
    project_id: i64,
    item_id: String,
    motive: String,
) -> Result<(), String> {
    orchestrator::return_task(&app, &state, project_id, &item_id, &motive)
        .await
        .map_err(|error| error.to_string())
}
