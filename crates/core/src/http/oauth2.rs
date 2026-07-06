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

    /// Drop one config's cached token so the next `get_token` re-fetches it —
    /// used for refresh-on-401 when the server rejects a token we thought fresh.
    pub fn invalidate(&self, cfg: &OAuth2Config) {
        let key = fingerprint(cfg);
        self.map.lock().expect("token cache lock").remove(&key);
    }
}

fn fingerprint(cfg: &OAuth2Config) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    // Length-prefix every field so distinct configs can't collide by
    // concatenation (client_id="app1" vs client_id="app"+secret starting "1"),
    // and include EVERY credential-bearing field — a token is only
    // interchangeable across requests whose full auth config is identical.
    let mut field = |bytes: &[u8]| {
        hasher.update(&(bytes.len() as u64).to_le_bytes());
        hasher.update(bytes);
    };
    field(cfg.token_url.as_bytes());
    field(cfg.client_id.as_bytes());
    field(cfg.client_secret.as_bytes());
    field(match cfg.grant {
        OAuth2Grant::ClientCredentials => b"cc",
        OAuth2Grant::Password => b"pw",
    });
    field(match cfg.client_auth {
        ClientAuth::BasicHeader => b"basic",
        ClientAuth::Body => b"body",
    });
    field(cfg.username.as_deref().unwrap_or("").as_bytes());
    field(cfg.password.as_deref().unwrap_or("").as_bytes());
    field(cfg.scopes.join(" ").as_bytes());
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
        .and_then(parse_expires_in_secs)
        // checked_add guards against a hostile/buggy `expires_in` overflowing Instant.
        .and_then(|secs| Instant::now().checked_add(Duration::from_secs(secs)));

    Ok(CachedToken {
        access_token,
        expires_at,
    })
}

/// Parse an OAuth2 `expires_in`. Per RFC 6749 it is a number of seconds, but in
/// the wild it also arrives as a JSON string or a float. Returns whole seconds;
/// `None` if absent, negative, or unparseable (token then treated as opaque).
fn parse_expires_in_secs(v: &serde_json::Value) -> Option<u64> {
    if let Some(n) = v.as_u64() {
        return Some(n);
    }
    if let Some(f) = v.as_f64() {
        return (f.is_finite() && f >= 0.0).then_some(f as u64);
    }
    v.as_str().and_then(|s| s.trim().parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::{fingerprint, parse_expires_in_secs};
    use crate::model::{ClientAuth, OAuth2Config, OAuth2Grant};
    use serde_json::json;
    use std::time::{Duration, Instant};

    fn cfg() -> OAuth2Config {
        OAuth2Config {
            grant: OAuth2Grant::Password,
            token_url: "https://id.example/token".into(),
            client_id: "app".into(),
            client_secret: "s".into(),
            username: Some("u".into()),
            password: Some("p".into()),
            scopes: vec!["read".into()],
            client_auth: ClientAuth::Body,
            cache_token: true,
        }
    }

    #[test]
    fn fingerprint_is_collision_free_and_credential_complete() {
        // identical configs share a cache key
        assert_eq!(fingerprint(&cfg()), fingerprint(&cfg()));

        // unseparated concatenation must NOT collide: ("app1","x") vs ("app","1x")
        let mut a = cfg();
        a.client_id = "app1".into();
        a.client_secret = "x".into();
        let mut b = cfg();
        b.client_id = "app".into();
        b.client_secret = "1x".into();
        assert_ne!(fingerprint(&a), fingerprint(&b));

        // password is part of the token's identity (was omitted before)
        let mut c = cfg();
        c.password = Some("other".into());
        assert_ne!(fingerprint(&cfg()), fingerprint(&c));
    }

    #[test]
    fn expires_in_accepts_int_string_and_float() {
        assert_eq!(parse_expires_in_secs(&json!(3600)), Some(3600));
        assert_eq!(parse_expires_in_secs(&json!("3600")), Some(3600));
        assert_eq!(parse_expires_in_secs(&json!(3600.9)), Some(3600));
        assert_eq!(parse_expires_in_secs(&json!("  90 ")), Some(90));
    }

    #[test]
    fn expires_in_rejects_garbage_and_negatives() {
        assert_eq!(parse_expires_in_secs(&json!("soon")), None);
        assert_eq!(parse_expires_in_secs(&json!(-5)), None);
        assert_eq!(parse_expires_in_secs(&json!(null)), None);
    }

    #[test]
    fn huge_expires_in_does_not_panic() {
        // u64::MAX used to overflow `Instant + Duration` and panic on a plain 200.
        let secs = parse_expires_in_secs(&json!(u64::MAX)).unwrap();
        let at = Instant::now().checked_add(Duration::from_secs(secs));
        assert!(at.is_none(), "checked_add must yield None, not panic");
    }
}
