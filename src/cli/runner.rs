// ============================================================================
// runner.rs — CLI command execution and dispatch
// Part of apfel-rs
// ============================================================================

use crate::backend::engine::{BackendEngine, GenerateRequest, StreamChunk};
use crate::cli::args::CliArgs;
use crate::cli::chat::run_chat_loop;
use crate::core::code_cropper::extract_code;
use crate::core::context::ContextStrategy;
use crate::core::error::ApfelExitCodes;
use crate::core::schema::SchemaParser;
use crate::core::stop_matcher::{StopMatchResult, StopSequenceMatcher};
use crate::mcp::client::MCPManager;
use crate::server::run_server;
use colored::Colorize;
use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

/// Executes the apfel CLI with the configured backend engine.
pub async fn run_cli(args: CliArgs) -> i32 {
    let engine = crate::backend::create_engine_with_adapter(
        args.engine.as_deref(),
        args.model.as_deref(),
        args.adapter.as_deref(),
    );
    run_cli_with_engine(args, engine).await
}

/// Executes the apfel CLI with a specified backend engine.
pub async fn run_cli_with_engine(args: CliArgs, engine: Arc<dyn BackendEngine>) -> i32 {
    if args.no_color {
        colored::control::set_override(false);
    }

    // Validation: --adapter path must exist if specified
    if let Some(path) = &args.adapter {
        if !std::path::Path::new(path).exists() {
            eprintln!(
                "{}: adapter path not found: {}",
                "Usage error".red().bold(),
                path
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
    }

    // Validation: --code conflict checks
    if args.code {
        if args.stream {
            eprintln!(
                "{}: --code cannot be used with --stream",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
        if args.chat {
            eprintln!(
                "{}: --code cannot be used with --chat",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
        if args.serve {
            eprintln!(
                "{}: --code cannot be used with --serve",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
        if args.batch {
            eprintln!(
                "{}: --code cannot be used with --batch",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
        if args.schema.is_some() {
            eprintln!(
                "{}: --code cannot be used with --schema",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
        if args.count_tokens || args.model_info || args.benchmark {
            eprintln!(
                "{}: --code cannot be used with non-generating modes",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
    }

    // Validation: --require-complete conflict checks
    if args.require_complete
        && (args.serve
            || args.model_info
            || args.count_tokens
            || args.benchmark
            || args.chat
            || args.batch)
    {
        eprintln!(
            "{}: --require-complete is only supported for prompt generation",
            "Usage error".red().bold()
        );
        return ApfelExitCodes::USAGE_ERROR;
    }

    // Validation: --batch conflict checks
    if args.batch {
        if args.stream {
            eprintln!(
                "{}: --batch cannot be used with --stream",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
        if args.chat {
            eprintln!(
                "{}: --batch cannot be used with --chat",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
        if args.serve {
            eprintln!(
                "{}: --batch cannot be used with --serve",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
        if args.count_tokens || args.model_info || args.benchmark {
            eprintln!(
                "{}: --batch cannot be used with non-generating modes",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
    }

    // Environmental fallback handling for non-generating / daemon modes (#496)
    if args.serve || args.model_info || args.benchmark {
        let mode_name = if args.serve {
            "--serve"
        } else if args.model_info {
            "--model-info"
        } else {
            "--benchmark"
        };

        if args.was_flag_explicit("-t", "--temperature") {
            eprintln!(
                "{}: --temperature cannot be used with {}",
                "Usage error".red().bold(),
                mode_name
            );
            return ApfelExitCodes::USAGE_ERROR;
        } else if args.temperature.is_some() && args.serve {
            eprintln!(
                "info: APFEL_TEMPERATURE set in environment is ignored in {} mode",
                mode_name
            );
        }

        if args.was_flag_explicit("", "--top-p") {
            eprintln!(
                "{}: --top-p cannot be used with {}",
                "Usage error".red().bold(),
                mode_name
            );
            return ApfelExitCodes::USAGE_ERROR;
        } else if args.top_p.is_some() && args.serve {
            eprintln!(
                "info: APFEL_TOP_P set in environment is ignored in {} mode",
                mode_name
            );
        }

        if args.was_flag_explicit("", "--max-tokens") {
            eprintln!(
                "{}: --max-tokens cannot be used with {}",
                "Usage error".red().bold(),
                mode_name
            );
            return ApfelExitCodes::USAGE_ERROR;
        } else if args.max_tokens.is_some() && args.serve {
            eprintln!(
                "info: APFEL_MAX_TOKENS set in environment is ignored in {} mode",
                mode_name
            );
        }

        if args.was_flag_explicit("", "--seed") {
            eprintln!(
                "{}: --seed cannot be used with {}",
                "Usage error".red().bold(),
                mode_name
            );
            return ApfelExitCodes::USAGE_ERROR;
        } else if args.seed.is_some() && args.serve {
            eprintln!(
                "info: APFEL_SEED set in environment is ignored in {} mode",
                mode_name
            );
        }

        if args.was_flag_explicit("-s", "--system") {
            eprintln!(
                "{}: --system cannot be used with {}",
                "Usage error".red().bold(),
                mode_name
            );
            return ApfelExitCodes::USAGE_ERROR;
        } else if args.system.is_some() && args.serve {
            eprintln!(
                "info: APFEL_SYSTEM_PROMPT set in environment is ignored in {} mode",
                mode_name
            );
        }
    }

    // Validation: stop sequences
    for s in &args.stop {
        if s.is_empty() {
            eprintln!(
                "{}: stop sequence cannot be empty",
                "Usage error".red().bold()
            );
            return ApfelExitCodes::USAGE_ERROR;
        }
    }

    // 0. Completions Generator
    if let Some(shell_name) = &args.completions {
        return run_completions(shell_name);
    }

    // 0.5 Batch Processing Mode
    if args.batch {
        return crate::cli::batch::run_batch_mode(args, engine).await;
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
            eprintln!(
                "Unsupported shell '{}'. Supported: bash, zsh, fish, powershell",
                shell_name
            );
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
    let measured_str = if engine.context_window_measured() {
        "(measured)"
    } else {
        "(assumed - model cold start)"
    };

    println!("apfel v{} — model info", env!("CARGO_PKG_VERSION"));
    println!("├ model:      {}", engine.model_name());
    println!("├ on-device:  true (always)");
    println!("├ available:  {}", available);
    println!("├ context:    {} tokens {}", context, measured_str);
    if let Some(adapter) = engine.adapter_path() {
        println!("├ adapter:    {}", adapter);
    }
    println!("├ languages:  {}", languages);
    println!("└ framework:  {}", engine.framework_name());

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
        use_case: None,
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
                prompt_text.push_str(&format!(
                    "--- File: {} ---\n{}\n--- End File ---",
                    file_path, content
                ));
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
    if args.code {
        if !system_instructions.is_empty() {
            system_instructions.push_str("\n\n");
        }
        system_instructions.push_str(
            "Output only the requested code. Do not include markdown code fences, backticks, or any conversational prose or explanations.",
        );
    }

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

    let should_stream =
        !args.code && (args.stream || io::stdout().is_terminal()) && !args.no_stream;

    let req = GenerateRequest {
        prompt: prompt_text,
        system_prompt: if system_instructions.is_empty() {
            None
        } else {
            Some(system_instructions)
        },
        messages: messages.clone(),
        temperature: args.temperature,
        top_p: args.top_p,
        max_tokens: args.max_tokens,
        permissive: args.permissive,
        seed: args.seed,
        use_case: None,
    };

    if should_stream && mcp_manager.is_none() {
        let mut rx = match engine.stream_generate(&req) {
            Ok(rx) => rx,
            Err(e) => {
                eprintln!("{}: {}", "Error".red().bold(), e);
                return e.exit_code();
            }
        };

        let mut stop_matcher = match StopSequenceMatcher::new(&args.stop) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("{}: {}", "Error".red().bold(), e);
                return e.exit_code();
            }
        };

        let mut matched_stop = false;
        let mut final_finish_reason = "stop".to_string();

        while let Some(chunk) = rx.recv().await {
            match chunk {
                StreamChunk::Delta(delta) => match stop_matcher.feed(&delta) {
                    StopMatchResult::Emit(s) => {
                        print!("{}", s);
                        let _ = io::stdout().flush();
                    }
                    StopMatchResult::Holding => {}
                    StopMatchResult::Matched { emitted, .. } => {
                        print!("{}", emitted);
                        let _ = io::stdout().flush();
                        matched_stop = true;
                        final_finish_reason = "stop".to_string();
                        break;
                    }
                },
                StreamChunk::Done { finish_reason } => {
                    let flushed = stop_matcher.flush();
                    if !flushed.is_empty() {
                        print!("{}", flushed);
                        let _ = io::stdout().flush();
                    }
                    if !matched_stop {
                        final_finish_reason = finish_reason;
                    }
                    println!();
                    break;
                }
                StreamChunk::Error(e) => {
                    eprintln!("\n{}: {}", "Stream error".red().bold(), e);
                    return e.exit_code();
                }
            }
        }

        if args.require_complete && final_finish_reason == "length" && !matched_stop {
            return ApfelExitCodes::INCOMPLETE_RESPONSE;
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

        let active_messages =
            messages.unwrap_or_else(|| vec![crate::core::models::OpenAIMessage::user(&req.prompt)]);
        let t_start = Instant::now();
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

        let mut content = res.content;
        let mut finish_reason = res.finish_reason;

        // Apply stop sequence truncation if requested
        if !args.stop.is_empty() {
            let (truncated, matched_seq) = StopSequenceMatcher::truncate_text(&content, &args.stop);
            if matched_seq.is_some() {
                finish_reason = "stop".to_string();
            }
            content = truncated;
        }

        // Check require_complete
        if args.require_complete && finish_reason == "length" {
            // Suppress stdout for incomplete generation in non-streaming mode
            return ApfelExitCodes::INCOMPLETE_RESPONSE;
        }

        // Telemetry recording if requested
        if let Some(telemetry_path) = &args.telemetry {
            let record = serde_json::json!({
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "duration_ms": t_start.elapsed().as_millis(),
                "finish_reason": finish_reason,
                "output_bytes": content.len(),
            });
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(telemetry_path)
            {
                let _ = writeln!(file, "{}", record);
            }
        }

        if args.code {
            match extract_code(&content) {
                Ok(extracted) => {
                    if args.output == "json" {
                        let json = serde_json::json!({
                            "content": extracted.content.trim_end_matches('\n'),
                            "language": extracted.language,
                            "finish_reason": finish_reason,
                            "tools_executed": res.tool_log
                        });
                        println!("{}", serde_json::to_string_pretty(&json).unwrap());
                    } else {
                        print!("{}", extracted.content);
                    }
                    ApfelExitCodes::SUCCESS
                }
                Err(e) => {
                    if args.output == "json" {
                        let json = serde_json::json!({
                            "content": "",
                            "language": null,
                            "finish_reason": finish_reason,
                            "error": e.to_string(),
                            "tools_executed": res.tool_log
                        });
                        println!("{}", serde_json::to_string_pretty(&json).unwrap());
                    } else if !args.quiet {
                        eprintln!("{}: {}", "Error".red().bold(), e);
                    }
                    e.exit_code()
                }
            }
        } else if args.output == "json" {
            let json = serde_json::json!({
                "content": content,
                "finish_reason": finish_reason,
                "tools_executed": res.tool_log
            });
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
            ApfelExitCodes::SUCCESS
        } else {
            println!("{}", content);
            ApfelExitCodes::SUCCESS
        }
    }
}
