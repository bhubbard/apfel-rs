use apfel::backend::engine::{BackendEngine, GenerateRequest};
use apfel::backend::mlx_engine::MlxBackendEngine;
use apfel::core::models::{EmbeddingInput, EmbeddingRequest};

#[test]
fn test_mlx_backend_engine_generation() {
    let engine = MlxBackendEngine::new("test-model");
    let req = GenerateRequest {
        prompt: "What is 2 + 2?".into(),
        system_prompt: None,
        messages: None,
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(32),
        permissive: true,
        seed: None,
        use_case: None,
    };
    let res = engine.generate(&req).expect("Generation should succeed");
    assert_eq!(res.content, "4");
    assert_eq!(res.finish_reason, "stop");
}

#[test]
fn test_embedding_input_conversions() {
    let single = EmbeddingInput::String("hello world".into());
    assert_eq!(single.to_vec(), vec!["hello world".to_string()]);

    let multiple = EmbeddingInput::Array(vec!["query 1".into(), "query 2".into()]);
    assert_eq!(
        multiple.to_vec(),
        vec!["query 1".to_string(), "query 2".to_string()]
    );

    let req = EmbeddingRequest {
        model: "mlx-embedding".into(),
        input: multiple,
        user: None,
    };
    assert_eq!(req.input.to_vec().len(), 2);
}

#[test]
fn test_generate_request_defaults_and_serde() {
    let def = GenerateRequest::default();
    assert_eq!(def.prompt, "");
    assert_eq!(def.system_prompt, None);
    assert_eq!(def.messages, None);
    assert_eq!(def.temperature, None);
    assert_eq!(def.top_p, None);
    assert_eq!(def.max_tokens, None);
    assert!(!def.permissive);
    assert_eq!(def.seed, None);
    assert_eq!(def.use_case, None);

    // Serialization with use_case
    let custom = GenerateRequest {
        prompt: "Classify this".into(),
        system_prompt: Some("You are a classifier".into()),
        messages: None,
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(4),
        permissive: true,
        seed: Some(42),
        use_case: Some("content_tagging".into()),
    };
    let json = serde_json::to_string(&custom).unwrap();
    assert!(json.contains("\"use_case\":\"content_tagging\""));
    assert!(json.contains("\"temperature\":0.0"));
    assert!(json.contains("\"max_tokens\":4"));

    // Deserialization with use_case
    let deserialized: GenerateRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.use_case, Some("content_tagging".to_string()));
    assert_eq!(deserialized.prompt, "Classify this");

    // Deserialization without use_case defaults to None
    let json_no_uc = r#"{"prompt":"Hello","permissive":false}"#;
    let req_no_uc: GenerateRequest = serde_json::from_str(json_no_uc).unwrap();
    assert_eq!(req_no_uc.use_case, None);
}

#[test]
fn test_backend_engine_trait_defaults() {
    let engine = apfel::backend::MockEngine::new();
    assert!(engine.context_window_measured());
    assert_eq!(engine.model_name(), "apple-foundationmodel");
    assert_eq!(engine.framework_name(), "FoundationModels (macOS 26+)");
}

#[tokio::test]
async fn test_ollama_version_show_and_streaming() {
    let mock = std::sync::Arc::new(apfel::backend::MockEngine::with_response(
        "Hello from mock server!",
    ));
    let port = 9131;
    let host = "127.0.0.1";
    let token = None;
    let allowed_origins = vec!["*".to_string()];

    let engine_clone = mock.clone();
    tokio::spawn(async move {
        let _ =
            apfel::server::run_server(host, port, token, allowed_origins, true, engine_clone, None)
                .await;
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let client = reqwest::Client::new();
    let base_url = format!("http://{}:{}", host, port);

    // 1. GET /api/version
    let res = client
        .get(format!("{}/api/version", base_url))
        .send()
        .await
        .expect("Ollama version request failed");
    assert_eq!(res.status(), 200);
    assert!(res
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("application/json"));
    let version_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(version_json["version"], "0.1.4");

    // 2. POST /api/show with name
    let show_by_name = serde_json::json!({
        "name": "apple-intelligence"
    });
    let res = client
        .post(format!("{}/api/show", base_url))
        .json(&show_by_name)
        .send()
        .await
        .expect("Ollama show request by name failed");
    assert_eq!(res.status(), 200);
    let show_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(show_json["parameters"], "context_length 8192");
    assert!(show_json["license"]
        .as_str()
        .unwrap()
        .contains("Apple Intelligence"));
    assert_eq!(show_json["details"]["family"], "foundation");
    assert_eq!(show_json["details"]["parameter_size"], "3B");

    // 3. POST /api/show with model
    let show_by_model = serde_json::json!({
        "model": "apple-intelligence"
    });
    let res = client
        .post(format!("{}/api/show", base_url))
        .json(&show_by_model)
        .send()
        .await
        .expect("Ollama show request by model failed");
    assert_eq!(res.status(), 200);
    let show_json: serde_json::Value = res.json().await.unwrap();
    assert_eq!(show_json["details"]["format"], "apple-intelligence");

    // 4. GET /api/ps
    let res = client
        .get(format!("{}/api/ps", base_url))
        .send()
        .await
        .expect("Ollama ps request failed");
    assert_eq!(res.status(), 200);
    let ps_json: serde_json::Value = res.json().await.unwrap();
    let ps_models = ps_json["models"]
        .as_array()
        .expect("models array in /api/ps");
    assert!(ps_models.iter().any(|m| m["name"] == "apple-intelligence"));

    // 5. POST /api/chat streaming NDJSON (default stream = None)
    let chat_req = serde_json::json!({
        "model": "apple-intelligence",
        "messages": [
            { "role": "user", "content": "Hello Ollama streaming" }
        ]
    });
    let res = client
        .post(format!("{}/api/chat", base_url))
        .json(&chat_req)
        .send()
        .await
        .expect("Ollama chat streaming request failed");
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("content-type").unwrap().to_str().unwrap(),
        "application/x-ndjson"
    );
    let text = res.text().await.unwrap();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        lines.len() >= 2,
        "Expected multiple NDJSON lines, got: {:?}",
        lines
    );

    // Delta lines
    let first_line: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first_line["model"], "apple-intelligence");
    assert_eq!(first_line["done"], false);
    assert!(first_line["message"]["content"].is_string());

    // Final line
    let last_line: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    assert_eq!(last_line["model"], "apple-intelligence");
    assert_eq!(last_line["done"], true);
    assert_eq!(last_line["done_reason"], "stop");

    // 6. POST /api/generate streaming NDJSON (explicit stream = true)
    let gen_req = serde_json::json!({
        "model": "apple-intelligence",
        "prompt": "Write a short poem",
        "stream": true
    });
    let res = client
        .post(format!("{}/api/generate", base_url))
        .json(&gen_req)
        .send()
        .await
        .expect("Ollama generate streaming request failed");
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("content-type").unwrap().to_str().unwrap(),
        "application/x-ndjson"
    );
    let text = res.text().await.unwrap();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(
        lines.len() >= 2,
        "Expected multiple NDJSON lines, got: {:?}",
        lines
    );

    let first_gen_line: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(first_gen_line["model"], "apple-intelligence");
    assert_eq!(first_gen_line["done"], false);
    assert!(first_gen_line["response"].is_string());

    let last_gen_line: serde_json::Value = serde_json::from_str(lines.last().unwrap()).unwrap();
    assert_eq!(last_gen_line["model"], "apple-intelligence");
    assert_eq!(last_gen_line["done"], true);
    assert_eq!(last_gen_line["done_reason"], "stop");
}
