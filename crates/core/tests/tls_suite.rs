//! mTLS client certificates: selection by host, traversal-safe path resolution,
//! and reqwest accepting the resolved identity. `[[tls.client_certs]]` used to
//! be parsed and then ignored — a silent no-op for anyone configuring mTLS.

use tomo_core::http::{ClientOptions, TomoJar, build_client, resolve_client_identity};
use tomo_core::model::{ClientCert, Tls};

const CERT_PEM: &str = include_str!("fixtures/client_cert.pem");
const KEY_PEM: &str = include_str!("fixtures/client_key.pem");

/// Write the cert+key fixtures under a temp collection root and return the root
/// plus a `Tls` config pointing at them (relative paths, as stored on disk).
fn collection_with_cert(host: &str) -> (tempfile::TempDir, Tls) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("certs")).unwrap();
    std::fs::write(dir.path().join("certs/client.pem"), CERT_PEM).unwrap();
    std::fs::write(dir.path().join("certs/client.key"), KEY_PEM).unwrap();
    let tls = Tls {
        client_certs: vec![ClientCert {
            host: host.into(),
            cert: "certs/client.pem".into(),
            key: "certs/client.key".into(),
        }],
    };
    (dir, tls)
}

#[test]
fn resolves_the_matching_host_cert_into_a_combined_pem() {
    let (dir, tls) = collection_with_cert("api.example.com");

    let pem = resolve_client_identity(&tls, Some("api.example.com"), dir.path())
        .unwrap()
        .expect("a cert is configured for this host");
    let text = String::from_utf8(pem).unwrap();
    assert!(text.contains("BEGIN CERTIFICATE"), "cert half present");
    assert!(text.contains("BEGIN PRIVATE KEY"), "key half present");
}

#[test]
fn host_match_is_case_insensitive() {
    let (dir, tls) = collection_with_cert("api.example.com");
    assert!(
        resolve_client_identity(&tls, Some("API.Example.COM"), dir.path())
            .unwrap()
            .is_some()
    );
}

#[test]
fn no_cert_for_the_host_leaves_the_request_untouched() {
    let (dir, tls) = collection_with_cert("api.example.com");

    // a different host -> None (no client cert presented)
    assert!(
        resolve_client_identity(&tls, Some("other.example"), dir.path())
            .unwrap()
            .is_none()
    );
    // no host at all -> None
    assert!(
        resolve_client_identity(&tls, None, dir.path())
            .unwrap()
            .is_none()
    );
    // an empty tls config -> None
    assert!(
        resolve_client_identity(&Tls::default(), Some("api.example.com"), dir.path())
            .unwrap()
            .is_none()
    );
}

#[test]
fn cert_paths_are_traversal_guarded() {
    let dir = tempfile::tempdir().unwrap();
    let tls = Tls {
        client_certs: vec![ClientCert {
            host: "api.example.com".into(),
            cert: "../../../etc/passwd".into(),
            key: "certs/client.key".into(),
        }],
    };
    assert!(
        resolve_client_identity(&tls, Some("api.example.com"), dir.path()).is_err(),
        "a cert path escaping the collection root must be rejected"
    );
}

#[test]
fn reqwest_accepts_the_resolved_identity() {
    let (dir, tls) = collection_with_cert("api.example.com");
    let pem = resolve_client_identity(&tls, Some("api.example.com"), dir.path())
        .unwrap()
        .unwrap();

    // build_client succeeding proves rustls parsed the cert+key and accepted
    // them as a matching identity (a malformed or mismatched pair errors here).
    let client = build_client(
        &ClientOptions {
            client_identity_pem: Some(pem),
            ..Default::default()
        },
        TomoJar::new(),
    );
    assert!(
        client.is_ok(),
        "reqwest should accept a valid client identity"
    );
}

#[test]
fn a_malformed_identity_pem_is_a_clean_error_not_a_panic() {
    let client = build_client(
        &ClientOptions {
            client_identity_pem: Some(b"-----BEGIN CERTIFICATE-----\nnot base64\n".to_vec()),
            ..Default::default()
        },
        TomoJar::new(),
    );
    assert!(client.is_err());
}
