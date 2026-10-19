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

#[test]
fn benchmark_context_compaction_throughput() {
    use apfel::core::context::ContextManager;

    let long_code = "fn process_data(data: &[u8]) -> Result<Vec<u8>, Error> {\n    let mut output = Vec::with_capacity(data.len());\n    for chunk in data.chunks(64) {\n        output.extend_from_slice(chunk);\n    }\n    Ok(output)\n}\n".repeat(200);

    let iterations = 1000;
    let start = Instant::now();
    let mut total_compacted_len = 0;

    for _ in 0..iterations {
        let compacted = ContextManager::compact_to_fit(&long_code, 500, &|s| s.len() / 4);
        total_compacted_len += compacted.len();
    }

    let elapsed = start.elapsed();
    let us_per_compaction = elapsed.as_micros() as f64 / iterations as f64;
    let compactions_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    println!("\n=======================================================");
    println!("    APFEL-RS CONTEXT COMPACTION BENCHMARK REPORT       ");
    println!("=======================================================");
    println!("  • Iterations:           {} compactions", iterations);
    println!("  • Total Elapsed:        {:.2?}", elapsed);
    println!("  • Latency/compaction:   {:.2} µs", us_per_compaction);
    println!("  • Throughput:           {:.0} compactions/sec", compactions_per_sec);
    println!("  • Total bytes:          {}", total_compacted_len);
    println!("=======================================================\n");

    assert!(compactions_per_sec > 500.0, "Compaction throughput too low: {:.0}", compactions_per_sec);
}

#[test]
fn benchmark_stop_sequence_matcher_streaming_throughput() {
    use apfel::core::stop_matcher::{StopMatchResult, StopSequenceMatcher};

    let stops = ["\nObservation:", "### End", "<|im_end|>"];
    let tokens = [
        "Thinking about ", "the algorithm ", "and structure.\n",
        "Step 1: Parse ", "input data.\n", "Step 2: Transform.\n",
        "Step 3: Emit ", "results.\nObservation: ", "Done."
    ];

    let iterations = 50_000;
    let start = Instant::now();
    let mut stopped_count = 0;

    for _ in 0..iterations {
        let mut matcher = StopSequenceMatcher::new(stops).expect("matcher creation");
        for token in &tokens {
            match matcher.feed(token) {
                StopMatchResult::Matched { .. } => {
                    stopped_count += 1;
                    break;
                }
                StopMatchResult::Emit(_) | StopMatchResult::Holding => {}
            }
        }
    }

    let elapsed = start.elapsed();
    let ns_per_stream = elapsed.as_nanos() as f64 / iterations as f64;
    let streams_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    println!("\n=======================================================");
    println!("   APFEL-RS STOP SEQUENCE MATCHER BENCHMARK REPORT     ");
    println!("=======================================================");
    println!("  • Iterations:           {} token streams", iterations);
    println!("  • Total Elapsed:        {:.2?}", elapsed);
    println!("  • Latency per stream:   {:.2} ns", ns_per_stream);
    println!("  • Throughput:           {:.0} streams/sec", streams_per_sec);
    println!("  • Stops detected:       {}", stopped_count);
    println!("=======================================================\n");

    assert_eq!(stopped_count, iterations);
    assert!(streams_per_sec > 50_000.0, "Stop matcher throughput too low: {:.0}", streams_per_sec);
}

#[test]
fn benchmark_shm_buffer_roundtrip_throughput() {
    use apfel::core::shm::ShmBuffer;

    let shm_name = format!("/apfel_bench_shm_{}", uuid::Uuid::new_v4().simple());
    let shm = ShmBuffer::create(&shm_name, 64 * 1024).expect("Failed to create shm");
    let payload = b"{\"model\": \"apple/foundation-model-v1\", \"content\": \"Benchmarking zero-copy POSIX shared memory\"}";

    let iterations = 20_000;
    let start = Instant::now();
    let mut total_bytes_read = 0;

    for _ in 0..iterations {
        shm.write_payload(payload).expect("write failed");
        let read_back = shm.read_payload();
        total_bytes_read += read_back.len();
    }

    let elapsed = start.elapsed();
    let ns_per_roundtrip = elapsed.as_nanos() as f64 / iterations as f64;
    let roundtrips_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    println!("\n=======================================================");
    println!("       APFEL-RS POSIX SHM IPC BENCHMARK REPORT         ");
    println!("=======================================================");
    println!("  • Iterations:           {} roundtrips", iterations);
    println!("  • Total Elapsed:        {:.2?}", elapsed);
    println!("  • Latency/roundtrip:    {:.2} ns", ns_per_roundtrip);
    println!("  • Roundtrip Throughput: {:.0} ops/sec", roundtrips_per_sec);
    println!("  • Total bytes read:     {}", total_bytes_read);
    println!("=======================================================\n");

    assert!(roundtrips_per_sec > 10_000.0, "SHM roundtrip throughput too low: {:.0}", roundtrips_per_sec);
}

#[test]
fn benchmark_apfs_cow_sandbox_throughput() {
    use apfel::core::apfs::ApfsSandbox;

    let temp_dir = tempfile::tempdir().expect("Failed to create tempdir");
    let ws = temp_dir.path();
    for i in 0..10 {
        std::fs::write(ws.join(format!("file_{}.txt", i)), "Pragmatic performance verification").unwrap();
    }

    let iterations = 50;
    let start = Instant::now();

    for i in 0..iterations {
        let mut sandbox = ApfsSandbox::begin(ws).expect("begin sandbox");
        std::fs::write(ws.join("file_0.txt"), format!("corrupted {}", i)).unwrap();
        sandbox.rollback().expect("rollback");
    }

    let elapsed = start.elapsed();
    let ms_per_cycle = (elapsed.as_secs_f64() * 1000.0) / iterations as f64;
    let cycles_per_sec = (iterations as f64) / elapsed.as_secs_f64();

    println!("\n=======================================================");
    println!("       APFEL-RS APFS SANDBOX BENCHMARK REPORT          ");
    println!("=======================================================");
    println!("  • Iterations:           {} begin + rollback cycles", iterations);
    println!("  • Total Elapsed:        {:.2?}", elapsed);
    println!("  • Latency per cycle:    {:.2} ms", ms_per_cycle);
    println!("  • Throughput:           {:.1} cycles/sec", cycles_per_sec);
    println!("=======================================================\n");

    assert!(cycles_per_sec > 5.0, "APFS sandbox throughput too low: {:.1}", cycles_per_sec);
}
