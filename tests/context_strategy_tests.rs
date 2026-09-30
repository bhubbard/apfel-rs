use apfel::core::context::{ContextConfig, ContextManager, ContextStrategy};
use apfel::core::models::{OpenAIMessage, ToolCall, ToolCallFunction};

#[test]
fn test_context_strategy_newest_first() {
    let messages = vec![
        OpenAIMessage::system("System instructions"), // ~4 tokens
        OpenAIMessage::user("Message 1"),             // ~2 tokens
        OpenAIMessage::assistant("Response 1"),       // ~2 tokens
        OpenAIMessage::user("Message 2"),             // ~2 tokens
        OpenAIMessage::assistant("Response 2"),       // ~2 tokens
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::NewestFirst,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    // Budget fits system prompt (2) + last 2 messages (2 + 2 = 4) = 6 tokens
    let trimmed =
        ContextManager::trim_messages(&messages, 6, &config, |s| s.split_whitespace().count())
            .expect("Trimming failed");

    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].role, "user");
    assert_eq!(trimmed[1].text_content(), "Message 2");
    assert_eq!(trimmed[2].role, "assistant");
    assert_eq!(trimmed[2].text_content(), "Response 2");
}

#[test]
fn test_context_strategy_strict_overflow() {
    let messages = vec![
        OpenAIMessage::system("System prompt"),
        OpenAIMessage::user("A very long message that definitely exceeds the tiny budget"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::Strict,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    let result = ContextManager::trim_messages(&messages, 5, &config, |s| s.len());
    assert!(result.is_none());
}

#[test]
fn test_context_strategy_oldest_first() {
    let messages = vec![
        OpenAIMessage::system("System"),
        OpenAIMessage::user("First"),
        OpenAIMessage::assistant("Second"),
        OpenAIMessage::user("Third"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::OldestFirst,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    // Budget: System(6) + First(5) + Second(6) = 17 bytes, Third(5) would be 22
    let trimmed = ContextManager::trim_messages(&messages, 18, &config, |s| s.len())
        .expect("Trimming failed");

    assert_eq!(trimmed.len(), 3);
    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].text_content(), "First");
    assert_eq!(trimmed[2].text_content(), "Second");
}

#[test]
fn test_context_strategy_sliding_window() {
    let messages = vec![
        OpenAIMessage::system("Sys"),
        OpenAIMessage::user("Turn 1"),
        OpenAIMessage::assistant("Turn 2"),
        OpenAIMessage::user("Turn 3"),
        OpenAIMessage::assistant("Turn 4"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::SlidingWindow,
        max_turns: Some(2),
        output_reserve: 0,
        permissive: false,
    };

    let trimmed = ContextManager::trim_messages(&messages, 100, &config, |s| s.len())
        .expect("Trimming failed");

    // Only last 2 turns plus system instruction should be preserved
    assert_eq!(trimmed.len(), 3);
    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].text_content(), "Turn 3");
    assert_eq!(trimmed[2].text_content(), "Turn 4");
}

#[test]
fn test_context_strategy_summarize() {
    let messages = vec![
        OpenAIMessage::system("Sys"),
        OpenAIMessage::user("Old message"),
        OpenAIMessage::user("New message"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::Summarize,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    // Budget fits "New message" within 70% sub-budget, dropping "Old message"
    let trimmed = ContextManager::trim_messages(&messages, 20, &config, |s| s.len())
        .expect("Trimming failed");

    // Should include system instruction, note about dropped messages, and kept message
    assert!(trimmed.iter().any(|m| m
        .text_content()
        .contains("prior messages were summarized/truncated")));
    assert_eq!(trimmed.last().unwrap().text_content(), "New message");
}

#[test]
fn test_context_instructions_exceeding_budget_fails() {
    let messages = vec![
        OpenAIMessage::system("Extremely long system prompt that will not fit"),
        OpenAIMessage::user("Hello"),
    ];

    let config = ContextConfig::default();
    let result = ContextManager::trim_messages(&messages, 10, &config, |s| s.len());
    assert!(result.is_none());
}

#[test]
fn test_context_strategy_from_str() {
    use std::str::FromStr;

    assert_eq!(
        ContextStrategy::from_str("newest-first").unwrap(),
        ContextStrategy::NewestFirst
    );
    assert_eq!(
        ContextStrategy::from_str("newest").unwrap(),
        ContextStrategy::NewestFirst
    );
    assert_eq!(
        ContextStrategy::from_str("oldest-first").unwrap(),
        ContextStrategy::OldestFirst
    );
    assert_eq!(
        ContextStrategy::from_str("oldest").unwrap(),
        ContextStrategy::OldestFirst
    );
    assert_eq!(
        ContextStrategy::from_str("sliding-window").unwrap(),
        ContextStrategy::SlidingWindow
    );
    assert_eq!(
        ContextStrategy::from_str("sliding").unwrap(),
        ContextStrategy::SlidingWindow
    );
    assert_eq!(
        ContextStrategy::from_str("summarize").unwrap(),
        ContextStrategy::Summarize
    );
    assert_eq!(
        ContextStrategy::from_str("strict").unwrap(),
        ContextStrategy::Strict
    );

    assert!(ContextStrategy::from_str("invalid-strategy").is_err());
}

#[test]
fn test_context_config_defaults_and_serde() {
    let default_cfg = ContextConfig::default();
    assert_eq!(default_cfg.strategy, ContextStrategy::NewestFirst);
    assert_eq!(default_cfg.output_reserve, 512);

    let json = serde_json::to_string(&default_cfg).unwrap();
    let deserialized: ContextConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(default_cfg, deserialized);
}

#[test]
fn test_context_strategy_strict_success() {
    let messages = vec![
        OpenAIMessage::system("System"),
        OpenAIMessage::user("Short message"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::Strict,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    let result = ContextManager::trim_messages(&messages, 100, &config, |s| s.len());
    assert!(result.is_some());
    let trimmed = result.unwrap();
    assert_eq!(trimmed.len(), 2);
    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].text_content(), "Short message");
}

#[test]
fn test_context_developer_role_instruction() {
    let messages = vec![
        OpenAIMessage::developer("You are a helpful coding assistant"),
        OpenAIMessage::user("Hello"),
    ];

    let config = ContextConfig::default();
    let trimmed = ContextManager::trim_messages(&messages, 100, &config, |s| s.len())
        .expect("Failed to trim developer role message");

    assert_eq!(trimmed.len(), 2);
    assert_eq!(trimmed[0].role, "developer");
    assert_eq!(trimmed[1].role, "user");
}

#[test]
fn test_context_sliding_window_fewer_turns() {
    let messages = vec![
        OpenAIMessage::system("System"),
        OpenAIMessage::user("Turn 1"),
        OpenAIMessage::assistant("Turn 2"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::SlidingWindow,
        max_turns: Some(5), // More than the 2 conversation turns
        output_reserve: 0,
        permissive: false,
    };

    let trimmed = ContextManager::trim_messages(&messages, 100, &config, |s| s.len())
        .expect("Trimming failed");

    assert_eq!(trimmed.len(), 3);
    assert_eq!(trimmed[1].text_content(), "Turn 1");
    assert_eq!(trimmed[2].text_content(), "Turn 2");
}

#[test]
fn test_context_summarize_all_fit() {
    let messages = vec![OpenAIMessage::system("System"), OpenAIMessage::user("Hi")];

    let config = ContextConfig {
        strategy: ContextStrategy::Summarize,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    let trimmed = ContextManager::trim_messages(&messages, 100, &config, |s| s.len())
        .expect("Trimming failed");

    // All messages fit, so no note should be injected
    assert_eq!(trimmed.len(), 2);
    assert!(!trimmed
        .iter()
        .any(|m| m.text_content().contains("prior messages were summarized")));
}

fn make_tool_call_assistant(call_id: &str, tool_name: &str, args: &str) -> OpenAIMessage {
    OpenAIMessage {
        role: "assistant".to_string(),
        content: None,
        name: None,
        tool_call_id: None,
        tool_calls: Some(vec![ToolCall {
            id: call_id.to_string(),
            call_type: "function".to_string(),
            function: ToolCallFunction {
                name: tool_name.to_string(),
                arguments: args.to_string(),
            },
        }]),
    }
}

fn make_parallel_tool_call_assistant(calls: &[(&str, &str, &str)]) -> OpenAIMessage {
    OpenAIMessage {
        role: "assistant".to_string(),
        content: None,
        name: None,
        tool_call_id: None,
        tool_calls: Some(
            calls
                .iter()
                .map(|(id, name, args)| ToolCall {
                    id: (*id).to_string(),
                    call_type: "function".to_string(),
                    function: ToolCallFunction {
                        name: (*name).to_string(),
                        arguments: (*args).to_string(),
                    },
                })
                .collect(),
        ),
    }
}

#[test]
fn test_tool_call_exchange_newest_first_kept_together() {
    let assistant_msg = make_tool_call_assistant("call_1", "get_weather", "{\"city\":\"Berlin\"}");
    let tool_msg = OpenAIMessage::tool("22C sunny", "call_1", Some("get_weather".to_string()));

    let messages = vec![
        OpenAIMessage::system("Sys"),
        OpenAIMessage::user("Old question that will be dropped"),
        assistant_msg.clone(),
        tool_msg.clone(),
        OpenAIMessage::user("Latest question"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::NewestFirst,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    // Cost breakdown with |s| s.len():
    // Sys: 3
    // Old question: 34
    // Assistant: "get_weather {\"city\":\"Berlin\"}".len() = 30
    // Tool: "22C sunny".len() = 9
    // Exchange total: 39
    // Latest question: 15
    // Total budget: 3 (Sys) + 15 (Latest) + 39 (Exchange) = 57.
    let trimmed = ContextManager::trim_messages(&messages, 57, &config, |s| s.len())
        .expect("Trimming should succeed");

    // Both assistant tool call and tool response must be preserved together
    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].role, "assistant");
    assert!(trimmed[1].tool_calls.is_some());
    assert_eq!(trimmed[2].role, "tool");
    assert_eq!(trimmed[2].tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(trimmed[3].role, "user");
    assert_eq!(trimmed[3].text_content(), "Latest question");
}

#[test]
fn test_tool_call_exchange_newest_first_dropped_together_not_split() {
    let assistant_msg = make_tool_call_assistant("call_1", "get_weather", "{\"city\":\"Berlin\"}");
    let tool_msg = OpenAIMessage::tool("22C sunny", "call_1", Some("get_weather".to_string()));

    let messages = vec![
        OpenAIMessage::system("Sys"),
        OpenAIMessage::user("Initial question"),
        assistant_msg,
        tool_msg,
        OpenAIMessage::user("Latest question"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::NewestFirst,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    // Budget fits Sys (3) + Latest question (15) + 20 tokens extra.
    // Exchange cost is 39, which cannot fit in 20.
    // Crucially: tool response (cost 9) could fit in 20, but because the exchange is atomic,
    // neither the tool response nor the assistant tool call should be included!
    let trimmed = ContextManager::trim_messages(&messages, 38, &config, |s| s.len())
        .expect("Trimming should succeed");

    // Only system instruction and Latest question should be kept; no orphaned tool response
    assert_eq!(trimmed.len(), 2);
    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].role, "user");
    assert_eq!(trimmed[1].text_content(), "Latest question");
    assert!(!trimmed.iter().any(|m| m.role == "tool"));
    assert!(!trimmed.iter().any(|m| m.tool_calls.is_some()));
}

#[test]
fn test_tool_call_exchange_oldest_first_dropped_together_not_split() {
    let assistant_msg = make_tool_call_assistant("call_1", "lookup", "{\"id\":42}");
    let tool_msg = OpenAIMessage::tool("found record", "call_1", None);

    let messages = vec![
        OpenAIMessage::system("Sys"),
        OpenAIMessage::user("First query"),
        assistant_msg,
        tool_msg,
        OpenAIMessage::user("Second query"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::OldestFirst,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    // Sys: 3
    // First query: 11
    // Assistant: "lookup {\"id\":42}".len() = 17
    // Tool: "found record".len() = 12
    // Exchange total: 29
    // Budget: 3 (Sys) + 11 (First query) + 20 extra = 34.
    // Assistant (17) could fit alone in 20, but Tool (12) would push exchange to 29.
    // Since exchange is atomic, assistant message must NOT be kept without its tool response!
    let trimmed = ContextManager::trim_messages(&messages, 34, &config, |s| s.len())
        .expect("Trimming should succeed");

    assert_eq!(trimmed.len(), 2);
    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].role, "user");
    assert_eq!(trimmed[1].text_content(), "First query");
    assert!(!trimmed.iter().any(|m| m.tool_calls.is_some()));
    assert!(!trimmed.iter().any(|m| m.role == "tool"));
}

#[test]
fn test_parallel_tool_call_exchange_preserved_atomically() {
    let assistant_msg = make_parallel_tool_call_assistant(&[
        ("call_a", "func_a", "{}"),
        ("call_b", "func_b", "{}"),
    ]);
    let tool_a = OpenAIMessage::tool("result_a", "call_a", None);
    let tool_b = OpenAIMessage::tool("result_b", "call_b", None);

    let messages = vec![
        OpenAIMessage::system("Sys"),
        assistant_msg,
        tool_a,
        tool_b,
        OpenAIMessage::user("Next user message"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::NewestFirst,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    // Full budget fits everything
    let trimmed_all = ContextManager::trim_messages(&messages, 200, &config, |s| s.len())
        .expect("Trimming should succeed");
    assert_eq!(trimmed_all.len(), 5);
    assert_eq!(trimmed_all[1].role, "assistant");
    assert_eq!(trimmed_all[2].role, "tool");
    assert_eq!(trimmed_all[2].tool_call_id.as_deref(), Some("call_a"));
    assert_eq!(trimmed_all[3].role, "tool");
    assert_eq!(trimmed_all[3].tool_call_id.as_deref(), Some("call_b"));

    // Budget fits only Sys + Next user message
    let trimmed_partial = ContextManager::trim_messages(&messages, 25, &config, |s| s.len())
        .expect("Trimming should succeed");
    assert_eq!(trimmed_partial.len(), 2);
    assert_eq!(trimmed_partial[0].role, "system");
    assert_eq!(trimmed_partial[1].text_content(), "Next user message");
    // Ensure all tool messages and assistant tool calls are dropped together
    assert!(!trimmed_partial.iter().any(|m| m.role == "tool"));
    assert!(!trimmed_partial.iter().any(|m| m.tool_calls.is_some()));
}

#[test]
fn test_tool_call_exchange_sliding_window_atomic() {
    let assistant_msg = make_tool_call_assistant("call_1", "calc", "1+1");
    let tool_msg = OpenAIMessage::tool("2", "call_1", None);

    let messages = vec![
        OpenAIMessage::system("Sys"),
        OpenAIMessage::user("Turn 1"),
        assistant_msg,
        tool_msg,
        OpenAIMessage::assistant("Result is 2"),
        OpenAIMessage::user("Turn 3"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::SlidingWindow,
        max_turns: Some(2), // Last 2 turns: Result is 2, Turn 3
        output_reserve: 0,
        permissive: false,
    };

    let trimmed = ContextManager::trim_messages(&messages, 100, &config, |s| s.len())
        .expect("Trimming should succeed");
    assert_eq!(trimmed.len(), 3);
    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].text_content(), "Result is 2");
    assert_eq!(trimmed[2].text_content(), "Turn 3");

    // With max_turns: 3, the tool exchange (Assistant + Tool) is included as 1 turn
    let config3 = ContextConfig {
        strategy: ContextStrategy::SlidingWindow,
        max_turns: Some(3),
        output_reserve: 0,
        permissive: false,
    };

    let trimmed3 = ContextManager::trim_messages(&messages, 100, &config3, |s| s.len())
        .expect("Trimming should succeed");
    assert_eq!(trimmed3.len(), 5);
    assert_eq!(trimmed3[0].role, "system");
    assert_eq!(trimmed3[1].role, "assistant");
    assert!(trimmed3[1].tool_calls.is_some());
    assert_eq!(trimmed3[2].role, "tool");
    assert_eq!(trimmed3[2].tool_call_id.as_deref(), Some("call_1"));
    assert_eq!(trimmed3[3].text_content(), "Result is 2");
    assert_eq!(trimmed3[4].text_content(), "Turn 3");
}

#[test]
fn test_orphaned_tool_message_dropped() {
    let orphaned_tool = OpenAIMessage::tool("orphaned result", "unknown_call", None);
    let messages = vec![
        OpenAIMessage::system("Sys"),
        OpenAIMessage::user("Hello"),
        orphaned_tool,
        OpenAIMessage::user("World"),
    ];

    let config = ContextConfig {
        strategy: ContextStrategy::NewestFirst,
        max_turns: None,
        output_reserve: 0,
        permissive: false,
    };

    let trimmed = ContextManager::trim_messages(&messages, 100, &config, |s| s.len())
        .expect("Trimming should succeed");
    assert_eq!(trimmed.len(), 3);
    assert_eq!(trimmed[0].role, "system");
    assert_eq!(trimmed[1].text_content(), "Hello");
    assert_eq!(trimmed[2].text_content(), "World");
    assert!(!trimmed.iter().any(|m| m.role == "tool"));
}
