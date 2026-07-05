//! Request body variants — `[body]` table, discriminated by `type`.

use serde::{Deserialize, Serialize};

use super::{default_true, is_true};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Body {
    Json {
        content: String,
    },
    Text {
        content: String,
    },
    Xml {
        content: String,
    },
    FormUrlencoded {
        fields: Vec<super::Pair>,
    },
    MultipartForm {
        parts: Vec<MultipartPart>,
    },
    /// Raw file as the request body. `path` is relative to the collection root.
    Binary {
        path: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_type: Option<String>,
    },
    Graphql {
        query: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        variables: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartKind {
    Text,
    File,
}

/// One multipart part: `kind = "text"` uses `value`, `kind = "file"` uses `path`
/// (relative to the collection root).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultipartPart {
    pub name: String,
    pub kind: PartKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
}
