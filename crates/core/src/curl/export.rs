//! Serialize a RequestFile to a `curl` command string.

use crate::model::{ApiKeyPlacement, Auth, Body, MultipartPart, OAuth2Config, Pair, RequestFile};
use crate::vars::{VarStack, interpolate};

fn quote(s: &str) -> String {
    // single-quote and escape embedded single quotes the shell way
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn to_curl(req: &RequestFile) -> String {
    let mut parts: Vec<String> = vec!["curl".into()];

    let method = req.http.method.to_ascii_uppercase();
    if !method.is_empty() && method != "GET" {
        parts.push(format!("-X {method}"));
    }

    parts.push(quote(&req.http.url));

    // enabled headers
    for h in req.http.headers.iter().filter(|h| h.enabled) {
        parts.push(format!("-H {}", quote(&format!("{}: {}", h.name, h.value))));
    }

    // auth
    match &req.auth {
        Some(Auth::Basic { username, password }) => {
            parts.push(format!("-u {}", quote(&format!("{username}:{password}"))));
        }
        Some(Auth::Bearer { token }) => {
            parts.push(format!(
                "-H {}",
                quote(&format!("Authorization: Bearer {token}"))
            ));
        }
        Some(Auth::ApiKey {
            key,
            value,
            placement,
        }) if *placement == ApiKeyPlacement::Header => {
            parts.push(format!("-H {}", quote(&format!("{key}: {value}"))));
        }
        _ => {}
    }

    // body
    match &req.body {
        Some(Body::Json { content }) => {
            parts.push(format!("-H {}", quote("Content-Type: application/json")));
            parts.push(format!("-d {}", quote(content)));
        }
        Some(Body::Text { content }) | Some(Body::Xml { content }) => {
            parts.push(format!("-d {}", quote(content)));
        }
        Some(Body::FormUrlencoded { fields }) => {
            let encoded = fields
                .iter()
                .filter(|f| f.enabled)
                .map(|f| format!("{}={}", f.name, f.value))
                .collect::<Vec<_>>()
                .join("&");
            parts.push(format!("-d {}", quote(&encoded)));
        }
        Some(Body::MultipartForm { parts: mp }) => {
            for part in mp.iter().filter(|p| p.enabled) {
                let value = match part.kind {
                    crate::model::PartKind::File => {
                        format!("@{}", part.path.clone().unwrap_or_default())
                    }
                    crate::model::PartKind::Text => part.value.clone().unwrap_or_default(),
                };
                parts.push(format!("-F {}", quote(&format!("{}={}", part.name, value))));
            }
        }
        Some(Body::Binary { path, .. }) => {
            parts.push(format!("--data-binary {}", quote(&format!("@{path}"))));
        }
        Some(Body::Graphql { query, variables }) => {
            let payload = serde_json::json!({
                "query": query,
                "variables": variables.as_deref().unwrap_or("{}"),
            });
            parts.push(format!("-H {}", quote("Content-Type: application/json")));
            parts.push(format!("-d {}", quote(&payload.to_string())));
        }
        None => {}
    }

    parts.join(" ")
}

pub fn to_curl_interpolated(req: &RequestFile, stack: &VarStack) -> String {
    to_curl(&interpolate_request(req, stack))
}

fn interp(text: &str, stack: &VarStack) -> String {
    interpolate(text, stack).text
}

fn interpolate_pair(pair: &Pair, stack: &VarStack) -> Pair {
    Pair {
        name: interp(&pair.name, stack),
        value: interp(&pair.value, stack),
        enabled: pair.enabled,
    }
}

fn interpolate_auth(auth: &Auth, stack: &VarStack) -> Auth {
    match auth {
        Auth::Basic { username, password } => Auth::Basic {
            username: interp(username, stack),
            password: interp(password, stack),
        },
        Auth::Bearer { token } => Auth::Bearer {
            token: interp(token, stack),
        },
        Auth::ApiKey {
            key,
            value,
            placement,
        } => Auth::ApiKey {
            key: interp(key, stack),
            value: interp(value, stack),
            placement: *placement,
        },
        Auth::Digest { username, password } => Auth::Digest {
            username: interp(username, stack),
            password: interp(password, stack),
        },
        Auth::Oauth2(cfg) => Auth::Oauth2(interpolate_oauth2(cfg, stack)),
        other => other.clone(),
    }
}

fn interpolate_oauth2(cfg: &OAuth2Config, stack: &VarStack) -> OAuth2Config {
    let mut out = cfg.clone();
    out.token_url = interp(&out.token_url, stack);
    out.client_id = interp(&out.client_id, stack);
    out.client_secret = interp(&out.client_secret, stack);
    out.username = out.username.as_deref().map(|value| interp(value, stack));
    out.password = out.password.as_deref().map(|value| interp(value, stack));
    out.scopes = out
        .scopes
        .iter()
        .map(|value| interp(value, stack))
        .collect();
    out
}

fn interpolate_body(body: &Body, stack: &VarStack) -> Body {
    match body {
        Body::Json { content } => Body::Json {
            content: interp(content, stack),
        },
        Body::Text { content } => Body::Text {
            content: interp(content, stack),
        },
        Body::Xml { content } => Body::Xml {
            content: interp(content, stack),
        },
        Body::FormUrlencoded { fields } => Body::FormUrlencoded {
            fields: fields
                .iter()
                .map(|field| interpolate_pair(field, stack))
                .collect(),
        },
        Body::MultipartForm { parts } => Body::MultipartForm {
            parts: parts
                .iter()
                .map(|part| interpolate_multipart_part(part, stack))
                .collect(),
        },
        Body::Binary { path, content_type } => Body::Binary {
            path: interp(path, stack),
            content_type: content_type.as_deref().map(|value| interp(value, stack)),
        },
        Body::Graphql { query, variables } => Body::Graphql {
            query: interp(query, stack),
            variables: variables.as_deref().map(|value| interp(value, stack)),
        },
    }
}

fn interpolate_multipart_part(part: &MultipartPart, stack: &VarStack) -> MultipartPart {
    MultipartPart {
        name: interp(&part.name, stack),
        kind: part.kind,
        value: part.value.as_deref().map(|value| interp(value, stack)),
        path: part.path.as_deref().map(|value| interp(value, stack)),
        content_type: part
            .content_type
            .as_deref()
            .map(|value| interp(value, stack)),
        enabled: part.enabled,
    }
}

fn interpolate_request(req: &RequestFile, stack: &VarStack) -> RequestFile {
    let mut out = req.clone();
    out.http.method = interp(&out.http.method, stack);
    out.http.url = interp(&out.http.url, stack);
    out.http.headers = out
        .http
        .headers
        .iter()
        .map(|header| interpolate_pair(header, stack))
        .collect();
    out.http.query = out
        .http
        .query
        .iter()
        .map(|query| interpolate_pair(query, stack))
        .collect();
    out.http.path = out
        .http
        .path
        .iter()
        .map(|path| interpolate_pair(path, stack))
        .collect();
    out.auth = out.auth.as_ref().map(|auth| interpolate_auth(auth, stack));
    out.body = out.body.as_ref().map(|body| interpolate_body(body, stack));
    out
}
