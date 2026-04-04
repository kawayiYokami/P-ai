fn gemini_to_openapi_schema(schema: &mut serde_json::Value) {
    let serde_json::Value::Object(map) = schema else {
        return;
    };
    let definitions = gemini_extract_schema_defs(map);
    if !definitions.is_empty() {
        let mut visited = std::collections::HashSet::<String>::new();
        gemini_resolve_schema_refs(map, &definitions, &mut visited);
    }
    gemini_simplify_schema_object(map);
    let filtered = gemini_filter_schema_object(std::mem::take(map));
    *map = filtered;
}

fn gemini_extract_schema_defs(
    map: &mut serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let mut definitions = serde_json::Map::<String, serde_json::Value>::new();
    for key in ["$defs", "definitions"] {
        if let Some(serde_json::Value::Object(value)) = map.remove(key) {
            definitions.extend(value);
        }
    }
    definitions
}

fn gemini_resolve_schema_refs(
    map: &mut serde_json::Map<String, serde_json::Value>,
    definitions: &serde_json::Map<String, serde_json::Value>,
    visited: &mut std::collections::HashSet<String>,
) {
    if let Some(serde_json::Value::String(reference)) = map.get("$ref") {
        let definition_name = reference.rsplit('/').next().unwrap_or("").to_string();
        if !definition_name.is_empty() && !visited.contains(&definition_name) {
            if let Some(definition) = definitions.get(&definition_name) {
                visited.insert(definition_name.clone());
                let mut resolved = definition.clone();
                if let serde_json::Value::Object(inner) = &mut resolved {
                    gemini_resolve_schema_refs(inner, definitions, visited);
                }
                visited.remove(&definition_name);
                map.remove("$ref");
                if let serde_json::Value::Object(inner) = resolved {
                    for (key, value) in inner {
                        map.entry(key).or_insert(value);
                    }
                }
                return;
            }
        }
    }

    for value in map.values_mut() {
        match value {
            serde_json::Value::Object(child) => {
                gemini_resolve_schema_refs(child, definitions, visited)
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    if let serde_json::Value::Object(child) = item {
                        gemini_resolve_schema_refs(child, definitions, visited);
                    }
                }
            }
            _ => {}
        }
    }
}

fn gemini_is_null_schema(value: &serde_json::Value) -> bool {
    value
        .as_object()
        .and_then(|map| map.get("type"))
        .and_then(serde_json::Value::as_str)
        == Some("null")
}

fn gemini_flatten_schema_composites(map: &mut serde_json::Map<String, serde_json::Value>) {
    for keyword in ["allOf", "anyOf", "oneOf"] {
        let Some(serde_json::Value::Array(variants)) = map.remove(keyword) else {
            continue;
        };
        let non_null = variants
            .into_iter()
            .filter(|value| !gemini_is_null_schema(value))
            .collect::<Vec<_>>();
        if non_null.len() == 1 {
            if let Some(serde_json::Value::Object(inner)) = non_null.into_iter().next() {
                for (key, value) in inner {
                    map.entry(key).or_insert(value);
                }
            }
        } else if !non_null.is_empty() {
            map.insert(keyword.to_string(), serde_json::Value::Array(non_null));
        }
    }
}

fn gemini_normalize_nullable_schema_type(map: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(serde_json::Value::Array(types)) = map.get("type") else {
        return;
    };
    let non_null = types
        .iter()
        .filter(|value| value.as_str() != Some("null"))
        .cloned()
        .collect::<Vec<_>>();
    if non_null.len() == 1 {
        if let Some(single) = non_null.into_iter().next() {
            map.insert("type".to_string(), single);
        }
    }
}

fn gemini_recurse_schema_children(map: &mut serde_json::Map<String, serde_json::Value>) {
    if let Some(serde_json::Value::Object(properties)) = map.get_mut("properties") {
        for property in properties.values_mut() {
            if let serde_json::Value::Object(inner) = property {
                gemini_simplify_schema_object(inner);
            }
        }
    }

    if let Some(items) = map.get_mut("items") {
        match items {
            serde_json::Value::Object(inner) => gemini_simplify_schema_object(inner),
            serde_json::Value::Array(values) => {
                for value in values {
                    if let serde_json::Value::Object(inner) = value {
                        gemini_simplify_schema_object(inner);
                    }
                }
            }
            _ => {}
        }
    }

    if let Some(serde_json::Value::Array(prefix_items)) = map.get_mut("prefixItems") {
        for item in prefix_items {
            if let serde_json::Value::Object(inner) = item {
                gemini_simplify_schema_object(inner);
            }
        }
    }

    for keyword in ["allOf", "anyOf", "oneOf"] {
        if let Some(serde_json::Value::Array(values)) = map.get_mut(keyword) {
            for value in values {
                if let serde_json::Value::Object(inner) = value {
                    gemini_simplify_schema_object(inner);
                }
            }
        }
    }
}

