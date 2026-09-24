// ============================================================================
// tests/stream_sink_tests.rs — Tests for StreamPrintSink deduplication on retry
// Ported from: StreamPrintSinkTests.swift
// ============================================================================

use apfel::core::stream_sink::StreamPrintSink;

#[test]
fn test_single_growing_stream_emits_deltas_in_order() {
    let mut sink = StreamPrintSink::new();
    let mut recorded = Vec::new();

    for snap in ["He", "Hello", "Hello, wo", "Hello, world"] {
        if let Some(delta) = sink.feed(snap) {
            recorded.push(delta.to_string());
        }
    }

    assert_eq!(recorded, vec!["He", "llo", ", wo", "rld"]);
    let joined = recorded.join("");
    assert_eq!(joined, "Hello, world");
}

#[test]
fn test_retry_restreaming_prefix_does_not_reprint() {
    let mut sink = StreamPrintSink::new();
    let mut recorded = Vec::new();

    // Attempt 1 streams and fails after "Hello, wo"
    for snap in ["He", "Hello", "Hello, wo"] {
        if let Some(delta) = sink.feed(snap) {
            recorded.push(delta.to_string());
        }
    }
    assert_eq!(recorded, vec!["He", "llo", ", wo"]);

    // Attempt 2 (retry) restarts from scratch and reaches "Hello, world!"
    for snap in ["He", "Hello", "Hello, wo", "Hello, world!"] {
        if let Some(delta) = sink.feed(snap) {
            recorded.push(delta.to_string());
        }
    }

    assert_eq!(recorded, vec!["He", "llo", ", wo", "rld!"]);
    let joined = recorded.join("");
    assert_eq!(joined, "Hello, world!");
}

#[test]
fn test_shorter_snapshot_emits_nothing() {
    let mut sink = StreamPrintSink::new();
    let mut recorded = Vec::new();

    if let Some(delta) = sink.feed("Hello, world") {
        recorded.push(delta.to_string());
    }

    // Shorter snapshots during retry startup
    assert_eq!(sink.feed("He"), None);
    assert_eq!(sink.feed("Hello"), None);

    assert_eq!(recorded, vec!["Hello, world"]);
}
