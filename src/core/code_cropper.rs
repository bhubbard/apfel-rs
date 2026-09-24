// ============================================================================
// code_cropper.rs — Extract first fenced code block for --code / --code-only
// Part of apfel-rs
// ============================================================================

pub struct CodeCropper;

impl CodeCropper {
    /// Extracts the contents of the first markdown code block (```...```).
    /// Returns None if no code block was found.
    pub fn extract(content: &str) -> Option<&str> {
        let open_tag = content.find("```")?;
        let newline_after_open = content[open_tag..].find('\n')? + open_tag + 1;
        let close_tag = content[newline_after_open..].find("```")? + newline_after_open;
        Some(content[newline_after_open..close_tag].trim_end_matches(['\r', '\n']))
    }
}
