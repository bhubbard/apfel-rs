// ============================================================================
// chat.rs — Interactive terminal chat loop
// Part of apfel-rs
// ============================================================================

use crate::backend::engine::{BackendEngine, GenerateRequest, StreamChunk};
use crate::backend::session::SessionManager;
use crate::core::context::{ContextConfig, ContextManager, ContextStrategy};
use crate::core::error::ApfelError;
use crate::core::models::OpenAIMessage;
use crate::core::schema::SchemaParser;
use crate::mcp::client::MCPManager;
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{self, Write};
use std::sync::Arc;

/// Restricts file access permissions to 0600 (owner-only read/write) on Unix platforms.
#[cfg(unix)]
pub fn set_private_permissions(path: &str) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(path) {
        let mut perms = metadata.permissions();
        perms.set_mode(0o600);
        let _ = std::fs::set_permissions(path, perms);
    }
}

/// Saves conversation history to disk formatted as either JSON or Markdown.
pub fn save_conversation(path: &str, history: &[OpenAIMessage]) -> Result<(), ApfelError> {
    if path.ends_with(".json") {
        let json = serde_json::to_string_pretty(history)
            .map_err(|e| ApfelError::Runtime(e.to_string()))?;
        std::fs::write(path, json).map_err(|e| ApfelError::Runtime(e.to_string()))?;
    } else {
        let mut md = String::new();
        for m in history {
            md.push_str(&format!(
                "### {}\n\n{}\n\n",
                m.role.to_uppercase(),
                m.text_content()
            ));
        }
        std::fs::write(path, md).map_err(|e| ApfelError::Runtime(e.to_string()))?;
    }
    #[cfg(unix)]
    set_private_permissions(path);
    Ok(())
}

