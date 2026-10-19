// ============================================================================
// examples/bench_compare.rs — Benchmark Regression & Comparison Suite for apfel-rs
// Tracks performance across git commits and updates with automated delta tables.
//
// Usage:
//   cargo run --release --example bench_compare
//   cargo run --release --example bench_compare -- --save baseline.json
//   cargo run --release --example bench_compare -- --compare baseline.json
//   cargo run --release --example bench_compare -- --compare baseline.json --max-regression 5.0
// ============================================================================

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use colored::Colorize;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MetricEntry {
    pub name: String,
    pub category: String,
    pub value: f64,
    pub unit: String,
    pub higher_is_better: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BenchmarkReport {
    pub timestamp: String,
    pub git_commit: Option<String>,
    pub platform: String,
    pub metrics: Vec<MetricEntry>,
}

fn get_git_commit() -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
}

fn run_token_counting_benchmarks() -> Vec<MetricEntry> {
    use apfel::core::token_counter::TokenCounter;

    let counter = TokenCounter::new();
    let short_text = "The quick brown fox jumps over the lazy dog.";
    let long_corpus = "Apple Intelligence on-device FoundationModels with safe Swift and Rust concurrency. \
                       High-performance LLM runtime and IPC zero-copy mechanisms. "
        .repeat(100);

    // 1. Short Token Counting
    let iterations = 50_000;
    let t0 = Instant::now();
    for _ in 0..iterations {
        let _ = counter.count_cached(short_text, |s| s.len() / 4);
    }
    let elapsed = t0.elapsed();
    let short_throughput = iterations as f64 / elapsed.as_secs_f64();

    // 2. Long Corpus Token Counting
    let iterations_long = 10_000;
    let t1 = Instant::now();
    for _ in 0..iterations_long {
        let _ = counter.count_cached(&long_corpus, |s| s.len() / 4);
    }
    let elapsed_long = t1.elapsed();
    let long_throughput = iterations_long as f64 / elapsed_long.as_secs_f64();

    vec![
        MetricEntry {
            name: "Token Counter (Short Text)".to_string(),
            category: "Core Tokenizer".to_string(),
            value: short_throughput,
            unit: "ops/sec".to_string(),
            higher_is_better: true,
        },
        MetricEntry {
            name: "Token Counter (Long Corpus)".to_string(),
            category: "Core Tokenizer".to_string(),
            value: long_throughput,
            unit: "ops/sec".to_string(),
            higher_is_better: true,
        },
    ]
}

fn run_context_compaction_benchmark() -> Vec<MetricEntry> {
    use apfel::core::context::ContextManager;

    let code_sample = "fn process_data(data: &[u8]) -> Result<Vec<u8>, Error> {\n    let mut out = Vec::with_capacity(data.len());\n    out.extend_from_slice(data);\n    Ok(out)\n}\n".repeat(150);
    let iterations = 2_000;
    let t0 = Instant::now();
    for _ in 0..iterations {
        let _ = ContextManager::compact_to_fit(&code_sample, 400, &|s| s.len() / 4);
    }
    let elapsed = t0.elapsed();
    let throughput = iterations as f64 / elapsed.as_secs_f64();

    vec![MetricEntry {
        name: "Context Compaction Speed".to_string(),
        category: "Context Engine".to_string(),
        value: throughput,
        unit: "ops/sec".to_string(),
        higher_is_better: true,
    }]
}

fn run_stop_matcher_benchmark() -> Vec<MetricEntry> {
    use apfel::core::stop_matcher::{StopMatchResult, StopSequenceMatcher};

    let stops = ["\nObservation:", "### End", "<|im_end|>"];
    let tokens = [
        "Analyzing user query... ",
        "Generating solution step-by-step.\n",
        "Step 1: Parse AST.\n",
        "Step 2: Check types.\n",
        "Step 3: Emit IR.\nObservation: Done.",
    ];

    let iterations = 100_000;
    let t0 = Instant::now();
    for _ in 0..iterations {
        let mut matcher = StopSequenceMatcher::new(stops).expect("matcher");
        for tok in &tokens {
            if let StopMatchResult::Matched { .. } = matcher.feed(tok) {
                break;
            }
        }
    }
    let elapsed = t0.elapsed();
    let throughput = iterations as f64 / elapsed.as_secs_f64();

    vec![MetricEntry {
        name: "Stop Sequence Stream Matcher".to_string(),
        category: "Streaming Delimiters".to_string(),
        value: throughput,
        unit: "streams/sec".to_string(),
        higher_is_better: true,
    }]
}

