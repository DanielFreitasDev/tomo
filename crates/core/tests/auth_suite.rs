//! Digest challenge flow and OAuth2 grant/caching tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;
use tomo_core::CoreError;
use tomo_core::http::{Chain, EngineConfig, RunSpec, TokenCache, TomoJar, execute};
use tomo_core::model::*;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

fn base_request(method_: &str, url: String) -> RequestFile {
    RequestFile {
        meta: RequestMeta {
            name: "t".into(),
            seq: None,
        },
        http: HttpDef {
            method: method_.into(),
            url,
            ..Default::default()
        },
        ..Default::default()
    }
}

async fn run(req: &RequestFile, cache: Arc<TokenCache>) -> Result<ResponseData, CoreError> {
    let collection = CollectionFile::new("test");
    let tmp = tempfile::tempdir().unwrap();
    let config = EngineConfig {
        network: NetworkSettings::default(),
        spill_dir: std::env::temp_dir().join("tomo-test-spill"),
    };
    execute(
        &config,
        RunSpec {
            chain: Chain {
                collection: &collection,
                folders: vec![],
                request: req,
            },
            environment: None,
            secrets: None,
            runtime_vars: None,
            process_env: IndexMap::new(),
            dotenv: IndexMap::new(),
            collection_root: tmp.path(),
            jar: TomoJar::new(),
            token_cache: cache,
            cancel: CancellationToken::new(),
        },
    )
    .await
}

// ---------------------------------------------------------------------------
// digest
// ---------------------------------------------------------------------------

/// 401 + Digest challenge when unauthenticated; 200 when a Digest answer
/// with the right shape arrives.
struct DigestGate {
    hits: Arc<AtomicU32>,
}

impl Respond for DigestGate {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        self.hits.fetch_add(1, Ordering::SeqCst);
        match req.headers.get("authorization") {
            None => ResponseTemplate::new(401).insert_header(
                "www-authenticate",
                "Digest realm=\"tomo-test\", nonce=\"abc123nonce\", qop=\"auth\", algorithm=MD5",
            ),
            Some(auth) => {
                let auth = auth.to_str().unwrap_or_default();
                let ok = auth.starts_with("Digest")
                    && auth.contains("username=\"admin\"")
                    && auth.contains("realm=\"tomo-test\"")
                    && auth.contains("nonce=\"abc123nonce\"")
                    && auth.contains("response=\"")
                    && auth.contains("qop=auth")
                    && auth.contains("nc=00000001");
                if ok {
                    ResponseTemplate::new(200).set_body_string("secret data")
                } else {
                    ResponseTemplate::new(403)
                }
            }
        }
    }
}

#[tokio::test]
async fn digest_challenge_flow_round_trips() {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicU32::new(0));
    Mock::given(method("GET"))
        .and(path("/protected"))
        .respond_with(DigestGate { hits: hits.clone() })
        .mount(&server)
        .await;

    let mut req = base_request("GET", format!("{}/protected", server.uri()));
    req.auth = Some(Auth::Digest {
        username: "admin".into(),
        password: "s3cret".into(),
    });

    let res = run(&req, TokenCache::new()).await.unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.body.preview_text(), "secret data");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "exactly challenge + answer");
}

#[tokio::test]
async fn plain_401_without_digest_challenge_is_surfaced() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let mut req = base_request("GET", format!("{}/x", server.uri()));
    req.auth = Some(Auth::Digest {
        username: "admin".into(),
        password: "pw".into(),
    });
    let res = run(&req, TokenCache::new()).await.unwrap();
    assert_eq!(res.status, 401);
}

#[tokio::test]
async fn digest_with_streaming_multipart_is_rejected_clearly() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("f.bin"), b"x").unwrap();

    let mut req = base_request("POST", "http://localhost:1/up".into());
    req.auth = Some(Auth::Digest {
        username: "u".into(),
        password: "p".into(),
    });
    req.body = Some(Body::MultipartForm {
        parts: vec![MultipartPart {
            name: "f".into(),
            kind: PartKind::File,
            value: None,
            path: Some("f.bin".into()),
            content_type: None,
            enabled: true,
        }],
    });

    let collection = CollectionFile::new("test");
    let config = EngineConfig {
        network: NetworkSettings::default(),
        spill_dir: std::env::temp_dir().join("tomo-test-spill"),
    };
    let err = execute(
        &config,
        RunSpec {
            chain: Chain {
                collection: &collection,
                folders: vec![],
                request: &req,
            },
            environment: None,
            secrets: None,
            runtime_vars: None,
            process_env: IndexMap::new(),
            dotenv: IndexMap::new(),
            collection_root: tmp.path(),
            jar: TomoJar::new(),
            token_cache: TokenCache::new(),
            cancel: CancellationToken::new(),
        },
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("re-sendable"), "{err}");
}

