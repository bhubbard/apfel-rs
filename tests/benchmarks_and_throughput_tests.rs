// ============================================================================
// tests/benchmarks_and_throughput_tests.rs — Comprehensive Benchmarks for apfel-rs
// ============================================================================

use apfel::core::token_counter::TokenCounter;
use apfel::core::tool_call::ToolCallHandler;
use std::time::Instant;

#[test]
fn benchmark_token_counter_throughput() {
    let counter = TokenCounter::new();
    let sample_texts = [
        "The quick brown fox jumps over the lazy dog.",
        "Apple Intelligence FoundationModels on-device inference pipeline.",
        "A long conversation turn with multiple system instructions, tools, and message frames.",
        "fn main() { println!(\"Hello, world!\"); }",
        "Short.",
    ];

    let iterations = 20_000;
    let start = Instant::now();
    let mut total_tokens = 0;

    for i in 0..iterations {
        let text = sample_texts[i % sample_texts.len()];
        let count = counter.count_cached(text, |s| s.len() / 4);
        total_tokens += count;
    }

    let elapsed = start.elapsed();
    let ns_per_lookup = elapsed.as_nanos() as f64 / iterations as f64;
    let lookups_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    println!("\n=======================================================");
    println!("      APFEL-RS TOKEN COUNTER BENCHMARK REPORT          ");
    println!("=======================================================");
    println!("  • Iterations:           {} lookups", iterations);
    println!("  • Total Elapsed:        {:.2?}", elapsed);
    println!("  • Latency per lookup:   {:.2} ns", ns_per_lookup);
    println!(
        "  • Throughput:           {:.0} lookups/sec",
        lookups_per_sec
    );
    println!("  • Total tokens counted: {}", total_tokens);
    println!("=======================================================\n");

    // Must exceed 100k lookups/sec
    assert!(
        lookups_per_sec > 100_000.0,
        "Lookup throughput too low: {:.0}",
        lookups_per_sec
    );
}

#[test]
fn benchmark_tool_call_detection_throughput() {
    let positive_json = r#"{"tool_calls": [{"id": "call_123", "type": "function", "function": {"name": "get_weather", "arguments": {"location": "San Francisco, CA"}}}]}"#;
    let positive_fence = format!("```json\n{}\n```", positive_json);
    let negative_prose =
        "Here is an explanation of the climate in northern California during the winter months.";

    let samples = [positive_json, &positive_fence, negative_prose];
    let iterations = 15_000;
    let start = Instant::now();
    let mut detected_count = 0;

    for i in 0..iterations {
        let text = samples[i % samples.len()];
        if ToolCallHandler::detect_tool_call(text).is_some() {
            detected_count += 1;
        }
    }

    let elapsed = start.elapsed();
    let us_per_parse = elapsed.as_micros() as f64 / iterations as f64;
    let parses_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    println!("\n=======================================================");
    println!("     APFEL-RS TOOL CALL DETECTION BENCHMARK REPORT     ");
    println!("=======================================================");
    println!(
        "  • Iterations:           {} candidate responses",
        iterations
    );
    println!("  • Total Elapsed:        {:.2?}", elapsed);
    println!("  • Latency per parse:    {:.2} µs", us_per_parse);
    println!("  • Detection throughput: {:.0} parses/sec", parses_per_sec);
    println!("  • Valid tool calls:     {}", detected_count);
    println!("=======================================================\n");

    // 2/3 of inputs have tool calls
    assert_eq!(detected_count, 10_000);
    assert!(
        parses_per_sec > 10_000.0,
        "Parsing throughput too low: {:.0}",
        parses_per_sec
    );
}
