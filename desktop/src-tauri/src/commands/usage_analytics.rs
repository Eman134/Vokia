use serde::Deserialize;
use tauri::{AppHandle, Manager};

use crate::db::{DbState, UsageAnalyticsQuery, UsageAnalyticsResponse};

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GetUsageAnalyticsArgs {
    #[serde(default)]
    pub days: Option<u32>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub project_id: Option<i64>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub provider_id: Option<String>,
}

impl From<GetUsageAnalyticsArgs> for UsageAnalyticsQuery {
    fn from(args: GetUsageAnalyticsArgs) -> Self {
        Self {
            days: args.days,
            from: args.from,
            to: args.to,
            timezone: args.timezone,
            project_id: args.project_id,
            agent_id: args.agent_id,
            provider_id: args.provider_id,
        }
    }
}

pub(crate) fn get_usage_analytics_exec(
    app: &AppHandle,
    args: GetUsageAnalyticsArgs,
) -> Result<UsageAnalyticsResponse, String> {
    let state = app.state::<DbState>();
    let db = state.require().map_err(|error| error.to_string())?;
    let query = UsageAnalyticsQuery::from(args);
    db.query_usage_analytics(&query)
        .map_err(|error| error.to_string())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub fn get_usage_analytics(
    app: AppHandle,
    days: Option<u32>,
    from: Option<String>,
    to: Option<String>,
    timezone: Option<String>,
    project_id: Option<i64>,
    agent_id: Option<String>,
    provider_id: Option<String>,
) -> Result<UsageAnalyticsResponse, String> {
    get_usage_analytics_exec(
        &app,
        GetUsageAnalyticsArgs {
            days,
            from,
            to,
            timezone,
            project_id,
            agent_id,
            provider_id,
        },
    )
}
