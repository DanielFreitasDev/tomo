//! boa-based script execution. Each run gets a fresh `Context` with
//! `RuntimeLimits` (loop/recursion bombs terminate) and runs on a dedicated
//! blocking thread with a wall-clock timeout on the async side (boa has no
//! cross-thread interrupt; the limits guarantee eventual termination).
//!
//! Data crosses as plain JSON: globals seeded via `JsValue::from_json`, the
//! API surface lives in prelude.js, and mutations are read back via `to_json`.
//! The only native binding is `__console`.

use std::cell::RefCell;
use std::time::Duration;

use boa_engine::object::builtins::JsArray;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsValue, NativeFunction, Source, js_string};
use indexmap::IndexMap;
use serde::Serialize;

use crate::CoreError;
use crate::model::VarValue;

const PRELUDE: &str = include_str!("prelude.js");
const LOOP_LIMIT: u64 = 4_000_000;
const RECURSION_LIMIT: usize = 512;
pub const SCRIPT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    PreRequest,
    PostResponse,
}

#[derive(Debug, Clone)]
pub struct ScriptSource {
    /// Where the code came from: "collection", "folder users", "request".
    pub origin: String,
    pub code: String,
}

/// The mutable request view exposed to scripts.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ScriptHttp {
    pub url: String,
    pub method: String,
    pub headers: Vec<HeaderEntry>,
    pub body: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct HeaderEntry {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsoleLine {
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    pub name: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScriptError {
    pub origin: String,
    pub message: String,
}

pub struct ScriptRun {
    pub phase: Phase,
    pub sources: Vec<ScriptSource>,
    pub http: ScriptHttp,
    /// Prebuilt `res` object (post-response phase only).
    pub response: Option<serde_json::Value>,
    /// Flattened winning values of every visible variable.
    pub vars_snapshot: serde_json::Value,
    pub env_name: Option<String>,
}

pub struct ScriptOutcome {
    pub http: ScriptHttp,
    pub var_sets: IndexMap<String, VarValue>,
    pub console: Vec<ConsoleLine>,
    pub tests: Vec<TestResult>,
    pub error: Option<ScriptError>,
}

thread_local! {
    static CONSOLE_SINK: RefCell<Vec<ConsoleLine>> = const { RefCell::new(Vec::new()) };
}

/// Run the chain on a blocking thread with a wall-clock timeout.
pub async fn run_scripts(run: ScriptRun) -> Result<ScriptOutcome, CoreError> {
    let handle = tokio::task::spawn_blocking(move || run_sync(run));
    match tokio::time::timeout(SCRIPT_TIMEOUT, handle).await {
        Ok(joined) => {
            joined.map_err(|e| CoreError::Invalid(format!("script thread panicked: {e}")))?
        }
        Err(_) => Err(CoreError::Invalid(format!(
            "script timed out after {}s",
            SCRIPT_TIMEOUT.as_secs()
        ))),
    }
}

fn run_sync(run: ScriptRun) -> Result<ScriptOutcome, CoreError> {
    CONSOLE_SINK.with_borrow_mut(Vec::clear);

    let mut ctx = Context::default();
    ctx.runtime_limits_mut()
        .set_loop_iteration_limit(LOOP_LIMIT);
    ctx.runtime_limits_mut()
        .set_recursion_limit(RECURSION_LIMIT);

    register_console(&mut ctx)?;
    seed(&mut ctx, &run)?;

    ctx.eval(Source::from_bytes(PRELUDE))
        .map_err(|e| CoreError::Invalid(format!("prelude failed: {e}")))?;

    let mut error = None;
    for source in &run.sources {
        if source.code.trim().is_empty() {
            continue;
        }
        if let Err(e) = ctx.eval(Source::from_bytes(&source.code)) {
            error = Some(ScriptError {
                origin: source.origin.clone(),
                message: first_line(&e.to_string()),
            });
            break;
        }
    }

    let http = read_back::<ScriptHttp>(&mut ctx, "req").unwrap_or(run.http);
    let var_sets: IndexMap<String, VarValue> =
        read_back(&mut ctx, "__var_sets").unwrap_or_default();
    let tests: Vec<TestResult> = read_back::<Vec<RawTest>>(&mut ctx, "__results")
        .unwrap_or_default()
        .into_iter()
        .map(|t| TestResult {
            name: t.name,
            ok: t.ok,
            message: t.message,
        })
        .collect();

    let console = CONSOLE_SINK.with_borrow_mut(std::mem::take);

    Ok(ScriptOutcome {
        http,
        var_sets,
        console,
        tests,
        error,
    })
}

#[derive(serde::Deserialize)]
struct RawTest {
    name: String,
    ok: bool,
    #[serde(default)]
    message: Option<String>,
}

fn register_console(ctx: &mut Context) -> Result<(), CoreError> {
    let log = NativeFunction::from_fn_ptr(|_this, args, ctx| {
        let level = args
            .first()
            .and_then(|v| v.as_string().map(|s| s.to_std_string_escaped()))
            .unwrap_or_else(|| "log".to_string());
        let mut parts = Vec::new();
        for a in args.iter().skip(1) {
            parts.push(display_value(a, ctx));
        }
        CONSOLE_SINK.with_borrow_mut(|sink| {
            sink.push(ConsoleLine {
                level,
                message: parts.join(" "),
            });
        });
        Ok(JsValue::undefined())
    });
    ctx.register_global_callable(js_string!("__console"), 1, log)
        .map_err(|e| CoreError::Invalid(format!("console setup failed: {e}")))?;
    Ok(())
}

/// Pretty console formatting: strings bare, everything else JSON.
fn display_value(value: &JsValue, ctx: &mut Context) -> String {
    if let Some(s) = value.as_string() {
        return s.to_std_string_escaped();
    }
    match value.to_json(ctx) {
        Ok(Some(json)) => json.to_string(),
        _ => value.display().to_string(),
    }
}

fn seed(ctx: &mut Context, run: &ScriptRun) -> Result<(), CoreError> {
    let req = serde_json::to_value(&run.http).expect("ScriptHttp serializes");
    seed_global(ctx, "req", &req)?;
    if let Some(res) = &run.response {
        seed_global(ctx, "res", res)?;
    }
    seed_global(ctx, "__vars", &run.vars_snapshot)?;
    match &run.env_name {
        Some(name) => seed_global(ctx, "__env_name", &serde_json::Value::String(name.clone()))?,
        None => seed_global(ctx, "__env_name", &serde_json::Value::Null)?,
    }
    Ok(())
}

fn seed_global(ctx: &mut Context, name: &str, value: &serde_json::Value) -> Result<(), CoreError> {
    let js = JsValue::from_json(value, ctx)
        .map_err(|e| CoreError::Invalid(format!("failed to seed `{name}`: {e}")))?;
    ctx.register_global_property(js_string!(name.to_owned()), js, Attribute::all())
        .map_err(|e| CoreError::Invalid(format!("failed to register `{name}`: {e}")))?;
    Ok(())
}

fn read_back<T: serde::de::DeserializeOwned>(ctx: &mut Context, name: &str) -> Option<T> {
    let value = ctx
        .global_object()
        .get(js_string!(name.to_owned()), ctx)
        .ok()?;
    // functions attached by the prelude (getHeader etc.) are dropped by to_json
    let json = to_json_lossy(&value, ctx)?;
    serde_json::from_value(json).ok()
}

fn to_json_lossy(value: &JsValue, ctx: &mut Context) -> Option<serde_json::Value> {
    match value.to_json(ctx) {
        Ok(v) => v,
        Err(_) => {
            // objects with function properties: retry via JSON.stringify semantics
            let global = ctx.global_object();
            let json_obj = global.get(js_string!("JSON"), ctx).ok()?;
            let stringify = json_obj
                .as_object()?
                .get(js_string!("stringify"), ctx)
                .ok()?;
            let s = stringify
                .as_callable()?
                .call(&JsValue::undefined(), std::slice::from_ref(value), ctx)
                .ok()?;
            let text = s.as_string()?.to_std_string_escaped();
            serde_json::from_str(&text).ok()
        }
    }
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).to_string()
}

// silence unused import when JsArray isn't needed on some boa versions
#[allow(unused)]
fn _keep(_: Option<JsArray>) {}
