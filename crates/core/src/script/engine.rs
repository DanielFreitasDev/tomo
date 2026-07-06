//! QuickJS (rquickjs) script execution behind the [`ScriptEngine`] seam.
//!
//! Each run gets a fresh `Runtime` with a real **memory limit** (an allocation
//! bomb becomes a catchable JS "out of memory" exception instead of aborting the
//! whole process), a **stack limit** (runaway recursion throws instead of
//! crashing), and an **interrupt handler** armed with a wall-clock deadline that
//! actually stops a hot/nested loop mid-execution — the thing boa could not do
//! from another thread. The run still executes on a blocking thread so a
//! CPU-bound script never stalls the async runtime; the outer timeout is only a
//! backstop, because the in-VM deadline makes the thread return on its own.
//!
//! Data crosses as plain JSON: globals are seeded via `JSON.parse`, the API
//! surface lives in prelude.js, and mutations are read back via `JSON.stringify`
//! (which conveniently drops the helper functions the prelude attaches). The
//! only native binding is `__console`.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use rquickjs::function::Rest;
use rquickjs::{Context, Ctx, Function, Runtime, Value};
use serde::Serialize;

use crate::CoreError;
use crate::model::VarValue;

const PRELUDE: &str = include_str!("prelude.js");
/// Per-run heap ceiling. Enough for real response bodies; small enough that a
/// runaway allocator trips an OOM exception long before it hurts the host.
const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
/// Well under the blocking thread's OS stack so deep recursion throws a JS
/// "stack overflow" instead of segfaulting the process.
const MAX_STACK_SIZE: usize = 512 * 1024;
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

/// The JS runtime seam. Swapping engines (a future WASM sandbox, say) means
/// providing another impl; the HTTP pipeline only ever calls [`run_scripts`].
pub trait ScriptEngine {
    fn run(&self, run: ScriptRun) -> Result<ScriptOutcome, CoreError>;
}

/// The QuickJS-backed engine. `timeout` is the in-VM wall-clock deadline.
pub struct QuickJsEngine {
    timeout: Duration,
}

