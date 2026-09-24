use apfel::core::models::*;

#[test]
fn test_chat_completion_request_serialization() {
    let req = ChatCompletionRequest {
        model: "apple-foundationmodel".to_string(),
        messages: vec![
            OpenAIMessage::system("You are a helpful assistant."),
            OpenAIMessage::user("Hello!"),
        ],
        stream: Some(true),
        temperature: Some(0.7),
        top_p: Some(0.9),
        max_tokens: Some(100),
        max_completion_tokens: None,
        seed: Some(42),
        tools: None,
        tool_choice: None,
        parallel_tool_calls: None,
        stop: None,
        response_format: None,
        user: Some("user_123".to_string()),
        x_context_strategy: Some("sliding-window".to_string()),
        x_context_max_turns: Some(10),
        x_context_output_reserve: Some(512),
    };

    let json = serde_json::to_string(&req).expect("Failed to serialize request");
    let deserialized: ChatCompletionRequest = serde_json::from_str(&json).expect("Failed to deserialize request");

    assert_eq!(req.model, deserialized.model);
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.effective_max_tokens(), Some(100));
}

#[test]
fn test_message_content_helpers() {
    let msg1 = OpenAIMessage::user("Single text");
    assert_eq!(msg1.text_content(), "Single text");

    let msg2 = OpenAIMessage {
        role: "user".to_string(),
        content: Some(MessageContent::Parts(vec![
            ContentPart::Text { text: "Line 1".to_string() },
            ContentPart::Text { text: "Line 2".to_string() },
        ])),
        name: None,
        tool_call_id: None,
        tool_calls: None,
    };
    assert_eq!(msg2.text_content(), "Line 1\nLine 2");
}

#[test]
fn test_chat_completion_response_serialization() {
    let resp = ChatCompletionResponse {
        id: "chatcmpl_abc".to_string(),
        object: "chat.completion".to_string(),
        created: 1700000000,
        model: "apple-foundationmodel".to_string(),
        choices: vec![ChatCompletionChoice {
            index: 0,
            message: OpenAIMessage::assistant("This is a response."),
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens: 15,
            completion_tokens: 5,
            total_tokens: 20,
        },
    };

    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains(r#""id":"chatcmpl_abc""#));
    assert!(json.contains(r#""finish_reason":"stop""#));
    assert!(json.contains(r#""total_tokens":20"#));
}

#[test]
fn test_chat_completion_chunk_sse_format() {
    let chunk = ChatCompletionChunk {
        id: "chatcmpl_chunk1".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 1700000000,
        model: "apple-foundationmodel".to_string(),
        choices: vec![ChatCompletionChunkChoice {
            index: 0,
            delta: ChatCompletionChunkDelta {
                role: None,
                content: Some("token".to_string()),
                tool_calls: None,
            },
            finish_reason: None,
        }],
    };

    let json = serde_json::to_string(&chunk).unwrap();
    assert!(json.contains(r#""content":"token""#));
    assert!(json.contains(r#""chat.completion.chunk""#));
}

#[test]
fn test_max_completion_tokens_and_conflict_resolution() {
    let mut req = ChatCompletionRequest {
        model: "apple-foundationmodel".to_string(),
        messages: vec![OpenAIMessage::user("Hello")],
        stream: None,
        temperature: None,
        top_p: None,
        max_tokens: Some(100),
        max_completion_tokens: None,
        seed: None,
        tools: None,
        tool_choice: None,
        parallel_tool_calls: None,
        stop: None,
        response_format: None,
        user: None,
        x_context_strategy: None,
        x_context_max_turns: None,
        x_context_output_reserve: None,
    };

    assert_eq!(req.validate_max_tokens().unwrap(), Some(100));

    // Modern max_completion_tokens
    req.max_tokens = None;
    req.max_completion_tokens = Some(200);
    assert_eq!(req.validate_max_tokens().unwrap(), Some(200));

    // Both identical -> succeeds
    req.max_tokens = Some(200);
    assert_eq!(req.validate_max_tokens().unwrap(), Some(200));

    // Conflicting -> fails with error
    req.max_tokens = Some(150);
    assert!(req.validate_max_tokens().is_err());
}

#[test]
fn test_stop_sequences_single_and_multiple() {
    let json_single = r#"{"model":"m","messages":[],"stop":"\n"}"#;
    let req1: ChatCompletionRequest = serde_json::from_str(json_single).unwrap();
    assert_eq!(req1.stop_sequences(), vec!["\n"]);

    let json_multiple = r#"{"model":"m","messages":[],"stop":["\nObservation:", "Human:"]}"#;
    let req2: ChatCompletionRequest = serde_json::from_str(json_multiple).unwrap();
    assert_eq!(req2.stop_sequences(), vec!["\nObservation:", "Human:"]);
}

