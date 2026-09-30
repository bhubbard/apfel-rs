# apfel-rs Feature & Enhancement Backlog (TODO)

This document tracks upcoming features, upstream parity items, protocol additions, and performance enhancements for **`apfel-rs`**.

---

## 🚀 High-Priority Backlog

### 1. High-Performance JSONL `--batch` Mode
- **Upstream Reference**: [Arthur-Ficial/apfel #481](https://github.com/Arthur-Ficial/apfel/issues/481)
- **Status**: Planned (Phase 2)
- **Scope**:
  - Read incremental UTF-8 JSONL records from `stdin` (`jq | apfel --batch | jq`).
  - Input record format: `{ "custom_id": "...", "prompt": "..." }` or `{ "custom_id": "...", "messages": [...] }`.
  - Process each record in a fresh, isolated session with bounded read-ahead (avoiding memory accumulation).
  - Emit one JSON line per record immediately:
    ```json
    {"line": 1, "custom_id": "ticket-101", "status": "ok", "content": "billing", "finish_reason": "stop"}
    ```
  - Yield exit code `0` on full success, exit code `1` if any record-level generation failed.
  - Zero process spawning overhead over thousands of records.

---

### 2. OpenAI SDK Token Preflight: `POST /v1/responses/input_tokens`
- **Upstream Reference**: [Arthur-Ficial/apfel #485](https://github.com/Arthur-Ficial/apfel/issues/485)
- **Status**: Planned (Phase 3)
- **Scope**:
  - Add `/v1/responses/input_tokens` route to the Axum HTTP server.
  - Decode standard OpenAI request payloads (`model`, `input` text or messages, `instructions`, tools/functions).
  - Return authoritative token preflight:
    ```json
    {
      "object": "response.input_tokens",
      "input_tokens": 142
    }
    ```
  - Zero generation and zero MCP tool invocation. Allows clients using `client.responses.input_tokens.count(...)` to budget context accurately.

---

### 3. Dynamic Context Window Measurement Disclosure
- **Upstream Reference**: [Arthur-Ficial/apfel #491](https://github.com/Arthur-Ficial/apfel/issues/491)
- **Status**: Planned (Phase 3)
- **Scope**:
  - Add `context_window_measured: bool` to `/health` and `/v1/models` wire payloads.
  - In `--model-info`, explicitly annotate whether the context size (e.g. 4096 vs 8192) was measured live from Apple Intelligence or is a fallback floor during cold-start:
    ```text
    Context Window : 4,096 tokens (assumed - model cold start)
    # or
    Context Window : 8,192 tokens (measured)
    ```

---

### 4. Graceful Shell Environment Variable Fallbacks for `--serve`
- **Upstream Reference**: [Arthur-Ficial/apfel #496](https://github.com/Arthur-Ficial/apfel/issues/496)
- **Status**: Planned (Phase 3)
- **Scope**:
  - Track argument provenance (explicit CLI flag vs. `APFEL_*` environment variable).
  - If standing prompt variables like `APFEL_TEMPERATURE` or `APFEL_SYSTEM_PROMPT` are exported in the user's shell, do not reject `apfel --serve`, `--benchmark`, or `--model-info` with exit code 2.
  - Emit an informational warning and start cleanly. Hard CLI flag conflicts retain exit code 2.

---

### 5. Multi-Turn Tool Call Exchange Preservation
- **Upstream Reference**: [Arthur-Ficial/apfel #482](https://github.com/Arthur-Ficial/apfel/issues/482)
- **Status**: Backlog
- **Scope**:
  - When applying context trimming strategies (`sliding-window`, `newest-first`), ensure an assistant's tool-call message and its corresponding tool result are treated as an atomic unit.
  - Prevent splitting a tool request from its output, which causes OpenAI API validation errors.

---

### 6. Local JSON Schema References (`$defs` / `$ref`)
- **Upstream Reference**: [Arthur-Ficial/apfel #479](https://github.com/Arthur-Ficial/apfel/issues/479)
- **Status**: Backlog
- **Scope**:
  - Resolve in-document `#/$defs/Type` references when generating schema instructions and validating structured JSON outputs.

---

### 7. Fine-Tuned Model Adapters (`--adapter <path>`)
- **Upstream Reference**: [Arthur-Ficial/apfel #362](https://github.com/Arthur-Ficial/apfel/issues/362)
- **Status**: Research / Experimental
- **Scope**:
  - Add `--adapter <path>` to load `.fmadapter` packages for FoundationModels or LoRA adapters for the MLX engine.
  - Expose loaded adapters under `/v1/models` with unique model IDs for dynamic routing.

---

## 🛠️ In-Flight Work
- [x] Swift bridge latency optimization (DispatchSemaphore + QoS)
- [x] Multi-engine benchmark documentation and GitHub Pages update
- [ ] `--code` code-block extraction flag with exit code 7 (Issue #373)
- [ ] `--require-complete` completion requirement with exit code 8 (Issue #484)
- [ ] Repeatable `--stop` CLI flag with stream-level matching (Issue #483)
