// ==================== 工具 schema 通用归一化 ====================
// OpenAI 兼容通道（含 DeepSeek / Kimi / WorkBuddy 等）把工具 parameters 原样转发，
// 上游校验器对超集写法容忍度不一：MCP 远端 schema 常带 $schema、$id 这类非参数关键字，
// 也常用 ["string","number"] 这种联合 type。内置工具定义从未使用这些写法，所以平时不暴露，
// 一旦挂上第三方 MCP 就会整轮被拒。
//
// 归一化原则：只放宽、不收窄。任何改写都必须保证「原本合法的调用现在仍然合法」，
// 绝不允许把可选变必填、把任意元素收成 string、把 nullable 收掉。

/// 只允许出现在文档根、不属于函数参数契约的关键字
const SCHEMA_META_KEYS: &[&str] = &[
    "$schema",
    "$id",
    "$anchor",
    "$comment",
    "examples",
    "$vocabulary",
];

/// 值是 schema 列表的联合关键字
const SCHEMA_UNION_KEYS: &[&str] = &["allOf", "anyOf", "oneOf"];

/// 值是单个 schema 的关键字（additionalProperties 的对象形态也在此递归）
const SCHEMA_SINGLE_KEYS: &[&str] = &[
    "items",
    "prefixItems",
    "additionalItems",
    "unevaluatedItems",
    "contains",
    "contentSchema",
    "not",
    "if",
    "then",
    "else",
    "propertyNames",
    "additionalProperties",
    "unevaluatedProperties",
];

/// 值是名称到 schema 映射的关键字
const SCHEMA_MAP_KEYS: &[&str] = &["properties", "patternProperties", "dependentSchemas"];

/// 允许保留布尔写法的关键字（true/false 在这里是合法 schema 简写）
const SCHEMA_BOOL_OK_KEYS: &[&str] = &["additionalProperties", "unevaluatedProperties"];

const JSON_SCHEMA_TYPES: &[&str] = &[
    "string", "number", "integer", "boolean", "object", "array", "null",
];

/// 归一化一个工具的参数 schema，原地修改。
pub(crate) fn normalize_tool_schema(schema: &mut serde_json::Value) {
    // 先把 $ref 内联，否则剥离 $defs/definitions 会制造悬空引用
    inline_local_schema_refs(schema);
    normalize_schema_value(schema, 0);
}

/// 把本地 $ref（#/$defs/x、#/definitions/x）就地内联，再丢弃定义容器。
/// 引用的定义按「只补缺失键」写回，不覆盖调用方已有的约束。
fn inline_local_schema_refs(schema: &mut serde_json::Value) {
    if !schema.is_object() {
        return;
    }
    // 快照必须在剥离定义容器之前，否则 $ref 将无处可解析
    let snapshot = schema.clone();
    let Some(root) = schema.as_object_mut() else {
        return;
    };
    let mut definitions = serde_json::Map::<String, serde_json::Value>::new();
    for key in ["$defs", "definitions"] {
        if let Some(serde_json::Value::Object(value)) = root.remove(key) {
            definitions.extend(value);
        }
    }
    if definitions.is_empty() {
        return;
    }
    let mut visited = std::collections::HashSet::<String>::new();
    resolve_refs_with(schema, &definitions, &snapshot, &mut visited, 0);
}

fn resolve_refs_with(
    value: &mut serde_json::Value,
    definitions: &serde_json::Map<String, serde_json::Value>,
    root: &serde_json::Value,
    visited: &mut std::collections::HashSet<String>,
    depth: usize,
) {
    if depth > 64 {
        return;
    }
    match value {
        serde_json::Value::Object(map) => {
            if let Some(reference) = map.get("$ref").and_then(|item| item.as_str()) {
                let reference = reference.to_string();
                if let Some(resolved) = lookup_local_ref(root, &reference) {
                    let name = reference.rsplit('/').next().unwrap_or_default().to_string();
                    if visited.insert(name.clone()) {
                        let mut resolved = resolved;
                        resolve_refs_with(&mut resolved, definitions, root, visited, depth + 1);
                        visited.remove(&name);
                        map.remove("$ref");
                        if let serde_json::Value::Object(inner) = resolved {
                            for (key, value) in inner {
                                map.entry(key).or_insert(value);
                            }
                        }
                    }
                }
            }
            for child in map.values_mut() {
                resolve_refs_with(child, definitions, root, visited, depth + 1);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                resolve_refs_with(item, definitions, root, visited, depth + 1);
            }
        }
        _ => {}
    }
}

