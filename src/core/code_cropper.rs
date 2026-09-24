// ============================================================================
// code_cropper.rs — Extract first fenced code block for --code / --code-only
// Part of apfel-rs
// ============================================================================

pub struct CodeCropper;

impl CodeCropper {
    /// Extracts the contents of the first markdown code block (```...```).
    /// Returns None if no code block was found.
    pub fn extract(content: &str) -> Option<String> {
        let trimmed = content.trim();
        let mut in_fence = false;
        let mut code_lines = Vec::new();

        for line in trimmed.lines() {
            let line_trimmed = line.trim();
            if line_trimmed.starts_with("```") {
                if in_fence {
                    // Closed fence
                    return Some(code_lines.join("\n"));
                } else {
                    // Opened fence
                    in_fence = true;
                    code_lines.clear();
                }
            } else if in_fence {
                code_lines.push(line);
            }
        }

        None
    }
}
