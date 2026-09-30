#![deny(clippy::all)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::unnested_or_patterns)]
#![allow(clippy::manual_strip)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::redundant_pattern_matching)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::needless_lifetimes)]
#![allow(clippy::unnecessary_map_or)]
#![allow(clippy::collapsible_match)]

// ============================================================================
// lib.rs — Top-level library exports for apfel-rs
// ============================================================================

pub mod backend;
pub mod cli;
pub mod core;
pub mod mcp;
pub mod server;

pub use backend::{
    create_engine, default_engine, BackendEngine, GenerateRequest, GenerateResponse, StreamChunk,
};
pub use core::*;
pub use mcp::{MCPConnection, MCPManager, MCPProtocol};
pub use server::run_server;
