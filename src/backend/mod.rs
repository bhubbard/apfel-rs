// ============================================================================
// backend/mod.rs — Backend module exports
// Part of apfel-rs
// ============================================================================

pub mod engine;
#[cfg(has_foundation_models)]
pub mod foundation_models;
pub mod mlx_engine;
pub mod mock;
pub mod session;

pub use engine::{BackendEngine, GenerateRequest, GenerateResponse, StreamChunk};
#[cfg(has_foundation_models)]
pub use foundation_models::FoundationModelsEngine;
pub use mlx_engine::MlxBackendEngine;
pub use mock::MockEngine;
pub use session::{SessionManager, SessionResult};

/// Returns the default backend engine for the current platform and build configuration.
pub fn default_engine() -> std::sync::Arc<dyn BackendEngine> {
    #[cfg(has_foundation_models)]
    {
        std::sync::Arc::new(FoundationModelsEngine::new())
    }
    #[cfg(not(has_foundation_models))]
    {
        if cfg!(target_arch = "aarch64") && cfg!(target_os = "macos") {
            std::sync::Arc::new(MlxBackendEngine::default())
        } else {
            std::sync::Arc::new(MockEngine::new())
        }
    }
}

/// Creates a backend engine by name, falling back to default if not specified.
pub fn create_engine(
    engine_name: Option<&str>,
    model_name: Option<&str>,
) -> std::sync::Arc<dyn BackendEngine> {
    match engine_name {
        Some("mlx") => {
            let engine = if let Some(m) = model_name {
                MlxBackendEngine::new(m)
            } else {
                MlxBackendEngine::default()
            };
            std::sync::Arc::new(engine)
        }
        Some("mock") => std::sync::Arc::new(MockEngine::new()),
        Some("foundation") | Some("apple") => {
            #[cfg(has_foundation_models)]
            {
                std::sync::Arc::new(FoundationModelsEngine::new())
            }
            #[cfg(not(has_foundation_models))]
            {
                std::sync::Arc::new(MockEngine::new())
            }
        }
        _ => default_engine(),
    }
}
