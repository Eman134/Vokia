pub mod limit_gate;
pub mod model;
pub mod read_gate;
pub mod readers;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::agents::Agent;

pub use limit_gate::{LimitEntry, ReleasedLimits, UsageLimitGate, UsageLimitReturn};
pub use model::{
    PlanUsageSnapshot, ProviderPlanUsage, DEFAULT_PAUSE_THRESHOLD, DEFAULT_WARN_THRESHOLD,
};
pub use read_gate::{ReadOutcome, UsageReadGate};
pub use readers::UsageReadError;

#[cfg_attr(not(test), allow(dead_code))]
pub type ReaderFuture = Pin<Box<dyn Future<Output = Result<ProviderPlanUsage, String>> + Send>>;
#[cfg_attr(not(test), allow(dead_code))]
pub type UsageReaderFn = Arc<dyn Fn() -> ReaderFuture + Send + Sync>;

pub type OptionalReaderFuture =
    Pin<Box<dyn Future<Output = Result<Option<ProviderPlanUsage>, UsageReadError>> + Send>>;
pub type OptionalUsageReaderFn = Arc<dyn Fn() -> OptionalReaderFuture + Send + Sync>;

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[derive(Default)]
pub struct UsageEngine {
    read_gate: UsageReadGate,
    limit_gate: Mutex<UsageLimitGate>,
    readers: Mutex<HashMap<String, OptionalUsageReaderFn>>,
}

impl UsageEngine {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn register_reader(&self, provider_id: impl Into<String>, reader: UsageReaderFn) {
        let wrapped: OptionalUsageReaderFn = Arc::new(move || {
            let fut = (reader)();
            Box::pin(async move {
                fut.await
                    .map(Some)
                    .map_err(UsageReadError::from)
            })
        });
        self.register_optional_reader(provider_id, wrapped);
    }

    pub fn register_optional_reader(
        &self,
        provider_id: impl Into<String>,
        reader: OptionalUsageReaderFn,
    ) {
        let mut readers = self.readers.lock().unwrap_or_else(|e| e.into_inner());
        readers.insert(provider_id.into(), reader);
    }

    pub fn read_gate(&self) -> UsageReadGate {
        self.read_gate.clone()
    }

    pub fn record_provider_usage(&self, usage: ProviderPlanUsage) {
        self.read_gate.record_push(usage);
    }

    pub fn invalidate_provider(&self, provider_id: &str) {
        self.read_gate.invalidate(provider_id);
    }

    pub fn remove_provider(&self, provider_id: &str) {
        self.read_gate.remove(provider_id);
    }

    pub fn is_provider_using_credits(&self, provider_id: &str) -> bool {
        self.read_gate
            .get(provider_id)
            .is_some_and(|usage| usage.is_using_credits())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn refresh_provider(&self, provider_id: &str, now_ms: u64) -> ReadOutcome {
        let reader = {
            let readers = self.readers.lock().unwrap_or_else(|e| e.into_inner());
            readers.get(provider_id).cloned()
        };
        let Some(reader) = reader else {
            return ReadOutcome {
                usage: self.read_gate.get(provider_id),
                from_cache: true,
                in_backoff: false,
                backoff_until_ms: None,
                error: None,
            };
        };
        self.read_gate
            .read_optional(provider_id, now_ms, move || async move {
                (reader)().await.map_err(|err| err.to_string())
            })
            .await
    }

    pub async fn refresh_all(
        &self,
        provider_filter: Option<&str>,
        now_ms: u64,
    ) -> Vec<ReadOutcome> {
        let targets: Vec<(String, Option<OptionalUsageReaderFn>)> = {
            let readers = self.readers.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(provider_id) = provider_filter {
                vec![(provider_id.to_string(), readers.get(provider_id).cloned())]
            } else {
                let mut ids: Vec<String> = readers.keys().cloned().collect();
                ids.sort();
                ids.into_iter()
                    .map(|id| {
                        let reader = readers.get(&id).cloned();
                        (id, reader)
                    })
                    .collect()
            }
        };

        let mut handles = Vec::with_capacity(targets.len());
        for (provider_id, reader) in targets {
            let gate = self.read_gate.clone();
            handles.push(tokio::spawn(async move {
                let Some(reader) = reader else {
                    return ReadOutcome {
                        usage: gate.get(&provider_id),
                        from_cache: true,
                        in_backoff: false,
                        backoff_until_ms: None,
                        error: None,
                    };
                };
                gate.read_optional(&provider_id, now_ms, move || async move {
                    (reader)().await.map_err(|err| err.to_string())
                })
                .await
            }));
        }

        let mut outcomes = Vec::with_capacity(handles.len());
        for handle in handles {
            if let Ok(outcome) = handle.await {
                outcomes.push(outcome);
            }
        }
        outcomes
    }

    pub fn snapshot(&self, provider_filter: Option<&str>, now_ms: u64) -> PlanUsageSnapshot {
        let mut providers = self.read_gate.snapshot_all();
        if let Some(filter) = provider_filter {
            providers.retain(|entry| entry.provider_id == filter);
        }
        let (mut active_limits, mut pending_returns) = {
            let gate = self.limit_gate.lock().unwrap_or_else(|e| e.into_inner());
            (gate.active_limits(now_ms), gate.pending_returns())
        };
        if let Some(filter) = provider_filter {
            active_limits.retain(|entry| entry.provider_id == filter);
            pending_returns.retain(|entry| {
                entry.original_provider_id == filter || entry.fallback_provider_id == filter
            });
        }
        PlanUsageSnapshot {
            providers,
            active_limits,
            pending_returns,
            warn_threshold: DEFAULT_WARN_THRESHOLD,
            pause_threshold: DEFAULT_PAUSE_THRESHOLD,
            fetched_at_ms: now_ms,
        }
    }

    pub fn is_model_limited(&self, provider_id: &str, model: &str, now_ms: u64) -> bool {
        let mut gate = self.limit_gate.lock().unwrap_or_else(|e| e.into_inner());
        gate.is_limited(provider_id, model, now_ms)
    }

    pub fn mark_limit_reached(
        &self,
        provider_id: &str,
        model: &str,
        resets_at_secs: Option<u64>,
        now_ms: u64,
    ) -> LimitEntry {
        let mut gate = self.limit_gate.lock().unwrap_or_else(|e| e.into_inner());
        gate.mark_reached(provider_id, model, resets_at_secs, now_ms)
    }

    pub fn register_limit_return(&self, entry: UsageLimitReturn) {
        let mut gate = self.limit_gate.lock().unwrap_or_else(|e| e.into_inner());
        gate.register_return(entry);
    }

    pub fn clear_limit_return(&self, agent: &Agent) -> Option<UsageLimitReturn> {
        let mut gate = self.limit_gate.lock().unwrap_or_else(|e| e.into_inner());
        gate.clear_return(agent)
    }

    pub fn release_expired_limits(&self, now_ms: u64) -> ReleasedLimits {
        let mut gate = self.limit_gate.lock().unwrap_or_else(|e| e.into_inner());
        gate.release_expired(now_ms)
    }

    pub fn confirm_provider_renewed(
        &self,
        provider_id: &str,
        model: Option<&str>,
    ) -> ReleasedLimits {
        let mut gate = self.limit_gate.lock().unwrap_or_else(|e| e.into_inner());
        gate.confirm_renewed(provider_id, model)
    }
}

#[cfg(test)]
#[path = "../../tests/usage_engine_tests.rs"]
mod tests;
