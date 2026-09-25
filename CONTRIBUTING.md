# Contributing to apfel-rs

Thank you for your interest in contributing to `apfel-rs`! We welcome contributions ranging from bug reports and documentation fixes to performance optimizations and new features.

---

## Code of Conduct

All contributors and maintainers are expected to adhere to our [Code of Conduct](CODE_OF_CONDUCT.md). Please read it to understand our community standards.

---

## Development Setup

`apfel-rs` is written in Rust. You will need:
- Rust toolchain (`stable`)
- `cargo`, `rustfmt`, and `clippy`

### Clone and Build

```bash
git clone https://github.com/bhubbard/apfel-rs.git
cd apfel-rs
cargo build
```

---

## Running Tests

Always verify that tests pass before opening a pull request:

```bash
cargo test
```

---

## Code Style & Standards

We enforce strict formatting and linting standards across the codebase:

1. **Formatting:**
   ```bash
   cargo fmt --check
   cargo fmt # Auto-format
   ```

2. **Clippy Linters:**
   We require clean clippy passes with zero warnings:
   ```bash
   cargo clippy -- -D warnings
   ```

3. **Conventional Commits:**
   Please format your commit messages following [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat(...)`: New feature or capability
   - `fix(...)`: Bug fix or security remediation
   - `perf(...)`: Performance optimization
   - `refactor(...)`: Code improvement without behavioral change
   - `test(...)`: Adding or updating test cases
   - `docs(...)`: Documentation updates

---

## Pull Request Process

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Ensure `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test` pass cleanly.
3. Submit a PR filling out the pull request template completely.
