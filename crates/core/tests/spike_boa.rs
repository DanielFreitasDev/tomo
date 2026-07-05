//! Spike: prove boa_engine 0.21 covers everything the script bridge (M7) needs:
//! JSON marshaling both ways, native closures capturing Rust state (console),
//! RuntimeLimits terminating hostile loops, and a jest-style expect()/test()
//! prelude whose results we can drain back into Rust.
//!
//! This is the go/no-go gate: if any of this fails, we swap to rquickjs behind
//! a ScriptEngine trait.

use std::cell::RefCell;

use boa_engine::object::ObjectInitializer;
use boa_engine::property::Attribute;
use boa_engine::{Context, JsValue, NativeFunction, Source, js_string};

fn eval(ctx: &mut Context, src: &str) -> Result<JsValue, String> {
    ctx.eval(Source::from_bytes(src)).map_err(|e| e.to_string())
}

#[test]
fn eval_basics() {
    let mut ctx = Context::default();
    let v = eval(&mut ctx, "21 * 2").unwrap();
    assert_eq!(v.as_number(), Some(42.0));
}

#[test]
fn json_marshals_both_ways() {
    let mut ctx = Context::default();

    // Rust -> JS: expose a response-like object as global `res`
    let res = serde_json::json!({
        "status": 201,
        "body": { "id": "abc-123", "tags": ["a", "b"] },
        "responseTime": 87
    });
    let js_res = JsValue::from_json(&res, &mut ctx).expect("from_json");
    ctx.register_global_property(js_string!("res"), js_res, Attribute::all())
        .expect("register res");

    // JS reads nested data and builds a result object
    let out = eval(
        &mut ctx,
        "({ id: res.body.id, second: res.body.tags[1], fast: res.responseTime < 100 })",
    )
    .unwrap();

    // JS -> Rust
    let json = out.to_json(&mut ctx).expect("to_json");
    let json = json.expect("object serializes to Some(json)");
    assert_eq!(json["id"], "abc-123");
    assert_eq!(json["second"], "b");
    assert_eq!(json["fast"], true);
}

