// Sem isso o executável release abre uma janela de console junto com a do app no Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod claude;
mod events;
mod orchestrator;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, State};

use claude::CliProblem;
use events::Agent;

pub struct AppState {
    pub cli: Result<PathBuf, CliProblem>,
    pub sessions: Mutex<HashMap<Agent, String>>,
}

#[tauri::command]
fn cli_problem(state: State<'_, AppState>) -> Option<CliProblem> {
    state.cli.as_ref().err().cloned()
}

#[tauri::command]
async fn send_request(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String> {
    if let Err(error) = orchestrator::handle_request(&app, &state, &text).await {
        orchestrator::report_failure(&app, &error).map_err(|emit_error| emit_error.to_string())?;
    }
    Ok(())
}

fn main() {
    let cli = tauri::async_runtime::block_on(claude::locate());
    tauri::Builder::default()
        .manage(AppState {
            cli,
            sessions: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![cli_problem, send_request])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o app Vokia");
}
