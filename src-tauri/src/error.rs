//! Serializable command error: `{ code, message }` — the frontend transport
//! maps this into TransportError.

use serde::Serialize;
use tomo_core::CoreError;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl From<CoreError> for ApiError {
    fn from(e: CoreError) -> Self {
        let code = match &e {
            CoreError::Io { .. } => "io",
            CoreError::TomlParse { .. } => "toml_parse",
            CoreError::Invalid(_) => "invalid",
            CoreError::Http { .. } => "http",
            CoreError::Timeout { .. } => "timeout",
            CoreError::Cancelled => "cancelled",
        };
        ApiError::new(code, e.to_string())
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

pub type ApiResult<T> = Result<T, ApiError>;
