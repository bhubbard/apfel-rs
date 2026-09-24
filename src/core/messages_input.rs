// ============================================================================
// messages_input.rs — Decoder and validator for --messages input
// Part of apfel-rs
// ============================================================================

use crate::core::error::ApfelError;
use crate::core::models::OpenAIMessage;
use serde::Deserialize;

#[derive(Deserialize)]
struct WrappedMessages {
    messages: Vec<OpenAIMessage>,
}

pub struct MessagesInput;

impl MessagesInput {
    pub fn decode(json_str: &str) -> Result<Vec<OpenAIMessage>, ApfelError> {
        let trimmed = json_str.trim();
        if trimmed.is_empty() {
            return Err(ApfelError::Usage("messages JSON string is empty".into()));
        }

        let messages: Vec<OpenAIMessage> = if trimmed.starts_with('[') {
            serde_json::from_str(trimmed)
                .map_err(|e| ApfelError::Usage(format!("invalid JSON messages array: {}", e)))?
        } else if trimmed.starts_with('{') {
            let wrapped: WrappedMessages = serde_json::from_str(trimmed)
                .map_err(|e| ApfelError::Usage(format!("invalid JSON messages object: {}", e)))?;
            wrapped.messages
        } else {
            return Err(ApfelError::Usage("invalid JSON messages input".into()));
        };

        if messages.is_empty() {
            return Err(ApfelError::Usage("messages array cannot be empty".into()));
        }

        for msg in &messages {
            match msg.role.as_str() {
                "system" | "user" | "assistant" | "developer" | "tool" => (),
                unknown => {
                    return Err(ApfelError::Usage(format!("unknown message role: {}", unknown)));
                }
            }
        }

        Ok(messages)
    }
}
