use apfel::mcp::protocol::MCPProtocol;

#[test]
fn test_mcp_initialize_and_tools_format() {
    let init_req = MCPProtocol::initialize_request(1);
    assert!(init_req.contains("\"method\":\"initialize\""));
    assert!(init_req.contains("\"id\":1"));

    let list_req = MCPProtocol::tools_list_request(2);
    assert!(list_req.contains("\"method\":\"tools/list\""));

    let call_req = MCPProtocol::tools_call_request(3, "calculator", "{\"expr\": \"1 + 1\"}");
    assert!(call_req.contains("\"method\":\"tools/call\""));
    assert!(call_req.contains("\"name\":\"calculator\""));
}

#[test]
fn test_mcp_parse_tools_list() {
    let resp_json = r#"{
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "tools": [
                {
                    "name": "calc",
                    "description": "Evaluate math",
                    "inputSchema": { "type": "object" }
                }
            ]
        }
    }"#;

    let tools = MCPProtocol::parse_tools_list_response(resp_json).expect("Failed to parse tools list");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].function.name, "calc");
    assert_eq!(tools[0].function.description, Some("Evaluate math".to_string()));

    // Error on missing tools key
    assert!(MCPProtocol::parse_tools_list_response(r#"{"result":{}}"#).is_err());
    // Error on invalid JSON
    assert!(MCPProtocol::parse_tools_list_response("not json").is_err());
}

#[test]
fn test_mcp_initialized_notification() {
    let notif = MCPProtocol::initialized_notification();
    assert!(notif.contains("notifications/initialized"));
}

#[test]
fn test_mcp_validate_tool_arguments() {
    // Empty arguments are allowed
    assert!(MCPProtocol::validate_tool_arguments("tool1", "").is_ok());
    assert!(MCPProtocol::validate_tool_arguments("tool1", "   ").is_ok());

    // Valid JSON
    assert!(MCPProtocol::validate_tool_arguments("tool1", r#"{"param": 42}"#).is_ok());

    // Invalid JSON returns ToolExecution error
    let res = MCPProtocol::validate_tool_arguments("tool1", "not a json string");
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("not valid JSON"));
}

#[test]
fn test_mcp_parse_initialize_response() {
    let init_json = r#"{
        "jsonrpc": "2.0",
        "result": {
            "serverInfo": {
                "name": "sqlite-server",
                "version": "1.2.3"
            }
        }
    }"#;

    let info = MCPProtocol::parse_initialize_response(init_json).expect("Failed to parse init response");
    assert_eq!(info.name, "sqlite-server");
    assert_eq!(info.version, "1.2.3");

    // Missing serverInfo
    assert!(MCPProtocol::parse_initialize_response(r#"{"result":{}}"#).is_err());
    // Invalid JSON
    assert!(MCPProtocol::parse_initialize_response("invalid").is_err());
}

#[test]
fn test_mcp_parse_tools_call_response() {
    // 1. Success response with text content
    let success_json = r#"{
        "jsonrpc": "2.0",
        "result": {
            "content": [
                { "type": "text", "text": "Computed result: 42" }
            ],
            "isError": false
        }
    }"#;
    let (text, is_err) = MCPProtocol::parse_tools_call_response(success_json).expect("Parse failed");
    assert_eq!(text, "Computed result: 42");
    assert!(!is_err);

    // 2. Error in result object with isError: true
    let tool_err_json = r#"{
        "jsonrpc": "2.0",
        "result": {
            "content": [
                { "type": "text", "text": "Division by zero" }
            ],
            "isError": true
        }
    }"#;
    let (err_text, is_err) = MCPProtocol::parse_tools_call_response(tool_err_json).expect("Parse failed");
    assert_eq!(err_text, "Division by zero");
    assert!(is_err);

    // 3. JSON-RPC protocol error
    let protocol_err_json = r#"{
        "jsonrpc": "2.0",
        "error": {
            "code": -32601,
            "message": "Tool not found"
        }
    }"#;
    let (err_msg, is_err) = MCPProtocol::parse_tools_call_response(protocol_err_json).expect("Parse failed");
    assert_eq!(err_msg, "Error: Tool not found");
    assert!(is_err);

    // 4. Missing result
    assert!(MCPProtocol::parse_tools_call_response(r#"{"jsonrpc":"2.0"}"#).is_err());
}
