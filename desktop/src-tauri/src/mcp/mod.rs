mod protocol;
mod registry;
mod server;
mod slot;
pub mod stdio_relay;
pub(crate) mod tools;

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use tauri::AppHandle;

pub use registry::{random_hex, Binding, RandomnessUnavailable};
#[cfg(test)]
pub use registry::Token;
pub use server::{McpConfigFile, McpServer, McpSession};
pub use stdio_relay::{
    McpRelayManager, RelayEndpoint, RelaySessionGuard, StdioMcpServerConfig, VokiaMcpBridge,
};
pub use tools::ToolScope;
pub use slot::{Delegation, DelegationIntent, ExchangeOutcome, ExchangeSlot, SlotDecision};

pub const SERVER_NAME: &str = "vokia";
pub const CONFIG_DIR: &str = "mcp";

#[derive(Debug)]
pub enum McpError {
    Bind(io::Error),
    BindTimeout,
    Randomness(RandomnessUnavailable),
    ConfigFile(io::Error),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::Bind(error) => {
                write!(f, "não foi possível abrir o servidor MCP local: {error}")
            }
            McpError::BindTimeout => write!(f, "o servidor MCP local não abriu a tempo"),
            McpError::Randomness(error) => write!(f, "{error}"),
            McpError::ConfigFile(error) => {
                write!(f, "não foi possível gravar a configuração do MCP: {error}")
            }
        }
    }
}

// Sobras de um app que caiu levam o token da sessão; a pasta inexistente já é o resultado desejado.
pub fn clear_stale_configs(app_data_dir: &Path) {
    let _ = fs::remove_dir_all(app_data_dir.join(CONFIG_DIR));
}

pub struct McpState(Result<McpServer, McpError>);

impl McpState {
    pub fn start(app: AppHandle) -> McpState {
        let server = tauri::async_runtime::block_on(McpServer::start());
        if let Ok(server) = &server {
            server.attach(app);
        }
        McpState(server)
    }

    pub fn server(&self) -> Option<&McpServer> {
        self.0.as_ref().ok()
    }

}

#[cfg(test)]
#[path = "../../tests/mcp_server_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/mcp_support.rs"]
pub(crate) mod support;

#[cfg(test)]
#[path = "../../tests/mcp_backlog_tests.rs"]
mod backlog_tests;

#[cfg(test)]
#[path = "../../tests/mcp_decision_tests.rs"]
mod decision_tests;

#[cfg(test)]
#[path = "../../tests/mcp_proposal_tests.rs"]
mod proposal_tests;

#[cfg(test)]
#[path = "../../tests/mcp_delegation_tests.rs"]
mod delegation_tests;

#[cfg(test)]
#[path = "../../tests/mcp_variables_tests.rs"]
mod variables_tests;

#[cfg(test)]
#[path = "../../tests/mcp_update_card_tests.rs"]
mod update_card_tests;

#[cfg(test)]
#[path = "../../tests/mcp_orchestration_tests.rs"]
mod orchestration_tests;
