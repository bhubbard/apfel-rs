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
        SchemaIR::Object {
            name,
            description,
            properties,
        } => {
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

#[test]
fn test_deeply_nested_schema_rejected() {
    let mut schema = String::from(r#"{"type":"string"}"#);
    for _ in 0..70 {
        schema = format!(
            r#"{{"type":"object","properties":{{"nested":{}}}}}"#,
            schema
        );
    }
    let res = SchemaParser::parse(&schema, "Deep");
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("exceeds maximum nesting depth") || err.contains("recursion limit"));
}

#[test]
fn test_schema_parser_array_and_enum() {
    let schema_json = r#"{
        "type": "object",
        "properties": {
            "tags": {
                "type": "array",
                "items": { "type": "string" }
            },
            "status": {
                "type": "string",
                "enum": ["draft", "published", "archived"]
            }
        }
    }"#;

    let ir = SchemaParser::parse(schema_json, "Article").expect("Failed to parse array and enum");
    match ir {
        SchemaIR::Object { properties, .. } => {
            assert_eq!(properties.len(), 2);
            assert_eq!(properties[0].name, "status");
            assert_eq!(properties[1].name, "tags");
        }
        _ => panic!("Expected SchemaIR::Object"),
    }
}

#[test]
fn test_schema_parser_type_array_null() {
    let schema_json = r#"{
        "type": "object",
        "properties": {
            "nickname": {
                "type": ["string", "null"]
            }
        }
    }"#;

    let ir = SchemaParser::parse(schema_json, "User").expect("Failed to parse type array null");
    match ir {
        SchemaIR::Object { properties, .. } => {
            assert_eq!(properties.len(), 1);
            assert!(properties[0].is_optional);
        }
        _ => panic!("Expected SchemaIR::Object"),
    }
}

#[test]
fn test_schema_parser_invalid_root_and_properties() {
    // Array root rejected
    assert!(SchemaParser::parse("[1, 2, 3]", "ArrayRoot").is_err());

    // Invalid JSON rejected
    assert!(SchemaParser::parse("{ invalid json }", "InvalidJson").is_err());

    // Property not an object rejected
    let bad_prop = r#"{"type": "object", "properties": {"age": 42}}"#;
    assert!(SchemaParser::parse(bad_prop, "BadProp").is_err());

    // Array missing items rejected
    let bad_arr = r#"{"type": "object", "properties": {"list": {"type": "array"}}}"#;
    assert!(SchemaParser::parse(bad_arr, "BadArray").is_err());

    // Array items not object rejected
    let bad_items =
        r#"{"type": "object", "properties": {"list": {"type": "array", "items": "string"}}}"#;
    assert!(SchemaParser::parse(bad_items, "BadItems").is_err());

    // Unsupported schema type rejected
    let unsupp = r#"{"type": "object", "properties": {"weird": {"type": "unsupported_xyz"}}}"#;
    assert!(SchemaParser::parse(unsupp, "Unsupp").is_err());
}

