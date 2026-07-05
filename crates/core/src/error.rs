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

    #[error("request failed: {message}")]
    Http { message: String },

    #[error("request timed out after {ms} ms")]
    Timeout { ms: u64 },

    #[error("request cancelled")]
    Cancelled,
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// Map a reqwest error to a user-facing CoreError.
    pub fn from_reqwest(e: reqwest::Error, timeout_ms: u64) -> Self {
        if e.is_timeout() {
            return Self::Timeout { ms: timeout_ms };
        }
        // unwrap the source chain for a more useful message than reqwest's
        // "error sending request" wrapper
        let mut message = e.to_string();
        let mut source: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(&e);
        while let Some(inner) = source {
            message = inner.to_string();
            source = std::error::Error::source(inner);
        }
        if e.is_redirect() {
            message = format!("redirect policy: {message}");
        }
        Self::Http { message }
    }
}
