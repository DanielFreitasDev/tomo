//! Unified error type for tomo-core. Variants are added as modules land.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("invalid TOML in {path}{}: {message}", line.map(|l| format!(" (line {l})")).unwrap_or_default())]
    TomlParse {
        path: PathBuf,
        line: Option<usize>,
        message: String,
    },

    #[error("{0}")]
    Invalid(String),
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
