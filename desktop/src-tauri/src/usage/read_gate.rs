use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use super::model::ProviderPlanUsage;

pub const CACHE_TTL_MS: u64 = 4 * 60 * 1_000;
pub const BACKOFF_STEPS_MINS: [u64; 5] = [5, 10, 20, 40, 60];
const MS_PER_MINUTE: u64 = 60_000;

pub fn backoff_duration_ms(consecutive_failures: u32) -> u64 {
    if consecutive_failures == 0 {
        return 0;
    }
    let idx = (consecutive_failures.saturating_sub(1) as usize).min(BACKOFF_STEPS_MINS.len() - 1);
    BACKOFF_STEPS_MINS[idx].saturating_mul(MS_PER_MINUTE)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadOutcome {
    pub usage: Option<ProviderPlanUsage>,
    pub from_cache: bool,
    pub in_backoff: bool,
    pub backoff_until_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Default)]
struct ProviderReadState {
    last_valid: Option<ProviderPlanUsage>,
    fetched_at_ms: Option<u64>,
    consecutive_failures: u32,
    backoff_until_ms: Option<u64>,
    last_error: Option<String>,
    invalidated: bool,
}

impl ProviderReadState {
    fn is_in_backoff(&self, now_ms: u64) -> bool {
        self.backoff_until_ms.is_some_and(|until| now_ms < until)
    }

    fn has_fresh_cache(&self, now_ms: u64) -> bool {
        if self.invalidated {
            return false;
        }
        let Some(fetched_at) = self.fetched_at_ms else {
            return false;
        };
        if let Some(usage) = &self.last_valid {
            if usage.has_expired_window(now_ms) {
                return false;
            }
        }
        now_ms.saturating_sub(fetched_at) < CACHE_TTL_MS
    }

    fn stale_with_error(&self) -> Option<ProviderPlanUsage> {
        self.last_valid.clone().map(|mut usage| {
            usage.read_failed = true;
            usage
        })
    }
}

#[derive(Default)]
struct ReadGateInner {
    entries: HashMap<String, ProviderReadState>,
    in_flight: HashMap<String, watch::Receiver<Option<ReadOutcome>>>,
}

/// Controls quota reads per provider with single-flight deduplication, 4-min cache,
/// exponential error backoff (`5 -> 10 -> 20 -> 40 -> 60 min`), and immediate
/// invalidation when `now >= resetsAt` (outside error backoff) or on credential/config changes.
#[derive(Clone, Default)]
pub struct UsageReadGate {
    inner: Arc<Mutex<ReadGateInner>>,
}

struct InFlightGuard {
    inner: Arc<Mutex<ReadGateInner>>,
    provider_id: String,
    active: bool,
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if self.active {
            if let Ok(mut guard) = self.inner.lock() {
                guard.in_flight.remove(&self.provider_id);
            }
        }
    }
}

impl UsageReadGate {
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn read<F, Fut>(&self, provider_id: &str, now_ms: u64, fetch: F) -> ReadOutcome
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ProviderPlanUsage, String>>,
    {
        self.read_optional(provider_id, now_ms, move || async move {
            fetch().await.map(Some)
        })
        .await
    }

