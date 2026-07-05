//! Surgical, comment-preserving updates of EXISTING files.
//!
//! Strategy: parse the on-disk text both as a typed struct (for equality
//! comparison) and as a `DocumentMut` (for editing). Only fields whose logical
//! value changed are touched; unchanged lines — including user comments and
//! whitespace — survive byte-for-byte. A no-op sync returns the input text
//! unchanged.

use std::path::Path;

use indexmap::IndexMap;
use toml_edit::{DocumentMut, InlineTable, Item, Table, Value};

use super::de;
use super::value::{assert_inline, items_array, multiline, pair_inline, part_inline, scalar};
use super::write::{auth_table, body_table, validate_asserts};
use crate::CoreError;
use crate::model::{
    Auth, CollectionFile, EnvironmentFile, FolderFile, RequestFile, Scripts, VarValue,
};

pub fn sync_request(text: &str, req: &RequestFile, path: &Path) -> Result<String, CoreError> {
    validate_asserts(&req.tests.asserts)?;
    let cur = de::parse_request(text, path)?;
    let mut doc = parse_doc(text, path)?;
    let root = doc.as_table_mut();

    // [meta]
    if cur.meta.name != req.meta.name {
        set_scalar(
            ensure_table(root, "meta"),
            "name",
            Value::from(req.meta.name.as_str()),
        );
    }
    if cur.meta.seq != req.meta.seq {
        sync_opt(
            ensure_table(root, "meta"),
            "seq",
            req.meta.seq.map(Value::from),
        );
    }

    // [http]
    if cur.http.method != req.http.method {
        set_scalar(
            ensure_table(root, "http"),
            "method",
            Value::from(req.http.method.as_str()),
        );
    }
    if cur.http.url != req.http.url {
        set_scalar(
            ensure_table(root, "http"),
            "url",
            Value::from(req.http.url.as_str()),
        );
    }
    sync_items(
        ensure_table(root, "http"),
        "headers",
        &cur.http.headers,
        &req.http.headers,
        &pair_inline,
    );
    sync_items(
        ensure_table(root, "http"),
        "query",
        &cur.http.query,
        &req.http.query,
        &pair_inline,
    );
    sync_items(
        ensure_table(root, "http"),
        "path",
        &cur.http.path,
        &req.http.path,
        &pair_inline,
    );

    // [auth] / [body] — small tagged sections: replace wholesale on change
    sync_auth(root, &cur.auth, &req.auth);
    if cur.body != req.body {
        match &req.body {
            Some(b) => {
                root.insert("body", Item::Table(reprefix(body_table(b), root, "body")));
            }
            None => {
                root.remove("body");
            }
        }
    }

    // [vars]
    sync_vars_table(root, "vars", &cur.vars, &req.vars)?;

    // [scripts]
    sync_scripts(root, &cur.scripts, &req.scripts);

    // [tests]
    if cur.tests.asserts != req.tests.asserts {
        if req.tests.asserts.is_empty() {
            root.remove("tests");
        } else {
            sync_items(
                ensure_table(root, "tests"),
                "asserts",
                &cur.tests.asserts,
                &req.tests.asserts,
                &assert_inline,
            );
        }
    }

    // [options]
    if cur.options != req.options {
        if req.options.is_empty() {
            root.remove("options");
        } else {
            let t = ensure_table(root, "options");
            sync_opt(
                t,
                "timeout_ms",
                req.options.timeout_ms.map(|v| Value::from(v as i64)),
            );
            sync_opt(
                t,
                "follow_redirects",
                req.options.follow_redirects.map(Value::from),
            );
            sync_opt(
                t,
                "max_redirects",
                req.options.max_redirects.map(|v| Value::from(v as i64)),
            );
            sync_opt(t, "ssl_verify", req.options.ssl_verify.map(Value::from));
        }
    }

    // [docs]
    if cur.docs != req.docs {
        if req.docs.is_empty() {
            root.remove("docs");
        } else {
            set_scalar(
                ensure_table(root, "docs"),
                "content",
                multiline(&req.docs.content),
            );
        }
    }

    Ok(doc.to_string())
}

