// ============================================================================
// tests/apfel_error_tests.rs — Tests for Apfel errors and exit code mappings
// Ported from: ApfelErrorTests.swift, ExitCodeMapTests.swift, ApfelErrorMessageTests.swift
// ============================================================================

use apfel::core::error::{ApfelError, ApfelExitCodes};

#[test]
fn test_exit_codes_match_specification() {
    assert_eq!(ApfelExitCodes::SUCCESS, 0);
    assert_eq!(ApfelExitCodes::RUNTIME_ERROR, 1);
    assert_eq!(ApfelExitCodes::USAGE_ERROR, 2);
    assert_eq!(ApfelExitCodes::GUARDRAIL, 3);
    assert_eq!(ApfelExitCodes::CONTEXT_OVERFLOW, 4);
    assert_eq!(ApfelExitCodes::MODEL_UNAVAILABLE, 5);
    assert_eq!(ApfelExitCodes::RATE_LIMITED, 6);
    assert_eq!(ApfelExitCodes::NO_CODE, 7);
}

#[test]
fn test_error_exit_code_mapping() {
    assert_eq!(ApfelError::Usage("bad flag".into()).exit_code(), 2);
    assert_eq!(ApfelError::Guardrail("content blocked".into()).exit_code(), 3);
    assert_eq!(ApfelError::ContextOverflow("too long".into()).exit_code(), 4);
    assert_eq!(ApfelError::ModelUnavailable("not ready".into()).exit_code(), 5);
    assert_eq!(ApfelError::RateLimited("too many reqs".into()).exit_code(), 6);
    assert_eq!(ApfelError::NoCodeFound.exit_code(), 7);

    // Runtime errors
    assert_eq!(ApfelError::Runtime("crash".into()).exit_code(), 1);
    assert_eq!(ApfelError::ToolExecution("failed".into()).exit_code(), 1);
    assert_eq!(ApfelError::MCP("mcp down".into()).exit_code(), 1);
    assert_eq!(ApfelError::NotImplemented("todo".into()).exit_code(), 1);
    assert_eq!(ApfelError::Unauthorized("no token".into()).exit_code(), 1);
    assert_eq!(ApfelError::ForbiddenOrigin("bad origin".into()).exit_code(), 1);
}

#[test]
fn test_http_status_mapping() {
    assert_eq!(ApfelError::Usage("invalid param".into()).http_status(), 400);
    assert_eq!(ApfelError::Unauthorized("missing token".into()).http_status(), 401);
    assert_eq!(ApfelError::ForbiddenOrigin("evil.com".into()).http_status(), 403);
    assert_eq!(ApfelError::RateLimited("slow down".into()).http_status(), 429);
    assert_eq!(ApfelError::ModelUnavailable("disabled".into()).http_status(), 503);
    assert_eq!(ApfelError::NotImplemented("endpoint".into()).http_status(), 501);
    assert_eq!(ApfelError::Runtime("panic".into()).http_status(), 500);
}

#[test]
fn test_to_openai_json_structure() {
    let err = ApfelError::Usage("invalid model".into());
    let wrapper = err.to_openai_json();
    assert_eq!(wrapper.error.type_name, "invalid_request_error");
    assert!(wrapper.error.message.contains("usage error: invalid model"));

    let auth_err = ApfelError::Unauthorized("invalid token".into());
    let auth_wrapper = auth_err.to_openai_json();
    assert_eq!(auth_wrapper.error.type_name, "authentication_error");

    let rate_err = ApfelError::RateLimited("too fast".into());
    assert_eq!(rate_err.to_openai_json().error.type_name, "rate_limit_error");

    let perm_err = ApfelError::ForbiddenOrigin("untrusted".into());
    assert_eq!(perm_err.to_openai_json().error.type_name, "permission_error");
}

#[test]
fn test_error_display_strings() {
    assert_eq!(
        ApfelError::Usage("bad arg".into()).to_string(),
        "usage error: bad arg"
    );
    assert_eq!(
        ApfelError::ModelUnavailable("disabled".into()).to_string(),
        "model unavailable: disabled"
    );
    assert_eq!(
        ApfelError::NoCodeFound.to_string(),
        "no code block found in response"
    );
}
