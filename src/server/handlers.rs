// ============================================================================
// handlers.rs — HTTP route handlers for OpenAI-compatible server
// Part of apfel-rs
// ============================================================================

use crate::backend::engine::{BackendEngine, GenerateRequest, StreamChunk};
use crate::backend::session::SessionManager;
use crate::core::context::{ContextConfig, ContextStrategy};
use crate::core::error::ApfelError;
use crate::core::json_stripper::JSONFenceStripper;
use crate::core::models::*;
use crate::core::responses_models::*;
use crate::mcp::client::MCPManager;
use axum::body::Body;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::str::FromStr;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<dyn BackendEngine>,
    pub mcp_manager: Option<Arc<MCPManager>>,
}

// MARK: - Health & Info

pub async fn health_handler(State(state): State<AppState>) -> impl IntoResponse {
    let languages = state.engine.supported_languages();
    let resp = serde_json::json!({
        "status": "ok",
        "model": "apple-foundationmodel",
        "on_device": true,
        "available": state.engine.is_available(),
        "context_size": state.engine.context_size(),
        "languages": languages,
        "framework": "FoundationModels (macOS 26+)"
    });
    Json(resp)
}

// MARK: - Models

pub async fn list_models_handler() -> impl IntoResponse {
    let resp = ModelList {
        object: "list".to_string(),
        data: vec![
            ModelObject {
                id: "apple-foundationmodel".to_string(),
                object: "model".to_string(),
                created: 1718000000,
                owned_by: "apple".to_string(),
            },
            ModelObject {
                id: "gpt-4o-mini".to_string(), // alias for compatibility
                object: "model".to_string(),
                created: 1718000000,
                owned_by: "apple".to_string(),
            },
        ],
    };
    Json(resp)
}

// MARK: - Chat Completions

pub async fn chat_completions_handler(
    State(state): State<AppState>,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    let is_stream = req.stream.unwrap_or(false);

    // Resolve context config from request
    let strategy = req
        .x_context_strategy
        .as_deref()
        .and_then(|s| ContextStrategy::from_str(s).ok())
        .unwrap_or_default();

    let config = ContextConfig {
        strategy,
        max_turns: req.x_context_max_turns,
        output_reserve: req.x_context_output_reserve.unwrap_or(512),
        permissive: false,
    };

    let session_mgr = SessionManager::new(state.engine.clone(), state.mcp_manager.clone());

    if is_stream {
        // Streaming SSE path
        let last_prompt = req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user" || m.role == "tool")
            .map(|m| m.text_content())
            .unwrap_or_default();

        let system_prompt = req
            .messages
            .iter()
            .filter(|m| m.role == "system")
            .map(|m| m.text_content())
            .collect::<Vec<_>>()
            .join("\n\n");

        let gen_req = GenerateRequest {
            prompt: last_prompt,
            system_prompt: if system_prompt.is_empty() { None } else { Some(system_prompt) },
            messages: Some(req.messages.clone()),
            temperature: req.temperature,
            top_p: req.top_p,
            max_tokens: req.effective_max_tokens(),
            permissive: false,
            seed: req.seed,
        };

        let mut rx = match state.engine.stream_generate(&gen_req) {
            Ok(rx) => rx,
            Err(e) => {
                return (
                    StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(e.to_openai_json()),
                )
                    .into_response();
            }
        };

        let stream_id = format!("chatcmpl-{}", Uuid::new_v4());
        let model_name = req.model.clone();

        let stream = async_stream::stream! {
            let created = chrono::Utc::now().timestamp();
            while let Some(chunk) = rx.recv().await {
                match chunk {
                    StreamChunk::Delta(delta_text) => {
                        let chunk_obj = ChatCompletionChunk {
                            id: stream_id.clone(),
                            object: "chat.completion.chunk".to_string(),
                            created,
                            model: model_name.clone(),
                            choices: vec![ChatCompletionChunkChoice {
                                index: 0,
                                delta: ChatCompletionChunkDelta {
                                    role: None,
                                    content: Some(delta_text),
                                    tool_calls: None,
                                },
                                finish_reason: None,
                            }],
                        };
                        let json = serde_json::to_string(&chunk_obj).unwrap_or_default();
                        yield Ok::<_, std::convert::Infallible>(format!("data: {}\n\n", json));
                    }
                    StreamChunk::Done { finish_reason } => {
                        let final_chunk = ChatCompletionChunk {
                            id: stream_id.clone(),
                            object: "chat.completion.chunk".to_string(),
                            created,
                            model: model_name.clone(),
                            choices: vec![ChatCompletionChunkChoice {
                                index: 0,
                                delta: ChatCompletionChunkDelta::default(),
                                finish_reason: Some(finish_reason),
                            }],
                        };
                        let json = serde_json::to_string(&final_chunk).unwrap_or_default();
                        yield Ok(format!("data: {}\n\ndata: [DONE]\n\n", json));
                        break;
                    }
                    StreamChunk::Error(e) => {
                        let err_json = serde_json::to_string(&e.to_openai_json()).unwrap_or_default();
                        yield Ok(format!("data: {}\n\ndata: [DONE]\n\n", err_json));
                        break;
                    }
                }
            }
        };

        Response::builder()
            .header(header::CONTENT_TYPE, "text/event-stream")
            .header(header::CACHE_CONTROL, "no-cache")
            .header(header::CONNECTION, "keep-alive")
            .body(Body::from_stream(stream))
            .unwrap()
    } else {
        // Non-streaming path with full MCP loop
        let result = match session_mgr
            .process_messages(
                &req.messages,
                req.tools.as_deref(),
                &config,
                req.temperature,
                req.top_p,
                req.effective_max_tokens(),
                req.seed,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return (
                    StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    Json(e.to_openai_json()),
                )
                    .into_response();
            }
        };

        let content = if let Some(rf) = &req.response_format {
            if rf.format_type == "json_object" || rf.format_type == "json_schema" {
                JSONFenceStripper::strip(&result.content)
            } else {
                result.content
            }
        } else {
            result.content
        };

        let prompt_tokens = req
            .messages
            .iter()
            .map(|m| state.engine.count_tokens(&m.text_content()))
            .sum();
        let completion_tokens = state.engine.count_tokens(&content);

        let resp = ChatCompletionResponse {
            id: format!("chatcmpl-{}", Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: req.model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: OpenAIMessage::assistant(content),
                finish_reason: result.finish_reason,
            }],
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens: prompt_tokens + completion_tokens,
            },
        };

        Json(resp).into_response()
    }
}

