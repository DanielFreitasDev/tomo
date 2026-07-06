//! reqwest Client construction from settings, incl. mTLS client certificates.

use std::path::Path;
use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
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
    /// Extra CA bundles (PEM bytes) trusted *in addition* to the system roots,
    /// from `[tls] extra_cas`. Empty means system roots only.
    pub extra_ca_pems: Vec<Vec<u8>>,
}

impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            follow_redirects: true,
            max_redirects: 10,
            ssl_verify: true,
            proxy: ProxySettings::default(),
            client_identity_pem: None,
            extra_ca_pems: Vec::new(),
        }
    }
}

/// Per-collection pool of reqwest clients, keyed by a fingerprint of the
/// connection-affecting options (redirect policy, TLS verify, proxy, mTLS
/// identity, extra CAs). Reusing a client keeps TCP/TLS connections and HTTP/2
/// sessions alive across requests instead of a fresh handshake every send.
///
/// A cache is bound to one collection's cookie jar — the jar its clients were
/// built with. Never share a `ClientCache` across collections.
pub struct ClientCache {
    inner: Mutex<IndexMap<String, reqwest::Client>>,
    cap: usize,
}

impl ClientCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(IndexMap::new()),
            cap: 8,
        })
    }

    /// Return the client for these options, building and caching one on a miss.
    /// `jar` is consumed only on a miss. A settings change that alters any
    /// connection-affecting option yields a different fingerprint, so a new
    /// client is built and the stale one ages out of the LRU.
    pub fn get_or_build(
        &self,
        opts: &ClientOptions,
        jar: Arc<TomoJar>,
    ) -> Result<reqwest::Client, CoreError> {
        let key = fingerprint(opts);
        if let Ok(mut map) = self.inner.lock()
            && let Some(idx) = map.get_index_of(&key)
        {
            // Move to the back = most-recently-used.
            let last = map.len() - 1;
            map.move_index(idx, last);
            return Ok(map[last].clone());
        }
        let client = build_client(opts, jar)?;
        if let Ok(mut map) = self.inner.lock() {
            map.insert(key, client.clone());
            while map.len() > self.cap {
                map.shift_remove_index(0); // evict least-recently-used
            }
        }
        Ok(client)
    }

    /// Drop all cached clients.
    pub fn clear(&self) {
        if let Ok(mut map) = self.inner.lock() {
            map.clear();
        }
    }

    /// Number of currently cached clients (test/introspection helper).
    pub fn len(&self) -> usize {
        self.inner.lock().map(|m| m.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Stable key over every option that changes how the underlying client
/// connects. Two option sets with the same fingerprint can safely share a
/// client (and its connection pool).
fn fingerprint(opts: &ClientOptions) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    opts.follow_redirects.hash(&mut h);
    opts.max_redirects.hash(&mut h);
    opts.ssl_verify.hash(&mut h);
    (opts.proxy.mode as u8).hash(&mut h);
    opts.proxy.url.hash(&mut h);
    opts.client_identity_pem.hash(&mut h);
    opts.extra_ca_pems.hash(&mut h);
    format!("{:016x}", h.finish())
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

    // Trust extra CA bundles on top of the system roots (private/self-signed
    // servers) without turning off verification globally.
    for pem in &opts.extra_ca_pems {
        for cert in reqwest::Certificate::from_pem_bundle(pem)
            .map_err(|e| CoreError::Invalid(format!("invalid extra CA PEM: {e}")))?
        {
            builder = builder.add_root_certificate(cert);
        }
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

/// Read the collection's `[tls] extra_cas` PEM bundles into raw bytes. Each path
/// is resolved relative to the collection root and traversal-guarded. Returns an
/// empty vec when none are configured (system roots only).
pub fn resolve_extra_cas(tls: &Tls, root: &Path) -> Result<Vec<Vec<u8>>, CoreError> {
    tls.extra_cas
        .iter()
        .map(|rel| {
            let path = resolve_rel(root, rel)?;
            std::fs::read(&path).map_err(|e| CoreError::io(&path, e))
        })
        .collect()
}