fn run_shm_roundtrip_benchmark() -> Vec<MetricEntry> {
    use apfel::core::shm::ShmBuffer;

    let shm_name = format!("/apfel_cmp_shm_{}", uuid::Uuid::new_v4().simple());
    let shm = ShmBuffer::create(&shm_name, 64 * 1024).expect("shm create");
    let payload = b"{\"action\": \"inference\", \"prompt\": \"apple intelligence on-device benchmark\", \"tokens\": 128}";

    let iterations = 50_000;
    let t0 = Instant::now();
    for _ in 0..iterations {
        let _ = shm.write_payload(payload);
        let _ = shm.read_payload();
    }
    let elapsed = t0.elapsed();
    let throughput = iterations as f64 / elapsed.as_secs_f64();

    vec![MetricEntry {
        name: "POSIX SHM Zero-Copy Roundtrip".to_string(),
        category: "IPC & Shared Memory".to_string(),
        value: throughput,
        unit: "ops/sec".to_string(),
        higher_is_better: true,
    }]
}

fn run_apfs_sandbox_benchmark() -> Vec<MetricEntry> {
    use apfel::core::apfs::ApfsSandbox;

    let temp_dir = tempfile::tempdir().expect("tempdir");
    let ws = temp_dir.path();
    for i in 0..15 {
        let _ = std::fs::write(ws.join(format!("module_{}.rs", i)), "pub fn run() { println!(\"ok\"); }");
    }

    let iterations = 100;
    let t0 = Instant::now();
    for i in 0..iterations {
        let mut sandbox = ApfsSandbox::begin(ws).expect("sandbox");
        let _ = std::fs::write(ws.join("module_0.rs"), format!("corrupted {}", i));
        let _ = sandbox.rollback();
    }
    let elapsed = t0.elapsed();
    let cycles_per_sec = iterations as f64 / elapsed.as_secs_f64();

    vec![MetricEntry {
        name: "APFS CoW Sandbox (Begin+Rollback)".to_string(),
        category: "Safety & Sandboxing".to_string(),
        value: cycles_per_sec,
        unit: "cycles/sec".to_string(),
        higher_is_better: true,
    }]
}

