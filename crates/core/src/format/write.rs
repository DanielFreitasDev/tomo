//! Canonical document builders for NEW files — the beautiful, hand-editable
//! style shown in docs/format.md. Existing files are edited via `sync` instead.

use indexmap::IndexMap;
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use super::value::{
    assert_inline, items_array, multiline, pair_inline, part_inline, scalar, string_array,
};
use crate::CoreError;
use crate::model::{
    ApiKeyPlacement, Assert, Auth, Body, ClientAuth, CollectionFile, EnvironmentFile, FolderFile,
    OAuth2Grant, RequestFile, Scripts, SecretsFile, Settings, VarValue,
};

pub fn request_to_string(req: &RequestFile) -> Result<String, CoreError> {
    Ok(request_document(req)?.to_string())
}

pub fn collection_to_string(c: &CollectionFile) -> Result<String, CoreError> {
    Ok(collection_document(c)?.to_string())
}

pub fn folder_to_string(f: &FolderFile) -> Result<String, CoreError> {
    Ok(folder_document(f)?.to_string())
}

pub fn environment_to_string(e: &EnvironmentFile) -> Result<String, CoreError> {
    Ok(environment_document(e)?.to_string())
}

pub fn secrets_to_string(s: &SecretsFile) -> String {
    let doc = secrets_document(s);
    format!(
        "# Tomo secrets — never commit this file.\n# Values here fill variables listed under `meta.secrets` in environment files.\n\n{doc}"
    )
}

pub fn settings_to_string(s: &Settings) -> Result<String, CoreError> {
    toml_edit::ser::to_string_pretty(s)
        .map_err(|e| CoreError::Invalid(format!("settings serialize: {e}")))
}

pub(super) fn validate_asserts(asserts: &[Assert]) -> Result<(), CoreError> {
    for a in asserts {
        if let Some(v) = &a.value
            && contains_null(v)
        {
            return Err(CoreError::Invalid(format!(
                "assert `{}`: null values are not representable — use the isNull operator",
                a.expr
            )));
        }
    }
    Ok(())
}

/// TOML has no null, so a null anywhere in an assert value (including nested in
/// an array/object for `in`/`notIn`) is unwritable. Catch it here rather than
/// letting the value builder panic on a user-typed `[200, null]`.
fn contains_null(v: &VarValue) -> bool {
    match v {
        VarValue::Null => true,
        VarValue::Array(items) => items.iter().any(contains_null),
        VarValue::Object(map) => map.values().any(contains_null),
        _ => false,
    }
}

fn request_document(req: &RequestFile) -> Result<DocumentMut, CoreError> {
    validate_asserts(&req.tests.asserts)?;
    let mut doc = DocumentMut::new();
    let root = doc.as_table_mut();

    let mut meta = Table::new();
    meta.insert("name", value(req.meta.name.as_str()));
    if let Some(seq) = req.meta.seq {
        meta.insert("seq", value(seq));
    }
    root.insert("meta", Item::Table(meta));

    let mut http = Table::new();
    http.insert("method", value(req.http.method.as_str()));
    http.insert("url", value(req.http.url.as_str()));
    if !req.http.headers.is_empty() {
        http.insert(
            "headers",
            Item::Value(items_array(&req.http.headers, pair_inline)),
        );
    }
    if !req.http.query.is_empty() {
        http.insert(
            "query",
            Item::Value(items_array(&req.http.query, pair_inline)),
        );
    }
    if !req.http.path.is_empty() {
        http.insert(
            "path",
            Item::Value(items_array(&req.http.path, pair_inline)),
        );
    }
    root.insert("http", Item::Table(http));

    if let Some(auth) = &req.auth {
        root.insert("auth", Item::Table(auth_table(auth)));
    }
    if let Some(body) = &req.body {
        root.insert("body", Item::Table(body_table(body)));
    }
    if !req.vars.is_empty() {
        root.insert("vars", Item::Table(vars_table(&req.vars)?));
    }
    if !req.scripts.is_empty() {
        root.insert("scripts", Item::Table(scripts_table(&req.scripts)));
    }
    if !req.tests.is_empty() {
        let mut tests = Table::new();
        tests.insert(
            "asserts",
            Item::Value(items_array(&req.tests.asserts, assert_inline)),
        );
        root.insert("tests", Item::Table(tests));
    }
    if !req.options.is_empty() {
        let mut opts = Table::new();
        if let Some(v) = req.options.timeout_ms {
            opts.insert("timeout_ms", value(v as i64));
        }
        if let Some(v) = req.options.follow_redirects {
            opts.insert("follow_redirects", value(v));
        }
        if let Some(v) = req.options.max_redirects {
            opts.insert("max_redirects", value(v as i64));
        }
        if let Some(v) = req.options.ssl_verify {
            opts.insert("ssl_verify", value(v));
        }
        root.insert("options", Item::Table(opts));
    }
    if !req.docs.is_empty() {
        let mut docs = Table::new();
        docs.insert("content", Item::Value(multiline(&req.docs.content)));
        root.insert("docs", Item::Table(docs));
    }

    pretty_sections(&mut doc);
    Ok(doc)
}

