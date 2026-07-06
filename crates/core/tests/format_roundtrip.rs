//! Golden tests for the TOML format layer: canonical style snapshots,
//! parse-back equality for every variant, and surgical-sync guarantees
//! (no-op is byte-identical; a single change touches a single line).

use std::path::Path;

use indexmap::IndexMap;
use pretty_assertions::assert_eq;
use tomo_core::format::{
    collection_to_string, environment_to_string, folder_to_string, parse_collection,
    parse_environment, parse_folder, parse_request, parse_secrets, parse_settings,
    request_to_string, secrets_to_string, settings_to_string, sync_request,
};
use tomo_core::model::*;

fn p() -> &'static Path {
    Path::new("test.toml")
}

fn full_request() -> RequestFile {
    RequestFile {
        meta: RequestMeta {
            name: "Create user".into(),
            seq: Some(3),
        },
        http: HttpDef {
            method: "POST".into(),
            url: "{{base_url}}/api/v2/users/:id/posts".into(),
            headers: vec![
                Pair::new("Content-Type", "application/json"),
                Pair::new("X-Request-Id", "{{$uuid}}"),
                Pair::disabled("X-Debug", "1"),
            ],
            query: vec![Pair::new("notify", "true"), Pair::disabled("verbose", "1")],
            path: vec![Pair::new("id", "{{user_id}}")],
        },
        auth: Some(Auth::Bearer {
            token: "{{access_token}}".into(),
        }),
        body: Some(Body::Json {
            content: "{\n  \"name\": \"Ada Lovelace\",\n  \"tags\": [\"math\", \"computing\"]\n}\n"
                .into(),
        }),
        vars: IndexMap::from([
            ("user_id".to_string(), serde_json::json!("42")),
            ("retries".to_string(), serde_json::json!(3)),
            ("strict".to_string(), serde_json::json!(true)),
        ]),
        scripts: Scripts {
            pre_request: Some("vars.set(\"nonce\", \"n-1\");\n".into()),
            post_response: Some(
                "test(\"created\", () => { expect(res.status).toBe(201); });\n".into(),
            ),
        },
        tests: Tests {
            asserts: vec![
                Assert {
                    expr: "res.status".into(),
                    op: AssertOp::Eq,
                    value: Some(serde_json::json!(201)),
                    enabled: true,
                },
                Assert {
                    expr: "res.body.id".into(),
                    op: AssertOp::IsDefined,
                    value: None,
                    enabled: true,
                },
                Assert {
                    expr: "res.headers.content-type".into(),
                    op: AssertOp::Matches,
                    value: Some(serde_json::json!("^application/json")),
                    enabled: false,
                },
            ],
        },
        options: RequestOptions {
            timeout_ms: Some(10_000),
            follow_redirects: Some(false),
            max_redirects: None,
            ssl_verify: None,
        },
        docs: Docs {
            content: "# Create user\nCreates a user and returns `201`.\n".into(),
        },
    }
}

#[test]
fn canonical_request_snapshot() {
    let text = request_to_string(&full_request()).unwrap();
    insta::assert_snapshot!(text);
}

#[test]
fn canonical_request_parses_back_identically() {
    let req = full_request();
    let text = request_to_string(&req).unwrap();
    let back = parse_request(&text, p()).unwrap();
    assert_eq!(back, req);
}

#[test]
fn every_body_variant_round_trips() {
    let bodies = vec![
        Body::Json {
            content: "{\"a\":1}".into(), // no trailing newline on purpose
        },
        Body::Text {
            content: "hello\nworld\n".into(),
        },
        Body::Xml {
            content: "<a>1</a>\n".into(),
        },
        Body::FormUrlencoded {
            fields: vec![
                Pair::new("grant_type", "password"),
                Pair::disabled("debug", "1"),
            ],
        },
        Body::MultipartForm {
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
                    name: "avatar".into(),
                    kind: PartKind::File,
                    value: None,
                    path: Some("assets/avatar.png".into()),
                    content_type: Some("image/png".into()),
                    enabled: false,
                },
            ],
        },
        Body::Binary {
            path: "payloads/firmware.bin".into(),
            content_type: Some("application/octet-stream".into()),
        },
        Body::Graphql {
            query: "query User($id: ID!) {\n  user(id: $id) { name }\n}\n".into(),
            variables: Some("{ \"id\": \"{{user_id}}\" }\n".into()),
        },
    ];

    for body in bodies {
        let mut req = full_request();
        req.body = Some(body.clone());
        let text = request_to_string(&req).unwrap();
        let back = parse_request(&text, p()).unwrap();
        assert_eq!(
            back.body,
            Some(body),
            "body variant must round-trip:\n{text}"
        );
    }
}

