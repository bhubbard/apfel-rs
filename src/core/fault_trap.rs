// ============================================================================
// fault_trap.rs — Semantic Kernel Fault Traps & Error Interception
// Captures non-zero process exits, compiler diagnostics, and crash logs to
// trigger immediate on-device micro-agent self-healing patches.
// ============================================================================

use std::path::PathBuf;
use std::process::Output;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FaultTrapError {
    #[error("Process executed successfully with code 0 (no fault)")]
    NoFault,

    #[error("Unable to parse diagnostic from stderr: {0}")]
    UnparseableDiagnostic(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Category of intercepted system or process fault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FaultCategory {
    RustcCompilerError { code: String },
    TypeScriptError { code: String },
    PythonTraceback,
    PanicBacktrace,
    ProcessNonZeroExit { exit_code: i32 },
}

/// Structured diagnostic payload extracted from process failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrappedFault {
    pub category: FaultCategory,
    pub primary_message: String,
    pub target_file: Option<PathBuf>,
    pub line_number: Option<usize>,
    pub column_number: Option<usize>,
    pub raw_snippet: String,
    pub full_stderr: String,
}

impl TrappedFault {
    /// Inspects command execution output and extracts a structured fault if non-zero.
    pub fn from_output(output: &Output) -> Result<Self, FaultTrapError> {
        if output.status.success() {
            return Err(FaultTrapError::NoFault);
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let combined = format!("{}\n{}", stderr, stdout);

        Self::parse_diagnostic(&combined, output.status.code().unwrap_or(-1))
    }

    /// Parses raw diagnostic stderr into a structured TrappedFault.
    pub fn parse_diagnostic(text: &str, exit_code: i32) -> Result<Self, FaultTrapError> {
        // 1. Check for Rustc compiler errors: error[E0382]: ...
        if let Some(rustc_fault) = Self::parse_rustc_diagnostic(text) {
            return Ok(rustc_fault);
        }

        // 2. Check for TypeScript diagnostics: error TS2322: ...
        if let Some(ts_fault) = Self::parse_ts_diagnostic(text) {
            return Ok(ts_fault);
        }

        // 3. Check for Rust/Go panics
        if text.contains("thread '") && text.contains("' panicked at") {
            let line = text.lines().find(|l| l.contains("panicked at")).unwrap_or("");
            return Ok(Self {
                category: FaultCategory::PanicBacktrace,
                primary_message: line.trim().to_string(),
                target_file: None,
                line_number: None,
                column_number: None,
                raw_snippet: line.trim().to_string(),
                full_stderr: text.to_string(),
            });
        }

        // 4. Fallback generic process failure
        Ok(Self {
            category: FaultCategory::ProcessNonZeroExit { exit_code },
            primary_message: format!("Process exited with non-zero status {}", exit_code),
            target_file: None,
            line_number: None,
            column_number: None,
            raw_snippet: text.lines().take(5).collect::<Vec<_>>().join("\n"),
            full_stderr: text.to_string(),
        })
    }

    fn parse_rustc_diagnostic(text: &str) -> Option<Self> {
        for line in text.lines() {
            if line.starts_with("error[E") {
                let parts: Vec<&str> = line.splitn(2, "]: ").collect();
                let code = parts[0].trim_start_matches("error[").to_string();
                let msg = parts.get(1).unwrap_or(&"").to_string();

                // Look for file span line: "  --> src/main.rs:12:5"
                let mut target_file = None;
                let mut line_no = None;
                let mut col_no = None;

                for span_line in text.lines() {
                    let trimmed = span_line.trim_start();
                    if trimmed.starts_with("--> ") {
                        let span_part = trimmed.trim_start_matches("--> ").trim();
                        let span_tokens: Vec<&str> = span_part.split(':').collect();
                        if span_tokens.len() >= 3 {
                            target_file = Some(PathBuf::from(span_tokens[0]));
                            line_no = span_tokens[1].parse::<usize>().ok();
                            col_no = span_tokens[2].parse::<usize>().ok();
                            break;
                        }
                    }
                }

                return Some(Self {
                    category: FaultCategory::RustcCompilerError { code },
                    primary_message: msg,
                    target_file,
                    line_number: line_no,
                    column_number: col_no,
                    raw_snippet: line.to_string(),
                    full_stderr: text.to_string(),
                });
            }
        }
        None
    }

    fn parse_ts_diagnostic(text: &str) -> Option<Self> {
        for line in text.lines() {
            if line.contains("error TS") {
                let parts: Vec<&str> = line.split("error TS").collect();
                let prefix = parts[0].trim().trim_end_matches(':').trim();
                let code_and_msg = parts.get(1).unwrap_or(&"");
                let subparts: Vec<&str> = code_and_msg.splitn(2, ':').collect();
                let code = format!("TS{}", subparts[0].trim());
                let msg = subparts.get(1).unwrap_or(&"").trim().to_string();

                let mut target_file = None;
                let mut line_no = None;
                let mut col_no = None;

                if let Some(pos) = prefix.rfind('(') {
                    let file_str = &prefix[..pos];
                    target_file = Some(PathBuf::from(file_str));
                    let coord_str = prefix[pos + 1..].trim_end_matches(')');
                    let coords: Vec<&str> = coord_str.split(',').collect();
                    if coords.len() == 2 {
                        line_no = coords[0].parse::<usize>().ok();
                        col_no = coords[1].parse::<usize>().ok();
                    }
                }

                return Some(Self {
                    category: FaultCategory::TypeScriptError { code },
                    primary_message: msg,
                    target_file,
                    line_number: line_no,
                    column_number: col_no,
                    raw_snippet: line.to_string(),
                    full_stderr: text.to_string(),
                });
            }
        }
        None
    }

    /// Formats the fault into a micro-agent healing prompt payload.
    pub fn to_micro_agent_prompt(&self) -> String {
        let mut prompt = String::new();
        prompt.push_str("DIAGNOSTIC:\n");
        prompt.push_str(&self.primary_message);
        prompt.push('\n');

        if let Some(ref path) = self.target_file {
            prompt.push_str(&format!(
                "FILE: {}\nLINE: {}\n",
                path.display(),
                self.line_number.unwrap_or(1)
            ));
        }

        prompt.push_str("ERROR OUTPUT:\n");
        prompt.push_str(&self.full_stderr);
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rustc_diagnostic() {
        let stderr = r#"
error[E0382]: use of moved value: `msg`
  --> src/server.rs:42:9
   |
40 |     let msg = String::from("hello");
   |         --- move occurs because `msg` has type `String`
41 |     consume(msg);
   |             --- value moved here
42 |     println!("{}", msg);
   |                    ^^^ value used here after move
"#;
        let fault = TrappedFault::parse_diagnostic(stderr, 1).unwrap();
        match &fault.category {
            FaultCategory::RustcCompilerError { code } => assert_eq!(code, "E0382"),
            _ => panic!("Expected RustcCompilerError"),
        }
        assert_eq!(fault.target_file, Some(PathBuf::from("src/server.rs")));
        assert_eq!(fault.line_number, Some(42));
        assert_eq!(fault.column_number, Some(9));
        assert!(fault.to_micro_agent_prompt().contains("E0382"));
    }

    #[test]
    fn test_parse_typescript_diagnostic() {
        let stderr = "src/index.ts(15,3): error TS2322: Type 'string' is not assignable to type 'number'.";
        let fault = TrappedFault::parse_diagnostic(stderr, 1).unwrap();
        match &fault.category {
            FaultCategory::TypeScriptError { code } => assert_eq!(code, "TS2322"),
            _ => panic!("Expected TypeScriptError"),
        }
        assert_eq!(fault.target_file, Some(PathBuf::from("src/index.ts")));
        assert_eq!(fault.line_number, Some(15));
        assert_eq!(fault.column_number, Some(3));
    }
}
