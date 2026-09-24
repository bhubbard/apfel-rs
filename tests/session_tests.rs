// ============================================================================
// tests/session_tests.rs — Unit tests for SessionManager pipeline
// ============================================================================

use apfel::backend::MockEngine;
use apfel::backend::session::SessionManager;
use apfel::core::context::ContextConfig;
use apfel::core::models::{FunctionDefinition, OpenAIMessage, OpenAITool};
use std::sync::Arc;

#[tokio::test]
async fn test_session_manager_basic_execution() {
    let mock = Arc::new(MockEngine::with_response("Hello from session pipeline"));
    let mgr = SessionManager::new(mock, None);
    let messages = vec![OpenAIMessage::user("Hi")];
    let config = ContextConfig::default();

    let res = mgr
        .process_messages(&messages, None, &config, Some(0.7), None, Some(100), None)
        .await
        .expect("Session processing failed");

    assert_eq!(res.content, "Hello from session pipeline");
    assert_eq!(res.finish_reason, "stop");
    assert!(res.tool_log.is_empty());
}

#[tokio::test]
async fn test_session_manager_with_tools_injection() {
    let mock = Arc::new(MockEngine::with_response("Answer with tools format"));
    let mgr = SessionManager::new(mock, None);
    let messages = vec![OpenAIMessage::user("What's the weather?")];
    let config = ContextConfig::default();

    let tools = vec![OpenAITool {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: "get_weather".to_string(),
            description: Some("Get the current weather".to_string()),
            parameters: None,
        },
    }];

    let res = mgr
        .process_messages(&messages, Some(&tools), &config, None, None, None, None)
        .await
        .expect("Session with tools failed");

    assert_eq!(res.content, "Answer with tools format");
}

#[tokio::test]
async fn test_session_manager_context_overflow() {
    let mock = Arc::new(MockEngine::with_response(""));
    let mgr = SessionManager::new(mock, None);
    let messages = vec![
        OpenAIMessage::system("Very long system prompt that will be configured to overflow budget"),
        OpenAIMessage::user("User question"),
    ];

    let mut config = ContextConfig::default();
    // Reserve all tokens so budget is 0
    config.output_reserve = 5000;

    let res = mgr
        .process_messages(&messages, None, &config, None, None, None, None)
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        apfel::core::error::ApfelError::ContextOverflow(_) => {}
        other => panic!("Expected ContextOverflow, got {:?}", other),
    }
}
