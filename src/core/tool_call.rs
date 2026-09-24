// ============================================================================
// tool_call.rs — Tool call detection, parsing, repair, and output truncation
// Part of apfel-rs
// ============================================================================

use crate::core::models::OpenAITool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParsedToolCall {
    pub id: String,
    pub name: String,
    pub arguments_string: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolLogEntry {
    pub name: String,
    pub args: String,
    pub result: String,
    pub is_error: bool,
}

pub struct ToolCallHandler;

impl ToolCallHandler {
    pub const MCP_REPROMPT_CAP: usize = 3;

    /// Build format instructions telling model how to emit tool calls
    pub fn build_output_format_instructions(tools: &[OpenAITool]) -> String {
        let tool_names = tools
            .iter()
            .map(|t| t.function.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "## Tool Calling Format\n\
            When you need to call a tool, respond ONLY with a JSON object in this format:\n\
            ```json\n\
            {{\n  \
              \"tool_calls\": [\n    \
                {{\n      \
                  \"id\": \"call_123\",\n      \
                  \"type\": \"function\",\n      \
                  \"function\": {{\n        \
                    \"name\": \"function_name\",\n        \
                    \"arguments\": {{ /* args here */ }}\n      \
                  }}\n    \
                }}\n  \
              ]\n\
            }}\n\
            ```\n\
            Available tools: ({})\n\
            Do not include any preamble before the tool call if calling a tool.",
            tool_names
        )
    }

    /// Detect and parse tool calls from response text
    pub fn detect_tool_call(response: &str) -> Option<Vec<ParsedToolCall>> {
        for candidate in Self::extract_candidates(response) {
            if let Some(calls) = Self::parse_tool_call_json(&candidate) {
                if !calls.is_empty() {
                    return Some(calls);
                }
            }
            if let Some(repaired) = Self::repair_unclosed_brackets(&candidate) {
                if let Some(calls) = Self::parse_tool_call_json(&repaired) {
                    if !calls.is_empty() {
                        return Some(calls);
                    }
                }
            }
        }
        None
    }

    fn extract_candidates(response: &str) -> Vec<String> {
        let mut candidates = Vec::new();
        let trimmed = response.trim();

        // 1. Whole response
        candidates.push(trimmed.to_string());

        // 2. Extract from markdown code fences
        let mut in_fence = false;
        let mut fence_buf = String::new();
        for line in trimmed.lines() {
            let line_trimmed = line.trim();
            if line_trimmed.starts_with("```") {
                if in_fence {
                    candidates.push(fence_buf.trim().to_string());
                    fence_buf.clear();
                    in_fence = false;
                } else {
                    in_fence = true;
                }
            } else if in_fence {
                fence_buf.push_str(line);
                fence_buf.push('\n');
            }
        }

        // 3. Find first { "tool_calls" substring
        if let Some(idx) = trimmed.find("{\"tool_calls\"") {
            candidates.push(trimmed[idx..].to_string());
        } else if let Some(idx) = trimmed.find("{\n  \"tool_calls\"") {
            candidates.push(trimmed[idx..].to_string());
        } else if let Some(idx) = trimmed.find("\"tool_calls\"") {
            if let Some(brace_idx) = trimmed[..idx].rfind('{') {
                candidates.push(trimmed[brace_idx..].to_string());
            }
        }

        candidates
    }

    fn parse_tool_call_json(candidate: &str) -> Option<Vec<ParsedToolCall>> {
        let val: serde_json::Value = serde_json::from_str(candidate).ok()?;
        let obj = val.as_object()?;
        let calls_arr = obj.get("tool_calls")?.as_array()?;

        let mut result = Vec::new();
        for (i, item) in calls_arr.iter().enumerate() {
            let call_obj = item.as_object()?;
            let id = call_obj
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or(&format!("call_{}", i))
                .to_string();

            let func_obj = call_obj.get("function")?.as_object()?;
            let name = func_obj.get("name")?.as_str()?.to_string();

            let args_str = match func_obj.get("arguments") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(v) => serde_json::to_string(v).unwrap_or_default(),
                None => "{}".to_string(),
            };

            result.push(ParsedToolCall {
                id,
                name,
                arguments_string: args_str,
            });
        }

        Some(result)
    }

    fn repair_unclosed_brackets(s: &str) -> Option<String> {
        let mut open_braces = 0;
        let mut open_brackets = 0;
        let mut in_str = false;
        let mut escape = false;

        for c in s.chars() {
            if escape {
                escape = false;
                continue;
            }
            if c == '\\' {
                escape = true;
                continue;
            }
            if c == '"' {
                in_str = !in_str;
                continue;
            }
            if !in_str {
                match c {
                    '{' => open_braces += 1,
                    '}' => if open_braces > 0 { open_braces -= 1 },
                    '[' => open_brackets += 1,
                    ']' => if open_brackets > 0 { open_brackets -= 1 },
                    _ => {}
                }
            }
        }

        if open_braces == 0 && open_brackets == 0 {
            return None;
        }

        let mut repaired = s.to_string();
        if in_str {
            repaired.push('"');
        }
        for _ in 0..open_brackets {
            repaired.push(']');
        }
        for _ in 0..open_braces {
            repaired.push('}');
        }

        Some(repaired)
    }
}

