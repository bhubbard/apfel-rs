use apfel::core::code_cropper::CodeCropper;

#[test]
fn test_code_cropper_extracts_first_fence() {
    let input = "Here is the code:\n```rust\nfn main() {\n    println!(\"Hello\");\n}\n```\nAnd some trailing text.";
    let extracted = CodeCropper::extract(input).expect("Failed to extract code");
    assert_eq!(extracted, "fn main() {\n    println!(\"Hello\");\n}");
}

#[test]
fn test_code_cropper_no_code() {
    let input = "Just regular prose without any fences.";
    assert!(CodeCropper::extract(input).is_none());
}
