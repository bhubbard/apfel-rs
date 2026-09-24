use apfel::core::models::{FunctionDefinition, OpenAITool};
use apfel::core::tool_call::{ToolCallHandler, ToolOutputTruncator};

#[test]
fn test_detect_tool_call_json() {
    let json_resp = r#"{
        "tool_calls": [
            {
                "id": "call_1",
                "type": "function",
                "function": {
                    "name": "calculate",
                    "arguments": "{\"expression\": \"2 + 2\"}"
                }
            }
        ]
    }"#;

    let calls = ToolCallHandler::detect_tool_call(json_resp).expect("Expected tool calls");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "calculate");
    assert_eq!(calls[0].id, "call_1");
}

#[test]
fn test_detect_tool_call_in_code_fences() {
    let fenced_resp = "Here is the calculation:\n```json\n{\n  \"tool_calls\": [\n    {\n      \"id\": \"call_abc\",\n      \"type\": \"function\",\n      \"function\": {\n        \"name\": \"get_weather\",\n        \"arguments\": \"{\\\"city\\\": \\\"San Francisco\\\"}\"\n      }\n    }\n  ]\n}\n```";

    let calls = ToolCallHandler::detect_tool_call(fenced_resp).expect("Expected tool calls from fence");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "get_weather");
}

#[test]
fn test_tool_output_truncator() {
    let long_text = "a".repeat(1000);
    let (truncated, was_truncated) = ToolOutputTruncator::truncate(&long_text, 1000, 100);

    assert!(was_truncated);
    assert!(truncated.contains("[tool output truncated:"));
    assert!(truncated.len() < long_text.len());

    let short_text = "short output";
    let (not_truncated, was_truncated) = ToolOutputTruncator::truncate(short_text, 10, 100);
    assert!(!was_truncated);
    assert_eq!(not_truncated, short_text);
}

#[test]
fn test_build_output_format_instructions() {
    let tools = vec![OpenAITool {
        tool_type: "function".to_string(),
        function: FunctionDefinition {
            name: "search_db".to_string(),
            description: Some("Search the database".to_string()),
            parameters: None,
        },
    }];

    let instructions = ToolCallHandler::build_output_format_instructions(&tools);
    assert!(instructions.contains("## Tool Calling Format"));
    assert!(instructions.contains("search_db"));
}
