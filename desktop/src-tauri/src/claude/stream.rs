use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use super::activity::TurnActivity;
use super::error::ClaudeError;

const RESULT_KIND: &str = "result";
const ASSISTANT_KIND: &str = "assistant";
const RATE_LIMIT_KIND: &str = "rate_limit_event";
const REJECTED_STATUS: &str = "rejected";
const USAGE_LIMIT_ERROR: &str = "usage_limit_reached";
const USAGE_LIMIT_STATUS: u16 = 429;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnUsage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_input_tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
}

impl TurnUsage {
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.provider.is_none()
            && self.model.is_none()
            && self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.cached_input_tokens.is_none()
            && self.cost_usd.is_none()
    }
}

pub struct Reply {
    pub text: String,
    pub limits: Option<RateLimits>,
    pub activity: TurnActivity,
    pub usage: Option<TurnUsage>,
}

#[derive(Clone, Copy, Debug)]
pub struct RateLimits {
    pub five_hour: LimitWindow,
    pub seven_day: LimitWindow,
}

#[derive(Clone, Copy, Debug)]
pub struct LimitWindow {
    pub utilization: f64,
    pub resets_at: u64,
    pub rejected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LimitName {
    FiveHour,
    SevenDay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UsageLimit {
    pub window: LimitName,
    pub resets_at: u64,
}

impl RateLimits {
    fn exhausted(self) -> Option<UsageLimit> {
        [
            (LimitName::FiveHour, self.five_hour),
            (LimitName::SevenDay, self.seven_day),
        ]
        .into_iter()
        .find(|(_, window)| window.rejected)
        .map(|(window, limit)| UsageLimit {
            window,
            resets_at: limit.resets_at,
        })
    }
}

#[derive(Deserialize)]
struct StreamLine {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct RateLimitLine {
    rate_limit_info: RateLimitInfo,
}

#[derive(Deserialize)]
struct RateLimitInfo {
    #[serde(rename = "unifiedWindows")]
    unified_windows: UnifiedWindows,
}

#[derive(Deserialize)]
struct UnifiedWindows {
    five_hour: WindowInfo,
    seven_day: WindowInfo,
}

#[derive(Deserialize)]
struct WindowInfo {
    utilization: f64,
    #[serde(rename = "resetsAt")]
    resets_at: u64,
    #[serde(default)]
    status: String,
}

impl From<UnifiedWindows> for RateLimits {
    fn from(windows: UnifiedWindows) -> Self {
        Self {
            five_hour: windows.five_hour.into(),
            seven_day: windows.seven_day.into(),
        }
    }
}

impl From<WindowInfo> for LimitWindow {
    fn from(window: WindowInfo) -> Self {
        Self {
            utilization: window.utilization,
            resets_at: window.resets_at,
            rejected: window.status == REJECTED_STATUS,
        }
    }
}

#[derive(Deserialize)]
pub(super) struct CliResponse {
    #[serde(default)]
    result: String,
    #[serde(default)]
    subtype: String,
    #[serde(default)]
    pub(super) session_id: String,
    is_error: bool,
    #[serde(default)]
    api_error_status: Option<u16>,
    #[serde(default)]
    api_error: Option<Json>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    cost_usd: Option<f64>,
    #[serde(default)]
    usage: Option<Json>,
    #[serde(default, rename = "modelUsage")]
    model_usage: Option<Json>,
    #[serde(skip)]
    assistant_model: Option<String>,
}

impl CliResponse {
    fn is_usage_limit(&self) -> bool {
        self.api_error_status == Some(USAGE_LIMIT_STATUS)
            || self.api_error.as_ref().and_then(Json::as_str) == Some(USAGE_LIMIT_ERROR)
    }

    fn extract_usage(&self) -> Option<TurnUsage> {
        let model = self
            .model_usage
            .as_ref()
            .and_then(Json::as_object)
            .and_then(|map| map.keys().next())
            .cloned()
            .or_else(|| self.assistant_model.clone());
        let input_tokens = self
            .usage
            .as_ref()
            .and_then(|u| u.get("input_tokens"))
            .and_then(Json::as_u64);
        let output_tokens = self
            .usage
            .as_ref()
            .and_then(|u| u.get("output_tokens"))
            .and_then(Json::as_u64);
        let cached_input_tokens = self
            .usage
            .as_ref()
            .and_then(|u| u.get("cache_read_input_tokens"))
            .and_then(Json::as_u64);
        let cost_usd = self.total_cost_usd.or(self.cost_usd);

        if model.is_none()
            && input_tokens.is_none()
            && output_tokens.is_none()
            && cached_input_tokens.is_none()
            && cost_usd.is_none()
        {
            return None;
        }
        Some(TurnUsage {
            provider: Some("claude".to_string()),
            model,
            input_tokens,
            output_tokens,
            cached_input_tokens,
            cost_usd,
        })
    }
}

pub(super) fn into_reply(
    response: CliResponse,
    limits: Option<RateLimits>,
) -> Result<Reply, ClaudeError> {
    if response.is_error && response.is_usage_limit() {
        if let Some(limit) = limits.and_then(RateLimits::exhausted) {
            return Err(ClaudeError::UsageLimit(limit));
        }
        return Err(ClaudeError::LimitExhausted);
    }
    if response.is_error {
        let message = if response.result.is_empty() {
            format!("sem texto (subtype: {})", response.subtype)
        } else {
            response.result
        };
        return Err(ClaudeError::Reported(message));
    }
    let usage = response.extract_usage();
    Ok(Reply {
        text: response.result,
        limits,
        activity: TurnActivity::default(),
        usage,
    })
}

#[derive(Default)]
pub(super) struct StreamAccumulator {
    response: Option<CliResponse>,
    limits: Option<RateLimits>,
    assistant_model: Option<String>,
}

impl StreamAccumulator {
    pub(super) fn push_line(&mut self, line: &[u8]) -> Result<bool, ClaudeError> {
        if line.iter().all(u8::is_ascii_whitespace) {
            return Ok(false);
        }
        let stream_line: StreamLine =
            serde_json::from_slice(line).map_err(ClaudeError::InvalidJson)?;
        match stream_line.kind.as_str() {
            RESULT_KIND => {
                self.response =
                    Some(serde_json::from_slice(line).map_err(ClaudeError::InvalidJson)?);
                return Ok(true);
            }
            ASSISTANT_KIND => {
                if let Ok(value) = serde_json::from_slice::<Json>(line) {
                    if let Some(model) = value
                        .pointer("/message/model")
                        .and_then(Json::as_str)
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                    {
                        self.assistant_model = Some(model.to_string());
                    }
                }
            }
            // Um formato novo de limite não pode derrubar a conversa: a linha é ignorada.
            RATE_LIMIT_KIND => {
                if let Ok(rate_limit_line) = serde_json::from_slice::<RateLimitLine>(line) {
                    self.limits = Some(rate_limit_line.rate_limit_info.unified_windows.into());
                }
            }
            _ => {}
        }
        Ok(false)
    }

    pub(super) fn finish(self) -> Result<(CliResponse, Option<RateLimits>), ClaudeError> {
        let mut response = self.response.ok_or(ClaudeError::MissingResult)?;
        if response.assistant_model.is_none() {
            response.assistant_model = self.assistant_model;
        }
        Ok((response, self.limits))
    }
}

pub(super) fn parse_stream(
    stdout: &[u8],
) -> Result<(CliResponse, Option<RateLimits>), ClaudeError> {
    let mut accumulator = StreamAccumulator::default();
    for line in stdout.split(|byte| *byte == b'\n') {
        accumulator.push_line(line)?;
    }
    accumulator.finish()
}

#[cfg(test)]
#[path = "../../tests/stream_tests.rs"]
mod tests;
