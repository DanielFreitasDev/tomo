//! HTTP engine integration tests against a local wiremock server.

use std::sync::Arc;
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;
use tomo_core::CoreError;
use tomo_core::http::{Chain, EngineConfig, RunSpec, TomoJar, execute};
use tomo_core::model::*;
use wiremock::matchers::{body_string_contains, header, method, path, query_param};
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

struct Ctx {
    collection: CollectionFile,
    config: EngineConfig,
    tmp: tempfile::TempDir,
}

impl Ctx {
    fn new() -> Self {
        Self {
            collection: CollectionFile::new("test"),
            config: EngineConfig {
                network: NetworkSettings::default(),
                spill_dir: std::env::temp_dir().join("tomo-test-spill"),
            },
            tmp: tempfile::tempdir().unwrap(),
        }
    }

    async fn run(&self, req: &RequestFile) -> Result<ResponseData, CoreError> {
        self.run_with(req, TomoJar::new(), CancellationToken::new())
            .await
    }

    async fn run_with(
        &self,
        req: &RequestFile,
        jar: Arc<TomoJar>,
        cancel: CancellationToken,
    ) -> Result<ResponseData, CoreError> {
        execute(
            &self.config,
            RunSpec {
                chain: Chain {
                    collection: &self.collection,
                    folders: vec![],
                    request: req,
                },
                environment: None,
                secrets: None,
                runtime_vars: None,
                process_env: IndexMap::new(),
                dotenv: IndexMap::new(),
                collection_root: self.tmp.path(),
                jar,
                cancel,
            },
        )
        .await
    }
}

#[tokio::test]
async fn get_captures_status_headers_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/hello"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("x-custom", "yes")
                .set_body_string("world"),
        )
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let res = ctx
        .run(&base_request("GET", format!("{}/hello", server.uri())))
        .await
        .unwrap();

    assert_eq!(res.status, 200);
    assert_eq!(res.status_text, "OK");
    assert_eq!(res.body.preview_text(), "world");
    assert_eq!(res.body.total_size, 5);
    assert!(!res.body.truncated);
    assert!(
        res.headers
            .iter()
            .any(|(k, v)| k == "x-custom" && v == "yes")
    );
    assert!(res.timing.total_ms < 5_000);
}

#[tokio::test]
async fn json_body_and_default_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/users"))
        .and(header("content-type", "application/json"))
        .and(body_string_contains("\"Ada\""))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let mut req = base_request("POST", format!("{}/users", server.uri()));
    req.body = Some(Body::Json {
        content: "{\"name\": \"Ada\"}".into(),
    });
    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 201);
}

#[tokio::test]
async fn interpolation_reaches_url_headers_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v2/users/42"))
        .and(header("x-trace", "abc"))
        .and(query_param("verbose", "true"))
        .and(body_string_contains("payload-abc"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let mut req = base_request("POST", format!("{}/v2/users/:id", server.uri()));
    req.http.path = vec![Pair::new("id", "{{user_id}}")];
    req.http.query = vec![Pair::new("verbose", "{{verbosity}}")];
    req.http.headers = vec![Pair::new("X-Trace", "{{trace}}")];
    req.body = Some(Body::Text {
        content: "payload-{{trace}}".into(),
    });
    req.vars = IndexMap::from([
        ("user_id".to_string(), serde_json::json!(42)),
        ("verbosity".to_string(), serde_json::json!(true)),
        ("trace".to_string(), serde_json::json!("abc")),
    ]);

    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 200);
    assert!(res.warnings.is_empty());
}

#[tokio::test]
async fn unknown_vars_produce_warnings_but_still_send() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let mut req = base_request("GET", format!("{}/x", server.uri()));
    req.http.headers = vec![Pair::new("X-Y", "{{missing_var}}")];
    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.warnings.len(), 1);
}

#[tokio::test]
async fn form_urlencoded_skips_disabled_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(body_string_contains("grant_type=password"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let mut req = base_request("POST", format!("{}/token", server.uri()));
    req.body = Some(Body::FormUrlencoded {
        fields: vec![
            Pair::new("grant_type", "password"),
            Pair::disabled("debug", "1"),
        ],
    });
    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 200);

    let received = server.received_requests().await.unwrap();
    let body = String::from_utf8_lossy(&received[0].body).into_owned();
    assert!(
        !body.contains("debug"),
        "disabled field must not be sent: {body}"
    );
}

