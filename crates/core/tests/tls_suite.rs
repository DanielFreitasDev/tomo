//! mTLS client certificates: selection by host, traversal-safe path resolution,
//! and reqwest accepting the resolved identity. `[[tls.client_certs]]` used to
//! be parsed and then ignored — a silent no-op for anyone configuring mTLS.

use tomo_core::http::{
    ClientCache, ClientOptions, TomoJar, build_client, resolve_client_identity, resolve_extra_cas,
};
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
        ..Default::default()
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
        ..Default::default()
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

// ---- extra CA bundles (`[tls] extra_cas`) --------------------------------

/// A collection root carrying a CA bundle at `certs/ca.pem` (the client cert
/// fixture doubles as a trust anchor) and a `Tls` config that references it.
fn collection_with_extra_ca() -> (tempfile::TempDir, Tls) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("certs")).unwrap();
    std::fs::write(dir.path().join("certs/ca.pem"), CERT_PEM).unwrap();
    let tls = Tls {
        extra_cas: vec!["certs/ca.pem".into()],
        ..Default::default()
    };
    (dir, tls)
}

#[test]
fn resolves_extra_ca_bundles_into_pem_bytes() {
    let (dir, tls) = collection_with_extra_ca();
    let cas = resolve_extra_cas(&tls, dir.path()).unwrap();
    assert_eq!(cas.len(), 1, "one CA bundle configured");
    let text = String::from_utf8(cas[0].clone()).unwrap();
    assert!(text.contains("BEGIN CERTIFICATE"), "CA PEM read verbatim");
}

#[test]
fn no_extra_cas_configured_yields_empty() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        resolve_extra_cas(&Tls::default(), dir.path())
            .unwrap()
            .is_empty()
    );
}

#[test]
fn extra_ca_paths_are_traversal_guarded() {
    let dir = tempfile::tempdir().unwrap();
    let tls = Tls {
        extra_cas: vec!["../../../etc/ssl/cert.pem".into()],
        ..Default::default()
    };
    assert!(
        resolve_extra_cas(&tls, dir.path()).is_err(),
        "a CA path escaping the collection root must be rejected"
    );
}

#[test]
fn reqwest_trusts_a_resolved_extra_ca() {
    let (dir, tls) = collection_with_extra_ca();
    let cas = resolve_extra_cas(&tls, dir.path()).unwrap();

    // build_client succeeding proves rustls parsed the CA bundle and accepted it
    // as an additional trust anchor on top of the system roots.
    let client = build_client(
        &ClientOptions {
            extra_ca_pems: cas,
            ..Default::default()
        },
        TomoJar::new(),
    );
    assert!(client.is_ok(), "reqwest should trust a valid extra CA");
}

#[test]
fn a_malformed_extra_ca_pem_is_a_clean_error_not_a_panic() {
    let client = build_client(
        &ClientOptions {
            extra_ca_pems: vec![b"-----BEGIN CERTIFICATE-----\nnot base64\n".to_vec()],
            ..Default::default()
        },
        TomoJar::new(),
    );
    assert!(client.is_err());
}

// ---- client cache (connection reuse) -------------------------------------

#[test]
fn client_cache_reuses_one_client_for_identical_options() {
    let cache = ClientCache::new();
    let opts = ClientOptions::default();
    cache.get_or_build(&opts, TomoJar::new()).unwrap();
    cache.get_or_build(&opts, TomoJar::new()).unwrap();
    assert_eq!(cache.len(), 1, "identical options must hit the cache");
}

#[test]
fn client_cache_keys_on_connection_affecting_options() {
    let cache = ClientCache::new();
    cache
        .get_or_build(&ClientOptions::default(), TomoJar::new())
        .unwrap();
    cache
        .get_or_build(
            &ClientOptions {
                ssl_verify: false,
                ..Default::default()
            },
            TomoJar::new(),
        )
        .unwrap();
    cache
        .get_or_build(
            &ClientOptions {
                max_redirects: 3,
                ..Default::default()
            },
            TomoJar::new(),
        )
        .unwrap();
    assert_eq!(
        cache.len(),
        3,
        "each distinct option set gets its own client"
    );
}

#[test]
fn client_cache_is_bounded() {
    let cache = ClientCache::new();
    for i in 0..12u32 {
        cache
            .get_or_build(
                &ClientOptions {
                    max_redirects: i,
                    ..Default::default()
                },
                TomoJar::new(),
            )
            .unwrap();
    }
    assert!(cache.len() <= 8, "cache is bounded, got {}", cache.len());
}

#[test]
fn client_cache_clear_drops_everything() {
    let cache = ClientCache::new();
    cache
        .get_or_build(&ClientOptions::default(), TomoJar::new())
        .unwrap();
    assert_eq!(cache.len(), 1);
    cache.clear();
    assert!(cache.is_empty());
}
