use serde::{Deserialize, Serialize};

use crate::claude::{LimitWindow, RateLimits};

use super::limit_gate::{LimitEntry, UsageLimitReturn};

pub const FIVE_HOUR_MINS: u32 = 5 * 60;
pub const DAILY_MINS: u32 = 24 * 60;
pub const WEEKLY_MINS: u32 = 7 * 24 * 60;
pub const MONTHLY_MINS: u32 = 30 * 24 * 60;

pub const DEFAULT_WARN_THRESHOLD: u8 = 75;
pub const DEFAULT_PAUSE_THRESHOLD: u8 = 90;

const PERCENT_SCALE: f64 = 100.0;
const MS_PER_SECOND: u64 = 1_000;
const MS_PER_MINUTE: u64 = 60_000;

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UsageWindowKind {
    FiveHour,
    Daily,
    Weekly,
    Monthly,
    Custom(u32),
}

impl UsageWindowKind {
    /// Classifies a window duration in minutes using a ±5% tolerance against standard windows.
    pub fn classify(duration_mins: u32) -> Self {
        if within_five_percent(duration_mins, FIVE_HOUR_MINS) {
            Self::FiveHour
        } else if within_five_percent(duration_mins, DAILY_MINS) {
            Self::Daily
        } else if within_five_percent(duration_mins, WEEKLY_MINS) {
            Self::Weekly
        } else if within_five_percent(duration_mins, MONTHLY_MINS) {
            Self::Monthly
        } else {
            Self::Custom(duration_mins)
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn label(self) -> String {
        match self {
            Self::FiveHour => "5 h".to_string(),
            Self::Daily => "diária".to_string(),
            Self::Weekly => "semanal".to_string(),
            Self::Monthly => "mensal".to_string(),
            Self::Custom(mins) if mins >= 60 && mins % 60 == 0 => format!("{} h", mins / 60),
            Self::Custom(mins) => format!("{mins} min"),
        }
    }
}

fn within_five_percent(actual: u32, target: u32) -> bool {
    let min = target * 95 / 100;
    let max = target * 105 / 100;
    actual >= min && actual <= max
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PaceStatus {
    Sustainable,
    AbovePace,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    /// Percentage of quota used (0..=100).
    pub percent: u8,
    /// Unix timestamp in seconds when this window resets (0 when unknown).
    pub resets_at: u64,
    /// Total duration of the window in minutes.
    pub duration_mins: u32,
    /// Automatically classified window kind (±5% tolerance).
    pub kind: UsageWindowKind,
    /// Optional label override supplied by a provider reader.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// True when the provider explicitly rejected execution on this window.
    #[serde(default, skip_serializing_if = "is_false")]
    pub rejected: bool,
}

impl UsageWindow {
    pub fn new(percent: u8, resets_at: u64, duration_mins: u32) -> Self {
        Self {
            percent: percent.min(100),
            resets_at,
            duration_mins,
            kind: UsageWindowKind::classify(duration_mins),
            label: None,
            rejected: false,
        }
    }

    pub fn from_claude_limit(window: LimitWindow, duration_mins: u32) -> Self {
        let percent = (window.utilization * PERCENT_SCALE)
            .round()
            .clamp(0.0, PERCENT_SCALE) as u8;
        let mut parsed = Self::new(percent, window.resets_at, duration_mins);
        parsed.rejected = window.rejected;
        parsed
    }

    pub fn remaining_percent(&self) -> u8 {
        100u8.saturating_sub(self.percent.min(100))
    }

    /// Returns the percentage of window time still remaining: `(resetsAt - now) / duration * 100`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn pace_remaining_percent(&self, now_ms: u64) -> Option<f64> {
        if self.duration_mins == 0 || self.resets_at == 0 {
            return None;
        }
        let resets_at_ms = self.resets_at.saturating_mul(MS_PER_SECOND);
        let duration_ms = u64::from(self.duration_mins).saturating_mul(MS_PER_MINUTE);
        if duration_ms == 0 {
            return None;
        }
        if now_ms >= resets_at_ms {
            return Some(0.0);
        }
        let remaining_ms = resets_at_ms.saturating_sub(now_ms);
        let ratio = (remaining_ms as f64 / duration_ms as f64).clamp(0.0, 1.0);
        Some(ratio * PERCENT_SCALE)
    }

    /// Returns the expected used percentage for a uniform consumption pace (`100 - pace_remaining_percent`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn expected_used_percent(&self, now_ms: u64) -> Option<f64> {
        self.pace_remaining_percent(now_ms)
            .map(|remaining| (PERCENT_SCALE - remaining).clamp(0.0, PERCENT_SCALE))
    }

    /// Compares remaining quota against remaining window time.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn pace_status(&self, now_ms: u64) -> Option<PaceStatus> {
        let time_remaining = self.pace_remaining_percent(now_ms)?;
        let quota_remaining = f64::from(self.remaining_percent());
        if quota_remaining < time_remaining {
            Some(PaceStatus::AbovePace)
        } else {
            Some(PaceStatus::Sustainable)
        }
    }

