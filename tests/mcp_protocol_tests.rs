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
}
