"use strict";

// Pure helpers of web/mill.js. Run: node --test web/tests/mill.test.js
const test = require("node:test");
const assert = require("node:assert/strict");
const mill = require("../mill.js");

test("esc escapes the characters that matter in HTML text and attributes", () => {
  assert.equal(
    mill.esc(`<a href="x" title='y'>Tom & Jerry</a>`),
    "&lt;a href=&quot;x&quot; title=&#39;y&#39;&gt;Tom &amp; Jerry&lt;/a&gt;"
  );
  assert.equal(mill.esc("§ 4.4 é 𝔽"), "§ 4.4 é 𝔽");
  assert.equal(mill.esc(7), "7");
});

test("isRfc3339 accepts the date-times the server parses", () => {
  for (const text of [
    "2026-09-17T12:00:00Z",
    "2034-03-01T09:00:00Z",
    "2026-08-23T12:00:00-04:00",
    "2033-01-01T00:00:00+00:00",
    "2026-09-17T12:00:00.250Z",
    "2026-09-17t12:00:00z",
    "2026-09-17 12:00:00Z",
    "2024-02-29T00:00:00Z"
  ]) {
    assert.equal(mill.isRfc3339(text), true, text);
  }
});

test("isRfc3339 rejects dates without a zone, impossible values, and non-strings", () => {
  for (const text of [
    "",
    "2026-09-17",
    "2026-09-17T12:00:00",
    "2026-09-17T12:00Z",
    "2026-13-01T00:00:00Z",
    "2026-00-10T00:00:00Z",
    "2025-02-29T00:00:00Z",
    "2026-04-31T00:00:00Z",
    "2026-09-17T24:00:00Z",
    "2026-09-17T12:60:00Z",
    "2026-09-17T12:00:00+24:00",
    " 2026-09-17T12:00:00Z",
    "tomorrow",
    null,
    20260917
  ]) {
    assert.equal(mill.isRfc3339(text), false, String(text));
  }
});

test("valueText writes runtime values the way the opinion sentences do", () => {
  const int = (n) => ({ kind: "int", data: n });
  const cases = [
    [int(7), "7"],
    [{ kind: "decimal", data: "100.00" }, "100.00"],
    [{ kind: "bool", data: false }, "false"],
    [{ kind: "instant", data: "2034-03-01T09:00:00Z" }, "2034-03-01T09:00:00Z"],
    [{ kind: "string", data: "Bob" }, "\"Bob\""],
    [{ kind: "entity", data: "Alice" }, "Alice"],
    [{ kind: "unit" }, "unit"],
    [{ kind: "ctor", data: { name: "Performed", fields: {} } }, "Performed"],
    [{ kind: "ctor", data: { name: "USD", fields: { _0: { kind: "decimal", data: "100.00" } } } }, "USD(100.00)"],
    [{ kind: "ctor", data: { name: "Pair", fields: { _1: int(2), _0: int(1) } } }, "Pair(1, 2)"],
    [{ kind: "ctor", data: { name: "Due", fields: { on: { kind: "instant", data: "2026-01-01T00:00:00Z" }, amount: int(5) } } },
      "Due(amount: 5, on: 2026-01-01T00:00:00Z)"],
    [{ kind: "set", data: [{ kind: "entity", data: "Alice" }, { kind: "entity", data: "Bob" }] }, "{Alice, Bob}"],
    [{ kind: "map", data: { b: int(2), a: int(1) } }, "{a: 1, b: 2}"],
    [{ kind: "option", data: null }, "none"],
    [{ kind: "option", data: int(3) }, "3"],
    [{ kind: "prop", data: { name: "Alive" } }, "{\"kind\":\"prop\",\"data\":{\"name\":\"Alive\"}}"],
    ["loose", "\"loose\""],
    [null, "null"]
  ];
  for (const [value, text] of cases) {
    assert.equal(mill.valueText(value), text, JSON.stringify(value));
  }
});

