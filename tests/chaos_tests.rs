// ============================================================================
// tests/chaos_tests.rs — Chaos, Fuzz, and Resilience Test Suite for apfel-rs
// Stress tests subsystems under adversarial conditions, corrupt inputs, and races.
// ============================================================================

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use apfel::core::apfs::ApfsSandbox;
use apfel::core::fault_trap::TrappedFault;
use apfel::core::shm::ShmBuffer;
use apfel::core::stop_matcher::{StopMatchResult, StopSequenceMatcher};
use apfel::mcp::parse_command_line;

// ============================================================================
// 1. POSIX SHM IPC Chaos Tests
// ============================================================================

#[test]
fn test_chaos_shm_malformed_headers_and_length_spoofing() {
    let shm_name = format!("/apfel_chaos_hdr_{}", uuid::Uuid::new_v4().simple());
    let capacity = 4096;
    let shm = ShmBuffer::create(&shm_name, capacity).expect("create shm");

    // Write a valid payload first
    shm.write_payload(b"initial healthy payload").expect("write payload");
    assert_eq!(shm.read_payload(), b"initial healthy payload");

    // Corrupt the header memory directly via raw pointer
    let base_ptr = shm.as_ptr() as *mut u8;

    unsafe {
        // Offset 0: magic (u32)
        let magic_ptr = base_ptr as *mut u32;
        *magic_ptr = 0xDEADBEEF; // Invalid magic!

        // Offset 4: version (u32)
        let ver_ptr = base_ptr.add(4) as *mut u32;
        *ver_ptr = 999; // Future unsupported version!

        // Offset 8: capacity (u32)
        let cap_ptr = base_ptr.add(8) as *mut u32;
        *cap_ptr = 0; // Zero capacity!

        // Offset 12: payload_size (u32)
        let size_ptr = base_ptr.add(12) as *mut u32;
        *size_ptr = u32::MAX; // Massive spoofed payload length!
    }

    // Reading from corrupted header must NOT panic or trigger Out-Of-Bounds reads
    let payload = shm.read_payload();
    // Clamped read should be safe and bounded
    assert!(payload.len() <= capacity);
}

#[test]
fn test_chaos_shm_concurrent_multithreaded_clobber() {
    let shm_name = format!("/apfel_chaos_race_{}", uuid::Uuid::new_v4().simple());
    let shm = Arc::new(ShmBuffer::create(&shm_name, 64 * 1024).expect("create shm"));
    let stop_flag = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::new();

    // Spawn 8 concurrent writer threads
    for t_idx in 0..8 {
        let shm_clone = Arc::clone(&shm);
        let stop_clone = Arc::clone(&stop_flag);
        handles.push(std::thread::spawn(move || {
            let mut iter = 0;
            while !stop_clone.load(Ordering::Relaxed) && iter < 1000 {
                let msg = format!("Thread {} iter {} data payload: {}", t_idx, iter, "x".repeat(t_idx * 50));
                let _ = shm_clone.write_payload(msg.as_bytes());
                iter += 1;
            }
        }));
    }

    // Spawn 4 concurrent reader threads
    for _ in 0..4 {
        let shm_clone = Arc::clone(&shm);
        let stop_clone = Arc::clone(&stop_flag);
        handles.push(std::thread::spawn(move || {
            let mut reads = 0;
            while !stop_clone.load(Ordering::Relaxed) && reads < 1000 {
                let payload = shm_clone.read_payload();
                // Ensure payload slice is within capacity bounds and never segfaults
                assert!(payload.len() <= 64 * 1024);
                reads += 1;
            }
        }));
    }

    for h in handles {
        h.join().expect("thread join");
    }
}

#[test]
fn test_chaos_shm_adversarial_name_sanitization() {
    let long_name = "a".repeat(300);
    let adversarial_names = [
        "///double//slashes///",
        "../../etc/passwd",
        "🚀🔥_multi_byte_emoji_shm_name_🍎",
        "name\0with\0null\0bytes",
        "   spaces   and \t tabs \n",
        &long_name, // Exceeding 255 char POSIX limit
        "",
        "////",
    ];

    for name in &adversarial_names {
        // Sanitize name must never panic on multibyte or malformed inputs
        let sanitized = ShmBuffer::sanitize_name(name);
        assert!(sanitized.starts_with('/'));
        assert!(sanitized.len() <= 255);
        assert!(!sanitized.contains('\0'));
    }
}

// ============================================================================
// 2. APFS Sandbox Chaos Tests
// ============================================================================

