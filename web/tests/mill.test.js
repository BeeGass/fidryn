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

// ---------------------------------------------------------------- editor

const fs = require("node:fs");
const path = require("node:path");

const ROOT = path.join(__dirname, "..", "..");
const read = (rel) => fs.readFileSync(path.join(ROOT, rel), "utf8");

// Node 18's test runner cannot report non-ASCII names or messages, so escape them.
const show = (text) => JSON.stringify(text).replace(/[^\x20-\x7e]/g, (c) => "\\u" + c.charCodeAt(0).toString(16).padStart(4, "0"));

/** The class of the segment covering string index `index`. */
function classAt(segments, index) {
  let pos = 0;
  for (const seg of segments) {
    if (index < pos + seg.text.length) return seg.cls;
    pos += seg.text.length;
  }
  throw new Error(`index ${index} is past the end`);
}

/** The class at the `nth` occurrence (from 0) of `needle` in `source`. */
function classOf(segments, source, needle, nth = 0) {
  let at = -1;
  for (let k = 0; k <= nth; k++) {
    at = source.indexOf(needle, at + 1);
    assert.notEqual(at, -1, `${show(needle)} #${nth} not in source`);
  }
  return classAt(segments, at);
}

const joined = (segments) => segments.map((s) => s.text).join("");

test("byteToIndex puts a span after section signs, accents, and astral characters on the right text", () => {
  const source = [
    "module Programs.RequireGate version \"0.1.0\" {",
    "    // § 4.4 é 𝔽 notes",
    "    jurisdiction Test",
    "    colour blue",
    "    query q() -> Int { require \"§é𝔽\" == \"x\"; return 7 }",
    "    colour red",
    "}",
    ""
  ].join("\n");
  const first = source.indexOf("colour");
  const second = source.indexOf("colour", first + 1);
  for (const index of [first, second]) {
    const byte = Buffer.byteLength(source.slice(0, index), "utf8");
    assert.notEqual(byte, index);
    assert.equal(mill.byteToIndex(source, byte), index);
    assert.equal(source.slice(mill.byteToIndex(source, byte), mill.byteToIndex(source, byte + 6)), "colour");
  }
});

test("byteToIndex counts 2-, 3-, and 4-byte characters exactly", () => {
  assert.equal(mill.byteToIndex("§x", 2), 1);
  assert.equal(mill.byteToIndex("éx", 2), 1);
  assert.equal(mill.byteToIndex("—x", 3), 1);
  assert.equal(mill.byteToIndex("中x", 3), 1);
  assert.equal(mill.byteToIndex("“quoted”", 3), 1);
  assert.equal(mill.byteToIndex("𝔽x", 4), 2);
  assert.equal(mill.byteToIndex("a𝔽b", 5), 3);
  assert.equal(mill.byteToIndex("\"§é𝔽\"", 1 + 2 + 2 + 4), 5);
  let bytes = 0;
  const text = "a§é𝔽—“中”\n// 𝔽§ … →\nz";
  for (let i = 0; i < text.length; ) {
    assert.equal(mill.byteToIndex(text, bytes), i, `byte ${bytes}`);
    const cp = text.codePointAt(i);
    bytes += Buffer.byteLength(String.fromCodePoint(cp), "utf8");
    i += cp > 0xffff ? 2 : 1;
  }
  assert.equal(mill.byteToIndex(text, bytes), text.length);
});

test("byteToIndex clamps offsets inside a character, past the end, and below zero", () => {
  assert.equal(mill.byteToIndex("𝔽", 2), 0);
  assert.equal(mill.byteToIndex("é", 1), 0);
  assert.equal(mill.byteToIndex("—", 1), 0);
  assert.equal(mill.byteToIndex("—", 2), 0);
  assert.equal(mill.byteToIndex("abc", 99), 3);
  assert.equal(mill.byteToIndex("abc", -1), 0);
  assert.equal(mill.byteToIndex("", 0), 0);
});

