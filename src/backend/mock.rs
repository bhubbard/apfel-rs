// ============================================================================
// mock.rs — Mock backend engine for testing and fallback
// Part of apfel-rs
// ============================================================================

use crate::backend::engine::{BackendEngine, GenerateRequest, GenerateResponse, StreamChunk};
use crate::core::error::ApfelError;
use tokio::sync::mpsc;

pub struct MockEngine {
    pub response: String,
    pub responses: std::sync::Mutex<Vec<String>>,
    pub available: bool,
    pub context_window: usize,
}

impl MockEngine {
    pub fn new() -> Self {
        Self {
            response: "This is a response from the mock backend engine.".to_string(),
            responses: std::sync::Mutex::new(Vec::new()),
            available: true,
            context_window: 4096,
        }
    }

    pub fn with_response(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
            responses: std::sync::Mutex::new(Vec::new()),
            available: true,
            context_window: 4096,
        }
    }

    pub fn with_responses<I, S>(responses: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            response: String::new(),
            responses: std::sync::Mutex::new(responses.into_iter().map(Into::into).collect()),
            available: true,
            context_window: 4096,
        }
    }
}

impl Default for MockEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl BackendEngine for MockEngine {
    fn is_available(&self) -> bool {
        self.available
    }

    fn context_size(&self) -> usize {
        self.context_window
    }

    fn count_tokens(&self, text: &str) -> usize {
        (text.chars().count() / 4).max(1)
    }

    fn supported_languages(&self) -> Vec<String> {
        vec!["en".to_string(), "es".to_string(), "de".to_string(), "fr".to_string()]
    }

    fn stream_generate(
        &self,
        _req: &GenerateRequest,
    ) -> Result<mpsc::Receiver<StreamChunk>, ApfelError> {
        if !self.available {
            return Err(ApfelError::ModelUnavailable("Mock engine is unavailable".to_string()));
        }

        let (tx, rx) = mpsc::channel(32);
        let resp = if let Ok(mut lock) = self.responses.lock() {
            if !lock.is_empty() {
                lock.remove(0)
            } else {
                self.response.clone()
            }
        } else {
            self.response.clone()
        };

        tokio::spawn(async move {
            let words: Vec<&str> = resp.split_whitespace().collect();
            for word in words {
                let _ = tx.send(StreamChunk::Delta(format!("{} ", word))).await;
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
            let _ = tx.send(StreamChunk::Done { finish_reason: "stop".to_string() }).await;
        });

        Ok(rx)
    }

    fn generate(&self, _req: &GenerateRequest) -> Result<GenerateResponse, ApfelError> {
        if !self.available {
            return Err(ApfelError::ModelUnavailable("Mock engine is unavailable".to_string()));
        }
        let content = if let Ok(mut lock) = self.responses.lock() {
            if !lock.is_empty() {
                lock.remove(0)
            } else {
                self.response.clone()
            }
        } else {
            self.response.clone()
        };
        Ok(GenerateResponse {
            content,
            finish_reason: "stop".to_string(),
        })
    }
}
