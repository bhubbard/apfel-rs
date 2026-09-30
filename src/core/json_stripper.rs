// ============================================================================
// json_stripper.rs — Strip markdown code fences around JSON output
// Part of apfel-rs
// ============================================================================

/// Utility for stripping markdown JSON fences from model responses without allocations.
#[derive(Debug)]
pub struct JSONFenceStripper;

impl JSONFenceStripper {
    /// Strips ```json ... ``` surrounding code fences if present without allocating memory.
    pub fn strip(content: &str) -> &str {
        let trimmed = content.trim();
        if !trimmed.starts_with("```") || !trimmed.ends_with("```") {
            return trimmed;
        }

        let Some(first_newline) = trimmed.find('\n') else {
            return trimmed;
        };

        let fence_header = trimmed[3..first_newline].trim();
        if !fence_header.is_empty() && !fence_header.eq_ignore_ascii_case("json") {
            // Not a JSON code block (e.g. ```python) — leave intact
            return trimmed;
        }

        let inner = &trimmed[first_newline + 1..];
        let Some(last_fence) = inner.rfind("```") else {
            return inner.trim();
        };

        inner[..last_fence].trim()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_valid_json_fence() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        let output = JSONFenceStripper::strip(input);
        assert_eq!(output, "{\"key\": \"value\"}");
    }

    #[test]
    fn test_strip_no_fence() {
        let input = "{\"key\": \"value\"}";
        let output = JSONFenceStripper::strip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_strip_whitespace_only() {
        let input = "   \n  ";
        let output = JSONFenceStripper::strip(input);
        assert_eq!(output, "");
    }

    #[test]
    fn test_strip_different_language() {
        let input = "```python\nprint('hello')\n```";
        let output = JSONFenceStripper::strip(input);
        assert_eq!(output, input);
    }

    #[test]
    fn test_strip_nested_fences() {
        let input = "```json\n{\n  \"code\": \"```python\\npass\\n```\"\n}\n```";
        let output = JSONFenceStripper::strip(input);
        assert_eq!(output, "{\n  \"code\": \"```python\\npass\\n```\"\n}");
    }

    #[test]
    fn test_strip_malformed_fence() {
        let input = "```json\n{\"key\": \"value\"}";
        let output = JSONFenceStripper::strip(input);
        assert_eq!(output, input);
    }
}
