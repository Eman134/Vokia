use tauri::{AppHandle, Manager};

use crate::claude::{ConsoleKind, ConsoleLine, TurnActivity};
use crate::events::VokiaEvent;
use crate::session_key::SessionKey;
use crate::state::AppState;

use super::emit::{emit, now_millis};
use super::error::FlowError;

const TURN_STARTED_TEXT: &str = "início do turno";
const TURN_PAUSED_TEXT: &str = "pausado: limite de uso atingido";
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

    // Só visual: não grava no banco, e a atividade final segue na mensagem do agente.
    pub(super) fn live_activity(&self, activity: TurnActivity) {
        let _ = emit(
            self.app,
            VokiaEvent::AgentActivity {
                agent: self.key.agent.clone(),
                project_id: self.key.project_id,
                activity,
            },
        );
    }

    pub(super) fn started(&self) {
        self.record(ConsoleLine::new(ConsoleKind::Start, TURN_STARTED_TEXT));
    }

    pub(super) fn finished<T>(&self, result: &Result<T, FlowError>) {
        let line = match result {
            Ok(_) => ConsoleLine::new(ConsoleKind::Done, TURN_DONE_TEXT),
            Err(error)
                if error.is_limit() && super::runs::auto_pause_on_limit(self.app) =>
            {
                ConsoleLine::new(ConsoleKind::Text, TURN_PAUSED_TEXT)
            }
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
