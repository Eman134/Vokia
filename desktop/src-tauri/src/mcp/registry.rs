use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde_json::Value;

use crate::events::Agent;

use super::slot::ExchangeSlot;
use super::tools::ToolScope;

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
    pub slot: ExchangeSlot,
    pub scope: ToolScope,
}

#[derive(Debug)]
pub struct RandomnessUnavailable(getrandom::Error);

impl fmt::Display for RandomnessUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "o sistema não forneceu bytes aleatórios: {}", self.0)
    }
}

pub fn random_hex(byte_count: usize) -> Result<String, RandomnessUnavailable> {
    let mut bytes = vec![0u8; byte_count];
    getrandom::fill(&mut bytes).map_err(RandomnessUnavailable)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

#[derive(Default)]
pub struct Sessions(Mutex<HashMap<String, Binding>>);

impl Sessions {
    pub fn register(&self, binding: Binding) -> Result<Token, RandomnessUnavailable> {
        let token = Token(random_hex(TOKEN_BYTES)?);
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
    definitions: OnceLock<Vec<Value>>,
}

impl ToolRegistry {
    pub fn definitions(&self) -> &[Value] {
        self.definitions.get().map_or(&[], Vec::as_slice)
    }

    // Só a primeira instalação vale: as ferramentas não mudam com o servidor no ar.
    pub fn install(&self, definitions: Vec<Value>) {
        let _ = self.definitions.set(definitions);
    }

    pub fn definitions_for(&self, scope: ToolScope) -> Vec<Value> {
        self.definitions()
            .iter()
            .filter(|definition| scope.allows(name_of(definition)))
            .cloned()
            .collect()
    }

    pub fn names(&self, scope: ToolScope) -> impl Iterator<Item = &str> {
        self.definitions()
            .iter()
            .map(name_of)
            .filter(move |name| scope.allows(name))
    }
}

fn name_of(definition: &Value) -> &str {
    definition.get("name").and_then(Value::as_str).unwrap_or_default()
}