// MARK: - Responses Endpoint (/v1/responses)

pub async fn responses_handler(
    State(state): State<AppState>,
    Json(req): Json<ResponsesRequest>,
) -> Response {
    if req.background == Some(true) {
        let err = ApfelError::NotImplemented("background=true is not supported on-device".to_string());
        return (StatusCode::NOT_IMPLEMENTED, Json(err.to_openai_json())).into_response();
    }
    if req.previous_response_id.is_some() {
        let err = ApfelError::NotImplemented(
            "previous_response_id is not supported (server is stateless)".to_string(),
        );
        return (StatusCode::NOT_IMPLEMENTED, Json(err.to_openai_json())).into_response();
    }

    let mut messages = Vec::new();
    if let Some(instructions) = req.instructions {
        messages.push(OpenAIMessage::system(instructions));
    }

    match req.input {
        Some(ResponsesInput::Text(t)) => {
            messages.push(OpenAIMessage::user(t));
        }
        Some(ResponsesInput::Items(items)) => {
            for item in items {
                let role = item.role.unwrap_or_else(|| "user".to_string());
                let text = match item.content {
                    Some(serde_json::Value::String(s)) => s,
                    Some(v) => serde_json::to_string(&v).unwrap_or_default(),
                    None => String::new(),
                };
                messages.push(OpenAIMessage {
                    role,
                    content: Some(MessageContent::Text(text)),
                    name: None,
                    tool_call_id: None,
                    tool_calls: None,
                });
            }
        }
        None => {
            messages.push(OpenAIMessage::user(""));
        }
    }

    let session_mgr = SessionManager::new(state.engine.clone(), state.mcp_manager.clone());
    let config = ContextConfig::default();

    let result = match session_mgr
        .process_messages(
            &messages,
            None,
            &config,
            req.temperature,
            req.top_p,
            req.max_output_tokens,
            None,
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::from_u16(e.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                Json(e.to_openai_json()),
            )
                .into_response();
        }
    };

    let input_tokens = messages
        .iter()
        .map(|m| state.engine.count_tokens(&m.text_content()))
        .sum();
    let output_tokens = state.engine.count_tokens(&result.content);

    let resp = ResponsesResponse {
        id: format!("resp-{}", Uuid::new_v4()),
        object: "response".to_string(),
        created_at: chrono::Utc::now().timestamp(),
        model: req.model.unwrap_or_else(|| "apple-foundationmodel".to_string()),
        status: "completed".to_string(),
        output: vec![ResponsesOutputItem::Message {
            id: format!("msg-{}", Uuid::new_v4()),
            role: "assistant".to_string(),
            content: vec![ResponsesOutputContent::Text {
                text: result.content,
            }],
        }],
        usage: ResponsesUsage {
            input_tokens,
            output_tokens,
            total_tokens: input_tokens + output_tokens,
        },
    };

    Json(resp).into_response()
}
