// Sem isso o executável release abre uma janela de console junto com a do app no Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod claude;
mod events;
mod orchestrator;

use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::sync::Mutex;

use tauri::menu::{Menu, MenuEvent, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Manager, State, Window, WindowEvent};

use claude::CliProblem;
use events::Agent;

const MAIN_WINDOW_LABEL: &str = "main";
const TRAY_OPEN_ID: &str = "open";
const TRAY_QUIT_ID: &str = "quit";
const TRAY_OPEN_LABEL: &str = "Abrir Vokia";
const TRAY_QUIT_LABEL: &str = "Sair";
const TRAY_TOOLTIP: &str = "Vokia";
const QUIT_EXIT_CODE: i32 = 0;

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

fn setup_tray(app: &App) -> Result<(), Box<dyn Error>> {
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("Ícone padrão da Vokia não encontrado; a bandeja não pode ser criada.")?;
    let open = MenuItemBuilder::with_id(TRAY_OPEN_ID, TRAY_OPEN_LABEL)
        .build(app)
        .map_err(tray_error)?;
    let quit = MenuItemBuilder::with_id(TRAY_QUIT_ID, TRAY_QUIT_LABEL)
        .build(app)
        .map_err(tray_error)?;
    let menu = Menu::with_items(app, &[&open, &quit]).map_err(tray_error)?;
    TrayIconBuilder::new()
        .icon(icon)
        .tooltip(TRAY_TOOLTIP)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_tray_menu)
        .on_tray_icon_event(handle_tray_click)
        .build(app)
        .map_err(tray_error)?;
    Ok(())
}

fn tray_error(error: tauri::Error) -> String {
    format!("Não foi possível criar o ícone da bandeja: {error}")
}

fn handle_tray_menu(app: &AppHandle, event: MenuEvent) {
    match event.id.as_ref() {
        TRAY_OPEN_ID => report_window_error(show_main_window(app)),
        TRAY_QUIT_ID => app.exit(QUIT_EXIT_CODE),
        _ => {}
    }
}

fn handle_tray_click(tray: &TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        report_window_error(show_main_window(tray.app_handle()));
    }
}

fn show_main_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}

fn hide_main_window_on_close(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == MAIN_WINDOW_LABEL {
            api.prevent_close();
            report_window_error(window.hide());
        }
    }
}

fn report_window_error(result: tauri::Result<()>) {
    if let Err(error) = result {
        eprintln!("Não foi possível atualizar a janela da Vokia: {error}");
    }
}

fn main() {
    let cli = tauri::async_runtime::block_on(claude::locate());
    tauri::Builder::default()
        .manage(AppState {
            cli,
            sessions: Mutex::new(HashMap::new()),
        })
        .invoke_handler(tauri::generate_handler![cli_problem, send_request])
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| setup_tray(app))
        .on_window_event(hide_main_window_on_close)
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o app Vokia");
}
