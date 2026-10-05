// ============================================================================
// breakthrough_features_tests.rs — End-to-end tests for apfel-rs breakthrough features
// Part of apfel-rs
// ============================================================================

use apfel::core::apfs::ApfsSandbox;
use apfel::core::fault_trap::{FaultCategory, TrappedFault};
use apfel::core::guard::ApfelGuard;
use apfel::core::shadow_compiler::ShadowCompiler;
use apfel::core::shm::ShmBuffer;
use apfel::core::thermal::{ComputeProfile, PowerSource, PowerThermalMonitor, ThermalState};
use std::fs;
use std::path::PathBuf;

#[test]
fn test_breakthrough_1_apfs_sandbox_rollback() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("mission_critical.rs");
    fs::write(&file_path, "fn untouched_source() -> bool { true }").unwrap();

    {
        let mut sandbox = ApfsSandbox::begin(temp_dir.path()).unwrap();
        // Mutate the file during speculative execution
        fs::write(&file_path, "fn corrupted_source() { panic!(); }").unwrap();
        assert_eq!(
            fs::read_to_string(&file_path).unwrap(),
            "fn corrupted_source() { panic!(); }"
        );
        // Explicit rollback or drop without commit
        sandbox.rollback().unwrap();
    }

    // Verify workspace restored to pristine original state
    let restored = fs::read_to_string(&file_path).unwrap();
    assert_eq!(restored, "fn untouched_source() -> bool { true }");
}

#[test]
fn test_breakthrough_1_apfs_sandbox_commit() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("code.rs");
    fs::write(&file_path, "fn version_1() {}").unwrap();

    {
        let mut sandbox = ApfsSandbox::begin(temp_dir.path()).unwrap();
        fs::write(&file_path, "fn version_2_promoted() {}").unwrap();
        sandbox.commit();
    }

    assert_eq!(
        fs::read_to_string(&file_path).unwrap(),
        "fn version_2_promoted() {}"
    );
}

#[test]
fn test_breakthrough_2_fault_trap_rustc_healing() {
    let stderr = r#"
error[E0502]: cannot borrow `data` as mutable because it is also borrowed as immutable
  --> src/pipeline.rs:88:13
   |
87 |     let ref_data = &data;
   |                    ----- immutable borrow occurs here
88 |     data.push(42);
   |     ^^^^^^^^^^^^^ mutable borrow occurs here
89 |     println!("{:?}", ref_data);
   |                      -------- immutable borrow later used here
"#;

    let fault = TrappedFault::parse_diagnostic(stderr, 1).unwrap();
    match &fault.category {
        FaultCategory::RustcCompilerError { code } => assert_eq!(code, "E0502"),
        _ => panic!("Expected RustcCompilerError"),
    }

    assert_eq!(fault.target_file, Some(PathBuf::from("src/pipeline.rs")));
    assert_eq!(fault.line_number, Some(88));
    assert_eq!(fault.column_number, Some(13));

    let prompt = fault.to_micro_agent_prompt();
    assert!(prompt.contains("E0502"));
    assert!(prompt.contains("pipeline.rs"));
    assert!(prompt.contains("DIAGNOSTIC:"));
    assert!(prompt.contains("ERROR OUTPUT:"));
}

#[test]
fn test_breakthrough_2_fault_trap_typescript() {
    let stderr = "src/api/auth.ts(45,12): error TS2339: Property 'userToken' does not exist on type 'Session'.";
    let fault = TrappedFault::parse_diagnostic(stderr, 1).unwrap();
    match &fault.category {
        FaultCategory::TypeScriptError { code } => assert_eq!(code, "TS2339"),
        _ => panic!("Expected TypeScriptError"),
    }
    assert_eq!(fault.target_file, Some(PathBuf::from("src/api/auth.ts")));
    assert_eq!(fault.line_number, Some(45));
    assert_eq!(fault.column_number, Some(12));
}

