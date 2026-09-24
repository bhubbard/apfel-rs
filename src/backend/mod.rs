// ============================================================================
// backend/mod.rs — Backend module exports
// Part of apfel-rs
// ============================================================================

pub mod engine;
#[cfg(has_foundation_models)]
pub mod foundation_models;
pub mod mock;
pub mod session;

pub use engine::{BackendEngine, GenerateRequest, GenerateResponse, StreamChunk};
#[cfg(has_foundation_models)]
pub use foundation_models::FoundationModelsEngine;
pub use mock::MockEngine;
pub use session::{SessionManager, SessionResult};

pub fn default_engine() -> std::sync::Arc<dyn BackendEngine> {
    #[cfg(has_foundation_models)]
    {
        std::sync::Arc::new(FoundationModelsEngine::new())
    }
    #[cfg(not(has_foundation_models))]
    {
        std::sync::Arc::new(MockEngine::new())
    }
}
