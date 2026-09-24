// ============================================================================
// chat.rs — Interactive terminal chat loop
// Part of apfel-rs
// ============================================================================

use crate::backend::engine::{BackendEngine, GenerateRequest, StreamChunk};
use crate::core::context::{ContextConfig, ContextManager, ContextStrategy};
use crate::core::error::ApfelError;
use crate::core::models::OpenAIMessage;
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::io::{self, Write};
use std::sync::Arc;

pub async fn run_chat_loop(
    engine: Arc<dyn BackendEngine>,
    system_prompt: Option<String>,
    permissive: bool,
    strategy: ContextStrategy,
) -> Result<(), ApfelError> {
    println!("{}", "apfel interactive chat session".bold().cyan());
    println!("{}", "Commands: /exit to quit, /clear to reset history, /info for status".dimmed());
    println!();

    let mut rl = DefaultEditor::new().map_err(|e| ApfelError::Runtime(e.to_string()))?;
    let mut history: Vec<OpenAIMessage> = Vec::new();

    if let Some(sys) = &system_prompt {
        history.push(OpenAIMessage::system(sys));
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

                match trimmed {
                    "/exit" | "/quit" => {
                        println!("Bye!");
                        break;
                    }
                    "/clear" => {
                        history.clear();
                        if let Some(sys) = &system_prompt {
                            history.push(OpenAIMessage::system(sys));
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
                    "/help" => {
                        println!("Available commands:");
                        println!("  /exit, /quit - Exit chat");
                        println!("  /clear       - Clear conversation history");
                        println!("  /info        - Display current context window status");
                        continue;
                    }
                    _ => {}
                }

                history.push(OpenAIMessage::user(trimmed));

                // Trim context to fit
                let budget = engine.context_size().saturating_sub(config.output_reserve);
                let trimmed_messages = match ContextManager::trim_messages(
                    &history,
                    budget,
                    &config,
                    |t| engine.count_tokens(t),
                ) {
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

                history.push(OpenAIMessage::assistant(assistant_resp));
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

    Ok(())
}