#[test]
fn every_auth_variant_round_trips() {
    let auths = vec![
        Auth::None,
        Auth::Inherit,
        Auth::Basic {
            username: "u".into(),
            password: "{{pw}}".into(),
        },
        Auth::Bearer { token: "t".into() },
        Auth::ApiKey {
            key: "X-Api-Key".into(),
            value: "{{key}}".into(),
            placement: ApiKeyPlacement::Query,
        },
        Auth::Digest {
            username: "admin".into(),
            password: "s".into(),
        },
        Auth::Oauth2(OAuth2Config {
            grant: OAuth2Grant::ClientCredentials,
            token_url: "https://id.example/token".into(),
            client_id: "cid".into(),
            client_secret: "{{secret}}".into(),
            username: None,
            password: None,
            scopes: vec!["read".into(), "write".into()],
            client_auth: ClientAuth::Body,
            cache_token: false,
        }),
        Auth::Oauth2(OAuth2Config {
            grant: OAuth2Grant::Password,
            token_url: "https://id.example/token".into(),
            client_id: "cid".into(),
            client_secret: String::new(),
            username: Some("bob".into()),
            password: Some("{{pw}}".into()),
            scopes: vec![],
            client_auth: ClientAuth::BasicHeader,
            cache_token: true,
        }),
    ];

    for auth in auths {
        let mut req = full_request();
        req.auth = Some(auth.clone());
        let text = request_to_string(&req).unwrap();
        let back = parse_request(&text, p()).unwrap();
        assert_eq!(
            back.auth,
            Some(auth),
            "auth variant must round-trip:\n{text}"
        );
    }
}

// ---------------------------------------------------------------------------
// surgical sync
// ---------------------------------------------------------------------------

/// A hand-written file, full of comments and personal style choices.
const HAND_WRITTEN: &str = r#"# Users API — create
# Owner: identity team

[meta]
name = "Create user" # renamed in v2
seq = 3

[http]
method = "POST"
url = "https://api.acme.test/users"
headers = [
  { name = "Content-Type", value = "application/json" }, # required
  { name = "X-Debug", value = "1", enabled = false },
]

[body]
type = "json"
content = '''
{ "name": "Ada" }
'''

# request-scoped variables
[vars]
user_id = "42"

[options]
timeout_ms = 5000
"#;

#[test]
fn noop_sync_is_byte_identical() {
    let parsed = parse_request(HAND_WRITTEN, p()).unwrap();
    let out = sync_request(HAND_WRITTEN, &parsed, p()).unwrap();
    assert_eq!(out, HAND_WRITTEN);
}

#[test]
fn crlf_noop_sync_preserves_line_endings() {
    // A Windows/CRLF file must survive a no-op save byte-for-byte, or every
    // line shows as changed in git the first time it's saved.
    let crlf = HAND_WRITTEN.replace('\n', "\r\n");
    let parsed = parse_request(&crlf, p()).unwrap();
    let out = sync_request(&crlf, &parsed, p()).unwrap();
    assert_eq!(out, crlf);
    assert!(out.contains("\r\n"));
}

