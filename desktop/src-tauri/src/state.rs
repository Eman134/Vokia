use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::cache::Cache;
use crate::claude::CliProblem;
use crate::events::Agent;
use crate::task::TaskRecord;

pub struct AppState {
    pub cli: Result<PathBuf, CliProblem>,
    pub cache: Mutex<Cache>,
    pub flow: Mutex<FlowQueue>,
    pub archive: Mutex<Vec<TaskRecord>>,
}

pub struct FlowQueue {
    pub active: bool,
    pub pending: VecDeque<(String, Agent)>,
}
