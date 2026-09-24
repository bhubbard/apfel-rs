// ============================================================================
// lib.rs — Top-level library exports for apfel-rs
// ============================================================================

pub mod backend;
pub mod cli;
pub mod core;
pub mod mcp;
pub mod server;

pub use backend::{default_engine, BackendEngine, GenerateRequest, GenerateResponse, StreamChunk};
pub use core::*;
pub use mcp::{MCPConnection, MCPManager, MCPProtocol};
pub use server::run_server;
