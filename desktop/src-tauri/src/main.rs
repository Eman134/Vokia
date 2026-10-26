// Sem isso o executável release abre uma janela de console junto com a do app no Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod claude;
mod commands;
mod connectivity;
mod events;
mod orchestrator;
mod state;
mod tray;
mod window;

use std::collections::HashMap;
use std::sync::Mutex;

use state::AppState;

fn main() {
    let cli = tauri::async_runtime::block_on(claude::locate());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            window::handle_second_instance,
        ))
        .manage(AppState {
            cli,
            sessions: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![
            commands::cli_problem,
            commands::send_request,
            commands::check_connection
        ])
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| tray::setup_tray(app))
        .on_window_event(window::hide_main_window_on_close)
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o app Vokia");
}
