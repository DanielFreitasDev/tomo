//! The request pipeline orchestrator. Each step is a small function; the
//! engine only sequences them (the anti-2,253-line-file design).
//!
//! resolve chain → var stack → interpolate → build URL → auth → body →
//! send (cancellable) → capture. Scripts (M7) hook in before/after send.

use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;

use indexmap::IndexMap;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use tokio_util::sync::CancellationToken;

use crate::CoreError;
use crate::fsops::resolve_rel;
use crate::model::{
    Auth, Body, EnvironmentFile, NetworkSettings, Pair, ResponseData, SecretsFile, VarValue,
};
use crate::vars::{Interpolated, StackInputs, VarStack, Warning, interpolate};

use super::auth::apply_simple_auth;
use super::build::build_url;
use super::capture::{CaptureConfig, capture};
use super::client::{ClientOptions, build_client};
use super::cookies::TomoJar;
use super::digest::send_with_digest;
use super::oauth2::{TokenCache, get_token};
use super::resolve::{Chain, resolve_chain};

pub struct EngineConfig {
    pub network: NetworkSettings,
    pub spill_dir: PathBuf,
}

pub struct RunSpec<'a> {
    pub chain: Chain<'a>,
    pub environment: Option<&'a EnvironmentFile>,
    pub secrets: Option<&'a SecretsFile>,
    pub runtime_vars: Option<&'a IndexMap<String, VarValue>>,
    pub process_env: IndexMap<String, String>,
    pub dotenv: IndexMap<String, String>,
    pub collection_root: &'a Path,
    pub jar: Arc<TomoJar>,
    pub token_cache: Arc<TokenCache>,
    pub cancel: CancellationToken,
}

/// Effective per-run options after merging settings defaults with request overrides.
#[derive(Debug, Clone, Copy)]
pub struct EffectiveOptions {
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    pub max_redirects: u32,
    pub ssl_verify: bool,
}

