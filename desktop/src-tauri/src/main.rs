// Sem isso o executável release abre uma janela de console junto com a do app no Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cache;
mod claude;
mod commands;
mod connectivity;
mod db;
mod events;
mod orchestrator;
mod permissions;
mod projects;
mod state;
mod task;
mod tray;
mod window;

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

use tauri::Manager;

use cache::Cache;
use db::DbState;
use permissions::PermissionBroker;
use state::{AppState, FlowQueue};

fn main() {
    let cli = tauri::async_runtime::block_on(claude::locate());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            window::handle_second_instance,
        ))
        .manage(AppState {
            cli,
            cache: Mutex::new(Cache::default()),
            flow: Mutex::new(FlowQueue {
                active: false,
                pending: VecDeque::new(),
            }),
            archive: Mutex::new(Vec::new()),
            permissions: PermissionBroker::default(),
            decisions: Mutex::new(HashMap::new()),
            next_decision_id: AtomicU64::new(0),
        })
        .invoke_handler(tauri::generate_handler![
            commands::cli_problem,
            commands::send_request,
            commands::answer_permission,
            commands::answer_decision,
            commands::check_connection,
            commands::db_problem,
            commands::list_projects,
            commands::select_project,
            commands::pick_folder,
            commands::inspect_folder,
            commands::inspect_github_repo,
            commands::create_project,
            commands::load_history,
            commands::get_settings,
            commands::set_setting
        ])
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(DbState::open(&data_dir));
            tray::setup_tray(app)
        })
        .on_window_event(window::hide_main_window_on_close)
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o app Vokia");
}
