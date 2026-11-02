use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

use crate::cache::Cache;
use crate::claude::CliProblem;
use crate::events::Agent;
use crate::orchestrator::SessionPool;
use crate::permissions::PermissionBroker;
use crate::session_key::SessionKey;
use crate::task::TaskRecord;

pub struct AppState {
    pub cli: Result<PathBuf, CliProblem>,
    pub cache: Mutex<HashMap<i64, Cache>>,
    pub flow: Mutex<FlowQueue>,
    pub sessions: SessionPool,
    pub archive: Mutex<Vec<TaskRecord>>,
    pub permissions: PermissionBroker,
    pub decisions: Mutex<HashMap<u64, PendingDecision>>,
    pub next_decision_id: AtomicU64,
}

pub struct PendingDecision {
    pub agent: Agent,
    pub project_id: i64,
    pub title: String,
    pub request: String,
}

pub struct QueuedJob {
    pub text: String,
    pub run_id: Option<u64>,
}

// Uma chave presente é um fluxo em andamento; os pedidos que chegam nesse meio-tempo esperam aqui.
#[derive(Default)]
pub struct FlowQueue {
    pub pending: HashMap<SessionKey, VecDeque<QueuedJob>>,
}
