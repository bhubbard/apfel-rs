use apfel::backend::MockEngine;
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
    let chat_resp: ChatCompletionResponse = res.json().await.expect("Failed to deserialize response");
    assert_eq!(chat_resp.choices[0].message.text_content(), "Hello from mock server!");

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
    assert!(models_body["data"].as_array().unwrap().iter().any(|m| m["id"] == "apple-foundationmodel"));

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
}

