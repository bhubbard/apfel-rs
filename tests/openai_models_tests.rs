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
