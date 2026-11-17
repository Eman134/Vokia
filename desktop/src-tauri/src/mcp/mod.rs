mod protocol;
mod registry;
mod server;
mod slot;
mod tools;

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use tauri::AppHandle;

pub use registry::{Binding, RandomnessUnavailable, Token};
pub use server::{McpConfigFile, McpServer, McpSession};
pub use slot::ExchangeSlot;

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

    pub fn problem(&self) -> Option<&McpError> {
        self.0.as_ref().err()
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
