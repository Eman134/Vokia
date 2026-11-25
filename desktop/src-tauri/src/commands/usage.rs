use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::events::{VokiaEvent, EVENT_NAME};
use crate::orchestrator::fallback::apply_released_limit_returns;
use crate::state::AppState;
use crate::usage::{now_millis, PlanUsageSnapshot};

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlanUsageArgs {
    #[serde(default)]
    pub provider_id: Option<String>,
}

pub(crate) async fn get_plan_usage_exec(
    app: &AppHandle,
    args: PlanUsageArgs,
) -> Result<PlanUsageSnapshot, String> {
    let filter = args
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let now_ms = now_millis();
    apply_released_limit_returns(app).await;
    let state = app.state::<AppState>();
    let outcomes = state.usage.refresh_all(filter, now_ms).await;
    for outcome in outcomes {
        if !outcome.from_cache {
            if let Some(usage) = outcome.usage {
                let _ = app.emit(EVENT_NAME, VokiaEvent::ProviderUsageUpdated { usage });
            }
        }
    }
    Ok(state.usage.snapshot(filter, now_millis()))
}

pub(crate) async fn refresh_plan_usage_exec(
    app: &AppHandle,
    args: PlanUsageArgs,
) -> Result<PlanUsageSnapshot, String> {
    let filter = args
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty());
    let now_ms = now_millis();
    apply_released_limit_returns(app).await;
    let state = app.state::<AppState>();
    let outcomes = state.usage.refresh_all(filter, now_ms).await;
    for outcome in outcomes {
        if !outcome.from_cache {
            if let Some(usage) = outcome.usage {
                let _ = app.emit(EVENT_NAME, VokiaEvent::ProviderUsageUpdated { usage });
            }
        }
    }
    Ok(state.usage.snapshot(filter, now_millis()))
}

#[tauri::command]
pub async fn get_plan_usage(
    app: AppHandle,
    provider_id: Option<String>,
) -> Result<PlanUsageSnapshot, String> {
    get_plan_usage_exec(&app, PlanUsageArgs { provider_id }).await
}

#[tauri::command]
pub async fn refresh_plan_usage(
    app: AppHandle,
    provider_id: Option<String>,
) -> Result<PlanUsageSnapshot, String> {
    refresh_plan_usage_exec(&app, PlanUsageArgs { provider_id }).await
}
