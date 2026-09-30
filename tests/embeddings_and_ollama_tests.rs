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
    assert_eq!(def.permissive, false);
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
