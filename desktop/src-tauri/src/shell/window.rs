use tauri::{AppHandle, Manager, Window, WindowEvent};

const MAIN_WINDOW_LABEL: &str = "main";

pub(crate) fn show_main_window(app: &AppHandle) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        window.show()?;
        window.unminimize()?;
        window.set_focus()?;
    }
    Ok(())
}

pub(crate) fn handle_second_instance(app: &AppHandle, _args: Vec<String>, _cwd: String) {
    report_window_error(show_main_window(app));
    if let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) {
        report_window_error(focus_over_foreground_thread(&window));
    }
}

fn focus_over_foreground_thread(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let foreground_thread =
        unsafe { GetWindowThreadProcessId(GetForegroundWindow(), std::ptr::null_mut()) };
    let current_thread = unsafe { GetCurrentThreadId() };
    let attached = unsafe { AttachThreadInput(current_thread, foreground_thread, ATTACH) } != 0;
    let result = window.set_focus();
    if attached {
        unsafe { AttachThreadInput(current_thread, foreground_thread, DETACH) };
    }
    result
}

const ATTACH: i32 = 1;
const DETACH: i32 = 0;

#[link(name = "user32")]
extern "system" {
    fn GetForegroundWindow() -> *mut std::ffi::c_void;
    fn GetWindowThreadProcessId(hwnd: *mut std::ffi::c_void, process_id: *mut u32) -> u32;
    fn AttachThreadInput(attach: u32, attach_to: u32, attach_flag: i32) -> i32;
}

extern "system" {
    fn GetCurrentThreadId() -> u32;
}

pub(crate) fn hide_main_window_on_close(window: &Window, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event {
        if window.label() == MAIN_WINDOW_LABEL {
            api.prevent_close();
            report_window_error(window.hide());
        }
    }
}

pub(crate) fn report_window_error(result: tauri::Result<()>) {
    if let Err(error) = result {
        eprintln!("Não foi possível atualizar a janela da Vokia: {error}");
    }
}
