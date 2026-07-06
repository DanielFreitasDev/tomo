//! Auth application at the data level (headers/URL), independent of reqwest —
//! trivially unit-testable. Digest and OAuth2 land in M6 behind the same API.

use base64::Engine as _;
use url::Url;

use crate::CoreError;
use crate::model::{ApiKeyPlacement, Auth};

/// Set the `Authorization` header, replacing any existing one (case-insensitive)
/// so a configured auth scheme never emits a duplicate alongside a manual header.
pub(crate) fn set_authorization(headers: &mut Vec<(String, String)>, value: String) {
    headers.retain(|(k, _)| !k.eq_ignore_ascii_case("authorization"));
    headers.push(("Authorization".into(), value));
}

/// Apply auth by mutating headers/URL. Returns an error for schemes that
/// need a network round-trip (handled by the engine in M6).
pub fn apply_simple_auth(
    auth: &Auth,
    url: &mut Url,
    headers: &mut Vec<(String, String)>,
) -> Result<(), CoreError> {
    match auth {
        Auth::None | Auth::Inherit => Ok(()),
        Auth::Basic { username, password } => {
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
            set_authorization(headers, format!("Basic {encoded}"));
            Ok(())
        }
        Auth::Bearer { token } => {
            set_authorization(headers, format!("Bearer {token}"));
            Ok(())
        }
        Auth::ApiKey {
            key,
            value,
            placement,
        } => {
            match placement {
                ApiKeyPlacement::Header => headers.push((key.clone(), value.clone())),
                ApiKeyPlacement::Query => {
                    url.query_pairs_mut().append_pair(key, value);
                }
            }
            Ok(())
        }
        // handled by the engine with challenge/token flows (M6)
        Auth::Digest { .. } | Auth::Oauth2(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_is_base64_of_user_colon_pass() {
        let mut url = Url::parse("https://x.test/").unwrap();
        let mut headers = Vec::new();
        apply_simple_auth(
            &Auth::Basic {
                username: "ada".into(),
                password: "s3cret".into(),
            },
            &mut url,
            &mut headers,
        )
        .unwrap();
        assert_eq!(headers[0].0, "Authorization");
        assert_eq!(headers[0].1, "Basic YWRhOnMzY3JldA==");
    }

    #[test]
    fn api_key_placements() {
        let mut url = Url::parse("https://x.test/?a=1").unwrap();
        let mut headers = Vec::new();
        apply_simple_auth(
            &Auth::ApiKey {
                key: "X-Api-Key".into(),
                value: "k".into(),
                placement: ApiKeyPlacement::Header,
            },
            &mut url,
            &mut headers,
        )
        .unwrap();
        assert_eq!(headers[0], ("X-Api-Key".to_string(), "k".to_string()));

        apply_simple_auth(
            &Auth::ApiKey {
                key: "api_key".into(),
                value: "v".into(),
                placement: ApiKeyPlacement::Query,
            },
            &mut url,
            &mut headers,
        )
        .unwrap();
        assert_eq!(url.as_str(), "https://x.test/?a=1&api_key=v");
    }

    #[test]
    fn bearer_replaces_a_manual_authorization_header() {
        let mut url = Url::parse("https://x.test/").unwrap();
        let mut headers = vec![("Authorization".to_string(), "Bearer manual".to_string())];
        apply_simple_auth(
            &Auth::Bearer {
                token: "cfg".into(),
            },
            &mut url,
            &mut headers,
        )
        .unwrap();
        let auths: Vec<_> = headers
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case("authorization"))
            .collect();
        assert_eq!(
            auths.len(),
            1,
            "exactly one Authorization header on the wire"
        );
        assert_eq!(auths[0].1, "Bearer cfg");
    }
}
