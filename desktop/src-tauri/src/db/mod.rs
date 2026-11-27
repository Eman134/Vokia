mod agent_removal;
mod agent_sessions;
mod approvals;
mod artifact_documents;
mod artifact_kinds;
mod artifacts;
mod attachments;
mod backlog;
mod channels;
mod connection;
mod error;
pub mod history;
pub mod learning;
mod messages;
mod migrations;
mod orchestration;
mod permission_rules;
mod projects;
mod queued_requests;
mod runs;
pub mod search;
mod settings;
mod sprints;
pub mod usage_analytics;
pub mod usage_pricing;
mod work_steps;

pub use agent_sessions::AgentPause;
pub use approvals::ApprovalHistory;
pub use artifacts::Artifact;
pub(crate) use artifacts::artifact_id;
pub(crate) use backlog::plan_stage_number;
pub use artifact_documents::{
    ArtifactDetail, ArtifactDocument, ArtifactDocumentPatch, ArtifactSummary, NewArtifactDocument,
};
pub use artifact_kinds::{ArtifactKind, ARTIFACT_KINDS};
pub use messages::{ClearConversationScope, StoredTaskResult};
pub use permission_rules::{root_command, PermissionRule};
pub use backlog::{
    board_column, is_stage_finished, is_unstarted, AgentActor, BacklogItem, BacklogSnapshot, CardOperation, CardOutcome, CardResponsible,
    CardResult, GitRefLinkInput, ItemPlanInput, Link, NewCard, AGENT_FORBIDDEN_COLUMNS, BACKLOG_LABEL_MAX_CHARS,
    BOARD_COLUMNS,
};
pub use sprints::{NewSprint, Sprint, SprintPatch};
#[cfg(test)]
pub use backlog::{MicroTask, ScriptBlock};
pub use channels::Channel;
pub use connection::{Db, DbState};
pub use error::{DbError, DbProblem};
pub use orchestration::{
    FlowEdge, FlowEdgeDraft, FlowGateway, FlowProblem,
    Handoff, HandoffDraft, Layer, LayerDraft, OrchestrationSeed, OrchestrationSettings,
    OrchestrationSnapshot, Placement, PlacementDraft, Team, TeamDraft,
    TaskStage, TeamFlowDraft,
};
pub(crate) use orchestration::{
    FLOW_END, FLOW_START, GATEWAY_EXCLUSIVE, GATEWAY_PARALLEL, LINK_COMMUNICATION, LINK_FLOW,
    REQUEST_KIND_VALUES, START_EDGE_EVENT, WHEN_EVENT_VALUES,
};
pub use projects::{NewProject, Project, ORIGIN_FOLDER, ORIGIN_GITHUB};
pub use queued_requests::{PersistedFlowJob, QueuedRequestRecord};
pub use runs::{InterruptedRun, RunHandoff, RunMessage, RunProgress};
pub use learning::{
    LearningSuggestion, MemoryOriginType, NewLearningSuggestion, NewProjectMemory, ProjectMemory,
    ReviewMessageEntry, SkillScope, SuggestionKind, SuggestionOperation, SuggestionStatus,
};
pub use settings::{
    parse_usage_pause_threshold, GRANT_SESSION, LEARNING_MODE_ASK, LEARNING_MODE_AUTO,
    LEARNING_MODE_OFF, SETTING_ACTIVE_LLM, SETTING_AUTO_TRIAGE, SETTING_CONTINUATION_ENABLED,
    SETTING_CONTINUATION_MAX_COUNT, SETTING_LLM_FALLBACK, SETTING_PERMISSION_GRANT,
    SETTING_USAGE_PAUSE_THRESHOLD, SETTING_WEB_SEARCH,
};
pub use usage_analytics::{
    NewUsageRecord, UsageAnalyticsQuery, UsageAnalyticsResponse,
};
pub use work_steps::{
    load_recent_work_steps_safe, work_steps_session_key, NewTurnWorkSteps, TurnWorkStepsRecord,
};
#[cfg(test)]
pub use settings::{
    SETTING_AUTO_PAUSE_ON_LIMIT, SETTING_LEARNING_MODE, SETTING_LEARNING_TURN_CADENCE,
};
