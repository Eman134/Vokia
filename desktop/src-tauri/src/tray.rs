use std::error::Error;

use tauri::menu::{Menu, MenuEvent, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle};

use crate::window::{report_window_error, show_main_window};

const TRAY_OPEN_ID: &str = "open";
const TRAY_QUIT_ID: &str = "quit";
const TRAY_OPEN_LABEL: &str = "Abrir Vokia";
const TRAY_QUIT_LABEL: &str = "Sair";
const TRAY_TOOLTIP: &str = "Vokia";
const QUIT_EXIT_CODE: i32 = 0;

pub(crate) fn setup_tray(app: &App) -> Result<(), Box<dyn Error>> {
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
