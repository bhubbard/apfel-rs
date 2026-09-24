// ============================================================================
// json_stripper.rs — Strip markdown code fences around JSON output
// Part of apfel-rs
// ============================================================================

pub struct JSONFenceStripper;

impl JSONFenceStripper {
    /// Strips ```json ... ``` surrounding code fences if present.
    pub fn strip(content: &str) -> String {
        let trimmed = content.trim();
        if !trimmed.starts_with("```") || !trimmed.ends_with("```") {
            return trimmed.to_string();
        }

        let Some(first_newline) = trimmed.find('\n') else {
            return trimmed.to_string();
        };

        let fence_header = trimmed[3..first_newline].trim().to_lowercase();
        if !fence_header.is_empty() && fence_header != "json" {
            // Not a JSON code block (e.g. ```python) — leave intact
            return trimmed.to_string();
        }

        let inner = &trimmed[first_newline + 1..];
        let Some(last_fence) = inner.rfind("```") else {
            return inner.trim().to_string();
        };

        inner[..last_fence].trim().to_string()
    }
}
