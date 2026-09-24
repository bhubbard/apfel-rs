use apfel::core::schema::{SchemaIR, SchemaParser};

#[test]
fn test_schema_parser_basic_object() {
    let schema_json = r#"{
        "type": "object",
        "description": "User profile",
        "properties": {
            "name": { "type": "string", "description": "User name" },
            "age": { "type": "integer" },
            "is_active": { "type": "boolean" }
        },
        "required": ["name"]
    }"#;

    let ir = SchemaParser::parse(schema_json, "UserProfile").expect("Failed to parse schema");
    match ir {
        SchemaIR::Object { name, description, properties } => {
            assert_eq!(name, "UserProfile");
            assert_eq!(description, Some("User profile".to_string()));
            assert_eq!(properties.len(), 3);

            // BTreeMap sorts properties alphabetically: age, is_active, name
            assert_eq!(properties[0].name, "age");
            assert!(properties[0].is_optional);

            assert_eq!(properties[1].name, "is_active");
            assert!(properties[1].is_optional);

            assert_eq!(properties[2].name, "name");
            assert!(!properties[2].is_optional);
        }
        _ => panic!("Expected SchemaIR::Object"),
    }
}

#[test]
fn test_schema_parser_nullable_normalization() {
    let schema_json = r#"{
        "type": "object",
        "properties": {
            "title": {
                "anyOf": [
                    { "type": "string" },
                    { "type": "null" }
                ]
            }
        },
        "required": ["title"]
    }"#;

    let ir = SchemaParser::parse(schema_json, "Post").expect("Failed to parse nullable schema");
    match ir {
        SchemaIR::Object { properties, .. } => {
            assert_eq!(properties.len(), 1);
            assert_eq!(properties[0].name, "title");
            assert!(properties[0].is_optional); // Nullable property is normalized to optional
        }
        _ => panic!("Expected SchemaIR::Object"),
    }
}
