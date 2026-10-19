// ============================================================================
// mcp/mod.rs — MCP module exports
// Part of apfel-rs
// ============================================================================

pub mod client;
pub mod protocol;

pub use client::{parse_command_line, MCPConnection, MCPManager};
pub use protocol::{MCPProtocol, McpConfig, McpServerConfig, ServerInfo, MCP_PROTOCOL_VERSION};