pub fn sync_collection(text: &str, c: &CollectionFile, path: &Path) -> Result<String, CoreError> {
    let cur = de::parse_collection(text, path)?;
    let mut doc = parse_doc(text, path)?;
    let root = doc.as_table_mut();

    if cur.meta.name != c.meta.name {
        set_scalar(
            ensure_table(root, "meta"),
            "name",
            Value::from(c.meta.name.as_str()),
        );
    }
    if cur.meta.format != c.meta.format {
        set_scalar(
            ensure_table(root, "meta"),
            "format",
            Value::from(c.meta.format as i64),
        );
    }
    sync_defaults(root, &cur.defaults.headers, &c.defaults.headers);
    sync_auth(root, &cur.auth, &c.auth);
    sync_vars_table(root, "vars", &cur.vars, &c.vars)?;
    sync_scripts(root, &cur.scripts, &c.scripts);

    if cur.tls != c.tls {
        if c.tls.is_empty() {
            root.remove("tls");
        } else {
            let mut aot = toml_edit::ArrayOfTables::new();
            for cert in &c.tls.client_certs {
                let mut t = Table::new();
                t.insert("host", toml_edit::value(cert.host.as_str()));
                t.insert("cert", toml_edit::value(cert.cert.as_str()));
                t.insert("key", toml_edit::value(cert.key.as_str()));
                t.decor_mut().set_prefix("\n");
                aot.push(t);
            }
            let mut tls = Table::new();
            tls.set_implicit(true);
            tls.insert("client_certs", Item::ArrayOfTables(aot));
            root.insert("tls", Item::Table(tls));
        }
    }

    Ok(doc.to_string())
}

pub fn sync_folder(text: &str, f: &FolderFile, path: &Path) -> Result<String, CoreError> {
    let cur = de::parse_folder(text, path)?;
    let mut doc = parse_doc(text, path)?;
    let root = doc.as_table_mut();

    if cur.meta.name != f.meta.name {
        set_scalar(
            ensure_table(root, "meta"),
            "name",
            Value::from(f.meta.name.as_str()),
        );
    }
    if cur.meta.seq != f.meta.seq {
        sync_opt(
            ensure_table(root, "meta"),
            "seq",
            f.meta.seq.map(Value::from),
        );
    }
    sync_defaults(root, &cur.defaults.headers, &f.defaults.headers);
    sync_auth(root, &cur.auth, &f.auth);
    sync_vars_table(root, "vars", &cur.vars, &f.vars)?;
    sync_scripts(root, &cur.scripts, &f.scripts);

    Ok(doc.to_string())
}

pub fn sync_environment(text: &str, e: &EnvironmentFile, path: &Path) -> Result<String, CoreError> {
    let cur = de::parse_environment(text, path)?;
    let mut doc = parse_doc(text, path)?;
    let root = doc.as_table_mut();

    if cur.meta.name != e.meta.name {
        set_scalar(
            ensure_table(root, "meta"),
            "name",
            Value::from(e.meta.name.as_str()),
        );
    }
    if cur.meta.secrets != e.meta.secrets {
        if e.meta.secrets.is_empty() {
            ensure_table(root, "meta").remove("secrets");
        } else {
            set_scalar(
                ensure_table(root, "meta"),
                "secrets",
                super::value::string_array(&e.meta.secrets),
            );
        }
    }
    sync_vars_table(root, "vars", &cur.vars, &e.vars)?;

    Ok(doc.to_string())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn parse_doc(text: &str, path: &Path) -> Result<DocumentMut, CoreError> {
    text.parse::<DocumentMut>()
        .map_err(|e| CoreError::TomlParse {
            path: path.to_path_buf(),
            line: e.span().map(|s| {
                text[..s.start.min(text.len())]
                    .bytes()
                    .filter(|b| *b == b'\n')
                    .count()
                    + 1
            }),
            message: e.message().to_string(),
        })
}

fn ensure_table<'a>(root: &'a mut Table, name: &str) -> &'a mut Table {
    let exists_as_table = root.get(name).is_some_and(|i| i.as_table().is_some());
    if !exists_as_table {
        root.insert(name, Item::Table(Table::new()));
        // new section appended at the end: give it a separating blank line
        if let Some(t) = root.get_mut(name).and_then(Item::as_table_mut) {
            t.decor_mut().set_prefix("\n");
        }
    }
    root.get_mut(name)
        .and_then(Item::as_table_mut)
        .expect("ensured above")
}

