// ============================================================================
// tests/cli_runner_tests.rs — Unit tests for CLI runner and dispatcher
// ============================================================================

use apfel::backend::mock::MockEngine;
use apfel::cli::args::CliArgs;
use apfel::cli::chat::save_conversation;
use apfel::cli::runner::{run_cli, run_cli_with_engine};
use apfel::core::error::ApfelExitCodes;
use apfel::core::models::OpenAIMessage;
use clap::Parser;
use std::sync::Arc;

#[tokio::test]
async fn test_cli_runner_model_info() {
    let args = CliArgs::parse_from(&["apfel", "--model-info"]);
    let code = run_cli(args).await;
    assert_eq!(code, ApfelExitCodes::SUCCESS);
}

#[tokio::test]
async fn test_cli_runner_completions() {
    let args_bash = CliArgs::parse_from(&["apfel", "--completions", "bash"]);
    assert_eq!(run_cli(args_bash).await, ApfelExitCodes::SUCCESS);

    let args_zsh = CliArgs::parse_from(&["apfel", "--completions", "zsh"]);
    assert_eq!(run_cli(args_zsh).await, ApfelExitCodes::SUCCESS);

    let args_bad = CliArgs::parse_from(&["apfel", "--completions", "invalid_shell"]);
    assert_eq!(run_cli(args_bad).await, ApfelExitCodes::USAGE_ERROR);
}

#[tokio::test]
async fn test_cli_runner_count_tokens() {
    let args = CliArgs::parse_from(&["apfel", "--count-tokens", "Test token counting prompt"]);
    assert_eq!(run_cli(args).await, ApfelExitCodes::SUCCESS);

    // JSON output format
    let args_json = CliArgs::parse_from(&["apfel", "--count-tokens", "-o", "json", "Test json token counting"]);
    assert_eq!(run_cli(args_json).await, ApfelExitCodes::SUCCESS);

    // Missing file attachment fails with usage error
    let args_file = CliArgs::parse_from(&["apfel", "--count-tokens", "-f", "/nonexistent/file/path/that/does/not/exist.txt"]);
    assert_eq!(run_cli(args_file).await, ApfelExitCodes::USAGE_ERROR);
}

#[tokio::test]
async fn test_cli_runner_benchmark() {
    let args = CliArgs::parse_from(&["apfel", "--benchmark"]);
    let code = run_cli(args).await;
    assert_eq!(code, ApfelExitCodes::SUCCESS);
}

#[tokio::test]
async fn test_cli_save_conversation_json_and_markdown() {
    let dir = tempfile::tempdir().unwrap();
    let json_path = dir.path().join("chat.json");
    let md_path = dir.path().join("chat.md");

    let history = vec![
        OpenAIMessage::user("Hello assistant"),
        OpenAIMessage::assistant("Hello user!"),
    ];

    // Save JSON
    let res_json = save_conversation(json_path.to_str().unwrap(), &history);
    assert!(res_json.is_ok());
    let json_content = std::fs::read_to_string(&json_path).unwrap();
    assert!(json_content.contains("Hello assistant"));

    // Save Markdown
    let res_md = save_conversation(md_path.to_str().unwrap(), &history);
    assert!(res_md.is_ok());
    let md_content = std::fs::read_to_string(&md_path).unwrap();
    assert!(md_content.contains("### USER"));
    assert!(md_content.contains("### ASSISTANT"));
}

