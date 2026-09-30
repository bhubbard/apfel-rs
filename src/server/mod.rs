// ============================================================================
// server/mod.rs — HTTP Server router and lifecycle
// Part of apfel-rs
// ============================================================================

pub(crate) mod handlers;
pub(crate) mod middleware;

use crate::backend::engine::BackendEngine;
use crate::mcp::client::MCPManager;
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

/// Starts the OpenAI-compatible HTTP server with CORS, CSRF, and MCP routing.
pub async fn run_server(
    host: &str,
    port: u16,
    token: Option<String>,
    allowed_origins: Vec<String>,
    footgun: bool,
    engine: Arc<dyn BackendEngine>,
    mcp_manager: Option<Arc<MCPManager>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app_state = handlers::AppState {
        engine: engine.clone(),
        mcp_manager,
    };

    let security_config = Arc::new(middleware::ServerSecurityConfig {
        allowed_origins: allowed_origins.clone(),
        required_token: token,
        footgun,
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let router = Router::new()
        .route("/health", get(handlers::health_handler))
        .route("/", get(handlers::health_handler))
        .route("/v1/models", get(handlers::list_models_handler))
        .route(
            "/v1/chat/completions",
            post(handlers::chat_completions_handler),
        )
        .route("/v1/responses", post(handlers::responses_handler))
        .route(
            "/v1/responses/input_tokens",
            post(handlers::responses_input_tokens_handler),
        )
        .route("/v1/embeddings", post(handlers::embeddings_handler))
        .route("/api/tags", get(handlers::ollama_tags_handler))
        .route("/api/chat", post(handlers::ollama_chat_handler))
        .route("/api/generate", post(handlers::ollama_generate_handler))
        .layer(from_fn_with_state(
            security_config,
            middleware::security_middleware,
        ))
        .layer(cors)
        .with_state(app_state);

    let addr: SocketAddr = format!("{}:{}", host, port).parse()?;
    if !crate::core::security::OriginValidator::origin_validation_constrains(
        footgun,
        &allowed_origins,
    ) {
        eprintln!("\x1b[31mWARNING: origin check disabled - all origins allowed\x1b[0m");
    }
    println!("apfel server listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).tcp_nodelay(true).await?;

    Ok(())
}
