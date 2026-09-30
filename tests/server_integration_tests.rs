use apfel::backend::{BackendEngine, MockEngine};
use apfel::core::models::ChatCompletionResponse;
use apfel::server::run_server;
use std::sync::Arc;

#[tokio::test]
async fn test_server_routes_and_security() {
    let mock = Arc::new(MockEngine::with_response("Hello from mock server!"));
    let port = 9123;
    let host = "127.0.0.1";
    let token = Some("test-token-123".to_string());
    let allowed_origins = vec!["http://localhost".to_string()];

    // Run server in background
    let engine_clone = mock.clone();
    tokio::spawn(async move {
        let _ = run_server(
            host,
            port,
            token,
            allowed_origins,
            false,
            engine_clone,
            None,
        )
        .await;
    });

    // Wait a brief moment for server to bind
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let base_url = format!("http://{}:{}", host, port);

    // 1. Health check with valid auth
    let res = client
        .get(format!("{}/health", base_url))
        .header("Authorization", "Bearer test-token-123")
        .send()
        .await
        .expect("Health request failed");
    assert_eq!(res.status(), 200);
    let health_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(health_json["context_window_measured"], true);

    // 2. Health check with missing auth -> 401 Unauthorized
    let res = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .expect("Unauthorized request failed");
    assert_eq!(res.status(), 401);

    // 3. Health check with forbidden origin -> 403 Forbidden
    let res = client
        .get(format!("{}/health", base_url))
        .header("Authorization", "Bearer test-token-123")
        .header("Origin", "http://evil-attacker.com")
        .send()
        .await
        .expect("Forbidden origin request failed");
    assert_eq!(res.status(), 403);

    // 4. Allowed origin -> 200 OK
    let res = client
        .get(format!("{}/health", base_url))
        .header("Authorization", "Bearer test-token-123")
        .header("Origin", "http://localhost:3000")
        .send()
        .await
        .expect("Allowed origin request failed");
    assert_eq!(res.status(), 200);

    // 5. Chat completion POST
    let body = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [
            { "role": "user", "content": "Hi there" }
        ]
    });

    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&body)
        .send()
        .await
        .expect("Chat completion failed");

    assert_eq!(res.status(), 200);
    let chat_resp: ChatCompletionResponse =
        res.json().await.expect("Failed to deserialize response");
    assert_eq!(
        chat_resp.choices[0].message.text_content(),
        "Hello from mock server!"
    );

    // 6. Stop sequence in non-streaming POST
    let stop_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [{ "role": "user", "content": "Hi" }],
        "stop": ["from"]
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&stop_body)
        .send()
        .await
        .expect("Stop sequence request failed");
    assert_eq!(res.status(), 200);
    let stop_resp: ChatCompletionResponse = res.json().await.unwrap();
    assert_eq!(stop_resp.choices[0].message.text_content(), "Hello ");
    assert_eq!(stop_resp.choices[0].finish_reason, "stop");

    // 7. Streaming chat completion POST
    let stream_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [{ "role": "user", "content": "Stream please" }],
        "stream": true
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&stream_body)
        .send()
        .await
        .expect("Streaming request failed");
    assert_eq!(res.status(), 200);
    let stream_text = res.text().await.unwrap();
    assert!(stream_text.contains("chat.completion.chunk"));
    assert!(stream_text.contains("[DONE]"));

    // 8. Conflicting tokens -> 400 Bad Request
    let conflict_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [{ "role": "user", "content": "Conflict test" }],
        "max_tokens": 50,
        "max_completion_tokens": 100
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&conflict_body)
        .send()
        .await
        .expect("Conflict request failed");
    assert_eq!(res.status(), 400);

    // 9. List models GET /v1/models
    let res = client
        .get(format!("{}/v1/models", base_url))
        .header("Authorization", "Bearer test-token-123")
        .send()
        .await
        .expect("List models failed");
    assert_eq!(res.status(), 200);
    let models_body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(models_body["object"], "list");
    let models_data = models_body["data"].as_array().unwrap();
    for m in models_data {
        assert!(m["context_window"].is_number());
        assert!(m["context_window_measured"].is_boolean());
    }
    let apple_model = models_data
        .iter()
        .find(|m| m["id"] == "apple-foundationmodel")
        .unwrap();
    assert_eq!(apple_model["context_window"], 4096);
    assert_eq!(apple_model["context_window_measured"], true);

    // 10. OpenAI responses endpoint POST /v1/responses
    let responses_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "input": "Tell me a joke"
    });
    let res = client
        .post(format!("{}/v1/responses", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&responses_body)
        .send()
        .await
        .expect("Responses endpoint failed");
    assert_eq!(res.status(), 200);
    let resp_obj: serde_json::Value = res.json().await.unwrap();
    assert_eq!(resp_obj["object"], "response");
    assert_eq!(resp_obj["status"], "completed");

    // 11. Response format JSON fence stripping
    let json_format_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [{ "role": "user", "content": "Return json" }],
        "response_format": { "type": "json_object" }
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&json_format_body)
        .send()
        .await
        .expect("Response format request failed");
    assert_eq!(res.status(), 200);

    // 12. Responses endpoint with background=true -> 501 Not Implemented
    let bg_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "input": "Async work",
        "background": true
    });
    let res = client
        .post(format!("{}/v1/responses", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&bg_body)
        .send()
        .await
        .expect("Background request failed");
    assert_eq!(res.status(), 501);

    // 13. Responses endpoint with previous_response_id -> 501 Not Implemented
    let prev_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "input": "Continuation",
        "previous_response_id": "resp_abc"
    });
    let res = client
        .post(format!("{}/v1/responses", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&prev_body)
        .send()
        .await
        .expect("Previous response id request failed");
    assert_eq!(res.status(), 501);

    // 14. Responses endpoint with structured items array input
    let items_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "instructions": "Be brief",
        "input": [
            { "role": "user", "content": "Hello in items format" }
        ]
    });
    let res = client
        .post(format!("{}/v1/responses", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&items_body)
        .send()
        .await
        .expect("Items input request failed");
    assert_eq!(res.status(), 200);

    // 15. Responses endpoint with null input
    let null_input_body = serde_json::json!({
        "model": "apple-foundationmodel"
    });
    let res = client
        .post(format!("{}/v1/responses", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&null_input_body)
        .send()
        .await
        .expect("Null input request failed");
    assert_eq!(res.status(), 200);

    // 16. Chat completions with tool_choice = "none"
    let no_tool_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [{ "role": "user", "content": "No tools" }],
        "tools": [{
            "type": "function",
            "function": { "name": "dummy", "description": "dummy tool" }
        }],
        "tool_choice": "none"
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&no_tool_body)
        .send()
        .await
        .expect("No tool request failed");
    assert_eq!(res.status(), 200);

    // 17. Streaming chat completion with stop sequence
    let stream_stop_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [{ "role": "user", "content": "Stream with stop" }],
        "stream": true,
        "stop": ["from"]
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&stream_stop_body)
        .send()
        .await
        .expect("Streaming stop request failed");
    assert_eq!(res.status(), 200);
    let stream_stop_text = res.text().await.unwrap();
    assert!(stream_stop_text.contains("chat.completion.chunk"));
    assert!(stream_stop_text.contains("[DONE]"));

    // 18. OpenAI SDK Responses input_tokens preflight (Arthur-Ficial #485)
    let preflight_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "instructions": "You are a concise assistant.",
        "input": "Calculate 15 * 3",
        "tools": [
            {
                "type": "function",
                "name": "calculator",
                "description": "Performs mathematical arithmetic",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "expr": { "type": "string" }
                    }
                }
            }
        ]
    });
    let res = client
        .post(format!("{}/v1/responses/input_tokens", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&preflight_body)
        .send()
        .await
        .expect("Input tokens preflight request failed");
    assert_eq!(res.status(), 200);
    let preflight_resp: serde_json::Value = res.json().await.unwrap();
    assert_eq!(preflight_resp["object"], "response.input_tokens");
    let tokens = preflight_resp["input_tokens"].as_u64().unwrap();
    assert!(tokens > 0);

    // 19. Responses input_tokens with messages array input
    let preflight_msgs_body = serde_json::json!({
        "model": "apple-foundationmodel",
        "input": [
            { "role": "user", "content": "How many tokens are here?" }
        ]
    });
    let res = client
        .post(format!("{}/v1/responses/input_tokens", base_url))
        .header("Authorization", "Bearer test-token-123")
        .json(&preflight_msgs_body)
        .send()
        .await
        .expect("Input tokens messages preflight failed");
    assert_eq!(res.status(), 200);
    let msgs_resp: serde_json::Value = res.json().await.unwrap();
    assert_eq!(msgs_resp["object"], "response.input_tokens");
    assert_eq!(
        msgs_resp["input_tokens"],
        mock.count_tokens("How many tokens are here?") as u64
    );
}