impl QuickJsEngine {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

/// Run the chain on a blocking thread. The in-VM deadline guarantees the thread
/// returns; the outer timeout is a belt-and-suspenders that should never fire.
pub async fn run_scripts(run: ScriptRun) -> Result<ScriptOutcome, CoreError> {
    let handle = tokio::task::spawn_blocking(move || QuickJsEngine::new(SCRIPT_TIMEOUT).run(run));
    let backstop = SCRIPT_TIMEOUT + Duration::from_secs(5);
    match tokio::time::timeout(backstop, handle).await {
        Ok(joined) => {
            joined.map_err(|e| CoreError::Invalid(format!("script thread panicked: {e}")))?
        }
        Err(_) => Err(CoreError::Invalid(format!(
            "script timed out after {}s",
            backstop.as_secs()
        ))),
    }
}

impl ScriptEngine for QuickJsEngine {
    fn run(&self, run: ScriptRun) -> Result<ScriptOutcome, CoreError> {
        let rt = Runtime::new()
            .map_err(|e| CoreError::Invalid(format!("script runtime init failed: {e}")))?;
        rt.set_memory_limit(MEMORY_LIMIT);
        rt.set_max_stack_size(MAX_STACK_SIZE);

        let timeout = self.timeout;
        // `Instant` is Copy, so the handler captures its own copy and `deadline`
        // stays usable below to tell a timeout apart from an ordinary throw.
        let deadline = Instant::now() + timeout;
        rt.set_interrupt_handler(Some(Box::new(move || Instant::now() >= deadline)));

        let ctx = Context::full(&rt)
            .map_err(|e| CoreError::Invalid(format!("script context init failed: {e}")))?;

        let console: Arc<Mutex<Vec<ConsoleLine>>> = Arc::new(Mutex::new(Vec::new()));

        ctx.with(|ctx| -> Result<ScriptOutcome, CoreError> {
            register_console(&ctx, console.clone())?;
            seed(&ctx, &run)?;

            if let Err(e) = ctx.eval::<Value, _>(PRELUDE) {
                return Err(CoreError::Invalid(format!(
                    "prelude failed: {}",
                    describe_error(&ctx, e)
                )));
            }

            let mut error = None;
            for source in &run.sources {
                if source.code.trim().is_empty() {
                    continue;
                }
                if let Err(e) = ctx.eval::<Value, _>(source.code.as_str()) {
                    let timed_out = Instant::now() >= deadline;
                    // Always drain the pending exception so read-back stays sound.
                    let described = describe_error(&ctx, e);
                    let message = if timed_out {
                        format!("script timed out after {}s", timeout.as_secs())
                    } else {
                        described
                    };
                    error = Some(ScriptError {
                        origin: source.origin.clone(),
                        message,
                    });
                    break;
                }
            }

            let http = read_back::<ScriptHttp>(&ctx, "req").unwrap_or_else(|| run.http.clone());
            let var_sets: IndexMap<String, VarValue> =
                read_back(&ctx, "__var_sets").unwrap_or_default();
            let tests: Vec<TestResult> = read_back::<Vec<RawTest>>(&ctx, "__results")
                .unwrap_or_default()
                .into_iter()
                .map(|t| TestResult {
                    name: t.name,
                    ok: t.ok,
                    message: t.message,
                })
                .collect();

            let console = std::mem::take(&mut *console.lock().unwrap_or_else(|e| e.into_inner()));

            Ok(ScriptOutcome {
                http,
                var_sets,
                console,
                tests,
                error,
            })
        })
    }
}

#[derive(serde::Deserialize)]
struct RawTest {
    name: String,
    ok: bool,
    #[serde(default)]
    message: Option<String>,
}

fn register_console(ctx: &Ctx, sink: Arc<Mutex<Vec<ConsoleLine>>>) -> Result<(), CoreError> {
    let log = Function::new(ctx.clone(), move |args: Rest<Value>| {
        let mut it = args.iter();
        let level = it
            .next()
            .and_then(|v| v.as_string())
            .and_then(|s| s.to_string().ok())
            .unwrap_or_else(|| "log".to_string());
        let message = it.map(display_value).collect::<Vec<_>>().join(" ");
        if let Ok(mut sink) = sink.lock() {
            sink.push(ConsoleLine { level, message });
        }
    })
    .map_err(|e| CoreError::Invalid(format!("console setup failed: {e}")))?;

    ctx.globals()
        .set("__console", log)
        .map_err(|e| CoreError::Invalid(format!("console register failed: {e}")))?;
    Ok(())
}

/// Pretty console formatting: strings bare, everything else JSON.
fn display_value(value: &Value) -> String {
    if let Some(s) = value.as_string() {
        return s.to_string().unwrap_or_default();
    }
    if value.is_undefined() {
        return "undefined".to_string();
    }
    if value.is_null() {
        return "null".to_string();
    }
    // reach the context through the value so the `'js` lifetimes unify
    match value.ctx().json_stringify(value.clone()) {
        Ok(Some(s)) => s.to_string().unwrap_or_default(),
        _ => "undefined".to_string(),
    }
}

fn seed(ctx: &Ctx, run: &ScriptRun) -> Result<(), CoreError> {
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

fn seed_global(ctx: &Ctx, name: &str, value: &serde_json::Value) -> Result<(), CoreError> {
    let text = serde_json::to_string(value)
        .map_err(|e| CoreError::Invalid(format!("failed to serialize `{name}`: {e}")))?;
    let js = ctx
        .json_parse(text)
        .map_err(|e| CoreError::Invalid(format!("failed to seed `{name}`: {e}")))?;
    ctx.globals()
        .set(name, js)
        .map_err(|e| CoreError::Invalid(format!("failed to register `{name}`: {e}")))?;
    Ok(())
}

fn read_back<T: serde::de::DeserializeOwned>(ctx: &Ctx, name: &str) -> Option<T> {
    let value: Value = ctx.globals().get(name).ok()?;
    // JSON.stringify drops function-valued props (getHeader etc.) the prelude adds.
    let json = ctx.json_stringify(value).ok().flatten()?;
    let text = json.to_string().ok()?;
    serde_json::from_str(&text).ok()
}

/// Turn a failed eval into a one-line message, draining the pending exception so
/// the context stays usable for read-back afterwards.
fn describe_error(ctx: &Ctx, err: rquickjs::Error) -> String {
    if !err.is_exception() {
        return first_line(&err.to_string());
    }
    let caught = ctx.catch();
    let text = if let Some(obj) = caught.as_object() {
        obj.get::<_, String>("message")
            .ok()
            .filter(|m| !m.is_empty())
            .or_else(|| obj.get::<_, String>("name").ok())
            .unwrap_or_else(|| "script error".to_string())
    } else if let Some(s) = caught.as_string() {
        s.to_string().unwrap_or_else(|_| "script error".to_string())
    } else {
        ctx.json_stringify(caught)
            .ok()
            .flatten()
            .and_then(|s| s.to_string().ok())
            .unwrap_or_else(|| "script error".to_string())
    };
    first_line(&text)
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or(s).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http() -> ScriptHttp {
        ScriptHttp {
            url: "http://example.test/".into(),
            method: "GET".into(),
            headers: vec![],
            body: serde_json::Value::Null,
        }
    }

    fn run_with(
        timeout: Duration,
        code: &str,
        response: Option<serde_json::Value>,
    ) -> ScriptOutcome {
        QuickJsEngine::new(timeout)
            .run(ScriptRun {
                phase: if response.is_some() {
                    Phase::PostResponse
                } else {
                    Phase::PreRequest
                },
                sources: vec![ScriptSource {
                    origin: "request".into(),
                    code: code.into(),
                }],
                http: http(),
                response,
                vars_snapshot: serde_json::json!({}),
                env_name: None,
            })
            .expect("engine run should not fail at infra level")
    }

    #[test]
    fn console_and_vars_and_header_helpers_round_trip() {
        let out = run_with(
            SCRIPT_TIMEOUT,
            r#"
            console.log('hi', 42, { a: 1 });
            vars.set('token', 'abc');
            req.setHeader('X-Test', 'yes');
            req.method = 'POST';
            "#,
            None,
        );
        assert!(out.error.is_none(), "{:?}", out.error);
        assert_eq!(out.console.len(), 1);
        assert_eq!(out.console[0].message, r#"hi 42 {"a":1}"#);
        assert_eq!(
            out.var_sets
                .get("token")
                .and_then(|v| serde_json::to_value(v).ok()),
            Some(serde_json::json!("abc"))
        );
        assert_eq!(out.http.method, "POST");
        assert!(
            out.http
                .headers
                .iter()
                .any(|h| h.name == "X-Test" && h.value == "yes")
        );
    }

    #[test]
    fn expect_and_test_record_pass_and_fail() {
        let out = run_with(
            SCRIPT_TIMEOUT,
            r#"
            test('passes', () => { expect(res.status).toBe(200); });
            test('fails', () => { expect(res.status).toBe(404); });
            "#,
            Some(serde_json::json!({ "status": 200 })),
        );
        assert_eq!(out.tests.len(), 2);
        assert!(out.tests[0].ok);
        assert!(!out.tests[1].ok);
        assert!(out.tests[1].message.as_deref().unwrap().contains("404"));
    }

    #[test]
    fn user_throw_is_captured_with_message_not_a_panic() {
        let out = run_with(SCRIPT_TIMEOUT, "throw new Error('boom in pre');", None);
        let err = out.error.expect("throw should surface as a script error");
        assert_eq!(err.origin, "request");
        assert!(err.message.contains("boom in pre"), "{}", err.message);
    }

    #[test]
    fn nested_loop_bomb_terminates_at_the_deadline() {
        let started = Instant::now();
        let out = run_with(Duration::from_millis(400), "for (;;) { for (;;) {} }", None);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "must stop near the 400ms deadline, took {:?}",
            started.elapsed()
        );
        let err = out.error.expect("timed-out loop must report an error");
        assert!(err.message.contains("timed out"), "{}", err.message);
    }

    #[test]
    fn memory_bomb_is_caught_without_aborting_the_process() {
        let out = run_with(
            SCRIPT_TIMEOUT,
            "const a = []; for (;;) { a.push(new Array(4000).fill(1)); }",
            None,
        );
        // reaching this assertion at all proves the OOM did not abort the process
        let err = out.error.expect("allocation bomb must report an error");
        assert!(
            !err.message.contains("timed out"),
            "should be OOM, not timeout: {}",
            err.message
        );
    }

    #[test]
    fn timeout_still_returns_the_console_logged_before_the_hang() {
        let out = run_with(
            Duration::from_millis(400),
            "console.log('before the hang'); for (;;) {}",
            None,
        );
        assert!(out.error.as_ref().unwrap().message.contains("timed out"));
        assert_eq!(
            out.console.len(),
            1,
            "partial console must survive a timeout"
        );
        assert_eq!(out.console[0].message, "before the hang");
    }
}
