// ============================================================================
// args.rs — Command-line argument definitions using clap
// Part of apfel-rs
// ============================================================================

use clap::Parser;

/// Parsed command-line arguments for the apfel CLI binary.
#[derive(Parser, Debug, Clone)]
#[command(
    name = "apfel",
    author = "Brandon Hubbard <bhubbard@users.noreply.github.com>",
    version = env!("CARGO_PKG_VERSION"),
    about = "Apple Intelligence on-device model from the command line and OpenAI-compatible server",
    after_help = "Examples:\n  apfel \"Explain quantum computing\"\n  apfel --stream \"Write a poem\"\n  apfel --chat\n  apfel --serve --port 8080\n  apfel --model-info\n  apfel --count-tokens \"Sample text\""
)]
pub struct CliArgs {
    /// The prompt text to generate from
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,

    /// System instructions for the model
    #[arg(
        short = 's',
        long = "system",
        value_name = "INSTRUCTIONS",
        env = "APFEL_SYSTEM_PROMPT"
    )]
    pub system: Option<String>,

    /// Attach text or file content to prompt (repeatable)
    #[arg(short = 'f', long = "file", value_name = "PATH")]
    pub file: Vec<String>,

    /// Stream response tokens to stdout (default when TTY)
    #[arg(long = "stream")]
    pub stream: bool,

    /// Disable streaming; wait for full response
    #[arg(long = "no-stream")]
    pub no_stream: bool,

    /// Start interactive chat session
    #[arg(long = "chat")]
    pub chat: bool,

    /// Start local OpenAI-compatible HTTP server
    #[arg(long = "serve")]
    pub serve: bool,

    /// Run batch processing reading JSONL from stdin and writing JSONL to stdout
    #[arg(long = "batch")]
    pub batch: bool,

    /// Port for the HTTP server
    #[arg(long = "port", default_value = "8080")]
    pub port: u16,

    /// Host interface to bind to
    #[arg(long = "host", default_value = "127.0.0.1")]
    pub host: String,

    /// Bearer token for server authentication
    #[arg(long = "token", env = "APFEL_TOKEN")]
    pub token: Option<String>,

    /// Allowed origins for CSRF protection (comma-separated or repeatable)
    #[arg(long = "allowed-origins", value_delimiter = ',')]
    pub allowed_origins: Vec<String>,

    /// Allow binding to non-localhost without authentication
    #[arg(long = "footgun")]
    pub footgun: bool,

    /// Backend engine to use: foundation (default on macOS), mlx, or mock
    #[arg(long = "engine", value_name = "ENGINE", env = "APFEL_ENGINE")]
    pub engine: Option<String>,

    /// Specific model name or HuggingFace repo (used with mlx engine)
    #[arg(short = 'm', long = "model", value_name = "MODEL", env = "APFEL_MODEL")]
    pub model: Option<String>,

    /// Fine-tuned model adapter path (.fmadapter or LoRA weights)
    #[arg(long = "adapter", value_name = "PATH", env = "APFEL_ADAPTER")]
    pub adapter: Option<String>,

    /// Show model information, availability, and context size
    #[arg(long = "model-info")]
    pub model_info: bool,

    /// Count tokens in prompt, stdin, or attached files
    #[arg(long = "count-tokens")]
    pub count_tokens: bool,

    /// Exit with code 4 if token count exceeds context budget (used with --count-tokens)
    #[arg(long = "strict")]
    pub strict: bool,

    /// Path to a JSON Schema file or raw JSON schema string for structured output enforcement
    #[arg(long = "schema", value_name = "SCHEMA")]
    pub schema: Option<String>,

    /// JSON conversation file path or '-' for stdin
    #[arg(long = "messages", value_name = "FILE")]
    pub messages: Option<String>,

    /// Extract only code blocks, stripping markdown fences and prose (exits 7 if no code found)
    #[arg(long = "code")]
    pub code: bool,

    /// Require complete generation; exit with code 8 if response is truncated by token budget or length
    #[arg(long = "require-complete")]
    pub require_complete: bool,

    /// Stop sequences to truncate generation (repeatable)
    #[arg(long = "stop", value_name = "SEQUENCE")]
    pub stop: Vec<String>,

    /// Sampling temperature
    #[arg(short = 't', long = "temperature", env = "APFEL_TEMPERATURE")]
    pub temperature: Option<f64>,

    /// Nucleus sampling probability threshold
    #[arg(long = "top-p", env = "APFEL_TOP_P")]
    pub top_p: Option<f64>,

    /// Maximum response tokens to generate
    #[arg(long = "max-tokens", env = "APFEL_MAX_TOKENS")]
    pub max_tokens: Option<usize>,

    /// Deterministic RNG seed
    #[arg(long = "seed", env = "APFEL_SEED")]
    pub seed: Option<u64>,

    /// Relax safety guardrails for permissive content transformations
    #[arg(long = "permissive")]
    pub permissive: bool,

    /// Output format: text, json, or raw
    #[arg(
        short = 'o',
        long = "output",
        default_value = "text",
        env = "APFEL_OUTPUT"
    )]
    pub output: String,

    /// Suppress non-essential output
    #[arg(short = 'q', long = "quiet")]
    pub quiet: bool,

    /// Disable ANSI color styling
    #[arg(long = "no-color")]
    pub no_color: bool,

    /// Enable verbose debug logging
    #[arg(short = 'd', long = "debug")]
    pub debug: bool,

    /// Context strategy: newest-first, oldest-first, sliding-window, summarize, strict
    #[arg(
        long = "context-strategy",
        default_value = "newest-first",
        env = "APFEL_CONTEXT_STRATEGY"
    )]
    pub context_strategy: String,

    /// Maximum turns for sliding-window context strategy
    #[arg(long = "max-turns")]
    pub max_turns: Option<usize>,

    /// Connect to external MCP server (command path or script, repeatable)
    #[arg(long = "mcp-server", value_name = "COMMAND")]
    pub mcp_servers: Vec<String>,

    /// Path to Model Context Protocol (MCP) configuration JSON file
    #[arg(long = "mcp-config", value_name = "PATH")]
    pub mcp_config: Option<String>,

    /// Run local latency and throughput benchmark
    #[arg(long = "benchmark")]
    pub benchmark: bool,

    /// Append structured execution telemetry JSONL to a file path
    #[arg(long = "telemetry", value_name = "PATH")]
    pub telemetry: Option<String>,

    /// Generate shell completion script (bash, zsh, fish, powershell)
    #[arg(long = "completions", value_name = "SHELL")]
    pub completions: Option<String>,

    /// Explicit CLI flag tokens passed by the user (used to disambiguate env var fallbacks)
    #[arg(skip)]
    pub explicit_cli_args: Option<Vec<String>>,
}

impl CliArgs {
    /// Returns true if a given flag was explicitly passed on the command line (rather than via environment fallback).
    pub fn was_flag_explicit(&self, short: &str, long: &str) -> bool {
        if let Some(explicit) = &self.explicit_cli_args {
            return explicit.iter().any(|arg| {
                (!short.is_empty() && arg == short)
                    || (!long.is_empty() && (arg == long || arg.starts_with(&format!("{}=", long))))
            });
        }
        for arg in std::env::args_os() {
            if let Some(s) = arg.to_str() {
                if (!short.is_empty() && s == short)
                    || (!long.is_empty() && (s == long || s.starts_with(&format!("{}=", long))))
                {
                    return true;
                }
            }
        }
        false
    }
}