/// 解析 #/ 开头的本地引用路径。
fn lookup_local_ref(root: &serde_json::Value, reference: &str) -> Option<serde_json::Value> {
    let path = reference.strip_prefix("#/")?;
    let mut current = root;
    for segment in path.split('/') {
        let segment = segment.replace("~1", "/").replace("~0", "~");
        current = current.get(&segment)?;
    }
    Some(current.clone())
}

fn normalize_schema_value(schema: &mut serde_json::Value, depth: usize) {
    if depth > 64 {
        return;
    }
    match schema {
        serde_json::Value::Object(map) => normalize_schema_object(map, depth),
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                normalize_schema_value(item, depth + 1);
            }
        }
        _ => {}
    }
}

fn normalize_schema_object(
    map: &mut serde_json::Map<String, serde_json::Value>,
    depth: usize,
) {
    for key in SCHEMA_META_KEYS {
        map.remove(*key);
    }

    normalize_schema_type(map);
    normalize_schema_enum(map);
    normalize_schema_required(map);

    // 数组缺 items 时补一个开放 schema，表达「任意元素」，不收窄元素类型
    if map.get("type").and_then(serde_json::Value::as_str) == Some("array")
        && !map.contains_key("items")
        && !map.contains_key("prefixItems")
    {
        map.insert("items".to_string(), serde_json::json!({}));
    }

    for key in SCHEMA_SINGLE_KEYS {
        let Some(value) = map.get_mut(*key) else {
            continue;
        };
        let keep_bool = SCHEMA_BOOL_OK_KEYS.contains(key);
        normalize_bool_schema(value, keep_bool);
        normalize_schema_value(value, depth + 1);
    }

    for key in SCHEMA_UNION_KEYS {
        let Some(serde_json::Value::Array(branches)) = map.get_mut(*key) else {
            continue;
        };
        for branch in branches.iter_mut() {
            normalize_bool_schema(branch, false);
            normalize_schema_value(branch, depth + 1);
        }
    }

    for key in SCHEMA_MAP_KEYS {
        let Some(serde_json::Value::Object(children)) = map.get_mut(*key) else {
            continue;
        };
        for child in children.values_mut() {
            normalize_bool_schema(child, false);
            normalize_schema_value(child, depth + 1);
        }
    }
}

/// 布尔位 schema（`items: true`）改写成等价的对象写法，`false` 用 `{"not": {}}`。
fn normalize_bool_schema(value: &mut serde_json::Value, keep_bool: bool) {
    if keep_bool {
        return;
    }
    match value {
        serde_json::Value::Bool(true) => {
            *value = serde_json::json!({});
        }
        serde_json::Value::Bool(false) => {
            *value = serde_json::json!({ "not": {} });
        }
        _ => {}
    }
}

/// 联合 type 收敛为单一 type，语义保持「原本合法的仍然合法」。
/// - `["string","null"]`：保留 string 并放开 null（用 anyOf 表达两者都收）
/// - `["string","number"]`：改写成 anyOf 分支，语义等价
/// - 含未知类型名：剔除；全无则删掉 type 交给上游按任意类型处理
fn normalize_schema_type(map: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(serde_json::Value::Array(types)) = map.get("type") else {
        return;
    };

    let mut allowed = Vec::<String>::new();
    for entry in types.iter() {
        let Some(name) = entry.as_str() else {
            continue;
        };
        let lowered = name.trim().to_ascii_lowercase();
        if JSON_SCHEMA_TYPES.contains(&lowered.as_str()) && !allowed.contains(&lowered) {
            allowed.push(lowered);
        }
    }

    if allowed.is_empty() {
        map.remove("type");
        return;
    }

    if allowed.len() == 1 {
        let single = allowed.remove(0);
        map.insert("type".to_string(), serde_json::json!(single));
        return;
    }

    // 多种类型：改写成 anyOf 分支。父级其余约束（enum/minimum 等）原样留在父级，
    // 与 anyOf 是合取关系，语义不变。
    let branches = allowed
        .into_iter()
        .map(|name| serde_json::json!({ "type": name }))
        .collect::<Vec<_>>();
    map.remove("type");

    match map
        .get_mut("anyOf")
        .and_then(|value| value.as_array_mut())
    {
        Some(list) => list.extend(branches),
        None => {
            map.insert("anyOf".to_string(), serde_json::Value::Array(branches));
        }
    }
}

