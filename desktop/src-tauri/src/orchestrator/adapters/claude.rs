use std::sync::atomic::{AtomicU32, Ordering};

use crate::claude::{Access, ClaudeError, LiveSession, Reply};
use crate::permissions::PermissionGate;

use super::super::agent_session::{open_mcp, send_in_slot, AgentCall, SESSION_CLOSE_GRACE};
use super::super::console::ConsoleSink;
use super::super::session_pool::NO_PID;

pub(super) async fn run_fresh(
    access: Access<'_>,
    gate: PermissionGate<'_>,
    pid: &AtomicU32,
    call: &AgentCall,
    message: &str,
    console: &ConsoleSink<'_>,
) -> Result<Reply, ClaudeError> {
    let mut live = LiveSession::open(
        &call.program,
        &call.user_args,
        &call.role,
        &call.model,
        access,
        None,
        open_mcp(call)?,
    )?;
    pid.store(live.pid().unwrap_or(NO_PID), Ordering::SeqCst);
    let reply = send_in_slot(&mut live, call, message, &gate, console).await;
    live.close(SESSION_CLOSE_GRACE).await;
    pid.store(NO_PID, Ordering::SeqCst);
    reply
}
