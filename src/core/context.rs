// ============================================================================
// context.rs — Context strategies and conversation trimming
// Part of apfel-rs
// ============================================================================

use crate::core::models::OpenAIMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Strategy used to truncate or prune messages when conversation exceeds context budget.
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

/// Configuration settings controlling context management and token limits.
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

/// Manages conversation context sizing and message pruning algorithms.
#[derive(Debug)]
pub struct ContextManager;

impl ContextManager {
    /// Calculate message token cost including tool call tokens if present
    pub fn message_cost<F>(msg: &OpenAIMessage, count_tokens: &F) -> usize
    where
        F: Fn(&str) -> usize,
    {
        let text = msg.text_content();
        let text_tokens = count_tokens(&text);
        if let Some(tool_calls) = &msg.tool_calls {
            let tc_text = tool_calls
                .iter()
                .map(|tc| format!("{} {}", tc.function.name, tc.function.arguments))
                .collect::<Vec<_>>()
                .join(" ");
            text_tokens + count_tokens(&tc_text)
        } else {
            text_tokens
        }
    }

    /// Groups conversation messages into atomic units.
    /// An assistant message containing `tool_calls` and all corresponding `role: "tool"`
    /// messages referencing those `tool_call_id`s are grouped together.
    /// Orphaned tool messages without preceding assistant tool calls are dropped.
    pub fn group_conversation(conversation: &[OpenAIMessage]) -> Vec<Vec<OpenAIMessage>> {
        let mut groups: Vec<Vec<OpenAIMessage>> = Vec::new();
        let mut i = 0;
        while i < conversation.len() {
            let msg = &conversation[i];
            if msg.role == "assistant" && msg.tool_calls.as_ref().map_or(false, |tc| !tc.is_empty())
            {
                let call_ids: HashSet<String> = msg
                    .tool_calls
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|tc| tc.id.clone())
                    .collect();
                let mut group = vec![msg.clone()];
                i += 1;
                while i < conversation.len() {
                    let next_msg = &conversation[i];
                    if next_msg.role == "tool" {
                        let matches = match &next_msg.tool_call_id {
                            Some(id) => call_ids.contains(id),
                            None => call_ids.len() == 1,
                        };
                        if matches {
                            group.push(next_msg.clone());
                            i += 1;
                            continue;
                        }
                    }
                    break;
                }
                groups.push(group);
            } else if msg.role == "tool" {
                // Orphaned tool response without an assistant tool-call message.
                // Per requirement: never leave a tool response without its assistant tool-call message.
                i += 1;
            } else {
                groups.push(vec![msg.clone()]);
                i += 1;
            }
        }
        groups
    }

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
        let groups = Self::group_conversation(&conversation);

        let trimmed_conv: Vec<OpenAIMessage> = match config.strategy {
            ContextStrategy::Strict => {
                let conv_text = conversation
                    .iter()
                    .map(|m| m.text_content())
                    .collect::<Vec<_>>()
                    .join("\n");
                let conv_cost = if conversation.iter().any(|m| m.tool_calls.is_some()) {
                    groups
                        .iter()
                        .flat_map(|g| g.iter())
                        .map(|m| Self::message_cost(m, &count_tokens))
                        .sum()
                } else {
                    count_tokens(&conv_text)
                };
                if conv_cost <= conv_budget {
                    groups.into_iter().flatten().collect()
                } else {
                    return None;
                }
            }
            ContextStrategy::NewestFirst => {
                let mut kept_groups = Vec::new();
                let mut current_cost = 0;
                for group in groups.iter().rev() {
                    let cost: usize = group
                        .iter()
                        .map(|m| Self::message_cost(m, &count_tokens))
                        .sum();
                    if current_cost + cost <= conv_budget {
                        kept_groups.push(group.clone());
                        current_cost += cost;
                    } else {
                        break;
                    }
                }
                if kept_groups.is_empty() && !groups.is_empty() {
                    if let Some(newest_group) = groups.last() {
                        if newest_group.len() == 1 {
                            let newest = &newest_group[0];
                            if newest.role != "tool" && newest.tool_calls.is_none() {
                                let text = newest.text_content();
                                let compacted =
                                    Self::compact_to_fit(&text, conv_budget, &count_tokens);
                                if !compacted.is_empty() {
                                    kept_groups.push(vec![OpenAIMessage {
                                        role: newest.role.clone(),
                                        content: Some(crate::core::models::MessageContent::Text(
                                            compacted,
                                        )),
                                        name: newest.name.clone(),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    }]);
                                }
                            }
                        }
                    }
                }
                kept_groups.reverse();
                kept_groups.into_iter().flatten().collect()
            }
            ContextStrategy::OldestFirst => {
                let mut kept_groups = Vec::new();
                let mut current_cost = 0;
                for group in groups.iter() {
                    let cost: usize = group
                        .iter()
                        .map(|m| Self::message_cost(m, &count_tokens))
                        .sum();
                    if current_cost + cost <= conv_budget {
                        kept_groups.push(group.clone());
                        current_cost += cost;
                    } else {
                        break;
                    }
                }
                if kept_groups.is_empty() && !groups.is_empty() {
                    if let Some(first_group) = groups.first() {
                        if first_group.len() == 1 {
                            let first = &first_group[0];
                            if first.role != "tool" && first.tool_calls.is_none() {
                                let text = first.text_content();
                                let compacted =
                                    Self::compact_to_fit(&text, conv_budget, &count_tokens);
                                if !compacted.is_empty() {
                                    kept_groups.push(vec![OpenAIMessage {
                                        role: first.role.clone(),
                                        content: Some(crate::core::models::MessageContent::Text(
                                            compacted,
                                        )),
                                        name: first.name.clone(),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    }]);
                                }
                            }
                        }
                    }
                }
                kept_groups.into_iter().flatten().collect()
            }
            ContextStrategy::SlidingWindow => {
                let max_turns = config.max_turns.unwrap_or(10);
                let turn_window = if groups.len() > max_turns {
                    &groups[groups.len() - max_turns..]
                } else {
                    &groups[..]
                };

                let mut kept_groups = Vec::new();
                let mut current_cost = 0;
                for group in turn_window.iter().rev() {
                    let cost: usize = group
                        .iter()
                        .map(|m| Self::message_cost(m, &count_tokens))
                        .sum();
                    if current_cost + cost <= conv_budget {
                        kept_groups.push(group.clone());
                        current_cost += cost;
                    } else {
                        break;
                    }
                }
                if kept_groups.is_empty() && !turn_window.is_empty() {
                    if let Some(newest_group) = turn_window.last() {
                        if newest_group.len() == 1 {
                            let newest = &newest_group[0];
                            if newest.role != "tool" && newest.tool_calls.is_none() {
                                let text = newest.text_content();
                                let compacted =
                                    Self::compact_to_fit(&text, conv_budget, &count_tokens);
                                if !compacted.is_empty() {
                                    kept_groups.push(vec![OpenAIMessage {
                                        role: newest.role.clone(),
                                        content: Some(crate::core::models::MessageContent::Text(
                                            compacted,
                                        )),
                                        name: newest.name.clone(),
                                        tool_calls: None,
                                        tool_call_id: None,
                                    }]);
                                }
                            }
                        }
                    }
                }
                kept_groups.reverse();
                kept_groups.into_iter().flatten().collect()
            }
            ContextStrategy::Summarize => {
                let sub_budget = (conv_budget as f64 * 0.7) as usize;
                let mut kept_groups = Vec::new();
                let mut current_cost = 0;
                for group in groups.iter().rev() {
                    let cost: usize = group
                        .iter()
                        .map(|m| Self::message_cost(m, &count_tokens))
                        .sum();
                    if current_cost + cost <= sub_budget {
                        kept_groups.push(group.clone());
                        current_cost += cost;
                    } else {
                        break;
                    }
                }
                kept_groups.reverse();
                let total_kept_messages: usize = kept_groups.iter().map(|g| g.len()).sum();
                let total_conv_messages = conversation.len();
                if total_kept_messages < total_conv_messages {
                    let dropped = total_conv_messages - total_kept_messages;
                    instructions.push(OpenAIMessage::system(format!(
                        "[Context Note: {} prior messages were summarized/truncated due to context limit]",
                        dropped
                    )));
                }
                kept_groups.into_iter().flatten().collect()
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
