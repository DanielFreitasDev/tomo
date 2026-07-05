//! Parse a `curl ...` command into a RequestFile.

use crate::CoreError;
use crate::model::{Auth, Body, HttpDef, Pair, RequestFile, RequestMeta};

pub fn from_curl(input: &str) -> Result<RequestFile, CoreError> {
    // strip line continuations then shell-tokenize
    let cleaned = input.replace("\\\n", " ").replace("\\\r\n", " ");
    let tokens = shell_words::split(cleaned.trim())
        .map_err(|e| CoreError::Invalid(format!("could not parse curl command: {e}")))?;

    let mut it = tokens.into_iter().peekable();
    // skip a leading "curl"
    if it.peek().map(|s| s.as_str()) == Some("curl") {
        it.next();
    }

    let mut url: Option<String> = None;
    let mut method: Option<String> = None;
    let mut headers: Vec<Pair> = Vec::new();
    let mut data: Vec<String> = Vec::new();
    let mut form_fields: Vec<Pair> = Vec::new();
    let mut is_multipart = false;
    let mut auth: Option<Auth> = None;
    let mut get_with_data = false;
    let mut json_flag = false;

    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-X" | "--request" => method = it.next(),
            "-H" | "--header" => {
                if let Some(h) = it.next()
                    && let Some((name, value)) = h.split_once(':')
                {
                    headers.push(Pair::new(name.trim(), value.trim()));
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-ascii" | "--data-binary" => {
                if let Some(d) = it.next() {
                    data.push(d);
                }
            }
            "--data-urlencode" => {
                if let Some(d) = it.next() {
                    data.push(d);
                }
            }
            "--json" => {
                json_flag = true;
                if let Some(d) = it.next() {
                    data.push(d);
                }
            }
            "-F" | "--form" => {
                is_multipart = true;
                if let Some(f) = it.next()
                    && let Some((name, value)) = f.split_once('=')
                {
                    form_fields.push(Pair::new(name.trim(), value.trim()));
                }
            }
            "-G" | "--get" => get_with_data = true,
            "-u" | "--user" => {
                if let Some(cred) = it.next() {
                    let (u, p) = cred.split_once(':').unwrap_or((cred.as_str(), ""));
                    auth = Some(Auth::Basic {
                        username: u.to_string(),
                        password: p.to_string(),
                    });
                }
            }
            "-b" | "--cookie" => {
                if let Some(c) = it.next() {
                    headers.push(Pair::new("Cookie", c));
                }
            }
            "-A" | "--user-agent" => {
                if let Some(ua) = it.next() {
                    headers.push(Pair::new("User-Agent", ua));
                }
            }
            "-e" | "--referer" => {
                if let Some(r) = it.next() {
                    headers.push(Pair::new("Referer", r));
                }
            }
            // ignored boolean flags
            "-L" | "--location" | "-k" | "--insecure" | "-s" | "--silent" | "-v" | "--verbose"
            | "-i" | "--include" | "-f" | "--fail" | "--compressed" => {}
            other => {
                let looks_like_url = other.starts_with("http://") || other.starts_with("https://");
                if looks_like_url || (!other.starts_with('-') && url.is_none()) {
                    url = Some(other.to_string());
                }
                // unknown short flags that take a value: best-effort skip it
                else if other.starts_with('-')
                    && !other.starts_with("--")
                    && it.peek().is_some_and(|n| !n.starts_with('-'))
                {
                    it.next();
                }
            }
        }
    }

    let url = url.ok_or_else(|| CoreError::Invalid("no URL found in curl command".into()))?;

    // build body
    let mut body = None;
    if is_multipart {
        let parts = form_fields
            .into_iter()
            .map(|p| {
                let (kind, value, path) = if let Some(file) = p.value.strip_prefix('@') {
                    (crate::model::PartKind::File, None, Some(file.to_string()))
                } else {
                    (crate::model::PartKind::Text, Some(p.value), None)
                };
                crate::model::MultipartPart {
                    name: p.name,
                    kind,
                    value,
                    path,
                    content_type: None,
                    enabled: true,
                }
            })
            .collect();
        body = Some(Body::MultipartForm { parts });
    } else if !data.is_empty() {
        let joined = data.join("&");
        let content_type = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("content-type"))
            .map(|h| h.value.to_ascii_lowercase());
        if json_flag || content_type.as_deref().is_some_and(|c| c.contains("json")) {
            body = Some(Body::Json { content: joined });
        } else if content_type
            .as_deref()
            .is_some_and(|c| c.contains("urlencoded"))
            || joined.contains('=')
        {
            let fields = joined
                .split('&')
                .filter_map(|kv| kv.split_once('=').map(|(k, v)| Pair::new(k, v)))
                .collect();
            body = Some(Body::FormUrlencoded { fields });
        } else {
            body = Some(Body::Text { content: joined });
        }
    }

    // method inference: explicit -X wins; else POST if body present (unless -G)
    let method = method.unwrap_or_else(|| {
        if body.is_some() && !get_with_data {
            "POST".to_string()
        } else {
            "GET".to_string()
        }
    });

    // bearer header -> auth
    if auth.is_none()
        && let Some(pos) = headers.iter().position(|h| {
            h.name.eq_ignore_ascii_case("authorization")
                && h.value.to_lowercase().starts_with("bearer ")
        })
    {
        let token = headers[pos].value[7..].trim().to_string();
        auth = Some(Auth::Bearer { token });
        headers.remove(pos);
    }

    Ok(RequestFile {
        meta: RequestMeta {
            name: "Imported from curl".into(),
            seq: None,
        },
        http: HttpDef {
            method,
            url,
            headers,
            query: Vec::new(),
            path: Vec::new(),
        },
        auth,
        body,
        ..Default::default()
    })
}
