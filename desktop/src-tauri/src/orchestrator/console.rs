use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use tauri::{AppHandle, Manager};

use crate::claude::{ConsoleKind, ConsoleLine, TurnActivity};
use crate::events::VokiaEvent;
use crate::orchestrator::turn_plan::{next_turn_id, TurnPlan, TurnPlanSnapshot};
use crate::session_key::SessionKey;
use crate::state::AppState;

use super::emit::{emit, now_millis};
use super::error::FlowError;

const TURN_STARTED_TEXT: &str = "início do turno";
const TURN_PAUSED_TEXT: &str = "pausado: limite de uso atingido";
const TURN_DONE_TEXT: &str = "concluído";
const TURN_FAILED_PREFIX: &str = "falhou: ";

pub(crate) struct ConsoleSink<'a> {
    app: &'a AppHandle,
    key: SessionKey,
    item_id: Option<String>,
    turn_id: String,
    acted: AtomicBool,
    last_plan: Mutex<Option<TurnPlan>>,
    last_activity: Mutex<Option<TurnActivity>>,
}

impl<'a> ConsoleSink<'a> {
    pub(super) fn new(app: &'a AppHandle, key: SessionKey, item_id: Option<String>) -> Self {
        Self {
            app,
            key,
            item_id,
            turn_id: next_turn_id(),
            acted: AtomicBool::new(false),
            last_plan: Mutex::new(None),
            last_activity: Mutex::new(None),
        }
    }

    pub(super) fn reset_acted(&self) {
        self.acted.store(false, Ordering::SeqCst);
        if let Ok(mut guard) = self.last_activity.lock() {
            *guard = None;
        }
    }

    pub(super) fn has_acted(&self) -> bool {
        self.acted.load(Ordering::SeqCst)
    }

    pub(super) fn take_last_activity(&self) -> Option<TurnActivity> {
        self.last_activity
            .lock()
            .ok()
            .and_then(|mut guard| guard.take())
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
        if let Some(plan) = activity.plan.clone() {
            self.turn_plan(plan);
        }
        if let Ok(mut guard) = self.last_activity.lock() {
            *guard = Some(activity.clone());
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

    // Publica e armazena em memória o plano vivo normalizado do turno, evitando reemissões idênticas.
    pub(super) fn turn_plan(&self, plan: TurnPlan) {
        let normalized = plan.normalize();
        if normalized.is_empty() {
            return;
        }
        {
            let mut guard = self
                .last_plan
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if guard.as_ref() == Some(&normalized) {
                return;
            }
            *guard = Some(normalized.clone());
        }
        let updated_at = now_millis().unwrap_or(0);
        let snapshot = TurnPlanSnapshot {
            project_id: self.key.project_id,
            agent: self.key.agent.clone(),
            card_id: self.item_id.clone(),
            turn_id: self.turn_id.clone(),
            plan: normalized.clone(),
            updated_at,
            active: true,
        };
        self.app.state::<AppState>().turn_plans.upsert(snapshot);
        let _ = emit(
            self.app,
            VokiaEvent::TurnPlanUpdated {
                project_id: self.key.project_id,
                agent: self.key.agent.clone(),
                card_id: self.item_id.clone(),
                turn_id: self.turn_id.clone(),
                plan: normalized,
            },
        );
    }

    pub(super) fn session_key(&self) -> &SessionKey {
        &self.key
    }

    pub(super) fn provider_usage(&self, usage: crate::usage::ProviderPlanUsage) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.usage.record_provider_usage(usage.clone());
            let exhausted = state
                .usage
                .pause_threshold()
                .is_some_and(|threshold| usage.is_exhausted(threshold));
            if !exhausted {
                let released = state.usage.confirm_provider_renewed(&usage.provider_id, None);
                if !released.returns.is_empty() {
                    let mut overrides = state
                        .temporary_overrides
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    let mut changed = false;
                    for ret in released.returns {
                        let should_remove = overrides.get(&ret.agent).is_some_and(|ov| {
                            ov.reason == crate::orchestrator::fallback::OverrideReason::UsageLimit
                                && ov.original_cli == ret.original_provider_id
                        });
                        if should_remove {
                            overrides.remove(&ret.agent);
                            changed = true;
                        }
                    }
                    drop(overrides);
                    if changed {
                        crate::orchestrator::fallback::emit_overrides_changed(self.app, &state);
                    }
                }
            }
        }
        let _ = emit(self.app, VokiaEvent::ProviderUsageUpdated { usage });
    }

    pub(super) fn started(&self) {
        self.record(ConsoleLine::new(ConsoleKind::Start, TURN_STARTED_TEXT));
    }

    pub(super) fn finished<T>(&self, result: &Result<T, FlowError>) {
        self.app.state::<AppState>().turn_plans.mark_turn_finished(
            self.key.project_id,
            &self.key.agent,
            &self.turn_id,
        );
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
