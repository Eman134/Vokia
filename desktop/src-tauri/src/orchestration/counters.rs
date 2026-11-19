use std::collections::HashMap;

use crate::agents::Agent;

// Vale só para uma tarefa: cada chamada de run_turns começa com contadores zerados.
#[derive(Clone, Default)]
pub(crate) struct TaskCounters {
    turns: i64,
    last_speaker: Option<Agent>,
    passages: HashMap<i64, i64>,
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

    // Passagens já entregues numa ligação do fluxo (por id da ligação) nesta tarefa.
    pub(crate) fn passages(&self, edge_id: i64) -> i64 {
        self.passages.get(&edge_id).copied().unwrap_or(0)
    }

    pub(crate) fn returns(&self, from: &Agent, to: &Agent) -> i64 {
        self.returns
            .get(&(from.clone(), to.clone()))
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn register_passage(&mut self, edge_id: i64) {
        *self.passages.entry(edge_id).or_insert(0) += 1;
    }

    pub(crate) fn register_return(&mut self, from: &Agent, to: &Agent) {
        *self.returns.entry((from.clone(), to.clone())).or_insert(0) += 1;
    }
}
