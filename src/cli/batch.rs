// ============================================================================
// batch.rs — Incremental JSONL batch processor
// Part of apfel-rs (conforming to Arthur-Ficial/apfel #481)
// ============================================================================

use crate::backend::engine::BackendEngine;
use crate::backend::session::SessionManager;
use crate::cli::args::CliArgs;
use crate::core::context::{ContextConfig, ContextStrategy};
use crate::core::error::ApfelExitCodes;
use crate::core::models::OpenAIMessage;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::str::FromStr;
use std::sync::Arc;

/// An input record received from stdin JSONL.
#[derive(Debug, Deserialize)]
pub struct BatchInputRecord {
    pub custom_id: Option<String>,
    pub prompt: Option<String>,
    pub messages: Option<Vec<OpenAIMessage>>,
}

/// An output record written to stdout JSONL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchOutputRecord {
    pub line: usize,
    pub custom_id: Option<String>,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Runs the --batch incremental processor reading UTF-8 JSONL from stdin.
pub async fn run_batch_mode(args: CliArgs, engine: Arc<dyn BackendEngine>) -> i32 {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run_batch_stream(stdin.lock(), stdout.lock(), args, engine).await
}

/// Core stream processor for --batch mode taking any BufRead and Write handles.
pub async fn run_batch_stream<R: BufRead, W: Write>(
    mut reader: R,
    mut out_handle: W,
    args: CliArgs,
    engine: Arc<dyn BackendEngine>,
) -> i32 {
    let mut line_number = 0;
    let mut any_failure = false;

    let config = ContextConfig {
        strategy: ContextStrategy::from_str(&args.context_strategy).unwrap_or_default(),
        max_turns: args.max_turns,
        output_reserve: 512,
        permissive: args.permissive,
    };

    let session_mgr = SessionManager::new(engine.clone(), None);

    let mut line_buffer = String::new();
    loop {
        line_buffer.clear();
        match reader.read_line(&mut line_buffer) {
            Ok(0) => break,
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading stdin line {}: {}", line_number + 1, e);
                return ApfelExitCodes::RUNTIME_ERROR;
            }
        }
        line_number += 1;

        let trimmed = line_buffer.trim();
        if trimmed.is_empty() {
            continue;
        }

        let record: Result<BatchInputRecord, _> = serde_json::from_str(trimmed);
        let record = match record {
            Ok(r) => r,
            Err(e) => {
                any_failure = true;
                let out_err = BatchOutputRecord {
                    line: line_number,
                    custom_id: None,
                    status: "error".to_string(),
                    content: None,
                    finish_reason: Some("error".to_string()),
                    error: Some(format!("Invalid JSON line: {}", e)),
                };
                if let Ok(json_str) = serde_json::to_string(&out_err) {
                    let _ = writeln!(out_handle, "{}", json_str);
                    let _ = out_handle.flush();
                }
                continue;
            }
        };

        // Validate that exactly one of prompt or messages is provided
        let active_messages = match (&record.prompt, &record.messages) {
            (Some(p), None) => vec![OpenAIMessage::user(p)],
            (None, Some(m)) => {
                if m.is_empty() {
                    any_failure = true;
                    let out_err = BatchOutputRecord {
                        line: line_number,
                        custom_id: record.custom_id,
                        status: "error".to_string(),
                        content: None,
                        finish_reason: Some("error".to_string()),
                        error: Some("Record messages array cannot be empty".to_string()),
                    };
                    if let Ok(json_str) = serde_json::to_string(&out_err) {
                        let _ = writeln!(out_handle, "{}", json_str);
                        let _ = out_handle.flush();
                    }
                    continue;
                }
                m.clone()
            }
            (Some(_), Some(_)) => {
                any_failure = true;
                let out_err = BatchOutputRecord {
                    line: line_number,
                    custom_id: record.custom_id,
                    status: "error".to_string(),
                    content: None,
                    finish_reason: Some("error".to_string()),
                    error: Some(
                        "Record must contain exactly one of 'prompt' or 'messages', not both"
                            .to_string(),
                    ),
                };
                if let Ok(json_str) = serde_json::to_string(&out_err) {
                    let _ = writeln!(out_handle, "{}", json_str);
                    let _ = out_handle.flush();
                }
                continue;
            }
            (None, None) => {
                any_failure = true;
                let out_err = BatchOutputRecord {
                    line: line_number,
                    custom_id: record.custom_id,
                    status: "error".to_string(),
                    content: None,
                    finish_reason: Some("error".to_string()),
                    error: Some("Record must contain either 'prompt' or 'messages'".to_string()),
                };
                if let Ok(json_str) = serde_json::to_string(&out_err) {
                    let _ = writeln!(out_handle, "{}", json_str);
                    let _ = out_handle.flush();
                }
                continue;
            }
        };

        let mut full_messages = active_messages;
        if let Some(sys) = &args.system {
            full_messages.insert(0, OpenAIMessage::system(sys));
        }

        // Execute inference with fresh isolated conversation state
        match session_mgr
            .process_messages(
                &full_messages,
                None,
                &config,
                args.temperature,
                args.top_p,
                args.max_tokens,
                args.seed,
            )
            .await
        {
            Ok(res) => {
                let out_rec = BatchOutputRecord {
                    line: line_number,
                    custom_id: record.custom_id,
                    status: "ok".to_string(),
                    content: Some(res.content),
                    finish_reason: Some(res.finish_reason),
                    error: None,
                };
                if let Ok(json_str) = serde_json::to_string(&out_rec) {
                    let _ = writeln!(out_handle, "{}", json_str);
                    let _ = out_handle.flush();
                }
            }
            Err(e) => {
                if matches!(e, crate::core::error::ApfelError::ModelUnavailable(_)) {
                    eprintln!(
                        "Fatal model unavailable error during batch processing: {}",
                        e
                    );
                    return ApfelExitCodes::MODEL_UNAVAILABLE;
                }

                any_failure = true;
                let out_err = BatchOutputRecord {
                    line: line_number,
                    custom_id: record.custom_id,
                    status: "error".to_string(),
                    content: None,
                    finish_reason: Some("error".to_string()),
                    error: Some(e.to_string()),
                };
                if let Ok(json_str) = serde_json::to_string(&out_err) {
                    let _ = writeln!(out_handle, "{}", json_str);
                    let _ = out_handle.flush();
                }
            }
        }
    }

    if any_failure {
        ApfelExitCodes::RUNTIME_ERROR
    } else {
        ApfelExitCodes::SUCCESS
    }
}
