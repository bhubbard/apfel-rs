// ============================================================================
// error.rs — Error types and exit codes for apfel-rs
// ============================================================================

use serde::Serialize;

/// Apfel standard exit codes matching the Swift reference implementation.
pub struct ApfelExitCodes;

impl ApfelExitCodes {
    pub const SUCCESS: i32 = 0;
    pub const RUNTIME_ERROR: i32 = 1;
    pub const USAGE_ERROR: i32 = 2;
    pub const GUARDRAIL: i32 = 3;
    pub const CONTEXT_OVERFLOW: i32 = 4;
    pub const MODEL_UNAVAILABLE: i32 = 5;
    pub const RATE_LIMITED: i32 = 6;
    pub const NO_CODE: i32 = 7;
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum ApfelError {
    #[error("usage error: {0}")]
    Usage(String),

    #[error("model unavailable: {0}")]
    ModelUnavailable(String),

    #[error("guardrail triggered: {0}")]
    Guardrail(String),

    #[error("context window overflow: {0}")]
    ContextOverflow(String),

    #[error("rate limited: {0}")]
    RateLimited(String),

    #[error("tool execution error: {0}")]
    ToolExecution(String),

    #[error("mcp error: {0}")]
    MCP(String),

    #[error("no code block found in response")]
    NoCodeFound,

    #[error("not implemented: {0}")]
    NotImplemented(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden origin: {0}")]
    ForbiddenOrigin(String),

    #[error("runtime error: {0}")]
    Runtime(String),
}

impl ApfelError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) => ApfelExitCodes::USAGE_ERROR,
            Self::ModelUnavailable(_) => ApfelExitCodes::MODEL_UNAVAILABLE,
            Self::Guardrail(_) => ApfelExitCodes::GUARDRAIL,
            Self::ContextOverflow(_) => ApfelExitCodes::CONTEXT_OVERFLOW,
            Self::RateLimited(_) => ApfelExitCodes::RATE_LIMITED,
            Self::NoCodeFound => ApfelExitCodes::NO_CODE,
            Self::ToolExecution(_)
            | Self::MCP(_)
            | Self::NotImplemented(_)
            | Self::Unauthorized(_)
            | Self::ForbiddenOrigin(_)
            | Self::Runtime(_) => ApfelExitCodes::RUNTIME_ERROR,
        }
    }

    pub fn http_status(&self) -> u16 {
        match self {
            Self::Usage(_) => 400,
            Self::Unauthorized(_) => 401,
            Self::ForbiddenOrigin(_) => 403,
            Self::NotImplemented(_) => 501,
            Self::RateLimited(_) => 429,
            Self::ModelUnavailable(_) => 503,
            Self::ContextOverflow(_) => 400,
            Self::Guardrail(_) => 400,
            _ => 500,
        }
    }

    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited(_) => true,
            Self::ModelUnavailable(msg) => {
                let lower = msg.to_lowercase();
                lower.contains("busy") || lower.contains("concurrent") || lower.contains("assets") || lower.contains("temporarily")
            }
            _ => false,
        }
    }

    pub fn to_openai_json(&self) -> OpenAIErrorWrapper {
        let error_type = match self {
            Self::Usage(_) => "invalid_request_error",
            Self::Unauthorized(_) => "authentication_error",
            Self::ForbiddenOrigin(_) => "permission_error",
            Self::RateLimited(_) => "rate_limit_error",
            Self::NotImplemented(_) => "not_implemented_error",
            _ => "api_error",
        };

        OpenAIErrorWrapper {
            error: OpenAIErrorDetail {
                message: self.to_string(),
                type_name: error_type.to_string(),
                code: None,
            },
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct OpenAIErrorWrapper {
    pub error: OpenAIErrorDetail,
}

#[derive(Debug, Serialize, Clone)]
pub struct OpenAIErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}
