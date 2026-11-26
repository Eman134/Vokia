#[cfg(test)]
pub(crate) mod adapters;
#[cfg(not(test))]
mod adapters;
mod agent_session;
mod archive;
mod console;
pub(crate) mod continuation;
mod emit;
mod error;
pub(crate) mod fallback;
mod flow;
mod goal;
mod loop_guard;
mod markers;
pub mod memory_review;
mod plan_stages;
mod project_context;
mod prompts;
mod runs;
mod session_policy;
mod session_pool;
pub(crate) mod step_labels;
mod text_fallback;
pub(crate) mod turn_plan;
mod turns;
mod validation;
pub(crate) mod web_search;
pub(crate) mod work_steps;

pub(crate) use adapters::{cli_has_tools, test_persistent_provider};
pub(crate) use archive::remove_archive;
pub(crate) use emit::{
    emit, emit_artifacts_changed, emit_backlog_changed, emit_feed_note, emit_message,
    reconcile_waiting, record_agent_state, MessageScope,
};
pub(crate) use prompts::variables_listing;
pub(crate) use turns::with_cache;
pub use emit::now_millis;
pub use error::{FlowError, RequestOutcome};
pub use flow::{answer_decision, handle_request, report_failure};
pub(crate) use flow::{schedule_done_cleanup, spawn_delegation_watchdog};
pub(crate) use memory_review::{
    capture_review_snapshot, schedule_memory_review, spawn_memory_review_idle_watcher,
    ReviewTrigger,
};
pub use runs::resume_run;
pub(crate) use runs::emit_agent_pauses_changed;
pub(crate) use flow::{
    active_instances, all_active_instances, drain_waiting_for_project, forget_project, has_flow,
    restore_flow_queue,
};
pub(crate) use plan_stages::start_approved_plan;
pub(crate) use session_pool::{schedule_preload, spawn_idle_reaper, SessionPool};
pub(crate) use validation::return_task;