#[tokio::test]
async fn multipart_streams_file_parts_with_length() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    // a file inside the collection root
    let payload = vec![b'z'; 32 * 1024];
    std::fs::create_dir_all(ctx.tmp.path().join("assets")).unwrap();
    std::fs::write(ctx.tmp.path().join("assets/blob.bin"), &payload).unwrap();

    let mut req = base_request("POST", format!("{}/upload", server.uri()));
    req.body = Some(Body::MultipartForm {
        parts: vec![
            MultipartPart {
                name: "meta".into(),
                kind: PartKind::Text,
                value: Some("{\"v\":1}".into()),
                path: None,
                content_type: Some("application/json".into()),
                enabled: true,
            },
            MultipartPart {
                name: "file".into(),
                kind: PartKind::File,
                value: None,
                path: Some("assets/blob.bin".into()),
                content_type: Some("application/octet-stream".into()),
                enabled: true,
            },
        ],
    });
    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 200);

    let received = server.received_requests().await.unwrap();
    let r = &received[0];
    let ct = r.headers.get("content-type").unwrap().to_str().unwrap();
    assert!(ct.starts_with("multipart/form-data; boundary="), "{ct}");
    assert!(r.body.len() > 32 * 1024, "file content included");
    assert!(
        r.headers.get("transfer-encoding").is_none(),
        "length known -> no chunked encoding"
    );
}

#[tokio::test]
async fn binary_body_within_limit() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(header("content-type", "application/octet-stream"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    std::fs::write(ctx.tmp.path().join("firmware.bin"), [0u8, 1, 2, 3, 255]).unwrap();
    let mut req = base_request("PUT", format!("{}/fw", server.uri()));
    req.body = Some(Body::Binary {
        path: "firmware.bin".into(),
        content_type: Some("application/octet-stream".into()),
    });
    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 200);
    let received = server.received_requests().await.unwrap();
    assert_eq!(received[0].body, vec![0u8, 1, 2, 3, 255]);
}

#[tokio::test]
async fn body_file_paths_cannot_escape_the_collection() {
    let ctx = Ctx::new();
    let mut req = base_request("PUT", "http://localhost:1/x".into());
    req.body = Some(Body::Binary {
        path: "../../etc/passwd".into(),
        content_type: None,
    });
    let err = ctx.run(&req).await.unwrap_err();
    assert!(err.to_string().contains("traversal"), "{err}");
}

#[tokio::test]
async fn headers_inherit_from_collection_and_disabled_are_not_sent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("user-agent", "tomo-test"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let mut ctx = Ctx::new();
    ctx.collection.defaults.headers = vec![Pair::new("User-Agent", "tomo-test")];

    let mut req = base_request("GET", format!("{}/x", server.uri()));
    req.http.headers = vec![Pair::disabled("X-Debug", "1")];
    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 200);

    let received = server.received_requests().await.unwrap();
    assert!(received[0].headers.get("x-debug").is_none());
}

#[tokio::test]
async fn redirects_follow_when_enabled_and_stop_when_disabled() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/mid", server.uri()).as_str()),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/mid"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", format!("{}/end", server.uri()).as_str()),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/end"))
        .respond_with(ResponseTemplate::new(200).set_body_string("arrived"))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let req = base_request("GET", format!("{}/start", server.uri()));
    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 200);
    assert!(res.final_url.ends_with("/end"));
    assert_eq!(res.body.preview_text(), "arrived");

    // disabled per-request
    let mut req2 = base_request("GET", format!("{}/start", server.uri()));
    req2.options.follow_redirects = Some(false);
    let res2 = ctx.run(&req2).await.unwrap();
    assert_eq!(res2.status, 302);

    // max redirects exceeded -> clear error
    let mut req3 = base_request("GET", format!("{}/start", server.uri()));
    req3.options.max_redirects = Some(1);
    let err = ctx.run(&req3).await.unwrap_err();
    assert!(err.to_string().contains("redirect"), "{err}");
}

#[tokio::test]
async fn gzip_is_transparently_decoded() {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::io::Write as _;

    let mut enc = GzEncoder::new(Vec::new(), Compression::default());
    enc.write_all(b"compressed payload").unwrap();
    let gz = enc.finish().unwrap();

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-encoding", "gzip")
                .set_body_bytes(gz),
        )
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let res = ctx
        .run(&base_request("GET", format!("{}/gz", server.uri())))
        .await
        .unwrap();
    assert_eq!(res.body.preview_text(), "compressed payload");
}

#[tokio::test]
async fn latin1_charset_is_decoded_for_preview() {
    let server = MockServer::start().await;
    // "ação" in ISO-8859-1
    let latin1: Vec<u8> = vec![b'a', 0xE7, 0xE3, b'o'];
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/plain; charset=iso-8859-1")
                .set_body_bytes(latin1),
        )
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let res = ctx
        .run(&base_request("GET", format!("{}/l1", server.uri())))
        .await
        .unwrap();
    assert_eq!(res.body.preview_text(), "ação");
    assert_eq!(res.body.charset.as_deref(), Some("iso-8859-1"));
}

