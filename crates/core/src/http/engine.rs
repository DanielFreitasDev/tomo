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
    Auth, Body, EnvironmentFile, NetworkSettings, Pair, ResponseData, Scripts, SecretsFile,
    VarValue,
};
use crate::script::{HeaderEntry, Phase, ScriptHttp, ScriptRun, ScriptSource, run_scripts};
use crate::vars::{Interpolated, StackInputs, VarStack, Warning, interpolate};

use super::auth::apply_simple_auth;
use super::build::build_url;
use super::capture::{CaptureConfig, capture};
use super::client::{ClientOptions, build_client, resolve_client_identity};
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

    // ---- working copies (pre-request scripts may mutate these) -----------
    let mut work_url = request.http.url.clone();
    let mut work_method = request.http.method.clone();
    let mut work_headers: Vec<Pair> = resolved.headers.clone();
    let mut work_body = request.body.clone();

    // ---- variable stack (rebuilt if pre-scripts set runtime vars) --------
    let folder_vars: Vec<&IndexMap<String, VarValue>> =
        spec.chain.folders.iter().map(|f| &f.vars).collect();
    let mut runtime_merged: IndexMap<String, VarValue> =
        spec.runtime_vars.cloned().unwrap_or_default();
    let build_stack = |runtime: &IndexMap<String, VarValue>| {
        VarStack::build(StackInputs {
            process_env: spec.process_env.clone(),
            dotenv: spec.dotenv.clone(),
            collection_vars: Some(&spec.chain.collection.vars),
            environment: spec.environment,
            secrets: spec.secrets,
            folder_vars: folder_vars.clone(),
            request_vars: Some(&request.vars),
            runtime_vars: Some(runtime),
        })
    };
    let mut stack = build_stack(&runtime_merged);

    let mut console: Vec<crate::script::ConsoleLine> = Vec::new();
    let mut runtime_sets_out: IndexMap<String, VarValue> = IndexMap::new();

    // ---- pre-request scripts (collection → folders → request) ------------
    let pre_sources = script_sources(&resolved.scripts_chain, &spec.chain, Phase::PreRequest);
    if !pre_sources.is_empty() {
        let outcome = run_scripts(ScriptRun {
            phase: Phase::PreRequest,
            sources: pre_sources,
            http: ScriptHttp {
                url: work_url.clone(),
                method: work_method.clone(),
                headers: header_entries(&work_headers),
                body: body_to_script_value(work_body.as_ref()),
            },
            response: None,
            vars_snapshot: stack.flatten(),
            env_name: spec.environment.map(|e| e.meta.name.clone()),
        })
        .await?;
        console.extend(outcome.console);
        if let Some(err) = outcome.error {
            return Err(CoreError::Invalid(format!(
                "pre-request script error ({}): {}",
                err.origin, err.message
            )));
        }
        work_url = outcome.http.url;
        work_method = outcome.http.method;
        work_headers = outcome
            .http
            .headers
            .into_iter()
            .map(|h| Pair::new(h.name, h.value))
            .collect();
        work_body = merge_script_body(work_body, outcome.http.body);
        if !outcome.var_sets.is_empty() {
            for (k, v) in outcome.var_sets {
                runtime_merged.insert(k.clone(), v.clone());
                runtime_sets_out.insert(k, v);
            }
            stack = build_stack(&runtime_merged);
        }
    }

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
    let url_text = interp(&work_url);
    let path_params = interp_pairs(&request.http.path, &mut interp);
    let query_params = interp_pairs(&request.http.query, &mut interp);
    let mut headers: Vec<(String, String)> = work_headers
        .iter()
        .filter(|p| p.enabled)
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

    // mTLS: present the collection's client certificate for this host, if any.
    let client_identity_pem = resolve_client_identity(
        &spec.chain.collection.tls,
        url.host_str(),
        spec.collection_root,
    )?;
    let client = build_client(
        &ClientOptions {
            follow_redirects: opts.follow_redirects,
            max_redirects: opts.max_redirects,
            ssl_verify: opts.ssl_verify,
            proxy: cfg.network.proxy.clone(),
            client_identity_pem,
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
        super::auth::set_authorization(&mut headers, format!("Bearer {token}"));
    }

    // ---- request builder --------------------------------------------------
    let method = reqwest::Method::from_str(&work_method.to_ascii_uppercase())
        .or_else(|_| reqwest::Method::from_bytes(work_method.as_bytes()))
        .map_err(|_| CoreError::Invalid(format!("invalid HTTP method `{work_method}`")))?;

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
        work_body.as_ref(),
        spec.collection_root,
        &headers,
        &mut interp,
    )
    .await?;

    let capture_cfg = CaptureConfig {
        cap_bytes: cfg.network.response_cap_bytes,
        spill_dir: cfg.spill_dir.clone(),
    };

    // ---- send (cancellable; digest does a 401-challenge round-trip) --------
    let started = Instant::now();
    // Keep a clone of an OAuth2 request so a token the server rejects (401) can
    // be refreshed and the request retried exactly once. `try_clone` is None for
    // streamed bodies — those just don't get the retry.
    let mut oauth_retry: Option<reqwest::Request> = None;
    let response = match &auth {
        Auth::Digest { username, password } => {
            send_with_digest(builder, username, password, &spec.cancel, opts.timeout_ms).await?
        }
        _ => {
            let request = builder
                .build()
                .map_err(|e| CoreError::from_reqwest(e, opts.timeout_ms))?;
            if matches!(&auth, Auth::Oauth2(_)) {
                oauth_retry = request.try_clone();
            }
            tokio::select! {
                biased;
                _ = spec.cancel.cancelled() => return Err(CoreError::Cancelled),
                r = client.execute(request) => r.map_err(|e| CoreError::from_reqwest(e, opts.timeout_ms))?,
            }
        }
    };

    let mut data = capture(
        response,
        &capture_cfg,
        &spec.cancel,
        started,
        opts.timeout_ms,
    )
    .await?;

    // ---- OAuth2 refresh-on-401: one retry with a freshly-minted token ------
    if data.status == 401
        && let Auth::Oauth2(oauth_cfg) = &auth
        && let Some(mut retry) = oauth_retry.take()
    {
        spec.token_cache.invalidate(oauth_cfg);
        let fresh = get_token(
            &client,
            oauth_cfg,
            &spec.token_cache,
            &spec.cancel,
            opts.timeout_ms,
        )
        .await?;
        let value = HeaderValue::from_str(&format!("Bearer {fresh}")).map_err(|_| {
            CoreError::Invalid("refreshed OAuth2 token is not a valid header".into())
        })?;
        retry
            .headers_mut()
            .insert(reqwest::header::AUTHORIZATION, value);
        let started = Instant::now();
        let response = tokio::select! {
            biased;
            _ = spec.cancel.cancelled() => return Err(CoreError::Cancelled),
            r = client.execute(retry) => r.map_err(|e| CoreError::from_reqwest(e, opts.timeout_ms))?,
        };
        data = capture(
            response,
            &capture_cfg,
            &spec.cancel,
            started,
            opts.timeout_ms,
        )
        .await?;
    }

    data.warnings = warnings;

    // parsed JSON body reused by post-scripts and asserts
    let body_json: Option<serde_json::Value> = if !data.body.truncated
        && data
            .body
            .mime
            .as_deref()
            .is_some_and(|m| m.contains("json"))
    {
        serde_json::from_slice(&data.body.bytes).ok()
    } else {
        None
    };

    // ---- post-response scripts --------------------------------------------
    let post_sources = script_sources(&resolved.scripts_chain, &spec.chain, Phase::PostResponse);
    if !post_sources.is_empty() {
        let run = ScriptRun {
            phase: Phase::PostResponse,
            sources: post_sources,
            http: ScriptHttp {
                url: data.final_url.clone(),
                method: work_method.clone(),
                headers: Vec::new(),
                body: serde_json::Value::Null,
            },
            response: Some(response_script_value(&data, body_json.as_ref())),
            vars_snapshot: stack.flatten(),
            env_name: spec.environment.map(|e| e.meta.name.clone()),
        };
        // a failing post script must never lose the response
        match run_scripts(run).await {
            Ok(outcome) => {
                console.extend(outcome.console);
                data.tests = outcome.tests;
                if let Some(err) = outcome.error {
                    data.script_error = Some(format!("({}) {}", err.origin, err.message));
                }
                for (k, v) in outcome.var_sets {
                    runtime_sets_out.insert(k, v);
                }
            }
            Err(e) => data.script_error = Some(e.to_string()),
        }
    }

    data.console = console;
    data.asserts = crate::asserts::run_asserts(&request.tests.asserts, &data, body_json.as_ref());
    data.runtime_sets = runtime_sets_out;
    Ok(data)
}

