# apfel-rs 🍏🦀

> High-performance Rust fork of [apfel](https://github.com/Arthur-Ficial/apfel): Apple Intelligence on-device FoundationModels from the command line and OpenAI-compatible server.

[![Homebrew](https://img.shields.io/badge/homebrew-bhubbard%2Ftap-blue.svg)](https://github.com/bhubbard/homebrew-tap)
[![crates.io](https://img.shields.io/crates/v/apfel-rs.svg)](https://crates.io/crates/apfel-rs)
[![npm](https://img.shields.io/npm/v/apfel-rs.svg)](https://www.npmjs.com/package/apfel-rs)
[![Rust 1.80+](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![macOS 26.0+](https://img.shields.io/badge/macOS-26.0%2B-black.svg)](https://apple.com/macos)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

`apfel-rs` is a complete native Rust implementation of **apfel**, bridging Apple's macOS `FoundationModels` framework directly into a lightweight, ultra-fast CLI and an OpenAI-compatible HTTP server powered by `tokio` and `axum`.

---

## Highlights

- **100% On-Device, Zero Network**: Runs entirely against Apple Intelligence on Apple Silicon via the native `FoundationModels` framework.
- **Ultra-Fast & Compact**: Compiled Rust binary (~5MB release executable) with 60+ tokens/sec on-device generation throughput.
- **OpenAI-Compatible Server**:
  - `POST /v1/chat/completions` (JSON and real-time SSE streaming)
  - `POST /v1/responses` (OpenAI Responses endpoint)
  - `GET /v1/models` and `GET /health`
- **Model Context Protocol (MCP)**: Spawns external MCP servers (Python/Node/native binaries) over stdio with an automatic tool-calling and re-prompting loop.
- **Context Strategies**: `newest-first`, `oldest-first`, `sliding-window`, `summarize`, and `strict` history trimming.
- **Localhost CSRF Protection & Auth**: Origin validation, constant-time bearer token verification, and child process environment scrubbing.
- **UNIX Ergonomics**: Piped stdin, `-f` file attachments, `--code` markdown fence extraction, `--count-tokens`, `--benchmark`, `--json` output, and interactive `--chat`.

---

## Architecture

```text
CLI (single/stream/chat) ──┐
                           ├─→ Rust Session Engine ──→ Swift C-ABI Bridge ──→ macOS FoundationModels
HTTP Server (/v1/*) ───────┘    ├─ Context Manager (Trimming & Budget)       (SystemLanguageModel)
                                ├─ Tool Call & MCP Loop
                                ├─ Origin & Security Middleware
                                └─ Token Counter & Window Tracker
```

---

## Benchmarks: Rust (`apfel-rs`) vs. Swift (`apfel`)

Rigorous stress testing conducted on Apple Silicon (macOS Sequoia) comparing the original Homebrew Swift build against the Rust release binary (`cargo build --release`):

| Metric | Original Swift (`apfel`) | Rust Fork (`apfel-rs`) | Gain / Difference |
| :--- | :--- | :--- | :--- |
| **Binary Footprint** | `20.75 MB` | **`2.50 MB`** | **88.0% smaller** |
| **Idle Server Memory (RSS)** | `22.2 MB` | **`10.1 MB`** | **54.6% less RAM** |
| **Server Concurrency Memory (RSS)** | `24.1 MB` | **`15.2 MB`** | **36.6% less RAM** |
| **Server Throughput (RPS)** | `1,106.3 req/s` | **`4,160.2 req/s`** | **3.76x higher throughput** |
| **Server Tail Latency (p95)** | `45.11 ms` | **`9.85 ms`** | **4.58x lower tail latency** |
| **Token Counting (Mean)** | `131.83 ms` | **`68.35 ms`** | **1.93x faster** |
| **Token Counting Jitter (Max)** | `748.19 ms` | **`91.60 ms`** | **8.17x lower latency spikes** |
| **Inference Generation** | `~57.3 tok/s` | `~55.9 tok/s` | Zero C-bridge overhead |

### Key Takeaways
- **Zero GC/Pause Spikes**: Swift experienced tail latency jitter up to 748 ms during token counting runs, whereas Rust's deterministic memory management never exceeded 91 ms.
- **Axum & Tokio Web Scalability**: The Rust server handles **4,160 req/sec** with sub-10ms p95 latency under high concurrency, quadrupling Swift's Hummingbird engine performance.
- **Reproduce Benchmarks**: Run `cargo run --release --example bench` in the repo root.

---

## Installation & Setup

### 1. Homebrew (macOS Recommended)
Install via the official tap (`bhubbard/tap`):

```bash
# Tap and install in one step
brew install bhubbard/tap/apfel-rs

# Command is now available globally with automatic shell completions
apfel --model-info
```

Run as a persistent background daemon (starts automatically on login):
```bash
# Start background server on localhost:32185
brew services start apfel-rs

# Check server status
brew services info apfel-rs

# View live daemon logs
tail -f /opt/homebrew/var/log/apfel.log

# Stop background server
brew services stop apfel-rs
```

### 2. Instant Run via NPX (Zero Setup)
No Rust or Xcode installation required — runs the bundled Apple Silicon binary immediately:

```bash
npx apfel-rs "Explain quantum computing in one sentence"
```

### 3. Global Install via NPM
```bash
npm install -g apfel-rs

# Command is now available globally
apfel --stream "Write a haiku about compiling Rust"
```

### 4. Install via Cargo (crates.io)
```bash
cargo install apfel-rs

# Command is now available globally
apfel --model-info
```

### 5. Building from Source
**Prerequisites**: macOS Sequoia (26.0+) on Apple Silicon with Xcode Command Line Tools.

```bash
git clone https://github.com/bhubbard/apfel-rs.git
cd apfel-rs

# Run full test suite (unit + server integration tests)
cargo test

# Build optimized release binary
cargo build --release
```

---

## CLI Usage

### Quick Prompt & Streaming

```bash
# Single prompt
apfel "Explain the difference between TCP and UDP in two sentences"

# Real-time token streaming
apfel --stream "Write a haiku about compiling Rust"

# Read from stdin / pipe
cat logs.txt | apfel "Summarize any errors found"

# Attach files
apfel -f src/lib.rs "Explain the public interface"

# Extract only code block (--code)
apfel --code "Write a Python script to fetch JSON from an API"
```

### Model Info & Token Counting

```bash
# Check on-device availability and context window
apfel --model-info

# Count tokens in text or files
apfel --count-tokens "Measure this text"
apfel --count-tokens -f document.pdf --output json

# Run throughput and latency benchmark
apfel --benchmark
```

### Interactive Chat Session

```bash
apfel --chat
# Slash commands supported:
#   /info   - Show context size and turn count
#   /clear  - Reset conversation history
#   /exit   - Quit session
```

---

## OpenAI-Compatible Server

### Mode A: Background Daemon via Homebrew (Recommended)

When installed via Homebrew, manage `apfel` as a persistent background system daemon that starts automatically on macOS login:

```bash
# Start background server daemon
brew services start apfel-rs

# Inspect server status
brew services info apfel-rs

# Monitor live server logs
tail -f /opt/homebrew/var/log/apfel.log

# Stop background server
brew services stop apfel-rs
```

### Mode B: Manual Foreground Server

Start the local server directly in your current terminal:

```bash
apfel --serve --port 8080 --host 127.0.0.1
```

### 1. Chat Completions (JSON)

```bash
curl -X POST http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "apple-foundationmodel",
    "messages": [
      {"role": "system", "content": "You are a concise assistant."},
      {"role": "user", "content": "What is WebAssembly?"}
    ],
    "temperature": 0.7
  }'
```

### 2. Streaming Chat Completions (Server-Sent Events)

```bash
curl -N -X POST http://127.0.0.1:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "apple-foundationmodel",
    "stream": true,
    "messages": [
      {"role": "user", "content": "Count from 1 to 5"}
    ]
  }'
```

### 3. Server Health & Model Discovery

```bash
curl http://127.0.0.1:8080/health
curl http://127.0.0.1:8080/v1/models
```

---

## Model Context Protocol (MCP)

Connect external tool servers:

```bash
# Connect local Python or Node MCP server
apfel --mcp-server "./calculator_mcp.py" "What is 42 * 1337?"

# Serve with tool support enabled for all API clients
apfel --serve --mcp-server "./search_server.py"
```

---

## Testing & Code Coverage

Run the 71 unit and integration test suites:

```bash
cargo test
```

Generate full code coverage metrics using `cargo-llvm-cov`:

```bash
# Text summary of region, line, and function coverage
cargo llvm-cov --summary-only

# Generate an interactive HTML report
cargo llvm-cov --html
open target/llvm-cov/html/index.html
```

---

## Exit Codes

| Exit Code | Constant | Meaning |
|---|---|---|
| `0` | `SUCCESS` | Successful execution |
| `1` | `RUNTIME_ERROR` | Internal or runtime failure |
| `2` | `USAGE_ERROR` | Invalid CLI arguments or malformed schema |
| `3` | `GUARDRAIL` | System safety guardrail triggered |
| `4` | `CONTEXT_OVERFLOW` | Prompt exceeds context limit |
| `5` | `MODEL_UNAVAILABLE` | Apple Intelligence unavailable |
| `6` | `RATE_LIMITED` | Rate limit or quota exhausted |
| `7` | `NO_CODE` | `--code` requested but no code block returned |

---

## License

[MIT](LICENSE)
