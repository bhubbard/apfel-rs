// ============================================================================
// tests/file_framing_tests.rs — Tests for FileFraming of attachments
// Ported from: FileFramingTests.swift
// ============================================================================

use apfel::core::file_framing::FileFraming;

#[test]
fn test_document_frame_wraps_text_with_name_and_kind() {
    let out = FileFraming::document("report.pdf", "pdf", "Q3 revenue up 12%.");
    assert_eq!(out, "=== report.pdf (pdf) ===\nQ3 revenue up 12%.");
}

#[test]
fn test_document_frame_trims_trailing_whitespace() {
    let out = FileFraming::document("a.pdf", "pdf", "hello\n\n");
    assert_eq!(out, "=== a.pdf (pdf) ===\nhello");
}

#[test]
fn test_image_frame_shows_labels_and_ocr() {
    let out = FileFraming::image("receipt.jpg", "receipt, paper, text", "TOTAL 9.99");
    assert_eq!(
        out,
        "=== receipt.jpg (image) ===\nwhat the image shows: receipt, paper, text\ntext in image:\nTOTAL 9.99"
    );
}

#[test]
fn test_image_frame_when_no_labels() {
    let out = FileFraming::image("blur.png", "", "SIGN");
    assert!(out.contains("what the image shows: (could not confidently identify the image)"));
    assert!(out.contains("text in image:\nSIGN"));
}

#[test]
fn test_image_frame_when_no_text() {
    let out = FileFraming::image("cat.jpg", "cat, animal", "");
    assert!(out.contains("what the image shows: cat, animal"));
    assert!(out.contains("text in image: (none detected)"));
}

#[test]
fn test_image_frame_with_neither_labels_nor_text() {
    let out = FileFraming::image("x.png", "", "");
    assert!(out.starts_with("=== x.png (image) ==="));
    assert!(out.contains("(could not confidently identify the image)"));
    assert!(out.contains("text in image: (none detected)"));
}