#[cfg(unix)]
#[test]
fn test_chaos_apfs_sandbox_symlink_cycles_and_dangling() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let ws = temp_dir.path();

    let file_a = ws.join("a.txt");
    let link_b = ws.join("link_b.txt");
    let link_c = ws.join("link_c.txt");
    let dangling = ws.join("dangling_link.txt");

    std::fs::write(&file_a, "real file").unwrap();

    // Create circular links: link_b -> link_c and link_c -> link_b
    std::os::unix::fs::symlink("link_c.txt", &link_b).unwrap();
    std::os::unix::fs::symlink("link_b.txt", &link_c).unwrap();

    // Create dangling link pointing to nonexistent file
    std::os::unix::fs::symlink("nonexistent_file.txt", &dangling).unwrap();

    {
        let mut sandbox = ApfsSandbox::begin(ws).expect("sandbox begin");
        // Mutate real file and add another file
        std::fs::write(&file_a, "corrupted mutation").unwrap();
        std::fs::write(ws.join("new_chaos.txt"), "noise").unwrap();

        // Rollback must not enter an infinite loop resolving symlinks
        sandbox.rollback().expect("rollback must succeed");
    }

    // Verify pristine state restored
    assert_eq!(std::fs::read_to_string(&file_a).unwrap(), "real file");
    assert!(!ws.join("new_chaos.txt").exists());
    assert!(link_b.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(link_c.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(dangling.symlink_metadata().unwrap().file_type().is_symlink());
}

#[test]
fn test_chaos_apfs_sandbox_deeply_nested_hierarchy_and_special_chars() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let ws = temp_dir.path();

    // Create a 25-level deep directory tree with special filename characters
    let mut current = ws.to_path_buf();
    for i in 0..25 {
        current = current.join(format!("dir_level_{}_🚀", i));
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(current.join("file with spaces and 'quotes'.txt"), format!("depth {}", i)).unwrap();
    }

    {
        let mut sandbox = ApfsSandbox::begin(ws).expect("sandbox begin");
        // Corrupt files throughout tree
        std::fs::write(current.join("file with spaces and 'quotes'.txt"), "corrupted at bottom").unwrap();
        std::fs::write(ws.join("root_intruder.txt"), "intruder").unwrap();

        sandbox.rollback().expect("rollback deeply nested");
    }

    // Deep file must be restored
    let deep_file = current.join("file with spaces and 'quotes'.txt");
    assert_eq!(std::fs::read_to_string(&deep_file).unwrap(), "depth 24");
    assert!(!ws.join("root_intruder.txt").exists());
}

#[test]
fn test_chaos_apfs_sandbox_uncommitted_drop_raii_rollback() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let ws = temp_dir.path();
    let original_file = ws.join("safe.rs");
    std::fs::write(&original_file, "pub fn safe() -> bool { true }").unwrap();

    // Scope block simulating unexpected crash or early return without explicit commit/rollback
    {
        let _sandbox = ApfsSandbox::begin(ws).expect("sandbox begin");
        std::fs::write(&original_file, "pub fn corrupted() -> bool { false }").unwrap();
        std::fs::write(ws.join("spawned_chaos.txt"), "temporary leak").unwrap();
        // Dropped here without calling commit() or rollback()!
    }

    // RAII Drop must automatically revert modifications and prune newly spawned files
    assert_eq!(std::fs::read_to_string(&original_file).unwrap(), "pub fn safe() -> bool { true }");
    assert!(!ws.join("spawned_chaos.txt").exists());
}

// ============================================================================
// 3. Streaming Stop Sequence Matcher Chaos Tests
// ============================================================================

#[test]
fn test_chaos_stop_matcher_split_multibyte_utf8_stream() {
    let stops = ["\nObservation:", "### End", "<|im_end|>"];
    let mut matcher = StopSequenceMatcher::new(stops).expect("create matcher");

    // Feed multi-byte UTF-8 emoji and delimiter byte-by-byte
    let full_stream = "Hello 🚀 world! \nObservation: final answer";

    let mut emitted = String::new();
    let mut matched = false;

    // Feed 1 byte at a time (splitting multi-byte UTF-8 code units across chunks)
    // Note: feed expects &str, so test splitting multi-character sub-slices
    let chars: Vec<char> = full_stream.chars().collect();
    for ch in chars {
        let chunk = ch.to_string();
        match matcher.feed(&chunk) {
            StopMatchResult::Emit(s) => emitted.push_str(&s),
            StopMatchResult::Matched { emitted: e, matched_seq } => {
                emitted.push_str(&e);
                assert_eq!(matched_seq, "\nObservation:");
                matched = true;
                break;
            }
            StopMatchResult::Holding => {}
        }
    }

    assert!(matched, "Stop sequence must be detected even on single-char feeds");
    assert_eq!(emitted, "Hello 🚀 world! ");
}

