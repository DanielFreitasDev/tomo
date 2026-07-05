//! Request file schema — one request per `<slug>.toml`.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::{Auth, Body, VarValue, default_true, is_true};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RequestFile {
    pub meta: RequestMeta,
    pub http: HttpDef,
    /// `None` = inherit auth from folder/collection. `Some(Auth::None)` opts out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Body>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub vars: IndexMap<String, VarValue>,
    #[serde(default, skip_serializing_if = "Scripts::is_empty")]
    pub scripts: Scripts,
    #[serde(default, skip_serializing_if = "Tests::is_empty")]
    pub tests: Tests,
    #[serde(default, skip_serializing_if = "RequestOptions::is_empty")]
    pub options: RequestOptions,
    #[serde(default, skip_serializing_if = "Docs::is_empty")]
    pub docs: Docs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RequestMeta {
    pub name: String,
    /// Display order among siblings; files without seq sort last, by filename.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HttpDef {
    pub method: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Pair>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query: Vec<Pair>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<Pair>,
}

/// A name/value entry with an enabled flag (headers, params, form fields).
/// `enabled` defaults to true and is omitted from disk when true.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pair {
    pub name: String,
    pub value: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

impl Pair {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: true,
        }
    }

    pub fn disabled(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Scripts {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre_request: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_response: Option<String>,
}

impl Scripts {
    pub fn is_empty(&self) -> bool {
        self.pre_request.is_none() && self.post_response.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Tests {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asserts: Vec<Assert>,
}

impl Tests {
    pub fn is_empty(&self) -> bool {
        self.asserts.is_empty()
    }
}

/// Declarative assertion over the response, e.g.
/// `{ expr = "res.status", op = "eq", value = 201 }`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Assert {
    pub expr: String,
    pub op: AssertOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<VarValue>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssertOp {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Contains,
    NotContains,
    Matches,
    NotMatches,
    IsDefined,
    IsUndefined,
    IsNull,
    IsNotNull,
    In,
    NotIn,
    Length,
}

/// Per-request overrides of network settings. All optional.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RequestOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follow_redirects: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_redirects: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssl_verify: Option<bool>,
}

impl RequestOptions {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Docs {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content: String,
}

impl Docs {
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}
