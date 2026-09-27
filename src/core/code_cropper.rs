// ============================================================================
// code_cropper.rs — Extract and sanitize code blocks for --code / --code-only
// Part of apfel-rs
// ============================================================================

pub struct CodeCropper;

impl CodeCropper {
    /// Extracts the contents of the first markdown code block (```...```).
    /// If no fences are present, falls back to the bare response.
    /// If fences are opened but not closed (truncated), extracts to EOF.
    pub fn extract(content: &str) -> Option<&str> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(open_tag) = trimmed.find("```") {
            let rest = &trimmed[open_tag + 3..];
            let code_start = if let Some(newline_pos) = rest.find('\n') {
                open_tag + 3 + newline_pos + 1
            } else {
                open_tag + 3
            };

            let code_slice = if let Some(close_tag) = trimmed[code_start..].find("```") {
                &trimmed[code_start..code_start + close_tag]
            } else {
                // Truncated without closing fence: take until EOF
                &trimmed[code_start..]
            };

            let res = code_slice.trim_end_matches(['\r', '\n']);
            if !res.is_empty() {
                return Some(res);
            }
        }

        // Unfenced fallback: only return trimmed bare content if it looks like code
        if Self::looks_like_code(trimmed) {
            Some(trimmed)
        } else {
            None
        }
    }

    /// Determines if unfenced text resembles programming code
    fn looks_like_code(s: &str) -> bool {
        let code_keywords = [
            "fn ", "pub ", "struct ", "impl ", "enum ", "let ", "const ",
            "use ", "mod ", "import ", "def ", "class ", "return ", "func ",
            "#include", "package ", "type ", "interface ", "//", "/*",
        ];
        let has_keyword = s.lines().any(|l| {
            let t = l.trim_start();
            code_keywords.iter().any(|&kw| t.starts_with(kw))
        });
        let has_structural_symbols = (s.contains('{') && s.contains('}')) || (s.contains(';') && s.contains('('));
        has_keyword || has_structural_symbols
    }

    /// Balances unclosed braces and removes trailing partial statement fragments.
    pub fn sanitize(code: &str) -> String {
        let mut lines: Vec<&str> = code.lines().collect();

        // Strip trailing incomplete lines (e.g. truncated `let`, `pub fn ... (` without body)
        while let Some(last) = lines.last() {
            let t = last.trim();
            if t == "let" || t.ends_with("->") || t.ends_with(',') || t.ends_with("::") || t.is_empty() {
                lines.pop();
            } else {
                break;
            }
        }

        let mut cleaned = lines.join("\n");

        // Count unmatched braces
        let mut brace_depth: i32 = 0;
        let mut in_string = false;
        let mut prev_char = ' ';

        for ch in cleaned.chars() {
            if ch == '"' && prev_char != '\\' {
                in_string = !in_string;
            } else if !in_string {
                if ch == '{' {
                    brace_depth += 1;
                } else if ch == '}' {
                    brace_depth = (brace_depth - 1).max(0);
                }
            }
            prev_char = ch;
        }

        // Auto-close missing braces
        if brace_depth > 0 {
            cleaned.push('\n');
            for _ in 0..brace_depth {
                cleaned.push_str("}\n");
            }
        }

        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fenced_extraction() {
        let input = "Here is code:\n```rust\nfn main() {}\n```\nDone.";
        assert_eq!(CodeCropper::extract(input), Some("fn main() {}"));
    }

    #[test]
    fn test_unclosed_fence_extraction() {
        let input = "```rust\nfn main() {\n    let x = 1;\n";
        assert_eq!(CodeCropper::extract(input), Some("fn main() {\n    let x = 1;"));
    }

    #[test]
    fn test_unfenced_fallback() {
        let input = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}";
        assert_eq!(CodeCropper::extract(input), Some("pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}"));
    }

    #[test]
    fn test_sanitize_brace_balancing() {
        let input = "pub struct Test {\n    pub a: i32,";
        let sanitized = CodeCropper::sanitize(input);
        assert!(sanitized.trim().ends_with('}'));
    }
}
