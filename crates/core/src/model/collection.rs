//! `collection.toml` (collection root) and `folder.toml` (any folder) schemas.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::request::{Pair, Scripts};
use super::{Auth, VarValue};

pub const COLLECTION_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionFile {
    pub meta: CollectionMeta,
    #[serde(default, skip_serializing_if = "Defaults::is_empty")]
    pub defaults: Defaults,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub vars: IndexMap<String, VarValue>,
    #[serde(default, skip_serializing_if = "Scripts::is_empty")]
    pub scripts: Scripts,
    #[serde(default, skip_serializing_if = "Tls::is_empty")]
    pub tls: Tls,
}

impl CollectionFile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            meta: CollectionMeta {
                name: name.into(),
                format: COLLECTION_FORMAT_VERSION,
            },
            defaults: Defaults::default(),
            auth: None,
            vars: IndexMap::new(),
            scripts: Scripts::default(),
            tls: Tls::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CollectionMeta {
    pub name: String,
    /// Tomo format version, for future migrations.
    #[serde(default = "default_format")]
    pub format: u32,
}

fn default_format() -> u32 {
    COLLECTION_FORMAT_VERSION
}

/// Defaults merged into every request (collection- or folder-level).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Defaults {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<Pair>,
}

impl Defaults {
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Tls {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub client_certs: Vec<ClientCert>,
    /// Extra CA certificate bundles (PEM) to trust *in addition* to the system
    /// roots — for self-signed / private-CA servers without disabling
    /// verification wholesale. Paths are relative to the collection root.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_cas: Vec<String>,
}

impl Tls {
    pub fn is_empty(&self) -> bool {
        self.client_certs.is_empty() && self.extra_cas.is_empty()
    }
}

/// Client certificate (mTLS) for a specific host. PEM only in v1; paths are
/// relative to the collection root.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientCert {
    pub host: String,
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FolderFile {
    pub meta: FolderMeta,
    #[serde(default, skip_serializing_if = "Defaults::is_empty")]
    pub defaults: Defaults,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<Auth>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub vars: IndexMap<String, VarValue>,
    #[serde(default, skip_serializing_if = "Scripts::is_empty")]
    pub scripts: Scripts,
}

impl FolderFile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            meta: FolderMeta {
                name: name.into(),
                seq: None,
            },
            defaults: Defaults::default(),
            auth: None,
            vars: IndexMap::new(),
            scripts: Scripts::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct FolderMeta {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<i64>,
}
