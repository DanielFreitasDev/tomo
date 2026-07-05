//! Scripting + declarative asserts integrated into the request pipeline.

use std::sync::Arc;

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use tokio_util::sync::CancellationToken;
use tomo_core::CoreError;
use tomo_core::http::{Chain, EngineConfig, RunSpec, TokenCache, TomoJar, execute};
use tomo_core::model::*;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Ctx {
    collection: CollectionFile,
    folders: Vec<FolderFile>,
    tmp: tempfile::TempDir,
}

impl Ctx {
    fn new() -> Self {
        Self {
            collection: CollectionFile::new("test"),
            folders: vec![],
            tmp: tempfile::tempdir().unwrap(),
        }
    }

    async fn run(&self, req: &RequestFile) -> Result<ResponseData, CoreError> {
        let config = EngineConfig {
            network: NetworkSettings::default(),
            spill_dir: std::env::temp_dir().join("tomo-test-spill"),
        };
        execute(
            &config,
            RunSpec {
                chain: Chain {
                    collection: &self.collection,
                    folders: self.folders.iter().collect(),
                    request: req,
                },
                environment: None,
                secrets: None,
                runtime_vars: None,
                process_env: IndexMap::new(),
                dotenv: IndexMap::new(),
                collection_root: self.tmp.path(),
                jar: TomoJar::new(),
                token_cache: TokenCache::new(),
                cancel: CancellationToken::new(),
            },
        )
        .await
    }
}

fn request_to(url: String) -> RequestFile {
    RequestFile {
        meta: RequestMeta {
            name: "t".into(),
            seq: None,
        },
        http: HttpDef {
            method: "GET".into(),
            url,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The plan's full-pipeline verify-by test: a pre-script sets a var that the
/// URL template consumes.
#[tokio::test]
async fn pre_script_var_feeds_url_interpolation() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/tenants/tenant-42/status"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let mut req = request_to(format!("{}/tenants/{{{{tenant}}}}/status", server.uri()));
    req.scripts.pre_request = Some("vars.set('tenant', 'tenant-42');".into());

    let res = Ctx::new().run(&req).await.unwrap();
    assert_eq!(res.status, 200);
    assert!(res.warnings.is_empty(), "{:?}", res.warnings);
    assert_eq!(
        res.runtime_sets.get("tenant"),
        Some(&serde_json::json!("tenant-42"))
    );
}

#[tokio::test]
async fn pre_script_mutates_method_headers_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("x-signed", "sig-1"))
        .and(body_string_contains("\"injected\": true"))
        .respond_with(ResponseTemplate::new(201))
        .mount(&server)
        .await;

    let mut req = request_to(format!("{}/x", server.uri()));
    req.body = Some(Body::Json {
        content: "{\"a\": 1}".into(),
    });
    req.scripts.pre_request = Some(
        r#"
        req.method = "POST";
        req.setHeader("X-Signed", "sig-1");
        req.body.injected = true;
        "#
        .into(),
    );

    let res = Ctx::new().run(&req).await.unwrap();
    assert_eq!(res.status, 201);
}

#[tokio::test]
async fn script_chain_runs_collection_then_folder_then_request() {
    let server = MockServer::start().await;
    // note: comma-separated values would be split by wiremock's header matcher
    // (HTTP list semantics), so the accumulator uses "->" as separator
    Mock::given(method("GET"))
        .and(header("x-order", "collection->folder->request"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let mut ctx = Ctx::new();
    ctx.collection.scripts.pre_request = Some("vars.set('order', 'collection');".into());
    let mut folder = FolderFile::new("Users");
    folder.scripts.pre_request = Some("vars.set('order', vars.get('order') + '->folder');".into());
    ctx.folders.push(folder);

    let mut req = request_to(format!("{}/x", server.uri()));
    req.scripts.pre_request = Some(
        "vars.set('order', vars.get('order') + '->request'); req.setHeader('X-Order', vars.get('order'));"
            .into(),
    );

    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 200);
}

#[tokio::test]
async fn post_script_reads_response_and_records_tests_and_console() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": "u-7",
            "items": [1, 2, 3]
        })))
        .mount(&server)
        .await;

    let mut req = request_to(format!("{}/x", server.uri()));
    req.scripts.pre_request = Some("console.log('pre says hi');".into());
    req.scripts.post_response = Some(
        r#"
        console.log('status is', res.status);
        vars.set('created_id', res.body.id);
        test('status ok', () => { expect(res.status).toBe(200); });
        test('has items', () => { expect(res.body.items).toHaveLength(3); });
        test('this fails', () => { expect(res.body.id).toBe('other'); });
        "#
        .into(),
    );

    let res = Ctx::new().run(&req).await.unwrap();

    assert_eq!(res.console.len(), 2);
    assert_eq!(res.console[0].message, "pre says hi");
    assert_eq!(res.console[1].message, "status is 200");

    assert_eq!(res.tests.len(), 3);
    assert!(res.tests[0].ok && res.tests[1].ok);
    assert!(!res.tests[2].ok);
    assert!(res.tests[2].message.as_deref().unwrap().contains("u-7"));

    assert_eq!(
        res.runtime_sets.get("created_id"),
        Some(&serde_json::json!("u-7"))
    );
    assert!(res.script_error.is_none());
}

