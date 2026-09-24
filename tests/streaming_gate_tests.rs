// ============================================================================
// tests/streaming_gate_tests.rs — Tests for StreamingToolCallGate decision logic
// Ported from: StreamingToolCallGateTests.swift
// ============================================================================

use apfel::core::tool_call::StreamingToolCallGate;

#[test]
fn test_hold_empty_or_whitespace() {
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix(""));
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("   \n\t "));
}

#[test]
fn test_hold_single_brace_or_partial_json() {
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("{"));
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("{\"tool"));
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("{\"tool_calls"));
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("{\"tool_calls\""));
}

#[test]
fn test_hold_committed_tool_calls_object() {
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix(
        "{\"tool_calls\": [{\"id\": \"call_1\", \"type\": \"function\""
    ));
}

#[test]
fn test_hold_leading_whitespace_before_brace() {
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix(
        "\n  {\"tool_calls\": ["
    ));
}

#[test]
fn test_hold_partial_or_full_code_fences() {
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("`"));
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("``"));
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("```"));
    assert!(StreamingToolCallGate::is_plausible_tool_call_prefix("```json\n{\"tool_calls\": ["));
}

#[test]
fn test_flush_plain_prose_immediately() {
    assert!(!StreamingToolCallGate::is_plausible_tool_call_prefix("Sure, here is"));
    assert!(!StreamingToolCallGate::is_plausible_tool_call_prefix("Hello, world!"));
}

#[test]
fn test_flush_json_not_tool_calls() {
    assert!(!StreamingToolCallGate::is_plausible_tool_call_prefix("{\"answer\": 42}"));
}
