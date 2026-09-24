// ============================================================================
// tests/sampling_and_retry_tests.rs — Tests for retryability and sampling options
// Ported from: RetryTests.swift, SamplingDecisionTests.swift
// ============================================================================

use apfel::backend::engine::GenerateRequest;
use apfel::core::error::ApfelError;

#[test]
fn test_is_retryable_rate_limited() {
    let err = ApfelError::RateLimited("Rate limited. Try again later.".into());
    assert!(err.is_retryable());
}

#[test]
fn test_is_retryable_concurrent_request_or_busy() {
    let err = ApfelError::ModelUnavailable("Too many concurrent requests".into());
    assert!(err.is_retryable());

    let err2 = ApfelError::ModelUnavailable("Model busy with background task".into());
    assert!(err2.is_retryable());

    let err3 = ApfelError::ModelUnavailable("Assets temporarily unavailable".into());
    assert!(err3.is_retryable());
}

#[test]
fn test_non_retryable_errors() {
    assert!(!ApfelError::Guardrail("Safety policy violated".into()).is_retryable());
    assert!(!ApfelError::ContextOverflow("Max context exceeded".into()).is_retryable());
    assert!(!ApfelError::Usage("Bad parameter".into()).is_retryable());
    assert!(!ApfelError::NoCodeFound.is_retryable());
    assert!(!ApfelError::ToolExecution("Command failed".into()).is_retryable());
    assert!(!ApfelError::Unauthorized("Invalid key".into()).is_retryable());
    assert!(!ApfelError::ForbiddenOrigin("evil.com".into()).is_retryable());
    assert!(!ApfelError::Runtime("Fatal error".into()).is_retryable());
}

#[test]
fn test_sampling_options_in_generate_request() {
    // Greedy: temperature 0
    let req_greedy = GenerateRequest {
        prompt: "test".into(),
        system_prompt: None,
        messages: None,
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(100),
        permissive: false,
        seed: None,
    };
    assert_eq!(req_greedy.temperature, Some(0.0));
    assert_eq!(req_greedy.top_p, None);

    // Nucleus sampling with seed
    let req_nucleus = GenerateRequest {
        prompt: "test".into(),
        system_prompt: None,
        messages: None,
        temperature: Some(0.7),
        top_p: Some(0.9),
        max_tokens: Some(250),
        permissive: true,
        seed: Some(1337),
    };
    assert_eq!(req_nucleus.top_p, Some(0.9));
    assert_eq!(req_nucleus.seed, Some(1337));
    assert!(req_nucleus.permissive);
}
