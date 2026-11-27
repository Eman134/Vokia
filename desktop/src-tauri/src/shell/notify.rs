use std::collections::HashSet;
use std::sync::{Mutex, OnceLock, PoisonError};

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::db::DbState;

const MAIN_WINDOW_LABEL: &str = "main";

static NOTIFIED_IDS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn notified_ids() -> &'static Mutex<HashSet<String>> {
    NOTIFIED_IDS.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn should_notify(window_focused: bool, enabled: bool, already_notified: bool) -> bool {
    !window_focused && enabled && !already_notified
}

pub(crate) fn notify_decision(app: &AppHandle, decision_id: &str, agent: &str, title: &str) {
    let window_focused = app
        .get_webview_window(MAIN_WINDOW_LABEL)
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false);
    let enabled = app
        .try_state::<DbState>()
        .and_then(|state| {
            state
                .require()
                .ok()
                .and_then(|db| db.notifications_enabled().ok())
        })
        .unwrap_or(true);
    let mut seen = notified_ids().lock().unwrap_or_else(PoisonError::into_inner);
    if !should_notify(window_focused, enabled, seen.contains(decision_id)) {
        return;
    }
    seen.insert(decision_id.to_string());
    drop(seen);

    let heading = format!("Vokia · {agent}");
    let _ = app.notification().builder().title(heading).body(title).show();
}

#[cfg(test)]
mod tests {
    use super::should_notify;

    #[test]
    fn notifica_apenas_quando_fora_de_foco_habilitado_e_inedito() {
        assert!(should_notify(false, true, false));
        assert!(!should_notify(true, true, false));
        assert!(!should_notify(false, false, false));
        assert!(!should_notify(false, true, true));
    }
}