#[tokio::test]
async fn pre_script_error_aborts_with_origin() {
    let mut req = request_to("http://localhost:1/never".into());
    req.scripts.pre_request = Some("throw new Error('boom in pre');".into());

    let err = Ctx::new().run(&req).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("pre-request script error"), "{msg}");
    assert!(msg.contains("request"), "{msg}");
    assert!(msg.contains("boom in pre"), "{msg}");
}

#[tokio::test]
async fn post_script_error_keeps_the_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("kept"))
        .mount(&server)
        .await;

    let mut req = request_to(format!("{}/x", server.uri()));
    req.scripts.post_response = Some("undefined_function_call();".into());

    let res = Ctx::new().run(&req).await.unwrap();
    assert_eq!(res.status, 200);
    assert_eq!(res.body.preview_text(), "kept");
    let err = res.script_error.as_deref().unwrap();
    assert!(err.contains("request"), "{err}");
}

#[tokio::test]
async fn loop_bombs_terminate_via_runtime_limits() {
    let mut req = request_to("http://localhost:1/never".into());
    req.scripts.pre_request = Some("while (true) {}".into());

    let started = std::time::Instant::now();
    let err = Ctx::new().run(&req).await.unwrap_err();
    assert!(
        started.elapsed() < std::time::Duration::from_secs(9),
        "must not hang"
    );
    assert!(err.to_string().to_lowercase().contains("loop"), "{err}");
}

#[tokio::test]
async fn declarative_asserts_evaluate_against_response() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(201)
                .insert_header("content-type", "application/json")
                .set_body_json(serde_json::json!({
                    "id": "abc",
                    "tags": ["a", "b"],
                    "count": 5,
                    "nested": { "deep": null }
                })),
        )
        .mount(&server)
        .await;

    let mut req = request_to(format!("{}/x", server.uri()));
    req.tests.asserts = vec![
        Assert {
            expr: "res.status".into(),
            op: AssertOp::Eq,
            value: Some(serde_json::json!(201)),
            enabled: true,
        },
        Assert {
            expr: "res.status".into(),
            op: AssertOp::In,
            value: Some(serde_json::json!([200, 201, 204])),
            enabled: true,
        },
        Assert {
            expr: "res.body.id".into(),
            op: AssertOp::IsDefined,
            value: None,
            enabled: true,
        },
        Assert {
            expr: "res.body.tags".into(),
            op: AssertOp::Contains,
            value: Some(serde_json::json!("a")),
            enabled: true,
        },
        Assert {
            expr: "res.body.tags".into(),
            op: AssertOp::Length,
            value: Some(serde_json::json!(2)),
            enabled: true,
        },
        Assert {
            expr: "res.body.count".into(),
            op: AssertOp::Gt,
            value: Some(serde_json::json!(4)),
            enabled: true,
        },
        Assert {
            expr: "res.body.count".into(),
            op: AssertOp::Lte,
            value: Some(serde_json::json!(5)),
            enabled: true,
        },
        Assert {
            expr: "res.body.nested.deep".into(),
            op: AssertOp::IsNull,
            value: None,
            enabled: true,
        },
        Assert {
            expr: "res.body.missing".into(),
            op: AssertOp::IsUndefined,
            value: None,
            enabled: true,
        },
        Assert {
            expr: "res.headers.content-type".into(),
            op: AssertOp::Matches,
            value: Some(serde_json::json!("^application/json")),
            enabled: true,
        },
        Assert {
            expr: "res.responseTime".into(),
            op: AssertOp::Lt,
            value: Some(serde_json::json!(30000)),
            enabled: true,
        },
        // a failing one
        Assert {
            expr: "res.body.id".into(),
            op: AssertOp::Eq,
            value: Some(serde_json::json!("zzz")),
            enabled: true,
        },
        // a disabled one is skipped entirely
        Assert {
            expr: "res.status".into(),
            op: AssertOp::Eq,
            value: Some(serde_json::json!(500)),
            enabled: false,
        },
    ];

    let res = Ctx::new().run(&req).await.unwrap();
    assert_eq!(res.asserts.len(), 12, "disabled assert skipped");
    let failures: Vec<_> = res.asserts.iter().filter(|a| !a.ok).collect();
    assert_eq!(failures.len(), 1, "{failures:?}");
    assert_eq!(failures[0].expr, "res.body.id");
    assert!(failures[0].message.as_deref().unwrap().contains("zzz"));
}

#[tokio::test]
async fn vars_get_sees_stack_and_script_overrides_win() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("x-from-collection", "col-value"))
        .and(header("x-overridden", "script-wins"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let mut ctx = Ctx::new();
    ctx.collection
        .vars
        .insert("col_var".to_string(), serde_json::json!("col-value"));
    ctx.collection
        .vars
        .insert("both".to_string(), serde_json::json!("col-loses"));

    let mut req = request_to(format!("{}/x", server.uri()));
    req.scripts.pre_request = Some(
        r#"
        req.setHeader('X-From-Collection', vars.get('col_var'));
        vars.set('both', 'script-wins');
        req.setHeader('X-Overridden', '{{both}}');
        "#
        .into(),
    );

    let res = ctx.run(&req).await.unwrap();
    assert_eq!(res.status, 200);
}

/// Arc is used by wiremock Responders elsewhere; silence unused warnings here.
#[allow(dead_code)]
fn _t(_: Arc<()>) {}