/// Runs the interactive terminal chat loop using rustyline and context trimming.
pub async fn run_chat_loop(
    engine: Arc<dyn BackendEngine>,
    system_prompt: Option<String>,
    permissive: bool,
    strategy: ContextStrategy,
    schema: Option<String>,
    mcp_manager: Option<Arc<MCPManager>>,
) -> Result<(), ApfelError> {
    println!("{}", "apfel interactive chat session".bold().cyan());
    println!(
        "{}",
        "Commands: /exit to quit, /clear to reset history, /info for status".dimmed()
    );

    if let Some(mcp) = &mcp_manager {
        let tools = mcp.all_tools();
        if !tools.is_empty() {
            println!("{}", format!("Loaded {} MCP tool(s)", tools.len()).dimmed());
        }
    }
    println!();

    let mut rl = DefaultEditor::new().map_err(|e| ApfelError::Runtime(e.to_string()))?;
    let histfile = std::env::var("APFEL_HISTFILE").ok();
    if let Some(ref hf) = histfile {
        let _ = rl.load_history(hf);
    }

    let (schema_ir, schema_directive) = if let Some(ref s) = schema {
        let schema_content = match std::fs::read_to_string(s) {
            Ok(c) => c,
            Err(_) => s.clone(),
        };
        match SchemaParser::parse(&schema_content, "OutputSchema") {
            Ok(ir) => {
                let directive = format!(
                    "You must respond ONLY with valid JSON conforming to this schema:\n{}",
                    schema_content
                );
                (Some(ir), Some(directive))
            }
            Err(e) => {
                eprintln!("{}: {}", "Schema error".red().bold(), e);
                return Err(ApfelError::Usage(e.to_string()));
            }
        }
    } else {
        (None, None)
    };

    let mut base_system = system_prompt;
    let build_system_content = |base: Option<&str>, directive: Option<&str>| -> Option<String> {
        match (base, directive) {
            (Some(b), Some(d)) if !b.is_empty() => Some(format!("{}\n\n{}", b, d)),
            (Some(b), _) if !b.is_empty() => Some(b.to_string()),
            (_, Some(d)) => Some(d.to_string()),
            _ => None,
        }
    };

    let mut history: Vec<OpenAIMessage> = Vec::new();

    if let Some(sys_text) =
        build_system_content(base_system.as_deref(), schema_directive.as_deref())
    {
        history.push(OpenAIMessage::system(sys_text));
    }

    let config = ContextConfig {
        strategy,
        max_turns: Some(20),
        output_reserve: 512,
        permissive,
    };

    loop {
        let readline = rl.readline(&format!("{} ", "›".bold().green()));
        match readline {
            Ok(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let _ = rl.add_history_entry(trimmed);

                if trimmed.starts_with("/save ") {
                    let path = trimmed[6..].trim();
                    match save_conversation(path, &history) {
                        Ok(_) => {
                            let fmt = if path.ends_with(".json") {
                                "JSON"
                            } else {
                                "Markdown"
                            };
                            println!(
                                "{}",
                                format!("Saved conversation {} to '{}'", fmt, path).green()
                            );
                        }
                        Err(e) => {
                            eprintln!("{}", format!("Failed to write to '{}': {}", path, e).red());
                        }
                    }
                    continue;
                }

                if trimmed.starts_with("/system ") {
                    let new_sys = trimmed[8..].trim();
                    history.retain(|m| m.role != "system");
                    if !new_sys.is_empty() {
                        base_system = Some(new_sys.to_string());
                    } else {
                        base_system = None;
                    }
                    if let Some(sys_text) =
                        build_system_content(base_system.as_deref(), schema_directive.as_deref())
                    {
                        history.insert(0, OpenAIMessage::system(sys_text));
                    }
                    if base_system.is_some() {
                        println!("{}", "Updated system instructions.".green());
                    } else {
                        println!("{}", "Cleared system instructions.".dimmed());
                    }
                    continue;
                }

                match trimmed {
                    "/exit" | "/quit" => {
                        println!("Bye!");
                        break;
                    }
                    "/clear" => {
                        history.clear();
                        if let Some(sys_text) = build_system_content(
                            base_system.as_deref(),
                            schema_directive.as_deref(),
                        ) {
                            history.push(OpenAIMessage::system(sys_text));
                        }
                        println!("{}", "Conversation cleared.".dimmed());
                        continue;
                    }
                    "/info" => {
                        println!(
                            "Context size: {} tokens | History turns: {}",
                            engine.context_size(),
                            history.len()
                        );
                        continue;
                    }
                    "/system" => {
                        let sys = history.iter().find(|m| m.role == "system");
                        match sys {
                            Some(m) => {
                                println!("Current system prompt:\n{}", m.text_content().cyan())
                            }
                            None => {
                                println!("No system prompt set. Use '/system <prompt>' to set one.")
                            }
                        }
                        continue;
                    }
                    "/help" => {
                        println!("Available commands:");
                        println!("  /exit, /quit      - Exit chat");
                        println!("  /clear            - Clear conversation history");
                        println!("  /info             - Display current context window status");
                        println!("  /system [prompt]  - View or update system instructions");
                        println!("  /save <file>      - Export conversation to .md or .json file");
                        continue;
                    }
                    _ => {}
                }

                history.push(OpenAIMessage::user(trimmed));

                if let Some(mcp) = &mcp_manager {
                    let session_mgr = SessionManager::new(engine.clone(), Some(mcp.clone()));
                    match session_mgr
                        .process_messages(&history, None, &config, None, None, None, None)
                        .await
                    {
                        Ok(res) => {
                            for log in &res.tool_log {
                                let status = if log.is_error {
                                    "error".red()
                                } else {
                                    "ok".green()
                                };
                                println!(
                                    "{}",
                                    format!("  ⚙ [mcp: {}] ({})", log.name, status).dimmed()
                                );
                            }
                            if let Some(ref ir) = schema_ir {
                                if let Err(err) = SchemaParser::validate(ir, &res.content) {
                                    eprintln!(
                                        "{}: Model response did not conform to schema: {}",
                                        "Warning".yellow().bold(),
                                        err
                                    );
                                }
                            }
                            print!("{}", "apple ".bold().purple());
                            println!("{}", res.content);
                            history.push(OpenAIMessage::assistant(res.content));
                        }
                        Err(e) => {
                            eprintln!("{}: {}", "Error".red().bold(), e);
                        }
                    }
                } else {
                    // Trim context to fit
                    let budget = engine.context_size().saturating_sub(config.output_reserve);
                    let trimmed_messages =
                        match ContextManager::trim_messages(&history, budget, &config, |t| {
                            engine.count_tokens(t)
                        }) {
                            Some(m) => m,
                            None => {
                                eprintln!("{}", "Error: conversation exceeds context limit.".red());
                                continue;
                            }
                        };

                    let sys = trimmed_messages
                        .iter()
                        .filter(|m| m.role == "system")
                        .map(|m| m.text_content())
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    let gen_req = GenerateRequest {
                        prompt: trimmed.to_string(),
                        system_prompt: if sys.is_empty() { None } else { Some(sys) },
                        messages: Some(trimmed_messages),
                        temperature: None,
                        top_p: None,
                        max_tokens: None,
                        permissive,
                        seed: None,
                        use_case: None,
                    };

                    let mut rx = match engine.stream_generate(&gen_req) {
                        Ok(rx) => rx,
                        Err(e) => {
                            eprintln!("{}: {}", "Error".red().bold(), e);
                            continue;
                        }
                    };

                    print!("{}", "apple ".bold().purple());
                    let _ = io::stdout().flush();

                    let mut assistant_resp = String::new();
                    while let Some(chunk) = rx.recv().await {
                        match chunk {
                            StreamChunk::Delta(delta) => {
                                print!("{}", delta);
                                let _ = io::stdout().flush();
                                assistant_resp.push_str(&delta);
                            }
                            StreamChunk::Done { .. } => {
                                println!();
                                break;
                            }
                            StreamChunk::Error(e) => {
                                eprintln!("\n{}: {}", "Stream error".red().bold(), e);
                                break;
                            }
                        }
                    }

                    if let Some(ref ir) = schema_ir {
                        if let Err(err) = SchemaParser::validate(ir, &assistant_resp) {
                            eprintln!(
                                "{}: Model response did not conform to schema: {}",
                                "Warning".yellow().bold(),
                                err
                            );
                        }
                    }

                    history.push(OpenAIMessage::assistant(assistant_resp));
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!("\nBye!");
                break;
            }
            Err(err) => {
                eprintln!("Readline error: {:?}", err);
                break;
            }
        }
    }

    if let Some(ref hf) = histfile {
        let _ = rl.save_history(hf);
        #[cfg(unix)]
        set_private_permissions(hf);
    }

    Ok(())
}