#[test]
fn auth_field_edit_keeps_sibling_comment() {
    let src = "[meta]\nname = \"R\"\nseq = 1\n\n[http]\nmethod = \"GET\"\nurl = \"https://api.test/x\"\n\n[auth]\ntype = \"basic\"\nusername = \"ops\" # ops account\npassword = \"old\"\n";
    let mut parsed = parse_request(src, p()).unwrap();
    if let Some(Auth::Basic { password, .. }) = parsed.auth.as_mut() {
        *password = "new".into();
    } else {
        panic!("expected basic auth");
    }
    let out = sync_request(src, &parsed, p()).unwrap();
    assert!(
        out.contains("# ops account"),
        "sibling comment on the untouched username survives:\n{out}"
    );
    assert!(out.contains(r#"password = "new""#));
    assert_eq!(parse_request(&out, p()).unwrap(), parsed);
}

#[test]
fn body_content_edit_keeps_type_comment() {
    let src = "[meta]\nname = \"R\"\nseq = 1\n\n[http]\nmethod = \"POST\"\nurl = \"https://api.test/x\"\n\n[body]\ntype = \"json\" # always json\ncontent = '''\n{ \"a\": 1 }\n'''\n";
    let mut parsed = parse_request(src, p()).unwrap();
    parsed.body = Some(Body::Json {
        content: "{ \"a\": 2 }\n".into(),
    });
    let out = sync_request(src, &parsed, p()).unwrap();
    assert!(
        out.contains("# always json"),
        "type comment survives a content edit:\n{out}"
    );
    assert!(out.contains("\"a\": 2"));
    assert_eq!(parse_request(&out, p()).unwrap(), parsed);
}

#[test]
fn url_change_touches_exactly_one_line() {
    let mut parsed = parse_request(HAND_WRITTEN, p()).unwrap();
    parsed.http.url = "https://api.acme.test/v2/users".into();
    let out = sync_request(HAND_WRITTEN, &parsed, p()).unwrap();

    let changed: Vec<(&str, &str)> = HAND_WRITTEN
        .lines()
        .zip(out.lines())
        .filter(|(a, b)| a != b)
        .collect();
    assert_eq!(HAND_WRITTEN.lines().count(), out.lines().count());
    assert_eq!(changed.len(), 1, "got {changed:?}");
    assert!(changed[0].1.contains("/v2/users"));
    // all comments intact
    assert!(out.contains("# Users API — create"));
    assert!(out.contains("# renamed in v2"));
    assert!(out.contains("# required"));
    assert!(out.contains("# request-scoped variables"));
}

#[test]
fn header_edit_in_place_keeps_sibling_comment() {
    let mut parsed = parse_request(HAND_WRITTEN, p()).unwrap();
    parsed.http.headers[1].value = "2".into(); // same count -> in-place item edit
    let out = sync_request(HAND_WRITTEN, &parsed, p()).unwrap();

    assert!(
        out.contains("# required"),
        "sibling comment must survive:\n{out}"
    );
    assert!(out.contains(r#"{ name = "X-Debug", value = "2", enabled = false }"#));
    let back = parse_request(&out, p()).unwrap();
    assert_eq!(back, parsed);
}

#[test]
fn adding_and_removing_sections() {
    let mut parsed = parse_request(HAND_WRITTEN, p()).unwrap();
    parsed.auth = Some(Auth::Bearer {
        token: "{{token}}".into(),
    });
    parsed.docs = Docs {
        content: "New docs\n".into(),
    };
    let out = sync_request(HAND_WRITTEN, &parsed, p()).unwrap();
    let back = parse_request(&out, p()).unwrap();
    assert_eq!(back, parsed, "sections added:\n{out}");

    // now remove them again + drop the body
    let mut trimmed = back.clone();
    trimmed.auth = None;
    trimmed.docs = Docs::default();
    trimmed.body = None;
    let out2 = sync_request(&out, &trimmed, p()).unwrap();
    assert!(!out2.contains("[auth]"));
    assert!(!out2.contains("[docs]"));
    assert!(!out2.contains("[body]"));
    let back2 = parse_request(&out2, p()).unwrap();
    assert_eq!(back2, trimmed);
}

#[test]
fn header_list_growth_rebuilds_array_canonically() {
    let mut parsed = parse_request(HAND_WRITTEN, p()).unwrap();
    parsed
        .http
        .headers
        .push(Pair::new("Accept", "application/json"));
    let out = sync_request(HAND_WRITTEN, &parsed, p()).unwrap();
    let back = parse_request(&out, p()).unwrap();
    assert_eq!(back.http.headers, parsed.http.headers);
    // one item per line style
    assert!(out.contains("\n  { name = \"Accept\", value = \"application/json\" },\n"));
}

#[test]
fn vars_upsert_and_remove_are_key_scoped() {
    let mut parsed = parse_request(HAND_WRITTEN, p()).unwrap();
    parsed
        .vars
        .insert("region".to_string(), serde_json::json!("sa-east-1"));
    parsed.vars.shift_remove("user_id");
    let out = sync_request(HAND_WRITTEN, &parsed, p()).unwrap();

    assert!(
        out.contains("# request-scoped variables"),
        "table comment survives"
    );
    assert!(out.contains(r#"region = "sa-east-1""#));
    assert!(!out.contains("user_id"));
    let back = parse_request(&out, p()).unwrap();
    assert_eq!(back.vars, parsed.vars);
}

// ---------------------------------------------------------------------------
// collection / folder / environment / secrets / settings
// ---------------------------------------------------------------------------

#[test]
fn collection_snapshot_and_roundtrip() {
    let col = CollectionFile {
        meta: CollectionMeta {
            name: "Acme API".into(),
            format: 1,
        },
        defaults: Defaults {
            headers: vec![Pair::new("User-Agent", "tomo/0.1")],
        },
        auth: Some(Auth::Bearer {
            token: "{{access_token}}".into(),
        }),
        vars: IndexMap::from([(
            "base_url".to_string(),
            serde_json::json!("https://api.acme.test"),
        )]),
        scripts: Scripts {
            pre_request: Some("// collection pre\n".into()),
            post_response: None,
        },
        tls: Tls {
            client_certs: vec![ClientCert {
                host: "mtls.acme.test".into(),
                cert: "certs/client.pem".into(),
                key: "certs/client-key.pem".into(),
            }],
        },
    };
    let text = collection_to_string(&col).unwrap();
    insta::assert_snapshot!(text);
    assert_eq!(parse_collection(&text, p()).unwrap(), col);
}

#[test]
fn folder_and_environment_roundtrip() {
    let folder = FolderFile {
        meta: FolderMeta {
            name: "Users".into(),
            seq: Some(2),
        },
        defaults: Defaults {
            headers: vec![Pair::new("X-Team", "identity")],
        },
        auth: Some(Auth::Inherit),
        vars: IndexMap::from([("users_path".to_string(), serde_json::json!("/api/v2/users"))]),
        scripts: Scripts::default(),
    };
    let text = folder_to_string(&folder).unwrap();
    assert_eq!(parse_folder(&text, p()).unwrap(), folder);

    let env = EnvironmentFile {
        meta: EnvMeta {
            name: "Development".into(),
            secrets: vec!["api_key".into(), "oauth_client_secret".into()],
        },
        vars: IndexMap::from([
            (
                "base_url".to_string(),
                serde_json::json!("https://dev.acme.test"),
            ),
            ("retries".to_string(), serde_json::json!(2)),
        ]),
    };
    let text = environment_to_string(&env).unwrap();
    insta::assert_snapshot!(text);
    assert_eq!(parse_environment(&text, p()).unwrap(), env);
}

#[test]
fn secrets_snapshot_and_roundtrip() {
    let secrets = SecretsFile {
        collection: IndexMap::from([("admin_pw".to_string(), "hunter2".to_string())]),
        environments: IndexMap::from([(
            "dev".to_string(),
            IndexMap::from([("api_key".to_string(), "sk-dev-123".to_string())]),
        )]),
    };
    let text = secrets_to_string(&secrets);
    insta::assert_snapshot!(text);
    assert!(text.starts_with("# Tomo secrets — never commit this file."));
    assert_eq!(parse_secrets(&text, p()).unwrap(), secrets);
}

#[test]
fn settings_roundtrip() {
    let s = Settings {
        theme: Theme::Dark,
        locale: Some("pt-BR".into()),
        ui_font_size: Some(13),
        editor_font_size: None,
        network: NetworkSettings {
            timeout_ms: 15_000,
            follow_redirects: true,
            max_redirects: 5,
            ssl_verify: false,
            response_cap_bytes: 1024,
            proxy: ProxySettings {
                mode: ProxyMode::Manual,
                url: Some("socks5://127.0.0.1:9050".into()),
            },
        },
    };
    let text = settings_to_string(&s).unwrap();
    assert_eq!(parse_settings(&text, p()).unwrap(), s);
    // defaults fill in for an empty file
    assert_eq!(parse_settings("", p()).unwrap(), Settings::default());
}

#[test]
fn parse_error_reports_line_number() {
    let bad = "[meta]\nname = \"x\"\n\n[http]\nmethod = \"GET\"\nurl = not-quoted\n";
    let err = parse_request(bad, Path::new("broken.toml")).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("broken.toml"), "{msg}");
    assert!(msg.contains("line 6"), "{msg}");
}

#[test]
fn content_without_trailing_newline_roundtrips_exactly() {
    let mut req = full_request();
    req.body = Some(Body::Text {
        content: "exact-bytes-no-newline".into(),
    });
    let text = request_to_string(&req).unwrap();
    let back = parse_request(&text, p()).unwrap();
    assert_eq!(
        back.body,
        Some(Body::Text {
            content: "exact-bytes-no-newline".into()
        })
    );
}
