// ============================================================================
// tests/mcp_client_tests.rs — Subprocess integration tests for MCPManager
// ============================================================================

use apfel::mcp::client::MCPManager;
use std::io::Write;
use tempfile::NamedTempFile;

#[tokio::test]
async fn test_mcp_manager_subprocess_lifecycle() {
    // Create a mock MCP server in Python
    let mut script_file = NamedTempFile::new().expect("Failed to create temp file");
    let script = r#"
import sys, json

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    method = req.get("method")
    req_id = req.get("id")
    
    if method == "initialize":
        resp = {
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "serverInfo": {"name": "test-mcp", "version": "0.1.0"},
                "capabilities": {}
            }
        }
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        resp = {
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "tools": [
                    {
                        "name": "greet",
                        "description": "Say hello to someone",
                        "inputSchema": {"type": "object"}
                    }
                ]
            }
        }
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
    elif method == "tools/call":
        args = req.get("params", {}).get("arguments", {})
        name = args.get("name", "World")
        resp = {
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "content": [{"type": "text", "text": f"Hello, {name}!"}],
                "isError": False
            }
        }
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
"#;
    script_file
        .write_all(script.as_bytes())
        .expect("Write failed");
    let script_path = script_file.path().to_str().unwrap().to_string();

    // Spawn MCPManager
    let manager = MCPManager::load_servers(&[format!("python3 {}", script_path)], 5)
        .await
        .expect("Failed to load mock MCP server");

    let tools = manager.all_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].function.name, "greet");
    assert_eq!(
        tools[0].function.description,
        Some("Say hello to someone".to_string())
    );

    // Call tool
    let call_res = manager
        .call_tool("greet", r#"{"name": "Alice"}"#)
        .await
        .expect("Expected tool to be found")
        .expect("Tool execution failed");

    assert_eq!(call_res.0, "Hello, Alice!");
    assert!(!call_res.1);

    // Call unknown tool returns None
    assert!(manager.call_tool("unknown_tool", "{}").await.is_none());
}

#[test]
fn test_mcp_config_parsing() {
    use apfel::mcp::protocol::McpConfig;

    // Format 1: mcpServers
    let cfg1_json = r#"{
        "mcpServers": {
            "fetch": {
                "command": "uvx",
                "args": ["mcp-server-fetch"],
                "env": { "DEBUG": "1" }
            }
        }
    }"#;
    let cfg1 = McpConfig::parse(cfg1_json).unwrap();
    let servers1 = cfg1.all_servers();
    assert_eq!(servers1.len(), 1);
    assert_eq!(servers1["fetch"].command, "uvx");
    assert_eq!(servers1["fetch"].args, vec!["mcp-server-fetch"]);
    assert_eq!(
        servers1["fetch"].env.get("DEBUG").map(|s| s.as_str()),
        Some("1")
    );

    // Format 2: servers
    let cfg2_json = r#"{
        "servers": {
            "sqlite": {
                "command": "uvx",
                "args": ["mcp-server-sqlite", "--db", "test.db"]
            }
        }
    }"#;
    let cfg2 = McpConfig::parse(cfg2_json).unwrap();
    let servers2 = cfg2.all_servers();
    assert_eq!(servers2.len(), 1);
    assert_eq!(servers2["sqlite"].args.len(), 3);

    // Format 3: direct dictionary
    let cfg3_json = r#"{
        "direct_server": {
            "command": "python3",
            "args": ["server.py"]
        }
    }"#;
    let cfg3 = McpConfig::parse(cfg3_json).unwrap();
    let servers3 = cfg3.all_servers();
    assert_eq!(servers3.len(), 1);
    assert_eq!(servers3["direct_server"].command, "python3");
}

#[tokio::test]
async fn test_mcp_manager_load_config_file() {
    let mut script_file = NamedTempFile::new().expect("Failed to create temp file");
    let script = r#"
import sys, json

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    method = req.get("method")
    req_id = req.get("id")
    
    if method == "initialize":
        resp = {
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "serverInfo": {"name": "config-test-mcp", "version": "0.1.0"},
                "capabilities": {}
            }
        }
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
    elif method == "notifications/initialized":
        pass
    elif method == "tools/list":
        resp = {
            "jsonrpc": "2.0",
            "id": req_id,
            "result": {
                "tools": [
                    {
                        "name": "calc_sum",
                        "description": "Calculate sum",
                        "inputSchema": {"type": "object"}
                    }
                ]
            }
        }
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
"#;
    script_file
        .write_all(script.as_bytes())
        .expect("Write failed");
    let script_path = script_file.path().to_str().unwrap().to_string();

    let mut config_file = NamedTempFile::new().expect("Failed to create config file");
    let config_json = format!(
        r#"{{
            "mcpServers": {{
                "math_server": {{
                    "command": "python3",
                    "args": ["{}"]
                }}
            }}
        }}"#,
        script_path
    );
    config_file
        .write_all(config_json.as_bytes())
        .expect("Write config failed");

    let manager = MCPManager::load_config_file(config_file.path(), 5)
        .await
        .expect("Failed to load MCP config file");

    let tools = manager.all_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].function.name, "calc_sum");
}
