#!/usr/bin/env python3
import subprocess
import time
import json
import statistics
import os
import signal
import sys

SWIFT_BIN = "/opt/homebrew/bin/apfel"
RUST_BIN = "/Users/bhubbard/PROJECTS/apfel-rs/target/release/apfel"

def run_command(cmd):
    start = time.perf_counter()
    p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    duration = time.perf_counter() - start
    return duration, p.stdout, p.returncode

def benchmark_cli(name, cmd_template, runs=5):
    print(f"\n--- Benchmarking: {name} ({runs} runs) ---")
    swift_times = []
    rust_times = []

    for i in range(runs):
        swift_cmd = [SWIFT_BIN] + cmd_template
        t, out, code = run_command(swift_cmd)
        if code == 0:
            swift_times.append(t * 1000.0)

    for i in range(runs):
        rust_cmd = [RUST_BIN] + cmd_template
        t, out, code = run_command(rust_cmd)
        if code == 0:
            rust_times.append(t * 1000.0)

    swift_mean = statistics.mean(swift_times) if swift_times else 0
    rust_mean = statistics.mean(rust_times) if rust_times else 0
    speedup = (swift_mean / rust_mean) if rust_mean > 0 else 0

    print(f"Original Swift : {swift_mean:.2f} ms (min: {min(swift_times):.2f}, max: {max(swift_times):.2f})")
    print(f"Rust (apfel-rs): {rust_mean:.2f} ms (min: {min(rust_times):.2f}, max: {max(rust_times):.2f})")
    print(f"Result         : Rust is {speedup:.2f}x {'faster' if speedup >= 1.0 else 'slower'}")
    return {
        "name": name,
        "swift_ms": swift_mean,
        "rust_ms": rust_mean,
        "speedup": speedup
    }

def main():
    print("==========================================================")
    print(" APFEL COMPARATIVE BENCHMARK: Swift vs Rust (apfel-rs)")
    print("==========================================================")

    # 1. Binary Sizes
    swift_size = os.path.getsize(SWIFT_BIN) / (1024 * 1024)
    rust_size = os.path.getsize(RUST_BIN) / (1024 * 1024)
    print(f"\n[1] Executable Binary Size:")
    print(f"  • Original Swift binary: {swift_size:.2f} MB")
    print(f"  • Rust binary (apfel-rs): {rust_size:.2f} MB ({(rust_size/swift_size)*100:.1f}% of Swift)")

    results = []

    # 2. CLI Startup & Model Info query
    results.append(benchmark_cli("CLI: --model-info", ["--model-info"], runs=5))

    # 3. Token Counting
    sample_text = "The quick brown fox jumps over the lazy dog. " * 20
    results.append(benchmark_cli("CLI: --count-tokens", ["--count-tokens", sample_text], runs=5))

    # 4. Inference & Generation
    print("\n--- Benchmarking Generation (Inference) ---")
    prompt = "Explain in exactly one sentence what a compiler does."
    
    swift_gen_times = []
    rust_gen_times = []
    
    for i in range(3):
        t, out, code = run_command([SWIFT_BIN, "--temperature", "0", prompt])
        if code == 0:
            swift_gen_times.append(t)
            swift_out = out.strip()

    for i in range(3):
        t, out, code = run_command([RUST_BIN, "--temperature", "0", prompt])
        if code == 0:
            rust_gen_times.append(t)
            rust_out = out.strip()

    swift_gen_avg = statistics.mean(swift_gen_times)
    rust_gen_avg = statistics.mean(rust_gen_times)
    gen_speedup = swift_gen_avg / rust_gen_avg

    print(f"Prompt: \"{prompt}\"")
    print(f"Swift average generation time : {swift_gen_avg:.3f} s")
    print(f"Rust average generation time  : {rust_gen_avg:.3f} s")
    print(f"Inference Speedup             : {gen_speedup:.2f}x")

    # 5. Server Startup, Memory Footprint & API Latency
    print("\n--- Benchmarking HTTP Server & Memory Footprint ---")
    swift_port = 8765
    rust_port = 8766

    p_swift = subprocess.Popen([SWIFT_BIN, "--serve", "--port", str(swift_port)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    p_rust = subprocess.Popen([RUST_BIN, "--serve", "--port", str(rust_port)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(1.5)

    def get_rss(pid):
        try:
            out = subprocess.check_output(["ps", "-o", "rss=", "-p", str(pid)]).decode().strip()
            return int(out) / 1024.0 # MB
        except Exception:
            return 0.0

    swift_mem = get_rss(p_swift.pid)
    rust_mem = get_rss(p_rust.pid)

    print(f"Server Idle Resident Memory (RSS):")
    print(f"  • Swift Hummingbird server RSS: {swift_mem:.1f} MB")
    print(f"  • Rust Axum/Tokio server RSS   : {rust_mem:.1f} MB ({(rust_mem/swift_mem)*100:.1f}%)")

    # Test /health latency over 20 requests
    import urllib.request
    
    def test_health_latency(port, runs=25):
        times = []
        for _ in range(runs):
            s = time.perf_counter()
            req = urllib.request.Request(f"http://127.0.0.1:{port}/health")
            with urllib.request.urlopen(req) as resp:
                resp.read()
            times.append((time.perf_counter() - s) * 1000.0)
        return statistics.mean(times)

    swift_health_lat = test_health_latency(swift_port)
    rust_health_lat = test_health_latency(rust_port)
    server_speedup = swift_health_lat / rust_health_lat

    print(f"\nServer /health Endpoint Latency (25 requests):")
    print(f"  • Swift server average latency : {swift_health_lat:.2f} ms")
    print(f"  • Rust server average latency  : {rust_health_lat:.2f} ms")
    print(f"  • Latency Speedup              : {server_speedup:.2f}x faster in Rust")

    # Clean up server processes
    p_swift.terminate()
    p_rust.terminate()
    p_swift.wait()
    p_rust.wait()

    print("\n==========================================================")
    print(" SUMMARY")
    print("==========================================================")
    for r in results:
        print(f"• {r['name']:<24}: Rust is {r['speedup']:.2f}x as fast (Rust: {r['rust_ms']:.1f}ms vs Swift: {r['swift_ms']:.1f}ms)")
    print(f"• Full Generation Inference: Rust is {gen_speedup:.2f}x as fast (Rust: {rust_gen_avg:.2f}s vs Swift: {swift_gen_avg:.2f}s)")
    print(f"• Server API Latency       : Rust is {server_speedup:.2f}x as fast (Rust: {rust_health_lat:.2f}ms vs Swift: {swift_health_lat:.2f}ms)")
    print(f"• Server Idle Memory (RSS) : Rust uses {rust_mem:.1f}MB vs Swift {swift_mem:.1f}MB ({((swift_mem - rust_mem)/swift_mem)*100:.1f}% less memory)")
    print(f"• Binary Executable Size   : Rust is {rust_size:.1f}MB vs Swift {swift_size:.1f}MB ({((swift_size - rust_size)/swift_size)*100:.1f}% smaller)")

if __name__ == "__main__":
    main()