fn gemini_take_schema_value(
    primary: &serde_json::Map<String, serde_json::Value>,
    fallback: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Option<serde_json::Value> {
    primary
        .get(key)
        .cloned()
        .or_else(|| fallback.and_then(|value| value.get(key).cloned()))
}

fn gemini_extract_schema_source_from_composition(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    for keyword in ["anyOf", "oneOf", "allOf"] {
        let Some(serde_json::Value::Array(variants)) = map.get(keyword) else {
            continue;
        };
        for variant in variants {
            let Some(object) = variant.as_object() else {
                continue;
            };
            if object.get("type").and_then(serde_json::Value::as_str) == Some("null") {
                continue;
            }
            return Some(object.clone());
        }
    }
    None
}

fn gemini_extract_schema_type(
    map: &serde_json::Map<String, serde_json::Value>,
) -> Option<String> {
    if let Some(serde_json::Value::String(type_name)) = map.get("type") {
        return Some(type_name.clone());
    }

    if let Some(serde_json::Value::Array(type_names)) = map.get("type") {
        if let Some(type_name) = type_names
            .iter()
            .filter_map(serde_json::Value::as_str)
            .find(|value| *value != "null")
        {
            return Some(type_name.to_string());
        }
    }

    if let Some(source) = gemini_extract_schema_source_from_composition(map) {
        if let Some(type_name) = gemini_extract_schema_type(&source) {
            return Some(type_name);
        }
        if source.contains_key("properties") {
            return Some("object".to_string());
        }
    }

    if map.contains_key("properties") {
        return Some("object".to_string());
    }

    None
}

fn gemini_filter_schema_object(
    map: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    let fallback = gemini_extract_schema_source_from_composition(&map);
    let fallback_ref = fallback.as_ref();
    let mut filtered = serde_json::Map::<String, serde_json::Value>::new();

    if let Some(type_name) = gemini_extract_schema_type(&map).filter(|value| !value.is_empty()) {
        filtered.insert("type".to_string(), serde_json::Value::String(type_name));
    }

    for key in ["format", "title", "description"] {
        if let Some(serde_json::Value::String(value)) =
            gemini_take_schema_value(&map, fallback_ref, key)
        {
            filtered.insert(key.to_string(), serde_json::Value::String(value));
        }
    }

    if let Some(serde_json::Value::Bool(value)) =
        gemini_take_schema_value(&map, fallback_ref, "nullable")
    {
        filtered.insert("nullable".to_string(), serde_json::Value::Bool(value));
    }

    if let Some(serde_json::Value::Array(values)) =
        gemini_take_schema_value(&map, fallback_ref, "enum")
    {
        filtered.insert("enum".to_string(), serde_json::Value::Array(values));
    }

    for key in ["maxItems", "minItems", "minimum", "maximum"] {
        if let Some(serde_json::Value::Number(value)) =
            gemini_take_schema_value(&map, fallback_ref, key)
        {
            filtered.insert(key.to_string(), serde_json::Value::Number(value));
        }
    }

    if let Some(serde_json::Value::Array(values)) =
        gemini_take_schema_value(&map, fallback_ref, "propertyOrdering")
    {
        filtered.insert("propertyOrdering".to_string(), serde_json::Value::Array(values));
    }

    if let Some(serde_json::Value::Object(properties)) =
        gemini_take_schema_value(&map, fallback_ref, "properties")
    {
        let filtered_properties = properties
            .into_iter()
            .filter_map(|(name, value)| match value {
                serde_json::Value::Object(inner) => {
                    Some((name, serde_json::Value::Object(gemini_filter_schema_object(inner))))
                }
                _ => None,
            })
            .collect::<serde_json::Map<String, serde_json::Value>>();
        if !filtered_properties.is_empty() {
            filtered.insert(
                "properties".to_string(),
                serde_json::Value::Object(filtered_properties),
            );
        }
    }

    if let Some(serde_json::Value::Array(required)) =
        gemini_take_schema_value(&map, fallback_ref, "required")
    {
        let filtered_required = required
            .into_iter()
            .filter(|value| value.is_string())
            .collect::<Vec<_>>();
        if !filtered_required.is_empty() {
            filtered.insert("required".to_string(), serde_json::Value::Array(filtered_required));
        }
    }

    if let Some(items_value) = gemini_take_schema_value(&map, fallback_ref, "items") {
        match items_value {
            serde_json::Value::Object(inner) => {
                filtered.insert(
                    "items".to_string(),
                    serde_json::Value::Object(gemini_filter_schema_object(inner)),
                );
            }
            serde_json::Value::Array(values) => {
                if let Some(serde_json::Value::Object(inner)) =
                    values.into_iter().find(|value| value.is_object())
                {
                    filtered.insert(
                        "items".to_string(),
                        serde_json::Value::Object(gemini_filter_schema_object(inner)),
                    );
                }
            }
            _ => {}
        }
    }

    if filtered.get("type").and_then(serde_json::Value::as_str) == Some("array")
        && !filtered.contains_key("items")
    {
        filtered.insert("items".to_string(), serde_json::json!({ "type": "string" }));
    }

    filtered
}

fn gemini_simplify_schema_object(map: &mut serde_json::Map<String, serde_json::Value>) {
    gemini_flatten_schema_composites(map);
    gemini_normalize_nullable_schema_type(map);
    map.remove("$schema");
    map.remove("examples");
    map.remove("example");
    map.remove("additionalProperties");
    gemini_recurse_schema_children(map);
}

#[cfg(test)]
mod gemini_bridge_tests {
    use super::*;

    #[test]
    fn gemini_schema_converter_should_remove_additional_properties() {
        let mut schema = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "name": {
                    "anyOf": [
                        { "type": "string" },
                        { "type": "null" }
                    ]
                }
            }
        });

        gemini_to_openapi_schema(&mut schema);

        assert!(schema.get("additionalProperties").is_none());
        assert_eq!(
            schema.pointer("/properties/name/type").and_then(serde_json::Value::as_str),
            Some("string")
        );
    }

    #[test]
    fn gemini_schema_converter_should_remove_schema_and_examples_fields() {
        let mut schema = serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "examples": [{ "command": "demo" }],
            "properties": {
                "command": {
                    "type": "string",
                    "example": "demo",
                    "examples": ["demo-1", "demo-2"]
                }
            }
        });

        gemini_to_openapi_schema(&mut schema);

        assert!(schema.get("$schema").is_none());
        assert!(schema.get("examples").is_none());
        assert!(schema.pointer("/properties/command/example").is_none());
        assert!(schema.pointer("/properties/command/examples").is_none());
    }

    #[test]
    fn gemini_schema_converter_should_whitelist_gemini_safe_fields() {
        let mut schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "examples": [{ "name": "demo" }],
            "properties": {
                "config": {
                    "allOf": [
                        {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string", "examples": ["demo"] },
                                "enabled": { "type": ["boolean", "null"] }
                            },
                            "required": ["name"],
                            "additionalProperties": false
                        }
                    ]
                },
                "items": {
                    "type": "array",
                    "items": [
                        {
                            "type": "object",
                            "properties": {
                                "value": { "type": "number", "example": 1 }
                            },
                            "additionalProperties": false
                        }
                    ]
                }
            },
            "additionalProperties": false
        });

        gemini_to_openapi_schema(&mut schema);

        assert_eq!(schema.get("$schema"), None);
        assert_eq!(
            schema.pointer("/properties/config/type").and_then(serde_json::Value::as_str),
            Some("object")
        );
        assert_eq!(
            schema
                .pointer("/properties/config/properties/enabled/type")
                .and_then(serde_json::Value::as_str),
            Some("boolean")
        );
        assert!(schema.pointer("/properties/config/additionalProperties").is_none());
        assert_eq!(
            schema
                .pointer("/properties/items/items/type")
                .and_then(serde_json::Value::as_str),
            Some("object")
        );
        assert!(schema.pointer("/properties/items/items/properties/value/example").is_none());
    }
}
