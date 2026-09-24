// ============================================================================
// core/mod.rs — ApfelCore module exports
// Part of apfel-rs
// ============================================================================

pub mod code_cropper;
pub mod context;
pub mod error;
pub mod file_framing;
pub mod json_stripper;
pub mod messages_input;
pub mod models;
pub mod responses_models;
pub mod schema;
pub mod security;
pub mod stream_sink;
pub mod token_counter;
pub mod tool_call;

pub use code_cropper::CodeCropper;
pub use context::{ContextConfig, ContextManager, ContextStrategy};
pub use error::{ApfelError, ApfelExitCodes, OpenAIErrorDetail, OpenAIErrorWrapper};
pub use file_framing::FileFraming;
pub use json_stripper::JSONFenceStripper;
pub use messages_input::MessagesInput;
pub use models::*;
pub use responses_models::*;
pub use schema::{PropertyIR, SchemaIR, SchemaParser};
pub use security::{scrub_mcp_environment, OriginValidator};
pub use stream_sink::StreamPrintSink;
pub use token_counter::TokenCounter;
pub use tool_call::{ParsedToolCall, StreamingToolCallGate, ToolCallHandler, ToolLogEntry, ToolOutputTruncator};
