mod protocol;
mod registry;
mod server;

use std::fmt;
use std::io;

pub use registry::{Binding, Token};
pub use server::McpServer;

#[derive(Debug)]
pub enum McpError {
    Bind(io::Error),
    BindTimeout,
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::Bind(error) => {
                write!(f, "não foi possível abrir o servidor MCP local: {error}")
            }
            McpError::BindTimeout => write!(f, "o servidor MCP local não abriu a tempo"),
        }
    }
}

pub struct McpState(Result<McpServer, McpError>);

impl McpState {
    pub fn start() -> McpState {
        McpState(tauri::async_runtime::block_on(McpServer::start()))
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