test("requestText names what a suspended evaluation waits for", () => {
  const cases = [
    [{ kind: "needCustom", effect: "require", payload: "requirement failed" }, "requirement failed (require)"],
    [{ kind: "needCustom", effect: "notify", payload: { to: "Alice" } }, "notify"],
    [{ kind: "needEvidence", issue: { kind: "ground", predicate: "PaymentRecord" }, schema: "PaymentRecord" },
      "evidence matching PaymentRecord"],
    [{ kind: "needInterpretation", family: "SuccessorEligibility" }, "an interpretation of SuccessorEligibility"],
    [{ kind: "needInterpretation", family: "SuccessorEligibility", source: "Instrument" },
      "an interpretation of SuccessorEligibility under Instrument"],
    [{ kind: "needJudgment", issue: "x", protocol: "CourtCapacityDetermination" },
      "a determination under CourtCapacityDetermination"],
    [{ kind: "needChoice", protocol: "TrusteeChoice", options: ["Alice", "Bob"] }, "a decision under TrusteeChoice among Alice, Bob"],
    [{ kind: "needChoice", protocol: "TrusteeChoice", options: [{ kind: "entity", data: "Alice" }] }, "a decision under TrusteeChoice"],
    [{ kind: "needApplicableLaw", issue: "x", candidates: ["MA", "NY"] }, "applicable law among MA, NY"],
    [{ kind: "needApplicableLaw", issue: "x" }, "applicable law"],
    [{ kind: "needConflict", graph: {}, doctrines: ["LexPosterior", "LexSpecialis"] },
      "one applicable conflict doctrine among LexPosterior, LexSpecialis"],
    [{ kind: "needConflict", graph: {} }, "one applicable conflict doctrine"],
    [{ kind: "needSomethingNew", detail: 1 }, "needSomethingNew"],
    [{ issue: "x" }, "{\"issue\":\"x\"}"]
  ];
  for (const [request, text] of cases) {
    assert.equal(mill.requestText(request), text, JSON.stringify(request));
  }
});

const FALLBACK = Object.freeze({
  v: 1,
  module: "module Programs.RequireGate version \"0.1.0\" {}\n",
  case: "{\n  \"schema\": \"fidryn.case-record/v0.1\",\n  \"admissibleCompletions\": {}\n}\n",
  template: "{{module}}@{{version}}\n",
  query: "q",
  validAt: "2026-09-17T12:00:00Z",
  knownAt: "2026-09-17T12:00:00Z",
  buffer: "module",
  sample: "require-gate",
  view: "opinion"
});

const SAVED = Object.freeze({
  v: 1,
  module: "module Mine version \"0.2.0\" {}\n",
  case: "{}",
  template: "{{module}}",
  query: "acting_trustee",
  validAt: "2034-03-01T09:00:00Z",
  knownAt: "2034-03-01T09:00:00Z",
  buffer: "case",
  sample: null,
  view: "table"
});

/** A localStorage stand-in holding `raw` under fidryn-mill. */
function storageWith(raw) {
  return {
    getItem(key) {
      return key === "fidryn-mill" ? raw : null;
    },
    setItem() {}
  };
}

test("loadState restores a saved state", () => {
  assert.deepEqual(mill.loadState(storageWith(JSON.stringify(SAVED)), FALLBACK), SAVED);
});

test("loadState falls back to the first sample when nothing is saved", () => {
  assert.deepEqual(mill.loadState(storageWith(null), FALLBACK), FALLBACK);
});

test("loadState falls back on corrupt JSON", () => {
  for (const raw of ["{", "not json", "", "{\"v\":1,", "null", "[]", "42", "\"text\"", "true"]) {
    assert.deepEqual(mill.loadState(storageWith(raw), FALLBACK), FALLBACK, raw);
  }
});

test("loadState falls back on a state from another version", () => {
  for (const v of [0, 2, "1", undefined, null]) {
    const raw = JSON.stringify(Object.assign({}, SAVED, { v }));
    assert.deepEqual(mill.loadState(storageWith(raw), FALLBACK), FALLBACK, String(v));
  }
});

