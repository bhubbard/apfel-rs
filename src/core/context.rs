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
                if kept.is_empty() && !conversation.is_empty() {
                    if let Some(newest) = conversation.last() {
                        let text = newest.text_content();
                        let compacted = Self::compact_to_fit(&text, conv_budget, &count_tokens);
                        if !compacted.is_empty() {
                            kept.push(OpenAIMessage {
                                role: newest.role.clone(),
                                content: Some(crate::core::models::MessageContent::Text(compacted)),
                                name: newest.name.clone(),
                                tool_calls: None,
                                tool_call_id: None,
                            });
                        }
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
                if kept.is_empty() && !conversation.is_empty() {
                    if let Some(first) = conversation.first() {
                        let text = first.text_content();
                        let compacted = Self::compact_to_fit(&text, conv_budget, &count_tokens);
                        if !compacted.is_empty() {
                            kept.push(OpenAIMessage {
                                role: first.role.clone(),
                                content: Some(crate::core::models::MessageContent::Text(compacted)),
                                name: first.name.clone(),
                                tool_calls: None,
                                tool_call_id: None,
                            });
                        }
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
                if kept.is_empty() && !turn_window.is_empty() {
                    if let Some(newest) = turn_window.last() {
                        let text = newest.text_content();
                        let compacted = Self::compact_to_fit(&text, conv_budget, &count_tokens);
                        if !compacted.is_empty() {
                            kept.push(OpenAIMessage {
                                role: newest.role.clone(),
                                content: Some(crate::core::models::MessageContent::Text(compacted)),
                                name: newest.name.clone(),
                                tool_calls: None,
                                tool_call_id: None,
                            });
                        }
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

    /// Compacts and truncates text to fit within a given token budget
    pub fn compact_to_fit<F>(text: &str, budget_tokens: usize, count_tokens: &F) -> String
    where
        F: Fn(&str) -> usize,
    {
        if count_tokens(text) <= budget_tokens {
            return text.to_string();
        }

        // Phase 1: Normalize excessive blank lines and trailing spaces
        let lines: Vec<&str> = text.lines().collect();
        let mut compacted_lines = Vec::new();
        let mut last_was_empty = false;
        for line in &lines {
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                if !last_was_empty {
                    compacted_lines.push("");
                    last_was_empty = true;
                }
            } else {
                compacted_lines.push(trimmed);
                last_was_empty = false;
            }
        }
        let joined = compacted_lines.join("\n");
        if count_tokens(&joined) <= budget_tokens {
            return joined;
        }

        // Phase 2: Binary search truncation of lines to strictly fit budget
        let mut low = 0;
        let mut high = compacted_lines.len();
        let mut best_str = String::new();

        while low <= high {
            let mid = (low + high) / 2;
            let candidate = compacted_lines[..mid].join("\n");
            if count_tokens(&candidate) <= budget_tokens {
                best_str = candidate;
                low = mid + 1;
            } else {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            }
        }

        if best_str.is_empty() {
            // Character-level truncation fallback
            let chars: Vec<char> = text.chars().collect();
            let mut c_low = 0;
            let mut c_high = chars.len();
            while c_low <= c_high {
                let c_mid = (c_low + c_high) / 2;
                let candidate: String = chars[..c_mid].iter().collect();
                if count_tokens(&candidate) <= budget_tokens {
                    best_str = candidate;
                    c_low = c_mid + 1;
                } else {
                    if c_mid == 0 {
                        break;
                    }
                    c_high = c_mid - 1;
                }
            }
        }

        best_str
    }
}
