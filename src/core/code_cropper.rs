// ============================================================================
// code_cropper.rs — Code block extraction and fence stripping for --code
// Part of apfel-rs (conforming to Arthur-Ficial/apfel #373)
// ============================================================================

use crate::core::error::ApfelError;

/// Result of extracting code from model output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedCode {
    /// Clean code content with exactly one trailing newline
    pub content: String,
    /// Language specified in the opening fence (e.g. "python", "rust"), if any
    pub language: Option<String>,
}

/// Extracts pure code from raw model output:
/// 1. First fenced block (` ``` ` or ` ~~~ `) wins.
/// 2. If no fence exists, unwraps a single-line inline code span if present (`code` or ```code```).
/// 3. Otherwise, the whole trimmed response passes through as code.
/// 4. An empty or whitespace-only code block returns `Err(ApfelError::NoCodeFound)`.
pub fn extract_code(raw: &str) -> Result<ExtractedCode, ApfelError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(ApfelError::NoCodeFound);
    }

    // Try finding fenced code block
    if let Some(extracted) = extract_first_fence(raw) {
        return Ok(extracted);
    }

    // Single-line inline span check: `code` or ```code``` on a single line
    if !trimmed.contains('\n') {
        if trimmed.starts_with("```") && trimmed.ends_with("```") && trimmed.len() > 6 {
            let inner = &trimmed[3..trimmed.len() - 3].trim();
            if !inner.is_empty() {
                return Ok(ExtractedCode {
                    content: format!("{}\n", inner),
                    language: None,
                });
            }
        } else if trimmed.starts_with('`') && trimmed.ends_with('`') && trimmed.len() > 2 {
            let inner = &trimmed[1..trimmed.len() - 1].trim();
            if !inner.is_empty() {
                return Ok(ExtractedCode {
                    content: format!("{}\n", inner),
                    language: None,
                });
            }
        }
    }

    // Fallback: whole trimmed response passes through
    let normalized = normalize_blank_lines(trimmed);
    if normalized.is_empty() {
        return Err(ApfelError::NoCodeFound);
    }

    Ok(ExtractedCode {
        content: format!("{}\n", normalized),
        language: None,
    })
}

fn extract_first_fence(text: &str) -> Option<ExtractedCode> {
    let lines: Vec<&str> = text.lines().collect();
    let mut in_fence = false;
    let mut fence_char = '`';
    let mut fence_len = 0;
    let mut language: Option<String> = None;
    let mut fence_lines: Vec<&str> = Vec::new();

    for line in lines {
        let trimmed_line = line.trim_start();
        if !in_fence {
            if trimmed_line.starts_with("```") || trimmed_line.starts_with("~~~") {
                fence_char = trimmed_line.chars().next().unwrap();
                fence_len = trimmed_line
                    .chars()
                    .take_while(|&c| c == fence_char)
                    .count();
                let rest = trimmed_line[fence_len..].trim();
                language = if !rest.is_empty() {
                    Some(rest.split_whitespace().next().unwrap_or(rest).to_string())
                } else {
                    None
                };
                in_fence = true;
            }
        } else {
            // Check for closing fence
            if trimmed_line.starts_with(fence_char) {
                let closing_len = trimmed_line
                    .chars()
                    .take_while(|&c| c == fence_char)
                    .count();
                if closing_len >= fence_len && trimmed_line[closing_len..].trim().is_empty() {
                    // Closed!
                    let body = normalize_lines(&fence_lines);
                    if body.trim().is_empty() {
                        return None;
                    }
                    return Some(ExtractedCode {
                        content: format!("{}\n", body),
                        language,
                    });
                }
            }
            fence_lines.push(line);
        }
    }

    // If fence was never closed, treat collected lines if non-empty
    if in_fence && !fence_lines.is_empty() {
        let body = normalize_lines(&fence_lines);
        if !body.trim().is_empty() {
            return Some(ExtractedCode {
                content: format!("{}\n", body),
                language,
            });
        }
    }

    None
}

/// Trims leading and trailing blank lines while preserving all interior whitespace and indentation.
fn normalize_lines(lines: &[&str]) -> String {
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map(|idx| idx + 1)
        .unwrap_or(0);

    if start >= end {
        return String::new();
    }

    lines[start..end].join("\n")
}

/// Trims leading and trailing blank lines from a multi-line string.
fn normalize_blank_lines(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    normalize_lines(&lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_code_with_language_fence() {
        let raw = "Here is the solution:\n\n```python\ndef add(a, b):\n    return a + b\n```\nHope that helps!";
        let extracted = extract_code(raw).unwrap();
        assert_eq!(extracted.language.as_deref(), Some("python"));
        assert_eq!(extracted.content, "def add(a, b):\n    return a + b\n");
    }

    #[test]
    fn test_extract_code_first_block_wins() {
        let raw =
            "First block:\n```rust\nfn one() {}\n```\nSecond block:\n```rust\nfn two() {}\n```";
        let extracted = extract_code(raw).unwrap();
        assert_eq!(extracted.content, "fn one() {}\n");
    }

    #[test]
    fn test_extract_code_with_tildes() {
        let raw = "~~~bash\necho \"hello world\"\n~~~";
        let extracted = extract_code(raw).unwrap();
        assert_eq!(extracted.language.as_deref(), Some("bash"));
        assert_eq!(extracted.content, "echo \"hello world\"\n");
    }

    #[test]
    fn test_extract_code_trims_interior_blank_lines_only() {
        let raw = "```javascript\n\n\nconsole.log('line 1');\n\nconsole.log('line 2');\n\n\n```";
        let extracted = extract_code(raw).unwrap();
        assert_eq!(
            extracted.content,
            "console.log('line 1');\n\nconsole.log('line 2');\n"
        );
    }

    #[test]
    fn test_extract_code_single_line_inline_span() {
        let raw = "`sysctl -n hw.ncpu`";
        let extracted = extract_code(raw).unwrap();
        assert_eq!(extracted.content, "sysctl -n hw.ncpu\n");
        assert_eq!(extracted.language, None);
    }

    #[test]
    fn test_extract_code_single_line_triple_fence() {
        let raw = "```pbcopy```";
        let extracted = extract_code(raw).unwrap();
        assert_eq!(extracted.content, "pbcopy\n");
    }

    #[test]
    fn test_extract_code_unfenced_fallback() {
        let raw = "import sys\nprint(sys.version)";
        let extracted = extract_code(raw).unwrap();
        assert_eq!(extracted.content, "import sys\nprint(sys.version)\n");
    }

    #[test]
    fn test_extract_code_empty_fails_with_no_code() {
        let raw = "   \n\n  ";
        let err = extract_code(raw).unwrap_err();
        assert_eq!(err, ApfelError::NoCodeFound);
    }
}
