// ============================================================================
// tests/responses_models_tests.rs — Tests for OpenAI /v1/responses models
// Ported from: ResponsesModelsTests.swift
// ============================================================================

use apfel::core::responses_models::{
    ResponsesInput, ResponsesOutputContent, ResponsesOutputItem, ResponsesRequest,
    ResponsesResponse, ResponsesUsage,
};

#[test]
fn test_decode_string_input() {
    let json = r#"{
        "model": "apple-foundationmodel",
        "input": "Summarize this document"
    }"#;

    let req: ResponsesRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.model, Some("apple-foundationmodel".into()));
    match req.input {
        Some(ResponsesInput::Text(text)) => assert_eq!(text, "Summarize this document"),
        _ => panic!("Expected string text input"),
    }
}

#[test]
fn test_decode_message_list_input() {
    let json = r#"{
        "model": "apple-foundationmodel",
        "input": [
            {"role": "system", "content": "You are helpful."},
            {"role": "user", "content": "What is 2+2?"}
        ]
    }"#;

    let req: ResponsesRequest = serde_json::from_str(json).unwrap();
    match req.input {
        Some(ResponsesInput::Items(items)) => {
            assert_eq!(items.len(), 2);
            assert_eq!(items[0].role, Some("system".into()));
            assert_eq!(items[1].role, Some("user".into()));
        }
        _ => panic!("Expected item list input"),
    }
}

#[test]
fn test_decode_instructions_and_sampling_params() {
    let json = r#"{
        "instructions": "Be strictly brief",
        "temperature": 0.2,
        "top_p": 0.8,
        "max_output_tokens": 150
    }"#;

    let req: ResponsesRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.instructions, Some("Be strictly brief".into()));
    assert_eq!(req.temperature, Some(0.2));
    assert_eq!(req.top_p, Some(0.8));
    assert_eq!(req.max_output_tokens, Some(150));
}

#[test]
fn test_response_object_serialization() {
    let resp = ResponsesResponse {
        id: "resp_12345".into(),
        object: "response".into(),
        created_at: 1700000000,
        model: "apple-foundationmodel".into(),
        status: "completed".into(),
        output: vec![ResponsesOutputItem::Message {
            id: "msg_1".into(),
            role: "assistant".into(),
            content: vec![ResponsesOutputContent::Text {
                text: "4".into(),
            }],
        }],
        usage: ResponsesUsage {
            input_tokens: 10,
            output_tokens: 1,
            total_tokens: 11,
        },
    };

    let serialized = serde_json::to_string(&resp).unwrap();
    assert!(serialized.contains(r#""id":"resp_12345""#));
    assert!(serialized.contains(r#""status":"completed""#));
    assert!(serialized.contains(r#""total_tokens":11"#));
}
