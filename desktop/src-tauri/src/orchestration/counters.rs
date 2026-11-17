use std::collections::HashMap;

use crate::agents::Agent;

// Vale só para uma tarefa: cada chamada de run_turns começa com contadores zerados.
#[derive(Default)]
pub(crate) struct TaskCounters {
    turns: i64,
    last_speaker: Option<Agent>,
    passages: HashMap<(Agent, Agent), i64>,
    returns: HashMap<(Agent, Agent), i64>,
}

impl TaskCounters {
    pub(crate) fn register_turn(&mut self, speaker: &Agent) {
        self.turns += 1;
        self.last_speaker = Some(speaker.clone());
    }

    pub(crate) fn turns(&self) -> i64 {
        self.turns
    }

    pub(crate) fn last_speaker(&self) -> Option<&Agent> {
        self.last_speaker.as_ref()
    }

    pub(crate) fn passages(&self, from: &Agent, to: &Agent) -> i64 {
        count(&self.passages, from, to)
    }

    pub(crate) fn returns(&self, from: &Agent, to: &Agent) -> i64 {
        count(&self.returns, from, to)
    }

    pub(crate) fn register_passage(&mut self, from: &Agent, to: &Agent) {
        increment(&mut self.passages, from, to);
    }

    pub(crate) fn register_return(&mut self, from: &Agent, to: &Agent) {
        increment(&mut self.returns, from, to);
    }
}

fn count(map: &HashMap<(Agent, Agent), i64>, from: &Agent, to: &Agent) -> i64 {
    map.get(&(from.clone(), to.clone())).copied().unwrap_or(0)
}

fn increment(map: &mut HashMap<(Agent, Agent), i64>, from: &Agent, to: &Agent) {
    *map.entry((from.clone(), to.clone())).or_insert(0) += 1;
}
