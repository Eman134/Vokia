use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard};

use serde_json::Value;

use crate::events::Agent;

const TOKEN_BYTES: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token(String);

impl Token {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct Binding {
    pub project_id: i64,
    pub agent: Agent,
    pub agent_name: String,
}

#[derive(Debug)]
pub struct RandomnessUnavailable(getrandom::Error);

impl fmt::Display for RandomnessUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "o sistema não forneceu bytes aleatórios: {}", self.0)
    }
}

#[derive(Default)]
pub struct Sessions(Mutex<HashMap<String, Binding>>);

impl Sessions {
    pub fn register(&self, binding: Binding) -> Result<Token, RandomnessUnavailable> {
        let mut bytes = [0u8; TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(RandomnessUnavailable)?;
        let token = Token(bytes.iter().map(|byte| format!("{byte:02x}")).collect());
        self.lock().insert(token.0.clone(), binding);
        Ok(token)
    }

    pub fn revoke(&self, token: &Token) {
        self.lock().remove(&token.0);
    }

    // Percorre todos os tokens sem parar no primeiro acerto, para o tempo não revelar qual casou.
    pub fn find(&self, presented: &str) -> Option<Binding> {
        let sessions = self.lock();
        let mut found = None;
        for (token, binding) in sessions.iter() {
            if constant_time_eq(token.as_bytes(), presented.as_bytes()) {
                found = Some(binding.clone());
            }
        }
        found
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<String, Binding>> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .fold(0u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

#[derive(Default)]
pub struct ToolRegistry {
    definitions: Vec<Value>,
}

impl ToolRegistry {
    pub fn definitions(&self) -> &[Value] {
        &self.definitions
    }
}
