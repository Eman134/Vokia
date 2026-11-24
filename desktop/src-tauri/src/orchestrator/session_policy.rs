pub const FRESH_AFTER_STALLS: u32 = 2;
pub const SESSION_RECOVERY_LIMIT: u32 = 2;
pub const SESSION_ROTATE_MESSAGES: u32 = 30;
pub const SESSION_IDLE_LIMIT_SECS: u64 = 600;
pub const SESSION_REAP_INTERVAL_SECS: u64 = 180;
pub const SESSION_CLOSE_TIMEOUT_SECS: u64 = 3;
pub const MAX_LIVE_SESSIONS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallMode {
    Fresh,
    Live,
}

pub fn call_mode(fresh_only: bool) -> CallMode {
    if fresh_only {
        CallMode::Fresh
    } else {
        CallMode::Live
    }
}

pub fn degrades_to_fresh(stalls: u32, repeats: u32, recoveries: u32) -> bool {
    stalls >= FRESH_AFTER_STALLS
        || repeats >= FRESH_AFTER_STALLS
        || recoveries >= SESSION_RECOVERY_LIMIT
}