#[test]
fn test_breakthrough_3_apple_silicon_shm_zero_copy() {
    let shm_name = format!("/apfel_test_{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    let writer = ShmBuffer::create(&shm_name, 4096).unwrap();

    let payload = b"{\"model\": \"apple-foundationmodel\", \"query\": \"explain zero-copy memory\"}";
    let seq = writer.write_payload(payload).unwrap();
    assert_eq!(seq, 1);
    assert_eq!(writer.sequence(), 1);

    // Open reader attached to same shared memory segment
    let reader = ShmBuffer::open(&shm_name).unwrap();
    assert_eq!(reader.sequence(), 1);
    assert_eq!(reader.read_payload(), payload);

    // Write second message
    let payload_v2 = b"{\"status\": \"complete\", \"latency_us\": 3}";
    let seq2 = writer.write_payload(payload_v2).unwrap();
    assert_eq!(seq2, 2);
    assert_eq!(reader.sequence(), 2);
    assert_eq!(reader.read_payload(), payload_v2);
}

#[test]
fn test_breakthrough_4_apfel_guard_anonymization_and_deanonymization() {
    let guard = ApfelGuard::new();
    let prompt = "Deploy using sk-live-99238472938472938472 and contact admin@internal.apple.com with SSN 000-12-3456";

    let (sanitized, count) = guard.sanitize(prompt);
    assert_eq!(count, 3);
    assert!(!sanitized.contains("sk-live-99238472938472938472"));
    assert!(!sanitized.contains("admin@internal.apple.com"));
    assert!(!sanitized.contains("000-12-3456"));
    assert!(sanitized.contains("__APFEL_VAULT_"));

    // Simulate Cloud AI response that echoes back the placeholder tokens
    let cloud_response = format!(
        "Confirmed. Authenticated with credentials and routed alert regarding {}.",
        sanitized
    );

    let restored = guard.de_anonymize(&cloud_response);
    assert!(restored.contains("sk-live-99238472938472938472"));
    assert!(restored.contains("admin@internal.apple.com"));
    assert!(restored.contains("000-12-3456"));
    assert!(!restored.contains("__APFEL_VAULT_"));
}

#[tokio::test]
async fn test_breakthrough_5_speculative_shadow_compiler() {
    let shadow = ShadowCompiler::new();
    // Simulate compilation command emitting an error
    let cmd = "echo 'error[E0382]: use of moved value: `buffer`\n  --> src/net.rs:25:9\n' >&2; echo '' >&2";

    let exit_code = shadow.execute_and_shadow(cmd, None).await.unwrap();
    assert_eq!(exit_code, 0);

    let faults = shadow.get_faults().await;
    assert_eq!(faults.len(), 1);

    let fixes = shadow.get_fixes().await;
    assert_eq!(fixes.len(), 1);
    assert!(fixes[0].candidate_patch.as_ref().unwrap().contains("E0382"));
}

#[test]
fn test_breakthrough_6_thermal_and_battery_adaptive_scheduling() {
    // Test profile decision matrices
    let eco_battery = PowerSource::Battery { percent: 18 };
    let profile_eco = PowerThermalMonitor::resolve_profile(&eco_battery, &ThermalState::Nominal);
    assert_eq!(profile_eco, ComputeProfile::EcoPowerSaver);
    assert_eq!(profile_eco.context_token_ceiling(), 2048);
    assert_eq!(profile_eco.enable_speculative_decoding(), false);
    assert_eq!(profile_eco.worker_threads(), 2);

    let perf_ac = PowerSource::AcPower;
    let profile_perf = PowerThermalMonitor::resolve_profile(&perf_ac, &ThermalState::Nominal);
    assert_eq!(profile_perf, ComputeProfile::Performance);
    assert_eq!(profile_perf.context_token_ceiling(), 4096);
    assert_eq!(profile_perf.enable_speculative_decoding(), true);
    assert_eq!(profile_perf.worker_threads(), 8);

    // Live probe
    let live_power = PowerThermalMonitor::current_power_source();
    let live_thermal = PowerThermalMonitor::current_thermal_state();
    let optimal = PowerThermalMonitor::resolve_optimal_profile();
    assert!(optimal.context_token_ceiling() >= 2048);
    println!("Live Power: {:?}, Thermal: {:?} -> Profile: {:?}", live_power, live_thermal, optimal);
}