#[test]
fn test_schema_parser_unions_and_number() {
    // anyOf with null first
    let null_first = r#"{
        "type": "object",
        "properties": {
            "val": {
                "anyOf": [
                    { "type": "null" },
                    { "type": "string" }
                ]
            }
        }
    }"#;
    let ir = SchemaParser::parse(null_first, "NullFirst").unwrap();
    if let SchemaIR::Object { properties, .. } = ir {
        assert!(properties[0].is_optional);
    } else {
        panic!("Expected Object");
    }

    // Complex anyOf >2 variants rejected
    let complex_anyof = r#"{
        "type": "object",
        "properties": {
            "val": { "anyOf": [{ "type": "string" }, { "type": "number" }, { "type": "null" }] }
        }
    }"#;
    assert!(SchemaParser::parse(complex_anyof, "Complex").is_err());

    // Multi-type without null rejected
    let bad_union = r#"{
        "type": "object",
        "properties": {
            "val": { "type": ["string", "number"] }
        }
    }"#;
    assert!(SchemaParser::parse(bad_union, "BadUnion").is_err());

    // Number type supported
    let num_schema = r#"{
        "type": "object",
        "properties": {
            "score": { "type": "number" }
        }
    }"#;
    let ir = SchemaParser::parse(num_schema, "Score").unwrap();
    if let SchemaIR::Object { properties, .. } = ir {
        match &properties[0].schema {
            SchemaIR::Number { name, .. } => assert_eq!(name, "score"),
            _ => panic!("Expected Number schema"),
        }
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_schema_parser_defs_resolution() {
    let schema_json = r##"{
        "type": "object",
        "properties": {
            "user": { "$ref": "#/$defs/User" }
        },
        "required": ["user"],
        "$defs": {
            "User": {
                "type": "object",
                "description": "User account",
                "properties": {
                    "name": { "type": "string" },
                    "age": { "type": "integer" }
                },
                "required": ["name"]
            }
        }
    }"##;

    let ir = SchemaParser::parse(schema_json, "Root").expect("Failed to parse schema with $defs");
    match ir {
        SchemaIR::Object { properties, .. } => {
            assert_eq!(properties.len(), 1);
            let user_prop = &properties[0];
            assert_eq!(user_prop.name, "user");
            assert!(!user_prop.is_optional);
            assert_eq!(user_prop.description.as_deref(), Some("User account"));

            match &user_prop.schema {
                SchemaIR::Object {
                    name,
                    properties: user_props,
                    ..
                } => {
                    assert_eq!(name, "user");
                    assert_eq!(user_props.len(), 2);
                    assert_eq!(user_props[0].name, "age");
                    assert!(user_props[0].is_optional);
                    assert_eq!(user_props[1].name, "name");
                    assert!(!user_props[1].is_optional);
                }
                _ => panic!("Expected Object schema for user"),
            }
        }
        _ => panic!("Expected Object schema for root"),
    }
}

#[test]
fn test_schema_parser_definitions_resolution() {
    let schema_json = r##"{
        "type": "object",
        "properties": {
            "address": { "$ref": "#/definitions/Address" }
        },
        "definitions": {
            "Address": {
                "type": "object",
                "properties": {
                    "street": { "type": "string" },
                    "city": { "type": "string" }
                }
            }
        }
    }"##;

    let ir = SchemaParser::parse(schema_json, "Profile")
        .expect("Failed to parse schema with definitions");
    match ir {
        SchemaIR::Object { properties, .. } => {
            assert_eq!(properties.len(), 1);
            let addr = &properties[0];
            assert_eq!(addr.name, "address");
            assert!(addr.is_optional);
            match &addr.schema {
                SchemaIR::Object {
                    properties: addr_props,
                    ..
                } => {
                    assert_eq!(addr_props.len(), 2);
                    assert_eq!(addr_props[0].name, "city");
                    assert_eq!(addr_props[1].name, "street");
                }
                _ => panic!("Expected Object schema for address"),
            }
        }
        _ => panic!("Expected Object schema for root"),
    }
}

#[test]
fn test_schema_parser_array_items_ref() {
    let schema_json = r##"{
        "type": "object",
        "properties": {
            "roles": {
                "type": "array",
                "items": { "$ref": "#/$defs/Role" }
            }
        },
        "$defs": {
            "Role": {
                "type": "string",
                "enum": ["admin", "member", "guest"]
            }
        }
    }"##;

    let ir = SchemaParser::parse(schema_json, "Auth").expect("Failed to parse array items $ref");
    match ir {
        SchemaIR::Object { properties, .. } => {
            assert_eq!(properties.len(), 1);
            let roles = &properties[0];
            assert_eq!(roles.name, "roles");
            match &roles.schema {
                SchemaIR::Array { item_name, items } => {
                    assert_eq!(item_name, "roles");
                    match items.as_ref() {
                        SchemaIR::String { enum_values, .. } => {
                            assert_eq!(
                                enum_values.as_ref().unwrap(),
                                &vec![
                                    "admin".to_string(),
                                    "member".to_string(),
                                    "guest".to_string()
                                ]
                            );
                        }
                        _ => panic!("Expected String inside Array"),
                    }
                }
                _ => panic!("Expected Array schema"),
            }
        }
        _ => panic!("Expected Object schema"),
    }
}

