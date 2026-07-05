//! Response data captured by the HTTP engine.

use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ResponseData {
    pub status: u16,
    pub status_text: String,
    pub http_version: String,
    /// Response headers in wire order (duplicates preserved).
    pub headers: Vec<(String, String)>,
    pub final_url: String,
    pub timing: Timing,
    pub body: BodyCapture,
    /// Interpolation warnings gathered while preparing the request.
    pub warnings: Vec<crate::vars::Warning>,
}

#[derive(Debug, Clone, Copy, Serialize, Default)]
pub struct Timing {
    pub total_ms: u64,
    /// Time until response headers arrived (TTFB approximation).
    pub ttfb_ms: u64,
    pub download_ms: u64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct BodyCapture {
    /// Up to `cap` bytes kept in memory for preview/parsing.
    #[serde(skip)]
    pub bytes: Vec<u8>,
    pub total_size: u64,
    /// True when the body exceeded the cap; the full body is in `spill_path`.
    pub truncated: bool,
    pub spill_path: Option<PathBuf>,
    pub mime: Option<String>,
    pub charset: Option<String>,
    /// Null byte seen in the head — treat as binary (hex/save-only in UIs).
    pub is_binary: bool,
}

impl BodyCapture {
    /// Decode the in-memory preview using the declared charset (lossy).
    pub fn preview_text(&self) -> String {
        let encoding = self
            .charset
            .as_deref()
            .and_then(|cs| encoding_rs::Encoding::for_label(cs.as_bytes()))
            .unwrap_or(encoding_rs::UTF_8);
        let (text, _, _) = encoding.decode(&self.bytes);
        text.into_owned()
    }
}
