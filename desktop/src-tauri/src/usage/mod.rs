pub mod limit_gate;
pub mod model;
pub mod read_gate;

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

pub type ReaderFuture = Pin<Box<dyn Future<Output = Result<ProviderPlanUsage, String>> + Send>>;
pub type UsageReaderFn = Arc<dyn Fn() -> ReaderFuture + Send + Sync>;

pub fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[derive(Default)]
pub struct UsageEngine {
    read_gate: UsageReadGate,
    limit_gate: Mutex<UsageLimitGate>,
    readers: Mutex<HashMap<String, UsageReaderFn>>,
}

impl UsageEngine {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn register_reader(&self, provider_id: impl Into<String>, reader: UsageReaderFn) {
        let mut readers = self.readers.lock().unwrap_or_else(|e| e.into_inner());
        readers.insert(provider_id.into(), reader);
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
            .read(provider_id, now_ms, move || (reader)())
            .await
    }

    pub async fn refresh_all(
        &self,
        provider_filter: Option<&str>,
        now_ms: u64,
    ) -> Vec<ReadOutcome> {
        let target_ids: Vec<String> = if let Some(provider_id) = provider_filter {
            vec![provider_id.to_string()]
        } else {
            let readers = self.readers.lock().unwrap_or_else(|e| e.into_inner());
            let mut ids: Vec<String> = readers.keys().cloned().collect();
            ids.sort();
            ids
        };

        let mut outcomes = Vec::with_capacity(target_ids.len());
        for provider_id in target_ids {
            outcomes.push(self.refresh_provider(&provider_id, now_ms).await);
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