pub async fn execute(cfg: &EngineConfig, spec: RunSpec<'_>) -> Result<ResponseData, CoreError> {
    let resolved = resolve_chain(&spec.chain);
    let request = spec.chain.request;

    // ---- variable stack -------------------------------------------------
    let folder_vars: Vec<&IndexMap<String, VarValue>> =
        spec.chain.folders.iter().map(|f| &f.vars).collect();
    let stack = VarStack::build(StackInputs {
        process_env: spec.process_env.clone(),
        dotenv: spec.dotenv.clone(),
        collection_vars: Some(&spec.chain.collection.vars),
        environment: spec.environment,
        secrets: spec.secrets,
        folder_vars,
        request_vars: Some(&request.vars),
        runtime_vars: spec.runtime_vars,
    });

    let mut warnings: Vec<Warning> = Vec::new();
    let mut interp = |text: &str| -> String {
        let Interpolated { text, warnings: w } = interpolate(text, &stack);
        for warning in w {
            if !warnings.contains(&warning) {
                warnings.push(warning);
            }
        }
        text
    };

    // ---- interpolate everything -----------------------------------------
    let url_text = interp(&request.http.url);
    let path_params = interp_pairs(&request.http.path, &mut interp);
    let query_params = interp_pairs(&request.http.query, &mut interp);
    let mut headers: Vec<(String, String)> = resolved
        .headers
        .iter()
        .map(|p| (interp(&p.name), interp(&p.value)))
        .collect();
    let auth = interpolate_auth(&resolved.auth, &mut interp);

    // ---- URL + auth ------------------------------------------------------
    let mut url = build_url(&url_text, &path_params, &query_params)?;
    apply_simple_auth(&auth, &mut url, &mut headers)?;

    // ---- effective options ----------------------------------------------
    let opts = EffectiveOptions {
        timeout_ms: request.options.timeout_ms.unwrap_or(cfg.network.timeout_ms),
        follow_redirects: request
            .options
            .follow_redirects
            .unwrap_or(cfg.network.follow_redirects),
        max_redirects: request
            .options
            .max_redirects
            .unwrap_or(cfg.network.max_redirects),
        ssl_verify: request.options.ssl_verify.unwrap_or(cfg.network.ssl_verify),
    };

    let client = build_client(
        &ClientOptions {
            follow_redirects: opts.follow_redirects,
            max_redirects: opts.max_redirects,
            ssl_verify: opts.ssl_verify,
            proxy: cfg.network.proxy.clone(),
        },
        spec.jar.clone(),
    )?;

    // ---- OAuth2 (network round-trip, cached) ------------------------------
    if let Auth::Oauth2(oauth_cfg) = &auth {
        let token = get_token(
            &client,
            oauth_cfg,
            &spec.token_cache,
            &spec.cancel,
            opts.timeout_ms,
        )
        .await?;
        headers.push(("Authorization".into(), format!("Bearer {token}")));
    }

    // ---- request builder --------------------------------------------------
    let method = reqwest::Method::from_str(&request.http.method.to_ascii_uppercase())
        .or_else(|_| reqwest::Method::from_bytes(request.http.method.as_bytes()))
        .map_err(|_| {
            CoreError::Invalid(format!("invalid HTTP method `{}`", request.http.method))
        })?;

    let mut header_map = HeaderMap::new();
    for (name, value) in &headers {
        let hname = HeaderName::from_str(name)
            .map_err(|_| CoreError::Invalid(format!("invalid header name `{name}`")))?;
        let hvalue = HeaderValue::from_str(value)
            .map_err(|_| CoreError::Invalid(format!("invalid value for header `{name}`")))?;
        header_map.append(hname, hvalue);
    }

    let mut builder = client
        .request(method, url)
        .headers(header_map)
        .timeout(std::time::Duration::from_millis(opts.timeout_ms));

    builder = attach_body(
        builder,
        request.body.as_ref(),
        spec.collection_root,
        &headers,
        &mut interp,
    )
    .await?;

    // ---- send (cancellable; digest does a 401-challenge round-trip) --------
    let started = Instant::now();
    let response = match &auth {
        Auth::Digest { username, password } => {
            send_with_digest(builder, username, password, &spec.cancel, opts.timeout_ms).await?
        }
        _ => tokio::select! {
            biased;
            _ = spec.cancel.cancelled() => return Err(CoreError::Cancelled),
            r = builder.send() => r.map_err(|e| CoreError::from_reqwest(e, opts.timeout_ms))?,
        },
    };

    let capture_cfg = CaptureConfig {
        cap_bytes: cfg.network.response_cap_bytes,
        spill_dir: cfg.spill_dir.clone(),
    };
    let mut data = capture(
        response,
        &capture_cfg,
        &spec.cancel,
        started,
        opts.timeout_ms,
    )
    .await?;
    data.warnings = warnings;
    Ok(data)
}

fn interp_pairs(pairs: &[Pair], interp: &mut impl FnMut(&str) -> String) -> Vec<Pair> {
    pairs
        .iter()
        .map(|p| Pair {
            name: interp(&p.name),
            value: interp(&p.value),
            enabled: p.enabled,
        })
        .collect()
}

fn interpolate_auth(auth: &Auth, interp: &mut impl FnMut(&str) -> String) -> Auth {
    match auth {
        Auth::Basic { username, password } => Auth::Basic {
            username: interp(username),
            password: interp(password),
        },
        Auth::Bearer { token } => Auth::Bearer {
            token: interp(token),
        },
        Auth::ApiKey {
            key,
            value,
            placement,
        } => Auth::ApiKey {
            key: interp(key),
            value: interp(value),
            placement: *placement,
        },
        Auth::Digest { username, password } => Auth::Digest {
            username: interp(username),
            password: interp(password),
        },
        Auth::Oauth2(cfg) => {
            let mut cfg = cfg.clone();
            cfg.token_url = interp(&cfg.token_url);
            cfg.client_id = interp(&cfg.client_id);
            cfg.client_secret = interp(&cfg.client_secret);
            cfg.username = cfg.username.as_deref().map(&mut *interp);
            cfg.password = cfg.password.as_deref().map(&mut *interp);
            Auth::Oauth2(cfg)
        }
        other => other.clone(),
    }
}

/// True when the user already set this header (case-insensitive).
fn has_header(headers: &[(String, String)], name: &str) -> bool {
    headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name))
}

