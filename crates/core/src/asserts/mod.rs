//! Declarative assertions over the response:
//! `{ expr = "res.body.id", op = "isDefined" }`.
//!
//! Selectors: `res.status`, `res.statusText`, `res.responseTime`, `res.size`,
//! `res.headers.<name>` (case-insensitive), `res.body[.path]`.

use serde::Serialize;
use serde_json::Value;

use crate::model::{Assert, AssertOp, ResponseData};
use crate::vars::path::{split_path, walk_path};

#[derive(Debug, Clone, Serialize)]
pub struct AssertResult {
    pub expr: String,
    pub op: AssertOp,
    pub expected: Option<Value>,
    pub actual: Option<Value>,
    pub ok: bool,
    pub message: Option<String>,
}

/// Evaluate enabled asserts. `body_json` is the parsed body when available.
pub fn run_asserts(
    asserts: &[Assert],
    response: &ResponseData,
    body_json: Option<&Value>,
) -> Vec<AssertResult> {
    asserts
        .iter()
        .filter(|a| a.enabled)
        .map(|a| evaluate(a, response, body_json))
        .collect()
}

fn evaluate(assert: &Assert, response: &ResponseData, body_json: Option<&Value>) -> AssertResult {
    let actual = select(&assert.expr, response, body_json);
    let (ok, message) = check(assert.op, actual.as_ref(), assert.value.as_ref());
    AssertResult {
        expr: assert.expr.clone(),
        op: assert.op,
        expected: assert.value.clone(),
        actual,
        ok,
        message,
    }
}

fn select(expr: &str, response: &ResponseData, body_json: Option<&Value>) -> Option<Value> {
    let expr = expr.trim();
    let rest = expr
        .strip_prefix("res.")
        .or_else(|| expr.strip_prefix("response."))?;

    match rest {
        "status" => return Some(Value::from(response.status)),
        "statusText" => return Some(Value::from(response.status_text.clone())),
        "responseTime" => return Some(Value::from(response.timing.total_ms)),
        "size" => return Some(Value::from(response.body.total_size)),
        _ => {}
    }

    if let Some(header_name) = rest.strip_prefix("headers.") {
        return response
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(header_name))
            .map(|(_, v)| Value::from(v.clone()));
    }

    if rest == "body" {
        return match body_json {
            Some(v) => Some(v.clone()),
            None => Some(Value::from(response.body.preview_text())),
        };
    }
    if let Some(path) = rest
        .strip_prefix("body.")
        .or_else(|| rest.strip_prefix("body["))
    {
        // reattach the '[' consumed by strip_prefix for index-first paths
        let path_owned = if rest.starts_with("body[") {
            format!("[{path}")
        } else {
            path.to_string()
        };
        let body = body_json?;
        // walk: fabricate a root token by prefixing a dummy root
        let full = format!("r{}", normalize_path(&path_owned));
        let (_root, segs) = split_path(&full);
        return walk_path(body, &segs).cloned();
    }

    None
}

/// Ensure the path starts with a separator so `split_path` sees only segments.
fn normalize_path(path: &str) -> String {
    if path.starts_with('[') || path.starts_with('.') {
        path.to_string()
    } else {
        format!(".{path}")
    }
}

fn check(op: AssertOp, actual: Option<&Value>, expected: Option<&Value>) -> (bool, Option<String>) {
    use AssertOp::*;

    // existence operators work on the Option itself
    match op {
        IsDefined => return (actual.is_some(), presence_msg(actual, "to be defined")),
        IsUndefined => return (actual.is_none(), presence_msg(actual, "to be undefined")),
        IsNull => {
            let ok = matches!(actual, Some(Value::Null));
            return (ok, presence_msg(actual, "to be null"));
        }
        IsNotNull => {
            let ok = actual.is_some() && !matches!(actual, Some(Value::Null));
            return (ok, presence_msg(actual, "to be not null"));
        }
        _ => {}
    }

    let Some(actual) = actual else {
        return (false, Some("selector resolved to nothing".into()));
    };

    let result = match op {
        Eq => loose_eq(actual, expected),
        Neq => loose_eq(actual, expected).map(|b| !b),
        Gt => compare(actual, expected).map(|o| o == std::cmp::Ordering::Greater),
        Gte => compare(actual, expected).map(|o| o != std::cmp::Ordering::Less),
        Lt => compare(actual, expected).map(|o| o == std::cmp::Ordering::Less),
        Lte => compare(actual, expected).map(|o| o != std::cmp::Ordering::Greater),
        Contains => contains(actual, expected),
        NotContains => contains(actual, expected).map(|b| !b),
        Matches => regex_match(actual, expected),
        NotMatches => regex_match(actual, expected).map(|b| !b),
        In => in_list(actual, expected),
        NotIn => in_list(actual, expected).map(|b| !b),
        Length => length_eq(actual, expected),
        IsDefined | IsUndefined | IsNull | IsNotNull => unreachable!("handled above"),
    };

    match result {
        Ok(true) => (true, None),
        Ok(false) => (
            false,
            Some(format!(
                "expected {} {op:?} {}",
                compact(actual),
                expected.map(compact).unwrap_or_default()
            )),
        ),
        Err(msg) => (false, Some(msg)),
    }
}

