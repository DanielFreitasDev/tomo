//! reqwest Client construction from settings, incl. mTLS client certificates.

use std::path::Path;
use std::sync::Arc;

use reqwest::redirect::Policy;

use crate::CoreError;
use crate::fsops::resolve_rel;
use crate::model::{ProxyMode, ProxySettings, Tls};

use super::cookies::TomoJar;

#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub ssl_verify: bool,
    pub proxy: ProxySettings,
    /// Combined client-cert + private-key PEM for mTLS, when the collection
    /// configured a `[[tls.client_certs]]` entry for the target host. `None`
    /// means no client certificate is presented.
    pub client_identity_pem: Option<Vec<u8>>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            follow_redirects: true,
            max_redirects: 10,
            ssl_verify: true,
            proxy: ProxySettings::default(),
            client_identity_pem: None,
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

    // mTLS: present the collection-configured client certificate for this host.
    if let Some(pem) = &opts.client_identity_pem {
        let identity = reqwest::Identity::from_pem(pem)
            .map_err(|e| CoreError::Invalid(format!("invalid client certificate/key PEM: {e}")))?;
        builder = builder.identity(identity);
    }

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

/// Resolve the client certificate configured for `host`, returning a combined
/// cert+key PEM buffer (what reqwest's rustls `Identity::from_pem` expects).
/// Cert/key paths are resolved relative to the collection root and are
/// traversal-guarded. `Ok(None)` when no cert is configured for the host, so
/// non-mTLS requests are untouched.
pub fn resolve_client_identity(
    tls: &Tls,
    host: Option<&str>,
    root: &Path,
) -> Result<Option<Vec<u8>>, CoreError> {
    let Some(host) = host else {
        return Ok(None);
    };
    let Some(entry) = tls
        .client_certs
        .iter()
        .find(|c| c.host.eq_ignore_ascii_case(host))
    else {
        return Ok(None);
    };

    let cert_path = resolve_rel(root, &entry.cert)?;
    let key_path = resolve_rel(root, &entry.key)?;

    let mut pem = std::fs::read(&cert_path).map_err(|e| CoreError::io(&cert_path, e))?;
    if !pem.ends_with(b"\n") {
        pem.push(b'\n');
    }
    let key = std::fs::read(&key_path).map_err(|e| CoreError::io(&key_path, e))?;
    pem.extend_from_slice(&key);
    Ok(Some(pem))
}
