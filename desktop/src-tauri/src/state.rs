use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

use crate::cache::Cache;
use crate::claude::CliProblem;
use crate::events::Agent;
use crate::permissions::PermissionBroker;
use crate::task::TaskRecord;

pub struct AppState {
    pub cli: Result<PathBuf, CliProblem>,
    pub cache: Mutex<Cache>,
    pub flow: Mutex<FlowQueue>,
    pub archive: Mutex<Vec<TaskRecord>>,
    pub permissions: PermissionBroker,
    pub decisions: Mutex<HashMap<u64, PendingDecision>>,
    pub next_decision_id: AtomicU64,
}

pub struct PendingDecision {
    pub agent: Agent,
    pub title: String,
    pub request: String,
}

pub struct FlowQueue {
    pub active: bool,
    pub project_id: Option<i64>,
    pub pending: VecDeque<(String, Agent)>,
}