/// Replace a key's value while preserving its decor (leading space, trailing
/// inline comment). Inserts with default decor when the key is new.
fn set_scalar(t: &mut Table, key: &str, new: Value) {
    let decor = t
        .get(key)
        .and_then(Item::as_value)
        .map(|v| v.decor().clone());
    let mut v = new;
    if let Some(d) = decor {
        *v.decor_mut() = d;
    }
    t.insert(key, Item::Value(v));
}

/// Set-or-remove for optional scalar keys.
fn sync_opt(t: &mut Table, key: &str, new: Option<Value>) {
    match new {
        Some(v) => set_scalar(t, key, v),
        None => {
            t.remove(key);
        }
    }
}

/// Element-wise sync of a `key = [ {..}, {..} ]` array.
/// Same length → only changed items are replaced (their line decor kept);
/// different length → the whole array is rebuilt in canonical style.
fn sync_items<T: PartialEq>(
    parent: &mut Table,
    key: &str,
    old: &[T],
    new: &[T],
    build: &dyn Fn(&T) -> InlineTable,
) {
    if old == new {
        return;
    }
    if new.is_empty() {
        parent.remove(key);
        return;
    }
    if old.len() == new.len()
        && let Some(arr) = parent
            .get_mut(key)
            .and_then(Item::as_value_mut)
            .and_then(Value::as_array_mut)
        && arr.len() == new.len()
    {
        for (i, (o, n)) in old.iter().zip(new.iter()).enumerate() {
            if o != n
                && let Some(slot) = arr.get_mut(i)
            {
                let decor = slot.decor().clone();
                let mut v = Value::InlineTable(build(n));
                *v.decor_mut() = decor;
                *slot = v;
            }
        }
        return;
    }
    parent.insert(key, Item::Value(items_array(new, build)));
}

fn sync_auth(root: &mut Table, old: &Option<Auth>, new: &Option<Auth>) {
    if old == new {
        return;
    }
    match new {
        Some(a) => {
            root.insert("auth", Item::Table(reprefix(auth_table(a), root, "auth")));
        }
        None => {
            root.remove("auth");
        }
    }
}

fn sync_scripts(root: &mut Table, old: &Scripts, new: &Scripts) {
    if old == new {
        return;
    }
    if new.is_empty() {
        root.remove("scripts");
        return;
    }
    let t = ensure_table(root, "scripts");
    if old.pre_request != new.pre_request {
        sync_opt(t, "pre_request", new.pre_request.as_deref().map(multiline));
    }
    if old.post_response != new.post_response {
        sync_opt(
            t,
            "post_response",
            new.post_response.as_deref().map(multiline),
        );
    }
}

fn sync_defaults(root: &mut Table, old: &[crate::model::Pair], new: &[crate::model::Pair]) {
    if old == new {
        return;
    }
    if new.is_empty() {
        root.remove("defaults");
        return;
    }
    sync_items(
        ensure_table(root, "defaults"),
        "headers",
        old,
        new,
        &pair_inline,
    );
}

fn sync_vars_table(
    root: &mut Table,
    key: &str,
    old: &IndexMap<String, VarValue>,
    new: &IndexMap<String, VarValue>,
) -> Result<(), CoreError> {
    if old == new {
        return Ok(());
    }
    if new.is_empty() {
        root.remove(key);
        return Ok(());
    }
    let t = ensure_table(root, key);
    let stale: Vec<String> = t
        .iter()
        .map(|(k, _)| k.to_string())
        .filter(|k| !new.contains_key(k.as_str()))
        .collect();
    for k in stale {
        t.remove(&k);
    }
    for (k, v) in new {
        if old.get(k) != Some(v) || !t.contains_key(k) {
            set_scalar(t, k, scalar(v)?);
        }
    }
    Ok(())
}

/// Keep the decor (comments above the section header) of a table being replaced;
/// new sections get a separating blank line.
fn reprefix(mut fresh: Table, root: &Table, key: &str) -> Table {
    match root.get(key).and_then(Item::as_table) {
        Some(existing) => {
            *fresh.decor_mut() = existing.decor().clone();
        }
        None => {
            fresh.decor_mut().set_prefix("\n");
        }
    }
    fresh
}

/// Multipart parts use a different inline builder; expose for reuse by tests.
#[allow(dead_code)]
fn _multipart_builder() -> &'static dyn Fn(&crate::model::MultipartPart) -> InlineTable {
    &part_inline
}