// boa native closures require `Trace` captures (GC integration), which plain
// Rc<RefCell<..>> does not implement. The engine runs each script on a dedicated
// thread (M7 design: spawn_blocking + wall-clock abandon), so a thread_local
// sink is the sound, safe pattern for streaming console output back to Rust.
thread_local! {
    static CONSOLE_SINK: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

#[test]
fn native_function_streams_console_into_thread_local_sink() {
    let mut ctx = Context::default();
    CONSOLE_SINK.with_borrow_mut(Vec::clear);

    let log = NativeFunction::from_fn_ptr(|_this, args, ctx| {
        let mut parts = Vec::new();
        for a in args {
            parts.push(a.to_string(ctx)?.to_std_string_escaped());
        }
        CONSOLE_SINK.with_borrow_mut(|sink| sink.push(parts.join(" ")));
        Ok(JsValue::undefined())
    });

    let console = ObjectInitializer::new(&mut ctx)
        .function(log, js_string!("log"), 1)
        .build();
    ctx.register_global_property(js_string!("console"), console, Attribute::all())
        .expect("register console");

    eval(&mut ctx, "console.log('hello', 42); console.log('second');").unwrap();

    CONSOLE_SINK.with_borrow(|got| {
        assert_eq!(got.as_slice(), ["hello 42", "second"]);
    });
}

#[test]
fn runtime_limits_stop_loop_bombs() {
    let mut ctx = Context::default();
    ctx.runtime_limits_mut().set_loop_iteration_limit(1_000_000);

    let err = eval(&mut ctx, "let i = 0; while (true) { i++; } i").unwrap_err();
    assert!(
        err.to_lowercase().contains("loop"),
        "error should mention the loop limit, got: {err}"
    );

    // the context stays usable afterwards
    let v = eval(&mut ctx, "1 + 1").unwrap();
    assert_eq!(v.as_number(), Some(2.0));
}

#[test]
fn recursion_limit_stops_stack_bombs() {
    let mut ctx = Context::default();
    ctx.runtime_limits_mut().set_recursion_limit(512);
    let err = eval(&mut ctx, "function f() { return f(); } f()").unwrap_err();
    assert!(
        err.to_lowercase().contains("recursi") || err.to_lowercase().contains("stack"),
        "error should mention recursion/stack, got: {err}"
    );
}

const PRELUDE: &str = r#"
globalThis.__results = [];

globalThis.test = function test(name, fn) {
  try {
    fn();
    __results.push({ name: String(name), ok: true });
  } catch (e) {
    __results.push({ name: String(name), ok: false, message: String((e && e.message) || e) });
  }
};

globalThis.expect = function expect(actual) {
  const fail = (msg) => { throw new Error(msg); };
  const check = (pass, msg) => { if (!pass) fail(msg); };
  const repr = (v) => { try { return JSON.stringify(v); } catch { return String(v); } };
  const matchers = (invert) => ({
    toBe: (exp) => check(Object.is(actual, exp) !== invert,
      `expected ${repr(actual)} ${invert ? "not " : ""}to be ${repr(exp)}`),
    toEqual: (exp) => check((repr(actual) === repr(exp)) !== invert,
      `expected ${repr(actual)} ${invert ? "not " : ""}to equal ${repr(exp)}`),
    toBeDefined: () => check((actual !== undefined) !== invert, `expected value to be defined`),
    toBeUndefined: () => check((actual === undefined) !== invert, `expected value to be undefined`),
    toContain: (item) => check(Boolean(actual && typeof actual.includes === "function" && actual.includes(item)) !== invert,
      `expected ${repr(actual)} ${invert ? "not " : ""}to contain ${repr(item)}`),
    toMatch: (re) => check(new RegExp(re).test(String(actual)) !== invert,
      `expected ${repr(actual)} ${invert ? "not " : ""}to match ${String(re)}`),
    toBeGreaterThan: (n) => check((actual > n) !== invert, `expected ${repr(actual)} > ${repr(n)}`),
    toBeLessThan: (n) => check((actual < n) !== invert, `expected ${repr(actual)} < ${repr(n)}`),
    toHaveLength: (n) => check(Boolean(actual != null && actual.length === n) !== invert,
      `expected length ${repr(actual && actual.length)} to be ${repr(n)}`),
  });
  return { ...matchers(false), not: matchers(true) };
};
"#;

#[test]
fn prelude_expect_and_test_record_results() {
    let mut ctx = Context::default();
    eval(&mut ctx, PRELUDE).expect("prelude loads");

    let res = serde_json::json!({ "status": 200, "body": { "items": [1, 2, 3] } });
    let js_res = JsValue::from_json(&res, &mut ctx).unwrap();
    ctx.register_global_property(js_string!("res"), js_res, Attribute::all())
        .unwrap();

    eval(
        &mut ctx,
        r#"
        test("status is 200", () => { expect(res.status).toBe(200); });
        test("has 3 items", () => { expect(res.body.items).toHaveLength(3); });
        test("this one fails", () => { expect(res.status).toBe(404); });
        test("not matcher", () => { expect(res.status).not.toBe(500); });
        "#,
    )
    .unwrap();

    let results = eval(&mut ctx, "__results").unwrap();
    let results = results.to_json(&mut ctx).unwrap().expect("results json");
    let arr = results.as_array().unwrap();
    assert_eq!(arr.len(), 4);
    assert_eq!(arr[0]["ok"], true);
    assert_eq!(arr[1]["ok"], true);
    assert_eq!(arr[2]["ok"], false);
    assert!(arr[2]["message"].as_str().unwrap().contains("404"));
    assert_eq!(arr[3]["ok"], true);
}