fn collection_document(c: &CollectionFile) -> Result<DocumentMut, CoreError> {
    let mut doc = DocumentMut::new();
    let root = doc.as_table_mut();

    let mut meta = Table::new();
    meta.insert("name", value(c.meta.name.as_str()));
    meta.insert("format", value(c.meta.format as i64));
    root.insert("meta", Item::Table(meta));

    if !c.defaults.is_empty() {
        let mut d = Table::new();
        d.insert(
            "headers",
            Item::Value(items_array(&c.defaults.headers, pair_inline)),
        );
        root.insert("defaults", Item::Table(d));
    }
    if let Some(auth) = &c.auth {
        root.insert("auth", Item::Table(auth_table(auth)));
    }
    if !c.vars.is_empty() {
        root.insert("vars", Item::Table(vars_table(&c.vars)?));
    }
    if !c.scripts.is_empty() {
        root.insert("scripts", Item::Table(scripts_table(&c.scripts)));
    }
    if !c.tls.is_empty() {
        let mut aot = ArrayOfTables::new();
        for cert in &c.tls.client_certs {
            let mut t = Table::new();
            t.insert("host", value(cert.host.as_str()));
            t.insert("cert", value(cert.cert.as_str()));
            t.insert("key", value(cert.key.as_str()));
            aot.push(t);
        }
        let mut tls = Table::new();
        tls.set_implicit(true);
        tls.insert("client_certs", Item::ArrayOfTables(aot));
        root.insert("tls", Item::Table(tls));
    }

    pretty_sections(&mut doc);
    Ok(doc)
}

fn folder_document(f: &FolderFile) -> Result<DocumentMut, CoreError> {
    let mut doc = DocumentMut::new();
    let root = doc.as_table_mut();

    let mut meta = Table::new();
    meta.insert("name", value(f.meta.name.as_str()));
    if let Some(seq) = f.meta.seq {
        meta.insert("seq", value(seq));
    }
    root.insert("meta", Item::Table(meta));

    if !f.defaults.is_empty() {
        let mut d = Table::new();
        d.insert(
            "headers",
            Item::Value(items_array(&f.defaults.headers, pair_inline)),
        );
        root.insert("defaults", Item::Table(d));
    }
    if let Some(auth) = &f.auth {
        root.insert("auth", Item::Table(auth_table(auth)));
    }
    if !f.vars.is_empty() {
        root.insert("vars", Item::Table(vars_table(&f.vars)?));
    }
    if !f.scripts.is_empty() {
        root.insert("scripts", Item::Table(scripts_table(&f.scripts)));
    }

    pretty_sections(&mut doc);
    Ok(doc)
}

fn environment_document(e: &EnvironmentFile) -> Result<DocumentMut, CoreError> {
    let mut doc = DocumentMut::new();
    let root = doc.as_table_mut();

    let mut meta = Table::new();
    meta.insert("name", value(e.meta.name.as_str()));
    if !e.meta.secrets.is_empty() {
        meta.insert("secrets", Item::Value(string_array(&e.meta.secrets)));
    }
    root.insert("meta", Item::Table(meta));

    if !e.vars.is_empty() {
        root.insert("vars", Item::Table(vars_table(&e.vars)?));
    }

    pretty_sections(&mut doc);
    Ok(doc)
}

fn secrets_document(s: &SecretsFile) -> DocumentMut {
    let mut doc = DocumentMut::new();
    let root = doc.as_table_mut();

    if !s.collection.is_empty() {
        let mut t = Table::new();
        for (k, v) in &s.collection {
            t.insert(k, value(v.as_str()));
        }
        root.insert("collection", Item::Table(t));
    }
    if !s.environments.is_empty() {
        let mut envs = Table::new();
        envs.set_implicit(true);
        for (env, vars) in &s.environments {
            let mut t = Table::new();
            for (k, v) in vars {
                t.insert(k, value(v.as_str()));
            }
            envs.insert(env, Item::Table(t));
        }
        root.insert("environments", Item::Table(envs));
    }

    pretty_sections(&mut doc);
    doc
}

pub(super) fn vars_table(vars: &IndexMap<String, VarValue>) -> Result<Table, CoreError> {
    let mut t = Table::new();
    for (k, v) in vars {
        t.insert(k, Item::Value(scalar(v)?));
    }
    Ok(t)
}

pub(super) fn scripts_table(s: &Scripts) -> Table {
    let mut t = Table::new();
    if let Some(pre) = &s.pre_request {
        t.insert("pre_request", Item::Value(multiline(pre)));
    }
    if let Some(post) = &s.post_response {
        t.insert("post_response", Item::Value(multiline(post)));
    }
    t
}