test("lineCol counts lines and characters from 1", () => {
  const text = "a§\n𝔽é colour\n";
  assert.deepEqual(mill.lineCol(text, 0), { line: 1, col: 1 });
  assert.deepEqual(mill.lineCol(text, 2), { line: 1, col: 3 });
  assert.deepEqual(mill.lineCol(text, text.indexOf("colour")), { line: 2, col: 4 });
  assert.deepEqual(mill.lineCol(text, text.length), { line: 3, col: 1 });
  assert.deepEqual(mill.lineCol(text, 999), { line: 3, col: 1 });
});

test("tokenizeFr gives back every sample module exactly", () => {
  for (const rel of [
    "tests/programs/require-gate.fr",
    "tests/programs/late-payment.fr",
    "examples/trust/bryan-revocable-trust.fr"
  ]) {
    const source = read(rel);
    assert.equal(joined(mill.tokenizeFr(source)), source, rel);
  }
  for (const odd of ["", "\"unterminated", "// only a comment", "x § é 𝔽 @ # $", "\"esc \\\" still\" 𝔽", "12026-01-01 2026-9 0..1"]) {
    assert.equal(joined(mill.tokenizeFr(odd)), odd, show(odd));
  }
});

test("tokenizeFr classes keywords, types, strings, literals, comments, and punctuation", () => {
  const source = [
    "module Programs.RequireGate version \"0.1.0\" {",
    "    // r must not skip ahead",
    "    import MA.TrustLaw.Fixture version \"2026-08-23\"",
    "    effective_at 2026-09-17",
    "    recorded_at 2026-08-23T12:00:00-04:00",
    "    entity Payer : NaturalPerson",
    "    query q() -> Int { require true; return 7 }",
    "    query due() -> Money<USD> { goal Evaluate { USD(100.00) } }",
    "    due 30 days after invoice_date",
    "    effective [execution_time, +inf)",
    "    flag false",
    "}"
  ].join("\n");
  const segs = mill.tokenizeFr(source);
  const expect = [
    ["module", 0, "tk-kw"], ["Programs", 0, "tk-ty"], [".RequireGate", 0, "tk-pu"], ["version", 0, "tk-kw"],
    ["\"0.1.0\"", 0, "tk-st"], ["{", 0, "tk-pu"], ["// r must", 0, "tk-co"],
    ["import", 0, "tk-kw"], ["MA", 0, "tk-ty"], [".TrustLaw", 0, "tk-pu"], ["TrustLaw", 0, "tk-ty"], ["Fixture", 0, "tk-ty"],
    ["effective_at", 0, "tk-kw"], ["2026-09-17", 0, "tk-nu"], ["2026-08-23T12:00:00-04:00", 0, "tk-nu"],
    ["entity", 0, "tk-kw"], ["Payer", 0, ""], [": Natural", 0, "tk-pu"], ["NaturalPerson", 0, "tk-ty"],
    ["query", 0, "tk-kw"], ["q()", 0, ""], ["->", 0, "tk-pu"], ["Int", 0, "tk-ty"], ["require", 0, "tk-kw"],
    ["true", 0, "tk-nu"], [";", 0, "tk-pu"], ["return", 0, "tk-kw"], ["7 }", 0, "tk-nu"],
    ["Money", 0, "tk-ty"], ["<", 0, "tk-pu"], ["USD>", 0, "tk-ty"], ["goal", 0, "tk-kw"], ["Evaluate", 0, ""],
    ["100.00", 0, "tk-nu"], ["due", 1, "tk-kw"], ["30", 0, "tk-nu"], ["days", 0, "tk-nu"], ["after", 0, ""],
    ["+inf", 0, "tk-nu"], ["false", 0, "tk-nu"]
  ];
  for (const [needle, nth, cls] of expect) {
    assert.equal(classOf(segs, source, needle, nth), cls, `${show(needle)} #${nth}`);
  }
});

