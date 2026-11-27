mod notify;
mod tray;
mod window;

pub(crate) use notify::notify_decision;
pub(crate) use tray::setup_tray;
pub(crate) use window::{handle_second_instance, hide_main_window_on_close};