#[tokio::test]
async fn test_context_window_measured_disclosure() {
    let mut mock = MockEngine::new();
    mock.context_window = 4096;
    mock.context_window_measured = false;
    let mock = Arc::new(mock);

    let port = 9125;
    let host = "127.0.0.1";
    let token = None;
    let allowed_origins = vec!["*".to_string()];

    let engine_clone = mock.clone();
    tokio::spawn(async move {
        let _ = run_server(host, port, token, allowed_origins, true, engine_clone, None).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let client = reqwest::Client::new();
    let base_url = format!("http://{}:{}", host, port);

    // Health check returns context_window_measured: false
    let res = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .expect("Health request failed");
    assert_eq!(res.status(), 200);
    let health_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(health_json["context_size"], 4096);
    assert_eq!(health_json["context_window_measured"], false);

    // Models endpoint returns context_window_measured: false for cold-start model
    let res = client
        .get(format!("{}/v1/models", base_url))
        .send()
        .await
        .expect("List models failed");
    assert_eq!(res.status(), 200);
    let models_body: serde_json::Value = res.json().await.unwrap();
    let models_data = models_body["data"].as_array().unwrap();
    let apple_model = models_data
        .iter()
        .find(|m| m["id"] == "apple-foundationmodel")
        .unwrap();
    assert_eq!(apple_model["context_window"], 4096);
    assert_eq!(apple_model["context_window_measured"], false);
}

#[tokio::test]
async fn test_adapter_exposed_in_models_endpoint() {
    let mock = Arc::new(MockEngine::with_adapter("/path/to/sentiment.fmadapter"));
    let port = 9126;
    let host = "127.0.0.1";
    let token = None;
    let allowed_origins = vec![
        "http://localhost".to_string(),
        "http://127.0.0.1".to_string(),
    ];

    let engine_clone = mock.clone();
    tokio::spawn(async move {
        let _ = run_server(host, port, token, allowed_origins, true, engine_clone, None).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{}:{}", host, port);

    let res = client
        .get(format!("{}/v1/models", base_url))
        .send()
        .await
        .expect("List models failed");
    assert_eq!(res.status(), 200);
    let models_body: serde_json::Value = res.json().await.unwrap();
    let models_data = models_body["data"].as_array().unwrap();

    let adapter_entry = models_data
        .iter()
        .find(|m| m["id"] == "apple-foundationmodel:adapter-sentiment");
    assert!(
        adapter_entry.is_some(),
        "Adapter model entry should be present in /v1/models"
    );
    let entry = adapter_entry.unwrap();
    assert_eq!(entry["owned_by"], "user-adapter");

    // Also assert that apple-content-tagging is listed
    let tagging_entry = models_data
        .iter()
        .find(|m| m["id"] == "apple-content-tagging");
    assert!(
        tagging_entry.is_some(),
        "apple-content-tagging model should be present in /v1/models"
    );
}

#[tokio::test]
async fn test_content_tagging_routing_and_use_case() {
    let mock = Arc::new(MockEngine::with_response("classified: billing"));
    let port = 9127;
    let host = "127.0.0.1";
    let token = None;
    let allowed_origins = vec!["*".to_string()];

    let engine_clone = mock.clone();
    tokio::spawn(async move {
        let _ = run_server(host, port, token, allowed_origins, true, engine_clone, None).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{}:{}", host, port);

    // 1. Non-streaming with model "apple-content-tagging"
    let tag_req = serde_json::json!({
        "model": "apple-content-tagging",
        "messages": [
            { "role": "user", "content": "Categorize this support ticket" }
        ]
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .json(&tag_req)
        .send()
        .await
        .expect("Chat completion failed");
    assert_eq!(res.status(), 200);
    let last = mock
        .last_request()
        .expect("Mock should have received a request");
    assert_eq!(last.use_case, Some("content_tagging".to_string()));

    // 2. Non-streaming with standard model -> use_case is None
    let std_req = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [
            { "role": "user", "content": "General text generation" }
        ]
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .json(&std_req)
        .send()
        .await
        .expect("Chat completion failed");
    assert_eq!(res.status(), 200);
    let last = mock
        .last_request()
        .expect("Mock should have received a request");
    assert_eq!(last.use_case, None);

    // 3. Streaming with model matching "classif" -> use_case is Some("content_tagging")
    let stream_req = serde_json::json!({
        "model": "support-classifier-v1",
        "stream": true,
        "messages": [
            { "role": "user", "content": "Ticket classification" }
        ]
    });
    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .json(&stream_req)
        .send()
        .await
        .expect("Streaming chat completion failed");
    assert_eq!(res.status(), 200);
    let last = mock
        .last_request()
        .expect("Mock should have received a request");
    assert_eq!(last.use_case, Some("content_tagging".to_string()));
}

#[tokio::test]
async fn test_embeddings_and_ollama_endpoints() {
    let mock = Arc::new(MockEngine::with_response("Ollama response from mock"));
    let port = 9128;
    let host = "127.0.0.1";
    let token = None;
    let allowed_origins = vec!["*".to_string()];

    let engine_clone = mock.clone();
    tokio::spawn(async move {
        let _ = run_server(host, port, token, allowed_origins, true, engine_clone, None).await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{}:{}", host, port);

    // 1. POST /v1/embeddings
    let emb_req = serde_json::json!({
        "model": "text-embedding-3-small",
        "input": ["Apple Foundation Models", "On-device intelligence"]
    });
    let res = client
        .post(format!("{}/v1/embeddings", base_url))
        .json(&emb_req)
        .send()
        .await
        .expect("Embeddings request failed");
    assert_eq!(res.status(), 200);
    let emb_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(emb_json["object"], "list");
    let data = emb_json["data"].as_array().expect("data array");
    assert_eq!(data.len(), 2);
    assert_eq!(data[0]["index"], 0);
    assert_eq!(data[1]["index"], 1);
    let emb0 = data[0]["embedding"].as_array().unwrap();
    assert_eq!(emb0.len(), 384);
    assert!(emb_json["usage"]["prompt_tokens"].as_u64().unwrap() > 0);

    // 2. GET /api/tags (Ollama models list)
    let res = client
        .get(format!("{}/api/tags", base_url))
        .send()
        .await
        .expect("Ollama tags request failed");
    assert_eq!(res.status(), 200);
    let tags_json: serde_json::Value = res.json().await.unwrap();
    let models = tags_json["models"].as_array().expect("models array");
    assert!(models.iter().any(|m| m["name"] == "apple-intelligence"));

    // 3. POST /api/generate (Ollama generate)
    let ollama_gen = serde_json::json!({
        "model": "apple-intelligence",
        "prompt": "What is the capital of France?"
    });
    let res = client
        .post(format!("{}/api/generate", base_url))
        .json(&ollama_gen)
        .send()
        .await
        .expect("Ollama generate failed");
    assert_eq!(res.status(), 200);
    let gen_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(gen_json["model"], "apple-intelligence");
    assert_eq!(gen_json["done"], true);
    assert_eq!(gen_json["response"], "Ollama response from mock");

    // 4. POST /api/chat (Ollama chat)
    let ollama_chat = serde_json::json!({
        "model": "apple-intelligence",
        "messages": [
            { "role": "user", "content": "Hello Ollama" }
        ]
    });
    let res = client
        .post(format!("{}/api/chat", base_url))
        .json(&ollama_chat)
        .send()
        .await
        .expect("Ollama chat failed");
    assert_eq!(res.status(), 200);
    let chat_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(chat_json["model"], "apple-intelligence");
    assert_eq!(chat_json["done"], true);
    assert_eq!(chat_json["message"]["content"], "Ollama response from mock");
}
