use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};

use serde::Serialize;

use crate::claude::{ConsoleKind, ConsoleLine};
use crate::session_key::SessionKey;

const CONSOLE_BUFFER_LINES: usize = 500;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleEntry {
    pub seq: u64,
    pub kind: ConsoleKind,
    pub text: String,
    pub time: u64,
}

#[derive(Default)]
struct Consoles {
    next_seq: u64,
    by_key: HashMap<SessionKey, VecDeque<ConsoleEntry>>,
}

// O seq é global e nunca volta: o front junta o buffer e o evento ao vivo por seq sem repetir linha.
#[derive(Default)]
pub struct ConsoleLog {
    consoles: Mutex<Consoles>,
}

impl ConsoleLog {
    pub fn push(&self, key: SessionKey, line: ConsoleLine, time: u64) -> ConsoleEntry {
        let mut consoles = self.consoles.lock().unwrap_or_else(PoisonError::into_inner);
        consoles.next_seq += 1;
        let entry = ConsoleEntry {
            seq: consoles.next_seq,
            kind: line.kind,
            text: line.text,
            time,
        };
        let entries = consoles.by_key.entry(key).or_default();
        if entries.len() == CONSOLE_BUFFER_LINES {
            entries.pop_front();
        }
        entries.push_back(entry.clone());
        entry
    }

    pub fn snapshot(&self, key: &SessionKey) -> Vec<ConsoleEntry> {
        let consoles = self.consoles.lock().unwrap_or_else(PoisonError::into_inner);
        consoles
            .by_key
            .get(key)
            .map(|entries| entries.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn forget_project(&self, project_id: i64) {
        self.consoles
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .by_key
            .retain(|key, _| key.project_id != project_id);
    }
}
