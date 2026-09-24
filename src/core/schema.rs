// ============================================================================
// schema.rs — JSON Schema to SchemaIR parser & validator
// Part of apfel-rs
// ============================================================================

use crate::core::error::ApfelError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertyIR {
    pub name: String,
    pub description: Option<String>,
    pub schema: SchemaIR,
    pub is_optional: bool,
}

pub struct SchemaParser;

impl SchemaParser {
    pub const MAX_SCHEMA_DEPTH: usize = 64;

    pub fn parse(json_str: &str, root_name: &str) -> Result<SchemaIR, ApfelError> {
        let val: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| ApfelError::Usage(format!("Invalid JSON in schema: {}", e)))?;

        let obj = val.as_object().ok_or_else(|| {
            ApfelError::Usage("Schema root must be a JSON object".to_string())
        })?;

        Self::parse_object(obj, root_name, 0)
    }

    fn parse_object(
        obj: &serde_json::Map<String, serde_json::Value>,
        name: &str,
        depth: usize,
    ) -> Result<SchemaIR, ApfelError> {
        if depth > Self::MAX_SCHEMA_DEPTH {
            return Err(ApfelError::Usage(format!(
                "JSON schema exceeds maximum nesting depth of {}",
                Self::MAX_SCHEMA_DEPTH
            )));
        }

        let (node, _nullable) = Self::normalize_union(obj)?;
        let node_type = node
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("object");
        let description = node
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

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
                    let (prop_node, prop_nullable) = Self::normalize_union(prop_obj)?;
                    let child_ir = Self::parse_object(prop_obj, key, depth + 1)?;
                    let child_desc = prop_node
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let is_optional = !required_set.contains(key) || prop_nullable;
                    properties.push(PropertyIR {
                        name: key.clone(),
                        description: child_desc,
                        schema: child_ir,
                        is_optional,
                    });
                }

                Ok(SchemaIR::Object {
                    name: name.to_string(),
                    description,
                    properties,
                })
            }
            "string" => {
                let enum_vals = node
                    .get("enum")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.as_str().map(|s| s.to_string()))
                            .collect()
                    });

                Ok(SchemaIR::String {
                    name: name.to_string(),
                    description,
                    enum_values: enum_vals,
                })
            }
            "integer" => Ok(SchemaIR::Integer {
                name: name.to_string(),
                description,
            }),
            "number" => Ok(SchemaIR::Number {
                name: name.to_string(),
                description,
            }),
            "boolean" => Ok(SchemaIR::Bool {
                name: name.to_string(),
                description,
            }),
            "array" => {
                let items_val = node.get("items").ok_or_else(|| {
                    ApfelError::Usage(format!("Array '{}' is missing 'items' schema", name))
                })?;
                let items_obj = items_val.as_object().ok_or_else(|| {
                    ApfelError::Usage(format!("'items' in array '{}' must be an object", name))
                })?;
                let inner = Self::parse_object(items_obj, &format!("{}_item", name), depth + 1)?;
                Ok(SchemaIR::Array {
                    item_name: name.to_string(),
                    items: Box::new(inner),
                })
            }
            other => Err(ApfelError::Usage(format!("Unsupported schema type: {}", other))),
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
                    if is_second_null && first.is_some() {
                        return Ok((first.unwrap(), true));
                    }
                    let is_first_null = first.map_or(false, |o| {
                        o.get("type").and_then(|t| t.as_str()) == Some("null")
                    });
                    if is_first_null && second.is_some() {
                        return Ok((second.unwrap(), true));
                    }
                }
            }
            return Err(ApfelError::Usage("Complex anyOf/oneOf schemas not supported".to_string()));
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
            return Err(ApfelError::Usage("Multi-type unions not supported".to_string()));
        }

        Ok((node, false))
    }
}