/// 枚举没有 type 时按取值补一个；多类型取值拆成 anyOf，不丢任何合法取值。
fn normalize_schema_enum(map: &mut serde_json::Map<String, serde_json::Value>) {
    let has_type = map.contains_key("type")
        || SCHEMA_UNION_KEYS
            .iter()
            .any(|key| map.contains_key(*key));
    if has_type {
        return;
    }
    let Some(serde_json::Value::Array(values)) = map.get("enum") else {
        return;
    };
    if values.is_empty() {
        map.remove("enum");
        return;
    }

    let mut kinds = Vec::<&str>::new();
    for value in values.iter() {
        let kind = schema_value_kind(value);
        if !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }

    if kinds.len() == 1 {
        map.insert("type".to_string(), serde_json::json!(kinds[0]));
        return;
    }

    // 多类型枚举：按类型分组成 anyOf，原 enum 拆散，全部取值都保留
    let mut grouped = std::collections::BTreeMap::<String, Vec<serde_json::Value>>::new();
    for value in values.iter() {
        grouped
            .entry(schema_value_kind(value).to_string())
            .or_default()
            .push(value.clone());
    }
    map.remove("enum");
    map.insert(
        "anyOf".to_string(),
        serde_json::Value::Array(
            grouped
                .into_iter()
                .map(|(kind, values)| serde_json::json!({ "type": kind, "enum": values }))
                .collect(),
        ),
    );
}

fn schema_value_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Number(number) => {
            if number.is_i64() || number.is_u64() {
                "integer"
            } else {
                "number"
            }
        }
    }
}

/// required 必须是字符串数组；只有在能确证「该名字不在 properties 里」时才剔除。
/// 无 properties 时（属性由 allOf 等组合声明）不做裁剪，避免误删合法必填。
fn normalize_schema_required(map: &mut serde_json::Map<String, serde_json::Value>) {
    let Some(serde_json::Value::Array(items)) = map.get("required") else {
        if map.contains_key("required") {
            map.remove("required");
        }
        return;
    };

    // 空数组是合法写法，保持原样，避免改动既有内置工具的字节形态
    if items.is_empty() {
        return;
    }

    let Some(properties) = map
        .get("properties")
        .and_then(|value| value.as_object())
        .cloned()
    else {
        // 没有 properties 可对照：只剔除类型不对的项，不按名字裁剪
        let names = items
            .iter()
            .filter(|item| item.is_string())
            .cloned()
            .collect::<Vec<_>>();
        if names.is_empty() {
            map.remove("required");
        } else {
            map.insert("required".to_string(), serde_json::Value::Array(names));
        }
        return;
    };

    let has_composed_properties = SCHEMA_UNION_KEYS
        .iter()
        .any(|key| map.contains_key(*key));
    let names = items
        .iter()
        .filter_map(|item| item.as_str())
        .filter(|name| properties.contains_key(*name) || has_composed_properties)
        .map(|name| serde_json::json!(name))
        .collect::<Vec<_>>();
    if names.is_empty() {
        map.remove("required");
    } else {
        map.insert("required".to_string(), serde_json::Value::Array(names));
    }
}

#[cfg(test)]
mod tool_schema_normalization_tests {
    use super::normalize_tool_schema;

