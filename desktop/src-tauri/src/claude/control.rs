use serde::Deserialize;

const CONTROL_REQUEST_KIND: &str = "control_request";
const CAN_USE_TOOL_SUBTYPE: &str = "can_use_tool";

#[derive(Clone)]
pub struct PermissionRequest {
    pub id: String,
    pub tool: String,
    pub input: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermissionVerdict {
    Allow,
    Deny(Option<String>),
}

pub(super) enum ControlRequest {
    Permission(PermissionRequest),
    Malformed(String),
}

#[derive(Deserialize)]
struct ControlLine {
    #[serde(rename = "type")]
    kind: String,
    request_id: Option<String>,
    request: Option<ControlBody>,
}

#[derive(Deserialize)]
struct ControlBody {
    subtype: String,
    tool_name: Option<String>,
    input: Option<serde_json::Value>,
}

// Pedido sem id é ignorado (não há a quem responder); com id mas incompleto, a resposta é negar, nunca permitir.
pub(super) fn parse_control_request(line: &[u8]) -> Option<ControlRequest> {
    let parsed: ControlLine = serde_json::from_slice(line).ok()?;
    if parsed.kind != CONTROL_REQUEST_KIND {
        return None;
    }
    let id = parsed.request_id?;
    let Some(body) = parsed
        .request
        .filter(|body| body.subtype == CAN_USE_TOOL_SUBTYPE)
    else {
        return Some(ControlRequest::Malformed(id));
    };
    match (body.tool_name, body.input) {
        (Some(tool), Some(input)) => Some(ControlRequest::Permission(PermissionRequest {
            id,
            tool,
            input,
        })),
        _ => Some(ControlRequest::Malformed(id)),
    }
}