test("tokenizeFr leaves non-ASCII outside strings and comments plain", () => {
  const source = "query λx() { \"§\" } // é";
  const segs = mill.tokenizeFr(source);
  assert.equal(classOf(segs, source, "λ"), "");
  assert.equal(classOf(segs, source, "\"§\""), "tk-st");
  assert.equal(classOf(segs, source, "// é"), "tk-co");
});

test("tokenizeJson gives back every case file and classes keys, values, and punctuation", () => {
  for (const rel of [
    "examples/trust/cases/two-certificates-open-eligibility.json",
    "examples/trust/cases/court-selects-i2.json",
    "examples/trust/cases/one-certificate.json"
  ]) {
    const text = read(rel);
    assert.equal(joined(mill.tokenizeJson(text)), text, rel);
  }
  const text = "{\n  \"schema\": \"fidryn.case-record/v0.1\",\n  \"n\" : -1.5e3,\n  \"ok\": [true, false, null],\n  bad §\n}";
  const segs = mill.tokenizeJson(text);
  assert.equal(joined(segs), text);
  assert.equal(classOf(segs, text, "\"schema\""), "tk-ty");
  assert.equal(classOf(segs, text, "\"fidryn"), "tk-st");
  assert.equal(classOf(segs, text, "\"n\""), "tk-ty");
  assert.equal(classOf(segs, text, "-1.5e3"), "tk-nu");
  assert.equal(classOf(segs, text, "true"), "tk-nu");
  assert.equal(classOf(segs, text, "null"), "tk-nu");
  assert.equal(classOf(segs, text, "{"), "tk-pu");
  assert.equal(classOf(segs, text, ":"), "tk-pu");
  assert.equal(classOf(segs, text, "bad"), "");
});

test("applyRanges splits a segment at the range edges", () => {
  assert.deepEqual(
    mill.applyRanges([{ text: "hello world", cls: "tk-kw" }], [{ start: 2, end: 7, cls: "mill-sq" }]),
    [
      { text: "he", cls: "tk-kw" },
      { text: "llo w", cls: "tk-kw mill-sq" },
      { text: "orld", cls: "tk-kw" }
    ]
  );
});

test("applyRanges covers ranges across segments and overlapping ranges", () => {
  const segs = [{ text: "ab", cls: "" }, { text: "cd", cls: "tk-st" }];
  assert.deepEqual(mill.applyRanges(segs, [{ start: 1, end: 3, cls: "a" }, { start: 2, end: 4, cls: "b" }]), [
    { text: "a", cls: "" },
    { text: "b", cls: "a" },
    { text: "c", cls: "tk-st a b" },
    { text: "d", cls: "tk-st b" }
  ]);
  assert.deepEqual(mill.applyRanges(segs, [{ start: 0, end: 4, cls: "x" }, { start: 0, end: 4, cls: "x" }]), [
    { text: "ab", cls: "x" },
    { text: "cd", cls: "tk-st x" }
  ]);
});

test("applyRanges ignores empty ranges and never changes the text", () => {
  const segs = mill.tokenizeFr("query q() -> Int { return 7 }");
  assert.deepEqual(mill.applyRanges(segs, [{ start: 3, end: 3, cls: "mill-sq" }]), segs);
  assert.deepEqual(mill.applyRanges(segs, []), segs);
  const marked = mill.applyRanges(segs, [{ start: 6, end: 20, cls: "mill-sq" }, { start: 9, end: 11, cls: "mill-sq" }]);
  assert.equal(joined(marked), joined(segs));
});

test("a server span after multi-byte text squiggles exactly the offending word", () => {
  const source = "module A version \"1\" {\n    // § é 𝔽 — “q” → 中\n    colour blue\n}\n";
  const start = Buffer.byteLength(source.slice(0, source.indexOf("colour")), "utf8");
  const from = mill.byteToIndex(source, start);
  const to = mill.byteToIndex(source, start + "colour".length);
  const segs = mill.applyRanges(mill.tokenizeFr(source), [{ start: from, end: to, cls: "mill-sq" }]);
  const squiggled = segs.filter((s) => s.cls.split(" ").includes("mill-sq")).map((s) => s.text).join("");
  assert.equal(squiggled, "colour");
});

