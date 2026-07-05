//! reqwest Client construction from settings.

use std::sync::Arc;

use reqwest::redirect::Policy;

use crate::CoreError;
use crate::model::{ProxyMode, ProxySettings};

use super::cookies::TomoJar;

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub ssl_verify: bool,
    pub proxy: ProxySettings,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            follow_redirects: true,
            max_redirects: 10,
            ssl_verify: true,
            proxy: ProxySettings::default(),
        }
    }
}

pub fn build_client(opts: &ClientOptions, jar: Arc<TomoJar>) -> Result<reqwest::Client, CoreError> {
    let redirect = if opts.follow_redirects {
        Policy::limited(opts.max_redirects as usize)
    } else {
        Policy::none()
    };

    let mut builder = reqwest::Client::builder()
        .redirect(redirect)
        .cookie_provider(jar)
        .danger_accept_invalid_certs(!opts.ssl_verify);

    builder = match opts.proxy.mode {
        ProxyMode::Off => builder.no_proxy(),
        // reqwest's default behavior reads env/system proxies
        ProxyMode::System => builder,
        ProxyMode::Manual => match &opts.proxy.url {
            Some(url) if !url.is_empty() => {
                let proxy = reqwest::Proxy::all(url)
                    .map_err(|e| CoreError::Invalid(format!("invalid proxy url `{url}`: {e}")))?;
                builder.proxy(proxy)
            }
            _ => {
                return Err(CoreError::Invalid(
                    "manual proxy mode requires a proxy url".into(),
                ));
            }
        },
    };

    builder
        .build()
        .map_err(|e| CoreError::Invalid(format!("failed to build HTTP client: {e}")))
}
