// ============================================================================
// tests/messages_flag_tests.rs — Tests for MessagesInput decoding rules
// Ported from: MessagesFlagTests.swift
// ============================================================================

use apfel::core::messages_input::MessagesInput;

#[test]
fn test_decodes_bare_json_array_of_messages() {
    let raw = r#"[
        {"role":"user","content":"What is the capital of Austria?"},
        {"role":"assistant","content":"Vienna."},
        {"role":"user","content":"And its population?"}
    ]"#;

    let msgs = MessagesInput::decode(raw).unwrap();
    assert_eq!(msgs.len(), 3);
    assert_eq!(msgs[0].role, "user");
    assert_eq!(msgs[1].role, "assistant");
    assert_eq!(msgs[2].text_content(), "And its population?");
}

#[test]
fn test_decodes_object_with_messages_key() {
    let raw = r#"{
        "messages": [
            {"role":"system","content":"Be terse."},
            {"role":"user","content":"hi"}
        ]
    }"#;

    let msgs = MessagesInput::decode(raw).unwrap();
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].role, "system");
    assert_eq!(msgs[1].role, "user");
}

#[test]
fn test_invalid_json_fails() {
    assert!(MessagesInput::decode("{nope").is_err());
    assert!(MessagesInput::decode("not json").is_err());
    assert!(MessagesInput::decode("").is_err());
}

#[test]
fn test_empty_array_fails() {
    assert!(MessagesInput::decode("[]").is_err());
    assert!(MessagesInput::decode(r#"{"messages":[]}"#).is_err());
}

#[test]
fn test_unknown_role_fails() {
    let raw = r#"[{"role":"wizard","content":"alakazam"}]"#;
    let err = MessagesInput::decode(raw).unwrap_err();
    assert!(err.to_string().contains("unknown message role: wizard"));
}
