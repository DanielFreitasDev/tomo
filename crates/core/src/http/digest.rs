//! Digest auth (RFC 7616) via a hand-rolled 401-challenge flow over the
//! `digest_auth` crate — no reqwest-version coupling (the diqwest risk from
//! the plan). `qop=auth-int` is not supported (documented limitation).

use reqwest::StatusCode;
use reqwest::header::{AUTHORIZATION, WWW_AUTHENTICATE};
use tokio_util::sync::CancellationToken;

use crate::CoreError;

/// Send with digest auth: first attempt unauthenticated; on a 401 with a
/// Digest challenge, compute the Authorization answer and retry once.
/// Requires a re-sendable body (`try_clone`), so streaming multipart bodies
/// are rejected with a clear error before any bytes hit the wire.
pub async fn send_with_digest(
    builder: reqwest::RequestBuilder,
    username: &str,
    password: &str,
    cancel: &CancellationToken,
    timeout_ms: u64,
) -> Result<reqwest::Response, CoreError> {
    let retry = builder.try_clone().ok_or_else(|| {
        CoreError::Invalid(
            "digest auth needs a re-sendable body — streaming multipart bodies are not supported with digest"
                .into(),
        )
    })?;

    // capture the method for the digest context before anything is consumed
    let method = retry
        .try_clone()
        .expect("clone of a cloneable builder")
        .build()
        .map_err(|e| CoreError::Http {
            message: e.to_string(),
        })?
        .method()
        .as_str()
        .to_owned();

    let first = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        r = builder.send() => r.map_err(|e| CoreError::from_reqwest(e, timeout_ms))?,
    };

    if first.status() != StatusCode::UNAUTHORIZED {
        return Ok(first);
    }
    let Some(challenge) = first
        .headers()
        .get(WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .filter(|v| v.trim_start().to_ascii_lowercase().starts_with("digest"))
        .map(str::to_owned)
    else {
        return Ok(first); // a plain 401 — surface it as the response
    };

    let url = first.url().clone();
    let uri = match url.query() {
        Some(q) => format!("{}?{}", url.path(), q),
        None => url.path().to_string(),
    };

    let mut prompt = digest_auth::parse(&challenge).map_err(|e| CoreError::Http {
        message: format!("invalid digest challenge: {e}"),
    })?;
    let mut context = digest_auth::AuthContext::new(username, password, &uri);
    context.method = digest_auth::HttpMethod::from(method.as_str());

    let answer = prompt
        .respond(&context)
        .map_err(|e| CoreError::Http {
            message: format!("digest response failed: {e}"),
        })?
        .to_header_string();

    let second = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(CoreError::Cancelled),
        r = retry.header(AUTHORIZATION, answer).send() =>
            r.map_err(|e| CoreError::from_reqwest(e, timeout_ms))?,
    };
    Ok(second)
}
