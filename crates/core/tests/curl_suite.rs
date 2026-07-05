//! cURL import/export corpus + round-trip.

use pretty_assertions::assert_eq;
use tomo_core::curl::{from_curl, to_curl};
use tomo_core::model::*;

#[test]
fn simple_get() {
    let req = from_curl("curl https://api.test/users").unwrap();
    assert_eq!(req.http.method, "GET");
    assert_eq!(req.http.url, "https://api.test/users");
}

#[test]
fn method_and_headers() {
    let req =
        from_curl("curl -X PUT https://api.test/x -H 'Accept: application/json' -H 'X-Trace: abc'")
            .unwrap();
    assert_eq!(req.http.method, "PUT");
    assert_eq!(req.http.headers.len(), 2);
    assert_eq!(req.http.headers[0].name, "Accept");
    assert_eq!(req.http.headers[1].value, "abc");
}

#[test]
fn json_data_infers_post_and_json_body() {
    let req = from_curl(
        "curl https://api.test/users -H 'Content-Type: application/json' -d '{\"name\":\"Ada\"}'",
    )
    .unwrap();
    assert_eq!(req.http.method, "POST");
    match req.body {
        Some(Body::Json { content }) => assert_eq!(content, "{\"name\":\"Ada\"}"),
        other => panic!("expected json body, got {other:?}"),
    }
}

#[test]
fn json_flag() {
    let req = from_curl("curl --json '{\"a\":1}' https://api.test/x").unwrap();
    assert!(matches!(req.body, Some(Body::Json { .. })));
    assert_eq!(req.http.method, "POST");
}

#[test]
fn form_urlencoded_data() {
    let req = from_curl("curl https://api.test/token -d 'grant_type=password&user=bob'").unwrap();
    match req.body {
        Some(Body::FormUrlencoded { fields }) => {
            assert_eq!(fields.len(), 2);
            assert_eq!(fields[0].name, "grant_type");
            assert_eq!(fields[1].value, "bob");
        }
        other => panic!("expected form body, got {other:?}"),
    }
}

#[test]
fn multipart_with_file() {
    let req =
        from_curl("curl https://api.test/up -F 'meta={\"v\":1}' -F 'file=@photo.png'").unwrap();
    match req.body {
        Some(Body::MultipartForm { parts }) => {
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0].kind, PartKind::Text);
            assert_eq!(parts[1].kind, PartKind::File);
            assert_eq!(parts[1].path.as_deref(), Some("photo.png"));
        }
        other => panic!("expected multipart, got {other:?}"),
    }
}

#[test]
fn basic_auth_and_bearer() {
    let basic = from_curl("curl -u admin:s3cret https://api.test/x").unwrap();
    assert!(matches!(basic.auth, Some(Auth::Basic { .. })));

    let bearer = from_curl("curl https://api.test/x -H 'Authorization: Bearer tok-123'").unwrap();
    match bearer.auth {
        Some(Auth::Bearer { token }) => assert_eq!(token, "tok-123"),
        other => panic!("expected bearer, got {other:?}"),
    }
    // the header was consumed into auth
    assert!(
        !bearer
            .http
            .headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("authorization"))
    );
}

#[test]
fn line_continuations_and_flags_ignored() {
    let req = from_curl(
        "curl -L -k -s \\\n  -X DELETE \\\n  'https://api.test/x/1' \\\n  -H 'Accept: */*'",
    )
    .unwrap();
    assert_eq!(req.http.method, "DELETE");
    assert_eq!(req.http.url, "https://api.test/x/1");
    assert_eq!(req.http.headers.len(), 1);
}

#[test]
fn get_flag_keeps_get_with_data() {
    let req = from_curl("curl -G https://api.test/search -d 'q=rust'").unwrap();
    assert_eq!(req.http.method, "GET");
}

#[test]
fn export_round_trips_through_import() {
    let cases = vec![
        {
            let mut r = RequestFile::default();
            r.http.method = "POST".into();
            r.http.url = "https://api.test/users".into();
            r.http.headers = vec![Pair::new("Accept", "application/json")];
            r.body = Some(Body::Json {
                content: "{\"name\":\"Ada\"}".into(),
            });
            r
        },
        {
            let mut r = RequestFile::default();
            r.http.method = "GET".into();
            r.http.url = "https://api.test/x".into();
            r.auth = Some(Auth::Bearer {
                token: "tok".into(),
            });
            r
        },
    ];

    for original in cases {
        let curl = to_curl(&original);
        let back = from_curl(&curl).unwrap();
        assert_eq!(back.http.method, original.http.method, "curl was: {curl}");
        assert_eq!(back.http.url, original.http.url);
        // body/auth shape survives (name differs — imports are always "Imported…")
        assert_eq!(
            std::mem::discriminant(&back.body),
            std::mem::discriminant(&original.body),
            "body kind for: {curl}"
        );
    }
}

#[test]
fn export_quotes_special_characters() {
    let mut r = RequestFile::default();
    r.http.url = "https://api.test/x".into();
    r.body = Some(Body::Text {
        content: "it's a test".into(),
    });
    let curl = to_curl(&r);
    assert!(curl.contains(r#"'it'\''s a test'"#), "{curl}");
    // and it re-imports
    assert!(from_curl(&curl).is_ok());
}
