// Sem isso o executável release abre uma janela de console junto com a do app no Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents;
mod app;
mod attachments;
mod claude;
mod commands;
mod console_log;
mod db;
mod events;
mod mcp;
mod orchestration;
mod orchestrator;
mod permissions;
mod plan_proposal;
mod projects;
mod providers;
mod roles;
mod session_key;
mod shell;
mod state;
mod task;
mod usage;

fn main() {
    app::run();
}
