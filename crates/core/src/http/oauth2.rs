//! OAuth2 client_credentials + password grants, hand-rolled (two POST shapes —
//! the `oauth2` crate is overkill). Token cache keyed by a blake3 fingerprint
//! of the config; the `TokenProvider` seam leaves room for auth-code+PKCE.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use tokio_util::sync::CancellationToken;

use crate::CoreError;
use crate::model::{ClientAuth, OAuth2Config, OAuth2Grant};

/// Refresh this long before the reported expiry.
const EXPIRY_SKEW: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: Option<Instant>,
}

impl CachedToken {
    fn is_fresh(&self) -> bool {
        match self.expires_at {
            Some(at) => Instant::now() + EXPIRY_SKEW < at,
            None => true,
        }
    }
}

#[derive(Debug, Default)]
pub struct TokenCache {
    map: Mutex<HashMap<[u8; 32], CachedToken>>,
}

impl TokenCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn clear(&self) {
        self.map.lock().expect("token cache lock").clear();
    }
}

fn fingerprint(cfg: &OAuth2Config) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(cfg.token_url.as_bytes());
    hasher.update(cfg.client_id.as_bytes());
    hasher.update(cfg.client_secret.as_bytes());
    hasher.update(match cfg.grant {
        OAuth2Grant::ClientCredentials => b"cc",
        OAuth2Grant::Password => b"pw",
    });
    hasher.update(cfg.username.as_deref().unwrap_or("").as_bytes());
    hasher.update(cfg.scopes.join(" ").as_bytes());
    *hasher.finalize().as_bytes()
}

/// Get a bearer token for the config, using the cache when allowed.
pub async fn get_token(
    client: &reqwest::Client,
    cfg: &OAuth2Config,
    cache: &TokenCache,
    cancel: &CancellationToken,
    timeout_ms: u64,
) -> Result<String, CoreError> {
    let key = fingerprint(cfg);

    if cfg.cache_token
        && let Some(hit) = cache.map.lock().expect("token cache lock").get(&key)
        && hit.is_fresh()
    {
        return Ok(hit.access_token.clone());
    }

    let token = fetch_token(client, cfg, cancel, timeout_ms).await?;
    if cfg.cache_token {
        cache
            .map
            .lock()
            .expect("token cache lock")
            .insert(key, token.clone());
    }
    Ok(token.access_token)
}

async fn fetch_token(
    client: &reqwest::Client,
    cfg: &OAuth2Config,
    cancel: &CancellationToken,
    timeout_ms: u64,
) -> Result<CachedToken, CoreError> {
    let mut form: Vec<(&str, String)> = vec![(
        "grant_type",
        match cfg.grant {
            OAuth2Grant::ClientCredentials => "client_credentials".to_string(),
            OAuth2Grant::Password => "password".to_string(),
        },
    )];
    if cfg.grant == OAuth2Grant::Password {
        form.push(("username", cfg.username.clone().unwrap_or_default()));
        form.push(("password", cfg.password.clone().unwrap_or_default()));
    }
    if !cfg.scopes.is_empty() {
        form.push(("scope", cfg.scopes.join(" ")));
    }

    let mut builder = client
        .post(&cfg.token_url)
        .timeout(Duration::from_millis(timeout_ms));

    match cfg.client_auth {
        ClientAuth::BasicHeader => {
            let credentials = base64::engine::general_purpose::STANDARD
                .encode(format!("{}:{}", cfg.client_id, cfg.client_secret));
            builder = builder.header("Authorization", format!("Basic {credentials}"));
        }
        ClientAuth::Body => {
            form.push(("client_id", cfg.client_id.clone()));
            form.push(("client_secret", cfg.client_secret.clone()));
        }
    }

    let response = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        r = builder.form(&form).send() => r.map_err(|e| CoreError::from_reqwest(e, timeout_ms))?,
    };

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let excerpt: String = body.chars().take(300).collect();
        return Err(CoreError::Http {
            message: format!("token endpoint returned {status}: {excerpt}"),
        });
    }

    let json: serde_json::Value = serde_json::from_str(&body).map_err(|e| CoreError::Http {
        message: format!("token endpoint returned invalid JSON: {e}"),
    })?;
    let access_token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| CoreError::Http {
            message: "token response has no `access_token`".into(),
        })?
        .to_string();
    let expires_at = json
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .map(|secs| Instant::now() + Duration::from_secs(secs));

    Ok(CachedToken {
        access_token,
        expires_at,
    })
}
