// ============================================================================
// examples/bench.rs — Native Rust Comparative Stress & Performance Benchmark
// Compares:
//   - Original Swift apfel: /opt/homebrew/bin/apfel
//   - Rust apfel-rs: target/release/apfel
//
// Run with:
//   cargo run --release --example bench
// ============================================================================

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

fn find_swift_binary() -> Option<PathBuf> {
    let homebrew_path = PathBuf::from("/opt/homebrew/bin/apfel");
    if homebrew_path.exists() {
        return Some(homebrew_path);
    }
    // Check `which apfel`
    if let Ok(output) = Command::new("which").arg("apfel").output() {
        if output.status.success() {
            let p = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !p.is_empty() && Path::new(&p).exists() {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
}

fn find_rust_binary() -> PathBuf {
    let candidate = PathBuf::from("target/release/apfel");
    if candidate.exists() {
        return candidate;
    }
    // Fall back to building it or checking target/debug
    PathBuf::from("target/debug/apfel")
}

fn get_rss_mb(pid: u32) -> f64 {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-p", &pid.to_string()])
        .output();
    if let Ok(out) = output {
        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if let Ok(kb) = text.parse::<f64>() {
            return kb / 1024.0;
        }
    }
    0.0
}

#[allow(dead_code)]
struct TestResult {
    swift_avg: f64,
    rust_avg: f64,
    speedup: f64,
}

fn test_1_token_counting_stress(swift_bin: Option<&Path>, rust_bin: &Path) -> Option<TestResult> {
    println!("\n=======================================================");
    println!(" [TEST 1] High-Volume Token Counting Under Load (30 runs)");
    println!("=======================================================");

    let corpus = "Apple Intelligence on-device FoundationModels with safe Swift and Rust concurrency. \
                  Modern systems programming requires memory safety without sacrificing throughput. "
        .repeat(50);
    let words = corpus.split_whitespace().count();
    println!("Payload size: {} characters (~{} words)", corpus.len(), words);

    let mut swift_durations = Vec::new();
    if let Some(swift) = swift_bin {
        for _ in 0..30 {
            let t0 = Instant::now();
            let _ = Command::new(swift)
                .args(["--count-tokens", &corpus])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output();
            swift_durations.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
    }

    let mut rust_durations = Vec::new();
    for _ in 0..30 {
        let t0 = Instant::now();
        let _ = Command::new(rust_bin)
            .args(["--count-tokens", &corpus])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        rust_durations.push(t0.elapsed().as_secs_f64() * 1000.0);
    }

    rust_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let rust_avg: f64 = rust_durations.iter().sum::<f64>() / rust_durations.len() as f64;
    let rust_p50 = rust_durations[rust_durations.len() / 2];
    let rust_min = rust_durations[0];
    let rust_max = rust_durations[rust_durations.len() - 1];

    if !swift_durations.is_empty() {
        swift_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let swift_avg: f64 = swift_durations.iter().sum::<f64>() / swift_durations.len() as f64;
        let swift_p50 = swift_durations[swift_durations.len() / 2];
        let swift_min = swift_durations[0];
        let swift_max = swift_durations[swift_durations.len() - 1];
        let speedup = swift_avg / rust_avg;

        println!(
            "Original Swift : mean = {:.2} ms | p50 = {:.2} ms (min: {:.2}, max: {:.2})",
            swift_avg, swift_p50, swift_min, swift_max
        );
        println!(
            "Rust (apfel-rs): mean = {:.2} ms | p50 = {:.2} ms (min: {:.2}, max: {:.2})",
            rust_avg, rust_p50, rust_min, rust_max
        );
        println!(
            "Advantage      : Rust is {:.2}x faster ({:.1}ms faster per operation)",
            speedup,
            swift_avg - rust_avg
        );

        Some(TestResult {
            swift_avg,
            rust_avg,
            speedup,
        })
    } else {
        println!(
            "Rust (apfel-rs): mean = {:.2} ms | p50 = {:.2} ms (min: {:.2}, max: {:.2})",
            rust_avg, rust_p50, rust_min, rust_max
        );
        None
    }
}

fn test_2_generation_throughput(swift_bin: Option<&Path>, rust_bin: &Path) {
    println!("\n=======================================================");
    println!(" [TEST 2] Heavy Generation Throughput & Latency (3 trials)");
    println!("=======================================================");

    let prompt = "Write a comprehensive technical overview detailing the architectural differences \
                  between monolithic kernels, microkernels, and hybrid kernels, with real-world examples.";
    println!("Prompt: \"{}...\"", &prompt[..60]);

    if let Some(swift) = swift_bin {
        let mut swift_times = Vec::new();
        let mut swift_chars = Vec::new();
        for _ in 0..3 {
            let t0 = Instant::now();
            if let Ok(out) = Command::new(swift)
                .args(["--temperature", "0", "--max-tokens", "600", prompt])
                .output()
            {
                if out.status.success() {
                    swift_times.push(t0.elapsed().as_secs_f64());
                    swift_chars.push(out.stdout.len());
                }
            }
        }
        if !swift_times.is_empty() {
            let avg_time: f64 = swift_times.iter().sum::<f64>() / swift_times.len() as f64;
            let avg_chars: f64 = swift_chars.iter().sum::<usize>() as f64 / swift_chars.len() as f64;
            let tps = (avg_chars / 4.0) / avg_time;
            println!(
                "Original Swift : {:.3} s total ({:.0} chars, ~{:.1} tokens/sec)",
                avg_time, avg_chars, tps
            );
        }
    }

    let mut rust_times = Vec::new();
    let mut rust_chars = Vec::new();
    for _ in 0..3 {
        let t0 = Instant::now();
        if let Ok(out) = Command::new(rust_bin)
            .args(["--temperature", "0", "--max-tokens", "600", prompt])
            .output()
        {
            if out.status.success() {
                rust_times.push(t0.elapsed().as_secs_f64());
                rust_chars.push(out.stdout.len());
            }
        }
    }
    if !rust_times.is_empty() {
        let avg_time: f64 = rust_times.iter().sum::<f64>() / rust_times.len() as f64;
        let avg_chars: f64 = rust_chars.iter().sum::<usize>() as f64 / rust_chars.len() as f64;
        let tps = (avg_chars / 4.0) / avg_time;
        println!(
            "Rust (apfel-rs): {:.3} s total ({:.0} chars, ~{:.1} tokens/sec)",
            avg_time, avg_chars, tps
        );
    }
}

struct ConcurrencyStats {
    total_dur: f64,
    rps: f64,
    p50: f64,
    p95: f64,
    rss_mb: f64,
}

async fn hammer_server(port: u16, total_requests: usize, concurrency: usize, child_pid: u32) -> ConcurrencyStats {
    let url = format!("http://127.0.0.1:{}/health", port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let latencies = Arc::new(Mutex::new(Vec::with_capacity(total_requests)));
    let t_start = Instant::now();

    let requests_per_worker = total_requests / concurrency;
    let mut handles = Vec::new();

    for _ in 0..concurrency {
        let client = client.clone();
        let url = url.clone();
        let latencies = Arc::clone(&latencies);

        let handle = tokio::spawn(async move {
            for _ in 0..requests_per_worker {
                let s = Instant::now();
                if let Ok(resp) = client.get(&url).send().await {
                    let _ = resp.bytes().await;
                    let dur = s.elapsed().as_secs_f64() * 1000.0;
                    latencies.lock().await.push(dur);
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.await;
    }

    let total_dur = t_start.elapsed().as_secs_f64();
    let rss_mb = get_rss_mb(child_pid);

    let mut lats = latencies.lock().await.clone();
    lats.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let rps = if total_dur > 0.0 {
        lats.len() as f64 / total_dur
    } else {
        0.0
    };

    let p50 = if !lats.is_empty() {
        lats[lats.len() / 2]
    } else {
        0.0
    };

    let p95_idx = ((lats.len() as f64) * 0.95) as usize;
    let p95 = if p95_idx < lats.len() {
        lats[p95_idx]
    } else if !lats.is_empty() {
        lats[lats.len() - 1]
    } else {
        0.0
    };

    ConcurrencyStats {
        total_dur,
        rps,
        p50,
        p95,
        rss_mb,
    }
}

async fn test_3_server_concurrency_stress(swift_bin: Option<&Path>, rust_bin: &Path) {
    println!("\n=======================================================");
    println!(" [TEST 3] Server Concurrency & Sustained Throughput (100 concurrent requests)");
    println!("=======================================================");

    let swift_port = 8911;
    let rust_port = 8912;

    let mut swift_child: Option<Child> = swift_bin.and_then(|bin| {
        Command::new(bin)
            .args(["--serve", "--port", &swift_port.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .ok()
    });

    let mut rust_child = Command::new(rust_bin)
        .args(["--serve", "--port", &rust_port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start Rust server");

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let swift_stats = if let Some(ref child) = swift_child {
        Some(hammer_server(swift_port, 100, 10, child.id()).await)
    } else {
        None
    };

    let rust_stats = hammer_server(rust_port, 100, 10, rust_child.id()).await;

    if let Some(mut child) = swift_child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = rust_child.kill();
    let _ = rust_child.wait();

    if let Some(s) = swift_stats {
        println!("Original Swift (Hummingbird):");
        println!("  • Total Time for 100 reqs : {:.3} s", s.total_dur);
        println!("  • Throughput (RPS)        : {:.1} req/sec", s.rps);
        println!("  • Latency Median (p50)    : {:.2} ms | p95: {:.2} ms", s.p50, s.p95);
        println!("  • Active Resident RAM     : {:.1} MB", s.rss_mb);

        println!("\nRust (apfel-rs Axum/Tokio):");
        println!("  • Total Time for 100 reqs : {:.3} s", rust_stats.total_dur);
        println!("  • Throughput (RPS)        : {:.1} req/sec", rust_stats.rps);
        println!(
            "  • Latency Median (p50)    : {:.2} ms | p95: {:.2} ms",
            rust_stats.p50, rust_stats.p95
        );
        println!("  • Active Resident RAM     : {:.1} MB", rust_stats.rss_mb);

        let speedup = rust_stats.rps / s.rps;
        let mem_saved = ((s.rss_mb - rust_stats.rss_mb) / s.rss_mb) * 100.0;
        println!("\nAdvantage:");
        println!("  • Throughput: Rust handles {:.2}x more requests/sec", speedup);
        println!(
            "  • Memory    : Rust uses {:.1}% less RAM under load ({:.1} MB vs {:.1} MB)",
            mem_saved, rust_stats.rss_mb, s.rss_mb
        );
    } else {
        println!("Rust (apfel-rs Axum/Tokio):");
        println!("  • Total Time for 100 reqs : {:.3} s", rust_stats.total_dur);
        println!("  • Throughput (RPS)        : {:.1} req/sec", rust_stats.rps);
        println!(
            "  • Latency Median (p50)    : {:.2} ms | p95: {:.2} ms",
            rust_stats.p50, rust_stats.p95
        );
        println!("  • Active Resident RAM     : {:.1} MB", rust_stats.rss_mb);
    }
}

#[tokio::main]
async fn main() {
    println!("#######################################################");
    println!("  INTENSIVE COMPARATIVE BENCHMARK: Swift vs Rust       ");
    println!("#######################################################");

    let swift_bin = find_swift_binary();
    let rust_bin = find_rust_binary();

    if let Some(ref swift) = swift_bin {
        println!("Swift binary: {}", swift.display());
    } else {
        println!("Swift binary: not found (will run Rust-only benchmarks)");
    }
    println!("Rust binary : {}", rust_bin.display());

    test_1_token_counting_stress(swift_bin.as_deref(), &rust_bin);
    test_2_generation_throughput(swift_bin.as_deref(), &rust_bin);
    test_3_server_concurrency_stress(swift_bin.as_deref(), &rust_bin).await;
}