pub struct ToolOutputTruncator;

impl ToolOutputTruncator {
    const MARKER_RESERVE_TOKENS: usize = 24;

    pub fn truncate(text: &str, token_count: usize, budget_tokens: usize) -> (String, bool) {
        let char_count = text.chars().count();
        if char_count == 0 || token_count <= budget_tokens {
            return (text.to_string(), false);
        }

        let tokens_per_char = (token_count as f64) / (char_count as f64);
        let content_budget = budget_tokens.saturating_sub(Self::MARKER_RESERVE_TOKENS);
        let allowed_chars = if tokens_per_char > 0.0 {
            ((content_budget as f64) / tokens_per_char) as usize
        } else {
            0
        }
        .min(char_count);

        let head_chars = allowed_chars / 2;
        let tail_chars = allowed_chars.saturating_sub(head_chars);
        let shown_tokens = ((allowed_chars as f64) * tokens_per_char).round() as usize;

        let head: String = text.chars().take(head_chars).collect();
        let tail: String = text.chars().skip(char_count.saturating_sub(tail_chars)).collect();
        let marker = format!(
            "\n\n[tool output truncated: {} of {} tokens shown]\n\n",
            shown_tokens, token_count
        );

        (format!("{}{}{}", head, marker, tail), true)
    }
}

pub struct StreamingToolCallGate;

impl StreamingToolCallGate {
    /// Determines whether the accumulated prefix could be the start of a tool-call JSON object.
    /// Returns true if the streamer should buffer/hold, or false if it should immediately flush.
    pub fn is_plausible_tool_call_prefix(text: &str) -> bool {
        let trimmed = text.trim_start();
        if trimmed.is_empty() {
            return true;
        }

        // Fenced blocks
        if trimmed.starts_with('`') {
            if trimmed.starts_with("```json") {
                let after = trimmed["```json".len()..].trim_start();
                if after.is_empty() {
                    return true;
                }
                return Self::is_plausible_tool_call_prefix(after);
            }
            if trimmed.starts_with("```") {
                let after = trimmed["```".len()..].trim_start();
                if after.is_empty() {
                    return true;
                }
                return Self::is_plausible_tool_call_prefix(after);
            }
            return true; // partial backticks "`", "``"
        }

        if trimmed.starts_with('{') {
            let target = "{\"tool_calls\"";
            if target.starts_with(trimmed) || trimmed.starts_with(target) {
                return true;
            }
            let no_spaces: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
            if target.starts_with(&no_spaces) || no_spaces.starts_with(target) {
                return true;
            }
        }

        false
    }
}