    #[test]
    fn should_strip_schema_meta_keys_at_every_depth() {
        // civitai MCP 的每个工具根 schema 都带 $schema，内置工具从不带
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "$schema": "http://json-schema.org/draft-07/schema#"
                }
            },
            "additionalProperties": false,
            "$schema": "http://json-schema.org/draft-07/schema#"
        });

        normalize_tool_schema(&mut schema);

        assert!(schema.get("$schema").is_none());
        assert!(schema.pointer("/properties/query/$schema").is_none());
        assert_eq!(
            schema.pointer("/properties/query/type").and_then(serde_json::Value::as_str),
            Some("string")
        );
        assert_eq!(
            schema.get("additionalProperties").and_then(serde_json::Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn should_split_union_type_into_any_of() {
        // toggle_follow_user / mark_notifications_read / send_direct_message 的真实形状
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "user": { "type": ["string", "number"], "description": "Target user: numeric id or username" }
            },
            "required": ["user"]
        });

        normalize_tool_schema(&mut schema);

        let user = schema.pointer("/properties/user").expect("user 属性应保留");
        assert!(user.get("type").is_none());
        let branches = user
            .get("anyOf")
            .and_then(serde_json::Value::as_array)
            .expect("联合类型应改写为 anyOf");
        assert_eq!(branches.len(), 2);
        assert_eq!(
            user.get("description").and_then(serde_json::Value::as_str),
            Some("Target user: numeric id or username")
        );
    }

    #[test]
    fn should_keep_null_acceptable_when_collapsing_nullable_union() {
        // 放宽优先：nullable 不能因为收敛而丢掉 null
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": {
                "mask": { "type": ["string", "null"] }
            }
        });

        normalize_tool_schema(&mut schema);

        let mask = schema.pointer("/properties/mask").expect("mask 属性应保留");
        let branches = mask
            .get("anyOf")
            .and_then(serde_json::Value::as_array)
            .expect("nullable 联合应改写为 anyOf 并保留 null");
        let kinds = branches
            .iter()
            .filter_map(|branch| branch.get("type").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"string"));
        assert!(kinds.contains(&"null"));
    }

    #[test]
    fn should_inline_local_ref_before_dropping_definitions() {
        // 不能删掉 $defs 却留下悬空 $ref
        let mut schema = serde_json::json!({
            "type": "object",
            "$defs": {
                "Id": { "type": "string", "description": "标识" }
            },
            "properties": {
                "id": { "$ref": "#/$defs/Id", "description": "目标" }
            }
        });

        normalize_tool_schema(&mut schema);

        assert!(schema.get("$defs").is_none());
        let id = schema.pointer("/properties/id").expect("id 属性应保留");
        assert!(id.get("$ref").is_none(), "$ref 必须已内联");
        assert_eq!(
            id.get("type").and_then(serde_json::Value::as_str),
            Some("string")
        );
        // 调用方已有的 description 不被定义的覆盖
        assert_eq!(
            id.get("description").and_then(serde_json::Value::as_str),
            Some("目标")
        );
    }

    #[test]
    fn should_recurse_into_additional_properties_object_form() {
        let mut schema = serde_json::json!({
            "type": "object",
            "additionalProperties": {
                "$schema": "http://json-schema.org/draft-07/schema#",
                "type": ["string", "null"]
            }
        });

        normalize_tool_schema(&mut schema);

        let extra = schema.get("additionalProperties").expect("additionalProperties 应保留");
        assert!(extra.get("$schema").is_none());
        assert!(extra.get("type").is_none());
        assert!(extra.get("anyOf").is_some());
    }

    #[test]
    fn should_recurse_into_prefix_items() {
        let mut schema = serde_json::json!({
            "type": "array",
            "prefixItems": [
                { "type": ["string", "number"] }
            ]
        });

        normalize_tool_schema(&mut schema);

        let first = schema
            .pointer("/prefixItems/0")
            .expect("prefixItems 应保留");
        assert!(first.get("anyOf").is_some(), "prefixItems 内的联合类型必须被处理");
    }

    #[test]
    fn should_not_narrow_element_type_for_array_without_items() {
        // 补 {"type":"string"} 会把合法对象数组判成非法
        let mut schema = serde_json::json!({
            "type": "array",
            "description": "规则对象列表"
        });

        normalize_tool_schema(&mut schema);

        let items = schema.get("items").expect("应补 items");
        assert!(
            items.as_object().is_some_and(|map| map.is_empty()),
            "补的 items 必须是开放 schema，不能收窄成 string"
        );
    }

    #[test]
    fn should_keep_required_when_properties_are_composed() {
        let mut schema = serde_json::json!({
            "allOf": [
                { "properties": { "x": { "type": "string" } } }
            ],
            "required": ["x"]
        });

        normalize_tool_schema(&mut schema);

        let required = schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .expect("组合声明下的 required 不能被误删");
        assert_eq!(required, &vec![serde_json::json!("x")]);
    }

    #[test]
    fn should_drop_required_names_missing_from_properties() {
        let mut schema = serde_json::json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query", "ghost"]
        });

        normalize_tool_schema(&mut schema);

        let required = schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .expect("required 应保留");
        assert_eq!(required, &vec![serde_json::json!("query")]);
    }

    #[test]
    fn should_keep_empty_required_array() {
        // recall / get_session / team_info 用的就是空数组形态
        let original = serde_json::json!({
            "type": "object",
            "properties": { "keyword": { "type": "string" } },
            "required": [],
            "additionalProperties": false
        });
        let mut schema = original.clone();

        normalize_tool_schema(&mut schema);

        assert_eq!(schema, original);
    }

    #[test]
    fn should_keep_valid_schema_untouched() {
        // 内置工具现有形状必须零改动，避免任何既有请求体发生变化
        let original = serde_json::json!({
            "type": "object",
            "properties": {
                "script": { "type": "string", "description": "桌面脚本文本，一行一个动作。" },
                "timeout_ms": { "type": "integer", "minimum": 1, "default": 300000 }
            },
            "required": ["script"],
            "additionalProperties": false
        });
        let mut schema = original.clone();

        normalize_tool_schema(&mut schema);

        assert_eq!(schema, original);
    }
}