#[tokio::test]
async fn test_cli_runner_generation_mock_engine() {
    let mock = Arc::new(MockEngine::with_response("```rust\nfn main() {}\n```"));

    // Generation with prompt
    let args = CliArgs::parse_from(&["apfel", "--no-stream", "Write code"]);
    assert_eq!(run_cli_with_engine(args, mock.clone()).await, ApfelExitCodes::SUCCESS);

    // Generation with code cropper extracting code
    let args_code = CliArgs::parse_from(&["apfel", "--code", "--no-stream", "Write code"]);
    assert_eq!(run_cli_with_engine(args_code, mock.clone()).await, ApfelExitCodes::SUCCESS);

    // Generation with code cropper failing when no code
    let mock_no_code = Arc::new(MockEngine::with_response("Just plain text with no code fence"));
    let args_no_code = CliArgs::parse_from(&["apfel", "--code", "--no-stream", "Explain"]);
    assert_eq!(run_cli_with_engine(args_no_code, mock_no_code).await, ApfelExitCodes::NO_CODE);

    // Generation with JSON output
    let args_json = CliArgs::parse_from(&["apfel", "-o", "json", "--no-stream", "Generate json"]);
    assert_eq!(run_cli_with_engine(args_json, mock.clone()).await, ApfelExitCodes::SUCCESS);

    // Generation with missing prompt fails with USAGE_ERROR
    let args_no_prompt = CliArgs::parse_from(&["apfel", "--no-stream"]);
    assert_eq!(run_cli_with_engine(args_no_prompt, mock.clone()).await, ApfelExitCodes::USAGE_ERROR);

    // Generation with non-existent file fails with USAGE_ERROR
    let args_bad_file = CliArgs::parse_from(&["apfel", "-f", "/path/to/invalid/file.txt", "prompt"]);
    assert_eq!(run_cli_with_engine(args_bad_file, mock.clone()).await, ApfelExitCodes::USAGE_ERROR);

    // Generation with valid file
    let dir = tempfile::tempdir().unwrap();
    let sample_file = dir.path().join("context.txt");
    std::fs::write(&sample_file, "Contextual file data").unwrap();
    let args_good_file = CliArgs::parse_from(&["apfel", "-f", sample_file.to_str().unwrap(), "--no-stream", "prompt"]);
    assert_eq!(run_cli_with_engine(args_good_file, mock.clone()).await, ApfelExitCodes::SUCCESS);

    // Generation with valid schema file
    let schema_file = dir.path().join("schema.json");
    std::fs::write(&schema_file, r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#).unwrap();
    let args_schema = CliArgs::parse_from(&["apfel", "--schema", schema_file.to_str().unwrap(), "--no-stream", "prompt"]);
    assert_eq!(run_cli_with_engine(args_schema, mock.clone()).await, ApfelExitCodes::SUCCESS);

    // Generation with invalid schema file
    let bad_schema_file = dir.path().join("bad_schema.json");
    std::fs::write(&bad_schema_file, r#"{ invalid json }"#).unwrap();
    let args_bad_schema = CliArgs::parse_from(&["apfel", "--schema", bad_schema_file.to_str().unwrap(), "--no-stream", "prompt"]);
    assert_eq!(run_cli_with_engine(args_bad_schema, mock.clone()).await, ApfelExitCodes::USAGE_ERROR);
}

#[tokio::test]
async fn test_cli_runner_count_tokens_strict_overflow() {
    let mock = Arc::new(MockEngine::new());
    let huge_prompt = "a".repeat(20000);
    let args = CliArgs::parse_from(&["apfel", "--count-tokens", "--strict", &huge_prompt]);
    assert_eq!(run_cli_with_engine(args, mock).await, ApfelExitCodes::CONTEXT_OVERFLOW);
}

#[tokio::test]
async fn test_cli_runner_messages_flag() {
    let mock = Arc::new(MockEngine::with_response("Hello from messages response"));
    let dir = tempfile::tempdir().unwrap();

    // Valid messages JSON array
    let msg_file = dir.path().join("messages.json");
    std::fs::write(&msg_file, r#"[{"role": "user", "content": "Hello from file"}]"#).unwrap();
    let args_good = CliArgs::parse_from(&["apfel", "--messages", msg_file.to_str().unwrap(), "--no-stream"]);
    assert_eq!(run_cli_with_engine(args_good, mock.clone()).await, ApfelExitCodes::SUCCESS);

    // Missing messages file
    let args_missing = CliArgs::parse_from(&["apfel", "--messages", "/nonexistent/messages.json", "--no-stream"]);
    assert_eq!(run_cli_with_engine(args_missing, mock.clone()).await, ApfelExitCodes::USAGE_ERROR);

    // Corrupt messages file
    let bad_msg_file = dir.path().join("bad_messages.json");
    std::fs::write(&bad_msg_file, r#"[{"role": "unknown_role", "content": "Hello"}]"#).unwrap();
    let args_bad = CliArgs::parse_from(&["apfel", "--messages", bad_msg_file.to_str().unwrap(), "--no-stream"]);
    assert_eq!(run_cli_with_engine(args_bad, mock.clone()).await, ApfelExitCodes::USAGE_ERROR);
}
