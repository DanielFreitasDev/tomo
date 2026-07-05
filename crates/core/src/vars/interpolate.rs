//! `{{variable}}` interpolation with dot/index paths, per-token recursive
//! expansion, cycle detection and unknown-variable warnings.
//!
//! Semantics (Bruno-compatible where it matters):
//! - unknown tokens stay verbatim (`{{missing}}`) and produce one warning
//! - cycles stay verbatim and produce a warning — never a hard error
//! - dynamic vars (`{{$uuid}}`) are fresh per occurrence

use crate::model::VarValue;

use super::dynamic;
use super::path::{split_path, walk_path};
use super::scope::VarStack;

const MAX_DEPTH: usize = 10;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Warning {
    Unknown { name: String },
    Cycle { name: String },
    DepthExceeded { name: String },
}

#[derive(Debug, Clone)]
pub struct Interpolated {
    pub text: String,
    pub warnings: Vec<Warning>,
}

pub fn interpolate(text: &str, stack: &VarStack) -> Interpolated {
    let mut warnings = Vec::new();
    let mut visiting = Vec::new();
    let out = expand(text, stack, 0, &mut visiting, &mut warnings);
    Interpolated {
        text: out,
        warnings,
    }
}

fn expand(
    text: &str,
    stack: &VarStack,
    depth: usize,
    visiting: &mut Vec<String>,
    warnings: &mut Vec<Warning>,
) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'{' && i + 1 < bytes.len() && bytes[i + 1] == b'{' {
            if let Some(close) = find_close(text, i + 2) {
                let raw = &text[i + 2..close];
                let token = raw.trim();
                out.push_str(&resolve_token(token, stack, depth, visiting, warnings));
                i = close + 2;
                continue;
            }
        }
        // advance one full UTF-8 char
        let ch_len = text[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&text[i..i + ch_len]);
        i += ch_len;
    }
    out
}

fn find_close(text: &str, from: usize) -> Option<usize> {
    text[from..].find("}}").map(|p| from + p)
}

fn resolve_token(
    token: &str,
    stack: &VarStack,
    depth: usize,
    visiting: &mut Vec<String>,
    warnings: &mut Vec<Warning>,
) -> String {
    if token.is_empty() {
        return "{{}}".to_string();
    }

    if let Some(dynamic_value) = dynamic::resolve(token) {
        return dynamic_value;
    }

    let (root, path) = split_path(token);

    let Some((value, _scope)) = stack.resolve(root) else {
        push_once(
            warnings,
            Warning::Unknown {
                name: token.to_string(),
            },
        );
        return format!("{{{{{token}}}}}");
    };

    let value = match walk_path(value, &path) {
        Some(v) => v,
        None => {
            push_once(
                warnings,
                Warning::Unknown {
                    name: token.to_string(),
                },
            );
            return format!("{{{{{token}}}}}");
        }
    };

    let as_text = value_to_string(value);

    // nested interpolation inside the resolved value
    if as_text.contains("{{") {
        if visiting.iter().any(|v| v == root) {
            push_once(
                warnings,
                Warning::Cycle {
                    name: root.to_string(),
                },
            );
            return format!("{{{{{token}}}}}");
        }
        if depth >= MAX_DEPTH {
            push_once(
                warnings,
                Warning::DepthExceeded {
                    name: root.to_string(),
                },
            );
            return format!("{{{{{token}}}}}");
        }
        visiting.push(root.to_string());
        let expanded = expand(&as_text, stack, depth + 1, visiting, warnings);
        visiting.pop();
        return expanded;
    }

    as_text
}

fn value_to_string(v: &VarValue) -> String {
    match v {
        VarValue::String(s) => s.clone(),
        VarValue::Bool(b) => b.to_string(),
        VarValue::Number(n) => n.to_string(),
        VarValue::Null => String::new(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

fn push_once(warnings: &mut Vec<Warning>, w: Warning) {
    if !warnings.contains(&w) {
        warnings.push(w);
    }
}
