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

/// Decodes conversation JSON arrays or object envelopes into vectors of OpenAIMessages.
#[derive(Debug)]
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
                    return Err(ApfelError::Usage(format!(
                        "unknown message role: {}",
                        unknown
                    )));
                }
            }
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_valid_array() {
        let json = r#"[{"role": "user", "content": "hello"}]"#;
        let msgs = MessagesInput::decode(json).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "user");
    }

    #[test]
    fn test_decode_valid_object_envelope() {
        let json = r#"{"messages": [{"role": "assistant", "content": "hi"}]}"#;
        let msgs = MessagesInput::decode(json).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].role, "assistant");
    }

    #[test]
    fn test_decode_empty_input() {
        let err = MessagesInput::decode("   ").unwrap_err();
        assert!(matches!(err, ApfelError::Usage(_)));
    }

    #[test]
    fn test_decode_empty_array() {
        let err = MessagesInput::decode("[]").unwrap_err();
        assert!(matches!(err, ApfelError::Usage(_)));
    }

    #[test]
    fn test_decode_invalid_json() {
        let err = MessagesInput::decode("[{invalid}]").unwrap_err();
        assert!(matches!(err, ApfelError::Usage(_)));
    }

    #[test]
    fn test_decode_invalid_role() {
        let json = r#"[{"role": "admin", "content": "hello"}]"#;
        let err = MessagesInput::decode(json).unwrap_err();
        assert!(matches!(err, ApfelError::Usage(_)));
    }
}
