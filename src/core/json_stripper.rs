// ============================================================================
// json_stripper.rs — Strip markdown code fences around JSON output
// Part of apfel-rs
// ============================================================================

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
