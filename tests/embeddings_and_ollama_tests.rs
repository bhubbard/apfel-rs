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
