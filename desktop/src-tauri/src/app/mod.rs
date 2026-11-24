mod bootstrap;

use tauri::{Manager, RunEvent};

use crate::state::AppState;
use crate::{claude, commands, orchestrator, shell};

pub(crate) fn run() {
    let cli = tauri::async_runtime::block_on(claude::locate());
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(
            shell::handle_second_instance,
        ))
        .manage(AppState::new(cli))
        .invoke_handler(tauri::generate_handler![
            // team
            commands::team::cli_problem,
            commands::team::send_request,
            commands::team::answer_permission,
            commands::team::answer_decision,
            commands::team::list_agent_pauses,
            commands::team::list_interrupted_runs,
            commands::team::resume_interrupted_run,
            commands::team::discard_interrupted_run,
            commands::team::set_agent_paused,
            commands::team::list_queued_requests,
            commands::team::enqueue_queued_request,
            commands::team::update_queued_request,
            commands::team::move_queued_request,
            commands::team::remove_queued_request,
            commands::team::check_connection,
            commands::team::agent_console,
            // projects
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
            commands::projects::load_history,
            commands::projects::set_permission_level,
            commands::projects::list_permission_rules,
            commands::projects::delete_permission_rule,
            commands::execution_mode::set_execution_mode,
            commands::execution_mode::get_execution_mode,
            // channels
            commands::channels::list_channels,
            commands::channels::create_channel,
            commands::channels::update_channel,
            commands::channels::delete_channel,
            // backlog
            commands::backlog::list_backlog,
            commands::backlog::save_backlog_item,
            commands::backlog::create_backlog_item,
            commands::backlog::create_request_from_proposal,
            commands::backlog::move_backlog_item,
            commands::backlog::reorder_backlog_column,
            commands::backlog::set_item_priority,
            commands::backlog::return_task,
            commands::backlog::save_item_plan,
            commands::backlog::add_backlog_label,
            commands::backlog::remove_backlog_label,
            commands::backlog::link_git_ref,
            commands::backlog::unlink_git_ref,
            commands::backlog::list_git_refs,
            commands::backlog::set_backlog_responsible,
            commands::backlog::archive_backlog_item,
            commands::backlog::restore_backlog_item,
            commands::backlog::delete_backlog_item,
            // artifacts
            commands::artifacts::list_artifact_kinds,
            commands::artifacts::list_artifacts,
            commands::artifacts::get_artifact,
            commands::artifacts::create_artifact,
            commands::artifacts::update_artifact,
            commands::artifacts::delete_artifact,
            commands::artifacts::set_artifact_check,
            commands::artifacts::update_item_plan,
            commands::artifacts::import_plan,
            commands::artifacts::save_plan_markdown,
            commands::artifacts::pick_plan_file,
            commands::artifacts::set_artifact_criterion,
            // attachments
            commands::attachments::pick_attachment_files,
            commands::attachments::save_attachment_files,
            commands::attachments::save_attachment_bytes,
            commands::attachments::discard_attachment,
            commands::attachments::read_attachment_preview,
            // agents
            commands::agents::list_agents,
            commands::agents::available_providers,
            commands::agents::create_agent,
            commands::agents::update_agent,
            commands::agents::delete_agent,
            commands::agents::set_agent_instructions,
            commands::agents::list_temporary_overrides,
            commands::agents::set_temporary_override,
            commands::agents::clear_temporary_override,
            commands::agents::clear_all_temporary_overrides,
            commands::agents::update_ember_llm,
            // roles
            commands::roles::role_list,
            commands::roles::role_create,
            commands::roles::role_update,
            commands::roles::role_delete,
            // orchestration
            commands::orchestration::get_orchestration,
            commands::orchestration::save_layer,
            commands::orchestration::delete_layer,
            commands::orchestration::reorder_layers,
            commands::orchestration::save_team,
            commands::orchestration::delete_team,
            commands::orchestration::place_agent,
            commands::orchestration::unplace_agent,
            commands::orchestration::save_team_flow,
            commands::orchestration::set_final_validation,
            commands::orchestration::set_judge_full_flow,
            commands::orchestration::save_orchestration_settings,
            commands::orchestration::list_agent_activity,
            commands::orchestration::list_agent_copies,
            // providers
            commands::providers::list_providers,
            commands::providers::save_provider,
            commands::providers::delete_provider,
            commands::providers::discover_provider_models,
            commands::providers::inspect_cli,
            commands::providers::build_setup_prompt,
            commands::providers::test_provider_config,
            commands::providers::auto_configure_provider,
            // settings
            commands::settings::db_problem,
            commands::settings::erase_all_data,
            commands::settings::get_settings,
            commands::settings::set_setting
        ])
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            bootstrap::setup(app)?;
            orchestrator::spawn_delegation_watchdog(app.handle().clone());
            Ok(())
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
