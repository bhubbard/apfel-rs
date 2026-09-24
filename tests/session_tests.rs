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

#[test]
fn test_default_engine_instantiation() {
    let engine = apfel::backend::default_engine();
    assert!(engine.context_size() > 0);
}

#[tokio::test]
async fn test_session_manager_tool_calling_loop_and_resolution() {
    let tool_call_json = r#"{
        "tool_calls": [
            {
                "id": "call_calc",
                "type": "function",
                "function": {
                    "name": "calculator",
                    "arguments": "{\"expr\": \"2+2\"}"
                }
            }
        ]
    }"#;

    let mock = Arc::new(MockEngine::with_responses(vec![
        tool_call_json,
        "The calculator returned error: No tool handler registered for 'calculator'.",
    ]));

    let mgr = SessionManager::new(mock, None);
    let messages = vec![
        OpenAIMessage::system("System instructions"),
        OpenAIMessage::user("Calculate 2+2"),
    ];

    let tools = vec![OpenAITool {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: "calculator".to_string(),
            description: Some("Performs math".to_string()),
            parameters: None,
        },
    }];

    let config = ContextConfig::default();
    let res = mgr
        .process_messages(&messages, Some(&tools), &config, None, None, None, None)
        .await
        .expect("Tool calling loop failed");

    assert_eq!(res.tool_log.len(), 1);
    assert_eq!(res.tool_log[0].name, "calculator");
    assert!(res.tool_log[0].is_error);
    assert!(res.content.contains("No tool handler registered"));
}

#[tokio::test]
async fn test_session_manager_reprompt_cap() {
    let tool_call_json = r#"{
        "tool_calls": [
            {
                "id": "call_inf",
                "type": "function",
                "function": {
                    "name": "inf_tool",
                    "arguments": "{}"
                }
            }
        ]
    }"#;
    let infinite_tool_calls = MockEngine::with_response(tool_call_json);
    let mgr = SessionManager::new(Arc::new(infinite_tool_calls), None);
    let messages = vec![OpenAIMessage::user("Run loop")];
    let config = ContextConfig::default();

    let res = mgr
        .process_messages(&messages, None, &config, None, None, None, None)
        .await;

    assert!(res.is_err());
    match res.unwrap_err() {
        apfel::core::error::ApfelError::ToolExecution(msg) => {
            assert!(msg.contains("round cap"));
        }
        other => panic!("Expected ToolExecution error with round cap, got {:?}", other),
    }
}

