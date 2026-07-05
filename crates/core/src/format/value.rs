//! toml_edit value builders shared by `write` and `sync`.

use toml_edit::{Array, DocumentMut, InlineTable, Value};

use crate::CoreError;
use crate::model::{Assert, MultipartPart, Pair, PartKind, VarValue};

/// Build a Value rendering as a literal multiline string (`'''`).
///
/// toml_edit has no first-class repr setter, so we parse a snippet and steal
/// the value. Falls back to a plain (escaped) basic string when the text
/// contains `'''` or control characters a literal string cannot hold.
pub fn multiline(text: &str) -> Value {
    let has_bad_control = text
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t');
    if text.contains("'''") || has_bad_control || text.is_empty() {
        return Value::from(text);
    }
    // No newline is appended: `'''` may close right after the last line, so the
    // stored string round-trips byte-for-byte (bodies without a trailing \n
    // must be sent exactly as written). The leading newline is trimmed by TOML.
    let snippet = format!("v = '''\n{text}'''");
    match snippet.parse::<DocumentMut>() {
        Ok(doc) => doc["v"].as_value().expect("just parsed").clone(),
        // extremely defensive: fall back to escaped basic string
        Err(_) => Value::from(text),
    }
}

/// JSON-shaped var value → TOML value. Nulls are not representable.
pub fn scalar(v: &VarValue) -> Result<Value, CoreError> {
    match v {
        VarValue::Null => Err(CoreError::Invalid(
            "null is not representable in TOML — use an empty string".into(),
        )),
        VarValue::Bool(b) => Ok(Value::from(*b)),
        VarValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::from(i))
            } else {
                Ok(Value::from(n.as_f64().unwrap_or(0.0)))
            }
        }
        VarValue::String(s) => Ok(Value::from(s.as_str())),
        VarValue::Array(items) => {
            let mut arr = Array::new();
            for item in items {
                arr.push_formatted(scalar(item)?);
            }
            Ok(Value::Array(arr))
        }
        VarValue::Object(map) => {
            let mut it = InlineTable::new();
            for (k, val) in map {
                it.insert(k, scalar(val)?);
            }
            Ok(Value::InlineTable(it))
        }
    }
}

pub fn pair_inline(p: &Pair) -> InlineTable {
    let mut it = InlineTable::new();
    it.insert("name", p.name.as_str().into());
    it.insert("value", p.value.as_str().into());
    if !p.enabled {
        it.insert("enabled", false.into());
    }
    it
}

pub fn part_inline(p: &MultipartPart) -> InlineTable {
    let mut it = InlineTable::new();
    it.insert("name", p.name.as_str().into());
    it.insert(
        "kind",
        match p.kind {
            PartKind::Text => "text".into(),
            PartKind::File => "file".into(),
        },
    );
    if let Some(v) = &p.value {
        it.insert("value", v.as_str().into());
    }
    if let Some(path) = &p.path {
        it.insert("path", path.as_str().into());
    }
    if let Some(ct) = &p.content_type {
        it.insert("content_type", ct.as_str().into());
    }
    if !p.enabled {
        it.insert("enabled", false.into());
    }
    it
}

/// Render a list of inline tables as a one-item-per-line array:
/// ```toml
/// headers = [
///   { name = "Accept", value = "application/json" },
/// ]
/// ```
pub fn items_array<T>(items: &[T], build: impl Fn(&T) -> InlineTable) -> Value {
    let mut arr = Array::new();
    for item in items {
        let mut v = Value::InlineTable(build(item));
        v.decor_mut().set_prefix("\n  ");
        arr.push_formatted(v);
    }
    arr.set_trailing_comma(true);
    arr.set_trailing("\n");
    Value::Array(arr)
}

/// Callers must run `write::validate_asserts` first (no null values).
pub fn assert_inline(a: &Assert) -> InlineTable {
    let mut it = InlineTable::new();
    it.insert("expr", a.expr.as_str().into());
    let op = serde_json::to_value(a.op)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .expect("AssertOp serializes to a string");
    it.insert("op", op.as_str().into());
    if let Some(v) = &a.value {
        it.insert(
            "value",
            scalar(v).expect("validated: no null assert values"),
        );
    }
    if !a.enabled {
        it.insert("enabled", false.into());
    }
    it
}

/// Single-line string array: `scopes = ["read", "write"]`.
pub fn string_array(items: &[String]) -> Value {
    let mut arr = Array::new();
    for s in items {
        arr.push(s.as_str());
    }
    Value::Array(arr)
}