pub(super) fn auth_table(a: &Auth) -> Table {
    let mut t = Table::new();
    match a {
        Auth::None => {
            t.insert("type", value("none"));
        }
        Auth::Inherit => {
            t.insert("type", value("inherit"));
        }
        Auth::Basic { username, password } => {
            t.insert("type", value("basic"));
            t.insert("username", value(username.as_str()));
            t.insert("password", value(password.as_str()));
        }
        Auth::Bearer { token } => {
            t.insert("type", value("bearer"));
            t.insert("token", value(token.as_str()));
        }
        Auth::ApiKey {
            key,
            value: v,
            placement,
        } => {
            t.insert("type", value("api_key"));
            t.insert("key", value(key.as_str()));
            t.insert("value", value(v.as_str()));
            if *placement != ApiKeyPlacement::default() {
                t.insert("placement", value("query"));
            }
        }
        Auth::Digest { username, password } => {
            t.insert("type", value("digest"));
            t.insert("username", value(username.as_str()));
            t.insert("password", value(password.as_str()));
        }
        Auth::Oauth2(cfg) => {
            t.insert("type", value("oauth2"));
            t.insert(
                "grant",
                value(match cfg.grant {
                    OAuth2Grant::ClientCredentials => "client_credentials",
                    OAuth2Grant::Password => "password",
                }),
            );
            t.insert("token_url", value(cfg.token_url.as_str()));
            t.insert("client_id", value(cfg.client_id.as_str()));
            if !cfg.client_secret.is_empty() {
                t.insert("client_secret", value(cfg.client_secret.as_str()));
            }
            if let Some(u) = &cfg.username {
                t.insert("username", value(u.as_str()));
            }
            if let Some(p) = &cfg.password {
                t.insert("password", value(p.as_str()));
            }
            if !cfg.scopes.is_empty() {
                t.insert("scopes", Item::Value(string_array(&cfg.scopes)));
            }
            if cfg.client_auth != ClientAuth::default() {
                t.insert("client_auth", value("body"));
            }
            if !cfg.cache_token {
                t.insert("cache_token", value(false));
            }
        }
    }
    t
}

pub(super) fn body_table(b: &Body) -> Table {
    let mut t = Table::new();
    match b {
        Body::Json { content } => {
            t.insert("type", value("json"));
            t.insert("content", Item::Value(multiline(content)));
        }
        Body::Text { content } => {
            t.insert("type", value("text"));
            t.insert("content", Item::Value(multiline(content)));
        }
        Body::Xml { content } => {
            t.insert("type", value("xml"));
            t.insert("content", Item::Value(multiline(content)));
        }
        Body::FormUrlencoded { fields } => {
            t.insert("type", value("form_urlencoded"));
            t.insert("fields", Item::Value(items_array(fields, pair_inline)));
        }
        Body::MultipartForm { parts } => {
            t.insert("type", value("multipart_form"));
            t.insert("parts", Item::Value(items_array(parts, part_inline)));
        }
        Body::Binary { path, content_type } => {
            t.insert("type", value("binary"));
            t.insert("path", value(path.as_str()));
            if let Some(ct) = content_type {
                t.insert("content_type", value(ct.as_str()));
            }
        }
        Body::Graphql { query, variables } => {
            t.insert("type", value("graphql"));
            t.insert("query", Item::Value(multiline(query)));
            if let Some(vars) = variables {
                t.insert("variables", Item::Value(multiline(vars)));
            }
        }
    }
    t
}

/// Blank line between top-level sections (skip the first).
fn pretty_sections(doc: &mut DocumentMut) {
    let mut first = true;
    for (_, item) in doc.as_table_mut().iter_mut() {
        match item {
            Item::Table(t) => {
                if t.is_implicit() {
                    // decor lives on the nested tables (e.g. [environments.dev], [[tls.client_certs]])
                    let mut inner_first = true;
                    for (_, inner) in t.iter_mut() {
                        match inner {
                            Item::Table(it) => {
                                if !(first && inner_first) {
                                    it.decor_mut().set_prefix("\n");
                                }
                                inner_first = false;
                            }
                            Item::ArrayOfTables(aot) => {
                                for (i, at) in aot.iter_mut().enumerate() {
                                    if !(first && inner_first && i == 0) {
                                        at.decor_mut().set_prefix("\n");
                                    }
                                }
                                inner_first = false;
                            }
                            _ => {}
                        }
                    }
                } else if !first {
                    t.decor_mut().set_prefix("\n");
                }
                first = false;
            }
            Item::ArrayOfTables(aot) => {
                for (i, at) in aot.iter_mut().enumerate() {
                    if !(first && i == 0) {
                        at.decor_mut().set_prefix("\n");
                    }
                }
                first = false;
            }
            _ => {}
        }
    }
}
