mod adapters;
mod agent_session;
mod archive;
mod console;
mod emit;
mod error;
pub(crate) mod fallback;
mod flow;
mod goal;
mod loop_guard;
mod markers;
mod project_context;
mod prompts;
mod runs;
mod session_policy;
mod session_pool;
mod text_fallback;
mod turns;
mod validation;

pub(crate) use adapters::cli_has_tools;
pub(crate) use archive::remove_archive;
pub(crate) use emit::{
    emit_artifacts_changed, emit_backlog_changed, emit_feed_note, emit_message, reconcile_waiting,
    record_agent_state, MessageScope,
};
pub(crate) use prompts::variables_listing;
pub(crate) use turns::with_cache;
pub use emit::now_millis;
pub use error::{FlowError, RequestOutcome};
pub use flow::{answer_decision, handle_request, report_failure};
pub(crate) use flow::{schedule_done_cleanup, spawn_delegation_watchdog};
pub use runs::resume_run;
pub(crate) use runs::emit_agent_pauses_changed;
pub(crate) use flow::{
    active_instances, all_active_instances, drain_waiting_for_project, forget_project, has_flow,
    restore_flow_queue,
};
pub(crate) use session_pool::{schedule_preload, SessionPool};
pub(crate) use validation::return_task;