    pub fn is_expired(&self, now_ms: u64) -> bool {
        self.resets_at > 0 && now_ms >= self.resets_at.saturating_mul(MS_PER_SECOND)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUsageGroup {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub models: Vec<String>,
    pub window: UsageWindow,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPlanUsage {
    pub provider_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary: Option<UsageWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary: Option<UsageWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tertiary: Option<UsageWindow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ProviderUsageGroup>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credits_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_credits_available: Option<u32>,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub read_failed: bool,
}

impl ProviderPlanUsage {
    pub fn from_claude_rate_limits(limits: RateLimits, now_ms: u64) -> Self {
        Self {
            provider_id: "claude".to_string(),
            label: Some("Claude Code".to_string()),
            primary: Some(UsageWindow::from_claude_limit(
                limits.five_hour,
                FIVE_HOUR_MINS,
            )),
            secondary: Some(UsageWindow::from_claude_limit(
                limits.seven_day,
                WEEKLY_MINS,
            )),
            tertiary: None,
            groups: Vec::new(),
            credits_available: None,
            reset_credits_available: None,
            updated_at_ms: now_ms,
            read_failed: false,
        }
    }

    pub fn all_windows(&self) -> Vec<&UsageWindow> {
        let mut windows = Vec::new();
        if let Some(window) = &self.primary {
            windows.push(window);
        }
        if let Some(window) = &self.secondary {
            windows.push(window);
        }
        if let Some(window) = &self.tertiary {
            windows.push(window);
        }
        for group in &self.groups {
            windows.push(&group.window);
        }
        windows
    }

    pub fn has_expired_window(&self, now_ms: u64) -> bool {
        self.all_windows()
            .into_iter()
            .any(|window| window.is_expired(now_ms))
    }

    pub fn is_using_credits(&self) -> bool {
        self.credits_available == Some(true)
            && self
                .all_windows()
                .into_iter()
                .any(|window| window.percent >= 100 || window.rejected)
    }

    pub fn is_exhausted(&self, pause_threshold: u8) -> bool {
        if self.credits_available == Some(true) {
            return false;
        }
        self.all_windows()
            .into_iter()
            .any(|window| window.rejected || window.percent >= pause_threshold)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn most_restrictive_window(&self, now_ms: u64) -> Option<&UsageWindow> {
        let windows = self.all_windows();
        let active: Vec<&UsageWindow> = windows
            .iter()
            .copied()
            .filter(|window| !window.is_expired(now_ms))
            .collect();
        let pool = if active.is_empty() { windows } else { active };
        pool.into_iter()
            .min_by_key(|window| window.remaining_percent())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanUsageSnapshot {
    pub providers: Vec<ProviderPlanUsage>,
    pub active_limits: Vec<LimitEntry>,
    pub pending_returns: Vec<UsageLimitReturn>,
    pub warn_threshold: u8,
    pub pause_threshold: u8,
    pub fetched_at_ms: u64,
}
