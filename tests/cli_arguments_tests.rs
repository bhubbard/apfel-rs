// ============================================================================
// tests/cli_arguments_tests.rs — Tests for CLI argument parsing
// Ported from: CLIArgumentsTests.swift, CLIErrorsTests.swift, CLIValidateTests.swift
// ============================================================================

use apfel::cli::args::CliArgs;
use clap::Parser;

#[test]
fn test_parse_default_args() {
    let args = CliArgs::try_parse_from(["apfel"]).unwrap();
    assert_eq!(args.prompt, None);
    assert!(!args.stream);
    assert!(!args.no_stream);
    assert!(!args.chat);
    assert!(!args.serve);
    assert_eq!(args.port, 8080);
    assert_eq!(args.host, "127.0.0.1");
    assert_eq!(args.output, "text");
    assert_eq!(args.context_strategy, "newest-first");
}

#[test]
fn test_parse_simple_prompt() {
    let args = CliArgs::try_parse_from(["apfel", "Hello from Rust"]).unwrap();
    assert_eq!(args.prompt, Some("Hello from Rust".into()));
}

#[test]
fn test_parse_serve_flags() {
    let args = CliArgs::try_parse_from([
        "apfel",
        "--serve",
        "--port",
        "9000",
        "--host",
        "0.0.0.0",
        "--footgun",
        "--allowed-origins",
        "http://foo.com,http://bar.com",
    ])
    .unwrap();

    assert!(args.serve);
    assert_eq!(args.port, 9000);
    assert_eq!(args.host, "0.0.0.0");
    assert!(args.footgun);
    assert_eq!(
        args.allowed_origins,
        vec!["http://foo.com".to_string(), "http://bar.com".to_string()]
    );
}

#[test]
fn test_parse_sampling_parameters() {
    let args = CliArgs::try_parse_from([
        "apfel",
        "--temperature",
        "0.7",
        "--top-p",
        "0.9",
        "--max-tokens",
        "256",
        "--seed",
        "42",
        "generate text",
    ])
    .unwrap();

    assert_eq!(args.temperature, Some(0.7));
    assert_eq!(args.top_p, Some(0.9));
    assert_eq!(args.max_tokens, Some(256));
    assert_eq!(args.seed, Some(42));
    assert_eq!(args.prompt, Some("generate text".into()));
}

#[test]
fn test_parse_file_attachments() {
    let args = CliArgs::try_parse_from([
        "apfel",
        "-f",
        "main.rs",
        "-f",
        "lib.rs",
        "Review these files",
    ])
    .unwrap();

    assert_eq!(args.file, vec!["main.rs", "lib.rs"]);
    assert_eq!(args.prompt, Some("Review these files".into()));
}

#[test]
fn test_parse_mcp_servers() {
    let args = CliArgs::try_parse_from([
        "apfel",
        "--mcp-server",
        "./mcp_math.py",
        "--mcp-server",
        "npx -y @modelcontextprotocol/server-postgres",
        "Calculate 2+2",
    ])
    .unwrap();

    assert_eq!(args.mcp_servers.len(), 2);
    assert_eq!(args.mcp_servers[0], "./mcp_math.py");
}

#[test]
fn test_parse_context_strategy_options() {
    let args = CliArgs::try_parse_from([
        "apfel",
        "--context-strategy",
        "sliding-window",
        "--max-turns",
        "10",
        "--chat",
    ])
    .unwrap();

    assert!(args.chat);
    assert_eq!(args.context_strategy, "sliding-window");
    assert_eq!(args.max_turns, Some(10));
}
