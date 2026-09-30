// ============================================================================
// cli/mod.rs — CLI module exports
// Part of apfel-rs
// ============================================================================

pub mod args;
pub mod batch;
pub mod chat;
pub mod runner;

pub use args::CliArgs;
pub use batch::{run_batch_mode, run_batch_stream};
pub use runner::run_cli;
