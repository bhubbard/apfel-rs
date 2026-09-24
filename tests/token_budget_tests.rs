// ============================================================================
// tests/token_budget_tests.rs — Tests for token budgeting and context tracking
// Ported from: TokenBudgetTests.swift, TokenCountFallbackTests.swift, ContextWindowTests.swift
// ============================================================================

use apfel::core::token_counter::TokenCounter;

#[test]
fn test_default_context_size_and_input_budget() {
    let counter = TokenCounter::new();
    assert_eq!(counter.context_size(), 4096);

    // Default reserve = 512
    assert_eq!(counter.input_budget(512), 3584);
    assert_eq!(counter.input_budget(1024), 3072);

    // Over budget saturation
    assert_eq!(counter.input_budget(5000), 0);
}

#[test]
fn test_observe_context_size_growth() {
    let counter = TokenCounter::new();
    assert_eq!(counter.context_size(), 4096);

    // Smaller or equal size does not reduce it
    assert_eq!(counter.observe_context_size(2048), 4096);
    assert_eq!(counter.context_size(), 4096);

    // Larger size expands the observed context window
    assert_eq!(counter.observe_context_size(8192), 8192);
    assert_eq!(counter.context_size(), 8192);
    assert_eq!(counter.input_budget(512), 8192 - 512);
}

#[test]
fn test_token_counter_caching() {
    let counter = TokenCounter::new();
    let text = "Hello world, testing token cache behavior";

    let mut compute_calls = 0;
    let count1 = counter.count_cached(text, |s| {
        compute_calls += 1;
        s.len() / 4
    });

    let count2 = counter.count_cached(text, |_| {
        compute_calls += 1;
        999
    });

    assert_eq!(count1, count2);
    assert_eq!(compute_calls, 1, "Cache hit must prevent re-computing");
}

#[test]
fn test_fallback_token_counting() {
    let counter = TokenCounter::new();
    assert!(!counter.did_fallback());

    let count = counter.fallback_count("12345678");
    assert_eq!(count, 2);
    assert!(counter.did_fallback());

    counter.reset_fallback_flag();
    assert!(!counter.did_fallback());
}

#[test]
fn test_token_counter_throughput_benchmark() {
    let counter = TokenCounter::new();
    let text = "Performance stress testing for token hash cache in apfel-rs";

    let start = std::time::Instant::now();
    for _ in 0..10_000 {
        let count = counter.count_cached(text, |s| s.len() / 4);
        assert!(count > 0);
    }
    let duration = start.elapsed();
    // 10,000 lookups should take well under 100ms with u64 hash lookups
    assert!(duration.as_millis() < 100, "10k hash cache lookups took {:?}", duration);
}

