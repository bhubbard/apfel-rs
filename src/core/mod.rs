// ============================================================================
// core/mod.rs — ApfelCore module exports
// Part of apfel-rs
// ============================================================================

pub mod code_cropper;
pub mod context;
pub mod error;
pub mod json_stripper;
pub mod models;
pub mod responses_models;
pub mod schema;
pub mod security;
pub mod token_counter;
pub mod tool_call;

pub use code_cropper::CodeCropper;
pub use context::{ContextConfig, ContextManager, ContextStrategy};
pub use error::{ApfelError, ApfelExitCodes, OpenAIErrorDetail, OpenAIErrorWrapper};
pub use json_stripper::JSONFenceStripper;
pub use models::*;
pub use responses_models::*;
pub use schema::{PropertyIR, SchemaIR, SchemaParser};
pub use security::{scrub_mcp_environment, OriginValidator};
pub use token_counter::TokenCounter;
pub use tool_call::{ParsedToolCall, ToolCallHandler, ToolLogEntry, ToolOutputTruncator};
