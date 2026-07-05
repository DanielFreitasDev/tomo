//! Per-collection session cookie jar.
//!
//! Own wrapper over `cookie_store` (instead of reqwest's opaque `Jar`) so the
//! UI can list and clear cookies.

use std::sync::{Arc, RwLock};

use reqwest::header::HeaderValue;
use serde::Serialize;

#[derive(Debug, Default)]
pub struct TomoJar {
    store: RwLock<cookie_store::CookieStore>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CookieDto {
    pub domain: String,
    pub path: String,
    pub name: String,
    pub value: String,
    pub secure: bool,
    pub http_only: bool,
    pub expires: Option<String>,
}

impl TomoJar {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn list(&self) -> Vec<CookieDto> {
        let store = self.store.read().expect("jar lock");
        store
            .iter_any()
            .map(|c| CookieDto {
                domain: c.domain().unwrap_or_default().to_string(),
                path: c.path().unwrap_or_default().to_string(),
                name: c.name().to_string(),
                value: c.value().to_string(),
                secure: c.secure().unwrap_or(false),
                http_only: c.http_only().unwrap_or(false),
                expires: match c.expires() {
                    Some(cookie::Expiration::DateTime(dt)) => dt
                        .format(&time::format_description::well_known::Rfc3339)
                        .ok(),
                    _ => None,
                },
            })
            .collect()
    }

    pub fn clear(&self, domain: Option<&str>) {
        let mut store = self.store.write().expect("jar lock");
        match domain {
            None => store.clear(),
            Some(d) => {
                let doomed: Vec<(String, String, String)> = store
                    .iter_any()
                    .filter(|c| {
                        c.domain().unwrap_or_default().trim_start_matches('.')
                            == d.trim_start_matches('.')
                    })
                    .map(|c| {
                        (
                            c.domain().unwrap_or_default().to_string(),
                            c.path().unwrap_or_default().to_string(),
                            c.name().to_string(),
                        )
                    })
                    .collect();
                for (domain, path, name) in doomed {
                    store.remove(&domain, &path, &name);
                }
            }
        }
    }
}

impl reqwest::cookie::CookieStore for TomoJar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &url::Url) {
        let mut store = self.store.write().expect("jar lock");
        for header in cookie_headers {
            if let Ok(text) = header.to_str()
                && let Ok(parsed) = cookie::Cookie::parse(text.to_owned())
            {
                let _ = store.insert_raw(&parsed, url);
            }
        }
    }

    fn cookies(&self, url: &url::Url) -> Option<HeaderValue> {
        let store = self.store.read().expect("jar lock");
        let pairs: Vec<String> = store
            .get_request_values(url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        if pairs.is_empty() {
            return None;
        }
        HeaderValue::from_str(&pairs.join("; ")).ok()
    }
}
