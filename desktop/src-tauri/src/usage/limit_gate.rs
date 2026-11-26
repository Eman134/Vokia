use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::agents::Agent;
use crate::providers::ModelTier;

pub const RESET_GRACE_SECS: u64 = 30;
pub const UNKNOWN_RESET_RECHECK_SECS: u64 = 15 * 60;
const MS_PER_SECOND: u64 = 1_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitEntry {
    pub provider_id: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    pub release_at_ms: u64,
    pub marked_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitReturn {
    pub agent: Agent,
    pub original_provider_id: String,
    pub original_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_tier: Option<ModelTier>,
    pub fallback_provider_id: String,
    pub fallback_model: String,
    pub release_at_ms: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReleasedLimits {
    pub released_keys: Vec<(String, String)>,
    pub returns: Vec<UsageLimitReturn>,
}

/// Tracks rate-limited `(provider_id, model)` pairs independently so hitting a quota
/// on one provider/model never stalls agents running on another, and records
/// `usage_limit_return` entries so agents automatically return to their original model
/// once `resets_at + 30s` (or a 15-min recheck when reset time is unknown) elapses.
pub struct UsageLimitGate {
    limits: HashMap<(String, String), LimitEntry>,
    returns: HashMap<Agent, UsageLimitReturn>,
    pause_threshold: Option<u8>,
}

impl Default for UsageLimitGate {
    fn default() -> Self {
        Self {
            limits: HashMap::new(),
            returns: HashMap::new(),
            pause_threshold: Some(super::DEFAULT_PAUSE_THRESHOLD),
        }
    }
}

impl UsageLimitGate {
    pub fn set_pause_threshold(&mut self, threshold: Option<u8>) {
        self.pause_threshold = threshold;
    }

    pub fn pause_threshold(&self) -> Option<u8> {
        self.pause_threshold
    }
    pub fn compute_release_at_ms(resets_at_secs: Option<u64>, now_ms: u64) -> u64 {
        match resets_at_secs.filter(|&ts| ts > 0) {
            Some(resets_at) => {
                let target = resets_at
                    .saturating_add(RESET_GRACE_SECS)
                    .saturating_mul(MS_PER_SECOND);
                if target > now_ms {
                    target
                } else {
                    now_ms.saturating_add(RESET_GRACE_SECS.saturating_mul(MS_PER_SECOND))
                }
            }
            None => now_ms.saturating_add(UNKNOWN_RESET_RECHECK_SECS.saturating_mul(MS_PER_SECOND)),
        }
    }

    /// Transparent failover is allowed only when the turn has not produced text or executed
    /// any tool yet (`!acted`) and at least one backup model is available.
    pub fn can_failover(acted: bool, has_backup: bool) -> bool {
        !acted && has_backup
    }

    pub fn mark_reached(
        &mut self,
        provider_id: &str,
        model: &str,
        resets_at_secs: Option<u64>,
        now_ms: u64,
    ) -> LimitEntry {
        let resets_at = resets_at_secs.filter(|&ts| ts > 0);
        let release_at_ms = Self::compute_release_at_ms(resets_at, now_ms);
        let entry = LimitEntry {
            provider_id: provider_id.to_string(),
            model: model.to_string(),
            resets_at,
            release_at_ms,
            marked_at_ms: now_ms,
        };
        self.limits
            .insert((provider_id.to_string(), model.to_string()), entry.clone());
        entry
    }

    pub fn is_limited(&mut self, provider_id: &str, model: &str, now_ms: u64) -> bool {
        let key = (provider_id.to_string(), model.to_string());
        if let Some(entry) = self.limits.get(&key) {
            if now_ms < entry.release_at_ms {
                return true;
            }
            self.limits.remove(&key);
        }
        false
    }

    pub fn register_return(&mut self, entry: UsageLimitReturn) {
        self.returns.insert(entry.agent.clone(), entry);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_return(&self, agent: &Agent) -> Option<UsageLimitReturn> {
        self.returns.get(agent).cloned()
    }

    pub fn clear_return(&mut self, agent: &Agent) -> Option<UsageLimitReturn> {
        self.returns.remove(agent)
    }

    pub fn release_expired(&mut self, now_ms: u64) -> ReleasedLimits {
        let mut released_keys = Vec::new();
        self.limits.retain(|key, entry| {
            if now_ms >= entry.release_at_ms {
                released_keys.push(key.clone());
                false
            } else {
                true
            }
        });

        let mut returns = Vec::new();
        let due_agents: Vec<Agent> = self
            .returns
            .iter()
            .filter(|(_, ret)| {
                let key = (ret.original_provider_id.clone(), ret.original_model.clone());
                now_ms >= ret.release_at_ms && !self.limits.contains_key(&key)
            })
            .map(|(agent, _)| agent.clone())
            .collect();

        for agent in due_agents {
            if let Some(ret) = self.returns.remove(&agent) {
                returns.push(ret);
            }
        }

        ReleasedLimits {
            released_keys,
            returns,
        }
    }

    pub fn confirm_renewed(
        &mut self,
        provider_id: &str,
        model: Option<&str>,
    ) -> ReleasedLimits {
        let mut released_keys = Vec::new();
        self.limits.retain(|key, _| {
            let matches = key.0 == provider_id && model.is_none_or(|m| key.1 == m);
            if matches {
                released_keys.push(key.clone());
                false
            } else {
                true
            }
        });

        let due_agents: Vec<Agent> = self
            .returns
            .iter()
            .filter(|(_, ret)| {
                ret.original_provider_id == provider_id
                    && model.is_none_or(|m| ret.original_model == m)
            })
            .map(|(agent, _)| agent.clone())
            .collect();

        let mut returns = Vec::new();
        for agent in due_agents {
            if let Some(ret) = self.returns.remove(&agent) {
                returns.push(ret);
            }
        }

        ReleasedLimits {
            released_keys,
            returns,
        }
    }

    pub fn active_limits(&self, now_ms: u64) -> Vec<LimitEntry> {
        let mut list: Vec<LimitEntry> = self
            .limits
            .values()
            .filter(|entry| now_ms < entry.release_at_ms)
            .cloned()
            .collect();
        list.sort_by(|a, b| {
            a.provider_id
                .cmp(&b.provider_id)
                .then_with(|| a.model.cmp(&b.model))
        });
        list
    }

    pub fn pending_returns(&self) -> Vec<UsageLimitReturn> {
        let mut list: Vec<UsageLimitReturn> = self.returns.values().cloned().collect();
        list.sort_by(|a, b| a.agent.as_str().cmp(b.agent.as_str()));
        list
    }
}
