use std::sync::atomic::{AtomicBool, Ordering};

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
    item_id: Option<String>,
    acted: AtomicBool,
}

impl<'a> ConsoleSink<'a> {
    pub(super) fn new(app: &'a AppHandle, key: SessionKey, item_id: Option<String>) -> Self {
        Self {
            app,
            key,
            item_id,
            acted: AtomicBool::new(false),
        }
    }

    pub(super) fn reset_acted(&self) {
        self.acted.store(false, Ordering::SeqCst);
    }

    pub(super) fn has_acted(&self) -> bool {
        self.acted.load(Ordering::SeqCst)
    }

    // O console é auxiliar: falha ao registrar ou emitir não pode derrubar o turno do agente.
    pub(super) fn record(&self, line: ConsoleLine) {
        if matches!(line.kind, ConsoleKind::Text | ConsoleKind::Tool) {
            self.acted.store(true, Ordering::SeqCst);
        }
        let _ = self.try_record(line);
    }

    // Só visual: não grava no banco, e a atividade final segue na mensagem do agente.
    pub(super) fn live_activity(&self, activity: TurnActivity) {
        if !activity.tools.is_empty() {
            self.acted.store(true, Ordering::SeqCst);
        }
        let _ = emit(
            self.app,
            VokiaEvent::AgentActivity {
                agent: self.key.agent.clone(),
                project_id: self.key.project_id,
                activity,
                item_id: self.item_id.clone(),
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
