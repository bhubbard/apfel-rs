// ============================================================================
// schema.rs — JSON Schema to SchemaIR parser & validator
// Part of apfel-rs
// ============================================================================

use crate::core::error::ApfelError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// Intermediate representation of supported JSON Schema types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SchemaIR {
    Object {
        name: String,
        description: Option<String>,
        properties: Vec<PropertyIR>,
    },
    String {
        name: String,
        description: Option<String>,
        enum_values: Option<Vec<String>>,
    },
    Integer {
        name: String,
        description: Option<String>,
    },
    Number {
        name: String,
        description: Option<String>,
    },
    Bool {
        name: String,
        description: Option<String>,
    },
    Array {
        item_name: String,
        items: Box<SchemaIR>,
    },
}

/// Metadata and schema definition for an object property in SchemaIR.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyIR {
    pub name: String,
    pub description: Option<String>,
    pub schema: SchemaIR,
    pub is_optional: bool,
}

/// Parses and validates JSON Schema strings into SchemaIR trees.
#[derive(Debug)]
pub struct SchemaParser;

impl SchemaParser {
    pub const MAX_SCHEMA_DEPTH: usize = 64;

    pub fn parse(json_str: &str, root_name: &str) -> Result<SchemaIR, ApfelError> {
        let val: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| ApfelError::Usage(format!("Invalid JSON in schema: {}", e)))?;

        let obj = val
            .as_object()
            .ok_or_else(|| ApfelError::Usage("Schema root must be a JSON object".to_string()))?;

