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

#[test]
fn test_frame_document_and_frame_image_top_level_functions() {
    use apfel::core::file_framing::{frame_document, frame_image};

    let doc = frame_document("data.csv", "a,b,c\n1,2,3", "csv");
    assert_eq!(doc, "=== data.csv (csv) ===\na,b,c\n1,2,3");

    let img = frame_image("photo.png", "mountain, sky", "SUMMIT 4000M");
    assert!(img.contains("=== photo.png (image) ==="));
    assert!(img.contains("what the image shows: mountain, sky"));
    assert!(img.contains("text in image:\nSUMMIT 4000M"));
}

#[test]
fn test_multimodal_message_content_parts_image_url() {
    use apfel::core::models::{ContentPart, ImageUrl, OpenAIMessage};

    let msg = OpenAIMessage::user_parts(vec![
        ContentPart::Text {
            text: "Describe this image:".to_string(),
        },
        ContentPart::ImageUrl {
            image_url: ImageUrl {
                url: "https://example.com/sunset.jpg".to_string(),
            },
        },
    ]);

    let text = msg.text_content();
    assert!(text.contains("Describe this image:"));
    assert!(text.contains("=== https://example.com/sunset.jpg (image) ==="));
    assert!(text.contains("what the image shows: (could not confidently identify the image)"));
}

#[test]
fn test_multimodal_message_content_parts_data_uri() {
    use apfel::core::models::{ContentPart, ImageUrl, OpenAIMessage};

    let msg = OpenAIMessage::user_parts(vec![
        ContentPart::Text {
            text: "Analyze chart:".to_string(),
        },
        ContentPart::ImageUrl {
            image_url: ImageUrl {
                url: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=".to_string(),
            },
        },
    ]);

    let text = msg.text_content();
    assert!(text.contains("Analyze chart:"));
    assert!(text.contains("=== attachment.png (image) ==="));
}
