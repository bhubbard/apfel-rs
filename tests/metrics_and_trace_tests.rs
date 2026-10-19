// ============================================================================
// metrics_and_trace_tests.rs — Comprehensive tests for Prometheus metrics & Trace HUD
// Part of apfel-rs
// ============================================================================

use apfel::backend::MockEngine;
use apfel::cli::{run_cli_with_engine, CliArgs};
use apfel::core::metrics::ApfelMetrics;
use apfel::server::run_server;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn test_prometheus_metrics_registry_unit() {
    let metrics = ApfelMetrics::new();

    // 1. Initial state
    assert_eq!(metrics.prompt_tokens_total.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.generated_tokens_total.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.errors_total.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.active_requests.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.active_streaming_connections.load(std::sync::atomic::Ordering::Relaxed), 0);

    // 2. Active request RAII guard
    let metrics_arc = Arc::new(metrics);
    {
        let guard = metrics_arc.start_request();
        assert_eq!(metrics_arc.active_requests.load(std::sync::atomic::Ordering::Relaxed), 1);
        std::thread::sleep(Duration::from_millis(5));
        assert!(guard.elapsed() >= Duration::from_millis(4));
    }
    assert_eq!(metrics_arc.active_requests.load(std::sync::atomic::Ordering::Relaxed), 0);

    // 3. Active streaming RAII guard
    {
        let _stream_guard = metrics_arc.start_streaming();
        assert_eq!(
            metrics_arc.active_streaming_connections.load(std::sync::atomic::Ordering::Relaxed),
            1
        );
    }
    assert_eq!(
        metrics_arc.active_streaming_connections.load(std::sync::atomic::Ordering::Relaxed),
        0
    );

    // 4. Record requests and errors
    metrics_arc.record_request("/v1/chat/completions", 200, 0.045);
    metrics_arc.record_request("/v1/chat/completions", 200, 0.055);
    metrics_arc.record_request("/v1/chat/completions", 400, 0.010);
    metrics_arc.record_request("/v1/responses", 200, 0.120);

    assert_eq!(metrics_arc.errors_total.load(std::sync::atomic::Ordering::Relaxed), 1);

    // 5. Record TTFT and tokens
    metrics_arc.record_ttft(0.024);
    metrics_arc.record_tokens(128, 64, 38.5);
    metrics_arc.record_cache_hit();
    metrics_arc.record_cache_miss();

    assert_eq!(metrics_arc.prompt_tokens_total.load(std::sync::atomic::Ordering::Relaxed), 128);
    assert_eq!(metrics_arc.generated_tokens_total.load(std::sync::atomic::Ordering::Relaxed), 64);
    assert_eq!(metrics_arc.cache_hits_total.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(metrics_arc.cache_misses_total.load(std::sync::atomic::Ordering::Relaxed), 1);

    // 6. Prometheus text exposition format validation
    let rendered = metrics_arc.render_prometheus();
    assert!(rendered.contains("# HELP apfel_requests_total"));
    assert!(rendered.contains("# TYPE apfel_requests_total counter"));
    assert!(rendered.contains("apfel_requests_total{endpoint=\"/v1/chat/completions\",status=\"200\"} 2"));
    assert!(rendered.contains("apfel_requests_total{endpoint=\"/v1/chat/completions\",status=\"400\"} 1"));
    assert!(rendered.contains("apfel_requests_total{endpoint=\"/v1/responses\",status=\"200\"} 1"));

    assert!(rendered.contains("# HELP apfel_time_to_first_token_seconds"));
    assert!(rendered.contains("# TYPE apfel_time_to_first_token_seconds histogram"));
    assert!(rendered.contains("apfel_time_to_first_token_seconds_bucket{le=\"0.025\"} 1"));
    assert!(rendered.contains("apfel_time_to_first_token_seconds_count 1"));

    assert!(rendered.contains("# HELP apfel_tokens_per_second"));
    assert!(rendered.contains("apfel_tokens_per_second 38.50"));

    assert!(rendered.contains("# HELP apfel_process_resident_memory_bytes"));
    assert!(rendered.contains("# TYPE apfel_process_resident_memory_bytes gauge"));

    assert!(rendered.contains("# HELP apfel_system_thermal_pressure"));
    assert!(rendered.contains("# TYPE apfel_system_thermal_pressure gauge"));
}

#[tokio::test]
async fn test_server_metrics_endpoint_integration() {
    let mock = Arc::new(MockEngine::with_response("Hello from metrics test server!"));
    let port = 9187;
    let host = "127.0.0.1";
    let token = None;
    let allowed_origins = vec!["http://127.0.0.1".to_string(), "http://localhost".to_string()];

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

    // Wait for server to bind
    tokio::time::sleep(Duration::from_millis(150)).await;

    let client = reqwest::Client::new();
    let base_url = format!("http://{}:{}", host, port);

    // 1. Initial GET /metrics
    let res = client
        .get(format!("{}/metrics", base_url))
        .send()
        .await
        .expect("metrics request failed");
    assert_eq!(res.status(), 200);
    assert!(res.headers().get("content-type").unwrap().to_str().unwrap().contains("text/plain"));
    let body = res.text().await.unwrap();
    assert!(body.contains("apfel_requests_total"));
    assert!(body.contains("apfel_active_requests 0"));

    // 2. Perform a non-streaming POST /v1/chat/completions
    let chat_req = serde_json::json!({
        "model": "apple-foundationmodel",
        "messages": [
            {"role": "user", "content": "What is the speed of light?"}
        ]
    });

    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .json(&chat_req)
        .send()
        .await
        .expect("chat completions request failed");
    assert_eq!(res.status(), 200);

    // 3. Perform a streaming POST /v1/chat/completions
    let stream_req = serde_json::json!({
        "model": "apple-foundationmodel",
        "stream": true,
        "messages": [
            {"role": "user", "content": "Stream me a message"}
        ]
    });

    let res = client
        .post(format!("{}/v1/chat/completions", base_url))
        .json(&stream_req)
        .send()
        .await
        .expect("stream request failed");
    assert_eq!(res.status(), 200);
    let _ = res.text().await.unwrap();

    // 4. Perform an embeddings request
    let embed_req = serde_json::json!({
        "model": "apple-foundationmodel",
        "input": "Vectorize this sentence"
    });
    let res = client
        .post(format!("{}/v1/embeddings", base_url))
        .json(&embed_req)
        .send()
        .await
        .expect("embeddings request failed");
    assert_eq!(res.status(), 200);

    // 5. Query /metrics again and verify counters incremented
    let res = client
        .get(format!("{}/metrics", base_url))
        .send()
        .await
        .expect("metrics second request failed");
    assert_eq!(res.status(), 200);
    let updated_metrics = res.text().await.unwrap();

    assert!(updated_metrics.contains("apfel_requests_total{endpoint=\"/v1/chat/completions\",status=\"200\"}"));
    assert!(updated_metrics.contains("apfel_requests_total{endpoint=\"/v1/embeddings\",status=\"200\"}"));
    assert!(updated_metrics.contains("apfel_prompt_tokens_total"));
    assert!(updated_metrics.contains("apfel_generated_tokens_total"));
}

#[tokio::test]
async fn test_cli_trace_hud_execution() {
    let mock = Arc::new(MockEngine::with_response("This is a traced test generation output."));

    let args = CliArgs {
        prompt: Some("Explain quantum computing".to_string()),
        system: None,
        file: vec![],
        image: vec![],
        stream: false,
        no_stream: true,
        chat: false,
        serve: false,
        batch: false,
        port: 8080,
        host: "127.0.0.1".to_string(),
        token: None,
        allowed_origins: vec![],
        footgun: false,
        engine: None,
        model: None,
        adapter: None,
        model_info: false,
        count_tokens: false,
        strict: false,
        schema: None,
        messages: None,
        code: false,
        require_complete: false,
        stop: vec![],
        temperature: None,
        top_p: None,
        max_tokens: None,
        seed: None,
        permissive: false,
        output: "text".to_string(),
        quiet: true,
        no_color: false,
        debug: false,
        context_strategy: "newest-first".to_string(),
        max_turns: None,
        mcp_servers: vec![],
        mcp_config: None,
        benchmark: false,
        trace: true, // Testing --trace flag
        telemetry: None,
        completions: None,
        install_completions: false,
        apfs_sandbox: false,
        guard: false,
        shadow: None,
        adaptive_power: false,
        shm_server: None,
        trap_cmd: None,
        explicit_cli_args: None,
    };

    let exit_code = run_cli_with_engine(args, mock).await;
    assert_eq!(exit_code, 0);
}

#[test]
fn test_metrics_concurrency_stress() {
    let metrics = Arc::new(ApfelMetrics::new());
    let mut handles = Vec::new();

    for _t in 0..8 {
        let m = Arc::clone(&metrics);
        handles.push(std::thread::spawn(move || {
            for i in 0..500 {
                let _req_guard = m.start_request();
                let _stream_guard = m.start_streaming();
                m.record_request("/v1/chat/completions", if i % 20 == 0 { 500 } else { 200 }, 0.025);
                m.record_ttft(0.015);
                m.record_tokens(10, 20, 40.0);
                if i % 2 == 0 {
                    m.record_cache_hit();
                } else {
                    m.record_cache_miss();
                }
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(metrics.active_requests.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.active_streaming_connections.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(metrics.prompt_tokens_total.load(std::sync::atomic::Ordering::Relaxed), 8 * 500 * 10);
    assert_eq!(metrics.generated_tokens_total.load(std::sync::atomic::Ordering::Relaxed), 8 * 500 * 20);
    assert_eq!(metrics.cache_hits_total.load(std::sync::atomic::Ordering::Relaxed), 8 * 250);
    assert_eq!(metrics.cache_misses_total.load(std::sync::atomic::Ordering::Relaxed), 8 * 250);
    assert_eq!(metrics.errors_total.load(std::sync::atomic::Ordering::Relaxed), 8 * 25);

    let rendered = metrics.render_prometheus();
    assert!(rendered.contains("apfel_prompt_tokens_total 40000"));
    assert!(rendered.contains("apfel_generated_tokens_total 80000"));
    assert!(rendered.contains("apfel_errors_total 200"));
}
