use apfel::core::context::{ContextConfig, ContextManager, ContextStrategy};
use apfel::core::models::OpenAIMessage;

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
    let trimmed = ContextManager::trim_messages(&messages, 6, &config, |s| s.split_whitespace().count())
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