#[test]
fn test_chaos_stop_matcher_repeated_false_alarm_prefixes() {
    let stops = ["\nObservation:"];
    let mut matcher = StopSequenceMatcher::new(stops).expect("matcher");

    // Feed repeated prefixes that almost match but divert at the last character
    let chunks = [
        "\nObserv", "able facts are important.\n",
        "\nObserv", "ing behavior in nature.\n",
        "\nObserva", "tion is not the same as deduction.\n",
        "\nObservation", "al studies only.\n",
        "\nObservation:", " Actual real stop!"
    ];

    let mut emitted = String::new();
    let mut matched = false;

    for chunk in chunks {
        match matcher.feed(chunk) {
            StopMatchResult::Emit(s) => emitted.push_str(&s),
            StopMatchResult::Matched { emitted: e, matched_seq } => {
                emitted.push_str(&e);
                assert_eq!(matched_seq, "\nObservation:");
                matched = true;
                break;
            }
            StopMatchResult::Holding => {}
        }
    }

    assert!(matched);
    assert!(emitted.contains("able facts"));
    assert!(emitted.contains("ing behavior"));
    assert!(emitted.contains("tion is not"));
    assert!(emitted.contains("al studies"));
}

// ============================================================================
// 4. Shadow Compiler & Diagnostic Parsing Chaos Tests
// ============================================================================

#[tokio::test]
async fn test_chaos_shadow_compiler_pipe_flooding() {
    use apfel::core::shadow_compiler::ShadowCompiler;

    let shadow = ShadowCompiler::new();
    // Child process prints 200KB of stdout without newlines (filling OS pipe buffer multiple times)
    // and emits an error diagnostic on stderr
    let cmd = "python3 -c 'import sys; sys.stdout.write(\"A\" * 200000); sys.stderr.write(\"error[E0308]: mismatched types\\n  --> src/main.rs:1:1\\n\")'";

    let exit_code = shadow.execute_and_shadow(cmd, None).await.expect("execute");
    assert_eq!(exit_code, 0);

    let faults = shadow.get_faults().await;
    assert_eq!(faults.len(), 1);
    let fixes = shadow.get_fixes().await;
    assert_eq!(fixes.len(), 1);
}

#[test]
fn test_chaos_diagnostic_parsing_fuzz_inputs() {
    let chaotic_diagnostics = [
        "", // Empty
        "\x1b[31m\x1b[1merror[E0001]\x1b[0m: colorized rustc error\n  --> \x1b[34msrc/main.rs:1:1\x1b[0m\n",
        "error[E]: malformed code without number\n",
        "error TS: typescript error without code\n",
        "thread 'main' panicked at 'explicit panic with null \0 byte': src/lib.rs:99\nstack backtrace:\n",
        &String::from_utf8_lossy(b"binary garbage \x00\x01\x02\xFF\xFE\xFD error[E9999]: corrupted\n"),
        "error[E0382]: use of moved value: `v`\n  --> /deeply/nested/path/to/src/file_with_spaces and 'quotes'.rs:42:15\n",
    ];

    for diag in &chaotic_diagnostics {
        // Parsing should never panic regardless of binary characters or malformed codes
        let res = TrappedFault::parse_diagnostic(diag, 1);
        assert!(res.is_ok());
    }
}

// ============================================================================
// 5. MCP Command Line Parsing Chaos Tests
// ============================================================================

#[test]
fn test_chaos_mcp_command_parsing_adversarial_strings() {
    // Unmatched single quotes (should parse remaining safely)
    let (prog, args) = parse_command_line("server 'unmatched single").expect("unmatched single");
    assert_eq!(prog, "server");
    assert_eq!(args, vec!["unmatched single"]);

    // Unmatched double quotes
    let (prog, args) = parse_command_line("server \"unmatched double").expect("unmatched double");
    assert_eq!(prog, "server");
    assert_eq!(args, vec!["unmatched double"]);

    // Mixed quotes with internal spaces
    let (prog, args) = parse_command_line("\"/Users/test user/bin/server\" --arg1 'val with spaces' --arg2 \"val with 'quotes'\"").expect("mixed");
    assert_eq!(prog, "/Users/test user/bin/server");
    assert_eq!(args, vec!["--arg1", "val with spaces", "--arg2", "val with 'quotes'"]);

    // Multiple consecutive spaces and tabs
    let (prog, args) = parse_command_line("   cmd    \t   --flag1   \t\t   --flag2    ").expect("spaces");
    assert_eq!(prog, "cmd");
    assert_eq!(args, vec!["--flag1", "--flag2"]);

    // Escaped spaces
    let (prog, args) = parse_command_line("my\\ server\\ bin --flag").expect("escaped spaces");
    assert_eq!(prog, "my server bin");
    assert_eq!(args, vec!["--flag"]);

    // Whitespace only must return Err
    assert!(parse_command_line("     ").is_err());
    assert!(parse_command_line("\t\n\r").is_err());
}
