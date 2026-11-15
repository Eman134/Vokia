use tauri::{AppHandle, Manager};

use crate::claude::{ConsoleKind, ConsoleLine};
use crate::events::VokiaEvent;
use crate::session_key::SessionKey;
use crate::state::AppState;

use super::emit::{emit, now_millis};
use super::error::FlowError;

const TURN_STARTED_TEXT: &str = "início do turno";
const TURN_DONE_TEXT: &str = "concluído";
const TURN_FAILED_PREFIX: &str = "falhou: ";

pub(super) struct ConsoleSink<'a> {
    app: &'a AppHandle,
    key: SessionKey,
}

impl<'a> ConsoleSink<'a> {
    pub(super) fn new(app: &'a AppHandle, key: SessionKey) -> Self {
        Self { app, key }
    }

    // O console é auxiliar: falha ao registrar ou emitir não pode derrubar o turno do agente.
    pub(super) fn record(&self, line: ConsoleLine) {
        let _ = self.try_record(line);
    }

    pub(super) fn started(&self) {
        self.record(ConsoleLine::new(ConsoleKind::Start, TURN_STARTED_TEXT));
    }

    pub(super) fn finished<T>(&self, result: &Result<T, FlowError>) {
        let line = match result {
            Ok(_) => ConsoleLine::new(ConsoleKind::Done, TURN_DONE_TEXT),
            Err(error) => {
                ConsoleLine::new(ConsoleKind::Failed, &format!("{TURN_FAILED_PREFIX}{error}"))
            }
        };
        self.record(line);
    }

    fn try_record(&self, line: ConsoleLine) -> Result<(), FlowError> {
        let time = now_millis()?;
        let entry = self
            .app
            .state::<AppState>()
            .console
            .push(self.key.clone(), line, time);
        emit(
            self.app,
            VokiaEvent::ConsoleLine {
                agent: self.key.agent.clone(),
                project_id: self.key.project_id,
                seq: entry.seq,
                kind: entry.kind,
                text: entry.text,
                time: entry.time,
            },
        )
    }
}
