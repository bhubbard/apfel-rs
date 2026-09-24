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

        let ocr_trimmed = ocr_text.trim();
        if ocr_trimmed.is_empty() {
            format!(
                "=== {} (image) ===\nwhat the image shows: {}\ntext in image: (none detected)",
                name, labels
            )
        } else {
            format!(
                "=== {} (image) ===\nwhat the image shows: {}\ntext in image:\n{}",
                name, labels, ocr_trimmed
            )
        }
    }
}
