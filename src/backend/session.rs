// ============================================================================
// session.rs — Session execution pipeline with MCP tool calling loops
// Part of apfel-rs
// ============================================================================

use crate::backend::engine::{BackendEngine, GenerateRequest};
use crate::core::context::{ContextConfig, ContextManager};
use crate::core::error::ApfelError;
use crate::core::models::{OpenAIMessage, OpenAITool};
use crate::core::tool_call::{ToolCallHandler, ToolLogEntry, ToolOutputTruncator};
use crate::mcp::client::MCPManager;
use std::sync::Arc;

pub struct SessionManager {
    engine: Arc<dyn BackendEngine>,
    mcp_manager: Option<Arc<MCPManager>>,
}

pub struct SessionResult {
    pub content: String,
    pub tool_log: Vec<ToolLogEntry>,
    pub finish_reason: String,
}

impl SessionManager {
    pub fn new(engine: Arc<dyn BackendEngine>, mcp_manager: Option<Arc<MCPManager>>) -> Self {
        Self { engine, mcp_manager }
    }

    pub async fn process_messages(
        &self,
        messages: &[OpenAIMessage],
        tools: Option<&[OpenAITool]>,
        config: &ContextConfig,
        temperature: Option<f64>,
        top_p: Option<f64>,
        max_tokens: Option<usize>,
        seed: Option<u64>,
    ) -> Result<SessionResult, ApfelError> {
        let budget = self
            .engine
            .context_size()
            .saturating_sub(config.output_reserve);

        let trimmed_messages = ContextManager::trim_messages(
            messages,
            budget,
            config,
            |t| self.engine.count_tokens(t),
        )
        .ok_or_else(|| {
            ApfelError::ContextOverflow("Messages exceed context budget after trimming".to_string())
        })?;

        // Combine explicit tools with MCP tools
        let mut all_tools = Vec::new();
        if let Some(t) = tools {
            all_tools.extend_from_slice(t);
        }
        if let Some(mcp) = &self.mcp_manager {
            all_tools.extend(mcp.all_tools());
        }

        // If tools are provided, add tool calling format to instructions
        let mut final_messages = trimmed_messages;
        if !all_tools.is_empty() {
            let tool_instructions = ToolCallHandler::build_output_format_instructions(&all_tools);
            let has_sys = final_messages.iter().any(|m| m.role == "system");
            if !has_sys {
                final_messages.insert(0, OpenAIMessage::system(tool_instructions));
            } else {
                for m in &mut final_messages {
                    if m.role == "system" {
                        let cur = m.text_content();
                        *m = OpenAIMessage::system(format!("{}\n\n{}", cur, tool_instructions));
                        break;
                    }
                }
            }
        }

        let mut current_messages = final_messages;
        let mut aggregated_log = Vec::new();
        let mut reprompts = 0;

        loop {
            let last_user_prompt = current_messages
                .iter()
                .rev()
                .find(|m| m.role == "user" || m.role == "tool")
                .map(|m| m.text_content())
                .unwrap_or_else(|| "Hello".to_string());

            let system_prompt = current_messages
                .iter()
                .filter(|m| m.role == "system")
                .map(|m| m.text_content())
                .collect::<Vec<_>>()
                .join("\n\n");

            let gen_req = GenerateRequest {
                prompt: last_user_prompt.clone(),
                system_prompt: if system_prompt.is_empty() { None } else { Some(system_prompt) },
                messages: Some(current_messages.clone()),
                temperature,
                top_p,
                max_tokens,
                permissive: config.permissive,
                seed,
            };

            let resp = self.engine.generate(&gen_req)?;

            // Check if model emitted a tool call
            if let Some(calls) = ToolCallHandler::detect_tool_call(&resp.content) {
                if reprompts >= ToolCallHandler::MCP_REPROMPT_CAP {
                    return Err(ApfelError::ToolExecution(format!(
                        "MCP tool loop hit its {}-round cap with a tool call still pending",
                        ToolCallHandler::MCP_REPROMPT_CAP
                    )));
                }

                reprompts += 1;
                let mut tool_results_text = Vec::new();

                for call in &calls {
                    let (res, is_err) = if let Some(mcp) = &self.mcp_manager {
                        if let Some(exec_res) = mcp.call_tool(&call.name, &call.arguments_string).await {
                            exec_res?
                        } else {
                            (format!("Tool '{}' not found", call.name), true)
                        }
                    } else {
                        (format!("No tool handler registered for '{}'", call.name), true)
                    };

                    aggregated_log.push(ToolLogEntry {
                        name: call.name.clone(),
                        args: call.arguments_string.clone(),
                        result: res.clone(),
                        is_error: is_err,
                    });

                    // Truncate output to fit within token budget
                    let token_cost = self.engine.count_tokens(&res);
                    let (truncated, _) = ToolOutputTruncator::truncate(&res, token_cost, 1024);
                    tool_results_text.push(format!("Tool '{}' output:\n{}", call.name, truncated));
                }

                // Add assistant response and tool output to conversation
                current_messages.push(OpenAIMessage::assistant(&resp.content));
                let follow_up = format!(
                    "The tool(s) returned:\n{}\n\nContinue the conversation using this information.",
                    tool_results_text.join("\n\n")
                );
                current_messages.push(OpenAIMessage::user(follow_up));
            } else {
                return Ok(SessionResult {
                    content: resp.content,
                    tool_log: aggregated_log,
                    finish_reason: resp.finish_reason,
                });
            }
        }
    }
}
