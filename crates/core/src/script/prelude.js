// Tomo script prelude — defines the scripting API over plain data globals
// seeded by the Rust side: req, res, __vars (read view), __env_name.
// Everything here is pure JS; the only native binding is __console(level, ...args).

"use strict";

globalThis.__var_sets = {};
globalThis.__results = [];

// ---- console ---------------------------------------------------------------
globalThis.console = {
  log: (...a) => __console("log", ...a),
  info: (...a) => __console("info", ...a),
  warn: (...a) => __console("warn", ...a),
  error: (...a) => __console("error", ...a),
  debug: (...a) => __console("debug", ...a),
};

// ---- vars ------------------------------------------------------------------
globalThis.vars = {
  get(name) {
    if (Object.prototype.hasOwnProperty.call(__var_sets, name)) return __var_sets[name];
    return __vars[name];
  },
  set(name, value) {
    __var_sets[String(name)] = value;
  },
  has(name) {
    return Object.prototype.hasOwnProperty.call(__var_sets, name) ||
      Object.prototype.hasOwnProperty.call(__vars, name);
  },
  delete(name) {
    delete __var_sets[name];
    delete __vars[name];
  },
};

globalThis.env = {
  name: () => (typeof __env_name === "string" ? __env_name : null),
  get: (name) => vars.get(name),
};

// ---- request helpers ---------------------------------------------------------
if (typeof globalThis.req === "object" && globalThis.req !== null) {
  const findHeader = (name) => req.headers.findIndex(
    (h) => h.name.toLowerCase() === String(name).toLowerCase(),
  );
  req.getHeader = (name) => {
    const i = findHeader(name);
    return i === -1 ? undefined : req.headers[i].value;
  };
  req.setHeader = (name, value) => {
    const i = findHeader(name);
    if (i === -1) req.headers.push({ name: String(name), value: String(value) });
    else req.headers[i].value = String(value);
  };
  req.removeHeader = (name) => {
    const i = findHeader(name);
    if (i !== -1) req.headers.splice(i, 1);
  };
}

// ---- res helpers -------------------------------------------------------------
if (typeof globalThis.res === "object" && globalThis.res !== null) {
  res.getHeader = (name) => res.headers[String(name).toLowerCase()];
}

// ---- test / expect -------------------------------------------------------------
globalThis.test = function test(name, fn) {
  try {
    fn();
    __results.push({ name: String(name), ok: true });
  } catch (e) {
    __results.push({
      name: String(name),
      ok: false,
      message: String((e && e.message) || e),
    });
  }
};

globalThis.expect = function expect(actual) {
  const fail = (msg) => { throw new Error(msg); };
  const check = (pass, msg) => { if (!pass) fail(msg); };
  const repr = (v) => {
    try { const s = JSON.stringify(v); return s === undefined ? String(v) : s; }
    catch { return String(v); }
  };
  const matchers = (invert) => ({
    toBe: (exp) => check(Object.is(actual, exp) !== invert,
      `expected ${repr(actual)}${invert ? " not" : ""} to be ${repr(exp)}`),
    toEqual: (exp) => check((repr(actual) === repr(exp)) !== invert,
      `expected ${repr(actual)}${invert ? " not" : ""} to equal ${repr(exp)}`),
    toBeDefined: () => check((actual !== undefined) !== invert, "expected value to be defined"),
    toBeUndefined: () => check((actual === undefined) !== invert, "expected value to be undefined"),
    toBeNull: () => check((actual === null) !== invert, `expected ${repr(actual)} to be null`),
    toBeTruthy: () => check(Boolean(actual) !== invert, `expected ${repr(actual)} to be truthy`),
    toBeFalsy: () => check(!actual !== invert, `expected ${repr(actual)} to be falsy`),
    toContain: (item) => check(
      Boolean(actual && typeof actual.includes === "function" && actual.includes(item)) !== invert,
      `expected ${repr(actual)}${invert ? " not" : ""} to contain ${repr(item)}`),
    toMatch: (re) => check(new RegExp(re).test(String(actual)) !== invert,
      `expected ${repr(actual)}${invert ? " not" : ""} to match ${String(re)}`),
    toBeGreaterThan: (n) => check((actual > n) !== invert, `expected ${repr(actual)} > ${repr(n)}`),
    toBeGreaterThanOrEqual: (n) => check((actual >= n) !== invert, `expected ${repr(actual)} >= ${repr(n)}`),
    toBeLessThan: (n) => check((actual < n) !== invert, `expected ${repr(actual)} < ${repr(n)}`),
    toBeLessThanOrEqual: (n) => check((actual <= n) !== invert, `expected ${repr(actual)} <= ${repr(n)}`),
    toHaveLength: (n) => check(Boolean(actual != null && actual.length === n) !== invert,
      `expected length ${repr(actual && actual.length)} to be ${repr(n)}`),
  });
  return { ...matchers(false), not: matchers(true) };
};
