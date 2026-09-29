"use strict";

// Search helpers from site/assets/fidryn.js. Run with:
//   node --test web/tests/site.test.js
const test = require("node:test");
const assert = require("node:assert/strict");
const site = require("../../site/assets/fidryn.js");

const { tokens, score, rank } = site;

// A small index in the shape of site/search-index.json. No entry contains a
// word starting with "z", and the "Envelope" entry has no word starting with "c".
const INDEX = [
  { u: "/docs/outcomes", n: "§6", h: "Outcomes", p: "Outcomes", t: "This guide explains the six honest results a Fidryn query can return." },
  { u: "/docs/outcomes#reading-a-result", n: "6.5", h: "Reading a result", p: "Outcomes", t: "Read modelBoundary and asOf before outcome.kind." },
  { u: "/docs/outcomes#envelope", n: "6.1", h: "Envelope", p: "Outcomes", t: "The outcome document is nested in the evaluation report." },
  { u: "/docs/outcomes#outcome-kinds", n: "6.3", h: "Outcome kinds", p: "Outcomes", t: "outcome is tagged by kind. Every kind includes trace." },
  { u: "/docs/cli", n: "§4", h: "CLI", p: "CLI", t: "Every fidryn subcommand and flag the reference binary accepts." },
  { u: "/docs/cases-and-time", n: "§3", h: "Cases and time", p: "Cases and time", t: "Case records, admissible completions, valid-at and known-at." },
];

function urls(entries) {
  return entries.map((e) => e.u);
}

test("the module exports exactly the search helpers when there is no DOM", () => {
  assert.deepEqual(Object.keys(site).sort(), ["rank", "score", "tokens"]);
});

test("tokens lowercases and splits on everything outside [a-z0-9_]", () => {
  assert.deepEqual(tokens("  Outcome KINDS "), ["outcome", "kinds"]);
  assert.deepEqual(tokens("valid-at/known_at"), ["valid", "at", "known_at"]);
  assert.deepEqual(tokens("E100: invalid token"), ["e100", "invalid", "token"]);
  assert.deepEqual(tokens(""), []);
  assert.deepEqual(tokens(null), []);
});

test("regex metacharacters are separators, never patterns", () => {
  assert.deepEqual(tokens("c++"), ["c"]);
  assert.deepEqual(tokens("("), []);
  assert.deepEqual(tokens("[a-z]"), ["a", "z"]);
  assert.deepEqual(tokens("$^"), []);
  assert.deepEqual(tokens("\\"), []);
  assert.deepEqual(tokens(".*"), []);
  assert.deepEqual(tokens("a|b"), ["a", "b"]);
});

test("regex-special queries never throw and never match everything", () => {
  for (const q of ["c++", "(", ")", "[a-z]", "$^", "\\", ".*", "a|b", "?", "*", "{2}", "\\d+"]) {
    let hits;
    assert.doesNotThrow(() => { hits = rank(INDEX, q); }, `query ${JSON.stringify(q)}`);
    assert.ok(Array.isArray(hits), `query ${JSON.stringify(q)}`);
    assert.ok(hits.length < INDEX.length, `query ${JSON.stringify(q)} matched every entry`);
  }
  assert.deepEqual(rank(INDEX, "("), []);
  assert.deepEqual(rank(INDEX, "$^"), []);
  assert.deepEqual(rank(INDEX, "\\"), []);
  assert.deepEqual(rank(INDEX, "[a-z]"), [], "no entry has a word starting with z");
  const plus = rank(INDEX, "c++");
  assert.ok(plus.length > 0, "c++ searches for words starting with c");
  assert.ok(!urls(plus).includes("/docs/outcomes#envelope"), "the Envelope entry has no word starting with c");
});

test("score weighs heading 10, page 4, text 1 per token, by word prefix", () => {
  assert.equal(score({ h: "Outcome kinds", p: "Guide", t: "nothing here" }, ["outc"]), 10);
  assert.equal(score({ h: "Envelope", p: "Outcomes", t: "nothing here" }, ["outc"]), 4);
  assert.equal(score({ h: "Envelope", p: "Guide", t: "the outcome is nested" }, ["outc"]), 1);
  assert.equal(score({ h: "Outcome kinds", p: "Outcomes", t: "outcome" }, ["outc"]), 15);
  assert.equal(score({ h: "Incoming", p: "Guide", t: "income" }, ["com"]), 0, "prefix of a word, not a substring");
});

test("every token must match some word", () => {
  const entry = { h: "Outcome kinds", p: "Outcomes", t: "Every kind includes trace." };
  assert.equal(score(entry, ["outcome", "trace"]), 10 + 4 + 1);
  assert.equal(score(entry, ["outcome", "zebra"]), 0);
  assert.deepEqual(urls(rank(INDEX, "valid known")), ["/docs/cases-and-time"]);
  assert.deepEqual(rank(INDEX, "outcome zebra"), []);
});

test("heading matches outrank text matches", () => {
  // "Reading a result" mentions outcome only in its text and comes first in the index.
  const hits = urls(rank(INDEX, "outcome"));
  assert.equal(hits[0], "/docs/outcomes#outcome-kinds");
  assert.ok(hits.indexOf("/docs/outcomes#outcome-kinds") < hits.indexOf("/docs/outcomes#reading-a-result"));
  assert.deepEqual(urls(rank(INDEX, "ENVELOPE")), ["/docs/outcomes#envelope"]);
});

test("rank returns the index's own entries", () => {
  assert.equal(rank(INDEX, "envelope")[0], INDEX[2]);
});

test("rank returns at most eight results, ties in index order", () => {
  const many = Array.from({ length: 20 }, (_, i) => ({ u: `/docs/x#r${i}`, n: `1.${i}`, h: `Rule ${i}`, p: "Rules", t: "" }));
  const hits = rank(many, "rule");
  assert.equal(hits.length, 8);
  assert.deepEqual(urls(hits), urls(many.slice(0, 8)));

  const mixed = [
    { u: "/a", h: "Alpha", p: "Guide", t: "the rule" },
    { u: "/b", h: "Rule one", p: "Guide", t: "" },
    { u: "/c", h: "Beta", p: "Guide", t: "a rule" },
    { u: "/d", h: "Rule two", p: "Guide", t: "" },
  ];
  assert.deepEqual(urls(rank(mixed, "rule")), ["/b", "/d", "/a", "/c"]);
});

test("empty and separator-only queries return nothing", () => {
  assert.deepEqual(rank(INDEX, ""), []);
  assert.deepEqual(rank(INDEX, "   "), []);
  assert.deepEqual(rank(INDEX, "-- / ."), []);
  assert.equal(score(INDEX[0], []), 0);
});
