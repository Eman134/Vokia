use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::claude::CliProblem;
use crate::events::Agent;

pub struct AppState {
    pub cli: Result<PathBuf, CliProblem>,
    pub sessions: Mutex<HashMap<Agent, String>>,
}
