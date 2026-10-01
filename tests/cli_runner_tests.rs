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
    let args_json = CliArgs::parse_from(&[
        "apfel",
        "--count-tokens",
        "-o",
        "json",
        "Test json token counting",
    ]);
    assert_eq!(run_cli(args_json).await, ApfelExitCodes::SUCCESS);

    // Missing file attachment fails with usage error
    let args_file = CliArgs::parse_from(&[
        "apfel",
        "--count-tokens",
        "-f",
        "/nonexistent/file/path/that/does/not/exist.txt",
    ]);
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
    assert_eq!(
        run_cli_with_engine(args, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // Generation with JSON output
    let args_json = CliArgs::parse_from(&["apfel", "-o", "json", "--no-stream", "Generate json"]);
    assert_eq!(
        run_cli_with_engine(args_json, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // Generation with missing prompt fails with USAGE_ERROR
    let args_no_prompt = CliArgs::parse_from(&["apfel", "--no-stream"]);
    assert_eq!(
        run_cli_with_engine(args_no_prompt, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // Generation with non-existent file fails with USAGE_ERROR
    let args_bad_file =
        CliArgs::parse_from(&["apfel", "-f", "/path/to/invalid/file.txt", "prompt"]);
    assert_eq!(
        run_cli_with_engine(args_bad_file, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // Generation with valid file
    let dir = tempfile::tempdir().unwrap();
    let sample_file = dir.path().join("context.txt");
    std::fs::write(&sample_file, "Contextual file data").unwrap();
    let args_good_file = CliArgs::parse_from(&[
        "apfel",
        "-f",
        sample_file.to_str().unwrap(),
        "--no-stream",
        "prompt",
    ]);
    assert_eq!(
        run_cli_with_engine(args_good_file, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // Generation with valid schema file
    let schema_file = dir.path().join("schema.json");
    std::fs::write(
        &schema_file,
        r#"{"type": "object", "properties": {"name": {"type": "string"}}}"#,
    )
    .unwrap();
    let args_schema = CliArgs::parse_from(&[
        "apfel",
        "--schema",
        schema_file.to_str().unwrap(),
        "--no-stream",
        "prompt",
    ]);
    assert_eq!(
        run_cli_with_engine(args_schema, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // Generation with invalid schema file
    let bad_schema_file = dir.path().join("bad_schema.json");
    std::fs::write(&bad_schema_file, r#"{ invalid json }"#).unwrap();
    let args_bad_schema = CliArgs::parse_from(&[
        "apfel",
        "--schema",
        bad_schema_file.to_str().unwrap(),
        "--no-stream",
        "prompt",
    ]);
    assert_eq!(
        run_cli_with_engine(args_bad_schema, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );
}

#[tokio::test]
async fn test_cli_runner_count_tokens_strict_overflow() {
    let mock = Arc::new(MockEngine::new());
    let huge_prompt = "a".repeat(20000);
    let args = CliArgs::parse_from(&["apfel", "--count-tokens", "--strict", &huge_prompt]);
    assert_eq!(
        run_cli_with_engine(args, mock).await,
        ApfelExitCodes::CONTEXT_OVERFLOW
    );
}

#[tokio::test]
async fn test_cli_runner_messages_flag() {
    let mock = Arc::new(MockEngine::with_response("Hello from messages response"));
    let dir = tempfile::tempdir().unwrap();

    // Valid messages JSON array
    let msg_file = dir.path().join("messages.json");
    std::fs::write(
        &msg_file,
        r#"[{"role": "user", "content": "Hello from file"}]"#,
    )
    .unwrap();
    let args_good = CliArgs::parse_from(&[
        "apfel",
        "--messages",
        msg_file.to_str().unwrap(),
        "--no-stream",
    ]);
    assert_eq!(
        run_cli_with_engine(args_good, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // Missing messages file
    let args_missing = CliArgs::parse_from(&[
        "apfel",
        "--messages",
        "/nonexistent/messages.json",
        "--no-stream",
    ]);
    assert_eq!(
        run_cli_with_engine(args_missing, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // Corrupt messages file
    let bad_msg_file = dir.path().join("bad_messages.json");
    std::fs::write(
        &bad_msg_file,
        r#"[{"role": "unknown_role", "content": "Hello"}]"#,
    )
    .unwrap();
    let args_bad = CliArgs::parse_from(&[
        "apfel",
        "--messages",
        bad_msg_file.to_str().unwrap(),
        "--no-stream",
    ]);
    assert_eq!(
        run_cli_with_engine(args_bad, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );
}

#[tokio::test]
async fn test_cli_runner_code_flag_success() {
    let mock = Arc::new(MockEngine::with_response(
        "Here is the function:\n```python\ndef greet():\n    return 'hello'\n```\nAll done!",
    ));

    // Plain text mode
    let args = CliArgs::parse_from(&["apfel", "--code", "Write a greeting function"]);
    assert_eq!(
        run_cli_with_engine(args, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // JSON mode
    let args_json =
        CliArgs::parse_from(&["apfel", "--code", "-o", "json", "Write a greeting function"]);
    assert_eq!(
        run_cli_with_engine(args_json, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );
}

#[tokio::test]
async fn test_cli_runner_code_flag_rejections() {
    let mock = Arc::new(MockEngine::new());

    // --code --stream conflict
    let args_stream = CliArgs::parse_from(&["apfel", "--code", "--stream", "Prompt"]);
    assert_eq!(
        run_cli_with_engine(args_stream, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // --code --chat conflict
    let args_chat = CliArgs::parse_from(&["apfel", "--code", "--chat"]);
    assert_eq!(
        run_cli_with_engine(args_chat, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // --code --serve conflict
    let args_serve = CliArgs::parse_from(&["apfel", "--code", "--serve"]);
    assert_eq!(
        run_cli_with_engine(args_serve, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // --code --schema conflict
    let args_schema =
        CliArgs::parse_from(&["apfel", "--code", "--schema", "schema.json", "Prompt"]);
    assert_eq!(
        run_cli_with_engine(args_schema, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );
}

#[tokio::test]
async fn test_cli_runner_code_flag_no_code_exits_7() {
    let mock = Arc::new(MockEngine::with_response("   \n\n   "));
    let args = CliArgs::parse_from(&["apfel", "--code", "-q", "Prompt"]);
    assert_eq!(
        run_cli_with_engine(args, mock.clone()).await,
        ApfelExitCodes::NO_CODE
    );
}

#[tokio::test]
async fn test_cli_runner_require_complete_flag() {
    // Truncated response with finish_reason: "length"
    let mock_truncated = Arc::new(MockEngine::with_finish_reason(
        "Truncated response...",
        "length",
    ));

    // With --require-complete: must return exit code 8
    let args_req = CliArgs::parse_from(&["apfel", "--no-stream", "--require-complete", "Prompt"]);
    assert_eq!(
        run_cli_with_engine(args_req, mock_truncated.clone()).await,
        ApfelExitCodes::INCOMPLETE_RESPONSE
    );

    // Without --require-complete: allows partial output, returns 0
    let args_normal = CliArgs::parse_from(&["apfel", "--no-stream", "Prompt"]);
    assert_eq!(
        run_cli_with_engine(args_normal, mock_truncated.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // Finished response with finish_reason: "stop" -> returns 0 with --require-complete
    let mock_complete = Arc::new(MockEngine::with_finish_reason("Complete response", "stop"));
    let args_complete =
        CliArgs::parse_from(&["apfel", "--no-stream", "--require-complete", "Prompt"]);
    assert_eq!(
        run_cli_with_engine(args_complete, mock_complete.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // Rejection in non-generation mode
    let args_bad_serve = CliArgs::parse_from(&["apfel", "--require-complete", "--serve"]);
    assert_eq!(
        run_cli_with_engine(args_bad_serve, mock_complete.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );
}

#[tokio::test]
async fn test_cli_runner_stop_flag() {
    let mock = Arc::new(MockEngine::with_response(
        "Action 1\nObservation: Stop before this\nAction 2",
    ));

    // Valid stop sequence truncates cleanly
    let args = CliArgs::parse_from(&["apfel", "--no-stream", "--stop", "\nObservation:", "Prompt"]);
    assert_eq!(
        run_cli_with_engine(args, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );

    // Empty stop sequence rejected with usage error (exit 2)
    let args_empty = CliArgs::parse_from(&["apfel", "--stop", "", "Prompt"]);
    assert_eq!(
        run_cli_with_engine(args_empty, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );
}

#[tokio::test]
async fn test_cli_runner_batch_flag_conflicts() {
    let mock = Arc::new(MockEngine::new());

    // --batch --stream conflict
    let args_stream = CliArgs::parse_from(&["apfel", "--batch", "--stream"]);
    assert_eq!(
        run_cli_with_engine(args_stream, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // --batch --chat conflict
    let args_chat = CliArgs::parse_from(&["apfel", "--batch", "--chat"]);
    assert_eq!(
        run_cli_with_engine(args_chat, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // --batch --serve conflict
    let args_serve = CliArgs::parse_from(&["apfel", "--batch", "--serve"]);
    assert_eq!(
        run_cli_with_engine(args_serve, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // --batch --benchmark conflict
    let args_bench = CliArgs::parse_from(&["apfel", "--batch", "--benchmark"]);
    assert_eq!(
        run_cli_with_engine(args_bench, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );
}

#[tokio::test]
async fn test_cli_runner_adapter_path_validation() {
    let mock = Arc::new(MockEngine::new());

    // Non-existent adapter path must fail with USAGE_ERROR
    let args_bad = CliArgs::parse_from(&[
        "apfel",
        "--adapter",
        "/nonexistent/path/to/my_adapter.fmadapter",
        "Hello",
    ]);
    assert_eq!(
        run_cli_with_engine(args_bad, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // Valid existing file path succeeds
    let dir = tempfile::tempdir().unwrap();
    let adapter_file = dir.path().join("fine_tuned.fmadapter");
    std::fs::write(&adapter_file, "adapter-weights-content").unwrap();

    let args_good = CliArgs::parse_from(&[
        "apfel",
        "--no-stream",
        "--adapter",
        adapter_file.to_str().unwrap(),
        "Hello",
    ]);
    assert_eq!(
        run_cli_with_engine(args_good, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );
}

#[tokio::test]
async fn test_cli_runner_env_variable_fallback_for_serve() {
    let mock = Arc::new(MockEngine::new());

    // Case 1: Explicit CLI flag --temperature passed to --serve -> must reject with USAGE_ERROR
    let mut args_explicit = CliArgs::parse_from(&["apfel", "--serve", "--temperature", "0.7"]);
    args_explicit.explicit_cli_args = Some(vec!["--serve".into(), "--temperature".into()]);
    assert_eq!(
        run_cli_with_engine(args_explicit, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // Case 2: Explicit CLI flag -t passed to --model-info -> must reject with USAGE_ERROR
    let mut args_explicit_info = CliArgs::parse_from(&["apfel", "--model-info", "-t", "0.7"]);
    args_explicit_info.explicit_cli_args = Some(vec!["--model-info".into(), "-t".into()]);
    assert_eq!(
        run_cli_with_engine(args_explicit_info, mock.clone()).await,
        ApfelExitCodes::USAGE_ERROR
    );

    // Case 3: APFEL_TEMPERATURE set in environment (NOT passed on CLI) -> must succeed without failing
    let mut args_env_only = CliArgs::parse_from(&["apfel", "--model-info"]);
    args_env_only.temperature = Some(0.7);
    args_env_only.explicit_cli_args = Some(vec!["--model-info".into()]);
    assert_eq!(
        run_cli_with_engine(args_env_only, mock.clone()).await,
        ApfelExitCodes::SUCCESS
    );
}

#[tokio::test]
async fn test_cli_runner_schema_raw_json() {
    let mock = Arc::new(MockEngine::with_response(r#"{"status": "ok", "count": 5}"#));
    let raw_schema = r#"{"type": "object", "properties": {"status": {"type": "string"}, "count": {"type": "integer"}}, "required": ["status", "count"]}"#;
    let args = CliArgs::parse_from(["apfel", "--schema", raw_schema, "Fetch metrics"]);
    assert_eq!(
        run_cli_with_engine(args, mock).await,
        ApfelExitCodes::SUCCESS
    );
}

#[tokio::test]
async fn test_cli_runner_schema_retry_success() {
    let mock = Arc::new(MockEngine::with_responses(vec![
        "Sorry, I didn't output JSON initially.",
        r#"{"status": "recovered", "count": 10}"#,
    ]));
    let raw_schema = r#"{"type": "object", "properties": {"status": {"type": "string"}, "count": {"type": "integer"}}, "required": ["status", "count"]}"#;
    let args = CliArgs::parse_from(["apfel", "--schema", raw_schema, "Fetch metrics"]);
    assert_eq!(
        run_cli_with_engine(args, mock).await,
        ApfelExitCodes::SUCCESS
    );
}

#[tokio::test]
async fn test_cli_runner_schema_exhaust_retries_require_complete() {
    let mock = Arc::new(MockEngine::with_response("This is never JSON"));
    let raw_schema =
        r#"{"type": "object", "properties": {"val": {"type": "integer"}}, "required": ["val"]}"#;
    let args = CliArgs::parse_from([
        "apfel",
        "--schema",
        raw_schema,
        "--require-complete",
        "Generate integer",
    ]);
    assert_eq!(
        run_cli_with_engine(args, mock).await,
        ApfelExitCodes::INCOMPLETE_RESPONSE
    );
}

#[tokio::test]
async fn test_cli_runner_schema_exhaust_retries_best_effort() {
    let mock = Arc::new(MockEngine::with_response("This is never JSON"));
    let raw_schema =
        r#"{"type": "object", "properties": {"val": {"type": "integer"}}, "required": ["val"]}"#;
    let args = CliArgs::parse_from(["apfel", "--schema", raw_schema, "Generate integer"]);
    assert_eq!(
        run_cli_with_engine(args, mock).await,
        ApfelExitCodes::SUCCESS
    );
}

#[tokio::test]
async fn test_cli_runner_mcp_config_failure() {
    let mock = Arc::new(MockEngine::new());
    let args = CliArgs::parse_from([
        "apfel",
        "--mcp-config",
        "/path/to/nonexistent/mcp_config.json",
        "Run something",
    ]);
    assert_eq!(
        run_cli_with_engine(args, mock).await,
        ApfelExitCodes::RUNTIME_ERROR
    );
}

#[tokio::test]
async fn test_cli_runner_image_attachment() {
    let mock = Arc::new(MockEngine::with_response("Image analysis response"));
    let dir = tempfile::tempdir().unwrap();
    let img_path = dir.path().join("photo.jpg");
    std::fs::write(&img_path, b"fake-jpg-content").unwrap();

    let args = CliArgs::parse_from([
        "apfel",
        "--image",
        img_path.to_str().unwrap(),
        "--no-stream",
        "Explain this image",
    ]);

    let code = run_cli_with_engine(args, mock.clone()).await;
    assert_eq!(code, ApfelExitCodes::SUCCESS);

    let last_req = mock.last_request().expect("Expected recorded request");
    assert!(last_req.prompt.contains("=== "));
    assert!(last_req.prompt.contains("(image) ==="));
    assert!(last_req.prompt.contains("what the image shows:"));
    assert!(last_req.prompt.contains("Explain this image"));
}

#[tokio::test]
async fn test_cli_runner_missing_image_attachment_fails() {
    let mock = Arc::new(MockEngine::new());
    let args = CliArgs::parse_from(["apfel", "--image", "/path/to/missing/image.png", "Describe"]);
    assert_eq!(
        run_cli_with_engine(args, mock).await,
        ApfelExitCodes::USAGE_ERROR
    );
}

#[test]
fn test_cli_install_completions_for_shells() {
    use apfel::cli::runner::install_completions_for_shell;

    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();

    // 1. Zsh
    let (zsh_path, zsh_hint) = install_completions_for_shell("zsh", home).unwrap();
    assert_eq!(
        zsh_path,
        home.join(".zsh").join("completions").join("_apfel")
    );
    assert!(zsh_path.exists());
    assert!(zsh_hint.contains("Installed zsh completions"));
    let zsh_content = std::fs::read_to_string(&zsh_path).unwrap();
    assert!(zsh_content.contains("apfel"));

    // 2. Bash
    let (bash_path, bash_hint) = install_completions_for_shell("bash", home).unwrap();
    assert_eq!(
        bash_path,
        home.join(".local")
            .join("share")
            .join("bash-completion")
            .join("completions")
            .join("apfel")
    );
    assert!(bash_path.exists());
    assert!(bash_hint.contains("Installed bash completions"));
    let bash_content = std::fs::read_to_string(&bash_path).unwrap();
    assert!(bash_content.contains("apfel"));

    // 3. Fish
    let (fish_path, fish_hint) = install_completions_for_shell("fish", home).unwrap();
    assert_eq!(
        fish_path,
        home.join(".config")
            .join("fish")
            .join("completions")
            .join("apfel.fish")
    );
    assert!(fish_path.exists());
    assert!(fish_hint.contains("Installed fish completions"));
    let fish_content = std::fs::read_to_string(&fish_path).unwrap();
    assert!(fish_content.contains("apfel"));

    // 4. Unsupported
    assert!(install_completions_for_shell("unknown_shell", home).is_err());
}
