// ============================================================================
// protocol.rs — MCP JSON-RPC 2.0 protocol formatting and parsing
// Part of apfel-rs
// ============================================================================

use crate::core::error::ApfelError;
use crate::core::models::{FunctionDefinition, OpenAITool};
use serde::{Deserialize, Serialize};

pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

pub struct MCPProtocol;

impl MCPProtocol {
    pub fn initialize_request(id: usize) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "apfel",
                    "version": "1.0.0"
                }
            }
        })
        .to_string()
    }

    pub fn initialized_notification() -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        })
        .to_string()
    }

    pub fn tools_list_request(id: usize) -> String {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/list"
        })
        .to_string()
    }

    pub fn tools_call_request(id: usize, name: &str, arguments: &str) -> String {
        let args_obj: serde_json::Value = serde_json::from_str(arguments).unwrap_or_else(|_| {
            serde_json::json!({})
        });

        serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": args_obj
            }
        })
        .to_string()
    }

    pub fn validate_tool_arguments(name: &str, arguments: &str) -> Result<(), ApfelError> {
        let trimmed = arguments.trim();
        if trimmed.is_empty() {
            return Ok(());
        }
        serde_json::from_str::<serde_json::Value>(trimmed).map_err(|e| {
            ApfelError::ToolExecution(format!(
                "Tool '{}' arguments are not valid JSON: {} ({})",
                name, trimmed, e
            ))
        })?;
        Ok(())
    }

    pub fn parse_initialize_response(json_str: &str) -> Result<ServerInfo, ApfelError> {
        let val: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| ApfelError::MCP(format!("Invalid initialize response: {}", e)))?;

        let server_info = val
            .get("result")
            .and_then(|r| r.get("serverInfo"))
            .ok_or_else(|| ApfelError::MCP("Missing serverInfo in initialize response".to_string()))?;

        let name = server_info
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let version = server_info
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(ServerInfo { name, version })
    }

    pub fn parse_tools_list_response(json_str: &str) -> Result<Vec<OpenAITool>, ApfelError> {
        let val: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| ApfelError::MCP(format!("Invalid tools/list response: {}", e)))?;

        let tools_arr = val
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .ok_or_else(|| ApfelError::MCP("Missing tools array in tools/list response".to_string()))?;

        let mut out = Vec::new();
        for item in tools_arr {
            let name = item
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let parameters = item.get("inputSchema").cloned();

            out.push(OpenAITool {
                tool_type: "function".to_string(),
                function: FunctionDefinition {
                    name,
                    description,
                    parameters,
                },
            });
        }

        Ok(out)
    }

    pub fn parse_tools_call_response(json_str: &str) -> Result<(String, bool), ApfelError> {
        let val: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| ApfelError::MCP(format!("Invalid tools/call response: {}", e)))?;

        if let Some(err) = val.get("error") {
            let msg = err
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown MCP error");
            return Ok((format!("Error: {}", msg), true));
        }

        let result_obj = val.get("result").ok_or_else(|| {
            ApfelError::MCP("Missing result in tools/call response".to_string())
        })?;

        let is_error = result_obj
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if let Some(content_arr) = result_obj.get("content").and_then(|c| c.as_array()) {
            let texts: Vec<String> = content_arr
                .iter()
                .filter_map(|item| {
                    if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                        item.get("text").and_then(|t| t.as_str()).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            return Ok((texts.join("\n"), is_error));
        }

        Ok((serde_json::to_string(result_obj).unwrap_or_default(), is_error))
    }
}
