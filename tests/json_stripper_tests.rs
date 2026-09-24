use apfel::core::json_stripper::JSONFenceStripper;

#[test]
fn test_json_fence_stripper() {
    let input = "```json\n{\n  \"message\": \"success\"\n}\n```";
    let stripped = JSONFenceStripper::strip(input);
    assert_eq!(stripped, "{\n  \"message\": \"success\"\n}");

    // Untagged code fence
    let untagged = "```\n{\n  \"status\": 200\n}\n```";
    let stripped_untagged = JSONFenceStripper::strip(untagged);
    assert_eq!(stripped_untagged, "{\n  \"status\": 200\n}");

    // Non-JSON fence should be preserved
    let python = "```python\nprint('hello')\n```";
    assert_eq!(JSONFenceStripper::strip(python), python);
}
