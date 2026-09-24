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
}
