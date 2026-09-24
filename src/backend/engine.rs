// ============================================================================
// engine.rs — Abstract backend engine trait for on-device FoundationModels
// Part of apfel-rs
// ============================================================================

use crate::core::error::ApfelError;
use crate::core::models::OpenAIMessage;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<OpenAIMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    pub permissive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerateResponse {
    pub content: String,
    pub finish_reason: String,
}

#[derive(Debug, Clone)]
pub enum StreamChunk {
    Delta(String),
    Done { finish_reason: String },
    Error(ApfelError),
}

pub trait BackendEngine: Send + Sync {
    fn is_available(&self) -> bool;
    fn context_size(&self) -> usize;
    fn count_tokens(&self, text: &str) -> usize;
    fn supported_languages(&self) -> Vec<String>;

    fn generate(&self, req: &GenerateRequest) -> Result<GenerateResponse, ApfelError>;
    fn stream_generate(
        &self,
        req: &GenerateRequest,
    ) -> Result<mpsc::Receiver<StreamChunk>, ApfelError>;
}