    pub async fn read_optional<F, Fut>(
        &self,
        provider_id: &str,
        now_ms: u64,
        fetch: F,
    ) -> ReadOutcome
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Option<ProviderPlanUsage>, String>>,
    {
        enum Action {
            Immediate(ReadOutcome),
            AwaitInFlight(watch::Receiver<Option<ReadOutcome>>),
            Fetch(watch::Sender<Option<ReadOutcome>>),
        }

        let action = {
            let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(entry) = inner.entries.get(provider_id) {
                if entry.is_in_backoff(now_ms) {
                    Action::Immediate(ReadOutcome {
                        usage: entry.stale_with_error(),
                        from_cache: true,
                        in_backoff: true,
                        backoff_until_ms: entry.backoff_until_ms,
                        error: entry.last_error.clone(),
                    })
                } else if entry.has_fresh_cache(now_ms) {
                    Action::Immediate(ReadOutcome {
                        usage: entry.last_valid.clone(),
                        from_cache: true,
                        in_backoff: false,
                        backoff_until_ms: None,
                        error: None,
                    })
                } else if let Some(rx) = inner.in_flight.get(provider_id) {
                    Action::AwaitInFlight(rx.clone())
                } else {
                    let (tx, rx) = watch::channel(None);
                    inner.in_flight.insert(provider_id.to_string(), rx);
                    Action::Fetch(tx)
                }
            } else if let Some(rx) = inner.in_flight.get(provider_id) {
                Action::AwaitInFlight(rx.clone())
            } else {
                let (tx, rx) = watch::channel(None);
                inner.in_flight.insert(provider_id.to_string(), rx);
                Action::Fetch(tx)
            }
        };

        match action {
            Action::Immediate(outcome) => outcome,
            Action::AwaitInFlight(mut rx) => {
                loop {
                    if let Some(outcome) = rx.borrow().clone() {
                        return outcome;
                    }
                    if rx.changed().await.is_err() {
                        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                        let usage = inner
                            .entries
                            .get(provider_id)
                            .and_then(|entry| entry.last_valid.clone());
                        return ReadOutcome {
                            usage,
                            from_cache: true,
                            in_backoff: false,
                            backoff_until_ms: None,
                            error: Some("Leitura concorrente de cota cancelada.".to_string()),
                        };
                    }
                }
            }
            Action::Fetch(tx) => {
                let mut cleanup = InFlightGuard {
                    inner: Arc::clone(&self.inner),
                    provider_id: provider_id.to_string(),
                    active: true,
                };
                let result = fetch().await;
                let outcome = {
                    let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
                    inner.in_flight.remove(provider_id);
                    cleanup.active = false;
                    let entry = inner.entries.entry(provider_id.to_string()).or_default();
                    match result {
                        Ok(Some(mut usage)) => {
                            usage.read_failed = false;
                            if usage.updated_at_ms == 0 {
                                usage.updated_at_ms = now_ms;
                            }
                            entry.last_valid = Some(usage.clone());
                            entry.fetched_at_ms = Some(now_ms);
                            entry.consecutive_failures = 0;
                            entry.backoff_until_ms = None;
                            entry.last_error = None;
                            entry.invalidated = false;
                            ReadOutcome {
                                usage: Some(usage),
                                from_cache: false,
                                in_backoff: false,
                                backoff_until_ms: None,
                                error: None,
                            }
                        }
                        Ok(None) => {
                            entry.last_valid = None;
                            entry.fetched_at_ms = Some(now_ms);
                            entry.consecutive_failures = 0;
                            entry.backoff_until_ms = None;
                            entry.last_error = None;
                            entry.invalidated = false;
                            ReadOutcome {
                                usage: None,
                                from_cache: false,
                                in_backoff: false,
                                backoff_until_ms: None,
                                error: None,
                            }
                        }
                        Err(err) => {
                            entry.consecutive_failures =
                                entry.consecutive_failures.saturating_add(1);
                            let wait_ms = backoff_duration_ms(entry.consecutive_failures);
                            let until = now_ms.saturating_add(wait_ms);
                            entry.backoff_until_ms = Some(until);
                            entry.last_error = Some(err.clone());
                            entry.invalidated = false;
                            if let Some(last) = &mut entry.last_valid {
                                last.read_failed = true;
                            }
                            ReadOutcome {
                                usage: entry.last_valid.clone(),
                                from_cache: false,
                                in_backoff: true,
                                backoff_until_ms: Some(until),
                                error: Some(err),
                            }
                        }
                    }
                };
                let _ = tx.send(Some(outcome.clone()));
                outcome
            }
        }
    }

    /// Records a passive quota update (for example from a Claude CLI `rate_limit_event`).
    pub fn record_push(&self, mut usage: ProviderPlanUsage) {
        usage.read_failed = false;
        let provider_id = usage.provider_id.clone();
        let fetched_at = usage.updated_at_ms;
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let entry = inner.entries.entry(provider_id).or_default();
        entry.last_valid = Some(usage);
        entry.fetched_at_ms = Some(fetched_at);
        entry.consecutive_failures = 0;
        entry.backoff_until_ms = None;
        entry.last_error = None;
        entry.invalidated = false;
    }

    /// Invalidates cached data and clears error backoff when provider credentials/config change.
    pub fn invalidate(&self, provider_id: &str) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = inner.entries.get_mut(provider_id) {
            entry.invalidated = true;
            entry.fetched_at_ms = None;
            entry.consecutive_failures = 0;
            entry.backoff_until_ms = None;
            entry.last_error = None;
        }
    }

    pub fn remove(&self, provider_id: &str) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.entries.remove(provider_id);
        inner.in_flight.remove(provider_id);
    }

    pub fn get(&self, provider_id: &str) -> Option<ProviderPlanUsage> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .entries
            .get(provider_id)
            .and_then(|entry| entry.last_valid.clone())
    }

    pub fn has_error(&self, provider_id: &str) -> bool {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner
            .entries
            .get(provider_id)
            .and_then(|entry| entry.last_error.as_ref())
            .is_some()
    }

    pub fn snapshot_all(&self) -> Vec<ProviderPlanUsage> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut list: Vec<ProviderPlanUsage> = inner
            .entries
            .values()
            .filter_map(|entry| entry.last_valid.clone())
            .collect();
        list.sort_by(|a, b| a.provider_id.cmp(&b.provider_id));
        list
    }
}
