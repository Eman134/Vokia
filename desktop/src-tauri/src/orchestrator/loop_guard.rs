use std::collections::HashMap;

use crate::agents::Agent;

pub(super) const LOOP_REPEAT_LIMIT: u32 = 3;
pub(super) const LOOP_MESSAGE: &str = "A conversa entre os agentes parou: a mesma mensagem se repetiu em turnos seguidos sem avançar a tarefa.";
pub(super) const STALL_LIMIT: u32 = 3;

#[derive(Default)]
pub(super) struct LoopGuard {
    last: HashMap<Agent, (String, u32)>,
}

impl LoopGuard {
    pub(super) fn register(&mut self, agent: Agent, message: &str) -> u32 {
        let normalized = normalize_message(message);
        let (last, repeats) = self
            .last
            .entry(agent)
            .or_insert_with(|| (normalized.clone(), 0));
        if *last == normalized {
            *repeats += 1;
        } else {
            *last = normalized;
            *repeats = 1;
        }
        *repeats
    }
}

fn normalize_message(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(super) fn stall_message() -> String {
    format!("A tarefa parou: o estado não avançou em {STALL_LIMIT} turnos seguidos.")
}
