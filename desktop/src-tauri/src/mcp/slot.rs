use std::sync::{Arc, Mutex, MutexGuard};

use crate::agents::Agent;
use crate::events::DecisionKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotDecision {
    pub kind: DecisionKind,
    pub title: String,
    pub detail: String,
    pub options: Vec<String>,
}

#[derive(Debug, Default)]
pub struct SlotState {
    pub item_id: Option<String>,
    pub channel_id: Option<i64>,
    pub confirmations: Vec<String>,
    pub changed: bool,
    pub decision: Option<SlotDecision>,
    pub delegation: Option<(Agent, String)>,
}

// O que a troca deixou para o orquestrador.
#[derive(Debug, Default)]
pub struct ExchangeOutcome {
    pub confirmations: Vec<String>,
    pub decision: Option<SlotDecision>,
    pub delegation: Option<(Agent, String)>,
}

impl ExchangeOutcome {
    // Várias tentativas da mesma pergunta (sessão que caiu e reabriu) somam o que cada uma deixou.
    pub fn absorb(&mut self, other: ExchangeOutcome) {
        self.confirmations.extend(other.confirmations);
        self.decision = other.decision.or(self.decision.take());
        self.delegation = other.delegation.or(self.delegation.take());
    }
}

// A sessão viva atravessa várias trocas; o slot guarda só o que muda de uma para a outra.
#[derive(Clone, Debug, Default)]
pub struct ExchangeSlot(Arc<Mutex<SlotState>>);

impl ExchangeSlot {
    pub fn begin(&self, item_id: Option<&str>, channel_id: Option<i64>) {
        *self.lock() = SlotState {
            item_id: item_id.map(str::to_string),
            channel_id,
            ..SlotState::default()
        };
    }

    pub fn end(&self) -> ExchangeOutcome {
        let mut state = self.lock();
        state.item_id = None;
        state.channel_id = None;
        state.changed = false;
        ExchangeOutcome {
            confirmations: std::mem::take(&mut state.confirmations),
            decision: state.decision.take(),
            delegation: state.delegation.take(),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, SlotState> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