fn presence_msg(actual: Option<&Value>, wanted: &str) -> Option<String> {
    Some(format!(
        "expected value {wanted} (got {})",
        actual.map(compact).unwrap_or_else(|| "nothing".into())
    ))
}

fn compact(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 80 {
        format!("{}…", &s[..80])
    } else {
        s
    }
}

fn loose_eq(actual: &Value, expected: Option<&Value>) -> Result<bool, String> {
    let expected = expected.ok_or("this operator needs a `value`")?;
    if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
        return Ok(a == b);
    }
    // allow comparing anything against its string representation
    if let (Value::String(a), b) = (actual, expected)
        && !b.is_string()
    {
        let rendered = b.to_string();
        return Ok(*a == rendered);
    }
    if let (a, Value::String(b)) = (actual, expected)
        && !a.is_string()
    {
        let rendered = a.to_string();
        return Ok(rendered == *b);
    }
    Ok(actual == expected)
}

fn compare(actual: &Value, expected: Option<&Value>) -> Result<std::cmp::Ordering, String> {
    let expected = expected.ok_or("this operator needs a `value`")?;
    if let (Some(a), Some(b)) = (actual.as_f64(), expected.as_f64()) {
        return a.partial_cmp(&b).ok_or_else(|| "NaN comparison".into());
    }
    if let (Value::String(a), Value::String(b)) = (actual, expected) {
        return Ok(a.cmp(b));
    }
    Err(format!(
        "cannot order {} against {}",
        compact(actual),
        compact(expected)
    ))
}

fn contains(actual: &Value, expected: Option<&Value>) -> Result<bool, String> {
    let expected = expected.ok_or("this operator needs a `value`")?;
    match actual {
        Value::String(s) => Ok(s.contains(expected.as_str().unwrap_or(&expected.to_string()))),
        Value::Array(items) => Ok(items
            .iter()
            .any(|item| loose_eq(item, Some(expected)).unwrap_or(false))),
        Value::Object(map) => Ok(expected
            .as_str()
            .map(|k| map.contains_key(k))
            .unwrap_or(false)),
        other => Err(format!("contains is not defined for {}", compact(other))),
    }
}

fn regex_match(actual: &Value, expected: Option<&Value>) -> Result<bool, String> {
    let pattern = expected
        .and_then(|v| v.as_str())
        .ok_or("matches needs a string pattern")?;
    let re = regex::Regex::new(pattern).map_err(|e| format!("invalid regex: {e}"))?;
    let text = match actual {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    Ok(re.is_match(&text))
}

fn in_list(actual: &Value, expected: Option<&Value>) -> Result<bool, String> {
    let list = expected
        .and_then(|v| v.as_array())
        .ok_or("in/notIn need an array `value`")?;
    Ok(list
        .iter()
        .any(|item| loose_eq(actual, Some(item)).unwrap_or(false)))
}

fn length_eq(actual: &Value, expected: Option<&Value>) -> Result<bool, String> {
    let want = expected
        .and_then(|v| v.as_u64())
        .ok_or("length needs a numeric `value`")?;
    let got = match actual {
        Value::String(s) => s.chars().count() as u64,
        Value::Array(a) => a.len() as u64,
        Value::Object(o) => o.len() as u64,
        other => return Err(format!("length is not defined for {}", compact(other))),
    };
    Ok(got == want)
}
