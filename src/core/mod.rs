// ============================================================================
// core/mod.rs — ApfelCore module exports
// Part of apfel-rs
// ============================================================================

pub mod apfs;
pub mod code_cropper;
pub mod context;
pub mod error;
pub mod fault_trap;
pub mod file_framing;
pub mod guard;
pub mod json_stripper;
pub mod messages_input;
pub mod models;
pub mod responses_models;
pub mod schema;
pub mod security;
pub mod shadow_compiler;
pub mod shm;
pub mod stop_matcher;
pub mod stream_sink;
pub mod thermal;
pub mod token_counter;
pub mod tool_call;

pub use apfs::{ApfsError, ApfsSandbox};
pub use code_cropper::{extract_code, ExtractedCode};
pub use context::{ContextConfig, ContextManager, ContextStrategy};
pub use error::{ApfelError, ApfelExitCodes, OpenAIErrorDetail, OpenAIErrorWrapper};
pub use fault_trap::{FaultCategory, FaultTrapError, TrappedFault};
pub use file_framing::{frame_document, frame_image, FileFraming};
pub use guard::{ApfelGuard, SecretVault};
pub use json_stripper::JSONFenceStripper;
pub use messages_input::MessagesInput;
pub use models::*;
pub mod openai_models {
    pub use crate::core::models::*;
}
pub use responses_models::*;
pub use schema::{PropertyIR, SchemaIR, SchemaParser};
pub use security::{scrub_mcp_environment, OriginValidator};
pub use shadow_compiler::{ShadowCompiler, SpeculativeFixCandidate};
pub use shm::{ShmBuffer, ShmError, ShmHeader};
pub use stop_matcher::{StopMatchResult, StopSequenceMatcher};
pub use stream_sink::StreamPrintSink;
pub use thermal::{ComputeProfile, PowerSource, PowerThermalMonitor, ThermalState};
pub use token_counter::TokenCounter;
pub use tool_call::{
    ParsedToolCall, StreamingToolCallGate, ToolCallHandler, ToolLogEntry, ToolOutputTruncator,
};
