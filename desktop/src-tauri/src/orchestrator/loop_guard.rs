use std::collections::HashMap;

use crate::agents::{Agent, Team};

pub(super) const LOOP_REPEAT_LIMIT: u32 = 3;
pub(super) const LOOP_MESSAGE: &str = "A conversa entre os agentes parou: a mesma mensagem se repetiu em turnos seguidos sem avançar a tarefa.";
pub(super) const PASS_REPEAT_LIMIT: u32 = 3;
pub(super) const SEM_AVANCO_LIMIT: u32 = 3;

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

// Por par (quem repassa, para quem): uma coordenadora que delega a ajudantes diferentes não é loop.
// Vive só dentro de run_turns, então nasce e morre com a tarefa.
#[derive(Default)]
pub(super) struct PassCounter {
    pairs: Vec<(Agent, Agent, u32)>,
}

impl PassCounter {
    pub(super) fn register(&mut self, from: &Agent, to: &Agent) -> u32 {
        if let Some(pair) = self
            .pairs
            .iter_mut()
            .find(|(pair_from, pair_to, _)| pair_from == from && pair_to == to)
        {
            pair.2 += 1;
            return pair.2;
        }
        self.pairs.push((from.clone(), to.clone(), 1));
        1
    }

    pub(super) fn by_speaker(&self, from: &Agent) -> u32 {
        self.pairs
            .iter()
            .filter(|(pair_from, _, _)| pair_from == from)
            .map(|(_, _, count)| count)
            .sum()
    }

    pub(super) fn reset(&mut self) {
        self.pairs.clear();
    }

    pub(super) fn history(&self, team: &Team) -> String {
        self.pairs
            .iter()
            .map(|(from, to, count)| {
                format!("{} → {} {count}", team.name_of(from), team.name_of(to))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub(super) fn pass_limit_message(
    counter: &PassCounter,
    from: &Agent,
    to: &Agent,
    team: &Team,
) -> String {
    format!(
        "A tarefa parou: {} repassou para {} {PASS_REPEAT_LIMIT} vezes sem o estado avançar. Repasses desde o último avanço: {}.",
        team.name_of(from),
        team.name_of(to),
        counter.history(team),
    )
}

pub(super) fn idle_limit_message(step: usize, from: &Agent, to: &Agent, team: &Team) -> String {
    format!(
        "A tarefa parou: sem avanço desde o passo {step}, par {} → {}.",
        team.name_of(from),
        team.name_of(to),
    )
}

#[cfg(test)]
#[path = "../../tests/loop_guard_tests.rs"]
mod tests;
