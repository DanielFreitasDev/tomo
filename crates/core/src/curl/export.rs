//! Serialize a RequestFile to a `curl` command string.

use crate::model::{ApiKeyPlacement, Auth, Body, RequestFile};

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
