// ============================================================================
// runner.rs — CLI command execution and dispatch
// Part of apfel-rs
// ============================================================================

use crate::backend::default_engine;
use crate::backend::engine::{BackendEngine, GenerateRequest, StreamChunk};
use crate::cli::args::CliArgs;
use crate::cli::chat::run_chat_loop;
use crate::core::code_cropper::CodeCropper;
use crate::core::context::ContextStrategy;
use crate::core::error::ApfelExitCodes;
use crate::core::schema::SchemaParser;
use crate::mcp::client::MCPManager;
use crate::server::run_server;
use colored::Colorize;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

pub async fn run_cli(args: CliArgs) -> i32 {
    let engine = default_engine();
    run_cli_with_engine(args, engine).await
}

pub async fn run_cli_with_engine(args: CliArgs, engine: Arc<dyn BackendEngine>) -> i32 {
    if args.no_color {
        colored::control::set_override(false);
    }

    // 0. Completions Generator
    if let Some(shell_name) = &args.completions {
        return run_completions(shell_name);
    }

    // 1. Model Info Mode
    if args.model_info {
        return run_model_info(engine.as_ref());
    }

    // 2. Count Tokens Mode
    if args.count_tokens {
        return run_count_tokens(&args, engine.as_ref());
    }

    // 3. Benchmark Mode
    if args.benchmark {
        return run_benchmark(engine.as_ref()).await;
    }

    // 4. Server Mode
    if args.serve {
        return run_server_mode(args, engine).await;
    }

    // 5. Chat Mode
    if args.chat {
        let strategy = ContextStrategy::from_str(&args.context_strategy).unwrap_or_default();
        match run_chat_loop(engine, args.system, args.permissive, strategy).await {
            Ok(_) => return ApfelExitCodes::SUCCESS,
            Err(e) => {
                eprintln!("{}: {}", "Error".red().bold(), e);
                return e.exit_code();
            }
        }
    }

    // 6. Generation (Single or Stream) Mode
    run_generation(args, engine).await
}

fn run_completions(shell_name: &str) -> i32 {
    use clap::CommandFactory;
    use clap_complete::Shell;

    let shell = match shell_name.to_lowercase().as_str() {
        "bash" => Shell::Bash,
        "zsh" => Shell::Zsh,
        "fish" => Shell::Fish,
        "powershell" => Shell::PowerShell,
        _ => {
            eprintln!("Unsupported shell '{}'. Supported: bash, zsh, fish, powershell", shell_name);
            return ApfelExitCodes::USAGE_ERROR;
        }
    };

    let mut cmd = CliArgs::command();
    clap_complete::generate(shell, &mut cmd, "apfel", &mut io::stdout());
    ApfelExitCodes::SUCCESS
}

fn run_model_info(engine: &dyn BackendEngine) -> i32 {
    let available = if engine.is_available() { "yes" } else { "no" };
    let languages = engine.supported_languages().join(", ");
    let context = engine.context_size();

    println!("apfel v0.1.0 — model info");
    println!("├ model:      apple-foundationmodel");
    println!("├ on-device:  true (always)");
    println!("├ available:  {}", available);
    println!("├ context:    {} tokens", context);
    println!("├ languages:  {}", languages);
    println!("└ framework:  FoundationModels (macOS 26+)");

    ApfelExitCodes::SUCCESS
}

fn run_count_tokens(args: &CliArgs, engine: &dyn BackendEngine) -> i32 {
    let mut text_to_count = String::new();

    if let Some(prompt) = &args.prompt {
        text_to_count.push_str(prompt);
    }

    // Read attached files
    for file_path in &args.file {
        match fs::read_to_string(file_path) {
            Ok(content) => {
                if !text_to_count.is_empty() {
                    text_to_count.push('\n');
                }
                text_to_count.push_str(&content);
            }
            Err(e) => {
                eprintln!("Error reading file '{}': {}", file_path, e);
                return ApfelExitCodes::USAGE_ERROR;
            }
        }
    }

    // Read stdin if pipe and prompt is empty
    if text_to_count.is_empty() && !io::stdin().is_terminal() {
        let mut stdin_buf = String::new();
        if let Ok(_) = io::stdin().read_to_string(&mut stdin_buf) {
            text_to_count = stdin_buf;
        }
    }

    let tokens = engine.count_tokens(&text_to_count);
    let budget = engine.context_size();

    if args.output == "json" {
        let json = serde_json::json!({
            "tokens": tokens,
            "context_size": budget,
            "within_budget": tokens <= budget,
            "approximate": false
        });
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    } else {
        println!("{} tokens (context limit: {})", tokens, budget);
    }

    if args.strict && tokens > budget {
        return ApfelExitCodes::CONTEXT_OVERFLOW;
    }

    ApfelExitCodes::SUCCESS
}