#[tokio::test]
async fn timeout_maps_to_timeout_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(10)))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let mut req = base_request("GET", format!("{}/slow", server.uri()));
    req.options.timeout_ms = Some(200);
    let started = Instant::now();
    let err = ctx.run(&req).await.unwrap_err();
    assert!(matches!(err, CoreError::Timeout { ms: 200 }), "{err}");
    assert!(started.elapsed() < Duration::from_secs(3));
}

#[tokio::test]
async fn cancellation_is_prompt() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(10)))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let req = base_request("GET", format!("{}/slow", server.uri()));
    let cancel = CancellationToken::new();
    let c2 = cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        c2.cancel();
    });

    let started = Instant::now();
    let err = ctx
        .run_with(&req, TomoJar::new(), cancel)
        .await
        .unwrap_err();
    assert!(matches!(err, CoreError::Cancelled), "{err}");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "cancel must be prompt"
    );
}

#[tokio::test]
async fn oversized_bodies_spill_to_disk_with_preview_cap() {
    let server = MockServer::start().await;
    let big = vec![b'x'; 100 * 1024];
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(big.clone()))
        .mount(&server)
        .await;

    let mut ctx = Ctx::new();
    ctx.config.network.response_cap_bytes = 10 * 1024;
    ctx.config.spill_dir = ctx.tmp.path().join("spill");

    let res = ctx
        .run(&base_request("GET", format!("{}/big", server.uri())))
        .await
        .unwrap();
    assert!(res.body.truncated);
    assert_eq!(res.body.total_size, 100 * 1024);
    assert_eq!(res.body.bytes.len(), 10 * 1024);
    let spill = res.body.spill_path.as_ref().expect("spill file");
    assert_eq!(std::fs::metadata(spill).unwrap().len(), 100 * 1024);
    assert_eq!(std::fs::read(spill).unwrap(), big);
}

#[tokio::test]
async fn cookies_are_stored_sent_listed_and_cleared() {
    struct SetCookie;
    impl Respond for SetCookie {
        fn respond(&self, _req: &Request) -> ResponseTemplate {
            ResponseTemplate::new(200)
                .insert_header("set-cookie", "session=abc123; Path=/; HttpOnly")
        }
    }

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/login"))
        .respond_with(SetCookie)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let jar = TomoJar::new();

    ctx.run_with(
        &base_request("GET", format!("{}/login", server.uri())),
        jar.clone(),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let cookies = jar.list();
    assert_eq!(cookies.len(), 1);
    assert_eq!(cookies[0].name, "session");
    assert_eq!(cookies[0].value, "abc123");
    assert!(cookies[0].http_only);

    ctx.run_with(
        &base_request("GET", format!("{}/me", server.uri())),
        jar.clone(),
        CancellationToken::new(),
    )
    .await
    .unwrap();

    let received = server.received_requests().await.unwrap();
    let me = received.iter().find(|r| r.url.path() == "/me").unwrap();
    assert_eq!(
        me.headers.get("cookie").unwrap().to_str().unwrap(),
        "session=abc123"
    );

    jar.clear(None);
    assert!(jar.list().is_empty());
}

#[tokio::test]
async fn simple_auths_reach_the_wire() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let ctx = Ctx::new();

    let mut req = base_request("GET", format!("{}/a", server.uri()));
    req.auth = Some(Auth::Basic {
        username: "ada".into(),
        password: "pw".into(),
    });
    ctx.run(&req).await.unwrap();

    req.auth = Some(Auth::Bearer {
        token: "tok-1".into(),
    });
    ctx.run(&req).await.unwrap();

    req.auth = Some(Auth::ApiKey {
        key: "api_key".into(),
        value: "v1".into(),
        placement: ApiKeyPlacement::Query,
    });
    ctx.run(&req).await.unwrap();

    let received = server.received_requests().await.unwrap();
    assert_eq!(
        received[0]
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap(),
        "Basic YWRhOnB3"
    );
    assert_eq!(
        received[1]
            .headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap(),
        "Bearer tok-1"
    );
    assert!(received[2].url.query().unwrap().contains("api_key=v1"));
}

#[tokio::test]
async fn custom_methods_are_supported() {
    let server = MockServer::start().await;
    Mock::given(method("PURGE"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let ctx = Ctx::new();
    let res = ctx
        .run(&base_request("PURGE", format!("{}/c", server.uri())))
        .await
        .unwrap();
    assert_eq!(res.status, 200);
}