#[test]
fn test_schema_parser_chained_refs() {
    let schema_json = r##"{
        "type": "object",
        "properties": {
            "val": { "$ref": "#/$defs/RefA" }
        },
        "$defs": {
            "RefA": { "$ref": "#/definitions/RefB" }
        },
        "definitions": {
            "RefB": { "type": "integer" }
        }
    }"##;

    let ir = SchemaParser::parse(schema_json, "Chained").expect("Failed chained ref resolution");
    match ir {
        SchemaIR::Object { properties, .. } => {
            assert_eq!(properties.len(), 1);
            match &properties[0].schema {
                SchemaIR::Integer { name, .. } => assert_eq!(name, "val"),
                _ => panic!("Expected Integer schema after chained resolution"),
            }
        }
        _ => panic!("Expected Object schema"),
    }
}

#[test]
fn test_schema_parser_circular_ref_prevention() {
    // Direct cycle
    let direct_cycle = r##"{
        "type": "object",
        "properties": {
            "loop": { "$ref": "#/$defs/Loop" }
        },
        "$defs": {
            "Loop": { "$ref": "#/$defs/Loop" }
        }
    }"##;
    let res = SchemaParser::parse(direct_cycle, "DirectCycle");
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("exceeds maximum nesting depth"));

    // Indirect recursive cycle (e.g. self-referencing tree node)
    let recursive_tree = r##"{
        "type": "object",
        "properties": {
            "tree": { "$ref": "#/$defs/Node" }
        },
        "$defs": {
            "Node": {
                "type": "object",
                "properties": {
                    "child": { "$ref": "#/$defs/Node" }
                }
            }
        }
    }"##;
    let res2 = SchemaParser::parse(recursive_tree, "Tree");
    assert!(res2.is_err());
    let err2 = res2.unwrap_err().to_string();
    assert!(err2.contains("exceeds maximum nesting depth"));
}

#[test]
fn test_schema_parser_unresolved_ref_error() {
    let missing_ref = r##"{
        "type": "object",
        "properties": {
            "ghost": { "$ref": "#/$defs/NonExistent" }
        }
    }"##;
    let res = SchemaParser::parse(missing_ref, "Missing");
    assert!(res.is_err());
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Unresolved schema reference") && err.contains("NonExistent"));
}

#[test]
fn test_schema_parser_root_ref() {
    let root_ref_schema = r##"{
        "$ref": "#/$defs/MyRoot",
        "$defs": {
            "MyRoot": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" }
                }
            }
        }
    }"##;

    let ir = SchemaParser::parse(root_ref_schema, "CustomRoot").expect("Root ref failed");
    match ir {
        SchemaIR::Object {
            name, properties, ..
        } => {
            assert_eq!(name, "CustomRoot");
            assert_eq!(properties.len(), 1);
            assert_eq!(properties[0].name, "title");
        }
        _ => panic!("Expected Object schema"),
    }
}

#[test]
fn test_schema_parser_ref_description_override() {
    let schema_json = r##"{
        "type": "object",
        "properties": {
            "author": {
                "description": "Article author override",
                "$ref": "#/$defs/Person"
            }
        },
        "$defs": {
            "Person": {
                "type": "object",
                "description": "Default person description",
                "properties": {
                    "name": { "type": "string" }
                }
            }
        }
    }"##;

    let ir = SchemaParser::parse(schema_json, "Article").unwrap();
    match ir {
        SchemaIR::Object { properties, .. } => {
            assert_eq!(
                properties[0].description.as_deref(),
                Some("Article author override")
            );
        }
        _ => panic!("Expected Object"),
    }
}