fn run_tool_call_benchmark() -> Vec<MetricEntry> {
    use apfel::core::tool_call::ToolCallHandler;

    let positive_json = r#"{"tool_calls": [{"id": "c1", "type": "function", "function": {"name": "search", "arguments": {"query": "apple"}}}]}"#;
    let prose = "This is a standard text response explaining the results of the calculation.";

    let iterations = 30_000;
    let t0 = Instant::now();
    for i in 0..iterations {
        let text = if i % 2 == 0 { positive_json } else { prose };
        let _ = ToolCallHandler::detect_tool_call(text);
    }
    let elapsed = t0.elapsed();
    let throughput = iterations as f64 / elapsed.as_secs_f64();

    vec![MetricEntry {
        name: "Tool Call Detection Throughput".to_string(),
        category: "Tool Calling".to_string(),
        value: throughput,
        unit: "parses/sec".to_string(),
        higher_is_better: true,
    }]
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut save_path: Option<PathBuf> = None;
    let mut compare_path: Option<PathBuf> = None;
    let mut max_regression_pct: f64 = 10.0;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--save" => {
                if i + 1 < args.len() {
                    save_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--compare" => {
                if i + 1 < args.len() {
                    compare_path = Some(PathBuf::from(&args[i + 1]));
                    i += 1;
                }
            }
            "--max-regression" => {
                if i + 1 < args.len() {
                    if let Ok(pct) = args[i + 1].parse::<f64>() {
                        max_regression_pct = pct;
                    }
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("Usage: cargo run --release --example bench_compare -- [options]");
                println!();
                println!("Options:");
                println!("  --save <path>             Save benchmark results as baseline JSON file");
                println!("  --compare <path>          Compare current performance against baseline JSON");
                println!("  --max-regression <pct>    Maximum allowed regression percentage (default: 10.0)");
                println!("  --help, -h                Show this help message");
                return Ok(());
            }
            _ => {}
        }
        i += 1;
    }

    println!("{}", "=======================================================".cyan().bold());
    println!("{}", "   APFEL-RS REGRESSION & PERFORMANCE COMPARISON SUITE   ".cyan().bold());
    println!("{}", "=======================================================".cyan().bold());

    print!("Running benchmarks... ");
    let mut metrics = Vec::new();
    metrics.extend(run_token_counting_benchmarks());
    metrics.extend(run_context_compaction_benchmark());
    metrics.extend(run_stop_matcher_benchmark());
    metrics.extend(run_shm_roundtrip_benchmark());
    metrics.extend(run_apfs_sandbox_benchmark());
    metrics.extend(run_tool_call_benchmark());
    println!("{}", "Done!".green().bold());

    let report = BenchmarkReport {
        timestamp: chrono::Utc::now().to_rfc3339(),
        git_commit: get_git_commit(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        metrics: metrics.clone(),
    };

    if let Some(ref path) = save_path {
        let json = serde_json::to_string_pretty(&report)?;
        std::fs::write(path, json)?;
        println!("\n{} Saved baseline benchmark report to: {}", "✓".green().bold(), path.display().to_string().yellow());
        return Ok(());
    }

    if let Some(ref path) = compare_path {
        if !path.exists() {
            eprintln!("{}: Baseline file '{}' not found", "Error".red().bold(), path.display());
            std::process::exit(1);
        }
        let content = std::fs::read_to_string(path)?;
        let baseline: BenchmarkReport = serde_json::from_str(&content)?;

        let baseline_map: HashMap<String, f64> = baseline
            .metrics
            .into_iter()
            .map(|m| (m.name, m.value))
            .collect();

        println!("\nComparing against baseline from {} (git: {}):", baseline.timestamp.yellow(), baseline.git_commit.unwrap_or_else(|| "unknown".into()).cyan());
        println!("{:-<100}", "");
        println!(
            "{:<36} | {:>14} | {:>14} | {:>12} | {:<12}",
            "Benchmark Metric", "Baseline", "Current", "Delta (%)", "Status"
        );
        println!("{:-<100}", "");

        let mut has_regression = false;

        for m in &metrics {
            if let Some(&base_val) = baseline_map.get(&m.name) {
                let delta_pct = if m.higher_is_better {
                    ((m.value - base_val) / base_val) * 100.0
                } else {
                    ((base_val - m.value) / base_val) * 100.0
                };

                let (delta_str, status_str) = if delta_pct >= 2.0 {
                    (
                        format!("+{:>5.2}%", delta_pct).green().bold(),
                        "FASTER".green().bold(),
                    )
                } else if delta_pct <= -max_regression_pct {
                    has_regression = true;
                    (
                        format!("{:>6.2}%", delta_pct).red().bold(),
                        "REGRESSION".red().bold(),
                    )
                } else if delta_pct < 0.0 {
                    (
                        format!("{:>6.2}%", delta_pct).yellow(),
                        "SLOWER".yellow(),
                    )
                } else {
                    (
                        format!("+{:>5.2}%", delta_pct).normal(),
                        "PARITY".normal(),
                    )
                };

                println!(
                    "{:<36} | {:>11.1} {} | {:>11.1} {} | {:>12} | {:<12}",
                    m.name, base_val, m.unit, m.value, m.unit, delta_str, status_str
                );
            } else {
                println!(
                    "{:<36} | {:>14} | {:>11.1} {} | {:>12} | {:<12}",
                    m.name, "NEW", m.value, m.unit, "N/A", "NEW".cyan()
                );
            }
        }
        println!("{:-<100}", "");

        if has_regression {
            eprintln!("\n{}: One or more benchmarks regressed beyond the {:.1}% threshold!", "FAILED".red().bold(), max_regression_pct);
            std::process::exit(1);
        } else {
            println!("\n{}: All performance metrics are within threshold (≤{:.1}% regression)!", "SUCCESS".green().bold(), max_regression_pct);
        }
    } else {
        println!("\nCurrent Benchmark Results (Commit: {}):", report.git_commit.unwrap_or_else(|| "working tree".into()).cyan());
        println!("{:-<80}", "");
        println!("{:<36} | {:>18} | {:<20}", "Benchmark Metric", "Performance", "Category");
        println!("{:-<80}", "");
        for m in &metrics {
            println!("{:<36} | {:>12.1} {:<5} | {:<20}", m.name, m.value, m.unit, m.category);
        }
        println!("{:-<80}", "");
        println!("\n{}", "Tips:".yellow().bold());
        println!("  • Save as baseline:    cargo run --release --example bench_compare -- --save baseline.json");
        println!("  • Compare to baseline: cargo run --release --example bench_compare -- --compare baseline.json");
    }

    Ok(())
}
