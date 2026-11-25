use serde::Deserialize;
use tauri::AppHandle;

use crate::orchestrator::web_search::{execute_web_search, WebSearchResponse};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebSearchArgs {
    pub query: String,
    #[serde(default)]
    pub project_id: Option<i64>,
}

pub(crate) async fn web_search_exec(
    app: &AppHandle,
    bound_project_id: Option<i64>,
    args: WebSearchArgs,
) -> Result<WebSearchResponse, String> {
    let project_id = bound_project_id.or(args.project_id);
    execute_web_search(app, project_id, &args.query).await
}

#[tauri::command]
pub async fn web_search(
    app: AppHandle,
    query: String,
    project_id: Option<i64>,
) -> Result<WebSearchResponse, String> {
    web_search_exec(&app, None, WebSearchArgs { query, project_id }).await
}
