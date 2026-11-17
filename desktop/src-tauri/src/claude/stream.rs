use serde::Deserialize;

use super::activity::TurnActivity;
use super::error::ClaudeError;

const RESULT_KIND: &str = "result";
const RATE_LIMIT_KIND: &str = "rate_limit_event";

pub struct Reply {
    pub text: String,
    pub limits: Option<RateLimits>,
    pub activity: TurnActivity,
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
}

pub(super) fn into_reply(
    response: CliResponse,
    limits: Option<RateLimits>,
) -> Result<Reply, ClaudeError> {
    if response.is_error {
        let message = if response.result.is_empty() {
            format!("sem texto (subtype: {})", response.subtype)
        } else {
            response.result
        };
        return Err(ClaudeError::Reported(message));
    }
    Ok(Reply {
        text: response.result,
        limits,
        activity: TurnActivity::default(),
    })
}

#[derive(Default)]
pub(super) struct StreamAccumulator {
    response: Option<CliResponse>,
    limits: Option<RateLimits>,
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
        let response = self.response.ok_or(ClaudeError::MissingResult)?;
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
