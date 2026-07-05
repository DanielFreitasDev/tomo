//! `environments/<name>.toml` and the git-ignored `secrets.toml` schemas.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::VarValue;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnvironmentFile {
    pub meta: EnvMeta,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub vars: IndexMap<String, VarValue>,
}

impl EnvironmentFile {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            meta: EnvMeta {
                name: name.into(),
                secrets: Vec::new(),
            },
            vars: IndexMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EnvMeta {
    pub name: String,
    /// Names of variables whose VALUES live outside this file
    /// (secrets.toml → .env → process env). Only names are committed.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<String>,
}

/// `secrets.toml` at the collection root — always git-ignored, never committed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SecretsFile {
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub collection: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub environments: IndexMap<String, IndexMap<String, String>>,
}

impl SecretsFile {
    pub fn is_empty(&self) -> bool {
        self.collection.is_empty() && self.environments.is_empty()
    }
}