async fn run_benchmark(engine: &dyn BackendEngine) -> i32 {
    println!("{}", "Running apfel on-device benchmark...".bold().cyan());
    let prompt = "Explain the difference between compiled and interpreted programming languages in three paragraphs.";

    let req = GenerateRequest {
        prompt: prompt.to_string(),
        system_prompt: None,
        messages: None,
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(512),
        permissive: false,
        seed: None,
    };

    let start = Instant::now();
    let mut rx = match engine.stream_generate(&req) {
        Ok(rx) => rx,
        Err(e) => {
            eprintln!("Benchmark failed: {}", e);
            return e.exit_code();
        }
    };

    let mut first_token_time = None;
    let mut total_chars = 0;
    let mut tokens_generated = 0;

    while let Some(chunk) = rx.recv().await {
        match chunk {
            StreamChunk::Delta(delta) => {
                if first_token_time.is_none() {
                    first_token_time = Some(start.elapsed());
                }
                total_chars += delta.len();
                tokens_generated += engine.count_tokens(&delta);
            }
            StreamChunk::Done { .. } => break,
            StreamChunk::Error(e) => {
                eprintln!("Benchmark error during generation: {}", e);
                return e.exit_code();
            }
        }
    }

    let total_time = start.elapsed();
    let ttft = first_token_time.unwrap_or(total_time);
    let gen_time = total_time.saturating_sub(ttft);
    let tokens_per_sec = if gen_time.as_secs_f64() > 0.0 {
        (tokens_generated as f64) / gen_time.as_secs_f64()
    } else {
        0.0
    };

    println!("\nBenchmark Results:");
    println!("├ Time to first token: {:.2?}", ttft);
    println!("├ Total generation time: {:.2?}", total_time);
    println!("├ Output tokens generated: ~{}", tokens_generated);
    println!("├ Throughput: {:.1} tokens/sec", tokens_per_sec);
    println!("└ Output characters: {}", total_chars);

    ApfelExitCodes::SUCCESS
}

async fn run_server_mode(args: CliArgs, engine: Arc<dyn BackendEngine>) -> i32 {
    let allowed_origins = if args.allowed_origins.is_empty() {
        vec![
            "http://127.0.0.1".to_string(),
            "http://localhost".to_string(),
            "http://[::1]".to_string(),
        ]
    } else {
        args.allowed_origins
    };

    let mcp_manager = if !args.mcp_servers.is_empty() {
        match MCPManager::load_servers(&args.mcp_servers, 10).await {
            Ok(m) => Some(Arc::new(m)),
            Err(e) => {
                eprintln!("Failed to initialize MCP servers: {}", e);
                return ApfelExitCodes::RUNTIME_ERROR;
            }
        }
    } else {
        None
    };

    if let Err(e) = run_server(
        &args.host,
        args.port,
        args.token,
        allowed_origins,
        args.footgun,
        engine,
        mcp_manager,
    )
    .await
    {
        eprintln!("Server error: {}", e);
        return ApfelExitCodes::RUNTIME_ERROR;
    }

    ApfelExitCodes::SUCCESS
}

