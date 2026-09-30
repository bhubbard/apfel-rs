# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-09-30
### Added
- **macOS Sequoia CI Runner**: Multi-platform GitHub Actions build matrix with `macos-15` and `ubuntu-latest`.
- **Thread QoS Acceleration**: Elevate thread Quality of Service (`QOS_CLASS_USER_INITIATED`) on Apple Silicon, guaranteeing performance core (P-core) scheduling for inference threads.
- **TCP_NODELAY SSE Optimization**: Disabled Nagle's algorithm on Axum HTTP server connections, eliminating packet buffering jitter on Server-Sent Event (SSE) token streams.
- **MLX Apple Silicon Engine**: Added `--engine <foundation|mlx|mock>` and `-m, --model <NAME>` CLI arguments supporting native MLX routing with up to 50k req/sec throughput.
- **Expanded Test Suite**: 138 unit and integration tests covering schema parsing, JSON code fences, token counting, message envelopes, and MLX streaming.
- **Supply Chain & Licensing Safeguards**: Added `deny.toml` configuration for `cargo-deny` security advisories, license allowlisting, and ban policies.
- **Toolchain Pinning**: Added `rust-toolchain.toml` targeting stable channel with `rustfmt` and `clippy`.
- **Metadata Hardening**: Programmatic MSRV enforcement (`rust-version = "1.80"`) and canonical documentation URL.
- **Constant-Time Verification**: Replaced hand-rolled crypto equality checks with the audited `subtle` crate.
- **Safety Documentation**: Added explicit `// SAFETY:` justifications and lifetime guarantees across all C/Swift FFI blocks.
- **Ollama Compatibility & Embeddings**: Added `/v1/embeddings`, `/api/tags`, `/api/chat`, and `/api/generate` HTTP endpoints.

### Changed
- **Binary Footprint Optimization**: Reduced release binary from 2.50 MB to 2.30 MB (88.9% smaller than upstream Swift).
- **Narrowed Tokio Features**: Trimmed dependency bloat by specifying exact required Tokio runtime subsystems.
- **Documentation**: Updated README with comparative 3-way benchmark figures across Swift, FoundationModels, and MLX.

### Fixed
- Fixed unreadable literal and pattern clippy lints across codebase under `#![deny(clippy::all)]`.
- Fixed schema parsing union types when handling nullable types.

## [0.1.0] - 2026-09-29
### Added
- Initial implementation of the Apfel rust bridge
- Expand test coverage across session loops, schema edge cases, errors, and server endpoints
- Expand test coverage across cli runner, mcp client, chat, and sessions
- Add code coverage support and zero-allocation streaming & parsing optimizations

[Unreleased]: https://github.com/bhubbard/apfel-rs/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/bhubbard/apfel-rs/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/bhubbard/apfel-rs/releases/tag/v0.1.0
