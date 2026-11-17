use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug, Default)]
pub struct SlotState {
    pub item_id: Option<String>,
    pub channel_id: Option<i64>,
    pub confirmations: Vec<String>,
    pub changed: bool,
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

    // Confirmações e `changed` ficam: o orquestrador as lê depois do envio.
    pub fn end(&self) {
        let mut state = self.lock();
        state.item_id = None;
        state.channel_id = None;
    }

    pub fn lock(&self) -> MutexGuard<'_, SlotState> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