// ---------------------------------------------------------------------------
// oauth2
// ---------------------------------------------------------------------------

fn oauth2_request(
    server_uri: &str,
    grant: OAuth2Grant,
    client_auth: ClientAuth,
    cache_token: bool,
) -> RequestFile {
    let mut req = base_request("GET", format!("{server_uri}/api"));
    req.auth = Some(Auth::Oauth2(OAuth2Config {
        grant,
        token_url: format!("{server_uri}/token"),
        client_id: "cid".into(),
        client_secret: "csecret".into(),
        username: Some("bob".into()),
        password: Some("bobpw".into()),
        scopes: vec!["read".into(), "write".into()],
        client_auth,
        cache_token,
    }));
    req
}

struct TokenEndpoint {
    hits: Arc<AtomicU32>,
    expires_in: u64,
}

impl Respond for TokenEndpoint {
    fn respond(&self, _req: &Request) -> ResponseTemplate {
        let n = self.hits.fetch_add(1, Ordering::SeqCst) + 1;
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "access_token": format!("tok-{n}"),
            "token_type": "Bearer",
            "expires_in": self.expires_in
        }))
    }
}

#[tokio::test]
async fn client_credentials_with_basic_header_and_cache_hit() {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicU32::new(0));

    // token endpoint requires HTTP basic of client credentials
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(header("authorization", "Basic Y2lkOmNzZWNyZXQ=")) // cid:csecret
        .and(body_string_contains("grant_type=client_credentials"))
        .and(body_string_contains("scope=read+write"))
        .respond_with(TokenEndpoint {
            hits: hits.clone(),
            expires_in: 3600,
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .and(header("authorization", "Bearer tok-1"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let cache = TokenCache::new();
    let req = oauth2_request(
        &server.uri(),
        OAuth2Grant::ClientCredentials,
        ClientAuth::BasicHeader,
        true,
    );

    let res = run(&req, cache.clone()).await.unwrap();
    assert_eq!(res.status, 200);
    let res = run(&req, cache.clone()).await.unwrap();
    assert_eq!(res.status, 200);

    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "second run must hit the cache"
    );
}

#[tokio::test]
async fn password_grant_with_body_client_auth() {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicU32::new(0));

    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=password"))
        .and(body_string_contains("username=bob"))
        .and(body_string_contains("password=bobpw"))
        .and(body_string_contains("client_id=cid"))
        .and(body_string_contains("client_secret=csecret"))
        .respond_with(TokenEndpoint {
            hits: hits.clone(),
            expires_in: 3600,
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .and(header("authorization", "Bearer tok-1"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let req = oauth2_request(&server.uri(), OAuth2Grant::Password, ClientAuth::Body, true);
    let res = run(&req, TokenCache::new()).await.unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // token request must not carry an Authorization header in body mode
    let received = server.received_requests().await.unwrap();
    let token_req = received.iter().find(|r| r.url.path() == "/token").unwrap();
    assert!(token_req.headers.get("authorization").is_none());
}

#[tokio::test]
async fn stale_tokens_are_refetched_and_cache_can_be_disabled() {
    let server = MockServer::start().await;
    let hits = Arc::new(AtomicU32::new(0));

    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(TokenEndpoint {
            hits: hits.clone(),
            expires_in: 0, // immediately stale given the 30s skew
        })
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let cache = TokenCache::new();
    let req = oauth2_request(
        &server.uri(),
        OAuth2Grant::ClientCredentials,
        ClientAuth::BasicHeader,
        true,
    );
    run(&req, cache.clone()).await.unwrap();
    run(&req, cache.clone()).await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 2, "expired tokens refetch");

    // cache disabled -> every run fetches
    let req2 = oauth2_request(
        &server.uri(),
        OAuth2Grant::ClientCredentials,
        ClientAuth::BasicHeader,
        false,
    );
    run(&req2, cache.clone()).await.unwrap();
    run(&req2, cache.clone()).await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn token_endpoint_errors_are_readable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_string("{\"error\":\"invalid_client\"}"))
        .mount(&server)
        .await;

    let req = oauth2_request(
        &server.uri(),
        OAuth2Grant::ClientCredentials,
        ClientAuth::BasicHeader,
        true,
    );
    let err = run(&req, TokenCache::new()).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("token endpoint returned"), "{msg}");
    assert!(msg.contains("invalid_client"), "{msg}");
}