test("loadState falls back when a text field is missing or not a string", () => {
  for (const key of ["module", "case", "template", "query", "validAt", "knownAt"]) {
    const missing = Object.assign({}, SAVED);
    delete missing[key];
    assert.deepEqual(mill.loadState(storageWith(JSON.stringify(missing)), FALLBACK), FALLBACK, `without ${key}`);
    const wrong = Object.assign({}, SAVED, { [key]: 5 });
    assert.deepEqual(mill.loadState(storageWith(JSON.stringify(wrong)), FALLBACK), FALLBACK, `${key}: 5`);
  }
});

test("loadState falls back when storage throws or is missing", () => {
  const throwing = {
    getItem() {
      throw new Error("SecurityError: storage is disabled");
    },
    setItem() {}
  };
  assert.deepEqual(mill.loadState(throwing, FALLBACK), FALLBACK);
  assert.deepEqual(mill.loadState(null, FALLBACK), FALLBACK);
  assert.deepEqual(mill.loadState(undefined, FALLBACK), FALLBACK);
});

test("loadState resets an unknown buffer, view, or sample on its own", () => {
  const odd = Object.assign({}, SAVED, { buffer: "notes", view: "cards", sample: 3 });
  assert.deepEqual(
    mill.loadState(storageWith(JSON.stringify(odd)), FALLBACK),
    Object.assign({}, SAVED, { buffer: "module", view: "opinion", sample: null })
  );
});

test("loadState returns a fresh object, never the fallback itself", () => {
  const state = mill.loadState(storageWith(null), FALLBACK);
  assert.notEqual(state, FALLBACK);
  state.module = "changed";
  assert.equal(FALLBACK.module.startsWith("module Programs.RequireGate"), true);
});

/** A fetch stand-in answering every request with `status` and body `text`. */
function answering(status, text, seen) {
  return (url, init) => {
    if (seen) seen.push({ url, init });
    return Promise.resolve({ status, ok: status >= 200 && status < 300, text: () => Promise.resolve(text) });
  };
}

test("postJson sends the body as a JSON POST", async () => {
  const seen = [];
  await mill.postJson("/api/check", { source: "module A version \"1\" {}" }, answering(200, "{\"ok\":true}", seen));
  assert.equal(seen.length, 1);
  assert.equal(seen[0].url, "/api/check");
  assert.equal(seen[0].init.method, "POST");
  assert.equal(seen[0].init.headers["content-type"], "application/json");
  assert.deepEqual(JSON.parse(seen[0].init.body), { source: "module A version \"1\" {}" });
});

test("postJson turns a rejected fetch into a network result instead of throwing", async () => {
  const result = await mill.postJson("/api/run", {}, () => Promise.reject(new TypeError("Failed to fetch")));
  assert.deepEqual(result, { network: true, status: 0, data: null, text: "" });
});

test("postJson turns a fetch that throws, or a body that fails mid-read, into a network result", async () => {
  const throwsNow = () => {
    throw new TypeError("fetch is not a function");
  };
  assert.equal((await mill.postJson("/api/run", {}, throwsNow)).network, true);
  const dropsBody = () => Promise.resolve({ status: 200, text: () => Promise.reject(new TypeError("network error")) });
  assert.equal((await mill.postJson("/api/run", {}, dropsBody)).network, true);
});

test("postJson returns an HTTP 400 JSON body as data", async () => {
  const body = {
    ok: false,
    error: "check failed",
    diagnostics: [{ code: "E100", message: "unknown declaration `colour`", primary_span: { start: 251, end: 257 } }]
  };
  const result = await mill.postJson("/api/run", {}, answering(400, JSON.stringify(body)));
  assert.equal(result.network, false);
  assert.equal(result.status, 400);
  assert.deepEqual(result.data, body);
  const engine = { kind: "engineError", error: "UnknownQuery", message: "unknown query nope", ok: false };
  assert.deepEqual((await mill.postJson("/api/run", {}, answering(400, JSON.stringify(engine)))).data, engine);
});

test("postJson keeps a body that is not JSON as text", async () => {
  const result = await mill.postJson("/api/run", {}, answering(500, "Internal Server Error"));
  assert.deepEqual(result, { network: false, status: 500, data: null, text: "Internal Server Error" });
  assert.equal((await mill.postJson("/api/run", {}, answering(200, "7"))).data, null);
});
