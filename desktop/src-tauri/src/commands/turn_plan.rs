use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::agents::Agent;
use crate::db::{Db, DbState};
use crate::events::{Speaker, VokiaEvent};
use crate::orchestrator::turn_plan::{TurnPlanSnapshot, TurnPlanStore};
use crate::state::AppState;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GetTurnPlanArgs {
    pub agent: Agent,
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub card_id: Option<String>,
}

pub(crate) fn resolve_turn_plan(
    store: &TurnPlanStore,
    db: Option<&Db>,
    args: &GetTurnPlanArgs,
) -> Result<Option<TurnPlanSnapshot>, String> {
    let card_id = args
        .card_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());

    if let Some(snapshot) = store.get(&args.agent, args.project_id, card_id) {
        return Ok(Some(snapshot));
    }

    let resolved_project_id = match args.project_id {
        Some(pid) => Some(pid),
        None => db.and_then(|db| db.selected_project().ok().map(|project| project.id)),
    };

    if let (Some(pid), None) = (resolved_project_id, args.project_id) {
        if let Some(snapshot) = store.get(&args.agent, Some(pid), card_id) {
            return Ok(Some(snapshot));
        }
    }

    let (Some(db), Some(project_id)) = (db, resolved_project_id) else {
        return Ok(None);
    };
    let history = db
        .load_history(project_id)
        .map_err(|error| error.to_string())?;
    let expected_speaker = Speaker::Agent(args.agent.clone());
    for event in history.into_iter().rev() {
        let VokiaEvent::Message {
            from,
            time,
            item_id,
            activity,
            ..
        } = event
        else {
            continue;
        };
        if from != expected_speaker {
            continue;
        }
        if let Some(expected_card) = card_id {
            if item_id.as_deref() != Some(expected_card) {
                continue;
            }
        }
        let Some(plan) = activity.and_then(|act| act.plan).filter(|p| !p.is_empty()) else {
            continue;
        };
        return Ok(Some(TurnPlanSnapshot {
            project_id,
            agent: args.agent.clone(),
            card_id: item_id,
            turn_id: format!("history-{time}"),
            plan,
            updated_at: time,
            active: false,
        }));
    }
    Ok(None)
}

pub(crate) fn get_turn_plan_exec(
    app: &AppHandle,
    args: GetTurnPlanArgs,
) -> Result<Option<TurnPlanSnapshot>, String> {
    let app_state = app.state::<AppState>();
    let db_state = app.state::<DbState>();
    resolve_turn_plan(&app_state.turn_plans, db_state.db(), &args)
}

#[tauri::command]
pub fn get_turn_plan(
    app: AppHandle,
    agent: Agent,
    project_id: Option<i64>,
    card_id: Option<String>,
) -> Result<Option<TurnPlanSnapshot>, String> {
    get_turn_plan_exec(
        &app,
        GetTurnPlanArgs {
            agent,
            project_id,
            card_id,
        },
    )
}
