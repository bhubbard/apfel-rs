// ============================================================================
// models.rs — OpenAI-compatible data types
// Part of apfel-rs
// ============================================================================

use serde::{Deserialize, Serialize};

/// Request payload for OpenAI-compatible chat completions (/v1/chat/completions).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<StopSequence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    // Custom Apfel context extensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_context_strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_context_max_turns: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x_context_output_reserve: Option<usize>,
}

/// Stop sequence condition representing either a single string or list of strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StopSequence {
    Single(String),
    Multiple(Vec<String>),
}

impl ChatCompletionRequest {
    pub fn effective_max_tokens(&self) -> Option<usize> {
        self.max_completion_tokens.or(self.max_tokens)
    }

    /// Validates max_tokens and max_completion_tokens according to OpenAI specification.
    /// Rejects conflicting values with an error.
    pub fn validate_max_tokens(&self) -> Result<Option<usize>, crate::core::error::ApfelError> {
        if let (Some(legacy), Some(modern)) = (self.max_tokens, self.max_completion_tokens) {
            if legacy != modern {
                return Err(crate::core::error::ApfelError::Usage(format!(
                    "Conflicting max_tokens ({}) and max_completion_tokens ({})",
                    legacy, modern
                )));
            }
        }
        Ok(self.effective_max_tokens())
    }

    pub fn stop_sequences(&self) -> Vec<&str> {
        match &self.stop {
            Some(StopSequence::Single(s)) => vec![s.as_str()],
            Some(StopSequence::Multiple(list)) => list.iter().map(|s| s.as_str()).collect(),
            None => Vec::new(),
        }
    }
}

/// A single message turn within a chat completion conversation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

impl OpenAIMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn developer(content: impl Into<String>) -> Self {
        Self {
            role: "developer".to_string(),
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: Some(MessageContent::Text(content.into())),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn tool(
        content: impl Into<String>,
        tool_call_id: impl Into<String>,
        name: Option<String>,
    ) -> Self {
        Self {
            role: "tool".to_string(),
            content: Some(MessageContent::Text(content.into())),
            name,
            tool_call_id: Some(tool_call_id.into()),
            tool_calls: None,
        }
    }

    pub fn user_parts(parts: Vec<ContentPart>) -> Self {
        Self {
            role: "user".to_string(),
            content: Some(MessageContent::Parts(parts)),
            name: None,
            tool_call_id: None,
            tool_calls: None,
        }
    }

    pub fn text_content(&self) -> String {
        match &self.content {
            Some(MessageContent::Text(t)) => t.clone(),
            Some(MessageContent::Parts(parts)) => {
                let mut texts = Vec::new();
                for p in parts {
                    match p {
                        ContentPart::Text { text } => texts.push(text.clone()),
                        ContentPart::ImageUrl { image_url } => {
                            let name = if image_url.url.starts_with("data:image/") {
                                if let Some(mime) = image_url
                                    .url
                                    .strip_prefix("data:image/")
                                    .and_then(|s| s.split(';').next())
                                {
                                    format!("attachment.{}", mime)
                                } else {
                                    "attachment.png".to_string()
                                }
                            } else {
                                image_url.url.clone()
                            };
                            texts.push(crate::core::file_framing::frame_image(&name, "", ""));
                        }
                    }
                }
                texts.join("\n")
            }
            None => String::new(),
        }
    }
}

/// Alias for MessageContent for OpenAI specification compatibility.
pub type OpenAIMessageContent = MessageContent;

/// Content of a conversation message, either plain text or a list of parts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

/// A multimodal content part within a message (text or image).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ImageUrl },
}

/// URL or data-URI payload for an image message attachment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

/// Tool definition exposed to the model conforming to OpenAI format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// Function schema and description for a declared tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Controls whether and how the model calls tools.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Mode(String), // "none", "auto", "required"
    Named {
        #[serde(rename = "type")]
        choice_type: String,
        function: FunctionChoice,
    },
}

/// Specific function selection inside a named tool choice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionChoice {
    pub name: String,
}

/// A tool invocation emitted by the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

/// Function call parameters for an emitted tool call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Desired format specification for model output (text, json_object, json_schema).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String, // "text", "json_object", "json_schema"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

/// Non-streaming chat completion response payload (/v1/chat/completions).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChoice>,
    pub usage: Usage,
}

/// A single completion choice in a chat completion response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: usize,
    pub message: OpenAIMessage,
    pub finish_reason: String,
}

/// Token usage accounting statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
}

/// Streaming chunk payload for Server-Sent Events chat completions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatCompletionChunkChoice>,
}

/// An individual choice chunk within a streaming chat completion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatCompletionChunkChoice {
    pub index: usize,
    pub delta: ChatCompletionChunkDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// Incremental delta content within a streaming choice chunk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChatCompletionChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Response payload for model enumeration (/v1/models).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelList {
    pub object: String,
    pub data: Vec<ModelObject>,
}

/// Metadata description of a single available model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelObject {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
    #[serde(default)]
    pub context_window: usize,
    #[serde(default)]
    pub context_window_measured: bool,
}

// ============================================================================
// Embedding Models (Phase 2)
// ============================================================================

/// Request payload for text embedding creation (/v1/embeddings).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    pub model: String,
    pub input: EmbeddingInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Input text representation for an embedding request, single string or array of strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EmbeddingInput {
    String(String),
    Array(Vec<String>),
}

impl EmbeddingInput {
    pub fn to_vec(&self) -> Vec<String> {
        match self {
            EmbeddingInput::String(s) => vec![s.clone()],
            EmbeddingInput::Array(arr) => arr.clone(),
        }
    }
}

/// Vector embedding data for an individual input index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingObject {
    pub object: String,
    pub index: usize,
    pub embedding: Vec<f32>,
}

/// Completed response for a text embedding request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    pub object: String,
    pub data: Vec<EmbeddingObject>,
    pub model: String,
    pub usage: Usage,
}

// ============================================================================
// Ollama Models (Phase 3)
// ============================================================================

/// Ollama model tag describing local model artifact metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OllamaModelTag {
    pub name: String,
    pub modified_at: String,
    pub size: u64,
}

/// Response listing available Ollama model tags (/api/tags).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OllamaTagsResponse {
    pub models: Vec<OllamaModelTag>,
}

/// Request payload for Ollama model show endpoint (/api/show).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct OllamaShowRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Request payload for Ollama chat endpoint (/api/chat).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(default)]
    pub stream: Option<bool>,
}

/// Request payload for Ollama text generation endpoint (/api/generate).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OllamaGenerateRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub stream: Option<bool>,
}