test("segmentsToHtml escapes every segment", () => {
  assert.equal(
    mill.segmentsToHtml([
      { text: "<b>", cls: "" },
      { text: "\"&'", cls: "tk-st" },
      { text: "x", cls: "tk-kw mill-sq" }
    ]),
    "&lt;b&gt;<span class=\"tk-st\">&quot;&amp;&#39;</span><span class=\"tk-kw mill-sq\">x</span>"
  );
  const html = mill.segmentsToHtml(mill.tokenizeFr("query q() -> Int { return \"<script>\" }"));
  assert.equal(html.includes("<script>"), false);
  assert.equal(html.includes("&lt;script&gt;"), true);
});

test("KEYWORDS is sorted, unique, and made of [a-z_]", () => {
  assert.ok(mill.KEYWORDS.length > 100);
  assert.deepEqual([...mill.KEYWORDS].sort(), mill.KEYWORDS);
  assert.equal(new Set(mill.KEYWORDS).size, mill.KEYWORDS.length);
  for (const word of mill.KEYWORDS) assert.match(word, /^[a-z_]{2,}$/);
  for (const word of ["module", "query", "require", "return", "outside_scope"]) assert.ok(mill.KEYWORDS.includes(word), word);
});

test("editorKey: Tab indents, Shift-Tab outdents, Enter keeps indentation", () => {
  assert.deepEqual(mill.editorKey("Tab", {}, false), { action: "indent", escaped: false });
  assert.deepEqual(mill.editorKey("Tab", { shift: true }, false), { action: "outdent", escaped: false });
  assert.deepEqual(mill.editorKey("Enter", {}, false), { action: "newline", escaped: false });
  assert.deepEqual(mill.editorKey("Enter", { ctrl: true }, false), { action: "", escaped: false });
  assert.deepEqual(mill.editorKey("Enter", { meta: true, shift: true }, false), { action: "", escaped: false });
  assert.deepEqual(mill.editorKey("Enter", { composing: true }, false), { action: "", escaped: false });
  assert.deepEqual(mill.editorKey("Tab", { ctrl: true }, false), { action: "", escaped: false });
});

test("editorKey: Escape lets the next Tab or Shift-Tab leave the editor once", () => {
  let step = mill.editorKey("Escape", {}, false);
  assert.deepEqual(step, { action: "", escaped: true });
  step = mill.editorKey("Tab", {}, step.escaped);
  assert.deepEqual(step, { action: "leave", escaped: false });
  assert.equal(mill.editorKey("Tab", {}, step.escaped).action, "indent");

  step = mill.editorKey("Escape", {}, false);
  step = mill.editorKey("Shift", { shift: true }, step.escaped);
  assert.equal(step.escaped, true);
  assert.equal(mill.editorKey("Tab", { shift: true }, step.escaped).action, "leave");
});

test("editorKey: any other key after Escape clears the flag", () => {
  for (const key of ["a", "Enter", "ArrowDown", "Backspace", " "]) {
    const step = mill.editorKey(key, {}, true);
    assert.equal(step.escaped, false, key);
    assert.equal(mill.editorKey("Tab", {}, step.escaped).action, "indent", key);
  }
});

test("JSON literals highlight only as whole words, as on the site", () => {
  const segs = mill.tokenizeJson("[nullable, null, truer, true, xfalse, false]");
  assert.deepEqual(segs.filter((s) => s.cls === "tk-nu").map((s) => s.text), ["null", "true", "false"]);
});

test("loadState turns CRLF and CR line ends into LF", () => {
  const stored = { v: 1, module: "a\r\nb\rc", case: "{\r\n}", template: "t\r\n", query: "q", validAt: "", knownAt: "" };
  const state = mill.loadState({ getItem: () => JSON.stringify(stored) }, {});
  assert.equal(state.module, "a\nb\nc");
  assert.equal(state.case, "{\n}");
  assert.equal(state.template, "t\n");
});
