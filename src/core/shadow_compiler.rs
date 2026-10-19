// ============================================================================
// shadow_compiler.rs — Speculative Shadow Compiler Execution for apfel-rs
// Streams compiler output asynchronously in real-time, speculatively triggering
// on-device micro-agent repairs while the build process is still running.
// ============================================================================

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, info};
use crate::backend::engine::{BackendEngine, GenerateRequest};
use crate::core::fault_trap::TrappedFault;

/// Candidate patch generated speculatively during ongoing compilation.
#[derive(Debug, Clone)]
pub struct SpeculativeFixCandidate {
    pub fault: TrappedFault,
    pub candidate_patch: Option<String>,
    pub generated_at_ms: u64,
}

/// Shadow compiler monitoring an active compiler process.
pub struct ShadowCompiler {
    discovered_faults: Arc<Mutex<Vec<TrappedFault>>>,
    speculative_fixes: Arc<Mutex<Vec<SpeculativeFixCandidate>>>,
}

impl Default for ShadowCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl ShadowCompiler {
    pub fn new() -> Self {
        Self {
            discovered_faults: Arc::new(Mutex::new(Vec::new())),
            speculative_fixes: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Spawns the compiler command and streams stderr incrementally without LLM synthesis.
    pub async fn execute_and_shadow(
        &self,
        command_line: &str,
        cwd: Option<PathBuf>,
    ) -> Result<i32, std::io::Error> {
        self.execute_and_shadow_with_engine(command_line, cwd, None).await
    }

    /// Spawns the compiler command, streams stdout and stderr incrementally (preventing pipe deadlocks),
    /// and uses the LLM engine to synthesize speculative micro-agent fixes concurrently.
    pub async fn execute_and_shadow_with_engine(
        &self,
        command_line: &str,
        cwd: Option<PathBuf>,
        engine: Option<Arc<dyn BackendEngine>>,
    ) -> Result<i32, std::io::Error> {
        let start_time = std::time::Instant::now();
        info!(cmd = %command_line, "Launching shadow compiler process");

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command_line)
            .current_dir(cwd.unwrap_or_else(|| PathBuf::from(".")))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Drain stdout asynchronously to avoid pipe buffer deadlock (>64KB buffer limit)
        let stdout = child.stdout.take().expect("Failed to open stdout pipe");
        let stdout_task = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                println!("{}", line);
            }
        });

        let stderr = child.stderr.take().expect("Failed to open stderr pipe");
        let mut reader = BufReader::new(stderr).lines();

        let mut stderr_buffer = String::new();
        let faults_ref = Arc::clone(&self.discovered_faults);
        let fixes_ref = Arc::clone(&self.speculative_fixes);

        while let Some(line) = reader.next_line().await? {
            eprintln!("{}", line);
            stderr_buffer.push_str(&line);
            stderr_buffer.push('\n');

            // Trigger diagnostics for Rustc (error[E) or TypeScript (error TS)
            let is_trigger = line.trim().is_empty()
                || line.starts_with("error:")
                || line.starts_with("error[E")
                || line.starts_with("error TS")
                || line.starts_with("warning:");

            if is_trigger && (stderr_buffer.contains("error[E") || stderr_buffer.contains("error TS")) {
                Self::process_diagnostic_chunk(
                    &stderr_buffer,
                    &start_time,
                    &faults_ref,
                    &fixes_ref,
                    &engine,
                ).await;
                stderr_buffer.clear();
            }
        }

        // Process any remaining diagnostic before EOF
        if !stderr_buffer.trim().is_empty() && (stderr_buffer.contains("error[E") || stderr_buffer.contains("error TS")) {
            Self::process_diagnostic_chunk(
                &stderr_buffer,
                &start_time,
                &faults_ref,
                &fixes_ref,
                &engine,
            ).await;
            stderr_buffer.clear();
        }

        let _ = stdout_task.await;
        let status = child.wait().await?;
        let exit_code = status.code().unwrap_or(-1);
        info!(exit_code, elapsed = ?start_time.elapsed(), "Shadow compilation finished");
        Ok(exit_code)
    }

    async fn process_diagnostic_chunk(
        buffer: &str,
        start_time: &std::time::Instant,
        faults_ref: &Arc<Mutex<Vec<TrappedFault>>>,
        fixes_ref: &Arc<Mutex<Vec<SpeculativeFixCandidate>>>,
        engine: &Option<Arc<dyn BackendEngine>>,
    ) {
        if let Ok(fault) = TrappedFault::parse_diagnostic(buffer, 1) {
            debug!(code = ?fault.category, "Shadow compiler trapped diagnostic mid-build");

            let mut faults = faults_ref.lock().await;
            faults.push(fault.clone());
            drop(faults);

            let elapsed = start_time.elapsed().as_millis() as u64;
            let candidate_patch = if let Some(eng) = engine {
                let req = GenerateRequest {
                    prompt: format!(
                        "Compiler diagnostic:\nCategory: {:?}\nMessage: {}\nLocation: {:?}:{:?}\nProvide a concise fix for this error.",
                        fault.category, fault.primary_message, fault.target_file, fault.line_number
                    ),
                    system_prompt: Some("You are an expert compiler error remediation agent. Provide concise, direct fixes.".to_string()),
                    messages: None,
                    temperature: Some(0.1),
                    top_p: None,
                    max_tokens: Some(512),
                    permissive: false,
                    seed: None,
                    use_case: None,
                };
                match eng.generate(&req) {
                    Ok(res) => Some(res.content),
                    Err(e) => Some(format!("// Speculative fix generation failed: {}", e)),
                }
            } else {
                Some(format!(
                    "// Speculative fix candidate generated in {}ms for {:?}",
                    elapsed, fault.category
                ))
            };

            let candidate = SpeculativeFixCandidate {
                fault: fault.clone(),
                candidate_patch,
                generated_at_ms: elapsed,
            };

            let mut fixes = fixes_ref.lock().await;
            fixes.push(candidate);
        }
    }

    pub async fn get_fixes(&self) -> Vec<SpeculativeFixCandidate> {
        self.speculative_fixes.lock().await.clone()
    }

    pub async fn get_faults(&self) -> Vec<TrappedFault> {
        self.discovered_faults.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shadow_compiler_captures_error() {
        let shadow = ShadowCompiler::new();
        // Simulate compiler emitting Rustc error to stderr then sleeping briefly
        let cmd = "echo 'error[E0499]: cannot borrow `v` as mutable more than once at a time\n  --> src/main.rs:10:5\n' >&2; echo '' >&2";

        let exit_code = shadow.execute_and_shadow(cmd, None).await.unwrap();
        assert_eq!(exit_code, 0);

        let faults = shadow.get_faults().await;
        assert_eq!(faults.len(), 1);
        let fixes = shadow.get_fixes().await;
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].candidate_patch.as_ref().unwrap().contains("E0499"));
    }

    #[tokio::test]
    async fn test_shadow_compiler_captures_typescript_error() {
        let shadow = ShadowCompiler::new();
        let cmd = "echo 'error TS2322: Type string is not assignable to type number.\n  --> src/index.ts:15:3\n' >&2";

        let exit_code = shadow.execute_and_shadow(cmd, None).await.unwrap();
        assert_eq!(exit_code, 0);

        let faults = shadow.get_faults().await;
        assert_eq!(faults.len(), 1);
        let fixes = shadow.get_fixes().await;
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].candidate_patch.is_some());
    }

    #[tokio::test]
    async fn test_shadow_compiler_stdout_drain_and_engine_synthesis() {
        use crate::backend::mock::MockEngine;

        let shadow = ShadowCompiler::new();
        let mock_engine: Arc<dyn BackendEngine> = Arc::new(MockEngine::new());
        // Emit large stdout (>100KB) to verify no pipe deadlock, along with an error
        let cmd = "python3 -c 'import sys; sys.stdout.write(\"x\" * 100000 + \"\\n\"); sys.stderr.write(\"error[E0382]: use of moved value: `s`\\n  --> src/main.rs:5:1\\n\")'";

        let exit_code = shadow
            .execute_and_shadow_with_engine(cmd, None, Some(mock_engine))
            .await
            .unwrap();
        assert_eq!(exit_code, 0);

        let faults = shadow.get_faults().await;
        assert_eq!(faults.len(), 1);
        let fixes = shadow.get_fixes().await;
        assert_eq!(fixes.len(), 1);
        assert!(fixes[0].candidate_patch.is_some());
    }
}
