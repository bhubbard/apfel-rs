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
    assert!(trimmed.iter().any(|m| m.text_content().contains("prior messages were summarized/truncated")));
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

    assert_eq!(ContextStrategy::from_str("newest-first").unwrap(), ContextStrategy::NewestFirst);
    assert_eq!(ContextStrategy::from_str("newest").unwrap(), ContextStrategy::NewestFirst);
    assert_eq!(ContextStrategy::from_str("oldest-first").unwrap(), ContextStrategy::OldestFirst);
    assert_eq!(ContextStrategy::from_str("oldest").unwrap(), ContextStrategy::OldestFirst);
    assert_eq!(ContextStrategy::from_str("sliding-window").unwrap(), ContextStrategy::SlidingWindow);
    assert_eq!(ContextStrategy::from_str("sliding").unwrap(), ContextStrategy::SlidingWindow);
    assert_eq!(ContextStrategy::from_str("summarize").unwrap(), ContextStrategy::Summarize);
    assert_eq!(ContextStrategy::from_str("strict").unwrap(), ContextStrategy::Strict);

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

