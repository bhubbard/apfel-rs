use apfel::core::security::{scrub_mcp_environment, OriginValidator};
use std::collections::HashMap;

#[test]
fn test_origin_validator() {
    let allowed = vec![
        "http://localhost".to_string(),
        "http://127.0.0.1".to_string(),
    ];

    // Non-browser client has no origin => always allowed
    assert!(OriginValidator::is_allowed(None, &allowed));

    // Exact matches
    assert!(OriginValidator::is_allowed(Some("http://localhost"), &allowed));
    assert!(OriginValidator::is_allowed(Some("http://127.0.0.1"), &allowed));

    // Port variants
    assert!(OriginValidator::is_allowed(Some("http://localhost:3000"), &allowed));
    assert!(OriginValidator::is_allowed(Some("http://127.0.0.1:8080"), &allowed));

    // HTTPS variants
    assert!(OriginValidator::is_allowed(Some("https://localhost"), &allowed));
    assert!(OriginValidator::is_allowed(Some("https://localhost:8443"), &allowed));

    // Subdomain attack must be blocked!
    assert!(!OriginValidator::is_allowed(Some("http://localhost.evil.com"), &allowed));
    assert!(!OriginValidator::is_allowed(Some("http://127.0.0.1.attacker.com"), &allowed));
    assert!(!OriginValidator::is_allowed(Some("http://external-site.com"), &allowed));

    // Wildcard allows everything
    let wildcard = vec!["*".to_string()];
    assert!(OriginValidator::is_allowed(Some("http://any-site.com"), &wildcard));
}

#[test]
fn test_constant_time_equals() {
    assert!(OriginValidator::constant_time_equals("secret123", "secret123"));
    assert!(!OriginValidator::constant_time_equals("secret123", "secret124"));
    assert!(!OriginValidator::constant_time_equals("secret123", "secret"));
    assert!(!OriginValidator::constant_time_equals("", "secret"));
}

#[test]
fn test_bearer_token_validation() {
    let expected = Some("my-secret-token");

    // Valid formats
    assert!(OriginValidator::is_valid_token(Some("Bearer my-secret-token"), expected));
    assert!(OriginValidator::is_valid_token(Some("my-secret-token"), expected));

    // Invalid tokens
    assert!(!OriginValidator::is_valid_token(Some("Bearer wrong-token"), expected));
    assert!(!OriginValidator::is_valid_token(Some(""), expected));
    assert!(!OriginValidator::is_valid_token(None, expected));

    // No auth required
    assert!(OriginValidator::is_valid_token(None, None));
}

#[test]
fn test_scrub_mcp_environment() {
    let mut env = HashMap::new();
    env.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
    env.insert("HOME".to_string(), "/Users/test".to_string());
    env.insert("APFEL_TOKEN".to_string(), "secret123".to_string());
    env.insert("AWS_SECRET_ACCESS_KEY".to_string(), "supersecret".to_string());
    env.insert("OPENAI_API_KEY".to_string(), "key123".to_string());

    let scrubbed = scrub_mcp_environment(&env);
    assert!(scrubbed.contains_key("PATH"));
    assert!(scrubbed.contains_key("HOME"));
    assert!(!scrubbed.contains_key("APFEL_TOKEN"));
    assert!(!scrubbed.contains_key("AWS_SECRET_ACCESS_KEY"));
    assert!(!scrubbed.contains_key("OPENAI_API_KEY"));
}

#[test]
fn test_origin_validation_constrains() {
    let normal = vec!["http://localhost:5173".to_string()];
    assert!(OriginValidator::origin_validation_constrains(false, &normal));

    // Footgun disables constraint
    assert!(!OriginValidator::origin_validation_constrains(true, &normal));

    // Wildcard disables constraint
    let wildcard = vec!["*".to_string()];
    assert!(!OriginValidator::origin_validation_constrains(false, &wildcard));
}

