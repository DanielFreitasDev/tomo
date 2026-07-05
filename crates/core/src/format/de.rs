//! serde-based parsing with positioned, user-friendly errors.

use std::path::Path;

use serde::de::DeserializeOwned;

use crate::CoreError;
use crate::model::{
    CollectionFile, EnvironmentFile, FolderFile, RequestFile, SecretsFile, Settings,
};

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

fn from_str<T: DeserializeOwned>(text: &str, path: &Path) -> Result<T, CoreError> {
    toml_edit::de::from_str(text).map_err(|e| CoreError::TomlParse {
        path: path.to_path_buf(),
        line: e.span().map(|s| line_of(text, s.start)),
        message: e.message().to_string(),
    })
}

pub fn parse_request(text: &str, path: &Path) -> Result<RequestFile, CoreError> {
    from_str(text, path)
}

pub fn parse_collection(text: &str, path: &Path) -> Result<CollectionFile, CoreError> {
    from_str(text, path)
}

pub fn parse_folder(text: &str, path: &Path) -> Result<FolderFile, CoreError> {
    from_str(text, path)
}

pub fn parse_environment(text: &str, path: &Path) -> Result<EnvironmentFile, CoreError> {
    from_str(text, path)
}

pub fn parse_secrets(text: &str, path: &Path) -> Result<SecretsFile, CoreError> {
    from_str(text, path)
}

pub fn parse_settings(text: &str, path: &Path) -> Result<Settings, CoreError> {
    from_str(text, path)
}
