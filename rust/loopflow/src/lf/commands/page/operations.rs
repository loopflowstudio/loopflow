//! Partial-to-strict `data-lf` Machine `Operation` expansion and validation.

use std::collections::HashMap;
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::Value;

use crate::server::schema;
use crate::server::wire::Operation;

use super::html::{enclosing_tag_name, escape_single_quoted_attr, line_at, unescape_html_attr};

const REQUEST_ID_SENTINEL: &str = "__LF_REQUEST_ID__";

static DATA_LF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)data-lf\s*=\s*(?:'([^']*)'|"([^"]*)")"#).expect("static data-lf regex")
});

/// Expand every `data-lf='…'` attribute in `input` into complete, strict wire
/// `Operation` JSON validated against `crate::server::wire::Operation`.
pub(super) fn expand(repo_root: &Path, input: &str) -> Result<String> {
    let mut schemas: Option<HashMap<String, Value>> = None;
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    for caps in DATA_LF_RE.captures_iter(input) {
        let full = caps.get(0).expect("full match");
        out.push_str(&input[cursor..full.start()]);
        let line = line_at(input, full.start());
        let elem = enclosing_tag_name(input, full.start()).unwrap_or("element");
        let raw_attr = caps
            .get(1)
            .or_else(|| caps.get(2))
            .map(|m| m.as_str())
            .unwrap_or("");
        let unescaped = unescape_html_attr(raw_attr);
        let strict_json = compile_operation_json(repo_root, &mut schemas, &unescaped, elem, line)?;
        out.push_str("data-lf='");
        out.push_str(&escape_single_quoted_attr(&strict_json));
        out.push('\'');
        cursor = full.end();
    }
    out.push_str(&input[cursor..]);
    Ok(out)
}

fn operation_schemas() -> HashMap<String, Value> {
    let doc = schema::describe();
    let mut ops = HashMap::new();
    for group in ["Write", "Read", "ServerOperation", "MeasuredOperation"] {
        if let Some(arms) = doc["$defs"][group]["oneOf"].as_array() {
            for arm in arms {
                if let Some(kind) = arm["properties"]["kind"]["const"].as_str() {
                    ops.insert(kind.to_owned(), arm.clone());
                }
            }
        }
    }
    ops
}

fn compile_operation_json(
    repo_root: &Path,
    schemas: &mut Option<HashMap<String, Value>>,
    raw: &str,
    elem: &str,
    line: usize,
) -> Result<String> {
    let mut value: Value = serde_json::from_str(raw).map_err(|err| {
        anyhow::anyhow!("line {line}: <{elem} data-lf=…> is not valid JSON: {err}")
    })?;
    let Some(object) = value.as_object_mut() else {
        bail!("line {line}: <{elem} data-lf=…> must be a JSON object");
    };
    let Some(kind) = object
        .get("kind")
        .and_then(Value::as_str)
        .map(str::to_owned)
    else {
        bail!("line {line}: <{elem} data-lf=…> requires a string `kind` field");
    };
    let map = schemas.get_or_insert_with(operation_schemas);
    let Some(arm) = map.get(&kind) else {
        bail!("line {line}: <{elem} data-lf=…> unknown operation kind `{kind}`");
    };

    // Expand missing required properties from the operation's generated JSON Schema.
    let properties = arm.get("properties").and_then(Value::as_object);
    let required: Vec<String> = arm
        .get("required")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    for field in required {
        if object.contains_key(&field) {
            continue;
        }
        if field == "directory" {
            object.insert(
                field,
                Value::String(repo_root.to_string_lossy().into_owned()),
            );
            continue;
        }
        if field == "request_id" {
            object.insert(field, Value::String(REQUEST_ID_SENTINEL.to_owned()));
            continue;
        }
        if let Some(default_value) = properties
            .and_then(|props| props.get(&field))
            .and_then(default_for_schema)
        {
            object.insert(field, default_value);
        }
    }

    // Strict wire validation via deserializing `Operation` (enforces `deny_unknown_fields`).
    let validated: Operation = serde_json::from_value(value).map_err(|err| {
        anyhow::anyhow!("line {line}: <{elem} data-lf=…> invalid `{kind}` operation: {err}")
    })?;
    serde_json::to_string(&validated).context("serialize validated Operation JSON")
}

fn default_for_schema(schema: &Value) -> Option<Value> {
    if schema_allows_null(schema) {
        return Some(Value::Null);
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("boolean") => Some(Value::Bool(false)),
        Some("array") => Some(Value::Array(Vec::new())),
        _ => None,
    }
}

fn schema_allows_null(schema: &Value) -> bool {
    if let Some(ty) = schema.get("type") {
        if ty.as_str() == Some("null")
            || ty
                .as_array()
                .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some("null")))
        {
            return true;
        }
    }
    ["anyOf", "oneOf"].iter().any(|key| {
        schema
            .get(*key)
            .and_then(Value::as_array)
            .is_some_and(|variants| variants.iter().any(schema_allows_null))
    })
}

#[cfg(test)]
mod tests {
    use super::{compile_operation_json, REQUEST_ID_SENTINEL};
    use std::path::Path;

    #[test]
    fn data_lf_expands_partial_task_create_into_strict_operation_and_rejects_unknown_fields() {
        let mut schemas = None;
        let strict = compile_operation_json(
            Path::new("/tmp/repo"),
            &mut schemas,
            r#"{"kind":"task.create","title":"Prune retry log rows"}"#,
            "button",
            12,
        )
        .unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&strict).unwrap();
        assert_eq!(parsed["kind"], "task.create");
        assert_eq!(parsed["directory"], "/tmp/repo");
        assert_eq!(parsed["request_id"], REQUEST_ID_SENTINEL);
        assert_eq!(parsed["title"], "Prune retry log rows");
        assert_eq!(parsed["run"], false);
        assert!(parsed["wave"].is_null());

        let unknown_field = compile_operation_json(
            Path::new("/tmp/repo"),
            &mut schemas,
            r#"{"kind":"task.create","title":"Ok","bogus":1}"#,
            "button",
            14,
        )
        .unwrap_err()
        .to_string();
        assert!(unknown_field.contains("line 14"), "{unknown_field}");
        assert!(unknown_field.contains("<button"), "{unknown_field}");

        let unknown_kind = compile_operation_json(
            Path::new("/tmp/repo"),
            &mut schemas,
            r#"{"kind":"task.fly"}"#,
            "button",
            19,
        )
        .unwrap_err()
        .to_string();
        assert!(unknown_kind.contains("line 19"), "{unknown_kind}");
        assert!(unknown_kind.contains("task.fly"), "{unknown_kind}");
    }
}
