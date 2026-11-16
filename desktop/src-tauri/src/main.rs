// Sem isso o executável release abre uma janela de console junto com a do app no Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents;
mod claude;
mod commands;
mod console_log;
mod db;
mod events;
mod orchestrator;
mod permissions;
mod projects;
mod session_key;
mod shell;
mod state;
mod task;

use tauri::{Manager, RunEvent};

use db::DbState;
use state::AppState;

fn main() {
    let cli = tauri::async_runtime::block_on(claude::locate());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            shell::handle_second_instance,
        ))
        .manage(AppState::new(cli))
        .invoke_handler(tauri::generate_handler![
            commands::team::cli_problem,
            commands::team::send_request,
            commands::team::answer_permission,
            commands::team::answer_decision,
            commands::team::list_agent_pauses,
            commands::team::set_agent_paused,
            commands::team::check_connection,
            commands::team::agent_console,
            commands::settings::db_problem,
            commands::projects::list_projects,
            commands::projects::select_project,
            commands::projects::pick_folder,
            commands::projects::inspect_folder,
            commands::projects::inspect_github_repo,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::delete_project,
            commands::projects::pin_project,
            commands::projects::clear_conversation,
            commands::settings::erase_all_data,
            commands::projects::load_history,
            commands::settings::get_settings,
            commands::settings::set_setting,
            commands::channels::list_channels,
            commands::channels::create_channel,
            commands::channels::delete_channel,
            commands::backlog::list_backlog,
            commands::backlog::save_backlog_item,
            commands::backlog::move_backlog_item,
            commands::backlog::save_item_plan,
            commands::projects::set_permission_level,
            commands::agents::list_agents,
            commands::agents::create_agent,
            commands::agents::update_agent
        ])
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            app.manage(agents::AgentRegistry::load(&data_dir));
            app.manage(DbState::open(&data_dir));
            shell::setup_tray(app)
        })
        .on_window_event(shell::hide_main_window_on_close)
        .build(tauri::generate_context!())
        .expect("falha ao iniciar o app Vokia")
        .run(|app, event| {
            if matches!(event, RunEvent::Exit) {
                app.state::<AppState>().sessions.close_all();
            }
        });
}
