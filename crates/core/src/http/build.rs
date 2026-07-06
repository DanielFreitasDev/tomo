//! URL assembly: scheme normalization, `:path` params, query merging.

use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use url::Url;

use crate::CoreError;
use crate::model::Pair;

/// Characters percent-encoded inside a path segment.
const SEGMENT: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'<')
    .add(b'>')
    .add(b'`')
    .add(b'#')
    .add(b'?')
    .add(b'{')
    .add(b'}')
    .add(b'/')
    .add(b'%');

/// Build the final URL: prepend `http://` when no scheme, substitute `:name`
/// path segments from `path` params (URL-encoded), append enabled `query`
/// params after whatever query the URL already carries.
pub fn build_url(raw: &str, path_params: &[Pair], query: &[Pair]) -> Result<Url, CoreError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(CoreError::Invalid("request URL is empty".into()));
    }
    // Prepend http:// only when there is no *leading* scheme. Checking for
    // "://" anywhere would misclassify a schemeless URL whose query carries one
    // (e.g. `api.test/redirect?to=https://x`), so validate the part before the
    // first "://" is a real scheme: `^[A-Za-z][A-Za-z0-9+.-]*`.
    let has_scheme = raw.split_once("://").is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
    });
    let with_scheme = if has_scheme {
        raw.to_string()
    } else {
        format!("http://{raw}")
    };

    let mut url = Url::parse(&with_scheme)
        .map_err(|e| CoreError::Invalid(format!("invalid URL `{raw}`: {e}")))?;

    // :name path segments
    if url.path().contains(':') {
        let replaced: Vec<String> = url
            .path()
            .split('/')
            .map(|seg| {
                if let Some(name) = seg.strip_prefix(':')
                    && !name.is_empty()
                    && let Some(p) = path_params.iter().find(|p| p.enabled && p.name == name)
                {
                    return utf8_percent_encode(&p.value, SEGMENT).to_string();
                }
                seg.to_string()
            })
            .collect();
        url.set_path(&replaced.join("/"));
    }

    let enabled: Vec<&Pair> = query.iter().filter(|p| p.enabled).collect();
    if !enabled.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for p in enabled {
            pairs.append_pair(&p.name, &p.value);
        }
    }

    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_defaults_to_http() {
        assert_eq!(
            build_url("example.com/x", &[], &[]).unwrap().as_str(),
            "http://example.com/x"
        );
        assert_eq!(
            build_url("https://example.com", &[], &[]).unwrap().as_str(),
            "https://example.com/"
        );
    }

    #[test]
    fn path_params_are_substituted_and_encoded() {
        let url = build_url(
            "https://api.test/users/:id/posts/:postId",
            &[Pair::new("id", "42"), Pair::new("postId", "a b/c")],
            &[],
        )
        .unwrap();
        assert_eq!(url.as_str(), "https://api.test/users/42/posts/a%20b%2Fc");
    }

    #[test]
    fn unmatched_or_disabled_path_params_stay_verbatim() {
        let url = build_url(
            "https://api.test/users/:id",
            &[Pair::disabled("id", "42")],
            &[],
        )
        .unwrap();
        assert_eq!(url.path(), "/users/:id");
    }

    #[test]
    fn query_params_append_after_existing_query() {
        let url = build_url(
            "https://api.test/x?keep=1",
            &[],
            &[
                Pair::new("added", "2"),
                Pair::disabled("skipped", "3"),
                Pair::new("q", "a b"),
            ],
        )
        .unwrap();
        assert_eq!(url.as_str(), "https://api.test/x?keep=1&added=2&q=a+b");
    }

    #[test]
    fn empty_url_is_an_error() {
        assert!(build_url("  ", &[], &[]).is_err());
    }

    #[test]
    fn schemeless_url_with_scheme_in_query_gets_http_prefix() {
        // The `://` lives in a query value, not a leading scheme, so this must
        // be treated as schemeless and prefixed (it used to fail to parse).
        let url = build_url("api.test/redirect?to=https://example.com", &[], &[]).unwrap();
        assert_eq!(url.scheme(), "http");
        assert_eq!(url.host_str(), Some("api.test"));
        assert!(url.query().unwrap().contains("https://example.com"));
    }
}