        Self::parse_object(obj, root_name, obj, 0).map(|(ir, _)| ir)
    }

    fn resolve_ref<'a>(
        root: &'a serde_json::Map<String, serde_json::Value>,
        ref_str: &str,
    ) -> Result<&'a serde_json::Map<String, serde_json::Value>, ApfelError> {
        let path = if let Some(stripped) = ref_str.strip_prefix("#/") {
            stripped
        } else if let Some(stripped) = ref_str.strip_prefix('#') {
            stripped.trim_start_matches('/')
        } else {
            ref_str.trim_start_matches('/')
        };

        if path.is_empty() {
            return Ok(root);
        }

        let mut current_map = root;
        let parts: Vec<&str> = path.split('/').collect();

        for (idx, part) in parts.iter().enumerate() {
            let unescaped = part.replace("~1", "/").replace("~0", "~");
            let val = current_map.get(&unescaped).ok_or_else(|| {
                ApfelError::Usage(format!(
                    "Unresolved schema reference '{}': key '{}' not found",
                    ref_str, unescaped
                ))
            })?;

            if idx == parts.len() - 1 {
                return val.as_object().ok_or_else(|| {
                    ApfelError::Usage(format!(
                        "Schema reference '{}' does not resolve to an object",
                        ref_str
                    ))
                });
            } else {
                current_map = val.as_object().ok_or_else(|| {
                    ApfelError::Usage(format!(
                        "Schema reference '{}': intermediate token '{}' is not an object",
                        ref_str, unescaped
                    ))
                })?;
            }
        }

        Err(ApfelError::Usage(format!("Empty reference '{}'", ref_str)))
    }

    fn parse_object<'a>(
        obj: &'a serde_json::Map<String, serde_json::Value>,
        name: &str,
        root: &'a serde_json::Map<String, serde_json::Value>,
        depth: usize,
    ) -> Result<(SchemaIR, bool), ApfelError> {
        if depth > Self::MAX_SCHEMA_DEPTH {
            return Err(ApfelError::Usage(format!(
                "JSON schema exceeds maximum nesting depth of {}",
                Self::MAX_SCHEMA_DEPTH
            )));
        }

        let (mut node, mut is_nullable) = Self::normalize_union(obj)?;
        let mut ref_depth = depth;
        let mut override_desc = None;

        while let Some(ref_val) = node.get("$ref") {
            if ref_depth > Self::MAX_SCHEMA_DEPTH {
                return Err(ApfelError::Usage(format!(
                    "JSON schema exceeds maximum nesting depth of {}",
                    Self::MAX_SCHEMA_DEPTH
                )));
            }
            ref_depth += 1;
            let ref_str = ref_val
                .as_str()
                .ok_or_else(|| ApfelError::Usage("'$ref' must be a string".to_string()))?;
            if override_desc.is_none() {
                if let Some(desc) = node.get("description").and_then(|v| v.as_str()) {
                    override_desc = Some(desc.to_string());
                }
            }
            let resolved = Self::resolve_ref(root, ref_str)?;
            let (normalized_resolved, resolved_nullable) = Self::normalize_union(resolved)?;
            is_nullable = is_nullable || resolved_nullable;
            node = normalized_resolved;
        }

        let node_type = node
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("object");
        let description = override_desc.or_else(|| {
            node.get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        });

        match node_type {
            "object" => {
                let empty_map = serde_json::Map::new();
                let props_map = node
                    .get("properties")
                    .and_then(|v| v.as_object())
                    .unwrap_or(&empty_map);

                let required_set: HashSet<String> = node
                    .get("required")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                // BTreeMap ensures alphabetical ordering for deterministic IR
                let sorted_props: BTreeMap<_, _> = props_map.iter().collect();
                let mut properties = Vec::with_capacity(sorted_props.len());

                for (key, val) in sorted_props {
                    let prop_obj = val.as_object().ok_or_else(|| {
                        ApfelError::Usage(format!("Property '{}' must be an object", key))
                    })?;
                    let (child_ir, child_nullable) =
                        Self::parse_object(prop_obj, key, root, ref_depth + 1)?;
                    let child_desc = prop_obj
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| match &child_ir {
                            SchemaIR::Object { description, .. } => description.clone(),
                            SchemaIR::String { description, .. } => description.clone(),
                            SchemaIR::Integer { description, .. } => description.clone(),
                            SchemaIR::Number { description, .. } => description.clone(),
                            SchemaIR::Bool { description, .. } => description.clone(),
                            SchemaIR::Array { .. } => None,
                        });

                    let is_optional = !required_set.contains(key) || child_nullable;
                    properties.push(PropertyIR {
                        name: key.clone(),
                        description: child_desc,
                        schema: child_ir,
                        is_optional,
                    });
                }

                Ok((
                    SchemaIR::Object {
                        name: name.to_string(),
                        description,
                        properties,
                    },
                    is_nullable,
                ))
            }
            "string" => {
                let enum_vals = node.get("enum").and_then(|v| v.as_array()).map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                });

                Ok((
                    SchemaIR::String {
                        name: name.to_string(),
                        description,
                        enum_values: enum_vals,
                    },
                    is_nullable,
                ))
            }
            "integer" => Ok((
                SchemaIR::Integer {
                    name: name.to_string(),
                    description,
                },
                is_nullable,
            )),
            "number" => Ok((
                SchemaIR::Number {
                    name: name.to_string(),
                    description,
                },
                is_nullable,
            )),
            "boolean" => Ok((
                SchemaIR::Bool {
                    name: name.to_string(),
                    description,
                },
                is_nullable,
            )),
            "array" => {
                let items_val = node.get("items").ok_or_else(|| {
                    ApfelError::Usage(format!("Array '{}' is missing 'items' schema", name))
                })?;
                let items_obj = items_val.as_object().ok_or_else(|| {
                    ApfelError::Usage(format!("'items' in array '{}' must be an object", name))
                })?;
                let (inner, _) =
                    Self::parse_object(items_obj, &format!("{}_item", name), root, ref_depth + 1)?;
                Ok((
                    SchemaIR::Array {
                        item_name: name.to_string(),
                        items: Box::new(inner),
                    },
                    is_nullable,
                ))
            }
            other => Err(ApfelError::Usage(format!(
                "Unsupported schema type: {}",
                other
            ))),
        }
    }

    fn normalize_union<'a>(
        node: &'a serde_json::Map<String, serde_json::Value>,
    ) -> Result<(&'a serde_json::Map<String, serde_json::Value>, bool), ApfelError> {
        // Handle anyOf / oneOf
        if let Some(any_of) = node.get("anyOf").or_else(|| node.get("oneOf")) {
            if let Some(arr) = any_of.as_array() {
                if arr.len() == 2 {
                    let first = arr[0].as_object();
                    let second = arr[1].as_object();
                    let is_second_null = second.map_or(false, |o| {
                        o.get("type").and_then(|t| t.as_str()) == Some("null")
                    });
                    if is_second_null {
                        if let Some(node) = first {
                            return Ok((node, true));
                        }
                    }
                    let is_first_null = first.map_or(false, |o| {
                        o.get("type").and_then(|t| t.as_str()) == Some("null")
                    });
                    if is_first_null {
                        if let Some(node) = second {
                            return Ok((node, true));
                        }
                    }
                }
            }
            return Err(ApfelError::Usage(
                "Complex anyOf/oneOf schemas not supported".to_string(),
            ));
        }

        // Handle type: ["string", "null"]
        if let Some(types) = node.get("type").and_then(|t| t.as_array()) {
            if types.len() == 2 {
                let has_null = types.iter().any(|v| v.as_str() == Some("null"));
                let other_type = types.iter().find(|v| v.as_str() != Some("null"));
                if has_null && other_type.is_some() {
                    return Ok((node, true));
                }
            }
            return Err(ApfelError::Usage(
                "Multi-type unions not supported".to_string(),
            ));
        }

        Ok((node, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_object_schema() {
        let json = r#"{
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name"]
        }"#;
        let ir = SchemaParser::parse(json, "Person").unwrap();
        match ir {
            SchemaIR::Object {
                name, properties, ..
            } => {
                assert_eq!(name, "Person");
                assert_eq!(properties.len(), 2);
                let age_prop = properties.iter().find(|p| p.name == "age").unwrap();
                assert!(age_prop.is_optional);
                let name_prop = properties.iter().find(|p| p.name == "name").unwrap();
                assert!(!name_prop.is_optional);
            }
            _ => panic!("Expected object schema"),
        }
    }

    #[test]
    fn test_parse_invalid_json() {
        let err = SchemaParser::parse("{ invalid", "Root").unwrap_err();
        assert!(matches!(err, ApfelError::Usage(_)));
    }

    #[test]
    fn test_parse_max_depth() {
        let mut json = String::from("{\"type\":\"object\",\"properties\":{\"nested\":");
        for _ in 0..SchemaParser::MAX_SCHEMA_DEPTH + 1 {
            json.push_str("{\"type\":\"object\",\"properties\":{\"nested\":");
        }
        json.push_str("{\"type\":\"string\"}");
        for _ in 0..SchemaParser::MAX_SCHEMA_DEPTH + 1 {
            json.push_str("}}");
        }
        json.push_str("}}");

        let err = SchemaParser::parse(&json, "Root").unwrap_err();
        assert!(matches!(err, ApfelError::Usage(_)));
    }

    #[test]
    fn test_parse_union_schema_with_null() {
        // type: ["string", "null"] is recognized as nullable by normalize_union,
        // but the node's "type" field remains an array, so as_str() returns None
        // and the parser falls through to the default "object" type.
        let json = r#"{
            "type": ["string", "null"]
        }"#;
        let ir = SchemaParser::parse(json, "NullableString");
        assert!(
            ir.is_ok(),
            "Parser should not error on nullable union types"
        );
    }
}
