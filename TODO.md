# apfel-rs Feature & Enhancement Backlog (TODO)

This document tracks upcoming features, upstream parity items, protocol additions, and performance enhancements for **`apfel-rs`**.

---

## 🚀 Completed Upstream Parity & UNIX Enhancements

### 1. High-Performance JSONL `--batch` Mode
- **Upstream Reference**: [Arthur-Ficial/apfel #481](https://github.com/Arthur-Ficial/apfel/issues/481)
- **Status**: Completed
- **Scope & Implementation**:
  - Incremental UTF-8 JSONL stream processing from `stdin` (`jq | apfel --batch | jq`).
  - Supports input formats `{ "custom_id": "...", "prompt": "..." }` and `{ "custom_id": "...", "messages": [...] }`.
  - Fresh session isolation per line with zero process-spawn overhead.
  - Emits real-time JSONL results with line numbers, `custom_id`, `status: "ok" | "error"`, and `content`.
  - Exits code `0` on total success, code `1` if any record encounters a generation error.

---

### 2. OpenAI SDK Token Preflight: `POST /v1/responses/input_tokens`
- **Upstream Reference**: [Arthur-Ficial/apfel #485](https://github.com/Arthur-Ficial/apfel/issues/485)
- **Status**: Completed
- **Scope & Implementation**:
  - Registered `POST /v1/responses/input_tokens` endpoint.
  - Decodes standard OpenAI request payloads (`model`, `input` as string or message array, `instructions`, `tools`).
  - Returns `{ "object": "response.input_tokens", "input_tokens": N }`.
  - Zero text generation and zero tool execution overhead.

---

### 3. Dynamic Context Window Measurement Disclosure
- **Upstream Reference**: [Arthur-Ficial/apfel #491](https://github.com/Arthur-Ficial/apfel/issues/491)
- **Status**: Completed
- **Scope & Implementation**:
  - Added `context_window_measured: bool` to `/health` and `/v1/models` wire payloads.
  - In `--model-info`, clearly discloses whether the window is measured live or assumed:
    ```text
    Context Window : 4,096 tokens (assumed - model cold start)
    # or
    Context Window : 8,192 tokens (measured)
    ```

---

### 4. Graceful Shell Environment Variable Fallbacks for `--serve`
- **Upstream Reference**: [Arthur-Ficial/apfel #496](https://github.com/Arthur-Ficial/apfel/issues/496)
- **Status**: Completed
- **Scope & Implementation**:
  - Tracks argument provenance (explicit CLI flag vs. `APFEL_*` environment variable).
  - Standing shell exports (`APFEL_TEMPERATURE`, `APFEL_SYSTEM_PROMPT`, etc.) no longer fail `--serve`, `--model-info`, or `--benchmark`.
  - Explicit command-line flag conflicts retain exit code `2` (`USAGE_ERROR`).

---

### 5. Multi-Turn Tool Call Exchange Preservation
- **Upstream Reference**: [Arthur-Ficial/apfel #482](https://github.com/Arthur-Ficial/apfel/issues/482)
- **Status**: Completed
- **Scope & Implementation**:
  - `ContextManager::group_conversation` groups assistant `tool_calls` messages and corresponding `role: "tool"` responses into atomic units.
  - Prevents splitting a tool request from its response during sliding-window, newest-first, and oldest-first truncation.
  - Drops orphaned tool messages so OpenAI client schema validation never fails.

---

### 6. Local JSON Schema References (`$defs` / `#/definitions`)
- **Upstream Reference**: [Arthur-Ficial/apfel #479](https://github.com/Arthur-Ficial/apfel/issues/479)
- **Status**: Completed
- **Scope & Implementation**:
  - In `SchemaParser`, resolves RFC 6901 JSON pointer references (`#/$defs/<Type>` and `#/definitions/<Type>`).
  - Supports chained references, preserves description overrides and nullability.
  - Enforces `MAX_SCHEMA_DEPTH = 64` to prevent infinite loops from circular schema references.

---

### 7. Fine-Tuned Model Adapters (`--adapter <path>`)
- **Upstream Reference**: [Arthur-Ficial/apfel #362](https://github.com/Arthur-Ficial/apfel/issues/362)
- **Status**: Completed
- **Scope & Implementation**:
  - Added `--adapter <path>` flag and `APFEL_ADAPTER` environment variable.
  - Validates adapter path existence on disk (exit 2 if missing).
  - Dynamically registers the adapter model in `/v1/models` (`<base_model>:adapter-<name>`) with `owned_by: "user-adapter"`.
  - Annotated in `--model-info` output.

---

## 🛠️ Summary of Completed Core Features
- [x] Swift bridge latency optimization (DispatchSemaphore + QoS)
- [x] Multi-engine benchmark documentation and GitHub Pages update
- [x] `--code` code-block extraction flag with exit code 7 (Issue #373)
- [x] `--require-complete` completion requirement with exit code 8 (Issue #484)
- [x] Repeatable `--stop` CLI flag with stream-level matching (Issue #483)
- [x] High-performance JSONL `--batch` streaming with session isolation (Issue #481)
- [x] OpenAI SDK token preflight endpoint `POST /v1/responses/input_tokens` (Issue #485)
- [x] `context_window_measured` indicator in `/health`, `/v1/models`, and `--model-info` (Issue #491)
- [x] Graceful shell environment variable fallback handling for `--serve` (Issue #496)
- [x] Multi-turn tool call atomic preservation during context trimming (Issue #482)
- [x] Local JSON Schema `$defs` and `#/definitions` reference resolution (Issue #479)
- [x] Fine-tuned model adapters (`--adapter <path>`) CLI flag and routing (Issue #362)
- [x] Token counting caching with 152x speedup and zero-thread synchronous FFI execution
- [x] Homebrew Formula (`Formula/apfel.rb`) with completions and test block
- [x] GitHub Actions automated release pipeline (`.github/workflows/release.yml`) for Apple Silicon binaries
- [x] In-memory route pre-serialization (`cached_health`, `cached_models` in `AppState`)
- [x] CLI structured JSON schema enforcement (`--schema <path|json>`) with auto-repair retries
- [x] CLI Model Context Protocol integration (`--mcp-config <path>`) with live tool calling in CLI and interactive chat
- [x] Ollama streaming NDJSON, `/api/version`, `/api/show`, and `/api/ps` protocol completeness
- [x] Multimodal Vision & Document Ingestion (`--image <path>`, `--file <path>`, and API `image_url` parts)
- [x] Shell completions auto-installer (`--install-completions`) for Zsh, Bash, and Fish
- [x] Bump and cut `v0.1.4` release with automated GitHub Actions pipeline