async fn run_generation(args: CliArgs, engine: Arc<dyn BackendEngine>) -> i32 {
    let mut prompt_text = String::new();

    if let Some(p) = &args.prompt {
        prompt_text.push_str(p);
    }

    // Attach file contents
    for file_path in &args.file {
        match fs::read_to_string(file_path) {
            Ok(content) => {
                if !prompt_text.is_empty() {
                    prompt_text.push('\n');
                }
                prompt_text.push_str(&format!("--- File: {} ---\n{}\n--- End File ---", file_path, content));
            }
            Err(e) => {
                eprintln!("Error reading file '{}': {}", file_path, e);
                return ApfelExitCodes::USAGE_ERROR;
            }
        }
    }

    // Read stdin if pipe
    if !io::stdin().is_terminal() {
        let mut stdin_buf = String::new();
        if let Ok(_) = io::stdin().read_to_string(&mut stdin_buf) {
            let stdin_trimmed = stdin_buf.trim();
            if !stdin_trimmed.is_empty() {
                if prompt_text.is_empty() {
                    prompt_text = stdin_trimmed.to_string();
                } else {
                    prompt_text = format!("{}\n\n{}", stdin_trimmed, prompt_text);
                }
            }
        }
    }

    // Read messages file or stdin if specified
    let messages = if let Some(msg_path) = &args.messages {
        let content = if msg_path == "-" {
            let mut buf = String::new();
            if let Err(e) = io::stdin().read_to_string(&mut buf) {
                eprintln!("Error reading messages from stdin: {}", e);
                return ApfelExitCodes::USAGE_ERROR;
            }
            buf
        } else {
            match fs::read_to_string(msg_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error reading messages file '{}': {}", msg_path, e);
                    return ApfelExitCodes::USAGE_ERROR;
                }
            }
        };
        match crate::core::messages_input::MessagesInput::decode(&content) {
            Ok(m) => Some(m),
            Err(e) => {
                eprintln!("Messages error: {}", e);
                return ApfelExitCodes::USAGE_ERROR;
            }
        }
    } else {
        None
    };

    if prompt_text.trim().is_empty() && messages.is_none() {
        eprintln!("No prompt provided. Run 'apfel --help' for usage.");
        return ApfelExitCodes::USAGE_ERROR;
    }

    // Validate schema if provided
    let mut system_instructions = args.system.clone().unwrap_or_default();
    if let Some(schema_path) = &args.schema {
        match fs::read_to_string(schema_path) {
            Ok(schema_content) => {
                if let Err(e) = SchemaParser::parse(&schema_content, "OutputSchema") {
                    eprintln!("Schema validation error: {}", e);
                    return ApfelExitCodes::USAGE_ERROR;
                }
                let schema_prompt = format!(
                    "\n\nYou must respond ONLY with valid JSON conforming to this schema:\n{}",
                    schema_content
                );
                system_instructions.push_str(&schema_prompt);
            }
            Err(e) => {
                eprintln!("Error reading schema file '{}': {}", schema_path, e);
                return ApfelExitCodes::USAGE_ERROR;
            }
        }
    }

    // Setup MCP servers if provided
    let mcp_manager = if !args.mcp_servers.is_empty() {
        match MCPManager::load_servers(&args.mcp_servers, 10).await {
            Ok(m) => Some(Arc::new(m)),
            Err(e) => {
                eprintln!("Failed to initialize MCP servers: {}", e);
                return ApfelExitCodes::RUNTIME_ERROR;
            }
        }
    } else {
        None
    };

    let should_stream = (args.stream || io::stdout().is_terminal()) && !args.no_stream && !args.code_only;

    let req = GenerateRequest {
        prompt: prompt_text,
        system_prompt: if system_instructions.is_empty() { None } else { Some(system_instructions) },
        messages: messages.clone(),
        temperature: args.temperature,
        top_p: args.top_p,
        max_tokens: args.max_tokens,
        permissive: args.permissive,
        seed: args.seed,
    };

    if should_stream && mcp_manager.is_none() {
        let mut rx = match engine.stream_generate(&req) {
            Ok(rx) => rx,
            Err(e) => {
                eprintln!("{}: {}", "Error".red().bold(), e);
                return e.exit_code();
            }
        };

        while let Some(chunk) = rx.recv().await {
            match chunk {
                StreamChunk::Delta(delta) => {
                    print!("{}", delta);
                    let _ = io::stdout().flush();
                }
                StreamChunk::Done { .. } => {
                    println!();
                    break;
                }
                StreamChunk::Error(e) => {
                    eprintln!("\n{}: {}", "Stream error".red().bold(), e);
                    return e.exit_code();
                }
            }
        }
        ApfelExitCodes::SUCCESS
    } else {
        let session_mgr = crate::backend::session::SessionManager::new(engine.clone(), mcp_manager);
        let config = crate::core::context::ContextConfig {
            strategy: ContextStrategy::from_str(&args.context_strategy).unwrap_or_default(),
            max_turns: args.max_turns,
            output_reserve: 512,
            permissive: args.permissive,
        };

        let active_messages = messages.unwrap_or_else(|| {
            vec![crate::core::models::OpenAIMessage::user(&req.prompt)]
        });
        let res = match session_mgr
            .process_messages(
                &active_messages,
                None,
                &config,
                args.temperature,
                args.top_p,
                args.max_tokens,
                args.seed,
            )
            .await
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}: {}", "Error".red().bold(), e);
                return e.exit_code();
            }
        };

        if args.code_only {
            if let Some(code) = CodeCropper::extract(&res.content) {
                println!("{}", code);
                ApfelExitCodes::SUCCESS
            } else {
                eprintln!("Error: no code block found in response");
                ApfelExitCodes::NO_CODE
            }
        } else if args.output == "json" {
            let json = serde_json::json!({
                "content": res.content,
                "finish_reason": res.finish_reason,
                "tools_executed": res.tool_log
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
            ApfelExitCodes::SUCCESS
        } else {
            println!("{}", res.content);
            ApfelExitCodes::SUCCESS
        }
    }
}
