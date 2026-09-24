// ============================================================================
// client.rs — MCP subprocess client and manager
// Part of apfel-rs
// ============================================================================

use crate::core::error::ApfelError;
use crate::core::models::OpenAITool;
use crate::core::security::scrub_mcp_environment;
use crate::mcp::protocol::MCPProtocol;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::timeout;

pub struct MCPConnection {
    command_path: String,
    tools: Vec<OpenAITool>,
    stdin: Mutex<ChildStdin>,
    reader: Mutex<BufReader<ChildStdout>>,
    child: Mutex<Child>,
    next_id: AtomicUsize,
    timeout_duration: Duration,
}

impl MCPConnection {
    pub async fn spawn(command_path: &str, timeout_secs: u64) -> Result<Self, ApfelError> {
        let mut cmd = if command_path.ends_with(".py") {
            let mut c = Command::new("python3");
            c.arg(command_path);
            c
        } else {
            let parts: Vec<&str> = command_path.split_whitespace().collect();
            if parts.is_empty() {
                return Err(ApfelError::MCP("Empty MCP command".to_string()));
            }
            let mut c = Command::new(parts[0]);
            for arg in &parts[1..] {
                c.arg(arg);
            }
            c
        };

        // Clean parent env to avoid leaking sensitive tokens
        let parent_env: HashMap<String, String> = std::env::vars().collect();
        let scrubbed = scrub_mcp_environment(&parent_env);
        cmd.env_clear();
        cmd.envs(scrubbed);

        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| ApfelError::MCP(format!("Failed to spawn MCP server '{}': {}", command_path, e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ApfelError::MCP("Failed to open child stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ApfelError::MCP("Failed to open child stdout".to_string()))?;

        let reader = BufReader::new(stdout);
        let timeout_duration = Duration::from_secs(timeout_secs.max(1));

        let conn = Self {
            command_path: command_path.to_string(),
            tools: Vec::new(),
            stdin: Mutex::new(stdin),
            reader: Mutex::new(reader),
            child: Mutex::new(child),
            next_id: AtomicUsize::new(1),
            timeout_duration,
        };

        // Initialize handshake
        let mut conn = conn;
        conn.handshake().await?;
        Ok(conn)
    }

    async fn handshake(&mut self) -> Result<(), ApfelError> {
        let init_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = MCPProtocol::initialize_request(init_id);
        let resp = self.send_and_receive(&req).await?;
        let _server_info = MCPProtocol::parse_initialize_response(&resp)?;

        // Send initialized notification (fire and forget)
        let notify = MCPProtocol::initialized_notification();
        self.send_only(&notify).await?;

        // List tools
        let list_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let list_req = MCPProtocol::tools_list_request(list_id);
        let list_resp = self.send_and_receive(&list_req).await?;
        self.tools = MCPProtocol::parse_tools_list_response(&list_resp)?;

        Ok(())
    }

    pub fn command_path(&self) -> &str {
        &self.command_path
    }

    pub fn tools(&self) -> &[OpenAITool] {
        &self.tools
    }

    pub async fn call_tool(&self, name: &str, arguments: &str) -> Result<(String, bool), ApfelError> {
        MCPProtocol::validate_tool_arguments(name, arguments)?;
        let call_id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = MCPProtocol::tools_call_request(call_id, name, arguments);
        let resp = self.send_and_receive(&req).await?;
        MCPProtocol::parse_tools_call_response(&resp)
    }

    async fn send_only(&self, line: &str) -> Result<(), ApfelError> {
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(format!("{}\n", line).as_bytes())
            .await
            .map_err(|e| ApfelError::MCP(format!("Failed to write to MCP stdin: {}", e)))?;
        stdin
            .flush()
            .await
            .map_err(|e| ApfelError::MCP(format!("Failed to flush MCP stdin: {}", e)))?;
        Ok(())
    }

    async fn send_and_receive(&self, line: &str) -> Result<String, ApfelError> {
        let mut stdin = self.stdin.lock().await;
        let mut reader = self.reader.lock().await;

        stdin
            .write_all(format!("{}\n", line).as_bytes())
            .await
            .map_err(|e| ApfelError::MCP(format!("Failed to write to MCP server: {}", e)))?;
        stdin
            .flush()
            .await
            .map_err(|e| ApfelError::MCP(format!("Failed to flush to MCP server: {}", e)))?;

        let mut line_buf = String::new();
        let read_future = reader.read_line(&mut line_buf);

        match timeout(self.timeout_duration, read_future).await {
            Ok(Ok(0)) => Err(ApfelError::MCP("MCP server closed connection unexpectedly".to_string())),
            Ok(Ok(_)) => Ok(line_buf.trim().to_string()),
            Ok(Err(e)) => Err(ApfelError::MCP(format!("Read error from MCP server: {}", e))),
            Err(_) => Err(ApfelError::MCP(format!(
                "MCP server timed out after {}s",
                self.timeout_duration.as_secs()
            ))),
        }
    }
}

impl Drop for MCPConnection {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.try_lock() {
            let _ = child.start_kill();
        }
    }
}

#[derive(Clone)]
pub struct MCPManager {
    connections: Vec<Arc<MCPConnection>>,
}

impl MCPManager {
    pub async fn load_servers(server_paths: &[String], timeout_secs: u64) -> Result<Self, ApfelError> {
        let mut conns = Vec::new();
        for path in server_paths {
            let conn = MCPConnection::spawn(path, timeout_secs).await?;
            conns.push(Arc::new(conn));
        }
        Ok(Self { connections: conns })
    }

    pub fn all_tools(&self) -> Vec<OpenAITool> {
        let mut out = Vec::new();
        for conn in &self.connections {
            out.extend(conn.tools().iter().cloned());
        }
        out
    }

    pub async fn call_tool(&self, name: &str, arguments: &str) -> Option<Result<(String, bool), ApfelError>> {
        for conn in &self.connections {
            if conn.tools().iter().any(|t| t.function.name == name) {
                return Some(conn.call_tool(name, arguments).await);
            }
        }
        None
    }
}
