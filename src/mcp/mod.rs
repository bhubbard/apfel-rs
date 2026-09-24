// ============================================================================
// mcp/mod.rs — MCP module exports
// Part of apfel-rs
// ============================================================================

pub mod client;
pub mod protocol;

pub use client::{MCPConnection, MCPManager};
pub use protocol::{MCPProtocol, ServerInfo, MCP_PROTOCOL_VERSION};
