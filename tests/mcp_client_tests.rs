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
    script_file.write_all(script.as_bytes()).expect("Write failed");
    let script_path = script_file.path().to_str().unwrap().to_string();

    // Spawn MCPManager
    let manager = MCPManager::load_servers(&[format!("python3 {}", script_path)], 5)
        .await
        .expect("Failed to load mock MCP server");

    let tools = manager.all_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].function.name, "greet");
    assert_eq!(tools[0].function.description, Some("Say hello to someone".to_string()));

    // Call tool
    let call_res = manager
        .call_tool("greet", r#"{"name": "Alice"}"#)
        .await
        .expect("Expected tool to be found")
        .expect("Tool execution failed");

    assert_eq!(call_res.0, "Hello, Alice!");
    assert_eq!(call_res.1, false);

    // Call unknown tool returns None
    assert!(manager.call_tool("unknown_tool", "{}").await.is_none());
}
