// ============================================================================
// args.rs — Command-line argument definitions using clap
// Part of apfel-rs
// ============================================================================

use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "apfel",
    author = "Brandon Hubbard <bhubbard@users.noreply.github.com>",
    version = "0.1.0",
    about = "Apple Intelligence on-device model from the command line and OpenAI-compatible server",
    after_help = "Examples:\n  apfel \"Explain quantum computing\"\n  apfel --stream \"Write a poem\"\n  apfel --chat\n  apfel --serve --port 8080\n  apfel --model-info\n  apfel --count-tokens \"Sample text\""
)]
pub struct CliArgs {
    /// The prompt text to generate from
    #[arg(value_name = "PROMPT")]
    pub prompt: Option<String>,

    /// System instructions for the model
    #[arg(short = 's', long = "system", value_name = "INSTRUCTIONS")]
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

    /// Show model information, availability, and context size
    #[arg(long = "model-info")]
    pub model_info: bool,

    /// Count tokens in prompt, stdin, or attached files
    #[arg(long = "count-tokens")]
    pub count_tokens: bool,

    /// Exit with code 4 if token count exceeds context budget (used with --count-tokens)
    #[arg(long = "strict")]
    pub strict: bool,

    /// Extract and print only the first markdown code block
    #[arg(long = "code")]
    pub code_only: bool,

    /// JSON schema file path for structured output
    #[arg(long = "schema", value_name = "FILE")]
    pub schema: Option<String>,

    /// JSON conversation file path or '-' for stdin
    #[arg(long = "messages", value_name = "FILE")]
    pub messages: Option<String>,

    /// Sampling temperature
    #[arg(short = 't', long = "temperature")]
    pub temperature: Option<f64>,

    /// Nucleus sampling probability threshold
    #[arg(long = "top-p")]
    pub top_p: Option<f64>,

    /// Maximum response tokens to generate
    #[arg(long = "max-tokens")]
    pub max_tokens: Option<usize>,

    /// Deterministic RNG seed
    #[arg(long = "seed")]
    pub seed: Option<u64>,

    /// Relax safety guardrails for permissive content transformations
    #[arg(long = "permissive")]
    pub permissive: bool,

    /// Output format: text, json, or raw
    #[arg(short = 'o', long = "output", default_value = "text")]
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
    #[arg(long = "context-strategy", default_value = "newest-first")]
    pub context_strategy: String,

    /// Maximum turns for sliding-window context strategy
    #[arg(long = "max-turns")]
    pub max_turns: Option<usize>,

    /// Connect to external MCP server (command path or script, repeatable)
    #[arg(long = "mcp-server", value_name = "COMMAND")]
    pub mcp_servers: Vec<String>,

    /// Run local latency and throughput benchmark
    #[arg(long = "benchmark")]
    pub benchmark: bool,

    /// Generate shell completion script (bash, zsh, fish, powershell)
    #[arg(long = "completions", value_name = "SHELL")]
    pub completions: Option<String>,
}