async fn attach_body(
    mut builder: reqwest::RequestBuilder,
    body: Option<&Body>,
    collection_root: &Path,
    headers: &[(String, String)],
    interp: &mut impl FnMut(&str) -> String,
) -> Result<reqwest::RequestBuilder, CoreError> {
    let Some(body) = body else { return Ok(builder) };

    match body {
        Body::Json { content } => {
            if !has_header(headers, "content-type") {
                builder = builder.header("Content-Type", "application/json");
            }
            Ok(builder.body(interp(content)))
        }
        Body::Text { content } => {
            if !has_header(headers, "content-type") {
                builder = builder.header("Content-Type", "text/plain");
            }
            Ok(builder.body(interp(content)))
        }
        Body::Xml { content } => {
            if !has_header(headers, "content-type") {
                builder = builder.header("Content-Type", "application/xml");
            }
            Ok(builder.body(interp(content)))
        }
        Body::Graphql { query, variables } => {
            let variables: serde_json::Value = match variables {
                Some(v) if !v.trim().is_empty() => {
                    serde_json::from_str(&interp(v)).map_err(|e| {
                        CoreError::Invalid(format!("GraphQL variables are not valid JSON: {e}"))
                    })?
                }
                _ => serde_json::Value::Object(serde_json::Map::new()),
            };
            let payload = serde_json::json!({ "query": interp(query), "variables": variables });
            if !has_header(headers, "content-type") {
                builder = builder.header("Content-Type", "application/json");
            }
            Ok(builder.body(payload.to_string()))
        }
        Body::FormUrlencoded { fields } => {
            let pairs: Vec<(String, String)> = fields
                .iter()
                .filter(|f| f.enabled)
                .map(|f| (interp(&f.name), interp(&f.value)))
                .collect();
            Ok(builder.form(&pairs))
        }
        Body::MultipartForm { parts } => {
            let mut form = reqwest::multipart::Form::new();
            for part in parts.iter().filter(|p| p.enabled) {
                match part.kind {
                    crate::model::PartKind::Text => {
                        let value = interp(part.value.as_deref().unwrap_or(""));
                        let mut p = reqwest::multipart::Part::text(value);
                        if let Some(ct) = &part.content_type {
                            p = p.mime_str(ct).map_err(|e| {
                                CoreError::Invalid(format!("part `{}`: {e}", part.name))
                            })?;
                        }
                        form = form.part(interp(&part.name), p);
                    }
                    crate::model::PartKind::File => {
                        let rel = interp(part.path.as_deref().unwrap_or(""));
                        let abs = resolve_rel(collection_root, &rel)?;
                        let file = tokio::fs::File::open(&abs)
                            .await
                            .map_err(|e| CoreError::io(&abs, e))?;
                        let len = file
                            .metadata()
                            .await
                            .map_err(|e| CoreError::io(&abs, e))?
                            .len();
                        let stream = tokio_util::io::ReaderStream::new(file);
                        let mut p = reqwest::multipart::Part::stream_with_length(
                            reqwest::Body::wrap_stream(stream),
                            len,
                        )
                        .file_name(
                            abs.file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "file".to_string()),
                        );
                        if let Some(ct) = &part.content_type {
                            p = p.mime_str(ct).map_err(|e| {
                                CoreError::Invalid(format!("part `{}`: {e}", part.name))
                            })?;
                        }
                        form = form.part(interp(&part.name), p);
                    }
                }
            }
            Ok(builder.multipart(form))
        }
        Body::Binary { path, content_type } => {
            let rel = interp(path);
            let abs = resolve_rel(collection_root, &rel)?;
            const BINARY_MAX: u64 = 64 * 1024 * 1024;
            let len = std::fs::metadata(&abs)
                .map_err(|e| CoreError::io(&abs, e))?
                .len();
            if len > BINARY_MAX {
                return Err(CoreError::Invalid(format!(
                    "binary body larger than 64 MiB ({len} bytes) is not supported yet"
                )));
            }
            let bytes = tokio::fs::read(&abs)
                .await
                .map_err(|e| CoreError::io(&abs, e))?;
            if let Some(ct) = content_type
                && !has_header(headers, "content-type")
            {
                builder = builder.header("Content-Type", ct.as_str());
            }
            Ok(builder.body(bytes))
        }
    }
}