fn script_sources(chain_scripts: &[Scripts], chain: &Chain<'_>, phase: Phase) -> Vec<ScriptSource> {
    let mut out = Vec::new();
    for (i, scripts) in chain_scripts.iter().enumerate() {
        let code = match phase {
            Phase::PreRequest => scripts.pre_request.as_deref(),
            Phase::PostResponse => scripts.post_response.as_deref(),
        };
        let Some(code) = code else { continue };
        if code.trim().is_empty() {
            continue;
        }
        let origin = if i == 0 {
            "collection".to_string()
        } else if i == chain_scripts.len() - 1 {
            "request".to_string()
        } else {
            format!(
                "folder {}",
                chain
                    .folders
                    .get(i - 1)
                    .map(|f| f.meta.name.as_str())
                    .unwrap_or("?")
            )
        };
        out.push(ScriptSource {
            origin,
            code: code.to_string(),
        });
    }
    out
}

fn header_entries(pairs: &[Pair]) -> Vec<HeaderEntry> {
    pairs
        .iter()
        .filter(|p| p.enabled)
        .map(|p| HeaderEntry {
            name: p.name.clone(),
            value: p.value.clone(),
        })
        .collect()
}

fn body_to_script_value(body: Option<&Body>) -> serde_json::Value {
    match body {
        Some(Body::Json { content }) => serde_json::from_str(content)
            .unwrap_or_else(|_| serde_json::Value::String(content.clone())),
        Some(Body::Text { content }) | Some(Body::Xml { content }) => {
            serde_json::Value::String(content.clone())
        }
        Some(Body::Graphql { query, variables }) => {
            serde_json::json!({ "query": query, "variables": variables })
        }
        _ => serde_json::Value::Null,
    }
}

