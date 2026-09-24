// ============================================================================
// middleware.rs — Server security, CORS, and authentication middleware
// Part of apfel-rs
// ============================================================================

use crate::core::error::ApfelError;
use crate::core::security::OriginValidator;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

pub struct ServerSecurityConfig {
    pub allowed_origins: Vec<String>,
    pub required_token: Option<String>,
    pub footgun: bool,
}

pub async fn security_middleware(
    State(config): State<Arc<ServerSecurityConfig>>,
    req: Request,
    next: Next,
) -> Response {
    let headers = req.headers();

    // 1. Origin check (CSRF protection for localhost)
    let origin_str = headers.get("origin").and_then(|v| v.to_str().ok());
    if !OriginValidator::is_allowed(origin_str, &config.allowed_origins) {
        let err = ApfelError::ForbiddenOrigin(format!(
            "Origin '{}' not allowed by localhost CSRF protection",
            origin_str.unwrap_or_default()
        ));
        let body = serde_json::to_string(&err.to_openai_json()).unwrap_or_default();
        return (StatusCode::FORBIDDEN, [("content-type", "application/json")], body).into_response();
    }

    // 2. Token auth check
    let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
    if !OriginValidator::is_valid_token(auth_header, config.required_token.as_deref()) {
        let err = ApfelError::Unauthorized("Invalid or missing Bearer token".to_string());
        let body = serde_json::to_string(&err.to_openai_json()).unwrap_or_default();
        return (StatusCode::UNAUTHORIZED, [("content-type", "application/json")], body).into_response();
    }

    next.run(req).await
}
