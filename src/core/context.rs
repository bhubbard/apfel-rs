// ============================================================================
// context.rs — Context strategies and conversation trimming
// Part of apfel-rs
// ============================================================================

use crate::core::models::OpenAIMessage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextStrategy {
    NewestFirst,
    OldestFirst,
    SlidingWindow,
    Summarize,
    Strict,
}

impl Default for ContextStrategy {
    fn default() -> Self {
        Self::NewestFirst
    }
}

impl std::str::FromStr for ContextStrategy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "newest-first" | "newest" => Ok(Self::NewestFirst),
            "oldest-first" | "oldest" => Ok(Self::OldestFirst),
            "sliding-window" | "sliding" => Ok(Self::SlidingWindow),
            "summarize" => Ok(Self::Summarize),
            "strict" => Ok(Self::Strict),
            other => Err(format!("Unknown context strategy: {}", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextConfig {
    pub strategy: ContextStrategy,
    pub max_turns: Option<usize>,
    pub output_reserve: usize,
    pub permissive: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            strategy: ContextStrategy::NewestFirst,
            max_turns: None,
            output_reserve: 512,
            permissive: false,
        }
    }
}

pub struct ContextManager;

impl ContextManager {
    /// Trim conversation messages to fit within budget tokens according to config
    pub fn trim_messages<F>(
        messages: &[OpenAIMessage],
        budget_tokens: usize,
        config: &ContextConfig,
        count_tokens: F,
    ) -> Option<Vec<OpenAIMessage>>
    where
        F: Fn(&str) -> usize,
    {
        // Split system/developer instructions from regular conversation
        let mut instructions = Vec::new();
        let mut conversation = Vec::new();

        for msg in messages {
            if msg.role == "system" || msg.role == "developer" {
                instructions.push(msg.clone());
            } else {
                conversation.push(msg.clone());
            }
        }

        // Measure instructions overhead
        let instructions_text = instructions
            .iter()
            .map(|m| m.text_content())
            .collect::<Vec<_>>()
            .join("\n");
        let instructions_cost = count_tokens(&instructions_text);

        if instructions_cost > budget_tokens {
            return None; // Even instructions don't fit
        }

        let conv_budget = budget_tokens - instructions_cost;

        let trimmed_conv = match config.strategy {
            ContextStrategy::Strict => {
                let conv_text = conversation
                    .iter()
                    .map(|m| m.text_content())
                    .collect::<Vec<_>>()
                    .join("\n");
                if count_tokens(&conv_text) <= conv_budget {
                    conversation
                } else {
                    return None;
                }
            }
            ContextStrategy::NewestFirst => {
                let mut kept = Vec::new();
                let mut current_cost = 0;
                for msg in conversation.iter().rev() {
                    let cost = count_tokens(&msg.text_content());
                    if current_cost + cost <= conv_budget {
                        kept.push(msg.clone());
                        current_cost += cost;
                    } else {
                        break;
                    }
                }
                kept.reverse();
                kept
            }
            ContextStrategy::OldestFirst => {
                let mut kept = Vec::new();
                let mut current_cost = 0;
                for msg in conversation.iter() {
                    let cost = count_tokens(&msg.text_content());
                    if current_cost + cost <= conv_budget {
                        kept.push(msg.clone());
                        current_cost += cost;
                    } else {
                        break;
                    }
                }
                kept
            }
            ContextStrategy::SlidingWindow => {
                let max_turns = config.max_turns.unwrap_or(10);
                let turn_window = if conversation.len() > max_turns {
                    &conversation[conversation.len() - max_turns..]
                } else {
                    &conversation[..]
                };

                let mut kept = Vec::new();
                let mut current_cost = 0;
                for msg in turn_window.iter().rev() {
                    let cost = count_tokens(&msg.text_content());
                    if current_cost + cost <= conv_budget {
                        kept.push(msg.clone());
                        current_cost += cost;
                    } else {
                        break;
                    }
                }
                kept.reverse();
                kept
            }
            ContextStrategy::Summarize => {
                // For summarize, take newest messages that fit 70% of budget,
                // and a brief note about older messages
                let sub_budget = (conv_budget as f64 * 0.7) as usize;
                let mut kept = Vec::new();
                let mut current_cost = 0;
                for msg in conversation.iter().rev() {
                    let cost = count_tokens(&msg.text_content());
                    if current_cost + cost <= sub_budget {
                        kept.push(msg.clone());
                        current_cost += cost;
                    } else {
                        break;
                    }
                }
                kept.reverse();
                if kept.len() < conversation.len() {
                    let dropped = conversation.len() - kept.len();
                    instructions.push(OpenAIMessage::system(format!(
                        "[Context Note: {} prior messages were summarized/truncated due to context limit]",
                        dropped
                    )));
                }
                kept
            }
        };

        let mut out = instructions;
        out.extend(trimmed_conv);
        Some(out)
    }
}