fn merge_script_body(original: Option<Body>, new_value: serde_json::Value) -> Option<Body> {
    let original = original?;
    Some(match original {
        Body::Json { .. } => Body::Json {
            content: match new_value {
                serde_json::Value::String(s) => s,
                other => serde_json::to_string_pretty(&other).unwrap_or_default(),
            },
        },
        Body::Text { .. } => Body::Text {
            content: script_value_to_text(new_value),
        },
        Body::Xml { .. } => Body::Xml {
            content: script_value_to_text(new_value),
        },
        Body::Graphql { query, variables } => match new_value {
            serde_json::Value::Object(map) => Body::Graphql {
                query: map
                    .get("query")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or(query),
                variables: map
                    .get("variables")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .or(variables),
            },
            _ => Body::Graphql { query, variables },
        },
        other => other,
    })
}

fn script_value_to_text(v: serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    }
}

fn response_script_value(
    data: &ResponseData,
    body_json: Option<&serde_json::Value>,
) -> serde_json::Value {
    let mut headers = serde_json::Map::new();
    for (k, v) in &data.headers {
        let key = k.to_ascii_lowercase();
        match headers.get_mut(&key) {
            Some(serde_json::Value::String(existing)) => {
                existing.push_str(", ");
                existing.push_str(v);
            }
            _ => {
                headers.insert(key, serde_json::Value::String(v.clone()));
            }
        }
    }
    let body = match body_json {
        Some(v) => v.clone(),
        None if data.body.is_binary => serde_json::Value::Null,
        None => serde_json::Value::String(data.body.preview_text()),
    };
    serde_json::json!({
        "status": data.status,
        "statusText": data.status_text,
        "headers": headers,
        "body": body,
        "responseTime": data.timing.total_ms,
        "size": data.body.total_size,
    })
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
