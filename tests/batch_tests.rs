// ============================================================================
// tests/batch_tests.rs — Unit & integration tests for --batch JSONL processing
// Part of apfel-rs (conforming to Arthur-Ficial/apfel #481)
// ============================================================================

use apfel::backend::mock::MockEngine;
use apfel::cli::args::CliArgs;
use apfel::cli::batch::{run_batch_stream, BatchOutputRecord};
use apfel::core::error::ApfelExitCodes;
use clap::Parser;
use std::io::Cursor;
use std::sync::Arc;

#[tokio::test]
async fn test_batch_processing_prompt_stream() {
    let mock = Arc::new(MockEngine::with_response("Classified: billing"));
    let input_jsonl = "\
{\"custom_id\": \"ticket-1\", \"prompt\": \"I was double charged\"}
{\"custom_id\": \"ticket-2\", \"prompt\": \"How do I reset password?\"}
";
    let reader = Cursor::new(input_jsonl.as_bytes());
    let mut out_buffer = Vec::new();

    let args = CliArgs::parse_from(["apfel", "--batch"]);
    let exit_code = run_batch_stream(reader, &mut out_buffer, args, mock).await;

    assert_eq!(exit_code, ApfelExitCodes::SUCCESS);

    let output_str = String::from_utf8(out_buffer).unwrap();
    let lines: Vec<&str> = output_str.lines().collect();
    assert_eq!(lines.len(), 2);

    let rec1: BatchOutputRecord = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(rec1.line, 1);
    assert_eq!(rec1.custom_id, Some("ticket-1".to_string()));
    assert_eq!(rec1.status, "ok");
    assert_eq!(rec1.content, Some("Classified: billing".to_string()));
    assert_eq!(rec1.finish_reason, Some("stop".to_string()));

    let rec2: BatchOutputRecord = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(rec2.line, 2);
    assert_eq!(rec2.custom_id, Some("ticket-2".to_string()));
    assert_eq!(rec2.status, "ok");
}

#[tokio::test]
async fn test_batch_processing_messages_stream() {
    let mock = Arc::new(MockEngine::with_response("Hello from batch!"));
    let input_jsonl = "\
{\"custom_id\": \"conv-1\", \"messages\": [{\"role\": \"user\", \"content\": \"Hi\"}]}
";
    let reader = Cursor::new(input_jsonl.as_bytes());
    let mut out_buffer = Vec::new();

    let args = CliArgs::parse_from(["apfel", "--batch"]);
    let exit_code = run_batch_stream(reader, &mut out_buffer, args, mock).await;

    assert_eq!(exit_code, ApfelExitCodes::SUCCESS);

    let output_str = String::from_utf8(out_buffer).unwrap();
    let lines: Vec<&str> = output_str.lines().collect();
    assert_eq!(lines.len(), 1);

    let rec: BatchOutputRecord = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(rec.custom_id, Some("conv-1".to_string()));
    assert_eq!(rec.status, "ok");
    assert_eq!(rec.content, Some("Hello from batch!".to_string()));
}

#[tokio::test]
async fn test_batch_processing_malformed_and_error_lines() {
    let mock = Arc::new(MockEngine::with_response("ok"));
    let input_jsonl = "\
{\"custom_id\": \"good-1\", \"prompt\": \"Valid line\"}
{ malformed json line }
{\"custom_id\": \"bad-both\", \"prompt\": \"Prompt\", \"messages\": [{\"role\": \"user\", \"content\": \"Hi\"}]}
{\"custom_id\": \"bad-neither\"}
{\"custom_id\": \"bad-empty-msgs\", \"messages\": []}
{\"custom_id\": \"good-2\", \"prompt\": \"Another valid line\"}
";
    let reader = Cursor::new(input_jsonl.as_bytes());
    let mut out_buffer = Vec::new();

    let args = CliArgs::parse_from(["apfel", "--batch"]);
    let exit_code = run_batch_stream(reader, &mut out_buffer, args, mock).await;

    // Must yield RUNTIME_ERROR (exit code 1) on any line failure
    assert_eq!(exit_code, ApfelExitCodes::RUNTIME_ERROR);

    let output_str = String::from_utf8(out_buffer).unwrap();
    let lines: Vec<&str> = output_str.lines().collect();
    assert_eq!(lines.len(), 6);

    let rec1: BatchOutputRecord = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(rec1.status, "ok");

    let rec2: BatchOutputRecord = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(rec2.status, "error");
    assert_eq!(rec2.line, 2);
    assert!(rec2.error.unwrap().contains("Invalid JSON line"));

    let rec3: BatchOutputRecord = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(rec3.status, "error");
    assert!(rec3.error.unwrap().contains("not both"));

    let rec4: BatchOutputRecord = serde_json::from_str(lines[3]).unwrap();
    assert_eq!(rec4.status, "error");
    assert!(rec4
        .error
        .unwrap()
        .contains("either 'prompt' or 'messages'"));

    let rec5: BatchOutputRecord = serde_json::from_str(lines[4]).unwrap();
    assert_eq!(rec5.status, "error");
    assert!(rec5
        .error
        .unwrap()
        .contains("messages array cannot be empty"));

    let rec6: BatchOutputRecord = serde_json::from_str(lines[5]).unwrap();
    assert_eq!(rec6.status, "ok");
}
