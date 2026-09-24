// ============================================================================
// file_framing.rs — Framing for non-text file attachments (PDFs, images)
// Part of apfel-rs
// ============================================================================

pub struct FileFraming;

impl FileFraming {
    /// Format extracted document text with name and kind header
    pub fn document(name: &str, kind: &str, text: &str) -> String {
        format!("=== {} ({}) ===\n{}", name, kind, text.trim_end())
    }

    /// Format image description and OCR text
    pub fn image(name: &str, what_it_shows: &str, ocr_text: &str) -> String {
        let labels = if what_it_shows.trim().is_empty() {
            "(could not confidently identify the image)"
        } else {
            what_it_shows.trim()
        };

        let text_section = if ocr_text.trim().is_empty() {
            "text in image: (none detected)".to_string()
        } else {
            format!("text in image:\n{}", ocr_text.trim_end())
        };

        format!(
            "=== {} (image) ===\nwhat the image shows: {}\n{}",
            name, labels, text_section
        )
    }
}
