# Statute site and mill redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the public site, the learner docs, and the localhost mill with the "Statute" design, with the docs generated from `docs/*.md` by `cargo xtask site` and every page working on desktop and phones.

**Architecture:** A `site` module in the existing `xtask` crate renders `site/` (landing page, nine docs pages, 404, markdown mirrors, search index, SEO files) from `docs/*.md`, using pulldown-cmark, the real `.fr` lexer for highlighting, and the real interpreter for the landing specimen. A hand-written design system (`site/assets/fidryn.css`, `site/assets/fidryn.js`) serves both the site and the mill. The mill (`web/index.html`, `web/mill.css`, `web/mill.js`) is embedded in the `fidryn` binary; `ui.rs` gains static routes, a samples API, a strict CSP, and an `opinion` field built by a new `fidryn_cli::opinion` module that the specimen also uses.

**Tech Stack:** Rust 1.98 (edition 2024) workspace, `pulldown-cmark` 0.13 (`default-features = false, features = ["html"]`), axum 0.8, serde_json; hand-written HTML, CSS, and vanilla JavaScript; self-hosted Fraunces and IBM Plex fonts. Node 18+ only to run `node --test` on pure JS helpers when available.

**Spec:** `.agents/docs/specs/2026-09-28-site-statute-redesign-design.md` (read it first; it holds every visual decision and the board pick codes).

## Global Constraints

- Branch `feat/site-statute-redesign`. Commit after every task with Conventional Commits (`feat(cli): …`, `feat(site): …`, `feat(mill): …`, `test(…)`, `docs: …`, `build: …`) and always `git -c commit.gpgsign=false commit` (the global signing key is expired; repo history is unsigned).
- Every cargo command runs offline: `cargo test --offline …`, `cargo clippy --offline …`, `cargo xtask …` (the alias is `run --package xtask --`).
- Rust stack only: no JS framework, no npm, no Node build step, no vendored editor, no CDN, no analytics, no external requests from any page. Node is never needed to build, serve, or deploy; it only runs `node --test` over `web/tests/*.test.js`.
- `docs/*.md` is canonical. Everything the generator writes under `site/` is overwritten on every run; hand-written sources (`site/assets/*`, `site/fonts/*`, `site/favicon.svg`, `site/vercel.json`, `site/README.md`, `site/.gitignore`) are never written by the generator.
- Generated output is deterministic: no timestamps, stable ordering. Two runs produce identical bytes.
- Pages are readable with JavaScript disabled; JS only enhances. Controls that need JS (`[data-copy]`, `[data-theme-toggle]`, `[data-drawer-open]`) ship with the `hidden` attribute and JS removes it.
- The mill gains no runtime filesystem access: every asset, font, and sample is embedded with `include_str!` / `include_bytes!`. `GET /` sends `Content-Security-Policy: default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`, so `web/index.html` has no inline `<script>` and no `style="…"` attributes.
- Dynamic content in JS is written with `textContent` or built DOM nodes, never `innerHTML` of server responses, pasted text, or the search index. (Highlighted code built by our own tokenizers from escaped text is the only HTML string assembly, and it escapes every token.)
- WCAG AA: every token pair in spec section 2 is at least 4.5:1 (enforced by a test); visible focus rings in `--rubric`; 44px touch targets below 720px; `prefers-reduced-motion` disables transitions.
- Breakpoints: phone `max-width: 719px`, tablet `720px–1199px`, desktop `min-width: 1200px`. At 390px wide no page scrolls horizontally; only code frames and tables scroll inside themselves.
- No emojis in code or comments. Never mention AI tools in code, comments, or commits.
- Copy rules: the site's words come from the spec and `docs/`; never invent claims about Fidryn's semantics. Opinion sentences are fixed templates filled from report fields.

## Review Focus

These five inputs are the most likely to bite a person and are not covered by the spec's own tests; each line names the task whose tests pin it.

1. Non-ASCII text in the mill editor (`§`, `é`, emoji-free but multi-byte characters in strings or comments): diagnostics arrive as UTF-8 byte offsets, and the squiggle and caret must land on the right characters. Pinned by `byteToIndex` tests in Task 16.
2. A phone (390px) on the Examples and CLI guides, which have very wide tables and long command lines: the page itself must never scroll sideways; only the table or code frame scrolls. Pinned by the table-wrap test in Task 8, the CSS rules in Task 5, and the overflow audit in Task 18.
3. The mill server stopping mid-session (the user presses Ctrl-C on `fidryn ui` while the page is open): actions must show "is `fidryn ui` still running?" and the health dot must turn red, with no unhandled promise rejection. Pinned by `postJson` tests in Task 15.
4. Old or corrupt saved mill state in `localStorage` (hand-edited, from an earlier build, or not JSON): the mill must fall back to the default sample instead of breaking. Pinned by `loadState` tests in Task 15.
5. Search queries with regex-special characters (`c++`, `(`, `[a-z]`, `\`): search must neither throw nor match everything. Pinned by the `tokens` / `score` tests in Task 6.

## File Structure

| Path | Responsibility | Task |
| --- | --- | --- |
| `crates/fidryn-cli/src/lib.rs` | add `CaseInput`, `RunRequest`, `run_report_text`; `cmd_run` delegates to it | 1 |
| `crates/fidryn-cli/tests/run_report_text.rs` | library-level tests of the `fidryn run` path | 1 |
| `crates/fidryn-cli/src/opinion.rs` | report JSON to plain sentences, per outcome kind | 2 |
| `crates/fidryn-cli/src/ui.rs` | `opinion` in eval transports (3); CSP, fidryn.css, favicon, fonts, `/api/samples` (13); `/assets/mill.css` route (14); `/assets/mill.js` route (15) | 3, 13, 14, 15 |
| `schemas/mill-evaluation-response-v0.1.json` | declares the optional `opinion` transport field | 3 |
| `docs/mill.md` | transport field (3), routes table (13), mill asset rows and "The HTML page" (17) | 3, 13, 17 |
| `xtask/src/main.rs` | `Site` subcommand | 4 |
| `xtask/src/site/mod.rs` | `SiteArgs`, `OutFile`, `run`, `check`, `build`, `stale_files`, `write_files` | 4, 9–11 |
| `xtask/src/site/guides.rs` | guide table, groups, URLs, site constants | 4 |
| `xtask/src/site/html.rs` | escaping, asset version hash | 4 |
| `xtask/src/site/templates.rs` | `{{slot}}` filling | 4 |
| `xtask/src/site/seo.rs` | robots, sitemap (4); head meta (10); mirrors, llms files, landing markdown (11) | 4, 10, 11 |
| `site/assets/fidryn.css` | the design system, tokens, all shared components | 5 |
| `site/favicon.svg` | F monogram | 5 |
| `xtask/tests/design_tokens.rs` | WCAG contrast of the tokens | 5 |
| `site/assets/fidryn.js` | theme, copy, drawer, search, scrollspy, specimen tabs | 6 |
| `web/tests/site.test.js` | node tests for search helpers | 6 |
| `site/package.json` | drop `"type": "module"` so Node can `require` the site script in tests (the file is deleted in Task 12) | 6 |
| `xtask/src/workspace.rs`, `xtask/src/ci.rs` | JS test step (6); site check step (12) | 6, 12 |
| `xtask/src/site/highlight.rs` | `Lang`, `Keywords`, `sniff`, `highlight`, `code_frame`, `numbered_frame` | 7 |
| `xtask/src/site/links.rs` | link rewriting for pages and mirrors | 8 |
| `xtask/src/site/markdown.rs` | guide markdown to `Page` (ids, numbers, TOC, sections) | 8 |
| `docs/examples.md` | drop a stale mirror front matter so the guide opens with its title | 8 |
| `xtask/src/site/specimen.rs` | the four landing runs through the interpreter | 9 |
| `xtask/src/site/templates/{base,landing,doc,404}.html` | page shells | 10 |
| `xtask/src/site/pages.rs` | landing, doc, 404, guides nav, pager, TOC | 10 |
| `xtask/src/site/search.rs` | `search-index.json` | 11 |
| `site/**` generated files, `site/vercel.json`, Node file removal | output and deploy config | 12 |
| `xtask/tests/site_output.rs` | integrity of the committed site | 12 |
| `web/index.html`, `web/mill.css` | mill page shell and mill-only styles | 14 |
| `xtask/tests/mill_page.rs` | CSP-compliance and required strings of `web/index.html` | 14 |
| `web/mill.js` | the mill app (15), the editor (16) | 15, 16 |
| `web/tests/mill.test.js` | node tests for mill helpers | 15, 16 |
| `xtask/tests/mill_keywords.rs` | `mill.js` keyword list equals the grammar | 16 |
| `site/README.md`, `docs/contributing.md`, `docs/outcomes.md` (callout), `.agents/adr/0001-rust-site-generator.md`, `.agents/ADR.md`, `.agents/ARCHITECTURE.md` | docs and decision record | 17 |
| `.agents/scripts/shoot.py` | headless-Chrome screenshots, overflow audit | 18 |

## Task Order and Dependencies

Tasks run in numeric order. Streams that share no files may run in parallel once their inputs exist:

- CLI stream: 1 → 2 → 3 → 13 (13 needs Task 5's `site/assets/fidryn.css` and `site/favicon.svg`, which it embeds).
- Generator stream: 4 → 7 → 8, then 9 (needs 1, 2) → 10 (needs 5, 6 files to exist for asset hashes) → 11 → 12 (needs every site task).
- Design stream: 5 → 6 (Task 5's test file lives in `xtask/tests/`, so it only needs the xtask crate).
- Mill stream: 14 (needs 5 and 13: it adds the `/assets/mill.css` route to `ui.rs` after Task 13's routes) → 15 (adds the `/assets/mill.js` route) → 16.
- 17 after 13 and 16; 18 last.

Parallel-safe pairs (no shared files): {1–3, 13} with {4, 7, 8} with {5, 6} with {14}. Tasks 9–12 wait for 1, 2, 5, 6. Task 12 edits `xtask/src/ci.rs` after Task 6 has; its edit anchors on a different line.

## Shared Contract

Every task below is written against this section. Names, signatures, markup, class names, ids, and data shapes here are fixed; a task may add private helpers but must not rename or reshape anything listed. Repository root: `/Users/beegass/Projects/fidryn`.

### C1. Facts verified before planning

- `fidryn run` path: `compile_module(path)` (uses the thread-local `Driver` and loads the source manifest), then the case JSON parsed with `serde_json` into `CaseRecord`, `apply_run_args`, `parse_instant` for both clocks, `Driver::run_report` (or `run_report_scenario`), then `render_report(&module, &QueryName::from(query), valid, known, &case, &report)` which returns the canonical `fidryn.evaluation-report/v0.1` JSON text. `render_report` is `pub(crate) use fidryn_trace::render_report;` inside `fidryn-cli`.
- Real outcomes (checked through the running mill and the CLI):
  - `tests/programs/require-gate.fr`, empty case, `2026-09-17T12:00:00Z`: `q` → `determinate`, value `{"kind":"int","data":7}`; `r` → `suspended`, `requests: [{"kind":"needCustom","effect":"require","payload":"requirement failed"}]`.
  - `tests/programs/late-payment.fr`, empty case, same time: `due` → `determinate` (a `ctor` value `USD` with field `_0` decimal `"100.00"`); `paid_on_time` → `suspended` (`needEvidence`, schema `PaymentRecord`).
  - `examples/trust/bryan-revocable-trust.fr`, `2034-03-01T09:00:00Z` for both clocks, query `acting_trustee`: case `examples/trust/cases/two-certificates-open-eligibility.json` → `contingent`, `alternatives: {"I1": {"kind":"entity","data":"Alice"}, "I2": {"kind":"entity","data":"Bob"}}`, `pivots: [{"family":"SuccessorEligibility","kind":"needInterpretation", …}]`; `court-selects-i2.json` → `determinate` `{"kind":"entity","data":"Bob"}`; `one-certificate.json` → `suspended`. The trust module also compiles through the mill's in-memory `compile_source` (pasted source, default manifest).
  - The empty case record: `{"schema":"fidryn.case-record/v0.1","admissibleCompletions":{}}`.
- Outcome shapes (schema `schemas/outcome-v0.1.json`): every outcome has `kind` and `trace`; `determinate` requires `value`, optional `convergenceCertificate` (32 hex or null) and `ignoredOpenIssues` (array); `contingent` requires `alternatives` (object: completion key → value) and `pivots` (array; items carry `kind` and `family` or `protocol`); `suspended` requires `requests` (array of `{kind, …}`: `needEvidence` {issue, schema}, `needJudgment` {issue, protocol}, `needChoice` {protocol, options}, `needInterpretation` {source, family}, `needApplicableLaw` {issue, candidates}, `needConflict` {graph, doctrines}, `needCustom` {effect, payload}); `normConflict` requires `doctrines` (items with `name`); `outsideCompetence` requires `request` and `reason`; `inconsistent` requires `core` (array of strings). Explore keys alternatives as `"i:SuccessorEligibility=I1"`; run keys them as `"I1"`.
- Runtime values are tagged `{"kind": K, "data": D}` with K in `unit` (no data), `bool`, `int`, `decimal` (string data), `string`, `instant`, `duration`, `prop`, `entity` (string), `ctor` (data `{"name", "fields": {"_0": value, …}}`), `set` (array), `map` (object), `option` (null or value), `clauseRef`; unknown shapes are passed through as plain JSON.
- Diagnostics serialize as `{"code": "E100", "message": "…", "primary_span": {"start": u32, "end": u32} | null, "related_spans": [...], "suggestion": string | null}`; spans are UTF-8 byte offsets into the pasted source. `E100` is the parse-error code; the parser's messages look like ``invalid token `colour` ``.
- `fidryn_syntax::lex(&str) -> Vec<Token>`; `Token { kind: TokenKind, start: u32, end: u32 }` covers trivia (`Whitespace`, `Newline`, `Comment`, `DocComment`) and ends with an empty `Eof`. `TokenKind` variants: `Ident, String, Int, Decimal, Date, DateTime, DurationUnit, PlusInf, MinusInf, Comment, DocComment, Whitespace, Newline, LBrace, RBrace, LParen, RParen, LBracket, RBracket, Comma, Dot, Colon, Semicolon, Bang, Arrow, FatArrow, Eq, EqEq, Ne, Lt, Le, Gt, Ge, Plus, Minus, Star, Slash, Pipe, Range, Eof, Error`. Keywords are contextual identifiers: the set is every quoted terminal in `grammar.ebnf` made only of `[a-z_]`, longer than one character (`"module"`, `"query"`, `"require"`, `"return"`, `"outside_scope"`, …). `true` and `false` are not quoted in the grammar and are highlighted as literals.
- `pulldown-cmark = { version = "0.13", default-features = false, features = ["html"] }` resolves and builds offline (0.13.4 is in the registry cache). `HeadingLevel` has discriminants `H1 = 1 … H6`; `Tag::Link { link_type, dest_url, title, id }`; `TagEnd::{Heading(HeadingLevel), CodeBlock, Table, …}`; `pulldown_cmark::html::push_html`.
- Docs link targets seen in `docs/*.md`: learner guides (`cli.md`, `mill.md`, `outcomes.md`, `getting-started.md`, `cases-and-time.md`, `examples.md`, `language.md`, `contributing.md`, `README.md`), implementer docs (`ARCHITECTURE.md`, `implementation-status.md`, `OBLIGATIONS.md`, `INTEGRATION-CONTRACT.md`, `WORKSTREAM-CONTRACT.md`, `REVIEW-FIX-CONTRACT.md`, `FULL-IMPLEMENTATION.md`), `../` repository paths (files and directories ending in `/`), and `#anchor`-only links. About 140 fenced blocks; nearly all unlabeled, plus `json`, `rust`, `text`.
- Existing SEO descriptions, kept verbatim in `guides.rs`; `site/robots.txt` and `site/llms.txt` formats are reproduced by `seo.rs`.
- Node v18.14 is installed locally; `node --test FILE…` and `require("node:test")` work. Pass explicit file paths (directory arguments differ across Node versions).
- `site/.gitignore` contains `node_modules/` and `_deploy_files.json`.

### C2. `fidryn-cli` public API (Tasks 1–3, 13)

```rust
// crates/fidryn-cli/src/lib.rs (Task 1)
#[derive(Clone, Copy, Debug)]
pub enum CaseInput<'a> { File(&'a Path), Json(&'a str) }

#[derive(Clone, Copy, Debug)]
pub struct RunRequest<'a> {
    pub path: &'a Path,
    pub query: &'a str,
    pub case: CaseInput<'a>,
    pub valid_at: &'a str,
    pub known_at: &'a str,
    pub args: &'a [String],
    pub scenario: bool,
}

/// Evaluate exactly as `fidryn run` does; Ok is the canonical report JSON
/// text the CLI prints, Err is the text the CLI writes to stderr.
pub fn run_report_text(req: &RunRequest<'_>) -> Result<String, String>;

// crates/fidryn-cli/src/opinion.rs (Task 2), `pub mod opinion;` in lib.rs
pub fn sentences(report: &serde_json::Value) -> Vec<String>;   // report = fidryn.evaluation-report/v0.1
pub fn value_text(value: &serde_json::Value) -> String;
pub fn request_text(request: &serde_json::Value) -> String;
```

Error strings of `run_report_text` are the CLI's existing stderr lines: compile diagnostics joined with `\n`; `cannot read {path}: {io error}`; `invalid case record: {serde error}`; the `--arg` messages of `apply_run_args`; `--valid-at: {err}. Use ISO 8601 / RFC 3339, for example 2033-01-01T00:00:00Z or 2033-01-01T00:00:00+00:00.` (same for `--known-at`); engine failures as `{Kind}: {message}` (for example `UnknownQuery: …`). The order of checks is unchanged: compile, case, `--arg`, valid-at, known-at, evaluate.

Opinion sentences (exact templates; `{q}` is `outcomeDocument.query`):

| Kind | Sentences |
| --- | --- |
| determinate | `{q} is {value}.`; if `convergenceCertificate` is a string: `Covered by convergence certificate {id}.`; if `ignoredOpenIssues` has n > 0 items: `1 open issue was set aside under that certificate.` / `{n} open issues were set aside under that certificate.` |
| contingent | if pivots name families: `{q} depends on {F1 and F2}.` (each pivot contributes `family`, else `protocol`, else `kind`; duplicates dropped, joined with ` and `); one `Under {completion} it is {value}.` per alternative in key order (completion: strip a leading `x:` prefix, then `=` becomes ` = `); then `No single answer is determinate across the admissible completions.` |
| suspended | `{q} is suspended.` then `Outstanding request: {r}.` for one request or `Outstanding requests: {r1}; {r2}.` for several |
| normConflict | `{q} ended in a norm conflict: no single applicable doctrine among {names}.` (names from `doctrines[].name`, joined `, `; if none: `{q} ended in a norm conflict.`) |
| outsideCompetence | `{q} is outside the module's competence: {reason}.` then `Request: {r}.` |
| inconsistent | `{q} has no consistent answer: the admitted model cannot be satisfied.` then, if `core` is non-empty, `Unsatisfiable core: {a, b}.` |
| other kind k | `{q} returned an outcome of kind {k}.` |
| no outcome object | `The report has no outcome.` |
| always last, if `modelBoundary.outsideScope` is non-empty | `Outside scope: {a, b}.` |

`value_text`: int, decimal, bool, instant, duration → the data as text; string → `"text"` in double quotes; entity → the name; unit → `unit`; ctor → `Name` when it has no fields, `Name(v0, v1)` when every field key starts with `_`, otherwise `Name(k: v, …)`; set → `{a, b}`; map → `{k: v, …}`; option → `none` or the inner value; anything else → compact JSON.

`request_text`: needCustom → `{payload} ({effect})` (payload as text; if payload is not a string, just `{effect}`); needEvidence → `evidence matching {schema}`; needInterpretation → `an interpretation of {family}` plus ` under {source}` when present; needJudgment → `a determination under {protocol}`; needChoice → `a decision under {protocol}` plus ` among {o1, o2}` when options are strings; needApplicableLaw → `applicable law` plus ` among {c1, c2}` when candidates are strings; needConflict → `one applicable conflict doctrine` plus ` among {names}`; other kind → the kind; missing kind → compact JSON.

### C3. Mill HTTP API (Tasks 3, 13 build it; Tasks 14–16 use it)

| Method and path | Request | Response |
| --- | --- | --- |
| `GET /` | | `web/index.html`, headers: `content-type: text/html; charset=utf-8`, the CSP from Global Constraints, `x-content-type-options: nosniff`, `referrer-policy: no-referrer`, `cache-control: no-cache` |
| `GET /assets/fidryn.css` | | `site/assets/fidryn.css`, `text/css; charset=utf-8` |
| `GET /assets/mill.css` | | `web/mill.css`, `text/css; charset=utf-8` |
| `GET /assets/mill.js` | | `web/mill.js`, `text/javascript; charset=utf-8` |
| `GET /favicon.svg` | | `site/favicon.svg`, `image/svg+xml` |
| `GET /fonts/{name}` | name in `fraunces.woff2`, `plex-sans.woff2`, `plex-mono-400.woff2`, `plex-mono-500.woff2` | the font, `font/woff2`; any other name → 404 |
| `GET /api/health` | | `ok` (unchanged) |
| `GET /api/samples` | | JSON array, see below, `application/json` |
| `POST /api/check` | `{"source"}` | unchanged: `200 {"ok": true, "diagnostics": []}` or `200 {"ok": false, "diagnostics": [...]}` |
| `POST /api/run`, `POST /api/explore` | `{"source","query","case","validAt","knownAt"}` (`case` is a JSON value) | success `200 {"ok": true, "report": {...}, "opinion": ["…", …]}`; check failure `400 {"ok": false, "error": "check failed", "diagnostics": [...]}`; bad case or time `400 {"ok": false, "error": "invalid case: …" / "validAt: …" / "knownAt: …", "diagnostics": []}`; engine failure `400` (or `500` for `Internal`) `{"kind": "engineError", "error": "UnknownQuery", "message": "…", "ok": false}` |
| `POST /api/render` | `{"source","template"}` | unchanged: `200 {"ok": true, "text"}` or `200 {"ok": false, "error"}` |

Static routes send `cache-control: no-cache`. `opinion` is transport, like `ok`; it is never inside `report`.

Which task adds which route, and the anchors later tasks edit against. Task 13 adds to `crates/fidryn-cli/src/ui.rs`, next to `const INDEX`:

```rust
const SITE_CSS: &str = include_str!("../../../site/assets/fidryn.css");
const FAVICON: &str = include_str!("../../../site/favicon.svg");
```

a helper and handlers:

```rust
/// A compile-time embedded text asset, revalidated on every load.
fn static_text(content_type: &'static str, body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, content_type), (header::CACHE_CONTROL, "no-cache")], body).into_response()
}

async fn site_css() -> Response {
    static_text("text/css; charset=utf-8", SITE_CSS)
}
```

and these router lines, in this order, right after `.route("/", get(index))`:

```rust
        .route("/assets/fidryn.css", get(site_css))
        .route("/favicon.svg", get(favicon))
        .route("/fonts/{name}", get(font))
```

plus `.route("/api/samples", get(samples))` after the `/api/health` route. Task 14 adds `const MILL_CSS: &str = include_str!("../../../web/mill.css");` on the line after `const FAVICON`, an `async fn mill_css() -> Response` using `static_text("text/css; charset=utf-8", MILL_CSS)`, and `.route("/assets/mill.css", get(mill_css))` on the line after the `site_css` route. Task 15 adds `const MILL_JS: &str = include_str!("../../../web/mill.js");` after `const MILL_CSS`, `async fn mill_js() -> Response` using `static_text("text/javascript; charset=utf-8", MILL_JS)`, and `.route("/assets/mill.js", get(mill_js))` after the `mill_css` route. Tasks 14 and 15 each add a router test for their route next to Task 13's static-route tests.

`/api/samples` (order fixed; `case` is the case file text, not parsed JSON):

| id | title | blurb | source | case | query | validAt = knownAt | action | expect |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `require-gate` | `require-gate` | `q returns 7; r stops at a false require` | `tests/programs/require-gate.fr` | empty case (pretty, below) | `q` | `2026-09-17T12:00:00Z` | `run` | `determinate` |
| `late-payment` | `late-payment` | `A duty; paid_on_time needs evidence` | `tests/programs/late-payment.fr` | empty case | `due` | `2026-09-17T12:00:00Z` | `run` | `determinate` |
| `trust-open` | `Trust, open eligibility` | `Two certificates; clause 4.4 unresolved` | `examples/trust/bryan-revocable-trust.fr` | `examples/trust/cases/two-certificates-open-eligibility.json` | `acting_trustee` | `2034-03-01T09:00:00Z` | `run` | `contingent` |
| `trust-court` | `Trust, court selects I2` | `A competent authority has decided` | same module | `examples/trust/cases/court-selects-i2.json` | `acting_trustee` | `2034-03-01T09:00:00Z` | `run` | `determinate` |
| `trust-one` | `Trust, one certificate` | `Evidence is still missing` | same module | `examples/trust/cases/one-certificate.json` | `acting_trustee` | `2034-03-01T09:00:00Z` | `run` | `suspended` |

Each element: `{"id","title","blurb","source","case","query","validAt","knownAt","action","expect"}`. The pretty empty case text is exactly:

```json
{
  "schema": "fidryn.case-record/v0.1",
  "admissibleCompletions": {}
}
```

(with a trailing newline). Evaluation reports have `schema`, `executionMode`, `assumptions`, `sourceTrust`, `verificationMethod`, `coverage`, and `outcomeDocument {schema, module, sourceSnapshot, query, asOf {validTime, recordTime}, modelBoundary {outsideScope, admissibleCompletions {interpretations, evidence, choices}}, outcome}`.

### C4. Generator modules (Tasks 4, 7–12)

Tasks 4 creates these three files exactly as written here (Task 4 adds their tests).

`xtask/src/site/guides.rs`:

```rust
//! The learner guides published on the site, in reading order.

pub const SITE: &str = "https://fidryn.onlygass.dev";
pub const REPO: &str = "https://github.com/BeeGass/fidryn";

/// Sidebar groups, in order. Every numbered guide belongs to one.
pub const GROUPS: &[&str] = &["Start", "Write", "Run", "Read results", "Contribute"];

pub struct Guide {
    /// URL slug; `index` is the docs overview at `/docs/`.
    pub slug: &'static str,
    /// Source file under `docs/`.
    pub file: &'static str,
    /// Page title and navigation label.
    pub title: &'static str,
    /// Section number, `None` for the overview.
    pub number: Option<u32>,
    /// Sidebar group; empty for the overview.
    pub group: &'static str,
    /// One line under the title in the sidebar and the landing contents.
    pub blurb: &'static str,
    /// Meta description.
    pub description: &'static str,
}

pub const GUIDES: &[Guide] = &[
    Guide { slug: "index", file: "README.md", title: "Documentation", number: None, group: "", blurb: "Where to begin", description: "Fidryn documentation hub — learner guides for the programming language for legal instruments." },
    Guide { slug: "getting-started", file: "getting-started.md", title: "Getting started", number: Some(1), group: "Start", blurb: "Check and run a tiny module", description: "Build, check, and run a tiny Fidryn (.fr) program in about an hour." },
    Guide { slug: "language", file: "language.md", title: "Language", number: Some(2), group: "Write", blurb: "Modules, queries, rules, duties", description: "Fidryn language map: modules, queries, rules, duties, and what the language will not do." },
    Guide { slug: "cases-and-time", file: "cases-and-time.md", title: "Cases and time", number: Some(3), group: "Write", blurb: "Records, valid-at, known-at", description: "Case records, admissible completions, valid-at and known-at in Fidryn." },
    Guide { slug: "cli", file: "cli.md", title: "CLI", number: Some(4), group: "Run", blurb: "Every subcommand and flag", description: "Every fidryn CLI subcommand and flag the reference binary accepts." },
    Guide { slug: "mill", file: "mill.md", title: "Mill", number: Some(5), group: "Run", blurb: "The localhost UI", description: "Localhost Fidryn mill UI on 127.0.0.1 — checks modules; does not live-file." },
    Guide { slug: "outcomes", file: "outcomes.md", title: "Outcomes", number: Some(6), group: "Read results", blurb: "The six kinds and the envelope", description: "Determinate, Suspended, Contingent, and the rest of the Fidryn outcome envelope." },
    Guide { slug: "examples", file: "examples.md", title: "Examples", number: Some(7), group: "Read results", blurb: "Trust, tax, fifty states", description: "Trust, tax, federal slices, and the fifty-state corpus map for Fidryn." },
    Guide { slug: "contributing", file: "contributing.md", title: "Contributing", number: Some(8), group: "Contribute", blurb: "Toolchain, tests, commits", description: "Toolchain, tests, and how to work on the Fidryn reference interpreter." },
];

/// Implementer documents, linked to GitHub from the sidebar: (title, file under docs/, blurb).
pub const IMPLEMENTER_DOCS: &[(&str, &str, &str)] = &[
    ("Architecture", "ARCHITECTURE.md", "Pipeline and crate contract"),
    ("Implementation status", "implementation-status.md", "Evidence-backed capability matrix"),
    ("Obligations", "OBLIGATIONS.md", "Review obligations"),
    ("Integration contract", "INTEGRATION-CONTRACT.md", "How the machinery is wired"),
];

/// Site path of a guide: `/docs/` for the overview, `/docs/{slug}` otherwise.
pub fn url(slug: &str) -> String {
    if slug == "index" { "/docs/".to_owned() } else { format!("/docs/{slug}") }
}

/// The guide rendered from `file` (a name under `docs/`), if it is published.
pub fn by_file(file: &str) -> Option<&'static Guide> {
    GUIDES.iter().find(|g| g.file == file)
}
```

`xtask/src/site/html.rs`:

```rust
//! HTML escaping and asset fingerprints.

/// Escape text for HTML element content and double-quoted attributes.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Eight lowercase hex digits identifying `bytes` (FNV-1a, 64-bit, low 32 bits),
/// used as `?v=` on asset URLs so they can be cached for a year.
pub fn asset_version(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", hash & 0xffff_ffff)
}
```

`xtask/src/site/templates.rs`:

```rust
//! `{{slot}}` templates. A slot with no value is a bug and panics.

/// Replace every `{{name}}` in `template` with its value from `vars`.
pub fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 4096);
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .unwrap_or_else(|| panic!("unclosed `{{{{` in template"));
        let key = after[..end].trim();
        let value = vars
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("template slot `{key}` has no value"));
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}
```

`xtask/src/site/mod.rs` as Task 4 leaves it (Tasks 9–11 add `mod` lines and replace `build`):

```rust
//! `cargo xtask site`: render the public site under `site/` from `docs/*.md`.

mod guides;
mod html;
mod seo;
mod templates;

use crate::workspace::workspace_root;
use anyhow::{Context, Result, bail};
use clap::Args;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct SiteArgs {
    /// Fail if the committed site differs from a fresh render instead of writing it
    #[arg(long)]
    check: bool,
}

/// One generated file, relative to `site/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

impl OutFile {
    pub fn text(path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self { path: path.into(), bytes: text.into().into_bytes() }
    }
}

pub fn run(args: SiteArgs) -> Result<()> {
    let root = workspace_root();
    if args.check {
        return check(&root);
    }
    let files = build(&root)?;
    write_files(&root.join("site"), &files)?;
    eprintln!("wrote {} files under site/", files.len());
    Ok(())
}

/// Fail when the committed site differs from a fresh render.
pub fn check(root: &Path) -> Result<()> {
    let files = build(root)?;
    let stale = stale_files(&root.join("site"), &files)?;
    if !stale.is_empty() {
        bail!("site/ is out of date; run `cargo xtask site`:\n  {}", stale.join("\n  "));
    }
    eprintln!("site/ is up to date ({} generated files)", files.len());
    Ok(())
}

/// Render every generated file. Reads inputs under `root`; writes nothing.
pub fn build(_root: &Path) -> Result<Vec<OutFile>> {
    Ok(vec![
        OutFile::text("robots.txt", seo::robots()),
        OutFile::text("sitemap.xml", seo::sitemap()),
    ])
}

/// Generated files that are missing or differ on disk, then files under
/// `site/docs/` that the generator no longer produces (sorted).
pub fn stale_files(site: &Path, files: &[OutFile]) -> Result<Vec<String>> {
    let mut stale = Vec::new();
    for file in files {
        match fs::read(site.join(&file.path)) {
            Ok(bytes) if bytes == file.bytes => {}
            Ok(_) => stale.push(format!("{} differs", file.path.display())),
            Err(_) => stale.push(format!("{} is missing", file.path.display())),
        }
    }
    let expected: BTreeSet<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let docs = site.join("docs");
    if docs.is_dir() {
        let mut extra = Vec::new();
        for entry in fs::read_dir(&docs).with_context(|| format!("read {}", docs.display()))? {
            let rel = Path::new("docs").join(entry?.file_name());
            if !expected.contains(&rel) {
                extra.push(format!("{} is not generated; delete it", rel.display()));
            }
        }
        extra.sort();
        stale.extend(extra);
    }
    Ok(stale)
}

/// Write `files` under `site`, creating directories as needed.
pub fn write_files(site: &Path, files: &[OutFile]) -> Result<()> {
    for file in files {
        let path = site.join(&file.path);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        fs::write(&path, &file.bytes).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}
```

Signatures of the remaining generator modules (each task owns its module; others call only these):

```rust
// seo.rs — Task 4
pub fn robots() -> String;   // exactly the robots.txt text given after this block
pub fn sitemap() -> String;  // <urlset> with <loc> for "/", then url(g.slug) for every guide in GUIDES order; no lastmod
// seo.rs — Task 10
pub enum PageKind { Landing, Doc, NotFound }
pub fn head(title: &str, description: &str, path: &str, markdown_path: Option<&str>, kind: PageKind) -> String;
//   canonical, rel=alternate text/markdown (when markdown_path is Some), og:*, twitter:*, JSON-LD
//   (SoftwareApplication for Landing, WebPage otherwise; `</` escaped as `<\/`), robots
//   ("index,follow,max-image-preview:large", or "noindex" for NotFound), author "Bryan Gass".
// seo.rs — Task 11
pub fn mirror(guide: &guides::Guide, md: &str) -> String;       // front matter + canonical note + md with links rewritten (LinkStyle::Markdown)
pub fn landing_markdown() -> String;                             // site/index.md
pub fn llms_txt() -> String;
pub fn llms_full(mirrors: &[(&guides::Guide, String)]) -> String;

// highlight.rs — Task 7
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang { Fr, Json, Shell, Plain }
impl Lang {
    pub fn label(self) -> &'static str;   // ".fr" | "json" | "shell" | "text"
    pub fn class(self) -> &'static str;   // "fr" | "json" | "shell" | "text"
}
pub struct Keywords(/* private */);
impl Keywords {
    pub fn from_grammar(grammar: &str) -> Self;
    pub fn load(root: &Path) -> anyhow::Result<Self>;  // reads root/grammar.ebnf
    pub fn contains(&self, word: &str) -> bool;
    pub fn iter(&self) -> impl Iterator<Item = &str>;
}
pub fn sniff(info: &str, code: &str, kw: &Keywords) -> Lang;
pub fn highlight(lang: Lang, code: &str, kw: &Keywords) -> String;     // escaped HTML with <span class="tk-..">
pub fn code_frame(lang: Lang, code: &str, kw: &Keywords, caption: Option<&str>) -> String;
pub fn numbered_frame(lang: Lang, segments: &[(usize, String)], kw: &Keywords, caption: &str) -> String;

// links.rs — Task 8
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkStyle { Html, Markdown }
pub fn rewrite(href: &str, style: LinkStyle) -> String;
pub fn rewrite_markdown_links(md: &str) -> String;   // every inline `](target)` via rewrite(_, Markdown)

// markdown.rs — Task 8
pub struct TocEntry { pub level: u8, pub number: String, pub id: String, pub text: String }
pub struct Section { pub number: String, pub id: String, pub heading: String, pub text: String }
pub struct Page { pub title: String, pub lead: String, pub body: String, pub toc: Vec<TocEntry>, pub sections: Vec<Section> }
pub fn render(md: &str, guide_number: Option<u32>, kw: &Keywords) -> Page;
pub fn slug(text: &str) -> String;

// specimen.rs — Task 9
pub struct Run {
    pub id: &'static str,       // "trust-open" | "trust-court" | "gate-q" | "gate-r"
    pub label: &'static str,    // "Trust, open eligibility" | "Trust, court selects I2" | "require-gate, query q" | "require-gate, query r"
    pub command: String,        // equivalent `fidryn run …` command line with repo-relative paths
    pub source_html: String,    // highlight::numbered_frame(Lang::Fr, …, caption = module path)
    pub case_html: String,      // highlight::code_frame(Lang::Json, case text, kw, Some(case path or "empty case record"))
    pub kind: String,           // outcome kind
    pub opinion: Vec<String>,   // fidryn_cli::opinion::sentences(&report)
}
pub fn runs(root: &Path, kw: &Keywords) -> anyhow::Result<Vec<Run>>;
pub fn excerpt(src: &str, starts: &[&str]) -> Vec<(usize, String)>;   // blocks whose first trimmed line starts with each prefix, with 1-based first line numbers

// pages.rs — Task 10
pub struct Assets { pub css: String, pub js: String }            // asset_version of site/assets/fidryn.css and fidryn.js
impl Assets { pub fn read(root: &Path) -> anyhow::Result<Self>; }
pub fn stamp(kind: &str) -> (&'static str, &'static str);         // ("det","Determinate") ("con","Contingent") ("sus","Suspended") ("nc","NormConflict") ("oc","OutsideCompetence") ("inc","Inconsistent"); unknown → ("", "Outcome")
pub fn guides_nav(current: Option<&str>) -> String;               // current = slug
pub fn landing(runs: &[specimen::Run], assets: &Assets) -> String;
pub fn doc(guide: &guides::Guide, page: &markdown::Page, assets: &Assets) -> String;
pub fn not_found(assets: &Assets) -> String;

// search.rs — Task 11
pub fn index(pages: &[(&guides::Guide, &markdown::Page)]) -> String;   // JSON, see C6
```

`seo::robots()` returns exactly (with a final newline):

```text
User-agent: *
Allow: /

Sitemap: https://fidryn.onlygass.dev/sitemap.xml

# LLM / agent maps
# https://fidryn.onlygass.dev/llms.txt
# https://fidryn.onlygass.dev/llms-full.txt
```

Final `build` (Task 11) produces exactly: `index.html`, `index.md`, `404.html`, `docs/{slug}.html` and `docs/{slug}.md` for all nine guides (slug `index` gives `docs/index.html`, `docs/index.md`), `search-index.json`, `sitemap.xml`, `robots.txt`, `llms.txt`, `llms-full.txt`.

`xtask/src/ci.rs` edits: Task 6 inserts its JS-test call on the line immediately before `    if workspace_has_benches(&metadata) {`; Task 12 inserts `    crate::site::check(&crate::workspace::workspace_root())?;` on the line immediately after `    run_schema_probes()?;`. Task 6 adds `pub fn run_js_tests() -> Result<()>` to `xtask/src/workspace.rs` (runs `node --test` with every `web/tests/*.test.js` path, sorted, from the workspace root; prints `node not found; skipping JS tests in web/tests` and returns Ok when `node --version` cannot be spawned).

### C5. Markup contract

Every page is `templates/base.html` with its slots filled. Slots: `title`, `description`, `head` (from `seo::head`), `css_v`, `js_v` (from `Assets`), `body_class` (`page-landing`, `page-doc`, `page-404`), `crumb` (empty on landing and 404; docs: `<p class="crumb"><a href="/docs/">Docs</a> <span aria-hidden="true">/</span> {title}</p>`), `nav_docs`, `nav_examples` (empty or ` aria-current="page"`: docs pages other than Examples set `nav_docs`; the Examples page sets `nav_examples`), `main`.

`xtask/src/site/templates/base.html` (exact):

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{{title}}</title>
<meta name="description" content="{{description}}">
{{head}}
<meta name="theme-color" content="#f7f2e8" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#15120e" media="(prefers-color-scheme: dark)">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="preload" href="/fonts/fraunces.woff2" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="/fonts/plex-sans.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/assets/fidryn.css?v={{css_v}}">
<script>document.documentElement.classList.add("js");try{var t=localStorage.getItem("fidryn-theme");if(t==="light"||t==="dark")document.documentElement.dataset.theme=t}catch(e){}</script>
<script src="/assets/fidryn.js?v={{js_v}}" defer></script>
</head>
<body class="{{body_class}}">
<a class="skip" href="#main">Skip to content</a>
<header class="site-head">
  <a class="brand" href="/"><span class="mono" aria-hidden="true">F</span><span class="wordmark">Fidryn</span></a>
  {{crumb}}
  <nav class="site-nav" aria-label="Site">
    <a href="/docs/"{{nav_docs}}>Docs</a>
    <a href="/docs/examples"{{nav_examples}}>Examples</a>
    <a href="https://github.com/BeeGass/fidryn">GitHub</a>
  </nav>
  <form class="search" action="/docs/" method="get" role="search">
    <label class="sr-only" for="site-search">Search the docs</label>
    <input id="site-search" name="q" type="search" placeholder="Search docs" autocomplete="off" spellcheck="false" role="combobox" aria-controls="search-results" aria-expanded="false" aria-autocomplete="list">
    <kbd aria-hidden="true">/</kbd>
    <ul id="search-results" class="search-results" role="listbox" aria-label="Search results" hidden></ul>
  </form>
  <button class="theme-toggle" type="button" data-theme-toggle aria-label="Switch color theme" hidden><span aria-hidden="true"></span></button>
  <button class="menu-btn" type="button" data-drawer-open aria-controls="drawer" aria-expanded="false" hidden>Menu</button>
</header>
<main id="main">
{{main}}
</main>
<div class="backdrop" data-drawer-close hidden></div>
<footer class="site-foot">
  <p>Fidryn v0.1 &middot; a research fixture, not legal advice &middot; Bryan Gass</p>
  <nav aria-label="Footer">
    <a href="/docs/">Docs</a>
    <a href="/llms.txt">llms.txt</a>
    <a href="https://github.com/BeeGass/fidryn">Source</a>
    <a href="https://onlygass.dev">onlygass.dev</a>
  </nav>
</footer>
</body>
</html>
```

Guides navigation (`pages::guides_nav`), on every page (it is the docs sidebar at 720px and up, and the phone drawer everywhere). Structure, with the overview first in Start, then groups in `GROUPS` order, then Implementers; the link to the current page carries `aria-current="page"`:

```html
<nav id="drawer" class="guides" aria-label="Guides">
  <div class="guides-head"><p class="label">Contents</p><button type="button" class="btn sec sm" data-drawer-close>Close</button></div>
  <div class="group">
    <p class="label">Start</p>
    <ol>
      <li><a href="/docs/"><span class="n"></span><span class="t">Overview</span><span class="d">Where to begin</span></a></li>
      <li><a href="/docs/getting-started" aria-current="page"><span class="n">&sect;1</span><span class="t">Getting started</span><span class="d">Check and run a tiny module</span></a></li>
    </ol>
  </div>
  <!-- Write, Run, Read results, Contribute, same shape -->
  <div class="group">
    <p class="label">Implementers</p>
    <ol>
      <li><a href="https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md"><span class="n" aria-hidden="true">&#8599;</span><span class="t">Architecture</span><span class="d">Pipeline and crate contract</span></a></li>
    </ol>
  </div>
</nav>
```

`xtask/src/site/templates/landing.html` (exact; slots `guides_nav`, `specimen`, `contents`):

```html
{{guides_nav}}
<section class="hero" aria-labelledby="hero-title">
  <div class="hero-top">
    <div>
      <p class="eyebrow">A programming language for legal instruments</p>
      <h1 id="hero-title">No false determinacy.</h1>
    </div>
    <div class="hero-side">
      <p class="lede">Precise where law is mechanical. Explicit where judgment enters. Incapable of hiding authority, discretion, or ambiguity inside a Boolean.</p>
      <div class="actions"><a class="btn pri" href="/docs/getting-started">Get started</a><a class="btn sec" href="/docs/">Read the docs</a></div>
      <div class="cmd"><span class="prompt" aria-hidden="true">$</span><code>cargo install --git https://github.com/BeeGass/fidryn --locked</code><button type="button" class="copy" data-copy hidden>Copy</button></div>
    </div>
  </div>
  {{specimen}}
</section>
<aside class="note" aria-label="Research fixture"><p><strong>Research fixture.</strong> Not legal advice, not an operative instrument, and not a complete statement of any jurisdiction&rsquo;s law.</p></aside>
<section class="band" aria-labelledby="what-it-is">
  <h2 id="what-it-is"><span class="mark">&sect; 2 &mdash;</span> What it is</h2>
  <div class="three">
    <div><h3>Instruments, not slogans</h3><p>Bounded slices of trusts, tax, and statute-shaped rules as checkable programs, with <code>outside_scope</code> named so omissions stay visible.</p></div>
    <div><h3>Honest outcomes</h3><p>Determinate only when invariant across every still-admissible resolution, or when a competent authority has already decided. Otherwise suspended, contingent, or another named kind.</p></div>
    <div><h3>A local mill</h3><p><code>fidryn ui</code> binds 127.0.0.1 only. It checks, runs, explores, and renders modules. It never files.</p></div>
  </div>
</section>
<section class="band" aria-labelledby="six-outcomes">
  <h2 id="six-outcomes"><span class="mark">&sect; 3 &mdash;</span> Six honest outcomes</h2>
  <p class="band-lede">A query answers with exactly one of six kinds, and every answer carries the declared <code>modelBoundary</code>. Each kind answers one question.</p>
  <ol class="legend">
    <li><p class="q">Is there one answer, invariant across every admissible completion, or already determined by a competent authority?</p><a class="stamp det" href="/docs/outcomes#determinate">Determinate</a></li>
    <li><p class="q">Do still-admissible answers disagree?</p><a class="stamp con" href="/docs/outcomes#contingent">Contingent</a></li>
    <li><p class="q">Does evaluation need a legal operation the case does not discharge, with no covering certificate?</p><a class="stamp sus" href="/docs/outcomes#suspended">Suspended</a></li>
    <li><p class="q">Do staged effects disagree with no single applicable doctrine?</p><a class="stamp nc" href="/docs/outcomes#normconflict">NormConflict</a></li>
    <li><p class="q">Does the case ask for something outside the declared model or the module&rsquo;s competence?</p><a class="stamp oc" href="/docs/outcomes#outsidecompetence">OutsideCompetence</a></li>
    <li><p class="q">Can the admitted model not be satisfied?</p><a class="stamp inc" href="/docs/outcomes#inconsistent">Inconsistent</a></li>
  </ol>
</section>
<section class="band" aria-labelledby="contents">
  <h2 id="contents"><span class="mark">&sect; 4 &mdash;</span> Contents</h2>
  {{contents}}
</section>
```

Landing contents (`{{contents}}`), numbered guides only, in `GUIDES` order:

```html
<ol class="contents">
  <li><a href="/docs/getting-started"><span class="n">&sect;1</span><span class="t">Getting started</span><span class="lead" aria-hidden="true"></span><span class="d">Check and run a tiny module</span></a></li>
</ol>
```

Specimen (`{{specimen}}`), runs in `specimen::runs` order; the first tab is selected; tabs after the first carry `tabindex="-1"`; `{kc}`/`{kl}` come from `pages::stamp(kind)`; the last opinion sentence gets `class="boundary"` when it starts with `Outside scope: `:

```html
<div class="specimen" data-specimen>
  <div class="spec-tabs" role="tablist" aria-label="Real runs">
    <button type="button" role="tab" id="tab-trust-open" aria-controls="run-trust-open" aria-selected="true">Trust, open eligibility <span class="stamp con">Contingent</span></button>
    <button type="button" role="tab" id="tab-trust-court" aria-controls="run-trust-court" aria-selected="false" tabindex="-1">Trust, court selects I2 <span class="stamp det">Determinate</span></button>
  </div>
  <div class="spec-panels">
    <section class="spec-panel" id="run-trust-open" role="tabpanel" aria-labelledby="tab-trust-open">
      <p class="spec-label">Trust, open eligibility</p>
      <p class="spec-cmd"><code>fidryn run examples/trust/bryan-revocable-trust.fr --query acting_trustee --case examples/trust/cases/two-certificates-open-eligibility.json --valid-at 2034-03-01T09:00:00Z --known-at 2034-03-01T09:00:00Z</code></p>
      <div class="steps">
        <div class="step step-source"><p class="step-h"><span class="step-n">1</span>Source</p>{source_html}</div>
        <div class="step step-case"><p class="step-h"><span class="step-n">2</span>Case</p>{case_html}</div>
        <div class="step step-outcome"><p class="step-h"><span class="step-n">3</span>Outcome</p><span class="stamp con">Contingent</span><ul class="opinion"><li>acting_trustee depends on SuccessorEligibility.</li><li class="boundary">Outside scope: tax, creditor_priority, real_property_recording, complete_Massachusetts_trust_law.</li></ul></div>
      </div>
    </section>
  </div>
  <div class="spec-dots" aria-hidden="true"><i class="on"></i><i></i><i></i><i></i></div>
  <p class="spec-foot">Computed by the Fidryn interpreter when this page was built.</p>
</div>
```

Code frames (`highlight::code_frame`); `{caption}` defaults to `lang.label()`; code has its final newline removed before highlighting; the copy button ships hidden:

```html
<figure class="code" data-lang="{lang.class()}"><figcaption><span>{caption}</span><button type="button" class="copy" data-copy hidden>Copy</button></figcaption><pre><code>{highlighted}</code></pre></figure>
```

Numbered frames (`highlight::numbered_frame`) use `<pre class="lines">`; each source line is `<span class="line" data-n="{n}">{highlighted line}</span>` followed by `\n` (the newline is outside the span); between two segments that are not adjacent, one gap line `<span class="line gap" data-n="">&hellip;</span>\n`.

Highlight token classes: `tk-kw` keyword, `tk-ty` type or module path or JSON key, `tk-st` string, `tk-nu` number, date, `true`, `false`, `null`, `tk-co` comment, `tk-pu` punctuation and operators. Everything else is plain escaped text. Stripping the `<span class="tk-…">` / `</span>` tags and unescaping gives the input exactly.

Docs headings (`markdown::render`): h1 becomes `Page.title` and is not in `body`; h2/h3 get `<span class="hn">6.1</span> ` (numbers only when the guide has a number; an h3 before any h2 gets no number); every heading h2–h6 gets an id and an anchor:

```html
<h2 id="envelope"><span class="hn">6.1</span> Envelope <a class="anchor" href="#envelope" aria-label="Link to this section">#</a></h2>
```

Tables: `<div class="table-wrap" tabindex="0" role="region" aria-label="Table">` + the table + `</div>`. Blockquotes stay `<blockquote>`. TOC entries (h2 and h3): `<li class="lvl-2"><a href="#envelope"><span class="n">6.1</span> Envelope</a></li>` (the `n` span is omitted when the number is empty).

`xtask/src/site/templates/doc.html` (exact; slots `guides_nav`, `kicker`, `title`, `onpage_inline`, `body`, `pager`, `edit_url`, `md_url`, `onpage`):

```html
<div class="doc-layout">
  {{guides_nav}}
  <article class="doc" aria-labelledby="doc-title">
    <p class="kicker">{{kicker}}</p>
    <h1 id="doc-title">{{title}}</h1>
    {{onpage_inline}}
    <div class="prose">
{{body}}
    </div>
    {{pager}}
    <p class="doc-meta"><a href="{{edit_url}}">Edit this page on GitHub</a><a href="{{md_url}}">View as Markdown</a></p>
    <p class="doc-disclaimer">The modules, records, and examples here are research fixtures. They are not legal advice, not operative instruments, and not a complete statement of any jurisdiction&rsquo;s law.</p>
  </article>
  {{onpage}}
</div>
```

`kicker`: `&sect; 6 &middot; Read results` for numbered guides, `Documentation` for the overview. `onpage_inline`: `<details class="onpage-inline"><summary>On this page</summary><ol>{toc}</ol></details>`; `onpage`: `<aside class="onpage" aria-label="On this page"><p class="label">On this page</p><ol>{toc}</ol></aside>`; both empty strings when the page has no TOC entries. `pager` (order: overview, then §1 … §8; omit the missing side; the example is the Outcomes page):

```html
<nav class="pager" aria-label="Previous and next guide"><a class="prev" href="/docs/mill"><span class="label">Previous &middot; &sect;5</span><span class="t">Mill</span></a><a class="next" href="/docs/examples"><span class="label">Next &middot; &sect;7</span><span class="t">Examples</span></a></nav>
```

(For the overview as a neighbor the label is `Previous` with no number.) `edit_url`: `https://github.com/BeeGass/fidryn/blob/main/docs/{file}`; `md_url`: `/docs/{slug}.md`.

`xtask/src/site/templates/404.html` (exact; slot `guides_nav`):

```html
{{guides_nav}}
<section class="notfound" aria-labelledby="nf-title">
  <p class="nf-mark" aria-hidden="true">&sect; 404</p>
  <p class="eyebrow">Not found</p>
  <h1 id="nf-title">No such provision.</h1>
  <p class="lede">This page is outside the declared model. Nothing was invented to fill the gap.</p>
  <div class="actions"><a class="btn pri" href="/">Back to the start</a><a class="btn sec" href="/docs/">Open the docs</a></div>
</section>
```

Titles: landing `Fidryn — a programming language for legal instruments`; docs `{title} — Fidryn`; 404 `Not found — Fidryn`. Landing description: the current one (`Fidryn (FID-rin) is a programming language for legal instruments: precise where law is mechanical, explicit where judgment enters, and incapable of hiding authority inside a Boolean. Research fixture — not legal advice.`). 404 description: `This page is outside the declared model.`

### C6. Search index (`site/search-index.json`)

A JSON array, compact, in guide order. Per guide: one page entry, then one entry per h2/h3 section:

```json
[{"u":"/docs/outcomes","n":"§6","h":"Outcomes","p":"Outcomes","t":"This guide explains the six honest results a Fidryn query can return."},
 {"u":"/docs/outcomes#outcome-kinds","n":"6.3","h":"Outcome kinds","p":"Outcomes","t":"outcome is tagged by kind. Every kind includes trace, …"}]
```

`u` URL, `n` number (`§6` for a page, `6.3` for a section, empty for the overview and its sections), `h` heading text, `p` page title, `t` the first 200 characters of the section's text (whitespace collapsed, cut at a character boundary, no trailing space).

### C7. CSS contract (`site/assets/fidryn.css`, Task 5)

Token blocks, verbatim (the contrast test parses the `:root {` block, the `:root[data-theme="dark"] {` block, and the `:root:not([data-theme="light"]) {` block inside the media query, and requires the two dark blocks to be identical):

```css
:root {
  color-scheme: light;
  --paper: #f7f2e8; --paper-2: #efe7d6; --paper-3: #e6dbc4;
  --ink: #1c1813; --ink-2: #3d362c; --ink-3: #6f6454; --rule: #d6c9b1; --rubric: #a3281d;
  --det: #2e6a39; --con: #23507f; --sus: #8a5d06; --nc: #9c3d10; --oc: #6b3a72; --inc: #b0182b;
  --tok-kw: #a3281d; --tok-type: #23507f; --tok-str: #3b6a2c; --tok-num: #86570a; --tok-com: #5e6652; --tok-punct: #6b6152;
  --grain-opacity: .06;
  --serif: "Fraunces", "Iowan Old Style", "Palatino Linotype", Georgia, serif;
  --sans: "IBM Plex Sans", system-ui, -apple-system, "Segoe UI", sans-serif;
  --mono: "IBM Plex Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  --r: 6px; --gutter: clamp(16px, 4vw, 40px); --measure: 42rem; --row: 30px;
}
:root[data-theme="dark"] {
  color-scheme: dark;
  --paper: #15120e; --paper-2: #1e1a15; --paper-3: #28221b;
  --ink: #efe6d6; --ink-2: #cfc3ae; --ink-3: #9a8e7a; --rule: #3a3228; --rubric: #e0725f;
  --det: #8cc79a; --con: #8fb4e8; --sus: #e2b457; --nc: #ec9a67; --oc: #cfa1d8; --inc: #f07b7b;
  --tok-kw: #ec8a74; --tok-type: #9dbde8; --tok-str: #a8cf95; --tok-num: #e2b86a; --tok-com: #9aa08a; --tok-punct: #a79a84;
  --grain-opacity: .03;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    color-scheme: dark;
    --paper: #15120e; --paper-2: #1e1a15; --paper-3: #28221b;
    --ink: #efe6d6; --ink-2: #cfc3ae; --ink-3: #9a8e7a; --rule: #3a3228; --rubric: #e0725f;
    --det: #8cc79a; --con: #8fb4e8; --sus: #e2b457; --nc: #ec9a67; --oc: #cfa1d8; --inc: #f07b7b;
    --tok-kw: #ec8a74; --tok-type: #9dbde8; --tok-str: #a8cf95; --tok-num: #e2b86a; --tok-com: #9aa08a; --tok-punct: #a79a84;
    --grain-opacity: .03;
  }
}
```

`@font-face` sources are root-relative: `/fonts/fraunces.woff2` (weight 100 900), `/fonts/plex-sans.woff2` (100 700), `/fonts/plex-mono-400.woff2` (400), `/fonts/plex-mono-500.woff2` (500), all `font-display: swap`. The grain is a `body::before` fixed layer whose `background-image` is an SVG `feTurbulence` data URI (percent-encode `#` as `%23`, use absolute width/height, no `%` characters), opacity `var(--grain-opacity)`, `pointer-events: none`, behind content.

Shared classes Task 5 must style (and that pages, templates, and the mill may use): `[hidden]{display:none!important}`, `.sr-only`, `.skip`, `.label` (Plex Sans 600, 12px, sentence case, `--ink-3`), `.eyebrow`, `.lede`, `.kicker`, `.site-head`, `.brand`, `.mono`, `.wordmark`, `.crumb`, `.site-nav` (and `[aria-current="page"]`), `.search` (input, `kbd`), `.search-results` (`li`, `a`, `.n`, `.h`, `.t`, `mark`, `li[aria-selected="true"]`, `.empty`), `.theme-toggle`, `.menu-btn`, `.btn` (`.pri`, `.sec`, `.sm`, `:disabled`, `[aria-busy="true"]`), `.actions`, `.cmd` (`.prompt`, `code`), `.copy` (`[data-copied]`), `.code` (`figcaption`, `pre`, `code`, `pre.lines .line`, `.line.gap`), `.tk-kw .tk-ty .tk-st .tk-nu .tk-co .tk-pu`, `.stamp` (`.det .con .sus .nc .oc .inc`; also on `a` and `button` content), `.table-wrap`, `table`, `th`, `td`, `blockquote`, `.hero`, `.hero-top`, `.hero-side`, `.note`, `.band`, `.band-lede`, `.mark`, `.three`, `.legend` (`.q`), `.contents` (`.n .t .lead .d`), `.specimen`, `.spec-tabs` (`[role="tab"]`, `[aria-selected="true"]`), `.spec-panels`, `.spec-panel`, `.spec-label` (shown only below 720px, as the card title), `.spec-cmd`, `.steps`, `.step`, `.step-h`, `.step-n`, `.opinion` (`li`, `.boundary`), `.spec-dots` (`i`, `.on`), `.spec-foot`, `.doc-layout`, `.guides` (`.guides-head`, `.group`, `ol`, `li a`, `.n`, `.t`, `.d`, `a[aria-current="page"]`), `.doc`, `.prose` (`h2`, `h3`, `.hn`, `.anchor`, `p`, `ul`, `ol`, `li`, `hr`, `code`), `.onpage` (`.lvl-2`, `.lvl-3`, `a[aria-current="true"]`, `.n`), `.onpage-inline` (`summary`), `.pager` (`.prev`, `.next`, `.label`, `.t`), `.doc-meta`, `.doc-disclaimer`, `.backdrop`, `body.drawer-open`, `.site-foot`, `.notfound`, `.nf-mark`. Drawer behavior below 720px is scoped under `html.js` so a page without JavaScript shows the guides list inline. On the landing page at 720px and up, `.guides` is not displayed.

The mill's own classes all start with `mill-` and live in `web/mill.css`.

### C8. JS contract

`site/assets/fidryn.js` (Task 6): one IIFE, `"use strict"`. Pure helpers first; then `if (typeof document === "undefined") { module.exports = { tokens, score, rank }; return; }`; then DOM wiring on `DOMContentLoaded`. Behaviors: theme toggles (`[data-theme-toggle]`: unhide, toggle `document.documentElement.dataset.theme` between `light` and `dark`, store in `localStorage` key `fidryn-theme` inside try/catch, label `Switch to dark theme` / `Switch to light theme`); copy buttons (`[data-copy]`: unhide; copy the `code` element's text inside the nearest `.code` or `.cmd`; show `Copied` for 1.4s with `data-copied`); heading anchors (`.anchor` click also copies the absolute URL); drawer (`[data-drawer-open]` unhidden; opens `#drawer` by adding `body.drawer-open`, unhiding `.backdrop`, setting `aria-expanded`; closes on `[data-drawer-close]`, Escape, and link clicks; traps Tab inside `#drawer`; returns focus to the Menu button); search (`#site-search`, `#search-results`; fetch `/search-index.json` on first focus; `tokens(q)` lowercases and splits on anything that is not `[a-z0-9_]`, dropping empties; `score(entry, qs)` needs every token to prefix-match a word of `h`, `p`, or `t`, weights heading 10, page 4, text 1, returns 0 when any token is unmatched; `rank(index, query)` returns up to 8 entries with score > 0, best first, stable for ties; results are `li[role=option]` with ids `sr-{i}`, `aria-selected`, and an `a` containing `.n`, `.h` with `<mark>` around matched prefixes built with DOM nodes, `.t` as `{p} · {t}`; ArrowUp/ArrowDown move, Enter opens, Escape closes, `/` or Ctrl/Cmd-K focuses the field when focus is not in a text field; clicking outside closes; the form's submit is prevented while JS runs); scrollspy (`.onpage a` gets `aria-current="true"` for the section being read, via IntersectionObserver with `rootMargin: "0px 0px -70% 0px"`); specimen (`[data-specimen]`: at 720px and up the tabs select one visible panel with ArrowLeft/ArrowRight/Home/End and roving `tabindex`; below 720px every panel is visible as a horizontal scroll-snap card and `.spec-dots i` follow the scroll position; re-applied when the media query changes).

`web/mill.js` (Tasks 15–16): one IIFE, `"use strict"`. Pure helpers exported for tests when `document` is undefined: `{ esc, isRfc3339, valueText, requestText, loadState, postJson, byteToIndex, lineCol, tokenizeFr, tokenizeJson, applyRanges, segmentsToHtml, editorKey, KEYWORDS }` (Task 15 provides the first six, Task 16 the rest, including `editorKey`, the pure helper behind the Escape-then-Tab rule; `module.exports` lists only what exists after each task). The theme code runs immediately at load (the script tag is in `<head>`, not deferred) and uses the same `fidryn-theme` key; everything else waits for `DOMContentLoaded`. Mill state lives in `localStorage` key `fidryn-mill` as `{"v":1,"module","case","template","query","validAt","knownAt","buffer","sample","view"}`; anything unreadable falls back to the first sample. The keyword list sits between the lines `// KEYWORDS-BEGIN` and `// KEYWORDS-END`, one JSON string per line, sorted, each followed by a comma.

Node tests: `web/tests/site.test.js` (Task 6) and `web/tests/mill.test.js` (Tasks 15–16) use `require("node:test")` and `require("node:assert/strict")`, and `require` the scripts by relative path (`../../site/assets/fidryn.js`, `../mill.js`). Run one file with `node --test web/tests/site.test.js`.

### C9. Mill page contract (`web/index.html`, Task 14)

Must contain: `<title>fidryn mill</title>`; a visually hidden `<h1 class="sr-only">fidryn mill</h1>`; the header text `localhost 127.0.0.1`; the sentence `Live filing is not available from the UI`; buttons whose text is exactly `Check`, `Run`, `Explore`, `Render` (so `>Run<`, `>Explore<`, `>Render<` appear); `<link rel="stylesheet" href="/assets/fidryn.css">`, `<link rel="stylesheet" href="/assets/mill.css">`, `<link rel="icon" href="/favicon.svg" type="image/svg+xml">`, and `<script src="/assets/mill.js"></script>` in `<head>`. No `style=` attribute, no inline `<script>` body, no `on…=` handler attributes. It reuses the shared classes `site-head`, `brand`, `mono`, `wordmark`, `label`, `btn`, `pri`, `sec`, `sm`, `actions`, `stamp`, `code`, `theme-toggle` (with `data-theme-toggle`), `skip`, `sr-only`. Element ids used by `mill.js`: `health`, `samples`, `confirm`, `confirm-replace`, `confirm-cancel`, `history`, `buffers` (the segmented control; its buttons carry `data-buffer="module|case|template"`, `role="tab"`, `aria-selected`), `buffer-status`, `editor-wrap`, `gutter`, `hl`, `editor` (the textarea), `diag-pop`, `query`, `query-names` (datalist), `validAt`, `validAt-err`, `knownAt`, `knownAt-err`, `check`, `run`, `explore`, `render`, `views` (buttons with `data-view="opinion|table|json"`), `result-body`.

---

## Tasks

### Task 1: Library entry point for `fidryn run`

**Files:**
- Create: `crates/fidryn-cli/tests/run_report_text.rs`
- Modify: `crates/fidryn-cli/src/lib.rs` (new public `CaseInput`, `RunRequest`, `run_report_text` after `compile_module`; the private `load_case` / `instant_or_exit` pair becomes `read_case` / `instant_arg` / `fail` plus two thin wrappers; `cmd_run` body)
- Test: `crates/fidryn-cli/tests/run_report_text.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks. Uses the existing `compile_module(path: &Path) -> Result<(CoreModule, SourceManifest), Vec<Diagnostic>>`, `apply_run_args(case: &mut CaseRecord, args: &[String]) -> Result<(), String>`, `parse_instant(text: &str) -> Result<Instant, TimeError>`, `EngineFailure::from_err(EngineError)`, `render_report` (C1), and the thread-local `DRIVER`.
- Produces (C2, used by Task 9's `specimen::runs`):
  - `#[derive(Clone, Copy, Debug)] pub enum CaseInput<'a> { File(&'a Path), Json(&'a str) }`
  - `#[derive(Clone, Copy, Debug)] pub struct RunRequest<'a> { pub path: &'a Path, pub query: &'a str, pub case: CaseInput<'a>, pub valid_at: &'a str, pub known_at: &'a str, pub args: &'a [String], pub scenario: bool }`
  - `pub fn run_report_text(req: &RunRequest<'_>) -> Result<String, String>`: Ok is the canonical `fidryn.evaluation-report/v0.1` text `fidryn run` prints (without the trailing newline); Err is exactly the text `fidryn run` writes to stderr (without the trailing newline). Compile and evaluation share the calling thread's driver, so `sourceTrust` matches the CLI (`fixture` for the trust module).
  - Private, in `lib.rs`: `read_case(input: CaseInput<'_>) -> Result<CaseRecord, String>`, `instant_arg(flag: &str, text: &str) -> Result<Instant, String>`, `fail(message: String) -> ExitCode`, `diagnostics_text(&[Diagnostic]) -> String`. `cmd_explore` keeps calling `load_case` and `instant_or_exit`, which now delegate to them, so every message comes from one place.

- [ ] **Step 1: Write the failing test**

Create `crates/fidryn-cli/tests/run_report_text.rs` with:

```rust
//! `run_report_text` is the `fidryn run` path as a library call: the same
//! report text on success and the same stderr text on failure.

use fidryn_cli::{CaseInput, RunRequest, compile_module, parse_instant, run_report_text};
use fidryn_core::CaseRecord;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

const EMPTY_CASE: &str = r#"{"schema":"fidryn.case-record/v0.1","admissibleCompletions":{}}"#;
const GATE_TIME: &str = "2026-09-17T12:00:00Z";
const TRUST_TIME: &str = "2034-03-01T09:00:00Z";
const TIME_HELP: &str =
    "Use ISO 8601 / RFC 3339, for example 2033-01-01T00:00:00Z or 2033-01-01T00:00:00+00:00.";

fn workspace_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// `fidryn run MODULE --query QUERY --case CASE --valid-at AT --known-at AT`.
fn request<'a>(
    module: &'a Path,
    query: &'a str,
    case: CaseInput<'a>,
    at: &'a str,
) -> RunRequest<'a> {
    RunRequest {
        path: module,
        query,
        case,
        valid_at: at,
        known_at: at,
        args: &[],
        scenario: false,
    }
}

fn report(text: &str) -> Value {
    let report: Value = serde_json::from_str(text).expect("report JSON");
    assert_eq!(report["schema"], "fidryn.evaluation-report/v0.1", "{text}");
    report
}

#[test]
fn require_gate_q_is_determinate_seven() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let text = run_report_text(&request(
        &module,
        "q",
        CaseInput::Json(EMPTY_CASE),
        GATE_TIME,
    ))
    .expect("run q");
    let report = report(&text);
    assert_eq!(report["outcomeDocument"]["query"], "q", "{text}");
    let outcome = &report["outcomeDocument"]["outcome"];
    assert_eq!(outcome["kind"], "determinate", "{text}");
    assert_eq!(
        outcome["value"],
        json!({"kind": "int", "data": 7}),
        "{text}"
    );
}

#[test]
fn require_gate_r_is_suspended_on_the_failed_require() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let text = run_report_text(&request(
        &module,
        "r",
        CaseInput::Json(EMPTY_CASE),
        GATE_TIME,
    ))
    .expect("run r");
    let outcome = &report(&text)["outcomeDocument"]["outcome"];
    assert_eq!(outcome["kind"], "suspended", "{text}");
    assert_eq!(
        outcome["requests"],
        json!([{"kind": "needCustom", "effect": "require", "payload": "requirement failed"}]),
        "{text}"
    );
}

#[test]
fn trust_case_file_court_selects_i2_is_determinate_bob() {
    let module = workspace_file("examples/trust/bryan-revocable-trust.fr");
    let case = workspace_file("examples/trust/cases/court-selects-i2.json");
    let text = run_report_text(&request(
        &module,
        "acting_trustee",
        CaseInput::File(&case),
        TRUST_TIME,
    ))
    .expect("run trust");
    let report = report(&text);
    assert_eq!(report["sourceTrust"], "fixture", "{text}");
    let outcome = &report["outcomeDocument"]["outcome"];
    assert_eq!(outcome["kind"], "determinate", "{text}");
    assert_eq!(
        outcome["value"],
        json!({"kind": "entity", "data": "Bob"}),
        "{text}"
    );
}

#[test]
fn bad_times_name_the_flag_and_the_accepted_forms() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let parse_err = parse_instant("yesterday").expect_err("not a timestamp");
    let mut req = request(&module, "q", CaseInput::Json(EMPTY_CASE), GATE_TIME);
    req.valid_at = "yesterday";
    assert_eq!(
        run_report_text(&req).expect_err("bad --valid-at"),
        format!("--valid-at: {parse_err}. {TIME_HELP}")
    );
    req.valid_at = GATE_TIME;
    req.known_at = "yesterday";
    assert_eq!(
        run_report_text(&req).expect_err("bad --known-at"),
        format!("--known-at: {parse_err}. {TIME_HELP}")
    );
}

#[test]
fn unknown_query_is_an_engine_failure() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let err = run_report_text(&request(
        &module,
        "no_such_query",
        CaseInput::Json(EMPTY_CASE),
        GATE_TIME,
    ))
    .expect_err("unknown query");
    assert_eq!(err, "UnknownQuery: unknown query no_such_query");
}

#[test]
fn unreadable_case_file_names_the_path() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let missing = workspace_file("examples/trust/cases/no-such-case.json");
    let io_err = std::fs::read_to_string(&missing).expect_err("case file is absent");
    let err = run_report_text(&request(&module, "q", CaseInput::File(&missing), GATE_TIME))
        .expect_err("missing case file");
    assert_eq!(err, format!("cannot read {}: {io_err}", missing.display()));
}

#[test]
fn invalid_case_json_is_an_invalid_case_record() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let bad = r#"{"schema": "#;
    let serde_err = serde_json::from_str::<CaseRecord>(bad).expect_err("truncated JSON");
    let err = run_report_text(&request(&module, "q", CaseInput::Json(bad), GATE_TIME))
        .expect_err("invalid case JSON");
    assert_eq!(err, format!("invalid case record: {serde_err}"));
}

#[test]
fn checks_run_in_the_cli_order() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let orphan = ["orphan".to_owned()];
    let all_bad = RunRequest {
        path: &module,
        query: "no_such_query",
        case: CaseInput::Json("not json"),
        valid_at: "never",
        known_at: "never",
        args: &orphan,
        scenario: false,
    };

    let missing_module = workspace_file("tests/programs/no-such-module.fr");
    let diagnostics = compile_module(&missing_module).expect_err("module is absent");
    let compile_text = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let err = run_report_text(&RunRequest {
        path: &missing_module,
        ..all_bad
    })
    .expect_err("compile fails first");
    assert_eq!(err, compile_text);

    let err = run_report_text(&all_bad).expect_err("case fails second");
    assert!(err.starts_with("invalid case record: "), "{err}");

    let err = run_report_text(&RunRequest {
        case: CaseInput::Json(EMPTY_CASE),
        ..all_bad
    })
    .expect_err("--arg fails third");
    assert_eq!(
        err,
        "--arg `orphan` must be KEY=VALUE (writes case.facts[KEY])"
    );

    let err = run_report_text(&RunRequest {
        case: CaseInput::Json(EMPTY_CASE),
        args: &[],
        ..all_bad
    })
    .expect_err("--valid-at fails fourth");
    assert!(err.starts_with("--valid-at: "), "{err}");

    let err = run_report_text(&RunRequest {
        case: CaseInput::Json(EMPTY_CASE),
        args: &[],
        valid_at: GATE_TIME,
        ..all_bad
    })
    .expect_err("--known-at fails fifth");
    assert!(err.starts_with("--known-at: "), "{err}");

    let err = run_report_text(&RunRequest {
        case: CaseInput::Json(EMPTY_CASE),
        args: &[],
        valid_at: GATE_TIME,
        known_at: GATE_TIME,
        ..all_bad
    })
    .expect_err("evaluation fails last");
    assert!(err.starts_with("UnknownQuery: "), "{err}");
}

#[test]
fn fidryn_run_prints_the_same_text() {
    let module = workspace_file("examples/trust/bryan-revocable-trust.fr");
    let case = workspace_file("examples/trust/cases/court-selects-i2.json");
    let fidryn_run = |valid_at: &str| {
        Command::new(env!("CARGO_BIN_EXE_fidryn"))
            .arg("run")
            .arg(&module)
            .args(["--query", "acting_trustee", "--case"])
            .arg(&case)
            .args(["--valid-at", valid_at, "--known-at", TRUST_TIME])
            .output()
            .expect("spawn fidryn")
    };
    let req = request(
        &module,
        "acting_trustee",
        CaseInput::File(&case),
        TRUST_TIME,
    );

    let text = run_report_text(&req).expect("library run");
    let out = fidryn_run(TRUST_TIME);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(String::from_utf8(out.stdout).unwrap(), format!("{text}\n"));

    let err = run_report_text(&RunRequest {
        valid_at: "yesterday",
        ..req
    })
    .expect_err("library failure");
    let out = fidryn_run("yesterday");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert_eq!(String::from_utf8(out.stderr).unwrap(), format!("{err}\n"));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p fidryn-cli --offline --test run_report_text`
Expected: FAIL to compile with ``error[E0432]: unresolved imports `fidryn_cli::CaseInput`, `fidryn_cli::RunRequest`, `fidryn_cli::run_report_text` ``.

- [ ] **Step 3: Write the implementation**

Add the public types and `run_report_text` right after `compile_module`.

In `crates/fidryn-cli/src/lib.rs`, find:

```rust
pub fn compile_module(path: &Path) -> Result<(CoreModule, SourceManifest), Vec<Diagnostic>> {
    DRIVER.with(|driver| driver.borrow_mut().check_path(path))
}
```

Replace it with:

```rust
pub fn compile_module(path: &Path) -> Result<(CoreModule, SourceManifest), Vec<Diagnostic>> {
    DRIVER.with(|driver| driver.borrow_mut().check_path(path))
}

/// Where `fidryn run` reads its case record from.
#[derive(Clone, Copy, Debug)]
pub enum CaseInput<'a> {
    /// A case record file, as passed to `--case`.
    File(&'a Path),
    /// Case record JSON text already in memory.
    Json(&'a str),
}

/// One `fidryn run` invocation, field for field with its flags.
#[derive(Clone, Copy, Debug)]
pub struct RunRequest<'a> {
    /// The `.fr` module path.
    pub path: &'a Path,
    /// `--query`.
    pub query: &'a str,
    /// `--case`, or case JSON text.
    pub case: CaseInput<'a>,
    /// `--valid-at`, ISO 8601 / RFC 3339.
    pub valid_at: &'a str,
    /// `--known-at`, ISO 8601 / RFC 3339.
    pub known_at: &'a str,
    /// `--arg KEY=VALUE` bindings.
    pub args: &'a [String],
    /// `--scenario`.
    pub scenario: bool,
}

/// Evaluate exactly as `fidryn run` does; Ok is the canonical report JSON
/// text the CLI prints, Err is the text the CLI writes to stderr.
///
/// Checks run in the CLI's order: compile, case, `--arg`, `--valid-at`,
/// `--known-at`, then evaluation. Compile and evaluation share this
/// thread's driver, so `sourceTrust` is the one the CLI reports.
pub fn run_report_text(req: &RunRequest<'_>) -> Result<String, String> {
    let (module, _) = compile_module(req.path).map_err(|ds| diagnostics_text(&ds))?;
    let mut case = read_case(req.case)?;
    apply_run_args(&mut case, req.args)?;
    let valid = instant_arg("--valid-at", req.valid_at)?;
    let known = instant_arg("--known-at", req.known_at)?;
    let ctx = RunContext::new(valid, known);
    let report = DRIVER
        .with(|driver| {
            let mut driver = driver.borrow_mut();
            if req.scenario {
                driver.run_report_scenario(&module, req.query, &case, &ctx)
            } else {
                driver.run_report(&module, req.query, &case, &ctx)
            }
        })
        .map_err(|err| EngineFailure::from_err(err).to_string())?;
    Ok(render_report(
        &module,
        &QueryName::from(req.query),
        valid,
        known,
        &case,
        &report,
    ))
}

/// Compile diagnostics as the CLI prints them, one per line.
fn diagnostics_text(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
```

Move the case and time messages into `Result<_, String>` helpers that both `run_report_text` and the existing `ExitCode` wrappers (still used by `cmd_explore`) share.

In `crates/fidryn-cli/src/lib.rs`, find:

```rust
fn load_case(path: &Path) -> Result<CaseRecord, ExitCode> {
    let text = fs::read_to_string(path).map_err(|e| {
        eprintln!("cannot read {}: {e}", path.display());
        ExitCode::from(1)
    })?;
    serde_json::from_str(&text).map_err(|e| {
        eprintln!("invalid case record: {e}");
        ExitCode::from(1)
    })
}

fn instant_or_exit(flag: &str, text: &str) -> Result<Instant, ExitCode> {
    parse_instant(text).map_err(|err| {
        eprintln!(
            "{flag}: {err}. Use ISO 8601 / RFC 3339, for example 2033-01-01T00:00:00Z or 2033-01-01T00:00:00+00:00."
        );
        ExitCode::from(1)
    })
}
```

Replace it with:

```rust
/// Read and parse a case record. Err is the CLI's stderr line.
fn read_case(input: CaseInput<'_>) -> Result<CaseRecord, String> {
    let owned;
    let text = match input {
        CaseInput::File(path) => {
            owned = fs::read_to_string(path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            owned.as_str()
        }
        CaseInput::Json(text) => text,
    };
    serde_json::from_str(text).map_err(|e| format!("invalid case record: {e}"))
}

/// Parse a `--valid-at` / `--known-at` value. Err is the CLI's stderr line.
fn instant_arg(flag: &str, text: &str) -> Result<Instant, String> {
    parse_instant(text).map_err(|err| {
        format!(
            "{flag}: {err}. Use ISO 8601 / RFC 3339, for example 2033-01-01T00:00:00Z or 2033-01-01T00:00:00+00:00."
        )
    })
}

/// Write `message` to stderr and return the failure exit code.
fn fail(message: String) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(1)
}

fn load_case(path: &Path) -> Result<CaseRecord, ExitCode> {
    read_case(CaseInput::File(path)).map_err(fail)
}

fn instant_or_exit(flag: &str, text: &str) -> Result<Instant, ExitCode> {
    instant_arg(flag, text).map_err(fail)
}
```

Make `cmd_run` delegate. The dispatcher in `run` is unchanged.

In `crates/fidryn-cli/src/lib.rs`, find:

```rust
fn cmd_run(
    path: &Path,
    query: &str,
    case_path: &Path,
    valid_at: &str,
    known_at: &str,
    args: &[String],
    scenario: bool,
) -> ExitCode {
    let Ok((module, _)) = compile_or_exit(path) else {
        return ExitCode::from(1);
    };
    let Ok(mut case) = load_case(case_path) else {
        return ExitCode::from(1);
    };
    if let Err(err) = apply_run_args(&mut case, args) {
        eprintln!("{err}");
        return ExitCode::from(1);
    }
    let Ok(valid) = instant_or_exit("--valid-at", valid_at) else {
        return ExitCode::from(1);
    };
    let Ok(known) = instant_or_exit("--known-at", known_at) else {
        return ExitCode::from(1);
    };
    let ctx = RunContext::new(valid, known);
    let report = match DRIVER.with(|driver| {
        let mut driver = driver.borrow_mut();
        if scenario {
            driver.run_report_scenario(&module, query, &case, &ctx)
        } else {
            driver.run_report(&module, query, &case, &ctx)
        }
    }) {
        Ok(report) => report,
        Err(err) => {
            eprintln!("{}", EngineFailure::from_err(err));
            return ExitCode::from(1);
        }
    };
    println!(
        "{}",
        render_report(
            &module,
            &QueryName::from(query),
            valid,
            known,
            &case,
            &report
        )
    );
    ExitCode::SUCCESS
}
```

Replace it with:

```rust
fn cmd_run(
    path: &Path,
    query: &str,
    case_path: &Path,
    valid_at: &str,
    known_at: &str,
    args: &[String],
    scenario: bool,
) -> ExitCode {
    let req = RunRequest {
        path,
        query,
        case: CaseInput::File(case_path),
        valid_at,
        known_at,
        args,
        scenario,
    };
    match run_report_text(&req) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(message) => fail(message),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p fidryn-cli --offline --test run_report_text`
Expected: PASS (`test result: ok. 9 passed; 0 failed`)

Run: `cargo test -p fidryn-cli --offline`
Expected: PASS; the suites report `46 passed` (lib), `0 passed` (main), `13 passed` (`cli_smoke`), `9 passed` (`run_report_text`), `0 passed` (doc-tests).

Run: `cargo clippy -p fidryn-cli --offline --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: `Finished` with no warnings, and no output from `cargo fmt`.

`fidryn_run_prints_the_same_text` pins that the binary prints exactly `run_report_text`'s text on stdout (success) and stderr (failure). The stderr lines themselves are the ones `cmd_run` printed before this change, byte for byte, in the same check order.

- [ ] **Step 5: Commit**

```bash
git add crates/fidryn-cli/src/lib.rs crates/fidryn-cli/tests/run_report_text.rs
git -c commit.gpgsign=false commit -m "feat(cli): expose the fidryn run path as run_report_text"
```

### Task 2: Opinion sentences

**Files:**
- Create: `crates/fidryn-cli/src/opinion.rs`
- Modify: `crates/fidryn-cli/src/lib.rs` (add `pub mod opinion;` above `pub mod ui;`)
- Test: `crates/fidryn-cli/src/opinion.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: nothing from earlier tasks (`serde_json` is already a dependency of `fidryn-cli`).
- Produces (C2):
  - `pub fn sentences(report: &serde_json::Value) -> Vec<String>`: `report` is a `fidryn.evaluation-report/v0.1` document; the sentences follow the C2 table exactly, with `Outside scope: …` last when `modelBoundary.outsideScope` is nonempty. Used by Task 3 (mill transport) and Task 9 (landing specimen).
  - `pub fn value_text(value: &serde_json::Value) -> String` and `pub fn request_text(request: &serde_json::Value) -> String`, per C2. Task 15's `valueText` / `requestText` in `web/mill.js` should produce the same strings for the same inputs (the unit tests below are a ready list of cases).
  - Where C2 is silent, the module chooses: a report without `outcomeDocument.query` uses `The query` for `{q}`; `outsideCompetence` with an empty or missing `reason` says `{q} is outside the module's competence.`, and `Request: …` is only added when `request` is present; a `ctor` whose `name` is not a string, or a `set` / `map` whose data has the wrong JSON type, is an unknown shape (compact JSON); positional `ctor` fields print in numeric order (`_2` before `_10`); `duration` data (an object `{"amount", "kind"}`) prints as its compact JSON, which is "the data as text".

- [ ] **Step 1: Write the failing test**

Create `crates/fidryn-cli/src/opinion.rs` with the module header and the tests only:

```rust
//! Plain sentences for an evaluation report, shared by the mill and the site.
//!
//! Every sentence is a fixed template filled from report fields. Nothing is
//! inferred beyond the data: a field the templates do not name is left out,
//! and an outcome kind without a template gets one sentence naming the kind.

use serde_json::{Map, Value};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const TRUST_SCOPE: [&str; 4] = [
        "tax",
        "creditor_priority",
        "real_property_recording",
        "complete_Massachusetts_trust_law",
    ];

    /// An evaluation report for `acting_trustee` with `outcome` and the
    /// given `outsideScope`.
    fn report_with(outcome: Value, outside_scope: &[&str]) -> Value {
        json!({
            "schema": "fidryn.evaluation-report/v0.1",
            "executionMode": "operative",
            "sourceTrust": "unauthenticated",
            "verificationMethod": "none",
            "assumptions": [],
            "coverage": null,
            "outcomeDocument": {
                "schema": "fidryn.outcome/v0.1",
                "module": "Examples.BryanRevocableTrust@0.1.0",
                "sourceSnapshot": "2026-08-23-ma-trust-fixture",
                "query": "acting_trustee",
                "asOf": {
                    "validTime": "2034-03-01T09:00:00Z",
                    "recordTime": "2034-03-01T09:00:00Z"
                },
                "modelBoundary": {
                    "outsideScope": outside_scope,
                    "admissibleCompletions": {}
                },
                "outcome": outcome
            }
        })
    }

    fn report(outcome: Value) -> Value {
        report_with(outcome, &[])
    }

    fn entity(name: &str) -> Value {
        json!({"kind": "entity", "data": name})
    }

    #[test]
    fn determinate_names_the_value() {
        let outcome = json!({
            "kind": "determinate",
            "value": entity("Bob"),
            "trace": "0".repeat(32),
            "ignoredOpenIssues": []
        });
        assert_eq!(sentences(&report(outcome)), ["acting_trustee is Bob."]);
    }

    #[test]
    fn determinate_with_a_null_certificate_adds_nothing() {
        let outcome = json!({
            "kind": "determinate",
            "value": {"kind": "int", "data": 7},
            "trace": "0".repeat(32),
            "convergenceCertificate": null,
            "ignoredOpenIssues": []
        });
        assert_eq!(sentences(&report(outcome)), ["acting_trustee is 7."]);
    }

    #[test]
    fn determinate_certificate_and_one_ignored_issue() {
        let outcome = json!({
            "kind": "determinate",
            "value": entity("Bob"),
            "trace": "0".repeat(32),
            "convergenceCertificate": "ab".repeat(16),
            "ignoredOpenIssues": [
                {"kind": "needInterpretation", "source": "Instrument", "family": "SuccessorEligibility"}
            ]
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee is Bob.".to_owned(),
                format!("Covered by convergence certificate {}.", "ab".repeat(16)),
                "1 open issue was set aside under that certificate.".to_owned(),
            ]
        );
    }

    #[test]
    fn determinate_certificate_and_several_ignored_issues() {
        let outcome = json!({
            "kind": "determinate",
            "value": entity("Bob"),
            "trace": "0".repeat(32),
            "convergenceCertificate": "cd".repeat(16),
            "ignoredOpenIssues": [
                {"kind": "needInterpretation", "source": "Instrument", "family": "SuccessorEligibility"},
                {"kind": "needEvidence", "issue": {}, "schema": "SecondConcurringCertificate"}
            ]
        });
        assert_eq!(
            sentences(&report(outcome))[2],
            "2 open issues were set aside under that certificate."
        );
    }

    #[test]
    fn contingent_with_run_keys() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {"I2": entity("Bob"), "I1": entity("Alice")},
            "pivots": [
                {"kind": "needInterpretation", "source": "Instrument.clause(\"4.4\")", "family": "SuccessorEligibility"}
            ],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report_with(outcome, &TRUST_SCOPE)),
            [
                "acting_trustee depends on SuccessorEligibility.",
                "Under I1 it is Alice.",
                "Under I2 it is Bob.",
                "No single answer is determinate across the admissible completions.",
                "Outside scope: tax, creditor_priority, real_property_recording, complete_Massachusetts_trust_law.",
            ]
        );
    }

    #[test]
    fn contingent_with_explore_keys() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {
                "i:SuccessorEligibility=I1": entity("Alice"),
                "i:SuccessorEligibility=I2": entity("Bob")
            },
            "pivots": [{"kind": "needInterpretation", "source": "Instrument", "family": "SuccessorEligibility"}],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee depends on SuccessorEligibility.",
                "Under SuccessorEligibility = I1 it is Alice.",
                "Under SuccessorEligibility = I2 it is Bob.",
                "No single answer is determinate across the admissible completions.",
            ]
        );
    }

    #[test]
    fn contingent_with_multi_binding_explore_keys() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {
                "i:SuccessorEligibility=I1,e:SecondConcurringCertificate=absent": entity("Alice")
            },
            "pivots": [],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "Under SuccessorEligibility = I1, SecondConcurringCertificate = absent it is Alice.",
                "No single answer is determinate across the admissible completions.",
            ]
        );
    }

    #[test]
    fn contingent_pivot_names_fall_back_and_drop_duplicates() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {},
            "pivots": [
                {"kind": "needInterpretation", "source": "A", "family": "SuccessorEligibility"},
                {"kind": "needJudgment", "issue": {}, "protocol": "CourtCapacityDetermination"},
                {"kind": "needInterpretation", "source": "B", "family": "SuccessorEligibility"},
                {"kind": "needEvidence", "issue": {}}
            ],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome))[0],
            "acting_trustee depends on SuccessorEligibility and CourtCapacityDetermination and needEvidence."
        );
    }

    #[test]
    fn contingent_without_pivots_skips_the_dependency_sentence() {
        let outcome = json!({
            "kind": "contingent",
            "alternatives": {"I1": entity("Alice")},
            "pivots": [],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "Under I1 it is Alice.",
                "No single answer is determinate across the admissible completions.",
            ]
        );
    }

    #[test]
    fn suspended_with_one_request() {
        let outcome = json!({
            "kind": "suspended",
            "requests": [{"kind": "needCustom", "effect": "require", "payload": "requirement failed"}],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee is suspended.",
                "Outstanding request: requirement failed (require).",
            ]
        );
    }

    #[test]
    fn suspended_with_several_requests() {
        let outcome = json!({
            "kind": "suspended",
            "requests": [
                {"kind": "needEvidence", "issue": {}, "schema": "PhysicianCertificate"},
                {"kind": "needJudgment", "issue": {}, "protocol": "CourtCapacityDetermination"}
            ],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee is suspended.",
                "Outstanding requests: evidence matching PhysicianCertificate; a determination under CourtCapacityDetermination.",
            ]
        );
    }

    #[test]
    fn norm_conflict_names_the_doctrines() {
        let outcome = json!({
            "kind": "normConflict",
            "doctrines": [{"name": "LexSpecialis"}, {"name": "LexPosterior"}],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee ended in a norm conflict: no single applicable doctrine among LexSpecialis, LexPosterior."
            ]
        );
        let bare = json!({"kind": "normConflict", "doctrines": [], "trace": "0".repeat(32)});
        assert_eq!(
            sentences(&report(bare)),
            ["acting_trustee ended in a norm conflict."]
        );
    }

    #[test]
    fn outside_competence_gives_reason_and_request() {
        let outcome = json!({
            "kind": "outsideCompetence",
            "request": {"kind": "needApplicableLaw", "issue": "situs", "candidates": ["Massachusetts", "New York"]},
            "reason": "the situs of the land is not modeled",
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee is outside the module's competence: the situs of the land is not modeled.",
                "Request: applicable law among Massachusetts, New York.",
            ]
        );
    }

    #[test]
    fn inconsistent_lists_the_core() {
        let outcome = json!({
            "kind": "inconsistent",
            "core": ["Alive(Bryan)", "Dead(Bryan)"],
            "trace": "0".repeat(32)
        });
        assert_eq!(
            sentences(&report(outcome)),
            [
                "acting_trustee has no consistent answer: the admitted model cannot be satisfied.",
                "Unsatisfiable core: Alive(Bryan), Dead(Bryan).",
            ]
        );
        let bare = json!({"kind": "inconsistent", "core": [], "trace": "0".repeat(32)});
        assert_eq!(
            sentences(&report(bare)),
            ["acting_trustee has no consistent answer: the admitted model cannot be satisfied."]
        );
    }

    #[test]
    fn unknown_kind_is_named_and_nothing_more() {
        let outcome = json!({"kind": "vacated", "order": "remand", "trace": "0".repeat(32)});
        assert_eq!(
            sentences(&report(outcome)),
            ["acting_trustee returned an outcome of kind vacated."]
        );
    }

    #[test]
    fn report_without_an_outcome() {
        assert_eq!(
            sentences(&report(Value::Null)),
            ["The report has no outcome."]
        );
        assert_eq!(sentences(&json!({})), ["The report has no outcome."]);
        assert_eq!(
            sentences(&report_with(Value::Null, &["tax"])),
            ["The report has no outcome.", "Outside scope: tax."]
        );
    }

    #[test]
    fn outside_scope_is_last_only_when_nonempty() {
        let outcome = json!({
            "kind": "determinate",
            "value": {"kind": "int", "data": 7},
            "trace": "0".repeat(32),
            "ignoredOpenIssues": []
        });
        assert_eq!(
            sentences(&report_with(outcome.clone(), &["complete_instruments"])),
            [
                "acting_trustee is 7.",
                "Outside scope: complete_instruments.",
            ]
        );
        assert_eq!(sentences(&report(outcome)), ["acting_trustee is 7."]);
    }

    #[test]
    fn value_text_covers_every_value_kind() {
        let cases = [
            (json!({"kind": "int", "data": 7}), "7"),
            (json!({"kind": "decimal", "data": "100.00"}), "100.00"),
            (json!({"kind": "bool", "data": true}), "true"),
            (
                json!({"kind": "instant", "data": "2034-03-01T09:00:00Z"}),
                "2034-03-01T09:00:00Z",
            ),
            (
                json!({"kind": "duration", "data": {"amount": 30, "kind": "counted_days"}}),
                r#"{"amount":30,"kind":"counted_days"}"#,
            ),
            (json!({"kind": "string", "data": "Alice"}), "\"Alice\""),
            (json!({"kind": "entity", "data": "Alice"}), "Alice"),
            (json!({"kind": "unit"}), "unit"),
            (
                json!({"kind": "ctor", "data": {"name": "Performed", "fields": {}}}),
                "Performed",
            ),
            (
                json!({"kind": "ctor", "data": {"name": "USD", "fields": {"_0": {"kind": "decimal", "data": "100.00"}}}}),
                "USD(100.00)",
            ),
            (
                json!({"kind": "ctor", "data": {"name": "Payment", "fields": {
                    "payer": entity("Alice"),
                    "amount": {"kind": "decimal", "data": "100.00"}
                }}}),
                "Payment(amount: 100.00, payer: Alice)",
            ),
            (
                json!({"kind": "set", "data": [entity("Alice"), entity("Bob")]}),
                "{Alice, Bob}",
            ),
            (json!({"kind": "set", "data": []}), "{}"),
            (
                json!({"kind": "map", "data": {"status": {"kind": "string", "data": "Due"}, "amount": {"kind": "int", "data": 3}}}),
                "{amount: 3, status: \"Due\"}",
            ),
            (json!({"kind": "option", "data": null}), "none"),
            (json!({"kind": "option", "data": entity("Bob")}), "Bob"),
        ];
        for (value, expected) in cases {
            assert_eq!(value_text(&value), expected, "{value}");
        }
    }

    #[test]
    fn value_text_orders_positional_fields_by_number() {
        let fields: Map<String, Value> = (0..11)
            .map(|i| (format!("_{i}"), json!({"kind": "int", "data": i})))
            .collect();
        let value = json!({"kind": "ctor", "data": {"name": "Row", "fields": fields}});
        assert_eq!(value_text(&value), "Row(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10)");
    }

    #[test]
    fn value_text_shows_unknown_shapes_as_compact_json() {
        let cases = [
            json!({"kind": "prop", "data": {"predicate": "Alive", "arguments": []}}),
            json!({"kind": "clauseRef", "data": {"module": "M", "clause": "4.4", "arguments": [], "digest": "00"}}),
            json!({"kind": "tensor", "data": [1, 2]}),
            json!({"kind": "set", "data": "not a list"}),
            json!({"kind": "ctor", "data": {"fields": {}}}),
            json!({"name": "no kind"}),
            json!(7),
            json!("bare"),
            Value::Null,
        ];
        for value in cases {
            assert_eq!(value_text(&value), value.to_string(), "{value}");
        }
    }

    #[test]
    fn request_text_covers_every_request_kind() {
        let cases = [
            (
                json!({"kind": "needCustom", "effect": "require", "payload": "requirement failed"}),
                "requirement failed (require)",
            ),
            (
                json!({"kind": "needCustom", "effect": "require", "payload": {"code": 3}}),
                "require",
            ),
            (
                json!({"kind": "needEvidence", "issue": {}, "schema": "PaymentRecord"}),
                "evidence matching PaymentRecord",
            ),
            (
                json!({"kind": "needInterpretation", "source": "Instrument", "family": "SuccessorEligibility"}),
                "an interpretation of SuccessorEligibility under Instrument",
            ),
            (
                json!({"kind": "needInterpretation", "family": "SuccessorEligibility"}),
                "an interpretation of SuccessorEligibility",
            ),
            (
                json!({"kind": "needJudgment", "issue": {}, "protocol": "CourtCapacityDetermination"}),
                "a determination under CourtCapacityDetermination",
            ),
            (
                json!({"kind": "needChoice", "protocol": "TrusteeDistributionDecision", "options": ["pay", "hold"]}),
                "a decision under TrusteeDistributionDecision among pay, hold",
            ),
            (
                json!({"kind": "needChoice", "protocol": "TrusteeDistributionDecision", "options": [1, 2]}),
                "a decision under TrusteeDistributionDecision",
            ),
            (
                json!({"kind": "needApplicableLaw", "issue": "situs", "candidates": ["Massachusetts", "New York"]}),
                "applicable law among Massachusetts, New York",
            ),
            (
                json!({"kind": "needApplicableLaw", "issue": "situs", "candidates": []}),
                "applicable law",
            ),
            (
                json!({"kind": "needConflict", "graph": [], "doctrines": ["LexSpecialis", "LexPosterior"]}),
                "one applicable conflict doctrine among LexSpecialis, LexPosterior",
            ),
            (
                json!({"kind": "needConflict", "graph": [], "doctrines": []}),
                "one applicable conflict doctrine",
            ),
            (json!({"kind": "needOracle", "question": "?"}), "needOracle"),
            (json!({"issue": "no kind"}), r#"{"issue":"no kind"}"#),
        ];
        for (request, expected) in cases {
            assert_eq!(request_text(&request), expected, "{request}");
        }
    }
}
```

Register the module:

In `crates/fidryn-cli/src/lib.rs`, find:

```rust
pub mod ui;
```

Replace it with:

```rust
pub mod opinion;
pub mod ui;
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p fidryn-cli --offline --lib opinion::tests`
Expected: FAIL to compile with ``error[E0425]: cannot find function `sentences` in this scope`` (also `value_text` and `request_text`), ending ``error: could not compile `fidryn-cli` (lib test) due to 25 previous errors``.

- [ ] **Step 3: Write the implementation**

Insert the implementation between the `use` line and the tests.

In `crates/fidryn-cli/src/opinion.rs`, find:

```rust
use serde_json::{Map, Value};

#[cfg(test)]
```

Replace it with:

```rust
use serde_json::{Map, Value};

/// Sentences describing `report`, a `fidryn.evaluation-report/v0.1` document.
///
/// The outcome's sentences come first; `Outside scope: …` is always last
/// when the model boundary names anything.
pub fn sentences(report: &Value) -> Vec<String> {
    let doc = &report["outcomeDocument"];
    let query = doc["query"].as_str().unwrap_or("The query");
    let mut out = if doc["outcome"].is_object() {
        outcome_sentences(query, &doc["outcome"])
    } else {
        vec!["The report has no outcome.".to_owned()]
    };
    let outside = texts(&doc["modelBoundary"]["outsideScope"]);
    if !outside.is_empty() {
        out.push(format!("Outside scope: {}.", outside.join(", ")));
    }
    out
}

/// A runtime value (`{"kind", "data"}`) as short plain text. Shapes this
/// function does not know are shown as compact JSON.
pub fn value_text(value: &Value) -> String {
    tagged_value_text(value).unwrap_or_else(|| value.to_string())
}

/// An open request (`{"kind", …}`) as a noun phrase, for example
/// `evidence matching PaymentRecord`. A request without a kind is shown as
/// compact JSON.
pub fn request_text(request: &Value) -> String {
    let Some(kind) = request["kind"].as_str() else {
        return request.to_string();
    };
    match kind {
        "needCustom" => {
            let effect = text(&request["effect"]);
            match request["payload"].as_str() {
                Some(payload) => format!("{payload} ({effect})"),
                None => effect,
            }
        }
        "needEvidence" => format!("evidence matching {}", text(&request["schema"])),
        "needInterpretation" => {
            let family = text(&request["family"]);
            match request["source"].as_str() {
                Some(source) if !source.is_empty() => {
                    format!("an interpretation of {family} under {source}")
                }
                _ => format!("an interpretation of {family}"),
            }
        }
        "needJudgment" => format!("a determination under {}", text(&request["protocol"])),
        "needChoice" => format!(
            "a decision under {}{}",
            text(&request["protocol"]),
            among(&request["options"])
        ),
        "needApplicableLaw" => format!("applicable law{}", among(&request["candidates"])),
        "needConflict" => {
            let names = names(&request["doctrines"]);
            if names.is_empty() {
                "one applicable conflict doctrine".to_owned()
            } else {
                format!(
                    "one applicable conflict doctrine among {}",
                    names.join(", ")
                )
            }
        }
        other => other.to_owned(),
    }
}

fn outcome_sentences(query: &str, outcome: &Value) -> Vec<String> {
    match outcome["kind"].as_str() {
        Some("determinate") => determinate(query, outcome),
        Some("contingent") => contingent(query, outcome),
        Some("suspended") => suspended(query, outcome),
        Some("normConflict") => norm_conflict(query, outcome),
        Some("outsideCompetence") => outside_competence(query, outcome),
        Some("inconsistent") => inconsistent(query, outcome),
        _ => vec![format!(
            "{query} returned an outcome of kind {}.",
            text(&outcome["kind"])
        )],
    }
}

fn determinate(query: &str, outcome: &Value) -> Vec<String> {
    let mut out = vec![format!("{query} is {}.", value_text(&outcome["value"]))];
    if let Some(id) = outcome["convergenceCertificate"].as_str() {
        out.push(format!("Covered by convergence certificate {id}."));
    }
    match outcome["ignoredOpenIssues"].as_array().map_or(0, Vec::len) {
        0 => {}
        1 => out.push("1 open issue was set aside under that certificate.".to_owned()),
        n => out.push(format!(
            "{n} open issues were set aside under that certificate."
        )),
    }
    out
}

fn contingent(query: &str, outcome: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut families: Vec<&str> = Vec::new();
    for pivot in outcome["pivots"].as_array().into_iter().flatten() {
        let name = ["family", "protocol", "kind"]
            .into_iter()
            .find_map(|key| pivot[key].as_str());
        if let Some(name) = name
            && !families.contains(&name)
        {
            families.push(name);
        }
    }
    if !families.is_empty() {
        out.push(format!("{query} depends on {}.", families.join(" and ")));
    }
    if let Some(alternatives) = outcome["alternatives"].as_object() {
        for (key, value) in sorted_entries(alternatives) {
            out.push(format!(
                "Under {} it is {}.",
                completion_text(key),
                value_text(value)
            ));
        }
    }
    out.push("No single answer is determinate across the admissible completions.".to_owned());
    out
}

fn suspended(query: &str, outcome: &Value) -> Vec<String> {
    let requests: Vec<String> = outcome["requests"]
        .as_array()
        .into_iter()
        .flatten()
        .map(request_text)
        .collect();
    let mut out = vec![format!("{query} is suspended.")];
    match requests.as_slice() {
        [] => {}
        [one] => out.push(format!("Outstanding request: {one}.")),
        many => out.push(format!("Outstanding requests: {}.", many.join("; "))),
    }
    out
}

fn norm_conflict(query: &str, outcome: &Value) -> Vec<String> {
    let names = names(&outcome["doctrines"]);
    if names.is_empty() {
        vec![format!("{query} ended in a norm conflict.")]
    } else {
        vec![format!(
            "{query} ended in a norm conflict: no single applicable doctrine among {}.",
            names.join(", ")
        )]
    }
}

fn outside_competence(query: &str, outcome: &Value) -> Vec<String> {
    let mut out = vec![match outcome["reason"].as_str() {
        Some(reason) if !reason.is_empty() => {
            format!("{query} is outside the module's competence: {reason}.")
        }
        _ => format!("{query} is outside the module's competence."),
    }];
    if !outcome["request"].is_null() {
        out.push(format!("Request: {}.", request_text(&outcome["request"])));
    }
    out
}

fn inconsistent(query: &str, outcome: &Value) -> Vec<String> {
    let mut out = vec![format!(
        "{query} has no consistent answer: the admitted model cannot be satisfied."
    )];
    let core = texts(&outcome["core"]);
    if !core.is_empty() {
        out.push(format!("Unsatisfiable core: {}.", core.join(", ")));
    }
    out
}

/// `I1` for a run key; `SuccessorEligibility = I1` for the explore key
/// `i:SuccessorEligibility=I1`. Each binding of a multi-binding key loses
/// its `x:` prefix, and the bindings are joined with `, `.
fn completion_text(key: &str) -> String {
    key.split(',')
        .map(|binding| {
            let bytes = binding.as_bytes();
            let rest = if bytes.len() > 1 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
                &binding[2..]
            } else {
                binding
            };
            rest.replace('=', " = ")
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn tagged_value_text(value: &Value) -> Option<String> {
    let data = &value["data"];
    let shown = match value["kind"].as_str()? {
        "int" | "decimal" | "bool" | "instant" | "duration" | "entity" => text(data),
        "string" => format!("\"{}\"", text(data)),
        "unit" => "unit".to_owned(),
        "ctor" => ctor_text(data)?,
        "set" => {
            let items: Vec<String> = data.as_array()?.iter().map(value_text).collect();
            format!("{{{}}}", items.join(", "))
        }
        "map" => {
            let entries: Vec<String> = sorted_entries(data.as_object()?)
                .into_iter()
                .map(|(key, item)| format!("{key}: {}", value_text(item)))
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
        "option" if data.is_null() => "none".to_owned(),
        "option" => value_text(data),
        _ => return None,
    };
    Some(shown)
}

/// `Name`, `Name(v0, v1)` for positional fields (`_0`, `_1`, …), or
/// `Name(k: v, …)` for named fields.
fn ctor_text(data: &Value) -> Option<String> {
    let name = data["name"].as_str()?;
    let fields = match data["fields"].as_object() {
        Some(fields) if !fields.is_empty() => fields,
        _ => return Some(name.to_owned()),
    };
    let args: Vec<String> = if fields.keys().all(|key| key.starts_with('_')) {
        let mut entries: Vec<(&String, &Value)> = fields.iter().collect();
        entries.sort_by_key(|(key, _)| (key[1..].parse::<u64>().ok(), key.as_str()));
        entries
            .into_iter()
            .map(|(_, item)| value_text(item))
            .collect()
    } else {
        sorted_entries(fields)
            .into_iter()
            .map(|(key, item)| format!("{key}: {}", value_text(item)))
            .collect()
    };
    Some(format!("{name}({})", args.join(", ")))
}

/// ` among a, b` when `list` is a nonempty array of strings, else nothing.
fn among(list: &Value) -> String {
    match list.as_array() {
        Some(items) if !items.is_empty() && items.iter().all(Value::is_string) => {
            format!(" among {}", texts(list).join(", "))
        }
        _ => String::new(),
    }
}

/// Doctrine names: string items, or the `name` of object items.
fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str().or_else(|| item["name"].as_str()))
        .map(str::to_owned)
        .collect()
}

/// Every item of an array as text; empty for anything else.
fn texts(list: &Value) -> Vec<String> {
    list.as_array().into_iter().flatten().map(text).collect()
}

/// A string as itself; any other JSON as compact JSON.
fn text(value: &Value) -> String {
    match value.as_str() {
        Some(string) => string.to_owned(),
        None => value.to_string(),
    }
}

fn sorted_entries(map: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut entries: Vec<(&String, &Value)> = map.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
}

#[cfg(test)]
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p fidryn-cli --offline --lib opinion::tests`
Expected: PASS (`test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 46 filtered out`)

Run: `cargo clippy -p fidryn-cli --offline --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: `Finished` with no warnings, and no output from `cargo fmt`.

- [ ] **Step 5: Commit**

```bash
git add crates/fidryn-cli/src/opinion.rs crates/fidryn-cli/src/lib.rs
git -c commit.gpgsign=false commit -m "feat(cli): add opinion sentences for evaluation reports"
```

### Task 3: Opinion in mill responses

**Files:**
- Modify: `crates/fidryn-cli/src/ui.rs` (`use crate::{…}` list; `mill_report_doc` doc comment and success body; new tests in `mod tests` before `render_interpolates_module_vars`)
- Modify: `schemas/mill-evaluation-response-v0.1.json` (declare `opinion`; the schema has `additionalProperties: false`, so without this every real success response would violate the published transport schema)
- Modify: `docs/mill.md` (Routes table, `/api/run` success cell; the success paragraph of "`POST /api/run` and `POST /api/explore`")
- Test: `crates/fidryn-cli/src/ui.rs` (`mod tests`)

**Interfaces:**
- Consumes: Task 2 `opinion::sentences(report: &serde_json::Value) -> Vec<String>`.
- Produces (C3): `POST /api/run` and `POST /api/explore` success bodies are `{"ok": true, "report": {...}, "opinion": ["…", …]}`; `report` never contains `opinion`. Task 15's Opinion view lays out `opinion` as is. `schemas/mill-evaluation-response-v0.1.json` declares `opinion` as an optional array of strings. Test helper in `ui.rs` tests: `fn opinion_of(json: &serde_json::Value) -> Vec<&str>`.

- [ ] **Step 1: Write the failing test**

Add the tests before `render_interpolates_module_vars`. They embed the trust module and the two-certificates case with `include_str!` (the mill compiles pasted source in memory, so the test posts the text, as the page does).

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
    #[tokio::test]
    async fn render_interpolates_module_vars() {
```

Replace it with:

```rust
    /// The `opinion` sentences of a success transport.
    fn opinion_of(json: &serde_json::Value) -> Vec<&str> {
        json["opinion"]
            .as_array()
            .unwrap_or_else(|| panic!("success transport must carry opinion: {json}"))
            .iter()
            .map(|sentence| {
                sentence
                    .as_str()
                    .unwrap_or_else(|| panic!("opinion holds strings: {json}"))
            })
            .collect()
    }

    #[tokio::test]
    async fn run_and_explore_transports_carry_opinion() {
        for uri in ["/api/run", "/api/explore"] {
            let (status, json) = post_json(uri, eval_body()).await;
            assert_eq!(status, StatusCode::OK, "{uri} {json}");
            let report = mill_report(&json);
            let sentences = opinion_of(&json);
            assert!(!sentences.is_empty(), "{uri} {json}");
            assert_eq!(sentences, crate::opinion::sentences(report), "{uri}");
            assert!(
                report.get("opinion").is_none(),
                "opinion is transport, not a report field: {json}"
            );
            assert_eq!(
                crate::result_snapshot_names(&json, "q"),
                crate::result_snapshot_names(report, "q"),
                "fidryn diff reads the report through the transport: {uri}"
            );
            assert_eq!(
                crate::assurance_snapshot_names(&json),
                crate::assurance_snapshot_names(report),
                "{uri}"
            );
        }
    }

    #[tokio::test]
    async fn success_transport_keys_are_in_the_mill_response_schema() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../schemas/mill-evaluation-response-v0.1.json"
        ))
        .expect("schema JSON");
        let declared = schema["properties"].as_object().expect("properties");
        let (status, json) = post_json("/api/run", eval_body()).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        for key in json.as_object().expect("transport object").keys() {
            assert!(
                declared.contains_key(key),
                "`{key}` is not declared in the transport schema"
            );
        }
        assert_eq!(
            schema["properties"]["opinion"],
            serde_json::json!({"type": "array", "items": {"type": "string"}})
        );
    }

    const TRUST_MODULE: &str = include_str!("../../../examples/trust/bryan-revocable-trust.fr");
    const TRUST_TWO_CERTIFICATES: &str =
        include_str!("../../../examples/trust/cases/two-certificates-open-eligibility.json");

    #[tokio::test]
    async fn trust_run_opinion_reads_the_contingent_report() {
        let case: serde_json::Value =
            serde_json::from_str(TRUST_TWO_CERTIFICATES).expect("case JSON");
        let body = serde_json::json!({
            "source": TRUST_MODULE,
            "query": "acting_trustee",
            "case": case,
            "validAt": "2034-03-01T09:00:00Z",
            "knownAt": "2034-03-01T09:00:00Z"
        });
        let (status, json) = post_json("/api/run", body).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_eq!(
            mill_report(&json)["outcomeDocument"]["outcome"]["kind"],
            "contingent",
            "{json}"
        );
        assert_eq!(
            opinion_of(&json),
            [
                "acting_trustee depends on SuccessorEligibility.",
                "Under I1 it is Alice.",
                "Under I2 it is Bob.",
                "No single answer is determinate across the admissible completions.",
                "Outside scope: tax, creditor_priority, real_property_recording, complete_Massachusetts_trust_law.",
            ]
        );
    }

    #[tokio::test]
    async fn render_interpolates_module_vars() {
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p fidryn-cli --offline --lib ui::tests`
Expected: FAIL (`test result: FAILED. 21 passed; 3 failed`). `run_and_explore_transports_carry_opinion` and `trust_run_opinion_reads_the_contingent_report` panic with `success transport must carry opinion: {"ok":true,"report":{…`; `success_transport_keys_are_in_the_mill_response_schema` fails with `left: Null` against `right: Object {"type": String("array"), "items": Object {"type": String("string")}}`.

- [ ] **Step 3: Write the implementation**

Import the module:

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
use crate::{
    EngineFailure, compile_source, explore_report, merge_bounds_json, parse_instant, render_report,
};
```

Replace it with:

```rust
use crate::{
    EngineFailure, compile_source, explore_report, merge_bounds_json, opinion, parse_instant,
    render_report,
};
```

Document the new transport field:

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
/// Mill success transport: `{ "ok": true, "report": <evaluation-report> }`.
///
/// `ok` is not a field of `fidryn.evaluation-report/v0.1`. Pasted compile
/// is `sourceTrust: unauthenticated` and is never `byteVerified`.
```

Replace it with:

```rust
/// Mill success transport:
/// `{ "ok": true, "report": <evaluation-report>, "opinion": [<sentence>, ...] }`.
///
/// `ok` and `opinion` are not fields of `fidryn.evaluation-report/v0.1`;
/// `opinion` is [`opinion::sentences`] of the report. Pasted compile is
/// `sourceTrust: unauthenticated` and is never `byteVerified`.
```

Add `opinion` beside `ok` and `report` (the `json!` macro serializes `report_json` by reference, so it is still available for `sentences`):

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
            Json(serde_json::json!({
                "ok": true,
                "report": report_json,
            })),
```

Replace it with:

```rust
            Json(serde_json::json!({
                "ok": true,
                "report": report_json,
                "opinion": opinion::sentences(&report_json),
            })),
```

Replace the contents of `schemas/mill-evaluation-response-v0.1.json` with:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://fidryn.dev/schemas/mill-evaluation-response-v0.1.json",
  "title": "fidryn.mill-evaluation-response/v0.1",
  "$comment": "HTTP success transport for mill /api/run and /api/explore. ok and opinion are not fields of fidryn.evaluation-report/v0.1. opinion holds plain sentences filled from the report by fixed templates. Error bodies use a different shape ({ok: false, ...}) and are not this schema.",
  "type": "object",
  "required": ["ok", "report"],
  "additionalProperties": false,
  "properties": {
    "ok": { "const": true },
    "report": { "$ref": "evaluation-report-v0.1.json" },
    "opinion": { "type": "array", "items": { "type": "string" } }
  }
}
```

Update the mill guide. The Routes table's `/api/run` row (the `/api/explore` row already says "same transport as `/api/run`"):

In `docs/mill.md`, find:

```markdown
| `POST` | `/api/run` | JSON `EvalRequest` | `{ "ok": true, "report": <evaluation-report> }` |
```

Replace it with:

```markdown
| `POST` | `/api/run` | JSON `EvalRequest` | `{ "ok": true, "report": <evaluation-report>, "opinion": [<sentence>, ...] }` |
```

The success paragraph:

In `docs/mill.md`, find:

```markdown
On evaluation success, HTTP 200, and the body is a transport wrapper
`{ "ok": true, "report": ... }`. `report` is the
`fidryn.evaluation-report/v0.1` envelope the CLI prints
(`schema`, `executionMode`, `assumptions`, `sourceTrust`,
`verificationMethod`, `coverage`, `outcomeDocument`). `ok` is not a
field of that report schema (`additionalProperties` is false). Nested
`outcomeDocument` is the `fidryn.outcome/v0.1` projection. Pasted
compile is `sourceTrust: unauthenticated`. `run` never chooses a
completion.
```

Replace it with:

```markdown
On evaluation success, HTTP 200, and the body is a transport wrapper
`{ "ok": true, "report": ..., "opinion": [...] }`
(`schemas/mill-evaluation-response-v0.1.json`). `report` is the
`fidryn.evaluation-report/v0.1` envelope the CLI prints
(`schema`, `executionMode`, `assumptions`, `sourceTrust`,
`verificationMethod`, `coverage`, `outcomeDocument`). `opinion` is the
report read as plain sentences, for example
`acting_trustee depends on SuccessorEligibility.` and
`Under I1 it is Alice.` for a contingent trust run. Each sentence is a
fixed template filled from report fields, so it never says more than
the report does. `ok` and `opinion` are not fields of that report schema
(`additionalProperties` is false). Nested `outcomeDocument` is the
`fidryn.outcome/v0.1` projection. Pasted compile is
`sourceTrust: unauthenticated`. `run` never chooses a completion.
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p fidryn-cli --offline --lib ui::tests`
Expected: PASS (`test result: ok. 24 passed; 0 failed`)

Run: `python3 conformance/schema_boundary_probes.py | grep -A1 '"name": "mill_'`
Expected: the same three results as before this task: `mill_flattened_transport_ok` then `"accepted_by_schema": false`, `mill_transport_wrapper` then `"accepted_by_schema": true`, and `mill_flattened_rejected_by_transport_schema` then `"accepted_by_schema": false`. The probe's `{ok, report}` wrapper stays valid because `opinion` is optional.

Run: `cargo clippy -p fidryn-cli --offline --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: `Finished` with no warnings, and no output from `cargo fmt`.

- [ ] **Step 5: Commit**

```bash
git add crates/fidryn-cli/src/ui.rs schemas/mill-evaluation-response-v0.1.json docs/mill.md
git -c commit.gpgsign=false commit -m "feat(mill): add opinion sentences to run and explore responses"
```

### Task 4: `cargo xtask site` skeleton

**Files:**
- Modify: `xtask/src/main.rs` (module list, `enum Command`, the `match` in `main`)
- Create: `xtask/src/site/mod.rs`
- Create: `xtask/src/site/guides.rs`
- Create: `xtask/src/site/html.rs`
- Create: `xtask/src/site/templates.rs`
- Create: `xtask/src/site/seo.rs`
- Test: `xtask/tests/task_selection.rs` (`help_lists_subcommands`, new `site_help_lists_check`); unit tests at the bottom of every new file

**Interfaces:**
- Consumes: `crate::workspace::workspace_root() -> PathBuf` (existing, `xtask/src/workspace.rs`).
- Produces (contract C4, token for token; the code below is the rustfmt output of the C4 text, because `rustfmt.toml` reflows the long `GUIDES` lines):
  - `site::SiteArgs` (clap `Args` with `--check`), `site::OutFile { pub path: PathBuf, pub bytes: Vec<u8> }` and `OutFile::text(path, text)`, `site::run(SiteArgs) -> Result<()>`, `site::check(root: &Path) -> Result<()>`, `site::build(root: &Path) -> Result<Vec<OutFile>>` (a placeholder rendering `robots.txt` and `sitemap.xml`; Tasks 9–11 replace it), `site::stale_files(site: &Path, files: &[OutFile]) -> Result<Vec<String>>`, `site::write_files(site: &Path, files: &[OutFile]) -> Result<()>`.
  - `site/mod.rs` module list after this task: `mod guides; mod html; mod seo; mod templates;`. Task 7 inserts `mod highlight;` after `mod guides;`; Task 8 inserts `mod links;` and `mod markdown;` after `mod html;`. The `#[cfg(test)] mod tests` block stays at the bottom of `mod.rs`; later edits to `build` do not touch it.
  - `guides::{SITE, REPO, GROUPS, Guide, GUIDES, IMPLEMENTER_DOCS, url, by_file}`, `html::{esc, asset_version}`, `templates::fill` (values are inserted verbatim and never re-scanned, so a page body containing `{{module}}` survives), `seo::{robots, sitemap}`. Tasks 10 and 11 add their `seo` functions above this file's `#[cfg(test)]` line and extend its `use super::guides::{…};` line as needed.
  - The CLI commands `cargo xtask site` and `cargo xtask site --check`.

- [ ] **Step 1: Write the failing tests**

In `xtask/tests/task_selection.rs`, replace the end of `help_lists_subcommands`:

```rust
    assert!(help.contains("ci"), "{help}");
}
```

with (the help test now requires `site`, and a new test checks `site --help`):

```rust
    assert!(help.contains("ci"), "{help}");
    assert!(help.contains("site"), "{help}");
    assert!(
        help.contains("Render the public site from docs/*.md"),
        "{help}"
    );
}

#[test]
fn site_help_lists_check() {
    let help = stdout_ok(&["site", "--help"]);
    assert!(help.contains("--check"), "{help}");
}
```

In `xtask/src/main.rs`, declare the module. Replace:

```rust
mod ci;
mod test;
```

with:

```rust
mod ci;
mod site;
mod test;
```

Create `xtask/src/site/mod.rs` with the module list and its tests (the implementation is inserted above `#[cfg(test)]` in Step 3). The tests use a fresh directory per test under `std::env::temp_dir()`, named by process id and test, so parallel tests never share files:

```rust
//! `cargo xtask site`: render the public site under `site/` from `docs/*.md`.

mod guides;
mod html;
mod seo;
mod templates;

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh directory under the system temp dir, private to one test.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("fidryn-xtask-site-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn stale_files_lists_missing_differing_then_extra_docs_files() {
        let site = scratch("stale");
        fs::create_dir_all(site.join("docs")).unwrap();
        fs::create_dir_all(site.join("assets")).unwrap();
        fs::write(site.join("robots.txt"), "same").unwrap();
        fs::write(site.join("docs/cli.html"), "old").unwrap();
        fs::write(site.join("docs/zeta.md"), "stray").unwrap();
        fs::write(site.join("docs/alpha.html"), "stray").unwrap();
        fs::write(site.join("assets/fidryn.css"), "hand-written").unwrap();
        fs::write(site.join("README.md"), "hand-written").unwrap();
        let files = [
            OutFile::text("docs/cli.html", "new"),
            OutFile::text("robots.txt", "same"),
            OutFile::text("sitemap.xml", "<urlset/>"),
        ];
        let stale = stale_files(&site, &files).unwrap();
        assert_eq!(
            stale,
            [
                "docs/cli.html differs",
                "sitemap.xml is missing",
                "docs/alpha.html is not generated; delete it",
                "docs/zeta.md is not generated; delete it",
            ]
        );
        fs::remove_dir_all(&site).unwrap();
    }

    #[test]
    fn stale_files_without_a_docs_dir_reports_only_generated_paths() {
        let site = scratch("no-docs");
        let files = [
            OutFile::text("docs/index.html", "x"),
            OutFile::text("robots.txt", "r"),
        ];
        let stale = stale_files(&site, &files).unwrap();
        assert_eq!(
            stale,
            ["docs/index.html is missing", "robots.txt is missing"]
        );
        fs::remove_dir_all(&site).unwrap();
    }

    #[test]
    fn write_then_check_is_clean() {
        let site = scratch("write-check");
        fs::create_dir_all(site.join("docs")).unwrap();
        fs::write(site.join("docs/cli.html"), "old").unwrap();
        let files = [
            OutFile::text("index.html", "<!doctype html>\n"),
            OutFile::text("docs/cli.html", "new"),
            OutFile {
                path: PathBuf::from("docs/data.bin"),
                bytes: vec![0, 159, 146, 150],
            },
        ];
        write_files(&site, &files).unwrap();
        assert!(stale_files(&site, &files).unwrap().is_empty());
        assert_eq!(
            fs::read_to_string(site.join("docs/cli.html")).unwrap(),
            "new"
        );
        assert_eq!(
            fs::read(site.join("docs/data.bin")).unwrap(),
            [0, 159, 146, 150]
        );
        write_files(&site, &files).unwrap();
        assert!(stale_files(&site, &files).unwrap().is_empty());
        fs::remove_dir_all(&site).unwrap();
    }
}
```

Create `xtask/src/site/guides.rs`:

```rust
//! The learner guides published on the site, in reading order.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_root;

    #[test]
    fn overview_comes_first_and_is_unnumbered() {
        let first = &GUIDES[0];
        assert_eq!(
            (first.slug, first.file, first.number, first.group),
            ("index", "README.md", None, "")
        );
        assert!(
            GUIDES[1..]
                .iter()
                .all(|g| g.number.is_some() && !g.group.is_empty())
        );
    }

    #[test]
    fn numbered_guides_run_one_to_eight_in_order() {
        let numbers: Vec<u32> = GUIDES.iter().filter_map(|g| g.number).collect();
        assert_eq!(numbers, (1..=8).collect::<Vec<u32>>());
    }

    #[test]
    fn groups_are_contiguous_and_in_sidebar_order() {
        let mut runs: Vec<&str> = Vec::new();
        for guide in &GUIDES[1..] {
            if runs.last() != Some(&guide.group) {
                runs.push(guide.group);
            }
        }
        assert_eq!(runs, GROUPS);
    }

    #[test]
    fn every_source_file_exists_under_docs() {
        let docs = workspace_root().join("docs");
        let files = GUIDES
            .iter()
            .map(|g| g.file)
            .chain(IMPLEMENTER_DOCS.iter().map(|d| d.1));
        for file in files {
            assert!(docs.join(file).is_file(), "docs/{file} does not exist");
        }
    }

    #[test]
    fn slugs_and_files_are_unique_and_every_guide_has_copy() {
        for (i, a) in GUIDES.iter().enumerate() {
            assert!(
                !a.title.is_empty() && !a.blurb.is_empty() && !a.description.is_empty(),
                "{}",
                a.slug
            );
            for b in &GUIDES[i + 1..] {
                assert_ne!(a.slug, b.slug);
                assert_ne!(a.file, b.file);
            }
        }
    }

    #[test]
    fn url_maps_the_overview_to_the_docs_root() {
        assert_eq!(url("index"), "/docs/");
        assert_eq!(url("cases-and-time"), "/docs/cases-and-time");
    }

    #[test]
    fn by_file_finds_published_guides_only() {
        assert_eq!(by_file("outcomes.md").map(|g| g.slug), Some("outcomes"));
        assert_eq!(by_file("README.md").map(|g| g.slug), Some("index"));
        assert!(by_file("ARCHITECTURE.md").is_none());
        assert!(by_file("docs/cli.md").is_none());
    }
}
```

Create `xtask/src/site/html.rs` (the pinned values are FNV-1a 64-bit, low 32 bits: the offset basis `0xcbf29ce484222325` gives `84222325` for empty input, and `fidryn` gives `388c70c5`):

```rust
//! HTML escaping and asset fingerprints.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn esc_escapes_markup_and_both_quotes() {
        assert_eq!(
            esc(r#"<a href="x">Tom & 'Jerry'</a>"#),
            "&lt;a href=&quot;x&quot;&gt;Tom &amp; &#39;Jerry&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn esc_leaves_other_text_alone() {
        assert_eq!(esc("§ 6 — café {{slot}}"), "§ 6 — café {{slot}}");
        assert_eq!(esc(""), "");
    }

    #[test]
    fn asset_version_is_pinned() {
        assert_eq!(asset_version(b""), "84222325");
        assert_eq!(asset_version(b"fidryn"), "388c70c5");
    }

    #[test]
    fn asset_version_is_eight_hex_digits_that_follow_the_content() {
        let v = asset_version(b"body { color: red }");
        assert_eq!(v.len(), 8);
        assert!(
            v.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "{v}"
        );
        assert_eq!(v, asset_version(b"body { color: red }"));
        assert_ne!(v, asset_version(b"body { color: blue }"));
    }
}
```

Create `xtask/src/site/templates.rs`:

```rust
//! `{{slot}}` templates. A slot with no value is a bug and panics.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_replaces_every_slot() {
        let page = fill(
            "<title>{{title}}</title><p>{{body}}</p>",
            &[("title", "T"), ("body", "B")],
        );
        assert_eq!(page, "<title>T</title><p>B</p>");
    }

    #[test]
    fn fill_repeats_a_slot() {
        assert_eq!(fill("{{x}} and {{x}}", &[("x", "a")]), "a and a");
    }

    #[test]
    fn fill_trims_spaces_inside_the_braces() {
        assert_eq!(fill("[{{ name }}]", &[("name", "v")]), "[v]");
    }

    #[test]
    fn fill_inserts_values_verbatim() {
        // docs/mill.md shows `{{module}}@{{version}}`; a filled body must keep it.
        let page = fill(
            "<main>{{main}}</main>",
            &[("main", "{{module}}@{{version}}")],
        );
        assert_eq!(page, "<main>{{module}}@{{version}}</main>");
    }

    #[test]
    #[should_panic(expected = "template slot `missing` has no value")]
    fn fill_panics_on_a_missing_slot() {
        fill("{{missing}}", &[("other", "x")]);
    }

    #[test]
    #[should_panic(expected = "unclosed `{{` in template")]
    fn fill_panics_on_an_unclosed_slot() {
        fill("{{open", &[]);
    }
}
```

Create `xtask/src/site/seo.rs`:

```rust
//! Files for search engines and agents.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn robots_is_exact() {
        assert_eq!(
            robots(),
            "User-agent: *\nAllow: /\n\nSitemap: https://fidryn.onlygass.dev/sitemap.xml\n\n# LLM / agent maps\n# https://fidryn.onlygass.dev/llms.txt\n# https://fidryn.onlygass.dev/llms-full.txt\n"
        );
    }

    #[test]
    fn sitemap_lists_the_landing_page_then_every_guide_in_order() {
        let xml = sitemap();
        let locs: Vec<&str> = xml
            .split("<loc>")
            .skip(1)
            .map(|rest| &rest[..rest.find("</loc>").expect("closed <loc>")])
            .collect();
        assert_eq!(
            locs,
            [
                "https://fidryn.onlygass.dev/",
                "https://fidryn.onlygass.dev/docs/",
                "https://fidryn.onlygass.dev/docs/getting-started",
                "https://fidryn.onlygass.dev/docs/language",
                "https://fidryn.onlygass.dev/docs/cases-and-time",
                "https://fidryn.onlygass.dev/docs/cli",
                "https://fidryn.onlygass.dev/docs/mill",
                "https://fidryn.onlygass.dev/docs/outcomes",
                "https://fidryn.onlygass.dev/docs/examples",
                "https://fidryn.onlygass.dev/docs/contributing",
            ]
        );
    }

    #[test]
    fn sitemap_is_a_complete_urlset_without_dates() {
        let xml = sitemap();
        assert!(xml.starts_with(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n  <url>\n    <loc>https://fidryn.onlygass.dev/</loc>\n    <changefreq>weekly</changefreq>\n    <priority>1.0</priority>\n  </url>\n"
        ));
        assert!(xml.ends_with("  </url>\n</urlset>\n"));
        assert_eq!(xml.matches("<url>").count(), 10);
        assert!(!xml.contains("lastmod"));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p xtask --offline`
Expected: FAIL to compile. The errors include ``error[E0425]: cannot find function `stale_files` in this scope``, ``error[E0425]: cannot find value `GUIDES` in this scope``, ``error[E0425]: cannot find function `esc` in this scope``, ``error[E0425]: cannot find function `fill` in this scope``, ``error[E0425]: cannot find function `robots` in this scope``, and the run ends with ``error: could not compile `xtask` (bin "xtask" test)``.

- [ ] **Step 3: Write the implementation**

In `xtask/src/main.rs`, add the subcommand. Replace:

```rust
    /// Workspace CI: tests, clippy -D warnings, schema probes
    Ci(ci::CiArgs),
}
```

with:

```rust
    /// Workspace CI: tests, clippy -D warnings, schema probes
    Ci(ci::CiArgs),
    /// Render the public site from docs/*.md (--check verifies the committed site)
    Site(site::SiteArgs),
}
```

and replace:

```rust
        Command::Ci(args) => ci::run(args),
```

with:

```rust
        Command::Ci(args) => ci::run(args),
        Command::Site(args) => site::run(args),
```

In `xtask/src/site/mod.rs`, replace the line `#[cfg(test)]` with:

```rust
use crate::workspace::workspace_root;
use anyhow::{Context, Result, bail};
use clap::Args;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Args)]
pub struct SiteArgs {
    /// Fail if the committed site differs from a fresh render instead of writing it
    #[arg(long)]
    check: bool,
}

/// One generated file, relative to `site/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

impl OutFile {
    pub fn text(path: impl Into<PathBuf>, text: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            bytes: text.into().into_bytes(),
        }
    }
}

pub fn run(args: SiteArgs) -> Result<()> {
    let root = workspace_root();
    if args.check {
        return check(&root);
    }
    let files = build(&root)?;
    write_files(&root.join("site"), &files)?;
    eprintln!("wrote {} files under site/", files.len());
    Ok(())
}

/// Fail when the committed site differs from a fresh render.
pub fn check(root: &Path) -> Result<()> {
    let files = build(root)?;
    let stale = stale_files(&root.join("site"), &files)?;
    if !stale.is_empty() {
        bail!(
            "site/ is out of date; run `cargo xtask site`:\n  {}",
            stale.join("\n  ")
        );
    }
    eprintln!("site/ is up to date ({} generated files)", files.len());
    Ok(())
}

/// Render every generated file. Reads inputs under `root`; writes nothing.
pub fn build(_root: &Path) -> Result<Vec<OutFile>> {
    Ok(vec![
        OutFile::text("robots.txt", seo::robots()),
        OutFile::text("sitemap.xml", seo::sitemap()),
    ])
}

/// Generated files that are missing or differ on disk, then files under
/// `site/docs/` that the generator no longer produces (sorted).
pub fn stale_files(site: &Path, files: &[OutFile]) -> Result<Vec<String>> {
    let mut stale = Vec::new();
    for file in files {
        match fs::read(site.join(&file.path)) {
            Ok(bytes) if bytes == file.bytes => {}
            Ok(_) => stale.push(format!("{} differs", file.path.display())),
            Err(_) => stale.push(format!("{} is missing", file.path.display())),
        }
    }
    let expected: BTreeSet<PathBuf> = files.iter().map(|f| f.path.clone()).collect();
    let docs = site.join("docs");
    if docs.is_dir() {
        let mut extra = Vec::new();
        for entry in fs::read_dir(&docs).with_context(|| format!("read {}", docs.display()))? {
            let rel = Path::new("docs").join(entry?.file_name());
            if !expected.contains(&rel) {
                extra.push(format!("{} is not generated; delete it", rel.display()));
            }
        }
        extra.sort();
        stale.extend(extra);
    }
    Ok(stale)
}

/// Write `files` under `site`, creating directories as needed.
pub fn write_files(site: &Path, files: &[OutFile]) -> Result<()> {
    for file in files {
        let path = site.join(&file.path);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
        }
        fs::write(&path, &file.bytes).with_context(|| format!("write {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
```

In `xtask/src/site/guides.rs`, replace the line `#[cfg(test)]` with:

```rust
pub const SITE: &str = "https://fidryn.onlygass.dev";
pub const REPO: &str = "https://github.com/BeeGass/fidryn";

/// Sidebar groups, in order. Every numbered guide belongs to one.
pub const GROUPS: &[&str] = &["Start", "Write", "Run", "Read results", "Contribute"];

pub struct Guide {
    /// URL slug; `index` is the docs overview at `/docs/`.
    pub slug: &'static str,
    /// Source file under `docs/`.
    pub file: &'static str,
    /// Page title and navigation label.
    pub title: &'static str,
    /// Section number, `None` for the overview.
    pub number: Option<u32>,
    /// Sidebar group; empty for the overview.
    pub group: &'static str,
    /// One line under the title in the sidebar and the landing contents.
    pub blurb: &'static str,
    /// Meta description.
    pub description: &'static str,
}

pub const GUIDES: &[Guide] = &[
    Guide {
        slug: "index",
        file: "README.md",
        title: "Documentation",
        number: None,
        group: "",
        blurb: "Where to begin",
        description: "Fidryn documentation hub — learner guides for the programming language for legal instruments.",
    },
    Guide {
        slug: "getting-started",
        file: "getting-started.md",
        title: "Getting started",
        number: Some(1),
        group: "Start",
        blurb: "Check and run a tiny module",
        description: "Build, check, and run a tiny Fidryn (.fr) program in about an hour.",
    },
    Guide {
        slug: "language",
        file: "language.md",
        title: "Language",
        number: Some(2),
        group: "Write",
        blurb: "Modules, queries, rules, duties",
        description: "Fidryn language map: modules, queries, rules, duties, and what the language will not do.",
    },
    Guide {
        slug: "cases-and-time",
        file: "cases-and-time.md",
        title: "Cases and time",
        number: Some(3),
        group: "Write",
        blurb: "Records, valid-at, known-at",
        description: "Case records, admissible completions, valid-at and known-at in Fidryn.",
    },
    Guide {
        slug: "cli",
        file: "cli.md",
        title: "CLI",
        number: Some(4),
        group: "Run",
        blurb: "Every subcommand and flag",
        description: "Every fidryn CLI subcommand and flag the reference binary accepts.",
    },
    Guide {
        slug: "mill",
        file: "mill.md",
        title: "Mill",
        number: Some(5),
        group: "Run",
        blurb: "The localhost UI",
        description: "Localhost Fidryn mill UI on 127.0.0.1 — checks modules; does not live-file.",
    },
    Guide {
        slug: "outcomes",
        file: "outcomes.md",
        title: "Outcomes",
        number: Some(6),
        group: "Read results",
        blurb: "The six kinds and the envelope",
        description: "Determinate, Suspended, Contingent, and the rest of the Fidryn outcome envelope.",
    },
    Guide {
        slug: "examples",
        file: "examples.md",
        title: "Examples",
        number: Some(7),
        group: "Read results",
        blurb: "Trust, tax, fifty states",
        description: "Trust, tax, federal slices, and the fifty-state corpus map for Fidryn.",
    },
    Guide {
        slug: "contributing",
        file: "contributing.md",
        title: "Contributing",
        number: Some(8),
        group: "Contribute",
        blurb: "Toolchain, tests, commits",
        description: "Toolchain, tests, and how to work on the Fidryn reference interpreter.",
    },
];

/// Implementer documents, linked to GitHub from the sidebar: (title, file under docs/, blurb).
pub const IMPLEMENTER_DOCS: &[(&str, &str, &str)] = &[
    (
        "Architecture",
        "ARCHITECTURE.md",
        "Pipeline and crate contract",
    ),
    (
        "Implementation status",
        "implementation-status.md",
        "Evidence-backed capability matrix",
    ),
    ("Obligations", "OBLIGATIONS.md", "Review obligations"),
    (
        "Integration contract",
        "INTEGRATION-CONTRACT.md",
        "How the machinery is wired",
    ),
];

/// Site path of a guide: `/docs/` for the overview, `/docs/{slug}` otherwise.
pub fn url(slug: &str) -> String {
    if slug == "index" {
        "/docs/".to_owned()
    } else {
        format!("/docs/{slug}")
    }
}

/// The guide rendered from `file` (a name under `docs/`), if it is published.
pub fn by_file(file: &str) -> Option<&'static Guide> {
    GUIDES.iter().find(|g| g.file == file)
}

#[cfg(test)]
```

In `xtask/src/site/html.rs`, replace the line `#[cfg(test)]` with:

```rust
/// Escape text for HTML element content and double-quoted attributes.
pub fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Eight lowercase hex digits identifying `bytes` (FNV-1a, 64-bit, low 32 bits),
/// used as `?v=` on asset URLs so they can be cached for a year.
pub fn asset_version(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{:08x}", hash & 0xffff_ffff)
}

#[cfg(test)]
```

In `xtask/src/site/templates.rs`, replace the line `#[cfg(test)]` with:

```rust
/// Replace every `{{name}}` in `template` with its value from `vars`.
pub fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 4096);
    let mut rest = template;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let end = after
            .find("}}")
            .unwrap_or_else(|| panic!("unclosed `{{{{` in template"));
        let key = after[..end].trim();
        let value = vars
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .unwrap_or_else(|| panic!("template slot `{key}` has no value"));
        out.push_str(value);
        rest = &after[end + 2..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
```

In `xtask/src/site/seo.rs`, replace the line `#[cfg(test)]` with (the sitemap keeps the `changefreq` and `priority` values of today's `site/sitemap.xml`; it has no `lastmod`, so it only changes when the guide list does):

```rust
use super::guides::{GUIDES, SITE, url};

/// `robots.txt`: allow everything and point at the sitemap and the llms maps.
pub fn robots() -> String {
    format!(
        "User-agent: *\nAllow: /\n\nSitemap: {SITE}/sitemap.xml\n\n# LLM / agent maps\n# {SITE}/llms.txt\n# {SITE}/llms-full.txt\n"
    )
}

/// `sitemap.xml`: the landing page, then every guide in reading order. No
/// `lastmod`, so the file only changes when the guide list does.
pub fn sitemap() -> String {
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    push_url(&mut out, "/", "1.0");
    for guide in GUIDES {
        let priority = if guide.number.is_some() { "0.8" } else { "0.9" };
        push_url(&mut out, &url(guide.slug), priority);
    }
    out.push_str("</urlset>\n");
    out
}

fn push_url(out: &mut String, path: &str, priority: &str) {
    out.push_str(&format!(
        "  <url>\n    <loc>{SITE}{path}</loc>\n    <changefreq>weekly</changefreq>\n    <priority>{priority}</priority>\n  </url>\n"
    ));
}

#[cfg(test)]
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline`
Expected: PASS. `test result: ok. 23 passed; 0 failed` for the unit tests in `src/main.rs`, then `test result: ok. 9 passed; 0 failed` for `tests/task_selection.rs`. Dead-code warnings such as ``function `fill` is never used`` and ``constant `GROUPS` is never used`` are expected: `build` starts using these items in Tasks 9–11.

Run: `cargo fmt -p xtask -- --check`
Expected: no output, exit status 0.

Run: `cargo xtask site --check`
Expected: exit status 1 and, on stderr, ``Error: site/ is out of date; run `cargo xtask site`:`` followed by `  sitemap.xml differs` and 18 sorted lines of the form `  docs/cli.html is not generated; delete it` (the committed site is still the old Node build). `robots.txt` is not listed, because `seo::robots()` reproduces the committed file byte for byte. Do not run `cargo xtask site` without `--check` yet; Task 12 regenerates the committed site in one step.

- [ ] **Step 5: Commit**

```bash
git add xtask/src/main.rs xtask/src/site xtask/tests/task_selection.rs
git -c commit.gpgsign=false commit -m "feat(site): add cargo xtask site skeleton"
```

### Task 5: Design system

The shared stylesheet for the site and the mill (`site/assets/fidryn.css`), the F monogram favicon, and a test that holds every text color pair to WCAG AA in both themes. The look follows the picked board options: cream paper, rubric accent, Ink night palette, Fraunces book display and body, sentence-case Plex Sans labels, double rules, soft 6px corners, ink buttons, bordered small-caps stamps, framed code, inline section marks, paper grain, the F monogram, comfortable density, full-grid tables; stack hero, three-step specimen, essay landing, question legend, swipe cards on phones, described sidebar, inline heading numbers, right rail, tinted-bar callouts, two-column pager, dropdown search, left drawer, and the "No such provision." 404.

**Files:**
- Create: `site/assets/fidryn.css`
- Create: `site/favicon.svg`
- Test: `xtask/tests/design_tokens.rs`

**Interfaces:**
- Consumes: the existing font files `site/fonts/fraunces.woff2`, `plex-sans.woff2`, `plex-mono-400.woff2`, `plex-mono-500.woff2` (unchanged); nothing from earlier tasks.
- Produces: `site/assets/fidryn.css` with the C7 token blocks verbatim, the four `@font-face` rules (root-relative `/fonts/…`, `font-display: swap`), the `body::before` grain layer, and a style for every C7 class, matching the C5 markup. It also styles the state hooks that Task 6's script and Task 10's base template set: `html.js`, `body.drawer-open`, `.backdrop[hidden]`, `[data-copied]` on `.copy` and `.anchor`, `.search-results li[aria-selected="true"]`, `.spec-tabs [aria-selected="true"]`, `.spec-panel[hidden]`, `.spec-dots i.on`, `.onpage a[aria-current="true"]`, and `.btn[aria-busy="true"]` / `.btn:disabled` for the mill. No custom property other than the C7 tokens is declared or used. `site/favicon.svg` is the monogram drawn with shapes (no font), with a `prefers-color-scheme: dark` variant. Task 10 fingerprints the stylesheet (`Assets::read`), Task 13 embeds both files with `include_str!`, Task 14's `web/mill.css` layers on the stylesheet.

- [ ] **Step 1: Write the failing test**

Create `xtask/tests/design_tokens.rs`:

```rust
//! WCAG AA contrast of the design tokens in `site/assets/fidryn.css`.
//!
//! The stylesheet declares its colors in three token blocks: the light
//! `:root {` block, the explicit `:root[data-theme="dark"] {` block, and the
//! same dark values inside `@media (prefers-color-scheme: dark)`. These tests
//! parse those blocks and hold every text pair the site and the mill rely on
//! to 4.5:1 in both themes.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

const LIGHT: &str = ":root {";
const DARK: &str = ":root[data-theme=\"dark\"] {";
const DARK_MEDIA: &str = "@media (prefers-color-scheme: dark) {";
const DARK_SYSTEM: &str = ":root:not([data-theme=\"light\"]) {";

/// WCAG 2 level AA for normal-size text.
const AA: f64 = 4.5;

/// (foreground, background) token pairs that carry text.
const PAIRS: &[(&str, &str)] = &[
    ("--ink", "--paper"),
    ("--ink-2", "--paper"),
    ("--ink-3", "--paper"),
    ("--ink-3", "--paper-2"),
    ("--ink", "--paper-2"),
    ("--rubric", "--paper"),
    ("--paper", "--ink"),
    ("--det", "--paper"),
    ("--con", "--paper"),
    ("--sus", "--paper"),
    ("--nc", "--paper"),
    ("--oc", "--paper"),
    ("--inc", "--paper"),
    ("--tok-kw", "--paper"),
    ("--tok-type", "--paper"),
    ("--tok-str", "--paper"),
    ("--tok-num", "--paper"),
    ("--tok-com", "--paper"),
    ("--tok-punct", "--paper"),
];

fn stylesheet() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one directory below the workspace root")
        .join("site/assets/fidryn.css");
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("cannot read {}: {err}", path.display()))
}

/// Byte range of the declarations between `start` (ending in `{`) and the next `}`.
fn block_range(css: &str, from: usize, start: &str) -> (usize, usize) {
    let at = css[from..]
        .find(start)
        .map(|i| from + i)
        .unwrap_or_else(|| panic!("fidryn.css has no `{start}` block"));
    let body = at + start.len();
    let end = css[body..]
        .find('}')
        .map(|i| body + i)
        .unwrap_or_else(|| panic!("the `{start}` block in fidryn.css is not closed"));
    (body, end)
}

struct Blocks<'a> {
    light: &'a str,
    dark: &'a str,
    dark_system: &'a str,
    /// The stylesheet with the three token blocks cut out.
    rest: String,
}

fn blocks(css: &str) -> Blocks<'_> {
    let light = block_range(css, 0, LIGHT);
    let dark = block_range(css, 0, DARK);
    let media = css
        .find(DARK_MEDIA)
        .unwrap_or_else(|| panic!("fidryn.css has no `{DARK_MEDIA}` block"));
    let dark_system = block_range(css, media, DARK_SYSTEM);
    let mut ranges = [light, dark, dark_system];
    ranges.sort_unstable();
    let mut rest = String::with_capacity(css.len());
    let mut cursor = 0;
    for (start, end) in ranges {
        rest.push_str(&css[cursor..start]);
        cursor = end;
    }
    rest.push_str(&css[cursor..]);
    Blocks {
        light: &css[light.0..light.1],
        dark: &css[dark.0..dark.1],
        dark_system: &css[dark_system.0..dark_system.1],
        rest,
    }
}

/// Custom properties declared in a block: name to value.
fn tokens(block: &str) -> BTreeMap<&str, &str> {
    block
        .split(';')
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            let name = name.trim();
            name.starts_with("--").then(|| (name, value.trim()))
        })
        .collect()
}

/// Names referenced with `var(--name` anywhere in `css`.
fn used_properties(css: &str) -> BTreeSet<&str> {
    css.match_indices("var(")
        .filter_map(|(at, _)| {
            let rest = css[at + 4..].trim_start();
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            let name = &rest[..len];
            name.starts_with("--").then_some(name)
        })
        .collect()
}

/// Names declared as `--name:` in `css`.
fn declared_properties(css: &str) -> BTreeSet<&str> {
    css.match_indices("--")
        .filter_map(|(at, _)| {
            let rest = &css[at..];
            let len = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            let preceded_by_name = css[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '(');
            let declares = rest[len..].trim_start().starts_with(':');
            (declares && !preceded_by_name && len > 2).then(|| &rest[..len])
        })
        .collect()
}

fn channel(hex: &str) -> f64 {
    let v = f64::from(u8::from_str_radix(hex, 16).expect("hex channel")) / 255.0;
    if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of a `#rrggbb` color.
fn luminance(color: &str) -> f64 {
    let hex = color
        .strip_prefix('#')
        .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .unwrap_or_else(|| panic!("`{color}` is not a #rrggbb color"));
    0.2126 * channel(&hex[0..2]) + 0.7152 * channel(&hex[2..4]) + 0.0722 * channel(&hex[4..6])
}

fn contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

fn failing_pairs(theme: &str, block: &str) -> Vec<String> {
    let colors = tokens(block);
    let color = |name: &str| {
        *colors
            .get(name)
            .unwrap_or_else(|| panic!("the {theme} token block does not define `{name}`"))
    };
    PAIRS
        .iter()
        .filter_map(|&(fg, bg)| {
            let ratio = contrast(color(fg), color(bg));
            (ratio < AA).then(|| format!("{theme}: {fg} on {bg} is {ratio:.2}:1"))
        })
        .collect()
}

#[test]
fn text_pairs_meet_wcag_aa_in_both_themes() {
    let css = stylesheet();
    let b = blocks(&css);
    let mut failures = failing_pairs("light", b.light);
    failures.extend(failing_pairs("dark", b.dark));
    failures.extend(failing_pairs("system dark", b.dark_system));
    assert!(
        failures.is_empty(),
        "below {AA}:1:\n  {}",
        failures.join("\n  ")
    );
}

#[test]
fn explicit_dark_block_equals_system_dark_block() {
    let css = stylesheet();
    let b = blocks(&css);
    let normalize = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        normalize(b.dark),
        normalize(b.dark_system),
        "`{DARK}` and the `{DARK_SYSTEM}` block inside `{DARK_MEDIA}` must declare the same values"
    );
}

#[test]
fn every_custom_property_used_is_defined_in_the_light_block() {
    let css = stylesheet();
    let b = blocks(&css);
    let light = tokens(b.light);
    let missing: Vec<&str> = used_properties(&css)
        .into_iter()
        .filter(|name| !light.contains_key(name))
        .collect();
    assert!(
        missing.is_empty(),
        "used but not defined in `{LIGHT}`: {missing:?}"
    );
}

#[test]
fn tokens_are_declared_only_in_the_token_blocks() {
    let css = stylesheet();
    for selector in [LIGHT, DARK, DARK_SYSTEM] {
        assert_eq!(
            css.matches(selector).count(),
            1,
            "`{selector}` must appear exactly once"
        );
    }
    let b = blocks(&css);
    let stray = declared_properties(&b.rest);
    assert!(
        stray.is_empty(),
        "custom properties declared outside the token blocks would bypass the contrast check: {stray:?}"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p xtask --offline --test design_tokens`
Expected: FAIL with `test result: FAILED. 0 passed; 4 failed`; each of the four tests panics with `cannot read <repo>/site/assets/fidryn.css: No such file or directory (os error 2)`.

- [ ] **Step 3: Write the stylesheet**

Create `site/assets/fidryn.css`:

```css
/* Fidryn design system ("Statute"): warm paper, black ink, rubric red.
   Shared by the public site and the localhost mill. The three token blocks
   below are parsed by xtask/tests/design_tokens.rs; keep them in this shape. */

:root {
  color-scheme: light;
  --paper: #f7f2e8; --paper-2: #efe7d6; --paper-3: #e6dbc4;
  --ink: #1c1813; --ink-2: #3d362c; --ink-3: #6f6454; --rule: #d6c9b1; --rubric: #a3281d;
  --det: #2e6a39; --con: #23507f; --sus: #8a5d06; --nc: #9c3d10; --oc: #6b3a72; --inc: #b0182b;
  --tok-kw: #a3281d; --tok-type: #23507f; --tok-str: #3b6a2c; --tok-num: #86570a; --tok-com: #5e6652; --tok-punct: #6b6152;
  --grain-opacity: .06;
  --serif: "Fraunces", "Iowan Old Style", "Palatino Linotype", Georgia, serif;
  --sans: "IBM Plex Sans", system-ui, -apple-system, "Segoe UI", sans-serif;
  --mono: "IBM Plex Mono", ui-monospace, "SF Mono", Menlo, Consolas, monospace;
  --r: 6px; --gutter: clamp(16px, 4vw, 40px); --measure: 42rem; --row: 30px;
}
:root[data-theme="dark"] {
  color-scheme: dark;
  --paper: #15120e; --paper-2: #1e1a15; --paper-3: #28221b;
  --ink: #efe6d6; --ink-2: #cfc3ae; --ink-3: #9a8e7a; --rule: #3a3228; --rubric: #e0725f;
  --det: #8cc79a; --con: #8fb4e8; --sus: #e2b457; --nc: #ec9a67; --oc: #cfa1d8; --inc: #f07b7b;
  --tok-kw: #ec8a74; --tok-type: #9dbde8; --tok-str: #a8cf95; --tok-num: #e2b86a; --tok-com: #9aa08a; --tok-punct: #a79a84;
  --grain-opacity: .03;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    color-scheme: dark;
    --paper: #15120e; --paper-2: #1e1a15; --paper-3: #28221b;
    --ink: #efe6d6; --ink-2: #cfc3ae; --ink-3: #9a8e7a; --rule: #3a3228; --rubric: #e0725f;
    --det: #8cc79a; --con: #8fb4e8; --sus: #e2b457; --nc: #ec9a67; --oc: #cfa1d8; --inc: #f07b7b;
    --tok-kw: #ec8a74; --tok-type: #9dbde8; --tok-str: #a8cf95; --tok-num: #e2b86a; --tok-com: #9aa08a; --tok-punct: #a79a84;
    --grain-opacity: .03;
  }
}

@font-face {
  font-family: "Fraunces";
  src: url("/fonts/fraunces.woff2") format("woff2");
  font-weight: 100 900;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "IBM Plex Sans";
  src: url("/fonts/plex-sans.woff2") format("woff2");
  font-weight: 100 700;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "IBM Plex Mono";
  src: url("/fonts/plex-mono-400.woff2") format("woff2");
  font-weight: 400;
  font-style: normal;
  font-display: swap;
}
@font-face {
  font-family: "IBM Plex Mono";
  src: url("/fonts/plex-mono-500.woff2") format("woff2");
  font-weight: 500;
  font-style: normal;
  font-display: swap;
}

/* ---------- base ---------- */

*, *::before, *::after { box-sizing: border-box; }
[hidden] { display: none !important; }

/* The page ground lives on html so the grain layer (body::before, z-index -1)
   paints above it and below everything else. */
html {
  background: var(--paper);
  color: var(--ink-2);
  -webkit-text-size-adjust: 100%;
  text-size-adjust: 100%;
}
body {
  display: flex;
  flex-direction: column;
  min-height: 100vh;
  min-height: 100dvh;
  margin: 0;
  padding: 0 max(var(--gutter), env(safe-area-inset-right, 0px)) 0 max(var(--gutter), env(safe-area-inset-left, 0px));
  font: 400 16px/1.6 var(--serif);
  font-variation-settings: "opsz" 14;
  color: var(--ink-2);
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
}
body::before {
  content: "";
  position: fixed;
  inset: 0;
  z-index: -1;
  pointer-events: none;
  opacity: var(--grain-opacity);
  background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='180' height='180'><filter id='n'><feTurbulence type='fractalNoise' baseFrequency='.9' numOctaves='3' stitchTiles='stitch'/><feColorMatrix type='saturate' values='0'/></filter><rect width='180' height='180' filter='url(%23n)'/></svg>");
}
.site-head, main, .site-foot { width: 100%; max-width: 1360px; margin-inline: auto; }
main { flex: 1 0 auto; }

img, video { max-width: 100%; height: auto; }
p { margin: 0 0 1em; }
h1, h2, h3, h4, h5, h6 { text-wrap: balance; }
p, li, dd { text-wrap: pretty; }
button, input, select, textarea { font: inherit; color: inherit; }
button { -webkit-tap-highlight-color: transparent; }

a {
  color: var(--rubric);
  text-decoration: underline;
  text-decoration-thickness: 1px;
  text-decoration-color: color-mix(in srgb, var(--rubric) 55%, transparent);
  text-underline-offset: 3px;
}
a:hover { text-decoration-color: currentColor; }
:focus-visible { outline: 2px solid var(--rubric); outline-offset: 2px; }
::selection { background: color-mix(in srgb, var(--rubric) 22%, transparent); color: var(--ink); }

:not(pre) > code {
  padding: .08em .32em;
  border-radius: min(var(--r), 4px);
  background: var(--paper-2);
  color: var(--ink);
  font: 400 .86em/1 var(--mono);
  overflow-wrap: break-word;
}
kbd {
  padding: 0 .35em;
  border: 1px solid var(--rule);
  border-radius: 3px;
  background: var(--paper);
  color: var(--ink-2);
  font: 500 .78em/1.5 var(--mono);
}
hr { margin: 2em 0; border: 0; border-top: 1px solid var(--rule); }

/* ---------- utilities ---------- */

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  margin: -1px;
  padding: 0;
  overflow: hidden;
  clip: rect(0 0 0 0);
  white-space: nowrap;
  border: 0;
}
.skip {
  position: absolute;
  z-index: 100;
  top: -80px;
  left: max(12px, env(safe-area-inset-left, 0px));
  padding: 12px 16px;
  border-radius: var(--r);
  background: var(--ink);
  color: var(--paper);
  font: 600 14px/1 var(--sans);
  text-decoration: none;
}
.skip:focus { top: calc(12px + env(safe-area-inset-top, 0px)); }
.label { margin: 0; font: 600 12px/1.35 var(--sans); color: var(--ink-3); letter-spacing: 0; text-transform: none; }
.eyebrow { margin: 0 0 14px; font: 600 13px/1.35 var(--sans); color: var(--rubric); }
.kicker { margin: 0 0 10px; font: 600 13px/1.35 var(--sans); color: var(--rubric); }
.lede { margin: 0 0 16px; font-size: 19px; line-height: 1.55; color: var(--ink-2); }

/* ---------- header ---------- */

.site-head {
  display: flex;
  align-items: center;
  gap: 10px 18px;
  padding: calc(14px + env(safe-area-inset-top, 0px)) 0 10px;
  border-bottom: 3px double var(--ink);
}
.brand { display: inline-flex; flex: none; align-items: center; gap: 10px; color: var(--ink); text-decoration: none; }
.brand:hover .wordmark { color: var(--rubric); }
.mono {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: 26px;
  height: 26px;
  border: 1.5px solid var(--ink);
  border-radius: var(--r);
  background: var(--paper);
  box-shadow: 2px 2px 0 var(--rubric);
  color: var(--ink);
  font: 600 16px/1 var(--serif);
  font-variation-settings: "opsz" 36;
}
.wordmark { font: 560 22px/1 var(--serif); font-variation-settings: "opsz" 72; color: var(--ink); }
.crumb {
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font: 600 12px/1.35 var(--sans);
  color: var(--ink-3);
  white-space: nowrap;
  text-overflow: ellipsis;
}
.crumb a { color: var(--ink-3); text-decoration: none; }
.crumb a:hover { color: var(--rubric); }
.site-nav { display: flex; align-items: center; gap: 18px; margin-left: auto; }
.site-nav a { font: 500 13px/1.3 var(--sans); color: var(--ink); text-decoration: none; }
.site-nav a:hover, .site-nav a[aria-current="page"] { color: var(--rubric); }

.search { position: relative; flex: 0 1 230px; min-width: 150px; margin: 0; }
.search input {
  display: block;
  width: 100%;
  height: var(--row);
  margin: 0;
  padding: 0 34px 0 10px;
  border: 1px solid var(--rule);
  border-radius: var(--r);
  background: var(--paper-2);
  color: var(--ink);
  font: 400 13px/1 var(--sans);
  -webkit-appearance: none;
  appearance: none;
}
.search input::placeholder { color: var(--ink-3); opacity: 1; }
.search input::-webkit-search-decoration,
.search input::-webkit-search-cancel-button { -webkit-appearance: none; appearance: none; }
.search kbd {
  position: absolute;
  top: 50%;
  right: 8px;
  transform: translateY(-50%);
  pointer-events: none;
  color: var(--ink-3);
  font-size: 11px;
}
.search input:focus ~ kbd, .search input:not(:placeholder-shown) ~ kbd { display: none; }

.search-results {
  position: absolute;
  z-index: 30;
  top: calc(100% + 8px);
  right: 0;
  width: min(440px, calc(100vw - 32px));
  max-height: min(70vh, 540px);
  margin: 0;
  padding: 6px 0;
  overflow-y: auto;
  list-style: none;
  background: var(--paper);
  border: 1px solid var(--ink);
  border-radius: var(--r);
  box-shadow: 0 20px 40px rgb(0 0 0 / .2);
}
.search-results a {
  display: grid;
  grid-template-columns: 3.25rem minmax(0, 1fr);
  gap: 1px 10px;
  padding: 8px 16px 9px;
  color: var(--ink);
  text-decoration: none;
}
.search-results .n { grid-row: span 2; padding-top: 3px; font: 500 12px/1.4 var(--sans); color: var(--rubric); }
.search-results .h { font: 500 15px/1.35 var(--serif); color: var(--ink); }
.search-results .t {
  display: -webkit-box;
  overflow: hidden;
  font-size: 13px;
  line-height: 1.45;
  color: var(--ink-3);
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}
.search-results mark { border-radius: 2px; background: color-mix(in srgb, var(--rubric) 18%, transparent); color: inherit; }
.search-results li[aria-selected="true"] a { background: var(--paper-2); box-shadow: inset 3px 0 0 var(--rubric); }
.search-results .empty { padding: 10px 16px; font: 400 13px/1.4 var(--sans); color: var(--ink-3); }

.theme-toggle {
  display: inline-grid;
  place-items: center;
  flex: none;
  width: var(--row);
  height: var(--row);
  padding: 0;
  border: 1px solid var(--rule);
  border-radius: 50%;
  background: transparent;
  color: var(--ink);
  cursor: pointer;
}
.theme-toggle span {
  width: 12px;
  height: 12px;
  border: 1.5px solid currentColor;
  border-radius: 50%;
  background: linear-gradient(90deg, currentColor 50%, transparent 50%);
}
.theme-toggle:hover { border-color: var(--ink-3); }
.menu-btn {
  display: inline-flex;
  flex: none;
  align-items: center;
  height: var(--row);
  padding: 0 12px;
  border: 1px solid var(--rule);
  border-radius: var(--r);
  background: transparent;
  color: var(--ink);
  font: 600 13px/1 var(--sans);
  cursor: pointer;
}
.menu-btn:hover { border-color: var(--ink-3); }

/* ---------- buttons, commands, copy ---------- */

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: calc(var(--row) + 4px);
  padding: 0 14px;
  border: 1px solid transparent;
  border-radius: var(--r);
  background: transparent;
  color: var(--ink);
  font: 500 13px/1 var(--sans);
  white-space: nowrap;
  text-decoration: none;
  cursor: pointer;
}
.btn.pri { border-color: var(--ink); background: var(--ink); color: var(--paper); }
.btn.pri:hover { border-color: var(--ink-2); background: var(--ink-2); }
.btn.sec { border-color: var(--ink); }
.btn.sec:hover { background: var(--paper-2); }
.btn.sm { height: calc(var(--row) - 2px); padding: 0 10px; }
.btn:disabled, .btn[aria-disabled="true"] { opacity: .42; cursor: not-allowed; }
.btn[aria-busy="true"] { cursor: progress; }
.btn[aria-busy="true"]::after {
  content: "";
  width: 10px;
  height: 10px;
  border: 1.5px solid currentColor;
  border-right-color: transparent;
  border-radius: 50%;
  animation: fidryn-spin .8s linear infinite;
}
@keyframes fidryn-spin { to { transform: rotate(360deg); } }
.actions { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; }

.cmd {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 16px;
  padding: 8px 0;
  border-top: 1px solid var(--rule);
  border-bottom: 1px solid var(--rule);
  color: var(--ink);
  font: 400 13px/1.5 var(--mono);
}
.cmd .prompt { flex: none; color: var(--ink-3); -webkit-user-select: none; user-select: none; }
.cmd code {
  flex: 1 1 auto;
  min-width: 0;
  padding: 0;
  overflow-x: auto;
  border-radius: 0;
  background: none;
  color: inherit;
  font: inherit;
  white-space: nowrap;
  scrollbar-width: thin;
}
.copy {
  position: relative;
  flex: none;
  margin-left: auto;
  padding: 3px 7px;
  border: 0;
  border-radius: 4px;
  background: transparent;
  color: var(--ink-3);
  font: 500 12px/1.2 var(--sans);
  cursor: pointer;
}
.copy:hover { background: var(--paper-3); color: var(--ink); }
.copy[data-copied] { color: var(--det); }

/* ---------- code frames and syntax ---------- */

.code { margin: 0 0 1.25em; overflow: hidden; border: 1px solid var(--rule); border-radius: var(--r); background: var(--paper); }
.code figcaption {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: var(--row);
  padding: 3px 6px 3px 12px;
  border-bottom: 1px solid var(--rule);
  background: var(--paper-2);
  color: var(--ink-3);
  font: 600 12px/1.3 var(--sans);
}
.code figcaption span { min-width: 0; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.code pre {
  margin: 0;
  padding: 12px 14px 13px;
  overflow-x: auto;
  background: none;
  color: var(--ink);
  font: 400 13px/1.62 var(--mono);
  tab-size: 4;
}
.code pre code { padding: 0; border-radius: 0; background: none; color: inherit; font: inherit; }
.code pre.lines { padding-left: 8px; }
.code pre.lines .line::before {
  content: attr(data-n);
  display: inline-block;
  width: 3ch;
  margin-right: 2ch;
  color: var(--ink-3);
  text-align: right;
}
.code pre.lines .line.gap { color: var(--ink-3); }
.tk-kw { color: var(--tok-kw); }
.tk-ty { color: var(--tok-type); }
.tk-st { color: var(--tok-str); }
.tk-nu { color: var(--tok-num); }
.tk-co { color: var(--tok-com); }
.tk-pu { color: var(--tok-punct); }

/* ---------- outcome stamps (small caps on purpose: they are marks, not labels) ---------- */

.stamp {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 4px 7px;
  border: 1px solid currentColor;
  border-radius: min(var(--r), 4px);
  color: var(--ink-2);
  font: 500 10.5px/1 var(--sans);
  letter-spacing: .12em;
  text-transform: uppercase;
  text-decoration: none;
  white-space: nowrap;
  vertical-align: middle;
}
.stamp::before { content: ""; flex: none; width: 6px; height: 6px; background: currentColor; }
.stamp.det { color: var(--det); }
.stamp.con { color: var(--con); }
.stamp.sus { color: var(--sus); }
.stamp.nc { color: var(--nc); }
.stamp.oc { color: var(--oc); }
.stamp.inc { color: var(--inc); }
a.stamp:hover { background: color-mix(in srgb, currentColor 10%, transparent); }

/* ---------- tables and notes ---------- */

.table-wrap { max-width: 100%; margin: 0 0 1.4em; overflow-x: auto; }
table { width: 100%; border: 1px solid var(--rule); border-collapse: collapse; font-size: 15px; line-height: 1.5; }
th, td { padding: 7px 10px; border: 1px solid var(--rule); text-align: left; vertical-align: top; }
th { background: var(--paper-2); color: var(--ink-2); font: 600 12px/1.35 var(--sans); }
td { color: var(--ink-2); }
blockquote {
  margin: 1.4em 0;
  padding: 11px 16px;
  border-left: 3px solid var(--rubric);
  border-radius: 0 var(--r) var(--r) 0;
  background: color-mix(in srgb, var(--rubric) 7%, var(--paper));
  color: var(--ink-2);
  font-size: .95em;
}
blockquote > :first-child { margin-top: 0; }
blockquote > p:first-child > strong:first-child { color: var(--rubric); font: 600 .8em/1 var(--sans); }
blockquote > :last-child { margin-bottom: 0; }

/* ---------- landing ---------- */

.hero { padding: 44px 0 34px; }
.hero-top {
  display: grid;
  grid-template-columns: minmax(0, 1.15fr) minmax(0, 1fr);
  align-items: end;
  gap: 48px;
  margin-bottom: 30px;
}
.hero h1 {
  margin: 0;
  color: var(--ink);
  font: 400 clamp(42px, 5.6vw, 80px)/1.02 var(--serif);
  font-variation-settings: "opsz" 144;
  letter-spacing: -.02em;
}
.hero-side .lede { max-width: 34rem; }
.note { margin: 0 0 10px; padding: 11px 16px; border: 1px solid var(--rule); border-left: 3px solid var(--ink); color: var(--ink-2); font-size: 15px; }
.note p { margin: 0; }
.note strong { margin-right: 4px; color: var(--ink); font: 600 12px/1 var(--sans); }
.band { padding: 28px 0 32px; border-top: 1px solid var(--ink); }
.band h2 {
  margin: 0 0 14px;
  color: var(--ink);
  font: 460 30px/1.2 var(--serif);
  font-variation-settings: "opsz" 48;
  letter-spacing: -.01em;
}
.mark {
  margin-right: .3em;
  color: var(--rubric);
  font-size: 15px;
  font-weight: 400;
  font-variation-settings: "opsz" 14;
  letter-spacing: 0;
  white-space: nowrap;
}
.band-lede { max-width: 46rem; margin: 0 0 18px; color: var(--ink-2); }
.three { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); border-top: 1px solid var(--ink); }
.three > div { padding: 14px 20px 4px 0; }
.three > div + div { padding-left: 20px; border-left: 1px solid var(--rule); }
.three h3 { margin: 0 0 6px; color: var(--ink); font: 560 18px/1.25 var(--serif); font-variation-settings: "opsz" 30; }
.three p { margin: 0; font-size: 15px; }

.legend { display: grid; gap: 10px; margin: 0; padding: 0; list-style: none; }
.legend li {
  display: grid;
  grid-template-columns: 26px minmax(0, 1fr) auto;
  align-items: center;
  gap: 12px;
  padding: 10px 14px;
  border: 1px solid var(--rule);
  border-radius: var(--r);
}
.legend li::before { content: "?"; color: var(--rubric); font: 500 13px/1 var(--sans); }
.legend .q { margin: 0; color: var(--ink); line-height: 1.45; }
.legend .stamp { position: relative; }

.contents { margin: 0; padding: 0; list-style: none; column-gap: 48px; }
.contents li { break-inside: avoid; }
.contents a { display: flex; align-items: baseline; gap: 10px; padding: 6px 0; color: var(--ink); text-decoration: none; }
.contents .n { flex: none; width: 38px; color: var(--rubric); font-size: 17px; }
.contents .t { flex: none; font-size: 17px; }
.contents .lead { flex: 1 1 24px; min-width: 24px; border-bottom: 1px dotted var(--ink-3); transform: translateY(-5px); }
.contents .d { flex: 0 1 auto; min-width: 0; color: var(--ink-3); font: 400 13px/1.35 var(--sans); text-align: right; }
.contents a:hover .t { color: var(--rubric); }

/* ---------- landing specimen ---------- */

.specimen { overflow: hidden; border: 1px solid var(--ink); border-radius: var(--r); background: var(--paper); }
.spec-tabs { display: flex; border-bottom: 1px solid var(--rule); }
.spec-tabs [role="tab"] {
  display: flex;
  flex: 1 1 0;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 6px 8px;
  min-width: 0;
  min-height: 44px;
  padding: 9px 10px;
  border: 0;
  border-right: 1px solid var(--rule);
  background: transparent;
  color: var(--ink-2);
  font: 500 13px/1.3 var(--sans);
  text-align: center;
  cursor: pointer;
}
.spec-tabs [role="tab"]:last-child { border-right: 0; }
.spec-tabs [role="tab"]:hover { background: var(--paper-2); color: var(--ink); }
.spec-tabs [role="tab"]:focus-visible { outline-offset: -3px; }
.spec-tabs [aria-selected="true"], .spec-tabs [aria-selected="true"]:hover { background: var(--ink); color: var(--paper); }
.spec-tabs [aria-selected="true"]:focus-visible { box-shadow: inset 0 0 0 5px var(--paper); }
.spec-tabs [aria-selected="true"] .stamp { color: var(--paper); }
.spec-panel + .spec-panel { border-top: 1px solid var(--ink); }
.spec-cmd {
  margin: 0;
  padding: 9px 14px;
  border-bottom: 1px solid var(--rule);
  background: var(--paper-2);
  color: var(--ink-2);
  font: 400 12px/1.55 var(--mono);
}
.spec-cmd code { padding: 0; border-radius: 0; background: none; color: inherit; font: inherit; overflow-wrap: anywhere; }
.spec-label { display: none; }
.steps { display: grid; grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr) minmax(0, 1fr); }
.step { display: grid; align-content: start; gap: 10px; min-width: 0; padding: 14px 16px 16px; }
.step + .step { border-left: 1px solid var(--rule); }
.step .code { margin: 0; }
.step-h { display: flex; align-items: center; gap: 8px; margin: 0; color: var(--ink-2); font: 600 12px/1 var(--sans); }
.step-n {
  display: inline-grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border: 1px solid var(--rubric);
  border-radius: 50%;
  color: var(--rubric);
  font: 500 11px/1 var(--sans);
}
.step-outcome .stamp { justify-self: start; }
.opinion { margin: 0; padding: 0; list-style: none; }
.opinion li { padding: 7px 0; border-top: 1px solid var(--rule); color: var(--ink); font-size: 15px; line-height: 1.45; }
.opinion li:first-child { padding-top: 0; border-top: 0; }
.opinion .boundary { color: var(--ink-3); font-size: 14px; }
.spec-dots { display: none; }
.spec-foot { margin: 0; padding: 8px 14px; border-top: 1px solid var(--rule); color: var(--ink-3); font: 400 12px/1.4 var(--sans); }
html:not(.js) .spec-tabs, html:not(.js) .spec-dots { display: none; }

/* ---------- docs ---------- */

.doc-layout { display: grid; grid-template-columns: minmax(0, 1fr); gap: 0 40px; align-items: start; padding-top: 26px; }
.doc { min-width: 0; }
.doc h1 {
  margin: 0 0 18px;
  color: var(--ink);
  font: 400 clamp(38px, 4.4vw, 52px)/1.04 var(--serif);
  font-variation-settings: "opsz" 144;
  letter-spacing: -.02em;
}

.guides-head { display: none; }
.guides .group + .group { margin-top: 18px; }
.guides .label { margin: 0 0 6px; }
.guides ol { margin: 0; padding: 0; list-style: none; }
.guides li + li { margin-top: 4px; }
.guides li a { display: block; padding: 7px 0 7px 12px; border-left: 2px solid var(--rule); color: var(--ink); text-decoration: none; }
.guides li a:hover { border-left-color: var(--ink-3); }
.guides .n { margin-right: 6px; color: var(--ink-3); font: 500 11px/1 var(--sans); }
.guides .n:empty { display: none; }
.guides .t { font-size: 15px; font-weight: 500; line-height: 1.3; }
.guides .d { display: block; margin-top: 2px; color: var(--ink-3); font: 400 12px/1.35 var(--sans); }
.guides a[aria-current="page"] { border-left-color: var(--rubric); }
.guides a[aria-current="page"] .t, .guides a[aria-current="page"] .n { color: var(--rubric); }

.prose { color: var(--ink-2); font-size: 17px; line-height: 1.6; overflow-wrap: break-word; }
.prose > p:first-child { font-size: 19px; line-height: 1.55; }
.prose p { margin: 0 0 1em; }
.prose h2 {
  margin: 1.9em 0 .6em;
  padding-top: 14px;
  border-top: 1px solid var(--ink);
  color: var(--ink);
  font: 460 27px/1.2 var(--serif);
  font-variation-settings: "opsz" 48;
  letter-spacing: -.01em;
  scroll-margin-top: 16px;
}
.prose h3 {
  margin: 1.6em 0 .5em;
  color: var(--ink);
  font: 560 19px/1.3 var(--serif);
  font-variation-settings: "opsz" 30;
  scroll-margin-top: 16px;
}
.prose h4, .prose h5, .prose h6 { margin: 1.4em 0 .4em; color: var(--ink); font: 600 15px/1.4 var(--sans); scroll-margin-top: 16px; }
.prose .hn { margin-right: .5em; color: var(--rubric); font: 500 13px/1 var(--sans); letter-spacing: .04em; }
.prose .anchor { margin-left: .2em; color: var(--ink-3); font: 400 .72em/1 var(--sans); text-decoration: none; opacity: 0; }
.prose :is(h2, h3, h4, h5, h6):hover .anchor, .prose .anchor:focus-visible, .prose .anchor[data-copied] { opacity: 1; }
.prose .anchor[data-copied]::after { content: " Link copied"; color: var(--det); font-size: 12px; }
.prose ul, .prose ol { margin: 0 0 1em; padding-left: 1.5em; }
.prose li { margin: .3em 0; }
.prose li > ul, .prose li > ol { margin: .3em 0; }
.prose li::marker { color: var(--ink-3); }
.prose hr { margin: 2em 0; }
.prose strong { color: var(--ink); font-weight: 600; }
.prose :not(pre) > code { font-size: .84em; }
@media (hover: none) { .prose .anchor { opacity: 1; } }

.onpage { display: none; }
.onpage ol, .onpage-inline ol { margin: 0; padding: 0; list-style: none; }
.onpage .label { margin: 0 0 10px; }
.onpage a {
  display: block;
  padding: 4px 0 4px 12px;
  border-left: 1px solid var(--rule);
  color: var(--ink-2);
  font: 400 13px/1.4 var(--sans);
  text-decoration: none;
}
.onpage .lvl-3 a { padding-left: 24px; color: var(--ink-3); }
.onpage a:hover { color: var(--ink); }
.onpage a[aria-current="true"] { padding-left: 11px; border-left: 2px solid var(--rubric); color: var(--rubric); }
.onpage .lvl-3 a[aria-current="true"] { padding-left: 23px; }
.onpage .n { margin-right: 5px; }
/* Both lists scroll inside themselves on wide screens, so keep focus rings inside the links. */
.guides a:focus-visible, .onpage a:focus-visible { outline-offset: -2px; }

.onpage-inline { margin: 0 0 26px; border-top: 1px solid var(--ink); border-bottom: 1px solid var(--rule); }
.onpage-inline summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 40px;
  padding: 8px 0;
  color: var(--ink-2);
  font: 600 12px/1.3 var(--sans);
  list-style: none;
  cursor: pointer;
}
.onpage-inline summary::-webkit-details-marker { display: none; }
.onpage-inline summary::after { content: "+"; color: var(--ink-3); font: 400 18px/1 var(--sans); }
.onpage-inline[open] summary::after { content: "\2212"; }
.onpage-inline ol { padding-bottom: 12px; columns: 2; column-gap: 28px; }
.onpage-inline li { break-inside: avoid; }
.onpage-inline a { display: block; padding: 3px 0; color: var(--ink-2); font: 400 13px/1.4 var(--sans); text-decoration: none; }
.onpage-inline .lvl-3 a { padding-left: 1.2em; color: var(--ink-3); }
.onpage-inline .n { display: inline-block; min-width: 2.8em; color: var(--rubric); }

.pager { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); margin-top: 40px; border-top: 3px double var(--ink); }
.pager a { display: grid; align-content: start; gap: 3px; padding: 14px 16px 14px 0; color: var(--ink); text-decoration: none; }
.pager .next { grid-column: 2; padding: 14px 0 14px 16px; text-align: right; }
.pager .prev + .next { border-left: 1px solid var(--rule); }
.pager .prev .label::before { content: "\2190\00a0"; }
.pager .next .label::after { content: "\00a0\2192"; }
.pager .t { color: var(--ink); font: 500 20px/1.25 var(--serif); font-variation-settings: "opsz" 30; }
.pager a:hover .t { color: var(--rubric); }
.doc-meta { display: flex; flex-wrap: wrap; gap: 4px 20px; margin: 18px 0 0; font: 400 13px/1.4 var(--sans); }
.doc-meta a { color: var(--ink-2); text-decoration-color: var(--rule); }
.doc-meta a:hover { color: var(--rubric); text-decoration-color: currentColor; }
.doc-disclaimer { margin: 12px 0 0; color: var(--ink-3); font-size: 14px; line-height: 1.55; }

/* ---------- drawer backdrop (the drawer itself is .guides, below 720px) ---------- */

.backdrop { position: fixed; z-index: 45; inset: 0; background: rgb(18 14 10 / .45); }
body.drawer-open { overflow: hidden; }

/* ---------- footer ---------- */

.site-foot {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  gap: 8px 24px;
  margin-top: 56px;
  padding: 14px 0 calc(24px + env(safe-area-inset-bottom, 0px));
  border-top: 3px double var(--ink);
  color: var(--ink-3);
  font: 400 12px/1.5 var(--sans);
}
.site-foot p { margin: 0; }
.site-foot nav { display: flex; flex-wrap: wrap; gap: 4px 16px; }
.site-foot a { color: var(--ink-3); text-decoration: none; }
.site-foot a:hover { color: var(--rubric); text-decoration: underline; }

/* ---------- 404 ---------- */

.notfound { position: relative; max-width: 60rem; padding: 72px 0 40px 84px; }
.nf-mark { position: absolute; top: 88px; left: 0; margin: 0; color: var(--rubric); font: 400 26px/1 var(--serif); font-variation-settings: "opsz" 36; }
.notfound h1 {
  margin: 0 0 18px;
  color: var(--ink);
  font: 400 clamp(52px, 8vw, 110px)/.95 var(--serif);
  font-variation-settings: "opsz" 144;
  letter-spacing: -.02em;
}
.notfound .lede { max-width: 36rem; margin-bottom: 24px; }

/* ---------- tablet and up ---------- */

@media (min-width: 720px) {
  .menu-btn, .backdrop { display: none !important; }
  .page-landing .guides, .page-404 .guides { display: none; }
  .doc-layout { grid-template-columns: 210px minmax(0, 1fr); gap: 0 36px; }
  .page-doc .guides {
    position: sticky;
    top: 16px;
    max-height: calc(100vh - 32px);
    overflow-y: auto;
    padding-bottom: 16px;
    scrollbar-width: thin;
  }
}

@media (min-width: 720px) and (max-width: 1199px) {
  .steps { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
  .step-source { grid-column: 1 / -1; border-bottom: 1px solid var(--rule); }
  .step-case { border-left: 0; }
  .site-nav { gap: 14px; }
  .search { flex-basis: 190px; min-width: 120px; }
}

/* ---------- desktop ---------- */

@media (min-width: 1200px) {
  .doc-layout { grid-template-columns: 230px minmax(0, var(--measure)) 210px; justify-content: space-between; gap: 0 48px; }
  .onpage { display: block; position: sticky; top: 16px; max-height: calc(100vh - 32px); overflow-y: auto; padding-bottom: 16px; }
  .onpage-inline { display: none; }
  .contents { columns: 2; }
}

/* ---------- phone ---------- */

@media (max-width: 719px) {
  .site-head { flex-wrap: wrap; gap: 8px 10px; padding-top: calc(10px + env(safe-area-inset-top, 0px)); }
  .brand { min-height: 44px; }
  .crumb { flex: 1 1 0; }
  .site-nav { display: none; }
  .theme-toggle { width: 44px; height: 44px; margin-left: auto; }
  .menu-btn { height: 44px; padding: 0 14px; }
  .search { flex: 1 0 100%; order: 5; min-width: 0; }
  .search input { height: 44px; font-size: 16px; }
  .search kbd { display: none; }
  .search-results { left: 0; width: auto; }
  .search-results a { min-height: 44px; }

  .btn { height: auto; min-height: 44px; padding: 0 18px; }
  .copy { min-height: 32px; padding: 4px 10px; }
  .copy::after { content: ""; position: absolute; top: 50%; left: 50%; width: max(100%, 44px); height: 44px; transform: translate(-50%, -50%); }
  .code pre { font-size: 12.5px; }
  .table-wrap table { font-size: 14px; }
  .prose .anchor { display: inline-block; min-width: 24px; text-align: center; }

  .hero { padding: 26px 0 26px; }
  .hero-top { grid-template-columns: minmax(0, 1fr); gap: 14px; margin-bottom: 24px; }
  .lede { font-size: 17px; }
  .band { padding: 24px 0 26px; }
  .band h2 { font-size: 25px; }
  .three { grid-template-columns: minmax(0, 1fr); }
  .three > div, .three > div + div { padding: 14px 0 12px; border-left: 0; }
  .three > div + div { border-top: 1px solid var(--rule); }
  .legend li { grid-template-columns: 18px minmax(0, 1fr); align-items: baseline; row-gap: 10px; padding: 12px 14px; }
  .legend .stamp { grid-column: 2; justify-self: start; }
  .legend .stamp::after { content: ""; position: absolute; inset: -11px -6px; }
  .contents { columns: 1; }
  .contents a { flex-wrap: wrap; row-gap: 0; min-height: 44px; padding: 8px 0; }
  .contents .lead { display: none; }
  .contents .d { flex-basis: 100%; padding-left: 48px; text-align: left; }

  .specimen { overflow: visible; border: 0; border-radius: 0; background: none; }
  .spec-tabs { display: none; }
  .spec-panels {
    display: flex;
    gap: 12px;
    margin-inline: calc(-1 * max(var(--gutter), env(safe-area-inset-left, 0px)));
    padding-inline: max(var(--gutter), env(safe-area-inset-left, 0px));
    overflow-x: auto;
    overscroll-behavior-x: contain;
    scroll-snap-type: x mandatory;
    scroll-padding-inline: max(var(--gutter), env(safe-area-inset-left, 0px));
    scrollbar-width: none;
  }
  .spec-panels::-webkit-scrollbar { display: none; }
  .spec-panel {
    display: flex;
    flex: 0 0 86%;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
    border: 1px solid var(--ink);
    border-radius: var(--r);
    background: var(--paper);
    scroll-snap-align: start;
  }
  .spec-panel + .spec-panel { border-top: 1px solid var(--ink); }
  .spec-cmd, .step-h { display: none; }
  .spec-label {
    display: block;
    margin: 0;
    padding: 10px 14px;
    border-bottom: 1px solid var(--rule);
    background: var(--paper-2);
    color: var(--ink);
    font: 600 13px/1.3 var(--sans);
  }
  .steps { display: flex; flex-direction: column; }
  .step { padding: 14px; }
  .step + .step { border-left: 0; }
  .step-outcome { order: -1; }
  .step-source { border-top: 1px solid var(--rule); }
  .step-case { display: none; }
  /* A short excerpt that never scrolls sideways inside a swipeable card. */
  .step-source pre { max-height: calc(17 * 1.62em + 25px); overflow: hidden; font-size: 11.5px; }
  .spec-dots { display: flex; justify-content: center; gap: 8px; margin-top: 14px; }
  .spec-dots i { width: 7px; height: 7px; border-radius: 50%; background: var(--rule); }
  .spec-dots i.on { background: var(--rubric); }
  .spec-foot { padding: 10px 0 0; border-top: 0; text-align: center; }

  .doc-layout { padding-top: 18px; }
  .guides { margin-bottom: 22px; padding-bottom: 14px; border-bottom: 1px solid var(--rule); }
  .guides li a { min-height: 44px; }
  .onpage-inline ol { columns: 1; }
  .onpage-inline a { display: flex; align-items: center; min-height: 44px; }
  .prose > p:first-child { font-size: 18px; }
  .prose h2 { font-size: 23px; }
  .prose h3 { font-size: 18px; }
  .pager .t { font-size: 17px; }
  .doc-meta a, .site-foot a { display: inline-flex; align-items: center; min-height: 44px; }
  .site-foot { margin-top: 40px; }

  .notfound { padding: 40px 0 24px; }
  .nf-mark { position: static; margin-bottom: 14px; }

  /* With JavaScript the guides become an off-canvas drawer; without it they stay inline. */
  html.js .guides {
    position: fixed;
    z-index: 50;
    top: 0;
    bottom: 0;
    left: 0;
    width: min(320px, 86vw);
    margin: 0;
    padding: calc(12px + env(safe-area-inset-top, 0px)) 18px calc(24px + env(safe-area-inset-bottom, 0px)) max(18px, env(safe-area-inset-left, 0px));
    overflow-y: auto;
    overscroll-behavior: contain;
    border-right: 1px solid var(--ink);
    border-bottom: 0;
    background: var(--paper);
    box-shadow: 20px 0 40px rgb(0 0 0 / .25);
    transform: translateX(-104%);
    visibility: hidden;
    transition: transform .22s ease, visibility 0s linear .22s;
  }
  html.js body.drawer-open .guides { transform: none; visibility: visible; transition: transform .22s ease; }
  html.js .guides-head { display: flex; align-items: center; justify-content: space-between; margin: 0 0 10px; }
}

/* ---------- motion and print ---------- */

@media (prefers-reduced-motion: no-preference) {
  html { scroll-behavior: smooth; }
}
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after { transition: none !important; animation: none !important; scroll-behavior: auto !important; }
}

@media print {
  body::before, .skip, .site-nav, .search, .theme-toggle, .menu-btn, .guides, .onpage, .onpage-inline,
  .backdrop, .copy, .anchor, .spec-tabs, .spec-dots, .pager, .doc-meta { display: none !important; }
  *, *::before, *::after { background: transparent !important; box-shadow: none !important; color: #000 !important; }
  html { color-scheme: light !important; background: #fff !important; }
  body { padding: 0; }
  .doc-layout { display: block; }
  .spec-panel[hidden] { display: block !important; }
  .code pre, .spec-cmd code, .cmd code { overflow: visible; white-space: pre-wrap; }
  .table-wrap { overflow: visible; }
  .prose a[href^="http"]::after { content: " (" attr(href) ")"; font-size: .85em; }
  h2, h3 { break-after: avoid; }
  .code, table, blockquote, .legend li { break-inside: avoid; }
}
```

- [ ] **Step 4: Write the favicon**

Create `site/favicon.svg`:

```xml
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32">
  <style>
    .shadow { fill: #a3281d; }
    .ink { fill: #1c1813; }
    .paper { fill: #f7f2e8; }
    @media (prefers-color-scheme: dark) {
      .shadow { fill: #e0725f; }
      .ink { fill: #efe6d6; }
      .paper { fill: #15120e; }
    }
  </style>
  <rect class="shadow" x="6" y="6" width="24" height="24" rx="4"/>
  <rect class="ink" x="2" y="2" width="24" height="24" rx="4"/>
  <rect class="paper" x="4" y="4" width="20" height="20" rx="2.5"/>
  <path class="ink" d="M8 8h12v6h-1.75v-3.25H14v3.5h4v2.75h-4V19h2v2H8v-2h2V11H8z"/>
</svg>
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline --test design_tokens`
Expected: PASS (`test result: ok. 4 passed; 0 failed`).

Run: `cargo clippy -p xtask --offline --all-targets -- -D warnings && cargo fmt -p xtask -- --check`
Expected: clippy finishes with no warnings; `cargo fmt --check` prints nothing.

- [ ] **Step 6: Visual check**

The favicon can be checked now. Render it at 16, 32, and 64 px on both a light and a dark strip:

```bash
P="${TMPDIR:-/tmp}/fidryn-favicon"; mkdir -p "$P"
printf '<!doctype html><body style="margin:0;padding:12px;background:#f7f2e8"><img src="file://%s/site/favicon.svg" width="16"> <img src="file://%s/site/favicon.svg" width="32"> <img src="file://%s/site/favicon.svg" width="64"><div style="background:#2b2b2e;padding:8px;margin-top:8px"><img src="file://%s/site/favicon.svg" width="16"> <img src="file://%s/site/favicon.svg" width="32"></div>' "$PWD" "$PWD" "$PWD" "$PWD" "$PWD" > "$P/favicon.html"
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --disable-gpu --hide-scrollbars --no-first-run \
  --disable-component-update --disable-background-networking --disable-sync --disable-default-apps --disable-extensions \
  --allow-file-access-from-files --user-data-dir="$P/profile" --force-device-scale-factor=2 --window-size=220,170 \
  --screenshot="$P/favicon.png" "file://$P/favicon.html"
rm -rf "$P/profile"
```

Look at `$P/favicon.png` (the command above wrote it under `${TMPDIR:-/tmp}/fidryn-favicon/`). Look for: a square with rounded corners and a bold slab-serif F centered in it, with a shadow offset down and to the right; the F still reads as an F at 16 px. The icon follows the operating system's color scheme: in light mode an ink outline and F on a cream face with a rubric shadow; in dark mode a cream outline and F on a near-black face with a coral shadow.

The pages that use the stylesheet are first generated in Task 12. After Task 12, serve the site (`python3 -m http.server 8000 --bind 127.0.0.1 --directory site`) and check `http://127.0.0.1:8000/index.html`, `/docs/outcomes.html`, `/docs/examples.html`, and `/404.html` at 1440, 820, and 390 px wide, in light and in dark (toggle, or the system setting). Look for exactly these:

- Header: the F monogram (soft ink square, 2px rubric offset shadow) beside "Fidryn" in Fraunces; Docs, Examples, GitHub; a paper-2 search field with a `/` key hint; a round theme toggle; a double rule under the header. At 390 px: brand, toggle, and Menu on one row and a full-width search field below.
- Landing hero at 1440: rubric eyebrow and "No false determinacy." (Fraunces book weight, about 80px) on the left; lede, an ink "Get started" and an ink-outlined "Read the docs" (6px corners), and the install command between two hairlines on the right.
- Specimen at 1440: an ink-bordered box; four tabs with the selected one in ink with paper text; a tinted command bar; Source (with real line numbers), Case, and Outcome (stamp, then ruled sentences, the "Outside scope" sentence muted) side by side; a footer strip. At 820: Source full width, Case and Outcome side by side. At 390: swipeable cards (Outcome first, then a source excerpt; no Case), and a row of dots.
- Landing body: `§ 2 —` marks in rubric before each heading; the three-up with rules; the six questions as bordered rows with a rubric `?` and a bordered small-caps stamp; the contents list with dotted leaders (two columns at 1440, one below); the footer under a double rule.
- Docs at 1440: guides sidebar with a 2px bar per guide and the current guide in rubric; kicker in rubric; `h2` with a rule above and a rubric number; framed code with a tinted caption bar and Copy; fully gridded tables with a tinted header row; any blockquote (where a guide has one) on a rubric-tinted ground with a rubric left bar; the "On this page" rail on the right; the two-column pager under a double rule. At 820: two columns and an "On this page" disclosure. At 390: the Menu button opens a left drawer over a dimmed page.
- Labels, nav, buttons, code captions, and table headers are sentence case (S6C); only the stamps are small caps (S10A).
- Dark: warm near-black ground, cream text, coral rubric; the selected specimen tab is cream with dark text.
- Paper grain: a faint noise on empty areas (zoom in to see it); none inside code frames or the specimen.
- At 390 px no page scrolls sideways; wide tables (the Examples guide) and long commands (the CLI guide) scroll inside their own frame.

- [ ] **Step 7: Commit**

```bash
git add site/assets/fidryn.css site/favicon.svg xtask/tests/design_tokens.rs
git -c commit.gpgsign=false commit -m "feat(site): add the Statute design system and favicon"
```

### Task 6: Site behavior

The site's only script, `site/assets/fidryn.js`: theme toggle, copy buttons, heading-link copy, the phone guides drawer, dropdown search over `search-index.json`, the on-this-page scrollspy, and the landing specimen tabs and swipe cards. Pure search helpers are tested under Node, and `cargo xtask ci` runs those tests when Node is installed.

**Files:**
- Create: `site/assets/fidryn.js`
- Create: `web/tests/site.test.js`
- Modify: `site/package.json` (drop `"type": "module"`; Task 12 deletes the file)
- Modify: `xtask/src/workspace.rs` (imports; new `js_test_files` and `run_js_tests` above `check_status`; a `tests` module at the end)
- Modify: `xtask/src/ci.rs` (import `run_js_tests`; call it on the line before `    if workspace_has_benches(&metadata) {`)
- Test: `web/tests/site.test.js`, and unit tests in `xtask/src/workspace.rs`

**Interfaces:**
- Consumes: the C5 markup hooks (`#site-search`, `#search-results`, `[data-theme-toggle]`, `[data-copy]`, `.anchor`, `#drawer`, `[data-drawer-open]`, `[data-drawer-close]`, `.backdrop`, `.onpage a`, `[data-specimen]` with its `[role="tab"]` buttons, `.spec-panels`, `.spec-dots i`), the C6 index entries `{"u","n","h","p","t"}` served at `/search-index.json`, and the `html.js` class plus the `fidryn-theme` value that the base template's inline head script sets. The CSS states it toggles are styled by Task 5.
- Produces: `site/assets/fidryn.js`, which in Node (no `document`) exports exactly `{ tokens, score, rank }`:
  - `tokens(query) -> string[]`: lowercase, split on anything outside `[a-z0-9_]`, empties dropped.
  - `score(entry, qs) -> number`: for each token, 10 if it prefixes a word of `h`, plus 4 for `p`, plus 1 for `t`; 0 as soon as one token matches nothing (and for an empty token list).
  - `rank(index, query) -> entry[]`: at most 8 of the index's own entries with a positive score, best first, ties in index order; `[]` for a query with no tokens.
  - `pub fn run_js_tests() -> Result<()>` in `xtask/src/workspace.rs`: runs `node --test` from the workspace root with every `web/tests/*.test.js` path, sorted (so Tasks 15–16's `web/tests/mill.test.js` joins automatically); prints `node not found; skipping JS tests in web/tests` and returns `Ok(())` when `node --version` cannot be spawned. `ci::run` calls it after the schema probes.

- [ ] **Step 1: Write the failing test**

Create `web/tests/site.test.js`:

```js
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `node --test web/tests/site.test.js`
Expected: FAIL; the output contains `Error: Cannot find module '../../site/assets/fidryn.js'` and the summary shows `# fail 1`.

- [ ] **Step 3: Let Node load the script as CommonJS**

`site/package.json` (the retired Node docs build, deleted in Task 12) declares `"type": "module"`, which makes Node load every `.js` file under `site/` as an ES module, so `require("../../site/assets/fidryn.js")` fails with `ERR_REQUIRE_ESM`. The old build script is `site/scripts/build-docs.mjs`, which is an ES module by its extension either way. In `site/package.json`, replace:

```json
  "private": true,
  "type": "module",
```

with:

```json
  "private": true,
```

- [ ] **Step 4: Write the script**

Create `site/assets/fidryn.js`:

```js
// Fidryn site enhancements: theme, copy buttons, heading links, the guides
// drawer, docs search, the on-this-page scrollspy, and the landing specimen.
// Every page works without this file; it only enhances.
(function () {
  "use strict";

  var MAX_RESULTS = 8;
  var WEIGHT_HEADING = 10;
  var WEIGHT_PAGE = 4;
  var WEIGHT_TEXT = 1;

  // Lowercased search words: runs of [a-z0-9_]. Everything else separates
  // words, so regex metacharacters in a query are plain separators.
  function tokens(query) {
    return String(query == null ? "" : query)
      .toLowerCase()
      .split(/[^a-z0-9_]+/)
      .filter(function (t) { return t.length > 0; });
  }

  function hasPrefix(words, token) {
    for (var i = 0; i < words.length; i++) {
      if (words[i].lastIndexOf(token, 0) === 0) return true;
    }
    return false;
  }

  // Sum of field weights over every token; 0 when any token matches no word.
  function score(entry, qs) {
    if (!entry || !qs || qs.length === 0) return 0;
    var h = tokens(entry.h);
    var p = tokens(entry.p);
    var t = tokens(entry.t);
    var total = 0;
    for (var i = 0; i < qs.length; i++) {
      var s = 0;
      if (hasPrefix(h, qs[i])) s += WEIGHT_HEADING;
      if (hasPrefix(p, qs[i])) s += WEIGHT_PAGE;
      if (hasPrefix(t, qs[i])) s += WEIGHT_TEXT;
      if (s === 0) return 0;
      total += s;
    }
    return total;
  }

  // Up to eight entries with a positive score, best first; ties keep index order.
  function rank(index, query) {
    var qs = tokens(query);
    if (qs.length === 0 || !Array.isArray(index)) return [];
    var hits = [];
    for (var i = 0; i < index.length; i++) {
      var s = score(index[i], qs);
      if (s > 0) hits.push({ entry: index[i], score: s, order: i });
    }
    hits.sort(function (a, b) { return b.score - a.score || a.order - b.order; });
    return hits.slice(0, MAX_RESULTS).map(function (hit) { return hit.entry; });
  }

  if (typeof document === "undefined") {
    module.exports = { tokens: tokens, score: score, rank: rank };
    return;
  }

  var root = document.documentElement;
  var THEME_KEY = "fidryn-theme";
  var PHONE = "(max-width: 719px)";
  var WIDE = "(min-width: 720px)";

  function all(selector, scope) {
    return Array.prototype.slice.call((scope || document).querySelectorAll(selector));
  }

  function onMedia(mq, fn) {
    if (mq.addEventListener) mq.addEventListener("change", fn);
    else if (mq.addListener) mq.addListener(fn);
  }

  function isTextField(el) {
    if (!el) return false;
    if (el.isContentEditable) return true;
    var tag = el.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT";
  }

  // ---- clipboard ----

  function copyWithSelection(text) {
    var area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.top = "-1000px";
    document.body.appendChild(area);
    area.select();
    var ok = false;
    try { ok = document.execCommand("copy"); } catch (e) { ok = false; }
    document.body.removeChild(area);
    return ok;
  }

  function copyText(text) {
    var fallback = function () {
      if (!copyWithSelection(text)) throw new Error("copy failed");
    };
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(text).catch(fallback);
    }
    return new Promise(function (resolve) { fallback(); resolve(); });
  }

  function flagCopied(el, label) {
    if (el._copiedTimer) clearTimeout(el._copiedTimer);
    var original = el._copyLabel || el.textContent;
    el._copyLabel = original;
    el.setAttribute("data-copied", "");
    if (label) el.textContent = label;
    el._copiedTimer = setTimeout(function () {
      el.removeAttribute("data-copied");
      if (label) el.textContent = original;
      el._copiedTimer = null;
    }, 1400);
  }

  function setupCopy() {
    all("[data-copy]").forEach(function (button) {
      var host = button.closest(".code, .cmd");
      var code = host && host.querySelector("code");
      if (!code) return;
      button.hidden = false;
      button.addEventListener("click", function () {
        copyText(code.textContent).then(function () { flagCopied(button, "Copied"); }, function () {});
      });
    });
    all(".anchor").forEach(function (anchor) {
      anchor.addEventListener("click", function () {
        copyText(anchor.href).then(function () { flagCopied(anchor, null); }, function () {});
      });
    });
  }

  // ---- theme ----

  function setupTheme() {
    var toggles = all("[data-theme-toggle]");
    if (toggles.length === 0) return;
    var dark = window.matchMedia ? window.matchMedia("(prefers-color-scheme: dark)") : null;
    function current() {
      var t = root.getAttribute("data-theme");
      if (t === "light" || t === "dark") return t;
      return dark && dark.matches ? "dark" : "light";
    }
    var metas = all("meta[name='theme-color']");
    function paint() {
      var label = current() === "dark" ? "Switch to light theme" : "Switch to dark theme";
      toggles.forEach(function (b) { b.setAttribute("aria-label", label); });
      // An explicit choice overrides the system scheme, so the browser chrome follows it too.
      if (root.hasAttribute("data-theme")) {
        var paper = window.getComputedStyle(root).getPropertyValue("--paper").trim();
        if (paper) metas.forEach(function (m) { m.setAttribute("content", paper); });
      }
    }
    toggles.forEach(function (b) {
      b.hidden = false;
      b.addEventListener("click", function () {
        var next = current() === "dark" ? "light" : "dark";
        root.setAttribute("data-theme", next);
        try { localStorage.setItem(THEME_KEY, next); } catch (e) { /* storage unavailable */ }
        paint();
      });
    });
    if (dark) onMedia(dark, paint);
    paint();
  }

  // ---- guides drawer (phones) ----

  function setupDrawer() {
    var drawer = document.getElementById("drawer");
    var opener = document.querySelector("[data-drawer-open]");
    var backdrop = document.querySelector(".backdrop");
    if (!drawer || !opener) return;
    var phone = window.matchMedia(PHONE);
    opener.hidden = false;

    function isOpen() { return document.body.classList.contains("drawer-open"); }
    function focusables() {
      return all("a[href], button:not([disabled]), [tabindex]:not([tabindex='-1'])", drawer)
        .filter(function (el) { return el.offsetWidth > 0 || el.offsetHeight > 0; });
    }
    function open() {
      document.body.classList.add("drawer-open");
      if (backdrop) backdrop.hidden = false;
      opener.setAttribute("aria-expanded", "true");
      var first = drawer.querySelector("[data-drawer-close]") || focusables()[0];
      if (first) first.focus();
    }
    function close(restoreFocus) {
      if (!isOpen()) return;
      document.body.classList.remove("drawer-open");
      if (backdrop) backdrop.hidden = true;
      opener.setAttribute("aria-expanded", "false");
      if (restoreFocus) opener.focus();
    }

    opener.addEventListener("click", function () {
      if (isOpen()) close(true); else open();
    });
    all("[data-drawer-close]").forEach(function (el) {
      el.addEventListener("click", function () { close(true); });
    });
    drawer.addEventListener("click", function (e) {
      if (e.target.closest && e.target.closest("a[href]")) close(false);
    });
    document.addEventListener("keydown", function (e) {
      if (!isOpen()) return;
      if (e.key === "Escape") {
        e.preventDefault();
        close(true);
        return;
      }
      if (e.key !== "Tab") return;
      var items = focusables();
      if (items.length === 0) return;
      var first = items[0];
      var last = items[items.length - 1];
      if (!drawer.contains(document.activeElement)) {
        e.preventDefault();
        first.focus();
      } else if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    });
    onMedia(phone, function () { if (!phone.matches) close(false); });
  }

  // ---- search ----

  function highlight(target, text, qs) {
    var re = /[A-Za-z0-9_]+/g;
    var last = 0;
    var m;
    while ((m = re.exec(text)) !== null) {
      var word = m[0].toLowerCase();
      var len = 0;
      for (var i = 0; i < qs.length; i++) {
        if (qs[i].length > len && word.lastIndexOf(qs[i], 0) === 0) len = qs[i].length;
      }
      if (len === 0) continue;
      if (m.index > last) target.appendChild(document.createTextNode(text.slice(last, m.index)));
      var mark = document.createElement("mark");
      mark.textContent = text.slice(m.index, m.index + len);
      target.appendChild(mark);
      last = m.index + len;
    }
    if (last < text.length) target.appendChild(document.createTextNode(text.slice(last)));
  }

  // Only same-site paths from the index become links.
  function isSitePath(url) {
    return typeof url === "string" && url.charAt(0) === "/" && url.charAt(1) !== "/";
  }

  function span(className, text) {
    var el = document.createElement("span");
    el.className = className;
    if (text != null) el.textContent = text;
    return el;
  }

  function setupSearch() {
    var input = document.getElementById("site-search");
    var list = document.getElementById("search-results");
    if (!input || !list) return;
    var form = input.form;
    var index = null;
    var loading = null;
    var failed = false;
    var results = [];
    var active = -1;

    function load() {
      if (loading) return loading;
      loading = fetch("/search-index.json", { credentials: "same-origin" })
        .then(function (r) {
          if (!r.ok) throw new Error("HTTP " + r.status);
          return r.json();
        })
        .then(function (data) {
          index = Array.isArray(data) ? data : [];
          render();
        }, function () {
          failed = true;
          render();
        });
      return loading;
    }

    function isShown() { return !list.hidden; }
    function show() {
      list.hidden = false;
      input.setAttribute("aria-expanded", "true");
    }
    function hide() {
      list.hidden = true;
      input.setAttribute("aria-expanded", "false");
      input.removeAttribute("aria-activedescendant");
      active = -1;
    }
    function message(text) {
      var li = document.createElement("li");
      li.className = "empty";
      li.setAttribute("role", "option");
      li.setAttribute("aria-disabled", "true");
      li.setAttribute("aria-selected", "false");
      li.textContent = text;
      list.appendChild(li);
    }

    function render() {
      var query = input.value;
      list.textContent = "";
      results = [];
      active = -1;
      input.removeAttribute("aria-activedescendant");
      if (query.trim() === "") {
        hide();
        return;
      }
      if (failed) {
        message("Search is unavailable right now.");
      } else if (!index) {
        message("Loading the index…");
      } else {
        var qs = tokens(query);
        results = rank(index, query).filter(function (entry) { return isSitePath(entry.u); });
        if (results.length === 0) message("No matching sections.");
        results.forEach(function (entry, i) {
          var li = document.createElement("li");
          li.id = "sr-" + i;
          li.setAttribute("role", "option");
          li.setAttribute("aria-selected", "false");
          var a = document.createElement("a");
          a.href = entry.u;
          a.tabIndex = -1;
          a.appendChild(span("n", entry.n || ""));
          var h = span("h", null);
          highlight(h, String(entry.h || ""), qs);
          a.appendChild(h);
          var page = String(entry.p || "");
          var text = String(entry.t || "");
          a.appendChild(span("t", text ? page + " · " + text : page));
          li.appendChild(a);
          li.addEventListener("mousemove", function () { if (active !== i) select(i); });
          list.appendChild(li);
        });
      }
      show();
    }

    function select(i) {
      var items = all("li[role='option']:not(.empty)", list);
      if (items.length === 0) return;
      active = (i + items.length) % items.length;
      items.forEach(function (li, j) { li.setAttribute("aria-selected", j === active ? "true" : "false"); });
      input.setAttribute("aria-activedescendant", items[active].id);
      var el = items[active];
      if (el.scrollIntoView) el.scrollIntoView({ block: "nearest" });
    }

    function openActive() {
      var entry = results[active >= 0 ? active : 0];
      if (entry) window.location.href = entry.u;
    }

    input.addEventListener("focus", function () {
      load();
      if (input.value.trim() !== "") render();
    });
    input.addEventListener("input", function () {
      load();
      render();
    });
    input.addEventListener("keydown", function (e) {
      if (e.key === "ArrowDown" || e.key === "ArrowUp") {
        if (!isShown()) render();
        if (results.length === 0) return;
        e.preventDefault();
        if (e.key === "ArrowDown") select(active < 0 ? 0 : active + 1);
        else select(active < 0 ? results.length - 1 : active - 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        openActive();
      } else if (e.key === "Escape") {
        if (isShown()) {
          e.preventDefault();
          hide();
        }
      }
    });
    if (form) {
      form.addEventListener("submit", function (e) {
        e.preventDefault();
        openActive();
      });
    }
    document.addEventListener("click", function (e) {
      if (isShown() && !(form || input).contains(e.target)) hide();
    });
    document.addEventListener("keydown", function (e) {
      var slash = e.key === "/" && !e.ctrlKey && !e.metaKey && !e.altKey;
      var cmdK = (e.key === "k" || e.key === "K") && (e.ctrlKey || e.metaKey) && !e.altKey;
      if (!slash && !cmdK) return;
      if (isTextField(document.activeElement)) return;
      if (document.body.classList.contains("drawer-open")) return;
      e.preventDefault();
      input.focus();
      input.select();
    });
  }

  // ---- on-this-page scrollspy ----

  function setupScrollspy() {
    var links = all(".onpage a[href^='#']");
    if (links.length === 0 || !("IntersectionObserver" in window)) return;
    var byId = {};
    var headings = [];
    links.forEach(function (a) {
      var id = a.getAttribute("href").slice(1);
      try { id = decodeURIComponent(id); } catch (e) { /* keep the raw fragment */ }
      var h = document.getElementById(id);
      if (h) {
        byId[id] = a;
        headings.push(h);
      }
    });
    if (headings.length === 0) return;
    var visible = {};
    var currentId = null;
    function mark(id) {
      if (id === currentId) return;
      currentId = id;
      links.forEach(function (a) { a.removeAttribute("aria-current"); });
      if (id && byId[id]) byId[id].setAttribute("aria-current", "true");
    }
    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        var id = entry.target.id;
        if (entry.isIntersecting) {
          visible[id] = true;
        } else {
          delete visible[id];
          // Scrolling up past the current heading hands the mark to the one before it.
          if (id === currentId && entry.boundingClientRect.top > 0) {
            var i = headings.indexOf(entry.target);
            mark(i > 0 ? headings[i - 1].id : null);
          }
        }
      });
      for (var i = 0; i < headings.length; i++) {
        if (visible[headings[i].id]) {
          mark(headings[i].id);
          break;
        }
      }
    }, { rootMargin: "0px 0px -70% 0px" });
    headings.forEach(function (h) { observer.observe(h); });
  }

  // ---- landing specimen ----

  function setupSpecimen(box) {
    var tabs = all("[role='tab']", box);
    var panels = tabs.map(function (t) { return document.getElementById(t.getAttribute("aria-controls")); });
    if (tabs.length === 0 || panels.some(function (p) { return !p; })) return;
    var scroller = box.querySelector(".spec-panels");
    var dots = all(".spec-dots i", box);
    var wide = window.matchMedia(WIDE);
    var current = Math.max(0, tabs.findIndex(function (t) { return t.getAttribute("aria-selected") === "true"; }));

    function select(i, focus) {
      current = i;
      tabs.forEach(function (t, j) {
        t.setAttribute("aria-selected", j === i ? "true" : "false");
        t.tabIndex = j === i ? 0 : -1;
      });
      if (wide.matches) panels.forEach(function (p, j) { p.hidden = j !== i; });
      dots.forEach(function (d, j) { d.classList.toggle("on", j === i); });
      if (focus) tabs[i].focus();
    }

    function syncFromScroll() {
      if (wide.matches || !scroller) return;
      var frame = scroller.getBoundingClientRect();
      var centre = frame.left + frame.width / 2;
      var best = 0;
      var bestDistance = Infinity;
      panels.forEach(function (p, j) {
        var r = p.getBoundingClientRect();
        var d = Math.abs(r.left + r.width / 2 - centre);
        if (d < bestDistance) {
          bestDistance = d;
          best = j;
        }
      });
      if (best !== current) select(best, false);
    }

    function apply() {
      if (wide.matches) {
        select(current, false);
      } else {
        panels.forEach(function (p) { p.hidden = false; });
        if (scroller) scroller.scrollLeft = panels[current].offsetLeft - panels[0].offsetLeft;
        syncFromScroll();
        dots.forEach(function (d, j) { d.classList.toggle("on", j === current); });
      }
    }

    tabs.forEach(function (t, i) {
      t.addEventListener("click", function () { select(i, false); });
    });
    box.querySelector("[role='tablist']").addEventListener("keydown", function (e) {
      var i = tabs.indexOf(document.activeElement);
      if (i < 0) return;
      var next = null;
      if (e.key === "ArrowRight") next = (i + 1) % tabs.length;
      else if (e.key === "ArrowLeft") next = (i - 1 + tabs.length) % tabs.length;
      else if (e.key === "Home") next = 0;
      else if (e.key === "End") next = tabs.length - 1;
      if (next === null) return;
      e.preventDefault();
      select(next, true);
    });
    if (scroller) {
      var pending = false;
      scroller.addEventListener("scroll", function () {
        if (pending) return;
        pending = true;
        window.requestAnimationFrame(function () {
          pending = false;
          syncFromScroll();
        });
      }, { passive: true });
    }
    onMedia(wide, apply);
    apply();
  }

  function start() {
    setupTheme();
    setupCopy();
    setupDrawer();
    setupSearch();
    setupScrollspy();
    all("[data-specimen]").forEach(setupSpecimen);
  }

  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", start);
  else start();
})();
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `node --check site/assets/fidryn.js && node --test web/tests/site.test.js`
Expected: PASS; the summary ends with `# pass 1` and `# fail 0` on Node 18 (the file is one test with 10 passing subtests; Node 20 and later report `# pass 10`).

- [ ] **Step 6: Write the failing Rust test for the test-file listing**

Append to the end of `xtask/src/workspace.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::js_test_files;
    use std::fs;
    use std::path::PathBuf;

    /// A fresh directory under the system temp dir for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("clear scratch dir");
        }
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn js_test_files_lists_only_test_scripts_sorted() {
        let root = scratch("js-tests");
        let tests = root.join("web").join("tests");
        fs::create_dir_all(tests.join("fixtures.test.js"))
            .expect("create a directory named like a test");
        for name in ["site.test.js", "mill.test.js", "helpers.js", "notes.md"] {
            fs::write(tests.join(name), "").expect("write file");
        }
        let files = js_test_files(&root).expect("list tests");
        fs::remove_dir_all(&root).expect("remove scratch dir");
        assert_eq!(
            files,
            vec![
                PathBuf::from("web/tests/mill.test.js"),
                PathBuf::from("web/tests/site.test.js"),
            ]
        );
    }

    #[test]
    fn js_test_files_is_empty_without_web_tests() {
        let root = scratch("no-js-tests");
        let files = js_test_files(&root).expect("list tests");
        fs::remove_dir_all(&root).expect("remove scratch dir");
        assert!(files.is_empty(), "{files:?}");
    }
}
```

- [ ] **Step 7: Run the test to verify it fails**

Run: `cargo test -p xtask --offline js_test_files`
Expected: FAIL to compile with ``error[E0432]: unresolved import `super::js_test_files` ``.

- [ ] **Step 8: Implement the JS test step and wire it into CI**

In `xtask/src/workspace.rs`, replace:

```rust
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
```

with:

```rust
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
```

In the same file, replace the line:

```rust
fn check_status(status: &ExitStatus) -> Result<()> {
```

with:

```rust
/// `web/tests/*.test.js` under `root`, sorted, relative to `root`. Empty when
/// the directory does not exist.
fn js_test_files(root: &Path) -> Result<Vec<PathBuf>> {
    let dir = root.join("web").join("tests");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("read {}", dir.display()))?;
        let name = entry.file_name();
        let is_test = name.to_str().is_some_and(|n| n.ends_with(".test.js"));
        if is_test && entry.path().is_file() {
            files.push(Path::new("web").join("tests").join(name));
        }
    }
    files.sort();
    Ok(files)
}

/// Run the JavaScript unit tests in `web/tests/` with `node --test`.
///
/// Node is optional for this workspace: nothing is built or served with it.
/// When `node --version` cannot be spawned the step is skipped.
pub fn run_js_tests() -> Result<()> {
    let probe = Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if probe.is_err() {
        eprintln!("node not found; skipping JS tests in web/tests");
        return Ok(());
    }
    let root = workspace_root();
    let files = js_test_files(&root)?;
    if files.is_empty() {
        eprintln!("no JS tests in web/tests; skipping");
        return Ok(());
    }
    let mut cmd = Command::new("node");
    cmd.current_dir(&root);
    cmd.arg("--test");
    cmd.args(&files);
    run_command(cmd)
}

fn check_status(status: &ExitStatus) -> Result<()> {
```

In `xtask/src/ci.rs`, replace:

```rust
use crate::workspace::{
    cargo_command, load_metadata, run_command, run_schema_probes, workspace_has_benches,
};
```

with:

```rust
use crate::workspace::{
    cargo_command, load_metadata, run_command, run_js_tests, run_schema_probes,
    workspace_has_benches,
};
```

and replace:

```rust
    if workspace_has_benches(&metadata) {
```

with:

```rust
    run_js_tests()?;
    if workspace_has_benches(&metadata) {
```

- [ ] **Step 9: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline js_test_files`
Expected: PASS (`test result: ok. 2 passed; 0 failed` for the `xtask` unit tests).

Run: `cargo test -p xtask --offline`
Expected: PASS; every `test result:` line reports `0 failed`.

Run: `cargo clippy -p xtask --offline --all-targets -- -D warnings && cargo fmt -p xtask -- --check`
Expected: no warnings; `cargo fmt --check` prints nothing.

- [ ] **Step 10: Behavior check**

The pages that load the script are first generated in Task 12. After Task 12, serve the site (`python3 -m http.server 8000 --bind 127.0.0.1 --directory site`), keep the browser console open, and check:

- `http://127.0.0.1:8000/index.html` at 1440 px: clicking the second specimen tab shows only its run; with a tab focused, ArrowRight, ArrowLeft (wrapping from the first to the last), Home, and End move both selection and focus, and only the selected tab is in the Tab order. Narrow the window below 720 px: all four runs become swipeable cards, and the dot under the cards follows the card in view. Widen again: the tab for the card last in view is selected.
- Press `/` (focus not in a field): the search field takes focus. Type `outc`: a dropdown opens under the field with matched prefixes marked; ArrowDown moves the highlighted row; Enter opens it; Escape closes the dropdown. `c++` lists results without errors; `(` shows "No matching sections."; Ctrl-K or Cmd-K also focuses the field; clicking outside closes the dropdown.
- The theme toggle switches to dark and back; after a reload the choice is kept; its label reads "Switch to light theme" while dark.
- The install command's Copy button shows "Copied" for about 1.4 s; a heading's `#` link on a docs page shows "Link copied".
- `/docs/outcomes.html` at 1440 px: scrolling marks the section being read in the "On this page" rail.
- `/docs/outcomes.html` at 390 px: Menu opens the drawer with focus on Close; Tab and Shift-Tab stay inside the drawer; Escape, the backdrop, and any link close it; after Escape focus is back on Menu.
- The console shows no errors on any page.

- [ ] **Step 11: Commit**

```bash
git add site/assets/fidryn.js web/tests/site.test.js site/package.json xtask/src/workspace.rs xtask/src/ci.rs
git -c commit.gpgsign=false commit -m "feat(site): add the site behavior script with node tests in xtask ci"
```

### Task 7: Build-time highlighting

**Files:**
- Modify: `xtask/Cargo.toml` (`[dependencies]`)
- Modify: `xtask/src/site/mod.rs` (module list)
- Create: `xtask/src/site/highlight.rs`
- Test: `xtask/src/site/highlight.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `super::html::esc(&str) -> String` (Task 4); `fidryn_syntax::{lex, TokenKind}` (`lex(&str) -> Vec<Token>`, lossless, trivia included, ends with an empty `Eof`); `grammar.ebnf` at the workspace root; `crate::workspace::workspace_root()` in tests.
- Produces (contract C4), all in `crate::site::highlight`:
  - `enum Lang { Fr, Json, Shell, Plain }` with `label()` (`.fr`, `json`, `shell`, `text`) and `class()` (`fr`, `json`, `shell`, `text`).
  - `struct Keywords` with `from_grammar(&str) -> Self` (every quoted terminal made only of `[a-z_]`, longer than one character, read line by line), `load(root: &Path) -> anyhow::Result<Self>` (reads `root/grammar.ebnf`; the error names that path), `contains(&str) -> bool`, `iter() -> impl Iterator<Item = &str>` (sorted).
  - `sniff(info, code, kw) -> Lang`: an explicit info string wins (`fr`/`fidryn` → Fr; `json` → Json; `sh`/`bash`/`shell`/`console`/`zsh` → Shell; any other non-empty info → Plain; only the first word of the info counts, case-insensitively). Unlabeled code: first non-space character `{` or `[` → Json; first word `cargo`, `fidryn`, `cat`, `rustup`, `git`, `curl`, or a `$` prompt → Shell; leading identifier a grammar keyword → Fr; otherwise Plain.
  - `highlight(lang, code, kw) -> String`: escaped HTML with `<span class="tk-kw|tk-ty|tk-st|tk-nu|tk-co|tk-pu">`; no span ever crosses a `\n`; stripping the spans and unescaping returns `code` exactly.
  - `code_frame(lang, code, kw, caption: Option<&str>) -> String`: the C5 frame; one final `\n` of `code` is dropped; the caption (default `lang.label()`) is escaped.
  - `numbered_frame(lang, segments: &[(usize, String)], kw, caption: &str) -> String`: the C5 numbered frame; each segment is (1-based first line number, text), one trailing `\n` per segment is dropped, and a gap line goes between segments that are not adjacent (never before the first).
  - `site/mod.rs` module list after this task: `mod guides; mod highlight; mod html; mod seo; mod templates;`.

Class rules. `.fr`: keywords (from the grammar) and duration units are `tk-kw`; capitalized identifiers right after `->`, `:` or `<`, and every identifier of the dotted name after `module` or `import`, are `tk-ty` (the dots are `tk-pu`); `true`, `false`, integers, decimals, dates, date-times, `+inf`, `-inf` are `tk-nu`; comments and doc comments `tk-co`; strings `tk-st`; every punctuation and operator token `tk-pu`; other identifiers, whitespace, and lexer `Error` tokens are plain; any byte range the lexer does not cover is emitted as plain text. JSON: keys (a string followed by `:`) `tk-ty`, other strings `tk-st`, numbers and whole-word `true`/`false`/`null` `tk-nu`, `{}[],:` `tk-pu`, anything else plain. Shell (line by line): the command word (line start, after `|`, `&&`, `||`, `;`, `&`, or a leading `$ ` prompt; not on a line continued by a trailing `\`; a `NAME=value` word before it stays plain) is `tk-kw`; words starting with `-` are `tk-ty`; `'…'` and `"…"` are `tk-st`; a `#` that starts a word begins a `tk-co` comment; operators, redirections, the `$` prompt, `<<'EOF'`/`<<EOF`/`<<"EOF"`/`<<-EOF`, the trailing `\`, and the heredoc terminator line are `tk-pu`; heredoc body lines are `tk-st`.

- [ ] **Step 1: Write the failing test**

In `xtask/src/site/mod.rs`, replace:

```rust
mod guides;
mod html;
```

with:

```rust
mod guides;
mod highlight;
mod html;
```

Create `xtask/src/site/highlight.rs` with its tests (the implementation is inserted above `#[cfg(test)]` in Step 3). The round-trip test runs every fenced block of every `docs/*.md` through all four languages, and the sniffed one, and requires the exact input back:

````rust
//! Build-time highlighting for code frames: `.fr` through the real lexer,
//! JSON, and shell. Output is escaped HTML with `tk-*` spans; no span
//! crosses a line break, so stripping the spans and unescaping gives the
//! input back exactly.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_root;

    fn keywords() -> Keywords {
        Keywords::load(&workspace_root()).expect("read grammar.ebnf")
    }

    fn unescape(text: &str) -> String {
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
    }

    /// Every `tk-*` span of `html` as (class, unescaped text), in order.
    fn spans(html: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        let mut rest = html;
        while let Some(at) = rest.find("<span class=\"tk-") {
            let open = &rest[at + "<span class=\"".len()..];
            let quote = open.find('"').expect("closing quote");
            let text = &open[quote + "\">".len()..];
            let close = text.find("</span>").expect("closing span");
            found.push((open[..quote].to_owned(), unescape(&text[..close])));
            rest = &text[close + "</span>".len()..];
        }
        found
    }

    /// The class of the first span whose text is exactly `text`.
    fn class_of(html: &str, text: &str) -> Option<String> {
        spans(html)
            .into_iter()
            .find(|(_, t)| t == text)
            .map(|(class, _)| class)
    }

    /// `html` without its `tk-*` spans, unescaped: the highlighted input.
    fn plain(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;
        while let Some(at) = rest.find("<span class=\"tk-") {
            out.push_str(&rest[..at]);
            let text = &rest[at + rest[at..].find('>').expect("tag end") + 1..];
            let close = text.find("</span>").expect("closing span");
            out.push_str(&text[..close]);
            rest = &text[close + "</span>".len()..];
        }
        out.push_str(rest);
        unescape(&out)
    }

    fn assert_classes(html: &str, expected: &[(&str, &str)]) {
        for (text, class) in expected {
            assert_eq!(
                class_of(html, text).as_deref(),
                Some(*class),
                "{text:?} in {html}"
            );
        }
    }

    const LANGS: [Lang; 4] = [Lang::Fr, Lang::Json, Lang::Shell, Lang::Plain];

    #[test]
    fn keywords_are_the_grammars_lowercase_quoted_terminals() {
        let kw = keywords();
        for word in [
            "module",
            "version",
            "query",
            "require",
            "return",
            "outside_scope",
            "interpretation_family",
            "days",
            "working_days",
            "fn",
            "as",
            "in",
        ] {
            assert!(kw.contains(word), "{word} should be a keyword");
        }
        for word in [
            "true",
            "false",
            "if",
            "_",
            "UniqueOccupant",
            "FiniteSet",
            "Evaluate",
        ] {
            assert!(!kw.contains(word), "{word} should not be a keyword");
        }
        let words: Vec<&str> = kw.iter().collect();
        assert!(
            words.windows(2).all(|pair| pair[0] < pair[1]),
            "sorted and unique"
        );
        assert!(
            words
                .iter()
                .all(|w| w.len() > 1 && w.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
        );
    }

    #[test]
    fn from_grammar_reads_closed_quotes_on_each_line() {
        let kw = Keywords::from_grammar(
            "Rule ::= \"rule\" Ident \":\" Kind\nKind ::= \"derive\" | \"x\" | \"Evaluate\" | \"a_b\" | \"..\"\nOpen ::= \"unclosed\n",
        );
        assert_eq!(kw.iter().collect::<Vec<_>>(), ["a_b", "derive", "rule"]);
    }

    #[test]
    fn load_names_the_missing_grammar() {
        let err = Keywords::load(Path::new("/nonexistent-fidryn-root")).unwrap_err();
        assert!(format!("{err:#}").contains("grammar.ebnf"), "{err:#}");
    }

    #[test]
    fn sniff_prefers_an_explicit_info_string() {
        let kw = keywords();
        for (info, lang) in [
            ("fr", Lang::Fr),
            ("fidryn", Lang::Fr),
            ("json", Lang::Json),
            ("JSON", Lang::Json),
            ("sh", Lang::Shell),
            ("bash", Lang::Shell),
            ("shell", Lang::Shell),
            ("console", Lang::Shell),
            ("zsh", Lang::Shell),
            ("rust", Lang::Plain),
            ("text", Lang::Plain),
            ("json title=x", Lang::Json),
        ] {
            assert_eq!(sniff(info, "cargo test", &kw), lang, "{info}");
        }
    }

    #[test]
    fn sniff_recognizes_unlabeled_blocks() {
        let kw = keywords();
        for (code, lang) in [
            ("{\"schema\": \"x\"}", Lang::Json),
            ("  [1, 2]", Lang::Json),
            ("cargo test --workspace --offline", Lang::Shell),
            ("fidryn run x.fr \\\n    --query q", Lang::Shell),
            ("$ fidryn ui", Lang::Shell),
            ("cat > /tmp/case.json <<'EOF'", Lang::Shell),
            ("rustup show", Lang::Shell),
            ("git status", Lang::Shell),
            ("curl -s http://127.0.0.1:8751/api/health", Lang::Shell),
            ("module Programs.RequireGate version \"0.1.0\" {", Lang::Fr),
            ("query q() -> Int { require true; return 7 }", Lang::Fr),
            ("interpretation_family DeadlineMeaning {", Lang::Fr),
            ("fidryn-syntax          lossless CST", Lang::Plain),
            ("syntax → hir → check", Lang::Plain),
            ("if amount <= 11925.00 {", Lang::Plain),
            ("$HOME/bin", Lang::Plain),
            ("", Lang::Plain),
        ] {
            assert_eq!(sniff("", code, &kw), lang, "{code:?}");
        }
    }

    #[test]
    fn fr_token_classes() {
        let kw = keywords();
        let src = "module Programs.RequireGate version \"0.1.0\" {\n  // gate\n  import MA.TrustLaw version \"2026-08-23\"\n  query q() -> Int { require true; return 7 }\n  deadline: Date = 2026-09-17 + 30 days\n  at 2026-09-17T12:00:00Z\n  window: Interval<Instant> = [-inf, +inf)\n  amount: Money<USD> = 1.50\n  holder = Alice\n}\n";
        let html = highlight(Lang::Fr, src, &kw);
        assert_classes(
            &html,
            &[
                ("module", "tk-kw"),
                ("Programs", "tk-ty"),
                (".", "tk-pu"),
                ("RequireGate", "tk-ty"),
                ("version", "tk-kw"),
                ("\"0.1.0\"", "tk-st"),
                ("{", "tk-pu"),
                ("// gate", "tk-co"),
                ("import", "tk-kw"),
                ("MA", "tk-ty"),
                ("TrustLaw", "tk-ty"),
                ("query", "tk-kw"),
                ("->", "tk-pu"),
                ("Int", "tk-ty"),
                ("require", "tk-kw"),
                ("true", "tk-nu"),
                (";", "tk-pu"),
                ("return", "tk-kw"),
                ("7", "tk-nu"),
                ("Date", "tk-ty"),
                ("2026-09-17", "tk-nu"),
                ("30", "tk-nu"),
                ("days", "tk-kw"),
                ("2026-09-17T12:00:00Z", "tk-nu"),
                ("Interval", "tk-ty"),
                ("Instant", "tk-ty"),
                ("-inf", "tk-nu"),
                ("+inf", "tk-nu"),
                ("Money", "tk-ty"),
                ("USD", "tk-ty"),
                ("1.50", "tk-nu"),
            ],
        );
        assert_eq!(class_of(&html, "q"), None, "a query name is plain");
        assert_eq!(class_of(&html, "deadline"), None);
        assert_eq!(
            class_of(&html, "Alice"),
            None,
            "capitalized but not after -> : <"
        );
        assert_eq!(plain(&html), src);
    }

    #[test]
    fn json_token_classes() {
        let kw = keywords();
        let src = "{\n  \"kind\": \"entity\",\n  \"data\" : \"Bob\",\n  \"n\": -1.5e3,\n  \"ok\": true,\n  \"none\": null,\n  \"list\": [1, false]\n}";
        let html = highlight(Lang::Json, src, &kw);
        assert_classes(
            &html,
            &[
                ("{", "tk-pu"),
                ("\"kind\"", "tk-ty"),
                (":", "tk-pu"),
                ("\"entity\"", "tk-st"),
                (",", "tk-pu"),
                ("\"data\"", "tk-ty"),
                ("\"Bob\"", "tk-st"),
                ("-1.5e3", "tk-nu"),
                ("true", "tk-nu"),
                ("null", "tk-nu"),
                ("[", "tk-pu"),
                ("1", "tk-nu"),
                ("false", "tk-nu"),
                ("}", "tk-pu"),
            ],
        );
        assert_eq!(plain(&html), src);
    }

    #[test]
    fn json_leaves_non_json_plain() {
        let kw = keywords();
        let html = highlight(Lang::Json, "{\"diagnostics\": [...], \"v\": nullable}", &kw);
        assert_eq!(class_of(&html, "..."), None);
        assert_eq!(
            class_of(&html, "null"),
            None,
            "`null` inside a word is not a literal"
        );
    }

    #[test]
    fn shell_token_classes() {
        let kw = keywords();
        let src = "$ cargo run -p fidryn-cli -- check x.fr # check it\nFIDRYN_X=1 fidryn file a.json \\\n    --live \"quoted arg\" | cat && git status; curl 'u'\ncat > /tmp/case.json <<'EOF'\n{\"schema\":\"x\"}\nEOF\n";
        let html = highlight(Lang::Shell, src, &kw);
        assert_classes(
            &html,
            &[
                ("$", "tk-pu"),
                ("cargo", "tk-kw"),
                ("-p", "tk-ty"),
                ("--", "tk-ty"),
                ("# check it", "tk-co"),
                ("fidryn", "tk-kw"),
                ("\\", "tk-pu"),
                ("--live", "tk-ty"),
                ("\"quoted arg\"", "tk-st"),
                ("|", "tk-pu"),
                ("cat", "tk-kw"),
                ("&&", "tk-pu"),
                ("git", "tk-kw"),
                (";", "tk-pu"),
                ("curl", "tk-kw"),
                ("'u'", "tk-st"),
                (">", "tk-pu"),
                ("<<'EOF'", "tk-pu"),
                ("{\"schema\":\"x\"}", "tk-st"),
                ("EOF", "tk-pu"),
            ],
        );
        for word in [
            "run",
            "fidryn-cli",
            "check",
            "x.fr",
            "FIDRYN_X=1",
            "file",
            "status",
        ] {
            assert_eq!(class_of(&html, word), None, "{word} is an argument");
        }
        assert_eq!(plain(&html), src);
    }

    #[test]
    fn shell_command_position_follows_continuations_and_heredocs() {
        let kw = keywords();
        let html = highlight(
            Lang::Shell,
            "cargo run \\\n  test\ncargo build && \\\n  cargo test",
            &kw,
        );
        let kinds: Vec<(String, String)> = spans(&html);
        assert_eq!(
            kinds,
            [
                ("tk-kw", "cargo"),
                ("tk-pu", "\\"),
                ("tk-kw", "cargo"),
                ("tk-pu", "&&"),
                ("tk-pu", "\\"),
                ("tk-kw", "cargo"),
            ]
            .map(|(c, t)| (c.to_owned(), t.to_owned()))
        );
        let html = highlight(Lang::Shell, "cat <<EOF\nhello\nEOF\necho done", &kw);
        assert_classes(
            &html,
            &[
                ("<<EOF", "tk-pu"),
                ("hello", "tk-st"),
                ("EOF", "tk-pu"),
                ("echo", "tk-kw"),
            ],
        );
    }

    #[test]
    fn highlighting_escapes_markup_and_quotes() {
        let kw = keywords();
        assert_eq!(
            highlight(Lang::Plain, "<a href=\"x\">&'</a>", &kw),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;"
        );
        assert_eq!(
            highlight(Lang::Fr, "\"<b> & 'c'\"", &kw),
            "<span class=\"tk-st\">&quot;&lt;b&gt; &amp; &#39;c&#39;&quot;</span>"
        );
        assert_eq!(
            highlight(Lang::Json, "{\"<k>\": \"a&b\"}", &kw),
            "<span class=\"tk-pu\">{</span><span class=\"tk-ty\">&quot;&lt;k&gt;&quot;</span><span class=\"tk-pu\">:</span> <span class=\"tk-st\">&quot;a&amp;b&quot;</span><span class=\"tk-pu\">}</span>"
        );
        assert_eq!(
            highlight(Lang::Shell, "echo '<x>' > out", &kw),
            "<span class=\"tk-kw\">echo</span> <span class=\"tk-st\">&#39;&lt;x&gt;&#39;</span> <span class=\"tk-pu\">&gt;</span> out"
        );
    }

    #[test]
    fn spans_never_cross_lines() {
        let kw = keywords();
        assert_eq!(
            highlight(Lang::Fr, "x = \"a\nb\"", &kw),
            "x <span class=\"tk-pu\">=</span> <span class=\"tk-st\">&quot;a</span>\n<span class=\"tk-st\">b&quot;</span>"
        );
    }

    #[test]
    fn code_frame_markup() {
        let kw = keywords();
        assert_eq!(
            code_frame(Lang::Json, "{\"a\": 1}\n", &kw, None),
            concat!(
                "<figure class=\"code\" data-lang=\"json\"><figcaption><span>json</span>",
                "<button type=\"button\" class=\"copy\" data-copy hidden>Copy</button></figcaption>",
                "<pre><code><span class=\"tk-pu\">{</span><span class=\"tk-ty\">&quot;a&quot;</span>",
                "<span class=\"tk-pu\">:</span> <span class=\"tk-nu\">1</span><span class=\"tk-pu\">}</span>",
                "</code></pre></figure>"
            )
        );
    }

    #[test]
    fn code_frame_drops_one_final_newline_and_escapes_the_caption() {
        let kw = keywords();
        let frame = code_frame(Lang::Plain, "a\n\n", &kw, Some("cases/<b>.json"));
        assert!(
            frame.contains("<span>cases/&lt;b&gt;.json</span>"),
            "{frame}"
        );
        assert!(
            frame.ends_with("<pre><code>a\n</code></pre></figure>"),
            "{frame}"
        );
        for (lang, label, class) in [
            (Lang::Fr, ".fr", "fr"),
            (Lang::Json, "json", "json"),
            (Lang::Shell, "shell", "shell"),
            (Lang::Plain, "text", "text"),
        ] {
            let frame = code_frame(lang, "", &kw, None);
            let head = format!("data-lang=\"{class}\"><figcaption><span>{label}</span>");
            assert!(frame.contains(&head), "{frame}");
        }
    }

    #[test]
    fn numbered_frame_markup_with_gap_lines() {
        let kw = keywords();
        let segments = [
            (3, "a\nb\n".to_owned()),
            (5, "c".to_owned()),
            (9, "d\n".to_owned()),
        ];
        assert_eq!(
            numbered_frame(Lang::Plain, &segments, &kw, "x.fr"),
            concat!(
                "<figure class=\"code\" data-lang=\"text\"><figcaption><span>x.fr</span>",
                "<button type=\"button\" class=\"copy\" data-copy hidden>Copy</button></figcaption>",
                "<pre class=\"lines\"><code>",
                "<span class=\"line\" data-n=\"3\">a</span>\n",
                "<span class=\"line\" data-n=\"4\">b</span>\n",
                "<span class=\"line\" data-n=\"5\">c</span>\n",
                "<span class=\"line gap\" data-n=\"\">&hellip;</span>\n",
                "<span class=\"line\" data-n=\"9\">d</span>\n",
                "</code></pre></figure>"
            )
        );
    }

    #[test]
    fn numbered_frame_highlights_inside_each_line() {
        let kw = keywords();
        let frame = numbered_frame(
            Lang::Fr,
            &[(12, "query q() -> Int {\n  return 7\n}\n".to_owned())],
            &kw,
            "tests/programs/require-gate.fr",
        );
        assert!(frame.starts_with("<figure class=\"code\" data-lang=\"fr\"><figcaption><span>tests/programs/require-gate.fr</span>"));
        assert!(frame.contains(
            "<span class=\"line\" data-n=\"13\">  <span class=\"tk-kw\">return</span> <span class=\"tk-nu\">7</span></span>\n"
        ));
        assert!(frame.contains(
            "<span class=\"line\" data-n=\"14\"><span class=\"tk-pu\">}</span></span>\n</code>"
        ));
    }

    /// Fenced blocks of a markdown file as (info, code), by line scanning.
    fn fenced_blocks(md: &str) -> Vec<(String, String)> {
        let mut blocks = Vec::new();
        let mut lines = md.lines();
        while let Some(line) = lines.next() {
            let Some(info) = line.trim_start().strip_prefix("```") else {
                continue;
            };
            let mut code = String::new();
            for body in lines.by_ref() {
                if body.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(body);
                code.push('\n');
            }
            blocks.push((info.trim().to_owned(), code));
        }
        blocks
    }

    #[test]
    fn every_fenced_block_in_docs_round_trips_in_every_language() {
        let kw = keywords();
        let mut count = 0;
        for entry in fs::read_dir(workspace_root().join("docs")).expect("read docs/") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }
            let md = fs::read_to_string(&path).expect("read guide");
            for (info, code) in fenced_blocks(&md) {
                for lang in LANGS {
                    let html = highlight(lang, &code, &kw);
                    assert_eq!(plain(&html), code, "{} as {lang:?}", path.display());
                }
                assert_eq!(
                    plain(&highlight(sniff(&info, &code, &kw), &code, &kw)),
                    code
                );
                count += 1;
            }
        }
        assert!(count >= 100, "found only {count} fenced blocks under docs/");
    }

    #[test]
    fn odd_input_round_trips_in_every_language() {
        let kw = keywords();
        for code in [
            "",
            "\n\n",
            "\"unterminated",
            "'",
            "§ é 日本 λx",
            "<<'EOF'\nbody",
            "tail \\",
            "{\"a\": [1, -2.5e3, true, null]}",
            "-",
            "#",
            "$ ",
            "x = \"a\\\"b\" // c\n  /// d",
            "2026-09-17T12:00:00Z +inf -inf 0..1",
        ] {
            for lang in LANGS {
                assert_eq!(
                    plain(&highlight(lang, code, &kw)),
                    code,
                    "{lang:?} {code:?}"
                );
            }
        }
    }
}
````

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p xtask --offline site::highlight`
Expected: FAIL to compile, with errors such as ``error[E0433]: cannot find type `Lang` in this scope``, ``error[E0425]: cannot find function `highlight` in this scope``, ``error[E0425]: cannot find function `sniff` in this scope``, ``error[E0425]: cannot find function `numbered_frame` in this scope``, ending with ``error: could not compile `xtask` (bin "xtask" test)``.

- [ ] **Step 3: Write the implementation**

In `xtask/Cargo.toml`, replace:

```toml
clap.workspace = true
```

with:

```toml
clap.workspace = true
fidryn-syntax.workspace = true
```

In `xtask/src/site/highlight.rs`, replace the line `#[cfg(test)]` with:

```rust
use super::html::esc;
use anyhow::{Context, Result};
use fidryn_syntax::{TokenKind, lex};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The language of a code frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Fr,
    Json,
    Shell,
    Plain,
}

impl Lang {
    /// Caption shown in the frame header when no other caption is given.
    pub fn label(self) -> &'static str {
        match self {
            Lang::Fr => ".fr",
            Lang::Json => "json",
            Lang::Shell => "shell",
            Lang::Plain => "text",
        }
    }

    /// Value of the frame's `data-lang` attribute.
    pub fn class(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::Json => "json",
            Lang::Shell => "shell",
            Lang::Plain => "text",
        }
    }
}

/// `.fr` keywords: every quoted terminal in `grammar.ebnf` made only of
/// `[a-z_]` and longer than one character. `true` and `false` are not quoted
/// in the grammar; they highlight as literals.
#[derive(Clone, Debug)]
pub struct Keywords(BTreeSet<String>);

impl Keywords {
    pub fn from_grammar(grammar: &str) -> Self {
        let mut words = BTreeSet::new();
        for line in grammar.lines() {
            let mut rest = line;
            while let Some(open) = rest.find('"') {
                let after = &rest[open + 1..];
                let Some(close) = after.find('"') else {
                    break;
                };
                let word = &after[..close];
                if word.len() > 1 && word.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                    words.insert(word.to_owned());
                }
                rest = &after[close + 1..];
            }
        }
        Self(words)
    }

    /// The keywords of `root/grammar.ebnf`.
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("grammar.ebnf");
        let grammar =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        Ok(Self::from_grammar(&grammar))
    }

    pub fn contains(&self, word: &str) -> bool {
        self.0.contains(word)
    }

    /// Every keyword, sorted. The site itself only asks `contains`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

/// The language of a code block. An explicit info string wins; an unlabeled
/// block is JSON when it opens with `{` or `[`, shell when its first word is
/// a known command or a `$` prompt, `.fr` when its first word is a keyword,
/// and plain text otherwise.
pub fn sniff(info: &str, code: &str, kw: &Keywords) -> Lang {
    let tag = info
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match tag.as_str() {
        "fr" | "fidryn" => return Lang::Fr,
        "json" => return Lang::Json,
        "sh" | "bash" | "shell" | "console" | "zsh" => return Lang::Shell,
        "" => {}
        _ => return Lang::Plain,
    }
    let text = code.trim_start();
    if text.starts_with(['{', '[']) {
        return Lang::Json;
    }
    let first = text.split_whitespace().next().unwrap_or("");
    if matches!(
        first,
        "$" | "cargo" | "fidryn" | "cat" | "rustup" | "git" | "curl"
    ) {
        return Lang::Shell;
    }
    let ident = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map_or(text, |end| &text[..end]);
    if kw.contains(ident) {
        Lang::Fr
    } else {
        Lang::Plain
    }
}

/// `code` as escaped HTML with `<span class="tk-…">` around recognized tokens.
pub fn highlight(lang: Lang, code: &str, kw: &Keywords) -> String {
    lines(lang, code, kw).join("\n")
}

/// A framed code block. The caption defaults to the language label; one
/// final newline is dropped so the frame does not end in a blank line.
pub fn code_frame(lang: Lang, code: &str, kw: &Keywords, caption: Option<&str>) -> String {
    let code = code.strip_suffix('\n').unwrap_or(code);
    format!(
        "<figure class=\"code\" data-lang=\"{}\"><figcaption><span>{}</span>{COPY}</figcaption><pre><code>{}</code></pre></figure>",
        lang.class(),
        esc(caption.unwrap_or(lang.label())),
        highlight(lang, code, kw)
    )
}

/// A framed excerpt with real line numbers. Each segment is (first line
/// number, text); a gap line marks the lines skipped between segments.
pub fn numbered_frame(
    lang: Lang,
    segments: &[(usize, String)],
    kw: &Keywords,
    caption: &str,
) -> String {
    let mut body = String::new();
    let mut next: Option<usize> = None;
    for (first, text) in segments {
        if next.is_some_and(|n| n != *first) {
            body.push_str("<span class=\"line gap\" data-n=\"\">&hellip;</span>\n");
        }
        let text = text.strip_suffix('\n').unwrap_or(text);
        let mut n = *first;
        for line in lines(lang, text, kw) {
            body.push_str(&format!(
                "<span class=\"line\" data-n=\"{n}\">{line}</span>\n"
            ));
            n += 1;
        }
        next = Some(n);
    }
    format!(
        "<figure class=\"code\" data-lang=\"{}\"><figcaption><span>{}</span>{COPY}</figcaption><pre class=\"lines\"><code>{body}</code></pre></figure>",
        lang.class(),
        esc(caption)
    )
}

const COPY: &str = "<button type=\"button\" class=\"copy\" data-copy hidden>Copy</button>";

/// Token classes, rendered as `tk-*` span classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tk {
    Kw,
    Ty,
    St,
    Nu,
    Co,
    Pu,
}

impl Tk {
    fn class(self) -> &'static str {
        match self {
            Tk::Kw => "tk-kw",
            Tk::Ty => "tk-ty",
            Tk::St => "tk-st",
            Tk::Nu => "tk-nu",
            Tk::Co => "tk-co",
            Tk::Pu => "tk-pu",
        }
    }
}

/// A slice of the input and its class; `None` is plain text.
type Piece<'a> = (Option<Tk>, &'a str);

/// Highlighted HTML for each line of `code`.
fn lines(lang: Lang, code: &str, kw: &Keywords) -> Vec<String> {
    let pieces = match lang {
        Lang::Fr => fr_pieces(code, kw),
        Lang::Json => json_pieces(code),
        Lang::Shell => shell_pieces(code),
        Lang::Plain => vec![(None, code)],
    };
    let mut lines = vec![String::new()];
    for (tk, text) in pieces {
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                lines.push(String::new());
            }
            if part.is_empty() {
                continue;
            }
            let line = lines.last_mut().expect("lines starts non-empty");
            match tk {
                Some(tk) => {
                    line.push_str("<span class=\"");
                    line.push_str(tk.class());
                    line.push_str("\">");
                    line.push_str(&esc(part));
                    line.push_str("</span>");
                }
                None => line.push_str(&esc(part)),
            }
        }
    }
    lines
}

/// Where a `module` or `import` name is in `A.B.C`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ModPath {
    Off,
    /// An identifier comes next.
    Name,
    /// A dot may continue the name.
    Dot,
}

/// `.fr` tokens from `fidryn_syntax::lex`. Bytes the lexer skips are kept
/// as plain text.
fn fr_pieces<'a>(code: &'a str, kw: &Keywords) -> Vec<Piece<'a>> {
    let mut pieces = Vec::new();
    let mut pos = 0;
    let mut prev = None;
    let mut path = ModPath::Off;
    for token in lex(code) {
        let (start, end) = (token.start as usize, token.end as usize);
        if start > pos {
            pieces.push((None, &code[pos..start]));
        }
        if token.kind == TokenKind::Eof {
            pos = pos.max(start);
            break;
        }
        let text = &code[start..end];
        let tk = match token.kind {
            TokenKind::Whitespace | TokenKind::Newline => None,
            TokenKind::Comment | TokenKind::DocComment => Some(Tk::Co),
            kind => {
                let tk = fr_class(kind, text, prev, &mut path, kw);
                prev = Some(kind);
                tk
            }
        };
        pieces.push((tk, text));
        pos = end;
    }
    if pos < code.len() {
        pieces.push((None, &code[pos..]));
    }
    pieces
}

/// The class of a token that is not whitespace or a comment.
fn fr_class(
    kind: TokenKind,
    text: &str,
    prev: Option<TokenKind>,
    path: &mut ModPath,
    kw: &Keywords,
) -> Option<Tk> {
    match (*path, kind) {
        (ModPath::Name, TokenKind::Ident) => {
            *path = ModPath::Dot;
            return Some(Tk::Ty);
        }
        (ModPath::Dot, TokenKind::Dot) => {
            *path = ModPath::Name;
            return Some(Tk::Pu);
        }
        _ => *path = ModPath::Off,
    }
    match kind {
        TokenKind::Ident if kw.contains(text) => {
            if matches!(text, "module" | "import") {
                *path = ModPath::Name;
            }
            Some(Tk::Kw)
        }
        TokenKind::Ident if matches!(text, "true" | "false") => Some(Tk::Nu),
        TokenKind::Ident
            if matches!(
                prev,
                Some(TokenKind::Arrow | TokenKind::Colon | TokenKind::Lt)
            ) && text.starts_with(|c: char| c.is_ascii_uppercase()) =>
        {
            Some(Tk::Ty)
        }
        TokenKind::Ident
        | TokenKind::Whitespace
        | TokenKind::Newline
        | TokenKind::Error
        | TokenKind::Eof => None,
        TokenKind::String => Some(Tk::St),
        TokenKind::Int
        | TokenKind::Decimal
        | TokenKind::Date
        | TokenKind::DateTime
        | TokenKind::PlusInf
        | TokenKind::MinusInf => Some(Tk::Nu),
        TokenKind::DurationUnit => Some(Tk::Kw),
        TokenKind::Comment | TokenKind::DocComment => Some(Tk::Co),
        TokenKind::LBrace
        | TokenKind::RBrace
        | TokenKind::LParen
        | TokenKind::RParen
        | TokenKind::LBracket
        | TokenKind::RBracket
        | TokenKind::Comma
        | TokenKind::Dot
        | TokenKind::Colon
        | TokenKind::Semicolon
        | TokenKind::Bang
        | TokenKind::Arrow
        | TokenKind::FatArrow
        | TokenKind::Eq
        | TokenKind::EqEq
        | TokenKind::Ne
        | TokenKind::Lt
        | TokenKind::Le
        | TokenKind::Gt
        | TokenKind::Ge
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Pipe
        | TokenKind::Range => Some(Tk::Pu),
    }
}

/// JSON tokens. Anything that is not JSON stays plain, so a block that only
/// looks like JSON still renders.
fn json_pieces(code: &str) -> Vec<Piece<'_>> {
    let bytes = code.as_bytes();
    let mut pieces = Vec::new();
    let mut plain = 0;
    let mut i = 0;
    while i < bytes.len() {
        let token = match bytes[i] {
            b'"' => {
                let end = quoted_end(bytes, i, true);
                let next = bytes[end..].iter().find(|b| !b.is_ascii_whitespace());
                Some((end, if next == Some(&b':') { Tk::Ty } else { Tk::St }))
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' => Some((i + 1, Tk::Pu)),
            b'-' | b'0'..=b'9' => number_end(bytes, i).map(|end| (end, Tk::Nu)),
            b't' | b'f' | b'n' => literal_end(bytes, i).map(|end| (end, Tk::Nu)),
            _ => None,
        };
        match token {
            Some((end, tk)) => {
                if plain < i {
                    pieces.push((None, &code[plain..i]));
                }
                pieces.push((Some(tk), &code[i..end]));
                i = end;
                plain = end;
            }
            None => i += 1,
        }
    }
    if plain < bytes.len() {
        pieces.push((None, &code[plain..]));
    }
    pieces
}

/// End of the quoted run opening at `start`: just past the closing quote,
/// or at the end of the line when it is unterminated.
fn quoted_end(bytes: &[u8], start: usize, escapes: bool) -> usize {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => return i,
            b'\\' if escapes && bytes.get(i + 1).is_some_and(|b| *b != b'\n') => i += 2,
            b if b == quote => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// End of a JSON number starting at `start`, if one starts there.
fn number_end(bytes: &[u8], start: usize) -> Option<usize> {
    if start > 0 && is_word_byte(bytes[start - 1]) {
        return None;
    }
    let digits = |from: usize| {
        from + bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let mut i = start + usize::from(bytes[start] == b'-');
    let int_end = digits(i);
    if int_end == i {
        return None;
    }
    i = int_end;
    if bytes.get(i) == Some(&b'.') && digits(i + 1) > i + 1 {
        i = digits(i + 1);
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(bytes.get(i + 1), Some(b'+' | b'-')));
        let exp_end = digits(i + 1 + sign);
        if exp_end > i + 1 + sign {
            i = exp_end;
        }
    }
    Some(i)
}

/// End of `true`, `false`, or `null` starting at `start`, as a whole word.
fn literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    if start > 0 && is_word_byte(bytes[start - 1]) {
        return None;
    }
    ["true", "false", "null"].iter().find_map(|lit| {
        let end = start + lit.len();
        let whole = bytes[start..].starts_with(lit.as_bytes())
            && !bytes.get(end).copied().is_some_and(is_word_byte);
        whole.then_some(end)
    })
}

/// Shell tokens, line by line. Command words come at the start of a line,
/// after `|`, `&&`, or `;`, and after a leading `$ ` prompt; a line that
/// continues the previous one (trailing `\`) does not start a command.
fn shell_pieces(code: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut heredoc: Option<&str> = None;
    let mut command = true;
    let mut continued = false;
    for (n, line) in code.split('\n').enumerate() {
        if n > 0 {
            pieces.push((None, "\n"));
        }
        if let Some(terminator) = heredoc {
            if line.trim() == terminator {
                heredoc = None;
                pieces.push((Some(Tk::Pu), line));
            } else if !line.is_empty() {
                pieces.push((Some(Tk::St), line));
            }
            continue;
        }
        if !continued {
            command = true;
        }
        let mut shell = ShellLine {
            pieces: &mut pieces,
            command,
            terminator: None,
        };
        continued = shell.scan(line, !continued);
        command = shell.command;
        heredoc = shell.terminator;
    }
    pieces
}

/// Scanner state for one line of shell.
struct ShellLine<'a, 'p> {
    pieces: &'p mut Vec<Piece<'a>>,
    /// The next word is a command word.
    command: bool,
    /// A heredoc announced on this line; its body starts on the next line.
    terminator: Option<&'a str>,
}

impl<'a> ShellLine<'a, '_> {
    /// Scan `line`; true when it ends with a continuation backslash.
    fn scan(&mut self, line: &'a str, fresh: bool) -> bool {
        let (body, continues) = match line.strip_suffix('\\') {
            Some(body) => (body, true),
            None => (line, false),
        };
        let bytes = body.as_bytes();
        let mut i = run(bytes, 0, |b| matches!(b, b' ' | b'\t'));
        if i > 0 {
            self.pieces.push((None, &body[..i]));
        }
        if fresh && body[i..].starts_with("$ ") {
            self.pieces.push((Some(Tk::Pu), &body[i..i + 1]));
            i += 1;
        }
        while i < bytes.len() {
            let (end, tk) = match bytes[i] {
                b' ' | b'\t' | b'\r' => {
                    (run(bytes, i, |b| matches!(b, b' ' | b'\t' | b'\r')), None)
                }
                b'#' if i == 0 || bytes[i - 1].is_ascii_whitespace() => (bytes.len(), Some(Tk::Co)),
                b'\'' | b'"' => {
                    self.command = false;
                    (quoted_end(bytes, i, bytes[i] == b'"'), Some(Tk::St))
                }
                b'|' | b'&' | b';' => {
                    self.command = true;
                    (
                        run(bytes, i, |b| matches!(b, b'|' | b'&' | b';')),
                        Some(Tk::Pu),
                    )
                }
                b'<' | b'>' => match heredoc_start(body, i) {
                    Some((end, terminator)) => {
                        self.terminator = Some(terminator);
                        (end, Some(Tk::Pu))
                    }
                    None => (run(bytes, i, |b| matches!(b, b'<' | b'>')), Some(Tk::Pu)),
                },
                _ => {
                    let end = run(bytes, i, |b| {
                        !b.is_ascii_whitespace()
                            && !matches!(b, b'\'' | b'"' | b'|' | b'&' | b';' | b'<' | b'>')
                    });
                    (end, self.word(&body[i..end]))
                }
            };
            self.pieces.push((tk, &body[i..end]));
            i = end;
        }
        if continues {
            self.pieces.push((Some(Tk::Pu), &line[line.len() - 1..]));
        }
        continues
    }

    /// A bare word: the command word, a flag, or plain text.
    fn word(&mut self, word: &str) -> Option<Tk> {
        if self.command {
            if is_assignment(word) {
                return None;
            }
            self.command = false;
            Some(Tk::Kw)
        } else if word.starts_with('-') {
            Some(Tk::Ty)
        } else {
            None
        }
    }
}

/// End of the run of bytes from `start` that satisfy `pred`.
fn run(bytes: &[u8], start: usize, pred: impl Fn(u8) -> bool) -> usize {
    start + bytes[start..].iter().take_while(|b| pred(**b)).count()
}

/// `NAME=value`, which may precede a command word.
fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && name.bytes().all(is_word_byte)
    })
}

/// `<<EOF`, `<<-EOF`, `<<'EOF'`, or `<<"EOF"` at `i`: its end and terminator.
fn heredoc_start(line: &str, i: usize) -> Option<(usize, &str)> {
    let rest = line[i..].strip_prefix("<<")?;
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    let quote = rest.bytes().next().filter(|b| matches!(b, b'\'' | b'"'));
    let word = &rest[usize::from(quote.is_some())..];
    let len = word.bytes().take_while(|b| is_word_byte(*b)).count();
    if len == 0 {
        return None;
    }
    let mut end = line.len() - word.len() + len;
    if let Some(quote) = quote {
        if word.as_bytes().get(len) != Some(&quote) {
            return None;
        }
        end += 1;
    }
    Some((end, &word[..len]))
}

#[cfg(test)]
```

`Keywords::iter` carries `#[cfg_attr(not(test), allow(dead_code))]`: `xtask` is a binary crate, so a public method that only tests call is dead code under `cargo clippy -- -D warnings`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline site::highlight`
Expected: PASS. `test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 23 filtered out` (the `task_selection` binary reports `0 passed … 9 filtered out`).

Run: `cargo test -p xtask --offline`
Expected: PASS. `test result: ok. 41 passed; 0 failed` then `test result: ok. 9 passed; 0 failed`. Dead-code warnings for `sniff`, `code_frame`, `numbered_frame`, and the Task 4 items are expected until Tasks 8–11 call them.

Run: `cargo fmt -p xtask -- --check`
Expected: no output, exit status 0.

- [ ] **Step 5: Commit**

```bash
git add xtask/Cargo.toml Cargo.lock xtask/src/site/mod.rs xtask/src/site/highlight.rs
git -c commit.gpgsign=false commit -m "feat(site): highlight fr, json, and shell at build time"
```

### Task 8: Links and markdown rendering

**Files:**
- Modify: `xtask/Cargo.toml` (`[dependencies]`)
- Modify: `xtask/src/site/mod.rs` (module list)
- Create: `xtask/src/site/links.rs`
- Create: `xtask/src/site/markdown.rs`
- Modify: `docs/examples.md` (delete the stray mirror header, lines 1–10; see Step 5)
- Test: `xtask/src/site/links.rs` and `xtask/src/site/markdown.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `guides::{SITE, REPO, url, by_file, GUIDES}`, `html::esc` (Task 4); `highlight::{Keywords, sniff, code_frame}` (Task 7); `pulldown_cmark` 0.13 (`Parser::new_ext`, `into_offset_iter`, `Event`, `Tag`, `TagEnd`, `HeadingLevel`, `CodeBlockKind`, `LinkType`, `CowStr`, `html::push_html`).
- Produces (contract C4), in `crate::site::links`:
  - `enum LinkStyle { Html, Markdown }`.
  - `rewrite(href: &str, style: LinkStyle) -> String`: a learner guide (`cli.md#file`) becomes `/docs/cli#file` (Html) or `https://fidryn.onlygass.dev/docs/cli.md#file` (Markdown), with `README.md` as the overview (`/docs/`, or `https://fidryn.onlygass.dev/docs/index.md`); any other name under `docs/` becomes `https://github.com/BeeGass/fidryn/blob/main/docs/{file}`; `../path` becomes `https://github.com/BeeGass/fidryn/blob/main/path`, or `tree/main/path` when it ends in `/`; fragments are kept; empty, `#…`, `/…`, and scheme URLs (`https:`, `http:`, `mailto:`) are returned unchanged.
  - `rewrite_markdown_links(md: &str) -> String`: every inline link target rewritten with `LinkStyle::Markdown`, found through the parser, so code spans and code blocks are untouched and every other byte is kept (images are left alone; the guides have none). Task 11's `seo::mirror` uses it.
- Produces, in `crate::site::markdown`:
  - `struct TocEntry { pub level: u8, pub number: String, pub id: String, pub text: String }`, `struct Section { pub number: String, pub id: String, pub heading: String, pub text: String }`, `struct Page { pub title: String, pub lead: String, pub body: String, pub toc: Vec<TocEntry>, pub sections: Vec<Section> }`, each deriving `Clone, Debug, PartialEq, Eq`.
  - `render(md: &str, guide_number: Option<u32>, kw: &Keywords) -> Page`. `title` is the text of the first `h1` (for example `Fidryn CLI` for `cli.md`; the navigation label stays `Guide.title`). `body` is HTML without that `h1` and ends with a newline. `toc` and `sections` have one entry per `h2`/`h3`, in order, with the same `number` and `id`; `TocEntry.level` is 2 or 3. `lead` and `Section.text` are the first 200 characters of the text before the first `h2`/`h3` and of each section (whitespace collapsed, cut at a character boundary, no trailing space; inline code without backticks; code blocks, tables, and `h4`+ headings count as section text). Every field except `body` is plain text: escape it with `html::esc` for HTML, JSON-encode it for the search index.
  - Heading ids: GitHub slugs (`slug`), de-duplicated as `x`, `x-1`, `x-2`; an explicit `{#id}` heading attribute replaces the slug; an empty slug becomes `section`. Numbers (`6.1`, `6.1.2`) only when `guide_number` is `Some`, only on `h2`/`h3`, never on an `h3` before the first `h2`. Markup is exactly C5: `<h2 id="envelope"><span class="hn">6.1</span> Envelope <a class="anchor" href="#envelope" aria-label="Link to this section">#</a></h2>`; `h4`–`h6` get an id and an anchor but no number.
  - `slug(text: &str) -> String`.
  - Tables are wrapped as `<div class="table-wrap" tabindex="0" role="region" aria-label="Table"><table>…</table>\n</div>`; fenced and indented code becomes `highlight::code_frame(sniff(info, code), code, kw, None)`; blockquotes stay `<blockquote>`; link targets go through `links::rewrite(_, LinkStyle::Html)`, headings included.
  - `site/mod.rs` module list after this task: `mod guides; mod highlight; mod html; mod links; mod markdown; mod seo; mod templates;` (Tasks 9–11 add `mod pages;`, `mod search;`, `mod specimen;`).
  - `docs/examples.md` opens with `# Example corpus`, like every other guide.

Review Focus item 2 is pinned here: `every_table_is_wrapped_in_a_scrolling_region` and `every_guide_renders_with_unique_ids_and_resolving_links` require every `<table>` in every rendered guide (the Examples and CLI guides have the widest) to sit inside the `table-wrap` region, and every `<pre>` to sit inside a code frame, so only those containers can scroll sideways on a phone.

- [ ] **Step 1: Write the failing tests**

In `xtask/src/site/mod.rs`, replace:

```rust
mod html;
mod seo;
```

with:

```rust
mod html;
mod links;
mod markdown;
mod seo;
```

Create `xtask/src/site/links.rs` with its tests (the implementation is inserted above `#[cfg(test)]` in Step 3):

````rust
//! Link targets for rendered pages and markdown mirrors. Links to learner
//! guides stay on the site; every other repository path goes to GitHub.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::guides::GUIDES;
    use crate::workspace::workspace_root;
    use std::fs;

    const STYLES: [LinkStyle; 2] = [LinkStyle::Html, LinkStyle::Markdown];

    #[test]
    fn guide_links_stay_on_the_site() {
        assert_eq!(rewrite("cli.md", LinkStyle::Html), "/docs/cli");
        assert_eq!(rewrite("cli.md#file", LinkStyle::Html), "/docs/cli#file");
        assert_eq!(
            rewrite("./outcomes.md#determinate", LinkStyle::Html),
            "/docs/outcomes#determinate"
        );
        assert_eq!(rewrite("README.md", LinkStyle::Html), "/docs/");
    }

    #[test]
    fn guide_links_in_mirrors_are_absolute_markdown_urls() {
        assert_eq!(
            rewrite("cli.md", LinkStyle::Markdown),
            "https://fidryn.onlygass.dev/docs/cli.md"
        );
        assert_eq!(
            rewrite("cases-and-time.md#checklist", LinkStyle::Markdown),
            "https://fidryn.onlygass.dev/docs/cases-and-time.md#checklist"
        );
        assert_eq!(
            rewrite("README.md", LinkStyle::Markdown),
            "https://fidryn.onlygass.dev/docs/index.md"
        );
    }

    #[test]
    fn other_docs_go_to_github() {
        for style in STYLES {
            assert_eq!(
                rewrite("ARCHITECTURE.md", style),
                "https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md"
            );
            assert_eq!(
                rewrite("implementation-status.md#trust", style),
                "https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md#trust"
            );
        }
    }

    #[test]
    fn repository_paths_go_to_blob_or_tree() {
        for style in STYLES {
            assert_eq!(
                rewrite("../grammar.ebnf", style),
                "https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf"
            );
            assert_eq!(
                rewrite("../README.md#install", style),
                "https://github.com/BeeGass/fidryn/blob/main/README.md#install"
            );
            assert_eq!(
                rewrite("../tests/programs/", style),
                "https://github.com/BeeGass/fidryn/tree/main/tests/programs/"
            );
        }
    }

    #[test]
    fn absolute_and_local_links_are_unchanged() {
        for href in [
            "https://fidryn.onlygass.dev/",
            "http://127.0.0.1:8751",
            "mailto:someone@example.com",
            "/docs/cli",
            "#file",
            "",
        ] {
            for style in STYLES {
                assert_eq!(rewrite(href, style), href);
            }
        }
    }

    #[test]
    fn mirrors_rewrite_inline_link_targets_and_nothing_else() {
        let md = concat!(
            "See [CLI](cli.md#run) and the\n",
            "[language\ngrammar](../grammar.ebnf \"Grammar\"), [top](#top), <https://example.com>.\n\n",
            "Code: `[x](cli.md)`\n\n",
            "```\n[y](cli.md)\n```\n\n",
            "| a | [b](mill.md) |\n| --- | --- |\n"
        );
        assert_eq!(
            rewrite_markdown_links(md),
            concat!(
                "See [CLI](https://fidryn.onlygass.dev/docs/cli.md#run) and the\n",
                "[language\ngrammar](https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf \"Grammar\"), [top](#top), <https://example.com>.\n\n",
                "Code: `[x](cli.md)`\n\n",
                "```\n[y](cli.md)\n```\n\n",
                "| a | [b](https://fidryn.onlygass.dev/docs/mill.md) |\n| --- | --- |\n"
            )
        );
    }

    /// The text a reader sees: every text and code event, in order.
    fn visible_text(md: &str) -> String {
        Parser::new_ext(md, Options::ENABLE_TABLES)
            .filter_map(|event| match event {
                Event::Text(text) | Event::Code(text) => Some(text.into_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn real_guide_mirrors_link_only_to_absolute_urls() {
        let docs = workspace_root().join("docs");
        for guide in GUIDES {
            let md = fs::read_to_string(docs.join(guide.file)).expect("read guide");
            let mirror = rewrite_markdown_links(&md);
            for event in Parser::new_ext(&mirror, Options::ENABLE_TABLES) {
                if let Event::Start(Tag::Link { dest_url, .. }) = event {
                    assert!(
                        dest_url.starts_with("https://") || dest_url.starts_with('#'),
                        "{}: {dest_url}",
                        guide.file
                    );
                }
            }
            assert_eq!(visible_text(&mirror), visible_text(&md), "{}", guide.file);
        }
    }
}
````

Create `xtask/src/site/markdown.rs` with its tests. The last two tests render every real guide: ids must be unique per page, every `#anchor` must resolve, no relative link may survive, tables and code must sit in their scrolling containers, each guide must open with its `# ` title, and the Outcomes guide must carry the six anchors the landing legend links to:

````rust
//! Guide markdown to page HTML. The first `h1` becomes the title; `h2`–`h6`
//! get GitHub-style ids and a `#` anchor; `h2`/`h3` get section numbers and
//! "On this page" entries; tables scroll inside a wrapper; code is framed
//! and highlighted; links are rewritten for the site.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::guides::GUIDES;
    use crate::workspace::workspace_root;
    use std::collections::HashSet;
    use std::fs;

    fn keywords() -> Keywords {
        Keywords::load(&workspace_root()).expect("read grammar.ebnf")
    }

    fn page(md: &str, guide_number: Option<u32>) -> Page {
        render(md, guide_number, &keywords())
    }

    /// Every value of `attr="…"` in `html`, in order.
    fn attrs<'a>(html: &'a str, attr: &str) -> Vec<&'a str> {
        html.split(&format!(" {attr}=\""))
            .skip(1)
            .map(|rest| &rest[..rest.find('"').expect("closing quote")])
            .collect()
    }

    #[test]
    fn the_first_h1_is_the_title_and_not_in_the_body() {
        let p = page(
            "# Outcomes\n\nThis guide explains.\n\n## Envelope\n\nText.\n",
            Some(6),
        );
        assert_eq!(p.title, "Outcomes");
        assert!(
            !p.body.contains("<h1") && !p.body.contains("Outcomes"),
            "{}",
            p.body
        );
        assert!(
            p.body
                .starts_with("<p>This guide explains.</p>\n<h2 id=\"envelope\">"),
            "{}",
            p.body
        );
    }

    #[test]
    fn numbered_guides_number_h2_and_h3_and_anchor_every_heading() {
        let p = page(
            "# T\n\n## Envelope\n\n### Fields\n\n#### Detail\n\n## Outcome kinds\n\n### Determinate\n",
            Some(6),
        );
        let body = &p.body;
        for expected in [
            "<h2 id=\"envelope\"><span class=\"hn\">6.1</span> Envelope <a class=\"anchor\" href=\"#envelope\" aria-label=\"Link to this section\">#</a></h2>\n",
            "<h3 id=\"fields\"><span class=\"hn\">6.1.1</span> Fields <a class=\"anchor\" href=\"#fields\" aria-label=\"Link to this section\">#</a></h3>\n",
            "<h4 id=\"detail\">Detail <a class=\"anchor\" href=\"#detail\" aria-label=\"Link to this section\">#</a></h4>\n",
            "<h2 id=\"outcome-kinds\"><span class=\"hn\">6.2</span> Outcome kinds <a class=\"anchor\"",
            "<h3 id=\"determinate\"><span class=\"hn\">6.2.1</span> Determinate <a class=\"anchor\"",
        ] {
            assert!(body.contains(expected), "missing {expected} in {body}");
        }
    }

    #[test]
    fn the_overview_is_unnumbered() {
        let p = page(
            "# Fidryn documentation\n\n## For users\n\n### Guides\n",
            None,
        );
        assert!(p.body.contains(
            "<h2 id=\"for-users\">For users <a class=\"anchor\" href=\"#for-users\" aria-label=\"Link to this section\">#</a></h2>"
        ));
        assert!(
            p.body
                .contains("<h3 id=\"guides\">Guides <a class=\"anchor\"")
        );
        assert!(!p.body.contains("class=\"hn\""));
        assert!(p.toc.iter().all(|entry| entry.number.is_empty()));
        assert!(p.sections.iter().all(|section| section.number.is_empty()));
    }

    #[test]
    fn an_h3_before_any_h2_is_unnumbered() {
        let p = page("# T\n\n### Early\n\n## First\n\n### Later\n", Some(2));
        let numbers: Vec<(&str, &str)> = p
            .toc
            .iter()
            .map(|e| (e.id.as_str(), e.number.as_str()))
            .collect();
        assert_eq!(
            numbers,
            [("early", ""), ("first", "2.1"), ("later", "2.1.1")]
        );
        assert!(
            p.body
                .contains("<h3 id=\"early\">Early <a class=\"anchor\""),
            "{}",
            p.body
        );
    }

    #[test]
    fn toc_lists_h2_and_h3_in_order() {
        let p = page(
            "# T\n\n## Envelope\n\n### Fields\n\n#### Deep\n\n## Kinds\n",
            Some(6),
        );
        let entry = |level, number: &str, id: &str, text: &str| TocEntry {
            level,
            number: number.to_owned(),
            id: id.to_owned(),
            text: text.to_owned(),
        };
        assert_eq!(
            p.toc,
            [
                entry(2, "6.1", "envelope", "Envelope"),
                entry(3, "6.1.1", "fields", "Fields"),
                entry(2, "6.2", "kinds", "Kinds"),
            ]
        );
    }

    #[test]
    fn slugs_follow_github() {
        for (text, id) in [
            (
                "Surface inventory (status-qualified)",
                "surface-inventory-status-qualified",
            ),
            (
                "POST /api/run and POST /api/explore",
                "post-apirun-and-post-apiexplore",
            ),
            (
                "Delaware, Illinois, Massachusetts, New York, Texas, UCC, everyday",
                "delaware-illinois-massachusetts-new-york-texas-ucc-everyday",
            ),
            (
                "Annotated example: court selects I2",
                "annotated-example-court-selects-i2",
            ),
            ("NormConflict", "normconflict"),
            ("snake_case and kebab-case", "snake_case-and-kebab-case"),
            ("Café § 4 — naïve", "café--4--naïve"),
        ] {
            assert_eq!(slug(text), id, "{text}");
        }
    }

    #[test]
    fn duplicate_headings_get_numbered_suffixes() {
        let p = page(
            "# T\n\n## Fields\n\n## Fields\n\n## Fields 1\n\n## Fields\n\n## §\n",
            None,
        );
        let ids: Vec<&str> = p.toc.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(
            ids,
            ["fields", "fields-1", "fields-1-1", "fields-2", "section"]
        );
    }

    #[test]
    fn inline_code_in_a_heading_keeps_its_markup_and_a_plain_id() {
        let p = page("# CLI\n\n## `run`\n", Some(4));
        assert!(
            p.body.contains(
                "<h2 id=\"run\"><span class=\"hn\">4.1</span> <code>run</code> <a class=\"anchor\" href=\"#run\" aria-label=\"Link to this section\">#</a></h2>"
            ),
            "{}",
            p.body
        );
        assert_eq!(p.toc[0].text, "run");
        assert_eq!(p.sections[0].heading, "run");
    }

    #[test]
    fn an_explicit_heading_id_replaces_the_slug() {
        let p = page("# T\n\n## Envelope {#env}\n", Some(6));
        assert_eq!(p.toc[0].id, "env");
        assert_eq!(p.toc[0].text, "Envelope");
    }

    #[test]
    fn every_table_is_wrapped_in_a_scrolling_region() {
        let md = "# T\n\nIntro.\n\n| Field | Meaning |\n| --- | --- |\n| `schema` | a long cell |\n\n- item\n\n  | a | b |\n  | - | - |\n  | 1 | 2 |\n";
        let body = page(md, Some(7)).body;
        assert_eq!(body.matches("<table>").count(), 2, "{body}");
        assert_eq!(
            body.matches(&format!("{TABLE_WRAP}<table>")).count(),
            2,
            "{body}"
        );
        assert_eq!(body.matches("</table>\n</div>\n").count(), 2, "{body}");
    }

    #[test]
    fn fenced_and_indented_code_become_highlighted_frames() {
        let md = "# T\n\n```json\n{\"a\": 1}\n```\n\n```\ncargo test --offline\n```\n\n```rust\nfn main() {}\n```\n\nIndented:\n\n    module X version \"1\" {}\n";
        let body = page(md, None).body;
        for expected in [
            "<figure class=\"code\" data-lang=\"json\"><figcaption><span>json</span>",
            "<pre><code><span class=\"tk-pu\">{</span><span class=\"tk-ty\">&quot;a&quot;</span>",
            "<figure class=\"code\" data-lang=\"shell\"><figcaption><span>shell</span>",
            "<span class=\"tk-kw\">cargo</span>",
            "<figure class=\"code\" data-lang=\"text\"><figcaption><span>text</span><button type=\"button\" class=\"copy\" data-copy hidden>Copy</button></figcaption><pre><code>fn main() {}</code></pre></figure>\n",
            "<figure class=\"code\" data-lang=\"fr\"><figcaption><span>.fr</span>",
            "<span class=\"tk-kw\">module</span>",
        ] {
            assert!(body.contains(expected), "missing {expected} in {body}");
        }
        assert_eq!(body.matches("<pre").count(), 4);
        assert_eq!(body.matches("<figure class=\"code\"").count(), 4);
    }

    #[test]
    fn links_are_rewritten_for_the_site() {
        let md = "# T\n\n[CLI](cli.md#file), [docs](README.md), [arch](ARCHITECTURE.md), [grammar](../grammar.ebnf), [programs](../tests/programs/), [web](https://example.com/x), [here](#here), <https://fidryn.onlygass.dev/>.\n\n## See [Outcomes](outcomes.md)\n";
        let body = page(md, None).body;
        assert_eq!(
            attrs(&body, "href"),
            [
                "/docs/cli#file",
                "/docs/",
                "https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md",
                "https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf",
                "https://github.com/BeeGass/fidryn/tree/main/tests/programs/",
                "https://example.com/x",
                "#here",
                "https://fidryn.onlygass.dev/",
                "/docs/outcomes",
                "#see-outcomes",
            ]
        );
    }

    #[test]
    fn blockquotes_stay_blockquotes() {
        let body = page("# T\n\n> **What it is not.** A court order.\n", None).body;
        assert_eq!(
            body,
            "<blockquote>\n<p><strong>What it is not.</strong> A court order.</p>\n</blockquote>\n"
        );
    }

    #[test]
    fn lead_and_section_text_are_collapsed_plain_text() {
        let md = "# T\n\nThe `outcome`   is\ntagged by **kind**.\n\n## One\n\nFirst  line\nsecond line.\n\n#### Deep\n\n```\ncargo test\n```\n\n| a | b |\n| - | - |\n| c | d |\n\n## Two\n";
        let p = page(md, Some(6));
        assert_eq!(p.lead, "The outcome is tagged by kind.");
        assert_eq!(p.sections[0].heading, "One");
        assert_eq!(p.sections[0].number, "6.1");
        assert_eq!(
            p.sections[0].text,
            "First line second line. Deep cargo test a b c d"
        );
        assert_eq!(p.sections[1].text, "");
    }

    #[test]
    fn summaries_stop_at_200_characters_on_a_char_boundary() {
        let long = format!("{} {}", "é".repeat(150), "x".repeat(100));
        let p = page(&format!("# T\n\n{long}\n"), None);
        assert_eq!(p.lead.chars().count(), 200);
        assert_eq!(p.lead, format!("{} {}", "é".repeat(150), "x".repeat(49)));
        let p = page(&format!("# T\n\n{} tail\n", "a".repeat(199)), None);
        assert_eq!(p.lead, "a".repeat(199), "no trailing space");
    }

    #[test]
    fn every_guide_renders_with_unique_ids_and_resolving_links() {
        let kw = keywords();
        let docs = workspace_root().join("docs");
        for guide in GUIDES {
            let md = fs::read_to_string(docs.join(guide.file)).expect("read guide");
            let p = render(&md, guide.number, &kw);
            let file = guide.file;
            assert_eq!(
                md.lines().next(),
                Some(format!("# {}", p.title).as_str()),
                "{file} must open with its # title"
            );
            assert!(!p.lead.is_empty(), "{file} has no lead");
            let ids = attrs(&p.body, "id");
            let unique: HashSet<&str> = ids.iter().copied().collect();
            assert_eq!(unique.len(), ids.len(), "{file} repeats an id");
            assert_eq!(p.sections.len(), p.toc.len());
            for entry in &p.toc {
                assert!(unique.contains(entry.id.as_str()), "{file}: {}", entry.id);
            }
            for href in attrs(&p.body, "href") {
                match href.strip_prefix('#') {
                    Some(anchor) => assert!(unique.contains(anchor), "{file}: #{anchor}"),
                    None => assert!(
                        href.starts_with('/')
                            || href.starts_with("https://")
                            || href.starts_with("http://"),
                        "{file}: relative link {href}"
                    ),
                }
            }
            let tables = p.body.matches("<table>").count();
            assert_eq!(
                p.body.matches(&format!("{TABLE_WRAP}<table>")).count(),
                tables,
                "{file}"
            );
            assert_eq!(
                p.body.matches("</table>\n</div>\n").count(),
                tables,
                "{file}"
            );
            let frames = p.body.matches("<figure class=\"code\"").count();
            assert_eq!(p.body.matches("<pre").count(), frames, "{file}");
        }
    }

    #[test]
    fn the_outcomes_guide_has_the_anchors_the_landing_page_links_to() {
        let md = fs::read_to_string(workspace_root().join("docs/outcomes.md")).expect("read guide");
        let body = render(&md, Some(6), &keywords()).body;
        let ids = attrs(&body, "id");
        for id in [
            "determinate",
            "contingent",
            "suspended",
            "normconflict",
            "outsidecompetence",
            "inconsistent",
        ] {
            assert!(ids.contains(&id), "missing #{id}");
        }
    }
}
````

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p xtask --offline site::`
Expected: FAIL to compile, with errors such as ``error[E0425]: cannot find function `rewrite` in this scope``, ``error[E0433]: cannot find type `LinkStyle` in this scope``, ``error[E0425]: cannot find function `render` in this scope``, ``error[E0425]: cannot find value `TABLE_WRAP` in this scope``, ``error[E0433]: cannot find type `Parser` in this scope``, ending with ``error: could not compile `xtask` (bin "xtask" test)``.

- [ ] **Step 3: Write the implementation**

In `xtask/Cargo.toml`, replace:

```toml
fidryn-syntax.workspace = true
```

with:

```toml
fidryn-syntax.workspace = true
pulldown-cmark = { version = "0.13", default-features = false, features = ["html"] }
```

In `xtask/src/site/links.rs`, replace the line `#[cfg(test)]` with:

```rust
use super::guides::{self, REPO, SITE};
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};

/// Where a rewritten link will be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkStyle {
    /// Rendered pages: guides become site paths such as `/docs/cli#file`.
    Html,
    /// Markdown mirrors: guides become absolute `.md` URLs.
    Markdown,
}

/// The target of a link written in `docs/*.md`. Guides stay on the site
/// (`README.md` is the overview at `/docs/`); other files under `docs/` and
/// `../` repository paths become GitHub `blob/main` URLs, or `tree/main` for
/// directories (a trailing `/`). Fragments are kept. Absolute URLs, `/…`,
/// `#…`, and `mailto:` targets are returned unchanged.
pub fn rewrite(href: &str, style: LinkStyle) -> String {
    if href.is_empty() || href.starts_with(['#', '/']) || has_scheme(href) {
        return href.to_owned();
    }
    let (path, fragment) = href.split_at(href.find('#').unwrap_or(href.len()));
    let repo_path = match path.strip_prefix("../") {
        Some(outside_docs) => outside_docs.to_owned(),
        None => {
            let file = path.strip_prefix("./").unwrap_or(path);
            if let Some(guide) = guides::by_file(file) {
                return match style {
                    LinkStyle::Html => format!("{}{fragment}", guides::url(guide.slug)),
                    LinkStyle::Markdown => format!("{SITE}/docs/{}.md{fragment}", guide.slug),
                };
            }
            format!("docs/{file}")
        }
    };
    let kind = if repo_path.is_empty() || repo_path.ends_with('/') {
        "tree"
    } else {
        "blob"
    };
    format!("{REPO}/{kind}/main/{repo_path}{fragment}")
}

/// `md` with the target of every inline link rewritten with
/// [`LinkStyle::Markdown`]. Everything else, code included, is kept byte for
/// byte.
pub fn rewrite_markdown_links(md: &str) -> String {
    let mut out = String::with_capacity(md.len() + 1024);
    let mut copied = 0;
    for (event, range) in Parser::new_ext(md, Options::ENABLE_TABLES).into_offset_iter() {
        let Event::Start(Tag::Link {
            link_type: LinkType::Inline,
            dest_url,
            ..
        }) = event
        else {
            continue;
        };
        let Some(open) = md[range.clone()].rfind("](") else {
            continue;
        };
        let mut start = range.start + open + 2;
        if md[start..].starts_with('<') {
            start += 1;
        }
        if start < copied || !md[start..].starts_with(dest_url.as_ref()) {
            continue;
        }
        out.push_str(&md[copied..start]);
        out.push_str(&rewrite(&dest_url, LinkStyle::Markdown));
        copied = start + dest_url.len();
    }
    out.push_str(&md[copied..]);
    out
}

/// Whether `href` starts with a URL scheme such as `https:` or `mailto:`.
fn has_scheme(href: &str) -> bool {
    href.split_once(':').is_some_and(|(scheme, _)| {
        scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

#[cfg(test)]
```

In `xtask/src/site/markdown.rs`, replace the line `#[cfg(test)]` with:

```rust
use super::highlight::{self, Keywords};
use super::html::esc;
use super::links::{self, LinkStyle};
use pulldown_cmark::{
    CodeBlockKind, CowStr, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd, html,
};
use std::collections::HashMap;
use std::mem;

/// One "On this page" entry: an `h2` or `h3`. Text fields are plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TocEntry {
    /// 2 or 3.
    pub level: u8,
    /// `6.1` or `6.1.2`; empty when the heading is unnumbered.
    pub number: String,
    pub id: String,
    pub text: String,
}

/// A searchable part of a page: one per `h2`/`h3`. Text fields are plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Section {
    pub number: String,
    pub id: String,
    pub heading: String,
    /// The first 200 characters of the section's text.
    pub text: String,
}

/// A rendered guide. `body` is HTML; every other text field is plain text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// The first `h1`, which is not part of `body`.
    pub title: String,
    /// The first 200 characters of the text before the first `h2`/`h3`.
    pub lead: String,
    pub body: String,
    pub toc: Vec<TocEntry>,
    pub sections: Vec<Section>,
}

/// Opening tag of the region every table scrolls inside.
const TABLE_WRAP: &str =
    "<div class=\"table-wrap\" tabindex=\"0\" role=\"region\" aria-label=\"Table\">";

/// Length of `Page::lead` and `Section::text`, in characters.
const SUMMARY_CHARS: usize = 200;

/// Render one guide. `guide_number` numbers its `h2`/`h3` headings (`6.1`,
/// `6.1.2`); `None`, for the overview, leaves them unnumbered. An `h3`
/// before any `h2` is never numbered.
pub fn render(md: &str, guide_number: Option<u32>, kw: &Keywords) -> Page {
    let mut parser = Parser::new_ext(
        md,
        Options::ENABLE_TABLES | Options::ENABLE_HEADING_ATTRIBUTES,
    );
    let mut page = Outline {
        guide_number,
        ..Outline::default()
    };
    let mut out = Vec::new();
    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::Heading { level, id, .. }) => {
                let inner: Vec<Event<'_>> = parser
                    .by_ref()
                    .take_while(|e| !matches!(e, Event::End(TagEnd::Heading(_))))
                    .collect();
                let text = collapse(&inline_text(&inner));
                if level == HeadingLevel::H1 && page.title.is_none() {
                    page.title = Some(text);
                } else {
                    let (id, number) = page.heading(level, id, text);
                    push_heading(&mut out, level, &id, &number, inner);
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let code = code_text(&mut parser);
                let info = match &kind {
                    CodeBlockKind::Fenced(info) => info.as_ref(),
                    CodeBlockKind::Indented => "",
                };
                let lang = highlight::sniff(info, &code, kw);
                let frame = highlight::code_frame(lang, &code, kw, None);
                out.push(Event::Html(format!("{frame}\n").into()));
                page.text.push_str(&code);
                page.text.push(' ');
            }
            Event::Start(Tag::Table(_)) => {
                out.push(Event::Html(TABLE_WRAP.into()));
                out.push(event);
            }
            Event::End(TagEnd::Table) => {
                out.push(event);
                out.push(Event::Html("</div>\n".into()));
            }
            event => {
                page.read(&event);
                out.push(site_link(event));
            }
        }
    }
    page.close_part();
    let mut body = String::with_capacity(md.len() * 2);
    html::push_html(&mut body, out.into_iter());
    Page {
        title: page.title.unwrap_or_default(),
        lead: page.lead,
        body,
        toc: page.toc,
        sections: page.sections,
    }
}

/// GitHub-style heading id: lowercase, each space becomes `-`, and every
/// character other than a letter, digit, `-`, or `_` is dropped.
pub fn slug(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_alphanumeric() => Some(c),
            _ => None,
        })
        .collect()
}

/// What `render` learns about a page besides its HTML.
#[derive(Default)]
struct Outline {
    guide_number: Option<u32>,
    title: Option<String>,
    ids: Ids,
    h2: u32,
    h3: u32,
    toc: Vec<TocEntry>,
    sections: Vec<Section>,
    lead: String,
    /// Plain text read since the last `h2`/`h3`, or since the start.
    text: String,
}

impl Outline {
    /// Give a heading its id and number. An `h2`/`h3` also starts a section
    /// and a TOC entry; a deeper heading's text belongs to the current part.
    fn heading(
        &mut self,
        level: HeadingLevel,
        explicit_id: Option<CowStr<'_>>,
        text: String,
    ) -> (String, String) {
        let base = explicit_id.map_or_else(|| slug(&text), CowStr::into_string);
        let id = self.ids.unique(if base.is_empty() {
            "section".to_owned()
        } else {
            base
        });
        let number = match level {
            HeadingLevel::H2 => {
                self.h2 += 1;
                self.h3 = 0;
                self.guide_number.map(|n| format!("{n}.{}", self.h2))
            }
            HeadingLevel::H3 if self.h2 > 0 => {
                self.h3 += 1;
                self.guide_number
                    .map(|n| format!("{n}.{}.{}", self.h2, self.h3))
            }
            _ => None,
        }
        .unwrap_or_default();
        if matches!(level, HeadingLevel::H2 | HeadingLevel::H3) {
            self.close_part();
            self.toc.push(TocEntry {
                level: level as u8,
                number: number.clone(),
                id: id.clone(),
                text: text.clone(),
            });
            self.sections.push(Section {
                number: number.clone(),
                id: id.clone(),
                heading: text,
                text: String::new(),
            });
        } else {
            self.text.push_str(&text);
            self.text.push(' ');
        }
        (id, number)
    }

    /// Keep the text a reader sees, with a space wherever a block or line ends.
    fn read(&mut self, event: &Event<'_>) {
        match event {
            Event::Text(chunk) | Event::Code(chunk) => self.text.push_str(chunk),
            Event::SoftBreak
            | Event::HardBreak
            | Event::Rule
            | Event::Start(Tag::Item | Tag::List(_))
            | Event::End(
                TagEnd::Paragraph | TagEnd::Item | TagEnd::TableCell | TagEnd::BlockQuote(_),
            ) => self.text.push(' '),
            _ => {}
        }
    }

    /// The text read so far becomes the lead, or the last section's text.
    fn close_part(&mut self) {
        let done = summary(&mem::take(&mut self.text));
        match self.sections.last_mut() {
            Some(section) => section.text = done,
            None => self.lead = done,
        }
    }
}

/// Heading ids handed out so far, de-duplicated the way GitHub does it:
/// the second `x` becomes `x-1`, the third `x-2`.
#[derive(Default)]
struct Ids(HashMap<String, usize>);

impl Ids {
    fn unique(&mut self, base: String) -> String {
        let mut id = base.clone();
        while self.0.contains_key(&id) {
            let count = self.0.get_mut(&base).expect("the base id is taken first");
            *count += 1;
            id = format!("{base}-{count}");
        }
        self.0.insert(id.clone(), 0);
        id
    }
}

/// `<h2 id="…"><span class="hn">6.1</span> Text <a class="anchor" …>#</a></h2>`.
fn push_heading<'a>(
    out: &mut Vec<Event<'a>>,
    level: HeadingLevel,
    id: &str,
    number: &str,
    inner: Vec<Event<'a>>,
) {
    let id = esc(id);
    let mut open = format!("<{level} id=\"{id}\">");
    if !number.is_empty() {
        open.push_str(&format!("<span class=\"hn\">{number}</span> "));
    }
    out.push(Event::Html(open.into()));
    out.extend(inner.into_iter().map(site_link));
    out.push(Event::Html(
        format!(
            " <a class=\"anchor\" href=\"#{id}\" aria-label=\"Link to this section\">#</a></{level}>\n"
        )
        .into(),
    ));
}

/// The text of the code block whose start tag was just read.
fn code_text(parser: &mut Parser<'_>) -> String {
    let mut code = String::new();
    for event in parser.by_ref() {
        match event {
            Event::Text(chunk) => code.push_str(&chunk),
            Event::End(TagEnd::CodeBlock) => break,
            _ => {}
        }
    }
    code
}

/// A link event with its target rewritten for the site.
fn site_link(event: Event<'_>) -> Event<'_> {
    match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) if link_type != LinkType::Email => {
            let dest_url = links::rewrite(&dest_url, LinkStyle::Html).into();
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            })
        }
        other => other,
    }
}

/// The plain text of inline events, such as a heading's content.
fn inline_text(events: &[Event<'_>]) -> String {
    let mut text = String::new();
    for event in events {
        match event {
            Event::Text(chunk) | Event::Code(chunk) => text.push_str(chunk),
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            _ => {}
        }
    }
    text
}

/// `text` with every run of whitespace turned into one space, trimmed.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first 200 characters of `text` after collapsing whitespace, cut at a
/// character boundary, without a trailing space.
fn summary(text: &str) -> String {
    let collapsed = collapse(text);
    match collapsed.char_indices().nth(SUMMARY_CHARS) {
        Some((cut, _)) => collapsed[..cut].trim_end().to_owned(),
        None => collapsed,
    }
}

#[cfg(test)]
```

- [ ] **Step 4: Run the tests and watch the real guides expose a bad source**

Run: `cargo test -p xtask --offline site::`
Expected: FAIL with `test result: FAILED. 64 passed; 1 failed`. The failure is `site::markdown::tests::every_guide_renders_with_unique_ids_and_resolving_links`:

```text
assertion `left == right` failed: examples.md must open with its # title
  left: Some("---")
 right: Some("# Example corpus")
```

`docs/examples.md` begins with a copy of the old Markdown mirror's front matter and "Canonical HTML" note (committed in `8534ebe`). pulldown-cmark reads that as a horizontal rule and a setext `h2` whose text is the YAML, so the Examples page would open with a numbered heading `7.1 title: "Examples" description: …`, and Task 11's mirror would repeat the header. The mirror header belongs to the generated `site/docs/examples.md`, never to the source.

- [ ] **Step 5: Remove the stray header from `docs/examples.md`**

In `docs/examples.md`, replace the first eleven lines:

```markdown
---
title: "Examples"
description: "Trust, tax, federal slices, and the fifty-state corpus map for Fidryn."
url: "https://fidryn.onlygass.dev/docs/examples"
markdown: "https://fidryn.onlygass.dev/docs/examples.md"
author: "Bryan Gass"
---

> Canonical HTML: https://fidryn.onlygass.dev/docs/examples
> This markdown mirror is for agents and plain-text readers.
# Example corpus
```

with:

```markdown
# Example corpus
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline`
Expected: PASS. `test result: ok. 65 passed; 0 failed` then `test result: ok. 9 passed; 0 failed`. Dead-code warnings for `render`, `rewrite_markdown_links`, `numbered_frame`, and the Task 4 items are expected until Tasks 9–11 call them.

Run: `cargo fmt -p xtask -- --check`
Expected: no output, exit status 0.

- [ ] **Step 7: Commit**

```bash
git add docs/examples.md
git -c commit.gpgsign=false commit -m "docs: drop the stray mirror header from the examples guide"
git add xtask/Cargo.toml Cargo.lock xtask/src/site/mod.rs xtask/src/site/links.rs xtask/src/site/markdown.rs
git -c commit.gpgsign=false commit -m "feat(site): render guides with numbered headings, framed code, and site links"
```

### Task 9: Landing specimen from real runs

**Files:**
- Create: `xtask/src/site/specimen.rs`
- Modify: `xtask/src/site/mod.rs` (the module list: add `mod specimen;` after `mod seo;`)
- Modify: `xtask/Cargo.toml` (`[dependencies]`: add `fidryn-cli` and `serde_json`)
- Modify: `Cargo.lock` (updated by cargo for the two new xtask dependencies)
- Test: `xtask/src/site/specimen.rs` (`mod tests`)

**Interfaces:**
- Consumes: `fidryn_cli::{CaseInput, RunRequest, run_report_text}` with `run_report_text(&RunRequest<'_>) -> Result<String, String>` (Task 1) and `fidryn_cli::opinion::sentences(&serde_json::Value) -> Vec<String>` (Task 2), both as in contract C2; `highlight::{Keywords, Lang}`, `Keywords::load(&Path) -> anyhow::Result<Keywords>`, `highlight::code_frame(Lang, &str, &Keywords, Option<&str>) -> String`, `highlight::numbered_frame(Lang, &[(usize, String)], &Keywords, &str) -> String` (Task 7); `fidryn_syntax::{lex, TokenKind}`; `crate::workspace::workspace_root() -> PathBuf`.
- Produces: `pub struct Run { pub id: &'static str, pub label: &'static str, pub command: String, pub source_html: String, pub case_html: String, pub kind: String, pub opinion: Vec<String> }`; `pub fn runs(root: &Path, kw: &Keywords) -> anyhow::Result<Vec<Run>>` (tab order: `trust-open`, `trust-court`, `gate-q`, `gate-r`); `pub fn excerpt(src: &str, starts: &[&str]) -> Vec<(usize, String)>`. Task 10 lays a `Run` out; Task 11's `build` calls `runs`.

The four runs (contracts C1 and C3). Each case file is read as text and evaluated with `CaseInput::Json`, and the same text goes into the Case frame, so the page shows exactly what was evaluated.

| id | label | module | case (caption) | query | valid-at = known-at | kind | first opinion sentence |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `trust-open` | Trust, open eligibility | `examples/trust/bryan-revocable-trust.fr` (excerpt) | `examples/trust/cases/two-certificates-open-eligibility.json` | `acting_trustee` | `2034-03-01T09:00:00Z` | `contingent` | `acting_trustee depends on SuccessorEligibility.` |
| `trust-court` | Trust, court selects I2 | same (excerpt) | `examples/trust/cases/court-selects-i2.json` | `acting_trustee` | `2034-03-01T09:00:00Z` | `determinate` | `acting_trustee is Bob.` |
| `gate-q` | require-gate, query q | `tests/programs/require-gate.fr` (whole module) | the pretty empty case record (`empty case record`) | `q` | `2026-09-17T12:00:00Z` | `determinate` | `q is 7.` |
| `gate-r` | require-gate, query r | same (whole module) | the pretty empty case record (`empty case record`) | `r` | `2026-09-17T12:00:00Z` | `suspended` | `r is suspended.` |

The trust excerpt is `excerpt(src, &["interpretation_family SuccessorEligibility", "query acting_trustee"])`, checked against the real file: the family block is lines 47 to 63 (its `{` is on line 49), and the query is lines 205 to 212. Line 207, `! {Observe, Determine, Interpret}`, is an effect set whose braces close on the same line and are followed directly by the body's `{` on line 208, so a block ends only at a close brace that brings the depth to zero and is not followed by `{`. Braces are counted on `fidryn_syntax::lex` tokens, so braces inside strings and comments never count. The gate runs' command names `/tmp/fidryn-empty-case.json`, the file the Getting started guide has readers write; the repository has no empty case file.

- [ ] **Step 1: Write the failing test**

In `xtask/src/site/mod.rs`, find the line

```rust
mod seo;
```

and replace it with

```rust
mod seo;
mod specimen;
```

Create `xtask/src/site/specimen.rs` with the module doc comment and the tests (the implementation goes between them in Step 3):

```rust
//! The landing specimen: four real runs, evaluated when the site is built
//! through the same library path as `fidryn run`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_root;

    fn keywords() -> Keywords {
        Keywords::load(&workspace_root()).expect("grammar.ebnf")
    }

    /// Text inside the frame's `<pre><code>`, with highlight spans removed and
    /// entities decoded.
    fn frame_text(frame: &str) -> String {
        let start = frame.find("<code>").expect("frame has <code>") + "<code>".len();
        let end = frame.rfind("</code>").expect("frame has </code>");
        let mut text = String::new();
        let mut rest = &frame[start..end];
        while let Some(open) = rest.find('<') {
            text.push_str(&rest[..open]);
            let close = rest[open..].find('>').expect("tag closes") + open;
            rest = &rest[close + 1..];
        }
        text.push_str(rest);
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
    }

    #[test]
    fn excerpt_keeps_the_real_line_numbers_of_the_trust_blocks() {
        let src = fs::read_to_string(workspace_root().join(TRUST)).unwrap();
        let blocks = excerpt(&src, TRUST_EXCERPT);
        assert_eq!(blocks.len(), 2, "{blocks:?}");

        let (first, family) = &blocks[0];
        assert_eq!(*first, 47);
        assert!(
            family.starts_with("    interpretation_family SuccessorEligibility\n"),
            "{family}"
        );
        assert!(
            family.ends_with("          or request Interpret\n    }"),
            "{family}"
        );
        assert_eq!(family.lines().count(), 17, "lines 47 through 63");

        let (first, query) = &blocks[1];
        assert_eq!(*first, 205);
        assert!(query.starts_with("    query acting_trustee()\n"), "{query}");
        assert!(
            query.ends_with("            office TrusteeOf(BRT)\n        }\n    }"),
            "{query}"
        );
        assert_eq!(query.lines().count(), 8, "lines 205 through 212");

        let file: Vec<&str> = src.lines().collect();
        for (first, text) in &blocks {
            for (i, line) in text.lines().enumerate() {
                assert_eq!(line, file[first - 1 + i], "line {}", first + i);
            }
        }
    }

    #[test]
    fn excerpt_counts_braces_on_tokens_not_characters() {
        let src = concat!(
            "module M version \"1\" {\n",
            "    query a() -> Text ! {Observe}\n",
            "    {\n",
            "        // a } in a comment\n",
            "        return \"}\"\n",
            "    }\n",
            "    query b() -> Int { return 1 }\n",
            "}\n",
        );
        let a = "    query a() -> Text ! {Observe}\n    {\n        // a } in a comment\n        return \"}\"\n    }";
        assert_eq!(
            excerpt(src, &["query a", "query b"]),
            vec![
                (2, a.to_owned()),
                (7, "    query b() -> Int { return 1 }".to_owned())
            ]
        );
    }

    #[test]
    fn excerpt_skips_missing_and_unclosed_blocks() {
        let src = "module M version \"1\" {\n    query a() -> Int {\n        return 1\n";
        assert!(excerpt(src, &["query a"]).is_empty(), "unclosed block");
        assert!(excerpt(src, &["query zz"]).is_empty(), "missing prefix");
    }

    #[test]
    fn runs_are_the_four_specimen_runs_with_real_outcomes() {
        let runs = runs(&workspace_root(), &keywords()).expect("specimen runs");
        let got: Vec<(&str, &str, &str, &str)> = runs
            .iter()
            .map(|r| (r.id, r.label, r.kind.as_str(), r.opinion[0].as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    "trust-open",
                    "Trust, open eligibility",
                    "contingent",
                    "acting_trustee depends on SuccessorEligibility."
                ),
                (
                    "trust-court",
                    "Trust, court selects I2",
                    "determinate",
                    "acting_trustee is Bob."
                ),
                ("gate-q", "require-gate, query q", "determinate", "q is 7."),
                (
                    "gate-r",
                    "require-gate, query r",
                    "suspended",
                    "r is suspended."
                ),
            ]
        );
        let open = &runs[0].opinion;
        assert!(
            open.contains(&"Under I1 it is Alice.".to_owned()),
            "{open:?}"
        );
        assert!(open.contains(&"Under I2 it is Bob.".to_owned()), "{open:?}");
        assert_eq!(
            open.last().map(String::as_str),
            Some(
                "Outside scope: tax, creditor_priority, real_property_recording, complete_Massachusetts_trust_law."
            )
        );
        assert_eq!(
            runs[3].opinion.last().map(String::as_str),
            Some("Outside scope: complete_instruments.")
        );
    }

    #[test]
    fn commands_are_the_equivalent_cli_lines() {
        let runs = runs(&workspace_root(), &keywords()).expect("specimen runs");
        let commands: Vec<&str> = runs.iter().map(|r| r.command.as_str()).collect();
        assert_eq!(
            commands,
            vec![
                "fidryn run examples/trust/bryan-revocable-trust.fr --query acting_trustee --case examples/trust/cases/two-certificates-open-eligibility.json --valid-at 2034-03-01T09:00:00Z --known-at 2034-03-01T09:00:00Z",
                "fidryn run examples/trust/bryan-revocable-trust.fr --query acting_trustee --case examples/trust/cases/court-selects-i2.json --valid-at 2034-03-01T09:00:00Z --known-at 2034-03-01T09:00:00Z",
                "fidryn run tests/programs/require-gate.fr --query q --case /tmp/fidryn-empty-case.json --valid-at 2026-09-17T12:00:00Z --known-at 2026-09-17T12:00:00Z",
                "fidryn run tests/programs/require-gate.fr --query r --case /tmp/fidryn-empty-case.json --valid-at 2026-09-17T12:00:00Z --known-at 2026-09-17T12:00:00Z",
            ]
        );
    }

    #[test]
    fn frames_show_the_numbered_source_and_the_evaluated_case() {
        let root = workspace_root();
        let runs = runs(&root, &keywords()).expect("specimen runs");

        let trust = &runs[0].source_html;
        assert!(trust.contains(&format!("<span>{TRUST}</span>")), "{trust}");
        for n in ["47", "63", "205", "212"] {
            assert!(
                trust.contains(&format!("data-n=\"{n}\"")),
                "line {n} missing"
            );
        }
        for n in ["46", "64", "204", "213"] {
            assert!(
                !trust.contains(&format!("data-n=\"{n}\"")),
                "line {n} should be cut"
            );
        }
        assert!(
            trust.contains("class=\"line gap\""),
            "gap between the two blocks"
        );

        let gate = &runs[2].source_html;
        assert!(
            gate.contains("data-n=\"1\"") && gate.contains("data-n=\"11\""),
            "{gate}"
        );
        assert!(!gate.contains("class=\"line gap\""), "{gate}");

        let case_file = fs::read_to_string(
            root.join("examples/trust/cases/two-certificates-open-eligibility.json"),
        )
        .unwrap();
        assert!(
            runs[0].case_html.contains(
                "<span>examples/trust/cases/two-certificates-open-eligibility.json</span>"
            )
        );
        assert_eq!(
            frame_text(&runs[0].case_html),
            case_file.trim_end_matches('\n')
        );
        assert!(runs[2].case_html.contains("<span>empty case record</span>"));
        assert_eq!(
            frame_text(&runs[2].case_html),
            EMPTY_CASE.trim_end_matches('\n')
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --offline -p xtask site::specimen`
Expected: FAIL to compile, with ``error[E0425]: cannot find function `excerpt` in this scope``, the same error for `runs`, `TRUST`, `TRUST_EXCERPT`, and `EMPTY_CASE`, ``cannot find type `Keywords` in this scope``, and ``cannot find module or crate `fs` in this scope``.

- [ ] **Step 3: Write the implementation**

In `xtask/Cargo.toml`, find

```toml
clap.workspace = true
```

and replace it with

```toml
clap.workspace = true
fidryn-cli.workspace = true
```

and find

```toml
pulldown-cmark = { version = "0.13", default-features = false, features = ["html"] }
```

and replace it with

```toml
pulldown-cmark = { version = "0.13", default-features = false, features = ["html"] }
serde_json.workspace = true
```

The table then reads:

```toml
[dependencies]
anyhow.workspace = true
cargo_metadata = "0.23"
clap.workspace = true
fidryn-cli.workspace = true
fidryn-syntax.workspace = true
pulldown-cmark = { version = "0.13", default-features = false, features = ["html"] }
serde_json.workspace = true
```

The workspace `serde_json` entry enables `preserve_order`, so report and JSON-LD keys keep their order. In `xtask/src/site/specimen.rs`, insert the following between the two `//!` lines at the top and the `#[cfg(test)]` line, with one blank line on each side:

```rust
use super::highlight::{self, Keywords, Lang};
use anyhow::{Context, Result, anyhow, bail};
use fidryn_cli::{CaseInput, RunRequest, run_report_text};
use fidryn_syntax::{TokenKind, lex};
use std::fs;
use std::path::Path;

/// One specimen run, ready to lay out.
pub struct Run {
    /// Tab and panel id suffix: `trust-open`, `trust-court`, `gate-q`, `gate-r`.
    pub id: &'static str,
    /// Tab label.
    pub label: &'static str,
    /// The equivalent `fidryn run …` command line, with repo-relative paths.
    pub command: String,
    /// The module, or an excerpt of it, with the file's real line numbers.
    pub source_html: String,
    /// The exact case JSON that was evaluated.
    pub case_html: String,
    /// Outcome kind as the report spells it, for example `contingent`.
    pub kind: String,
    /// The report in sentences, worded as the mill words them.
    pub opinion: Vec<String>,
}

const TRUST: &str = "examples/trust/bryan-revocable-trust.fr";
const GATE: &str = "tests/programs/require-gate.fr";

/// The empty case record, pretty-printed exactly as the mill's samples serve it.
const EMPTY_CASE: &str =
    "{\n  \"schema\": \"fidryn.case-record/v0.1\",\n  \"admissibleCompletions\": {}\n}\n";

/// Where the Getting started guide writes the empty case record.
const EMPTY_CASE_PATH: &str = "/tmp/fidryn-empty-case.json";

/// First lines of the two trust blocks the specimen shows.
const TRUST_EXCERPT: &[&str] = &[
    "interpretation_family SuccessorEligibility",
    "query acting_trustee",
];

struct Spec {
    id: &'static str,
    label: &'static str,
    module: &'static str,
    /// First lines of the blocks to show; empty shows the whole module.
    excerpt: &'static [&'static str],
    /// Case file under the repository root; `None` is the empty case record.
    case: Option<&'static str>,
    query: &'static str,
    /// Used for both `--valid-at` and `--known-at`.
    at: &'static str,
}

const SPECS: &[Spec] = &[
    Spec {
        id: "trust-open",
        label: "Trust, open eligibility",
        module: TRUST,
        excerpt: TRUST_EXCERPT,
        case: Some("examples/trust/cases/two-certificates-open-eligibility.json"),
        query: "acting_trustee",
        at: "2034-03-01T09:00:00Z",
    },
    Spec {
        id: "trust-court",
        label: "Trust, court selects I2",
        module: TRUST,
        excerpt: TRUST_EXCERPT,
        case: Some("examples/trust/cases/court-selects-i2.json"),
        query: "acting_trustee",
        at: "2034-03-01T09:00:00Z",
    },
    Spec {
        id: "gate-q",
        label: "require-gate, query q",
        module: GATE,
        excerpt: &[],
        case: None,
        query: "q",
        at: "2026-09-17T12:00:00Z",
    },
    Spec {
        id: "gate-r",
        label: "require-gate, query r",
        module: GATE,
        excerpt: &[],
        case: None,
        query: "r",
        at: "2026-09-17T12:00:00Z",
    },
];

/// Evaluate the four specimen runs, in tab order.
pub fn runs(root: &Path, kw: &Keywords) -> Result<Vec<Run>> {
    SPECS.iter().map(|spec| run(root, kw, spec)).collect()
}

fn run(root: &Path, kw: &Keywords, spec: &Spec) -> Result<Run> {
    let module_path = root.join(spec.module);
    let src = fs::read_to_string(&module_path)
        .with_context(|| format!("read {}", module_path.display()))?;
    let segments = if spec.excerpt.is_empty() {
        vec![(1, src.trim_end().to_owned())]
    } else {
        let blocks = excerpt(&src, spec.excerpt);
        if blocks.len() != spec.excerpt.len() {
            bail!(
                "{}: expected {} blocks starting with {:?}, found {}",
                spec.module,
                spec.excerpt.len(),
                spec.excerpt,
                blocks.len()
            );
        }
        blocks
    };
    let (case_text, case_caption) = match spec.case {
        Some(rel) => {
            let path = root.join(rel);
            let text =
                fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
            (text, rel)
        }
        None => (EMPTY_CASE.to_owned(), "empty case record"),
    };
    let command = format!(
        "fidryn run {} --query {} --case {} --valid-at {at} --known-at {at}",
        spec.module,
        spec.query,
        spec.case.unwrap_or(EMPTY_CASE_PATH),
        at = spec.at,
    );
    let report_text = run_report_text(&RunRequest {
        path: &module_path,
        query: spec.query,
        case: CaseInput::Json(&case_text),
        valid_at: spec.at,
        known_at: spec.at,
        args: &[],
        scenario: false,
    })
    .map_err(|err| anyhow!("specimen `{command}` failed: {err}"))?;
    let report: serde_json::Value = serde_json::from_str(&report_text)
        .with_context(|| format!("specimen `{command}` printed invalid JSON"))?;
    let kind = report["outcomeDocument"]["outcome"]["kind"]
        .as_str()
        .ok_or_else(|| anyhow!("specimen `{command}`: the report has no outcome kind"))?
        .to_owned();
    Ok(Run {
        id: spec.id,
        label: spec.label,
        command,
        source_html: highlight::numbered_frame(Lang::Fr, &segments, kw, spec.module),
        case_html: highlight::code_frame(Lang::Json, &case_text, kw, Some(case_caption)),
        kind,
        opinion: fidryn_cli::opinion::sentences(&report),
    })
}

/// Blocks of `src` whose first line, trimmed, starts with each prefix in
/// `starts`, in that order. A block runs from that line through the line of
/// the brace that closes it, and comes with the 1-based number of its first
/// line. Braces are counted on lexer tokens, so braces in strings and
/// comments do not count, and a brace group followed directly by `{` (the
/// effect set in `query q() -> T ! {Observe} {`) does not end the block. A
/// prefix with no matching line, or whose block never closes, yields nothing;
/// callers compare the count.
pub fn excerpt(src: &str, starts: &[&str]) -> Vec<(usize, String)> {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut offsets = Vec::with_capacity(lines.len());
    let mut at = 0;
    for line in &lines {
        offsets.push(at);
        at += line.len() + 1;
    }
    let tokens: Vec<_> = lex(src)
        .into_iter()
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::Whitespace
                    | TokenKind::Newline
                    | TokenKind::Comment
                    | TokenKind::DocComment
            )
        })
        .collect();
    let line_of = |pos: usize| offsets.partition_point(|&o| o <= pos) - 1;
    let mut out = Vec::new();
    for prefix in starts {
        let Some(first) = lines
            .iter()
            .position(|l| l.trim_start().starts_with(prefix))
        else {
            continue;
        };
        let from = tokens.partition_point(|t| (t.start as usize) < offsets[first]);
        let mut depth = 0usize;
        let mut last = None;
        for (i, token) in tokens.iter().enumerate().skip(from) {
            match token.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace if depth == 0 => break,
                TokenKind::RBrace => {
                    depth -= 1;
                    let reopens = tokens
                        .get(i + 1)
                        .is_some_and(|t| t.kind == TokenKind::LBrace);
                    if depth == 0 && !reopens {
                        last = Some(line_of(token.start as usize));
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(last) = last {
            let text: Vec<&str> = lines[first..=last]
                .iter()
                .map(|l| l.trim_end_matches('\r'))
                .collect();
            out.push((first + 1, text.join("\n")));
        }
    }
    out
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --offline -p xtask site::specimen`
Expected: PASS (`test result: ok. 6 passed`). The first run compiles `fidryn-cli` and its dependencies into xtask, which takes a few minutes once. Until Task 11 makes `build` call them, rustc warns that `runs`, `excerpt`, `Run`, and the other generator items from Tasks 4 to 9 are never used; that is expected and needs no change.

- [ ] **Step 5: Commit**

```bash
git add xtask/Cargo.toml Cargo.lock xtask/src/site/mod.rs xtask/src/site/specimen.rs
git -c commit.gpgsign=false commit -m "feat(site): evaluate the landing specimen runs at build time"
```

### Task 10: Page templates and pages

**Files:**
- Create: `xtask/src/site/templates/base.html`, `xtask/src/site/templates/landing.html`, `xtask/src/site/templates/doc.html`, `xtask/src/site/templates/404.html`
- Create: `xtask/src/site/pages.rs`
- Modify: `xtask/src/site/seo.rs` (import `html::esc`; add `LANDING_TITLE`, `LANDING_DESCRIPTION`, `PageKind`, and `head` above Task 4's test module; append `mod head_tests` at the end)
- Modify: `xtask/src/site/mod.rs` (the module list: add `mod pages;` before `mod seo;`)
- Test: `xtask/src/site/pages.rs` (`mod tests`), `xtask/src/site/seo.rs` (`mod head_tests`)

**Interfaces:**
- Consumes: `guides::{GUIDES, GROUPS, IMPLEMENTER_DOCS, REPO, SITE, Guide, url}`, `html::{esc, asset_version}`, `templates::fill(&str, &[(&str, &str)]) -> String` (Task 4, contract C4); `markdown::{Page, TocEntry}` (Task 8); `specimen::Run` (Task 9).
- Produces: `pub struct Assets { pub css: String, pub js: String }` with `Assets::read(root: &Path) -> anyhow::Result<Assets>`; `pub fn stamp(kind: &str) -> (&'static str, &'static str)`; `pub fn guides_nav(current: Option<&str>) -> String`; `pub fn landing(runs: &[specimen::Run], assets: &Assets) -> String`; `pub fn doc(guide: &guides::Guide, page: &markdown::Page, assets: &Assets) -> String`; `pub fn not_found(assets: &Assets) -> String`; in `seo.rs`: `pub enum PageKind { Landing, Doc, NotFound }`, `pub fn head(title: &str, description: &str, path: &str, markdown_path: Option<&str>, kind: PageKind) -> String`, and `pub const LANDING_TITLE: &str`, `pub const LANDING_DESCRIPTION: &str` (Task 11 reuses both for `index.md` and `llms.txt`).

Choices the contract leaves open, fixed here and pinned by the tests: a docs page's `<title>` is `{Guide.title} — Fidryn` and its `<h1>` is `Page.title` (the markdown's own `h1`, such as "Fidryn CLI"), as on today's site; the overview is named "Overview" in the sidebar (C5), in its pager link, and in its crumb, while its `<title>` stays "Documentation — Fidryn"; the pager label carries the neighbour's own number, so §6 Outcomes shows `Previous · §5` Mill and `Next · §7` Examples; the 404 page's canonical is `https://fidryn.onlygass.dev/404` with `noindex`, since every page carries exactly one canonical link. `Page.title` and `TocEntry.text` are plain text and are escaped here. Inserted HTML (frames, the doc body) is never re-indented, because frames hold `<pre>` text.

- [ ] **Step 1: Write the failing test**

In `xtask/src/site/mod.rs`, find the line

```rust
mod seo;
```

and replace it with

```rust
mod pages;
mod seo;
```

Create `xtask/src/site/pages.rs` with the module doc comment and the tests (the implementation goes between them in Step 3):

```rust
//! Page assembly: the landing page, the docs pages, and the 404 page, each
//! a filled `templates/base.html`.

#[cfg(test)]
mod tests {
    use super::*;

    fn assets() -> Assets {
        Assets {
            css: "0123abcd".to_owned(),
            js: "89abcdef".to_owned(),
        }
    }

    fn guide(slug: &str) -> &'static Guide {
        GUIDES.iter().find(|g| g.slug == slug).expect("guide")
    }

    fn page(with_toc: bool) -> Page {
        let toc = if with_toc {
            vec![
                TocEntry {
                    level: 2,
                    number: "6.1".to_owned(),
                    id: "envelope".to_owned(),
                    text: "Envelope".to_owned(),
                },
                TocEntry {
                    level: 3,
                    number: "6.1.1".to_owned(),
                    id: "as-of".to_owned(),
                    text: "As <of>".to_owned(),
                },
            ]
        } else {
            Vec::new()
        };
        Page {
            title: "Outcomes".to_owned(),
            lead: "Lead.".to_owned(),
            body: "<p>Body text.</p>\n".to_owned(),
            toc,
            sections: Vec::new(),
        }
    }

    fn run(id: &'static str, label: &'static str, kind: &str, opinion: &[&str]) -> Run {
        Run {
            id,
            label,
            command: format!("fidryn run m.fr --query {id} --case <case>.json"),
            source_html: format!("<figure class=\"code\">source {id}</figure>"),
            case_html: format!("<figure class=\"code\">case {id}</figure>"),
            kind: kind.to_owned(),
            opinion: opinion.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    fn runs() -> Vec<Run> {
        vec![
            run(
                "trust-open",
                "Trust, open eligibility",
                "contingent",
                &[
                    "acting_trustee depends on SuccessorEligibility.",
                    "Outside scope: tax.",
                ],
            ),
            run(
                "trust-court",
                "Trust, court selects I2",
                "determinate",
                &["acting_trustee is Bob."],
            ),
            run(
                "gate-q",
                "require-gate, query q",
                "determinate",
                &["q is 7.", "Outside scope: complete_instruments."],
            ),
            run(
                "gate-r",
                "require-gate, query r",
                "suspended",
                &["Outside scope: first, not last.", "r is suspended."],
            ),
        ]
    }

    /// The text between `start` and the next `end` after it.
    fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
        let from = text
            .find(start)
            .unwrap_or_else(|| panic!("missing {start}"))
            + start.len();
        let to = text[from..]
            .find(end)
            .unwrap_or_else(|| panic!("missing {end}"))
            + from;
        &text[from..to]
    }

    fn all_pages() -> Vec<String> {
        let mut pages = vec![landing(&runs(), &assets()), not_found(&assets())];
        for g in GUIDES {
            pages.push(doc(g, &page(true), &assets()));
            pages.push(doc(g, &page(false), &assets()));
        }
        pages
    }

    #[test]
    fn no_slot_is_left_in_any_page() {
        for html in all_pages() {
            assert!(!html.contains("{{"), "unfilled slot in:\n{html}");
            assert!(html.starts_with("<!doctype html>\n") && html.ends_with("</html>\n"));
        }
    }

    #[test]
    fn guides_nav_marks_only_the_current_guide() {
        for g in GUIDES {
            let nav = guides_nav(Some(g.slug));
            assert_eq!(nav.matches(CURRENT).count(), 1, "{}", g.slug);
            let href = format!("<a href=\"{}\"{CURRENT}>", guides::url(g.slug));
            assert!(nav.contains(&href), "{} not marked:\n{nav}", g.slug);
        }
        assert!(!guides_nav(None).contains("aria-current"));
    }

    #[test]
    fn guides_nav_lists_the_groups_in_order_then_implementers() {
        let nav = guides_nav(None);
        let labels: Vec<&str> = nav
            .match_indices("<p class=\"label\">")
            .map(|(i, m)| between(&nav[i..], m, "</p>"))
            .collect();
        assert_eq!(
            labels,
            [
                "Contents",
                "Start",
                "Write",
                "Run",
                "Read results",
                "Contribute",
                "Implementers"
            ]
        );
        let start = between(&nav, "<p class=\"label\">Start</p>", "</ol>");
        assert!(start.contains(
            "<a href=\"/docs/\"><span class=\"n\"></span><span class=\"t\">Overview</span><span class=\"d\">Where to begin</span></a>"
        ));
        assert!(start.find("/docs/\"").unwrap() < start.find("/docs/getting-started").unwrap());
        assert!(nav.contains(
            "<a href=\"/docs/outcomes\"><span class=\"n\">&sect;6</span><span class=\"t\">Outcomes</span><span class=\"d\">The six kinds and the envelope</span></a>"
        ));
        assert!(nav.contains(
            "<a href=\"https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md\"><span class=\"n\" aria-hidden=\"true\">&#8599;</span><span class=\"t\">Architecture</span>"
        ));
        assert!(nav.starts_with("<nav id=\"drawer\" class=\"guides\" aria-label=\"Guides\">"));
    }

    #[test]
    fn doc_page_nav_marks_one_guide_and_the_right_site_link() {
        for g in GUIDES {
            let html = doc(g, &page(true), &assets());
            let nav = between(&html, "<nav id=\"drawer\"", "</nav>");
            assert_eq!(nav.matches(CURRENT).count(), 1, "{}", g.slug);
            let site_nav = between(&html, "<nav class=\"site-nav\"", "</nav>");
            let (docs, examples) = if g.slug == "examples" {
                ("", CURRENT)
            } else {
                (CURRENT, "")
            };
            assert!(
                site_nav.contains(&format!("<a href=\"/docs/\"{docs}>Docs</a>")),
                "{site_nav}"
            );
            assert!(
                site_nav.contains(&format!(
                    "<a href=\"/docs/examples\"{examples}>Examples</a>"
                )),
                "{site_nav}"
            );
        }
        let landing = landing(&runs(), &assets());
        assert!(!between(&landing, "<nav class=\"site-nav\"", "</nav>").contains("aria-current"));
    }

    #[test]
    fn pager_links_the_neighbors_in_reading_order() {
        assert_eq!(
            pager("index"),
            "<nav class=\"pager\" aria-label=\"Previous and next guide\"><a class=\"next\" href=\"/docs/getting-started\"><span class=\"label\">Next &middot; &sect;1</span><span class=\"t\">Getting started</span></a></nav>"
        );
        assert_eq!(
            pager("getting-started"),
            "<nav class=\"pager\" aria-label=\"Previous and next guide\"><a class=\"prev\" href=\"/docs/\"><span class=\"label\">Previous</span><span class=\"t\">Overview</span></a><a class=\"next\" href=\"/docs/language\"><span class=\"label\">Next &middot; &sect;2</span><span class=\"t\">Language</span></a></nav>"
        );
        assert_eq!(
            pager("outcomes"),
            "<nav class=\"pager\" aria-label=\"Previous and next guide\"><a class=\"prev\" href=\"/docs/mill\"><span class=\"label\">Previous &middot; &sect;5</span><span class=\"t\">Mill</span></a><a class=\"next\" href=\"/docs/examples\"><span class=\"label\">Next &middot; &sect;7</span><span class=\"t\">Examples</span></a></nav>"
        );
        assert_eq!(
            pager("contributing"),
            "<nav class=\"pager\" aria-label=\"Previous and next guide\"><a class=\"prev\" href=\"/docs/examples\"><span class=\"label\">Previous &middot; &sect;7</span><span class=\"t\">Examples</span></a></nav>"
        );
    }

    #[test]
    fn doc_page_has_kicker_title_links_and_body() {
        let html = doc(guide("outcomes"), &page(true), &assets());
        assert!(html.contains("<p class=\"kicker\">&sect; 6 &middot; Read results</p>"));
        assert!(html.contains("<h1 id=\"doc-title\">Outcomes</h1>"));
        assert!(html.contains("<title>Outcomes — Fidryn</title>"));
        assert!(html.contains(
            "<p class=\"crumb\"><a href=\"/docs/\">Docs</a> <span aria-hidden=\"true\">/</span> Outcomes</p>"
        ));
        assert!(html.contains("<body class=\"page-doc\">"));
        assert!(html.contains("<a href=\"https://github.com/BeeGass/fidryn/blob/main/docs/outcomes.md\">Edit this page on GitHub</a>"));
        assert!(html.contains("<a href=\"/docs/outcomes.md\">View as Markdown</a>"));
        assert!(html.contains("<p>Body text.</p>"));
        assert!(
            html.contains("/assets/fidryn.css?v=0123abcd")
                && html.contains("/assets/fidryn.js?v=89abcdef")
        );

        let overview = doc(guide("index"), &page(true), &assets());
        assert!(overview.contains("<p class=\"kicker\">Documentation</p>"));
        assert!(overview.contains("<title>Documentation — Fidryn</title>"));
        assert!(overview.contains("<a href=\"/docs/index.md\">View as Markdown</a>"));
    }

    #[test]
    fn on_this_page_blocks_follow_the_toc() {
        let with = doc(guide("outcomes"), &page(true), &assets());
        let items = "<li class=\"lvl-2\"><a href=\"#envelope\"><span class=\"n\">6.1</span> Envelope</a></li><li class=\"lvl-3\"><a href=\"#as-of\"><span class=\"n\">6.1.1</span> As &lt;of&gt;</a></li>";
        assert!(with.contains(&format!(
            "<details class=\"onpage-inline\"><summary>On this page</summary><ol>{items}</ol></details>"
        )));
        assert!(with.contains(&format!(
            "<aside class=\"onpage\" aria-label=\"On this page\"><p class=\"label\">On this page</p><ol>{items}</ol></aside>"
        )));

        let without = doc(guide("outcomes"), &page(false), &assets());
        assert!(!without.contains("onpage"), "{without}");

        let unnumbered = toc_items(&[TocEntry {
            level: 2,
            number: String::new(),
            id: "for-users".to_owned(),
            text: "For users".to_owned(),
        }]);
        assert_eq!(
            unnumbered,
            "<li class=\"lvl-2\"><a href=\"#for-users\">For users</a></li>"
        );
    }

    #[test]
    fn specimen_has_tabs_panels_stamps_and_dots() {
        let html = specimen(&runs());
        assert!(html.contains(
            "<button type=\"button\" role=\"tab\" id=\"tab-trust-open\" aria-controls=\"run-trust-open\" aria-selected=\"true\">Trust, open eligibility <span class=\"stamp con\">Contingent</span></button>"
        ));
        assert!(html.contains(
            "<button type=\"button\" role=\"tab\" id=\"tab-trust-court\" aria-controls=\"run-trust-court\" aria-selected=\"false\" tabindex=\"-1\">Trust, court selects I2 <span class=\"stamp det\">Determinate</span></button>"
        ));
        assert_eq!(html.matches("role=\"tab\"").count(), 4);
        assert_eq!(html.matches("aria-selected=\"true\"").count(), 1);
        assert_eq!(html.matches("tabindex=\"-1\"").count(), 3);
        assert_eq!(html.matches("<p class=\"spec-label\">").count(), 4);
        assert!(html.contains("<p class=\"spec-label\">Trust, court selects I2</p>"));
        for id in ["trust-open", "trust-court", "gate-q", "gate-r"] {
            assert!(html.contains(&format!(
                "<section class=\"spec-panel\" id=\"run-{id}\" role=\"tabpanel\" aria-labelledby=\"tab-{id}\">"
            )));
            assert!(html.contains(&format!("<figure class=\"code\">source {id}</figure>")));
            assert!(html.contains(&format!("<figure class=\"code\">case {id}</figure>")));
        }
        assert!(html.contains(
            "<p class=\"spec-cmd\"><code>fidryn run m.fr --query gate-r --case &lt;case&gt;.json</code></p>"
        ));
        assert!(html.contains(
            "<p class=\"step-h\"><span class=\"step-n\">3</span>Outcome</p><span class=\"stamp sus\">Suspended</span><ul class=\"opinion\"><li>Outside scope: first, not last.</li><li>r is suspended.</li></ul>"
        ));
        assert!(html.contains(
            "<ul class=\"opinion\"><li>acting_trustee depends on SuccessorEligibility.</li><li class=\"boundary\">Outside scope: tax.</li></ul>"
        ));
        assert!(html.contains("<ul class=\"opinion\"><li>acting_trustee is Bob.</li></ul>"));
        assert!(html.contains(
            "<div class=\"spec-dots\" aria-hidden=\"true\"><i class=\"on\"></i><i></i><i></i><i></i></div>"
        ));
        assert!(html.contains("<p class=\"spec-foot\">Computed by the Fidryn interpreter when this page was built.</p>"));
    }

    #[test]
    fn stamp_maps_every_outcome_kind() {
        assert_eq!(stamp("determinate"), ("det", "Determinate"));
        assert_eq!(stamp("contingent"), ("con", "Contingent"));
        assert_eq!(stamp("suspended"), ("sus", "Suspended"));
        assert_eq!(stamp("normConflict"), ("nc", "NormConflict"));
        assert_eq!(stamp("outsideCompetence"), ("oc", "OutsideCompetence"));
        assert_eq!(stamp("inconsistent"), ("inc", "Inconsistent"));
        assert_eq!(stamp("somethingNew"), ("", "Outcome"));
        assert_eq!(
            stamp_html("somethingNew"),
            "<span class=\"stamp\">Outcome</span>"
        );
    }

    #[test]
    fn landing_page_has_hero_specimen_and_contents() {
        let html = landing(&runs(), &assets());
        assert!(
            html.contains("<title>Fidryn — a programming language for legal instruments</title>")
        );
        assert!(html.contains("<body class=\"page-landing\">"));
        assert!(html.contains("<h1 id=\"hero-title\">No false determinacy.</h1>"));
        assert!(html.contains("<div class=\"specimen\" data-specimen>"));
        let contents = between(&html, "<ol class=\"contents\">", "</ol>");
        let hrefs: Vec<&str> = contents
            .match_indices("href=\"")
            .map(|(i, m)| between(&contents[i..], m, "\""))
            .collect();
        assert_eq!(
            hrefs,
            [
                "/docs/getting-started",
                "/docs/language",
                "/docs/cases-and-time",
                "/docs/cli",
                "/docs/mill",
                "/docs/outcomes",
                "/docs/examples",
                "/docs/contributing",
            ]
        );
        assert!(contents.contains(
            "<li><a href=\"/docs/getting-started\"><span class=\"n\">&sect;1</span><span class=\"t\">Getting started</span><span class=\"lead\" aria-hidden=\"true\"></span><span class=\"d\">Check and run a tiny module</span></a></li>"
        ));
        assert!(
            !between(&html, "<header class=\"site-head\">", "</header>")
                .contains("class=\"crumb\"")
        );
    }

    #[test]
    fn not_found_page_says_no_such_provision() {
        let html = not_found(&assets());
        assert!(html.contains("<title>Not found — Fidryn</title>"));
        assert!(html.contains(
            "<meta name=\"description\" content=\"This page is outside the declared model.\">"
        ));
        assert!(html.contains("<body class=\"page-404\">"));
        assert!(html.contains("<p class=\"nf-mark\" aria-hidden=\"true\">&sect; 404</p>"));
        assert!(html.contains("<h1 id=\"nf-title\">No such provision.</h1>"));
        assert!(html.contains(
            "<p class=\"lede\">This page is outside the declared model. Nothing was invented to fill the gap.</p>"
        ));
        assert!(html.contains("<a class=\"btn pri\" href=\"/\">Back to the start</a>"));
        assert!(html.contains("<a class=\"btn sec\" href=\"/docs/\">Open the docs</a>"));
        assert!(html.contains("<meta name=\"robots\" content=\"noindex\">"));
        assert!(!html.contains("text/markdown"));
    }

    #[test]
    fn assets_read_hashes_the_two_files() {
        let dir = std::env::temp_dir().join(format!("fidryn-assets-{}", std::process::id()));
        fs::create_dir_all(dir.join("site/assets")).unwrap();
        fs::write(dir.join("site/assets/fidryn.css"), "body{}").unwrap();
        fs::write(dir.join("site/assets/fidryn.js"), "\"use strict\";").unwrap();
        let assets = Assets::read(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(assets.css, asset_version(b"body{}"));
        assert_eq!(assets.js, asset_version(b"\"use strict\";"));
        assert!(Assets::read(&dir).is_err(), "missing files are an error");
    }
}
```

At the end of `xtask/src/site/seo.rs`, after the closing brace of Task 4's `mod tests`, append this second test module (a module after the test module does not trip clippy's `items_after_test_module`; Task 4's module keeps its name):

```rust
#[cfg(test)]
mod head_tests {
    use super::*;

    fn json_ld(head: &str) -> serde_json::Value {
        let start = head
            .find("<script type=\"application/ld+json\">")
            .expect("JSON-LD")
            + "<script type=\"application/ld+json\">".len();
        let end = head[start..].find("</script>").expect("script closes") + start;
        serde_json::from_str(&head[start..end]).expect("JSON-LD parses")
    }

    #[test]
    fn doc_head_has_canonical_alternate_social_tags_and_web_page_data() {
        let head = head(
            "Outcomes — Fidryn",
            "Determinate & the rest.",
            "/docs/outcomes",
            Some("/docs/outcomes.md"),
            PageKind::Doc,
        );
        for line in [
            "<meta name=\"robots\" content=\"index,follow,max-image-preview:large\">",
            "<meta name=\"author\" content=\"Bryan Gass\">",
            "<link rel=\"canonical\" href=\"https://fidryn.onlygass.dev/docs/outcomes\">",
            "<link rel=\"alternate\" type=\"text/markdown\" href=\"https://fidryn.onlygass.dev/docs/outcomes.md\" title=\"Markdown\">",
            "<meta property=\"og:type\" content=\"website\">",
            "<meta property=\"og:title\" content=\"Outcomes — Fidryn\">",
            "<meta property=\"og:description\" content=\"Determinate &amp; the rest.\">",
            "<meta property=\"og:url\" content=\"https://fidryn.onlygass.dev/docs/outcomes\">",
            "<meta name=\"twitter:card\" content=\"summary\">",
            "<meta name=\"twitter:title\" content=\"Outcomes — Fidryn\">",
            "<meta name=\"twitter:description\" content=\"Determinate &amp; the rest.\">",
        ] {
            assert!(head.lines().any(|l| l == line), "missing {line}\n{head}");
        }
        let data = json_ld(&head);
        assert_eq!(data["@type"], "WebPage");
        assert_eq!(data["name"], "Outcomes — Fidryn");
        assert_eq!(data["description"], "Determinate & the rest.");
        assert_eq!(data["url"], "https://fidryn.onlygass.dev/docs/outcomes");
        assert_eq!(
            data["significantLink"],
            "https://fidryn.onlygass.dev/docs/outcomes.md"
        );
        assert!(!head.ends_with('\n'));
    }

    #[test]
    fn landing_head_describes_the_software() {
        let head = head(
            LANDING_TITLE,
            LANDING_DESCRIPTION,
            "/",
            Some("/index.md"),
            PageKind::Landing,
        );
        assert!(head.contains("<link rel=\"canonical\" href=\"https://fidryn.onlygass.dev/\">"));
        assert!(head.contains("href=\"https://fidryn.onlygass.dev/index.md\""));
        let data = json_ld(&head);
        assert_eq!(data["@type"], "SoftwareApplication");
        assert_eq!(data["name"], "Fidryn");
        assert_eq!(data["description"], LANDING_DESCRIPTION);
        assert_eq!(data["downloadUrl"], "https://github.com/BeeGass/fidryn");
    }

    #[test]
    fn not_found_head_is_noindex_without_alternate() {
        let head = head(
            "Not found — Fidryn",
            "Gone.",
            "/404",
            None,
            PageKind::NotFound,
        );
        assert!(head.contains("<meta name=\"robots\" content=\"noindex\">"));
        assert!(!head.contains("index,follow"));
        assert!(!head.contains("text/markdown"));
        assert!(head.contains("<link rel=\"canonical\" href=\"https://fidryn.onlygass.dev/404\">"));
        let data = json_ld(&head);
        assert_eq!(data["@type"], "WebPage");
        assert!(data.get("significantLink").is_none());
    }

    #[test]
    fn json_ld_cannot_close_its_script_element() {
        let head = head("A </script><b>", "</p>", "/docs/x", None, PageKind::Doc);
        let script = &head[head.find("<script").unwrap()..];
        assert_eq!(
            script.matches("</").count(),
            1,
            "only the real closing tag: {script}"
        );
        assert!(script.contains("A <\\/script><b>"));
        assert_eq!(json_ld(&head)["name"], "A </script><b>");
        assert!(
            head.contains("<meta property=\"og:title\" content=\"A &lt;/script&gt;&lt;b&gt;\">")
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --offline -p xtask -- site::pages site::seo::head_tests`
Expected: FAIL to compile, with errors such as ``error[E0425]: cannot find function `landing` in this scope``, ``cannot find function `guides_nav` in this scope``, ``cannot find function `pager` in this scope``, ``cannot find function `head` in this scope``, ``cannot find type `PageKind` in this scope``, ``cannot find value `LANDING_TITLE` in this scope``, and ``cannot find struct, variant or union type `Assets` in this scope``.

- [ ] **Step 3: Write the implementation**

Create the four templates exactly as in contract C5.

`xtask/src/site/templates/base.html`:

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{{title}}</title>
<meta name="description" content="{{description}}">
{{head}}
<meta name="theme-color" content="#f7f2e8" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#15120e" media="(prefers-color-scheme: dark)">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="preload" href="/fonts/fraunces.woff2" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="/fonts/plex-sans.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/assets/fidryn.css?v={{css_v}}">
<script>document.documentElement.classList.add("js");try{var t=localStorage.getItem("fidryn-theme");if(t==="light"||t==="dark")document.documentElement.dataset.theme=t}catch(e){}</script>
<script src="/assets/fidryn.js?v={{js_v}}" defer></script>
</head>
<body class="{{body_class}}">
<a class="skip" href="#main">Skip to content</a>
<header class="site-head">
  <a class="brand" href="/"><span class="mono" aria-hidden="true">F</span><span class="wordmark">Fidryn</span></a>
  {{crumb}}
  <nav class="site-nav" aria-label="Site">
    <a href="/docs/"{{nav_docs}}>Docs</a>
    <a href="/docs/examples"{{nav_examples}}>Examples</a>
    <a href="https://github.com/BeeGass/fidryn">GitHub</a>
  </nav>
  <form class="search" action="/docs/" method="get" role="search">
    <label class="sr-only" for="site-search">Search the docs</label>
    <input id="site-search" name="q" type="search" placeholder="Search docs" autocomplete="off" spellcheck="false" role="combobox" aria-controls="search-results" aria-expanded="false" aria-autocomplete="list">
    <kbd aria-hidden="true">/</kbd>
    <ul id="search-results" class="search-results" role="listbox" aria-label="Search results" hidden></ul>
  </form>
  <button class="theme-toggle" type="button" data-theme-toggle aria-label="Switch color theme" hidden><span aria-hidden="true"></span></button>
  <button class="menu-btn" type="button" data-drawer-open aria-controls="drawer" aria-expanded="false" hidden>Menu</button>
</header>
<main id="main">
{{main}}
</main>
<div class="backdrop" data-drawer-close hidden></div>
<footer class="site-foot">
  <p>Fidryn v0.1 &middot; a research fixture, not legal advice &middot; Bryan Gass</p>
  <nav aria-label="Footer">
    <a href="/docs/">Docs</a>
    <a href="/llms.txt">llms.txt</a>
    <a href="https://github.com/BeeGass/fidryn">Source</a>
    <a href="https://onlygass.dev">onlygass.dev</a>
  </nav>
</footer>
</body>
</html>
```

`xtask/src/site/templates/landing.html`:

```html
{{guides_nav}}
<section class="hero" aria-labelledby="hero-title">
  <div class="hero-top">
    <div>
      <p class="eyebrow">A programming language for legal instruments</p>
      <h1 id="hero-title">No false determinacy.</h1>
    </div>
    <div class="hero-side">
      <p class="lede">Precise where law is mechanical. Explicit where judgment enters. Incapable of hiding authority, discretion, or ambiguity inside a Boolean.</p>
      <div class="actions"><a class="btn pri" href="/docs/getting-started">Get started</a><a class="btn sec" href="/docs/">Read the docs</a></div>
      <div class="cmd"><span class="prompt" aria-hidden="true">$</span><code>cargo install --git https://github.com/BeeGass/fidryn --locked</code><button type="button" class="copy" data-copy hidden>Copy</button></div>
    </div>
  </div>
  {{specimen}}
</section>
<aside class="note" aria-label="Research fixture"><p><strong>Research fixture.</strong> Not legal advice, not an operative instrument, and not a complete statement of any jurisdiction&rsquo;s law.</p></aside>
<section class="band" aria-labelledby="what-it-is">
  <h2 id="what-it-is"><span class="mark">&sect; 2 &mdash;</span> What it is</h2>
  <div class="three">
    <div><h3>Instruments, not slogans</h3><p>Bounded slices of trusts, tax, and statute-shaped rules as checkable programs, with <code>outside_scope</code> named so omissions stay visible.</p></div>
    <div><h3>Honest outcomes</h3><p>Determinate only when invariant across every still-admissible resolution, or when a competent authority has already decided. Otherwise suspended, contingent, or another named kind.</p></div>
    <div><h3>A local mill</h3><p><code>fidryn ui</code> binds 127.0.0.1 only. It checks, runs, explores, and renders modules. It never files.</p></div>
  </div>
</section>
<section class="band" aria-labelledby="six-outcomes">
  <h2 id="six-outcomes"><span class="mark">&sect; 3 &mdash;</span> Six honest outcomes</h2>
  <p class="band-lede">A query answers with exactly one of six kinds, and every answer carries the declared <code>modelBoundary</code>. Each kind answers one question.</p>
  <ol class="legend">
    <li><p class="q">Is there one answer, invariant across every admissible completion, or already determined by a competent authority?</p><a class="stamp det" href="/docs/outcomes#determinate">Determinate</a></li>
    <li><p class="q">Do still-admissible answers disagree?</p><a class="stamp con" href="/docs/outcomes#contingent">Contingent</a></li>
    <li><p class="q">Does evaluation need a legal operation the case does not discharge, with no covering certificate?</p><a class="stamp sus" href="/docs/outcomes#suspended">Suspended</a></li>
    <li><p class="q">Do staged effects disagree with no single applicable doctrine?</p><a class="stamp nc" href="/docs/outcomes#normconflict">NormConflict</a></li>
    <li><p class="q">Does the case ask for something outside the declared model or the module&rsquo;s competence?</p><a class="stamp oc" href="/docs/outcomes#outsidecompetence">OutsideCompetence</a></li>
    <li><p class="q">Can the admitted model not be satisfied?</p><a class="stamp inc" href="/docs/outcomes#inconsistent">Inconsistent</a></li>
  </ol>
</section>
<section class="band" aria-labelledby="contents">
  <h2 id="contents"><span class="mark">&sect; 4 &mdash;</span> Contents</h2>
  {{contents}}
</section>
```

`xtask/src/site/templates/doc.html`:

```html
<div class="doc-layout">
  {{guides_nav}}
  <article class="doc" aria-labelledby="doc-title">
    <p class="kicker">{{kicker}}</p>
    <h1 id="doc-title">{{title}}</h1>
    {{onpage_inline}}
    <div class="prose">
{{body}}
    </div>
    {{pager}}
    <p class="doc-meta"><a href="{{edit_url}}">Edit this page on GitHub</a><a href="{{md_url}}">View as Markdown</a></p>
    <p class="doc-disclaimer">The modules, records, and examples here are research fixtures. They are not legal advice, not operative instruments, and not a complete statement of any jurisdiction&rsquo;s law.</p>
  </article>
  {{onpage}}
</div>
```

`xtask/src/site/templates/404.html`:

```html
{{guides_nav}}
<section class="notfound" aria-labelledby="nf-title">
  <p class="nf-mark" aria-hidden="true">&sect; 404</p>
  <p class="eyebrow">Not found</p>
  <h1 id="nf-title">No such provision.</h1>
  <p class="lede">This page is outside the declared model. Nothing was invented to fill the gap.</p>
  <div class="actions"><a class="btn pri" href="/">Back to the start</a><a class="btn sec" href="/docs/">Open the docs</a></div>
</section>
```

Each file ends with one newline after its last line.

In `xtask/src/site/seo.rs`, find the import line Task 4 wrote

```rust
use super::guides::{GUIDES, SITE, url};
```

and replace it with

```rust
use super::guides::{GUIDES, SITE, url};
use super::html::esc;
```

Then insert the following immediately above the `#[cfg(test)]` line that opens Task 4's `mod tests`, with one blank line on each side:

```rust
/// `<title>` of the landing page.
pub const LANDING_TITLE: &str = "Fidryn — a programming language for legal instruments";

/// Meta description of the landing page, also the summary in `llms.txt`.
pub const LANDING_DESCRIPTION: &str = "Fidryn (FID-rin) is a programming language for legal instruments: precise where law is mechanical, explicit where judgment enters, and incapable of hiding authority inside a Boolean. Research fixture — not legal advice.";

/// Which structured data and robots policy a page gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageKind {
    Landing,
    Doc,
    NotFound,
}

/// SEO tags for `<head>`, one per line: robots, author, canonical, the
/// markdown alternate (when `markdown_path` is given), Open Graph, Twitter,
/// and JSON-LD. `path` and `markdown_path` are site paths such as
/// `/docs/cli` and `/docs/cli.md`; `title` and `description` are plain text.
pub fn head(
    title: &str,
    description: &str,
    path: &str,
    markdown_path: Option<&str>,
    kind: PageKind,
) -> String {
    let url = format!("{SITE}{path}");
    let markdown_url = markdown_path.map(|p| format!("{SITE}{p}"));
    let robots = match kind {
        PageKind::NotFound => "noindex",
        PageKind::Landing | PageKind::Doc => "index,follow,max-image-preview:large",
    };
    let (t, d) = (esc(title), esc(description));
    let mut lines = vec![
        format!("<meta name=\"robots\" content=\"{robots}\">"),
        "<meta name=\"author\" content=\"Bryan Gass\">".to_owned(),
        format!("<link rel=\"canonical\" href=\"{}\">", esc(&url)),
    ];
    if let Some(md) = &markdown_url {
        lines.push(format!(
            "<link rel=\"alternate\" type=\"text/markdown\" href=\"{}\" title=\"Markdown\">",
            esc(md)
        ));
    }
    lines.extend([
        "<meta property=\"og:type\" content=\"website\">".to_owned(),
        "<meta property=\"og:site_name\" content=\"Fidryn\">".to_owned(),
        format!("<meta property=\"og:title\" content=\"{t}\">"),
        format!("<meta property=\"og:description\" content=\"{d}\">"),
        format!("<meta property=\"og:url\" content=\"{}\">", esc(&url)),
        "<meta property=\"og:locale\" content=\"en_US\">".to_owned(),
        "<meta name=\"twitter:card\" content=\"summary\">".to_owned(),
        format!("<meta name=\"twitter:title\" content=\"{t}\">"),
        format!("<meta name=\"twitter:description\" content=\"{d}\">"),
    ]);
    let data = match kind {
        PageKind::Landing => serde_json::json!({
            "@context": "https://schema.org",
            "@type": "SoftwareApplication",
            "name": "Fidryn",
            "applicationCategory": "DeveloperApplication",
            "operatingSystem": "Linux, macOS, Windows",
            "programmingLanguage": "Fidryn",
            "url": SITE,
            "downloadUrl": "https://github.com/BeeGass/fidryn",
            "author": {"@type": "Person", "name": "Bryan Gass", "alternateName": "BeeGass", "url": "https://onlygass.dev"},
            "description": description,
            "license": "https://github.com/BeeGass/fidryn/blob/main/LICENSE",
        }),
        PageKind::Doc | PageKind::NotFound => {
            let mut page = serde_json::json!({
                "@context": "https://schema.org",
                "@type": "WebPage",
                "name": title,
                "description": description,
                "url": url,
                "isPartOf": {"@type": "WebSite", "name": "Fidryn", "url": SITE},
                "author": {"@type": "Person", "name": "Bryan Gass", "alternateName": "BeeGass"},
            });
            if let Some(md) = markdown_url {
                page["significantLink"] = serde_json::Value::String(md);
            }
            page
        }
    };
    // `</` inside a script element would end it early; `<\/` is the same JSON.
    let json = data.to_string().replace("</", "<\\/");
    lines.push(format!(
        "<script type=\"application/ld+json\">{json}</script>"
    ));
    lines.join("\n")
}
```

In `xtask/src/site/pages.rs`, insert the following between the two `//!` lines at the top and the `#[cfg(test)]` line, with one blank line on each side:

```rust
use super::guides::{self, GROUPS, GUIDES, Guide, IMPLEMENTER_DOCS, REPO};
use super::html::{asset_version, esc};
use super::markdown::{Page, TocEntry};
use super::seo::{self, LANDING_DESCRIPTION, LANDING_TITLE, PageKind};
use super::specimen::Run;
use super::templates::fill;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const BASE: &str = include_str!("templates/base.html");
const LANDING: &str = include_str!("templates/landing.html");
const DOC: &str = include_str!("templates/doc.html");
const NOT_FOUND: &str = include_str!("templates/404.html");

const NOT_FOUND_TITLE: &str = "Not found — Fidryn";
const NOT_FOUND_DESCRIPTION: &str = "This page is outside the declared model.";
const CURRENT: &str = " aria-current=\"page\"";

/// Content versions of the hand-written assets, used as `?v=` so the
/// stylesheet and script can be cached for a year.
pub struct Assets {
    /// `asset_version` of `site/assets/fidryn.css`.
    pub css: String,
    /// `asset_version` of `site/assets/fidryn.js`.
    pub js: String,
}

impl Assets {
    /// Hash `site/assets/fidryn.css` and `site/assets/fidryn.js` under `root`.
    pub fn read(root: &Path) -> Result<Self> {
        let version = |name: &str| -> Result<String> {
            let path = root.join("site/assets").join(name);
            let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
            Ok(asset_version(&bytes))
        };
        Ok(Self {
            css: version("fidryn.css")?,
            js: version("fidryn.js")?,
        })
    }
}

/// Stamp class and label for an outcome kind as a report spells it.
pub fn stamp(kind: &str) -> (&'static str, &'static str) {
    match kind {
        "determinate" => ("det", "Determinate"),
        "contingent" => ("con", "Contingent"),
        "suspended" => ("sus", "Suspended"),
        "normConflict" => ("nc", "NormConflict"),
        "outsideCompetence" => ("oc", "OutsideCompetence"),
        "inconsistent" => ("inc", "Inconsistent"),
        _ => ("", "Outcome"),
    }
}

fn stamp_html(kind: &str) -> String {
    match stamp(kind) {
        ("", label) => format!("<span class=\"stamp\">{label}</span>"),
        (class, label) => format!("<span class=\"stamp {class}\">{label}</span>"),
    }
}

/// Navigation name of a guide: the overview is "Overview", the rest their title.
fn nav_title(guide: &Guide) -> &'static str {
    if guide.number.is_none() {
        "Overview"
    } else {
        guide.title
    }
}

/// The guides list: the docs sidebar at 720px and up, the drawer on phones.
/// `current` is the slug of the page being rendered, if it is a guide.
pub fn guides_nav(current: Option<&str>) -> String {
    let mut out = String::from(concat!(
        "<nav id=\"drawer\" class=\"guides\" aria-label=\"Guides\">\n",
        "  <div class=\"guides-head\"><p class=\"label\">Contents</p>",
        "<button type=\"button\" class=\"btn sec sm\" data-drawer-close>Close</button></div>\n",
    ));
    for group in GROUPS {
        out.push_str(&format!(
            "  <div class=\"group\">\n    <p class=\"label\">{}</p>\n    <ol>\n",
            esc(group)
        ));
        let members = GUIDES
            .iter()
            .filter(|g| g.group == *group || (*group == GROUPS[0] && g.number.is_none()));
        for g in members {
            let number = g.number.map(|n| format!("&sect;{n}")).unwrap_or_default();
            let aria = if current == Some(g.slug) { CURRENT } else { "" };
            out.push_str(&format!(
                "      <li><a href=\"{}\"{aria}><span class=\"n\">{number}</span><span class=\"t\">{}</span><span class=\"d\">{}</span></a></li>\n",
                guides::url(g.slug),
                esc(nav_title(g)),
                esc(g.blurb),
            ));
        }
        out.push_str("    </ol>\n  </div>\n");
    }
    out.push_str("  <div class=\"group\">\n    <p class=\"label\">Implementers</p>\n    <ol>\n");
    for (title, file, blurb) in IMPLEMENTER_DOCS {
        out.push_str(&format!(
            "      <li><a href=\"{REPO}/blob/main/docs/{file}\"><span class=\"n\" aria-hidden=\"true\">&#8599;</span><span class=\"t\">{}</span><span class=\"d\">{}</span></a></li>\n",
            esc(title),
            esc(blurb),
        ));
    }
    out.push_str("    </ol>\n  </div>\n</nav>");
    out
}

/// The landing page, with the specimen built from `runs`.
pub fn landing(runs: &[Run], assets: &Assets) -> String {
    let main = fill(
        LANDING,
        &[
            ("guides_nav", &guides_nav(None)),
            ("specimen", &specimen(runs)),
            ("contents", &contents()),
        ],
    );
    shell(
        &Shell {
            title: LANDING_TITLE,
            description: LANDING_DESCRIPTION,
            path: "/",
            markdown_path: Some("/index.md"),
            kind: PageKind::Landing,
            body_class: "page-landing",
            crumb: String::new(),
            current_nav: None,
        },
        &main,
        assets,
    )
}

/// A docs page for `guide`, rendered from its markdown `page`.
pub fn doc(guide: &Guide, page: &Page, assets: &Assets) -> String {
    let toc = toc_items(&page.toc);
    let (onpage_inline, onpage) = if page.toc.is_empty() {
        (String::new(), String::new())
    } else {
        (
            format!(
                "<details class=\"onpage-inline\"><summary>On this page</summary><ol>{toc}</ol></details>"
            ),
            format!(
                "<aside class=\"onpage\" aria-label=\"On this page\"><p class=\"label\">On this page</p><ol>{toc}</ol></aside>"
            ),
        )
    };
    let main = fill(
        DOC,
        &[
            ("guides_nav", &guides_nav(Some(guide.slug))),
            ("kicker", &kicker(guide)),
            ("title", &esc(&page.title)),
            ("onpage_inline", &onpage_inline),
            ("body", &page.body),
            ("pager", &pager(guide.slug)),
            ("edit_url", &format!("{REPO}/blob/main/docs/{}", guide.file)),
            ("md_url", &format!("/docs/{}.md", guide.slug)),
            ("onpage", &onpage),
        ],
    );
    let title = format!("{} — Fidryn", guide.title);
    let path = guides::url(guide.slug);
    let markdown_path = format!("/docs/{}.md", guide.slug);
    shell(
        &Shell {
            title: &title,
            description: guide.description,
            path: &path,
            markdown_path: Some(&markdown_path),
            kind: PageKind::Doc,
            body_class: "page-doc",
            crumb: format!(
                "<p class=\"crumb\"><a href=\"/docs/\">Docs</a> <span aria-hidden=\"true\">/</span> {}</p>",
                esc(nav_title(guide))
            ),
            current_nav: Some(if guide.slug == "examples" {
                Nav::Examples
            } else {
                Nav::Docs
            }),
        },
        &main,
        assets,
    )
}

/// The 404 page.
pub fn not_found(assets: &Assets) -> String {
    let main = fill(NOT_FOUND, &[("guides_nav", &guides_nav(None))]);
    shell(
        &Shell {
            title: NOT_FOUND_TITLE,
            description: NOT_FOUND_DESCRIPTION,
            path: "/404",
            markdown_path: None,
            kind: PageKind::NotFound,
            body_class: "page-404",
            crumb: String::new(),
            current_nav: None,
        },
        &main,
        assets,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav {
    Docs,
    Examples,
}

/// Everything `base.html` needs besides the main content and the assets.
struct Shell<'a> {
    /// Plain text; escaped when filled.
    title: &'a str,
    /// Plain text; escaped when filled.
    description: &'a str,
    path: &'a str,
    markdown_path: Option<&'a str>,
    kind: PageKind,
    body_class: &'a str,
    /// Markup for the header breadcrumb, empty for none.
    crumb: String,
    current_nav: Option<Nav>,
}

fn shell(page: &Shell<'_>, main: &str, assets: &Assets) -> String {
    let head = seo::head(
        page.title,
        page.description,
        page.path,
        page.markdown_path,
        page.kind,
    );
    let mark = |nav: Nav| {
        if page.current_nav == Some(nav) {
            CURRENT
        } else {
            ""
        }
    };
    fill(
        BASE,
        &[
            ("title", &esc(page.title)),
            ("description", &esc(page.description)),
            ("head", &head),
            ("css_v", &assets.css),
            ("js_v", &assets.js),
            ("body_class", page.body_class),
            ("crumb", &page.crumb),
            ("nav_docs", mark(Nav::Docs)),
            ("nav_examples", mark(Nav::Examples)),
            ("main", main.trim_end()),
        ],
    )
}

/// `§ 6 · Read results` for a numbered guide, `Documentation` for the overview.
fn kicker(guide: &Guide) -> String {
    match guide.number {
        Some(n) => format!("&sect; {n} &middot; {}", esc(guide.group)),
        None => "Documentation".to_owned(),
    }
}

fn toc_items(toc: &[TocEntry]) -> String {
    toc.iter()
        .map(|entry| {
            let number = if entry.number.is_empty() {
                String::new()
            } else {
                format!("<span class=\"n\">{}</span> ", esc(&entry.number))
            };
            format!(
                "<li class=\"lvl-{}\"><a href=\"#{}\">{number}{}</a></li>",
                entry.level,
                esc(&entry.id),
                esc(&entry.text)
            )
        })
        .collect()
}

/// Previous and next guide in reading order: the overview, then §1 to §8.
fn pager(slug: &str) -> String {
    let Some(at) = GUIDES.iter().position(|g| g.slug == slug) else {
        return String::new();
    };
    let link = |class: &str, word: &str, g: &Guide| {
        let label = match g.number {
            Some(n) => format!("{word} &middot; &sect;{n}"),
            None => word.to_owned(),
        };
        format!(
            "<a class=\"{class}\" href=\"{}\"><span class=\"label\">{label}</span><span class=\"t\">{}</span></a>",
            guides::url(g.slug),
            esc(nav_title(g))
        )
    };
    let mut out = String::from("<nav class=\"pager\" aria-label=\"Previous and next guide\">");
    if let Some(prev) = at.checked_sub(1).map(|i| &GUIDES[i]) {
        out.push_str(&link("prev", "Previous", prev));
    }
    if let Some(next) = GUIDES.get(at + 1) {
        out.push_str(&link("next", "Next", next));
    }
    out.push_str("</nav>");
    out
}

/// The landing contents: every numbered guide in reading order.
fn contents() -> String {
    let mut out = String::from("<ol class=\"contents\">\n");
    for g in GUIDES {
        let Some(n) = g.number else { continue };
        out.push_str(&format!(
            "  <li><a href=\"{}\"><span class=\"n\">&sect;{n}</span><span class=\"t\">{}</span><span class=\"lead\" aria-hidden=\"true\"></span><span class=\"d\">{}</span></a></li>\n",
            guides::url(g.slug),
            esc(g.title),
            esc(g.blurb),
        ));
    }
    out.push_str("</ol>");
    out
}

/// The specimen: a tab per run, a panel per run with its three steps, and the
/// dot row the phone layout uses. Without JavaScript every panel shows.
fn specimen(runs: &[Run]) -> String {
    let mut out = String::from(concat!(
        "<div class=\"specimen\" data-specimen>\n",
        "  <div class=\"spec-tabs\" role=\"tablist\" aria-label=\"Real runs\">\n",
    ));
    for (i, run) in runs.iter().enumerate() {
        let (selected, tabindex) = if i == 0 {
            ("true", "")
        } else {
            ("false", " tabindex=\"-1\"")
        };
        out.push_str(&format!(
            "    <button type=\"button\" role=\"tab\" id=\"tab-{id}\" aria-controls=\"run-{id}\" aria-selected=\"{selected}\"{tabindex}>{} {}</button>\n",
            esc(run.label),
            stamp_html(&run.kind),
            id = run.id,
        ));
    }
    out.push_str("  </div>\n  <div class=\"spec-panels\">\n");
    for run in runs {
        out.push_str(&format!(
            "    <section class=\"spec-panel\" id=\"run-{id}\" role=\"tabpanel\" aria-labelledby=\"tab-{id}\">\n",
            id = run.id
        ));
        out.push_str(&format!(
            "      <p class=\"spec-label\">{}</p>\n",
            esc(run.label)
        ));
        out.push_str(&format!(
            "      <p class=\"spec-cmd\"><code>{}</code></p>\n",
            esc(&run.command)
        ));
        out.push_str("      <div class=\"steps\">\n");
        out.push_str(&format!(
            "        <div class=\"step step-source\"><p class=\"step-h\"><span class=\"step-n\">1</span>Source</p>{}</div>\n",
            run.source_html
        ));
        out.push_str(&format!(
            "        <div class=\"step step-case\"><p class=\"step-h\"><span class=\"step-n\">2</span>Case</p>{}</div>\n",
            run.case_html
        ));
        out.push_str(&format!(
            "        <div class=\"step step-outcome\"><p class=\"step-h\"><span class=\"step-n\">3</span>Outcome</p>{}{}</div>\n",
            stamp_html(&run.kind),
            opinion(&run.opinion)
        ));
        out.push_str("      </div>\n    </section>\n");
    }
    out.push_str("  </div>\n  <div class=\"spec-dots\" aria-hidden=\"true\">");
    for i in 0..runs.len() {
        out.push_str(if i == 0 {
            "<i class=\"on\"></i>"
        } else {
            "<i></i>"
        });
    }
    out.push_str(concat!(
        "</div>\n",
        "  <p class=\"spec-foot\">Computed by the Fidryn interpreter when this page was built.</p>\n",
        "</div>",
    ));
    out
}

/// Opinion sentences as a list; a closing `Outside scope: …` sentence is
/// marked as the model boundary.
fn opinion(sentences: &[String]) -> String {
    let last = sentences.len().saturating_sub(1);
    let items: String = sentences
        .iter()
        .enumerate()
        .map(|(i, s)| {
            if i == last && s.starts_with("Outside scope: ") {
                format!("<li class=\"boundary\">{}</li>", esc(s))
            } else {
                format!("<li>{}</li>", esc(s))
            }
        })
        .collect();
    format!("<ul class=\"opinion\">{items}</ul>")
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --offline -p xtask -- site::pages site::seo::head_tests`
Expected: PASS (`test result: ok. 16 passed` for the xtask unit tests). The never-used warnings for generator items remain until Task 11.

- [ ] **Step 5: Commit**

```bash
git add xtask/src/site/templates xtask/src/site/pages.rs xtask/src/site/seo.rs xtask/src/site/mod.rs
git -c commit.gpgsign=false commit -m "feat(site): render landing, docs, and 404 pages from templates"
```

### Task 11: Mirrors, llms files, search index, and the full build

**Files:**
- Create: `xtask/src/site/search.rs`
- Modify: `xtask/src/site/seo.rs` (imports; add `mirror`, `landing_markdown`, `llms_txt`, `llms_full`, and the private `front_matter` above Task 4's test module; append `mod corpus_tests` at the end)
- Modify: `xtask/src/site/mod.rs` (the module list: add `mod search;`; replace `build`; append `mod build_tests` at the end)
- Test: `xtask/src/site/search.rs` (`mod tests`), `xtask/src/site/seo.rs` (`mod corpus_tests`), `xtask/src/site/mod.rs` (`mod build_tests`)

**Interfaces:**
- Consumes: `seo::{robots, sitemap}` and `OutFile::text` (Task 4); `highlight::Keywords::load` (Task 7); `links::rewrite_markdown_links(&str) -> String` and `markdown::{render, Page, Section}` with `render(&str, Option<u32>, &Keywords) -> Page` (Task 8); `specimen::runs` (Task 9); `pages::{Assets, landing, doc, not_found}` and `seo::{LANDING_TITLE, LANDING_DESCRIPTION}` (Task 10).
- Produces: `pub fn mirror(guide: &guides::Guide, md: &str) -> String`, `pub fn landing_markdown() -> String`, `pub fn llms_txt() -> String`, `pub fn llms_full(mirrors: &[(&guides::Guide, String)]) -> String` in `seo.rs`; `pub fn index(pages: &[(&guides::Guide, &markdown::Page)]) -> String` in `search.rs` (contract C6); the final `build(root: &Path) -> anyhow::Result<Vec<OutFile>>` producing exactly the 26 files listed in C4. Task 12 writes them with `cargo xtask site`.

Formats kept from today's files. A mirror is today's front matter (`title`, `description`, `url`, `markdown`, `author`, all double-quoted), a blank line, the two-line `> Canonical HTML: …` note, a blank line, then the guide source with every link rewritten by `links::rewrite_markdown_links`, ending in one newline. `index.md` and `llms.txt` reproduce today's text line for line; the only change is that the guides follow the new reading order (Cases and time becomes third) and `index.md` spells the label "Cases and time" instead of "Cases & time". `llms-full.txt` keeps its header and guide index (now with a Documentation row, in reading order), then carries every mirror in full under a `========== {mirror URL} ==========` banner, the separator the retired Node script documented; the old hand-written summaries go, because the full text replaces them. `search-index.json` follows C6: page entries use `h` = the page's `h1` (`Page.title`) and `p` = `Guide.title`, and `t` is at most 200 characters.

Every guide source opens with its `# ` title (Task 8 removed the stray mirror header from `docs/examples.md` and tests the rule), so each mirror carries exactly one front-matter block, the one written here; `every_mirror_is_in_llms_full_and_every_page_has_a_mirror_link` checks that on the real guides. After this task no generator item is left unread: `pages`, `seo`, `search`, `specimen`, and `build` read every field of `Page`, `TocEntry`, `Section`, and `Guide`, and call `Keywords::load`, `code_frame`, `numbered_frame`, `rewrite_markdown_links`, `asset_version`, `fill`, `GROUPS`, and `IMPLEMENTER_DOCS`, so `cargo clippy -D warnings` is clean.

- [ ] **Step 1: Write the failing test**

In `xtask/src/site/mod.rs`, find the line

```rust
mod seo;
```

and replace it with

```rust
mod search;
mod seo;
```

Create `xtask/src/site/search.rs` with the module doc comment and the tests (the implementation goes between them in Step 3):

```rust
//! `search-index.json`: one entry per guide page, then one per `h2`/`h3`
//! section, in guide order. The site script fetches it on first focus of the
//! search field.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::highlight::Keywords;
    use crate::site::markdown::{self, Section};
    use crate::workspace::workspace_root;
    use std::fs;

    fn guide(slug: &str) -> &'static Guide {
        guides::GUIDES
            .iter()
            .find(|g| g.slug == slug)
            .expect("guide")
    }

    fn entries(json: &str) -> Vec<serde_json::Map<String, Value>> {
        let value: Value = serde_json::from_str(json).expect("index parses");
        value
            .as_array()
            .expect("array")
            .iter()
            .map(|e| e.as_object().expect("object").clone())
            .collect()
    }

    #[test]
    fn page_then_sections_in_order_with_numbers() {
        let page = Page {
            title: "Outcomes".to_owned(),
            lead: "This guide explains\n  the six honest results.".to_owned(),
            body: String::new(),
            toc: Vec::new(),
            sections: vec![
                Section {
                    number: "6.3".to_owned(),
                    id: "outcome-kinds".to_owned(),
                    heading: "Outcome kinds".to_owned(),
                    text: "outcome is tagged by kind.".to_owned(),
                },
                Section {
                    number: "6.3.1".to_owned(),
                    id: "determinate".to_owned(),
                    heading: "Determinate".to_owned(),
                    text: "One answer.".to_owned(),
                },
            ],
        };
        let overview = Page {
            title: "Fidryn documentation".to_owned(),
            lead: "Start here.".to_owned(),
            body: String::new(),
            toc: Vec::new(),
            sections: vec![Section {
                number: String::new(),
                id: "for-users".to_owned(),
                heading: "For users".to_owned(),
                text: "Guides.".to_owned(),
            }],
        };
        let json = index(&[(guide("index"), &overview), (guide("outcomes"), &page)]);
        assert!(
            json.ends_with("]\n") && !json.contains("\n "),
            "compact: {json}"
        );
        assert_eq!(
            json,
            concat!(
                r#"[{"u":"/docs/","n":"","h":"Fidryn documentation","p":"Documentation","t":"Start here."},"#,
                r##"{"u":"/docs/#for-users","n":"","h":"For users","p":"Documentation","t":"Guides."},"##,
                r#"{"u":"/docs/outcomes","n":"§6","h":"Outcomes","p":"Outcomes","t":"This guide explains the six honest results."},"#,
                r##"{"u":"/docs/outcomes#outcome-kinds","n":"6.3","h":"Outcome kinds","p":"Outcomes","t":"outcome is tagged by kind."},"##,
                r##"{"u":"/docs/outcomes#determinate","n":"6.3.1","h":"Determinate","p":"Outcomes","t":"One answer."}]"##,
                "\n"
            )
        );
    }

    #[test]
    fn snippets_are_collapsed_and_cut_at_200_characters() {
        let long = "§ word ".repeat(60);
        let cut = snippet(&long);
        assert!(cut.chars().count() <= 200, "{}", cut.chars().count());
        assert!(!cut.ends_with(' ') && !cut.contains("  "));
        assert!(long.starts_with(&cut));
        assert_eq!(snippet("  a \n\t b  "), "a b");
        assert_eq!(snippet(&"é".repeat(250)), "é".repeat(200));
    }

    #[test]
    fn real_guides_index_every_section_with_short_snippets() {
        let root = workspace_root();
        let kw = Keywords::load(&root).unwrap();
        let rendered: Vec<(&Guide, Page)> = guides::GUIDES
            .iter()
            .map(|g| {
                let md = fs::read_to_string(root.join("docs").join(g.file)).unwrap();
                (g, markdown::render(&md, g.number, &kw))
            })
            .collect();
        let pages: Vec<(&Guide, &Page)> = rendered.iter().map(|(g, p)| (*g, p)).collect();
        let entries = entries(&index(&pages));
        let sections: usize = rendered.iter().map(|(_, p)| p.sections.len()).sum();
        assert_eq!(entries.len(), guides::GUIDES.len() + sections);
        let kinds = entries
            .iter()
            .find(|e| e["u"] == "/docs/outcomes#outcome-kinds")
            .expect("outcome kinds section");
        assert_eq!(kinds["n"], "6.3");
        assert_eq!(kinds["h"], "Outcome kinds");
        assert_eq!(kinds["p"], "Outcomes");
        let page = entries
            .iter()
            .find(|e| e["u"] == "/docs/outcomes")
            .expect("outcomes page");
        assert_eq!(page["n"], "§6");
        for entry in &entries {
            let t = entry["t"].as_str().unwrap();
            assert!(t.chars().count() <= 200, "{t}");
            assert!(!t.ends_with(' ') && !t.contains('\n'), "{t:?}");
            assert_eq!(entry.len(), 5, "{entry:?}");
        }
    }
}
```

At the end of `xtask/src/site/seo.rs`, after `mod head_tests` from Task 10, append:

```rust
#[cfg(test)]
mod corpus_tests {
    use super::*;

    fn guide(slug: &str) -> &'static Guide {
        GUIDES.iter().find(|g| g.slug == slug).expect("guide")
    }

    /// Every `](target)` link target in markdown text.
    fn link_targets(md: &str) -> Vec<&str> {
        md.match_indices("](")
            .filter_map(|(i, _)| {
                let rest = &md[i + 2..];
                rest.find(')').map(|end| &rest[..end])
            })
            .collect()
    }

    #[test]
    fn mirror_has_front_matter_note_and_absolute_links() {
        let md = "# Outcomes\n\nSee [CLI](cli.md), [the kinds](#outcome-kinds), [the overview](README.md), and [the schema](../schemas/outcome-v0.1.json).\n";
        let text = mirror(guide("outcomes"), md);
        assert!(text.starts_with(concat!(
            "---\n",
            "title: \"Outcomes\"\n",
            "description: \"Determinate, Suspended, Contingent, and the rest of the Fidryn outcome envelope.\"\n",
            "url: \"https://fidryn.onlygass.dev/docs/outcomes\"\n",
            "markdown: \"https://fidryn.onlygass.dev/docs/outcomes.md\"\n",
            "author: \"Bryan Gass\"\n",
            "---\n",
            "\n",
            "> Canonical HTML: https://fidryn.onlygass.dev/docs/outcomes\n",
            "> This markdown mirror is for agents and plain-text readers.\n",
            "\n",
            "# Outcomes\n",
        )));
        assert!(text.ends_with(".json).\n") && !text.ends_with("\n\n"));
        let targets = link_targets(&text);
        assert_eq!(targets.len(), 4, "{targets:?}");
        for target in targets {
            assert!(
                target.starts_with("https://") || target.starts_with('#'),
                "relative link left in the mirror: {target}"
            );
        }
        assert!(
            text.contains(
                "](https://github.com/BeeGass/fidryn/blob/main/schemas/outcome-v0.1.json)"
            )
        );

        let overview = mirror(guide("index"), "# Fidryn documentation\n");
        assert!(overview.contains("url: \"https://fidryn.onlygass.dev/docs/\"\n"));
        assert!(overview.contains("markdown: \"https://fidryn.onlygass.dev/docs/index.md\"\n"));
    }

    #[test]
    fn landing_markdown_keeps_its_sections_and_lists_every_guide() {
        let md = landing_markdown();
        assert!(md.starts_with(
            "---\ntitle: \"Fidryn — a programming language for legal instruments\"\n"
        ));
        assert!(md.contains("url: \"https://fidryn.onlygass.dev/\"\nmarkdown: \"https://fidryn.onlygass.dev/index.md\"\n"));
        for heading in [
            "# Fidryn (FID-rin)",
            "## What it is",
            "## Install",
            "## Documentation",
            "## Agent maps",
            "## Source",
        ] {
            assert!(md.contains(&format!("\n{heading}\n")), "missing {heading}");
        }
        assert!(md.contains(
            "- [Docs hub](https://fidryn.onlygass.dev/docs/) · [docs/index.md](https://fidryn.onlygass.dev/docs/index.md)\n"
        ));
        assert!(md.contains(
            "- [Cases and time](https://fidryn.onlygass.dev/docs/cases-and-time) · [cases-and-time.md](https://fidryn.onlygass.dev/docs/cases-and-time.md)\n"
        ));
        let listed = md.matches("](https://fidryn.onlygass.dev/docs/").count();
        assert_eq!(listed, 2 * GUIDES.len());
        assert!(md.ends_with("## Source\n\nhttps://github.com/BeeGass/fidryn\n"));
    }

    #[test]
    fn llms_txt_lists_every_guide_with_html_and_markdown_urls() {
        let txt = llms_txt();
        assert!(txt.starts_with(&format!("# Fidryn\n\n> {LANDING_DESCRIPTION}\n\n")));
        assert!(txt.contains(&format!(
            "- [Docs hub]({SITE}/docs/): Learner documentation index\n  - Markdown: {SITE}/docs/index.md\n"
        )));
        let mut last = 0;
        for g in GUIDES.iter().filter(|g| g.number.is_some()) {
            let entry = format!(
                "- [{}]({SITE}/docs/{}.md): {}\n  - HTML: {SITE}/docs/{}\n",
                g.title, g.slug, g.description, g.slug
            );
            let at = txt
                .find(&entry)
                .unwrap_or_else(|| panic!("missing {}:\n{txt}", g.slug));
            assert!(at > last, "{} out of order", g.slug);
            last = at;
        }
        assert!(txt.ends_with("is not exposed on this site.\n"));
    }

    #[test]
    fn llms_full_contains_the_index_and_every_mirror() {
        let mirrors: Vec<(&Guide, String)> = GUIDES
            .iter()
            .map(|g| {
                (
                    g,
                    mirror(g, &format!("# {}\n\nBody of {}.\n", g.title, g.slug)),
                )
            })
            .collect();
        let full = llms_full(&mirrors);
        assert!(full.starts_with("# Fidryn — full public documentation corpus\n\n"));
        assert!(full.contains(&format!("| Home | {SITE}/ | {SITE}/index.md |\n")));
        for (g, text) in &mirrors {
            assert!(full.contains(&format!(
                "| {} | {SITE}{} | {SITE}/docs/{}.md |\n",
                g.title,
                url(g.slug),
                g.slug
            )));
            assert!(
                full.contains(&format!(
                    "\n========== {SITE}/docs/{}.md ==========\n\n{text}",
                    g.slug
                )),
                "{} mirror missing",
                g.slug
            );
        }
        assert!(full.ends_with("Body of contributing.\n"));
    }
}
```

At the end of `xtask/src/site/mod.rs`, after the closing brace of Task 4's `mod tests`, append:

```rust
#[cfg(test)]
mod build_tests {
    use super::*;

    #[test]
    fn build_writes_exactly_the_site_files() {
        let files = build(&workspace_root()).expect("build");
        let mut got: Vec<String> = files.iter().map(|f| f.path.display().to_string()).collect();
        got.sort();
        let mut want: Vec<String> = [
            "index.html",
            "index.md",
            "404.html",
            "search-index.json",
            "sitemap.xml",
            "robots.txt",
            "llms.txt",
            "llms-full.txt",
        ]
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
        for guide in guides::GUIDES {
            want.push(format!("docs/{}.html", guide.slug));
            want.push(format!("docs/{}.md", guide.slug));
        }
        want.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn two_builds_are_byte_identical() {
        let root = workspace_root();
        assert!(build(&root).expect("first build") == build(&root).expect("second build"));
    }

    #[test]
    fn every_mirror_is_in_llms_full_and_every_page_has_a_mirror_link() {
        let files = build(&workspace_root()).expect("build");
        let text = |path: &str| {
            let file = files
                .iter()
                .find(|f| f.path == Path::new(path))
                .unwrap_or_else(|| panic!("{path}"));
            String::from_utf8(file.bytes.clone()).expect("utf-8")
        };
        let full = text("llms-full.txt");
        for guide in guides::GUIDES {
            let mirror = text(&format!("docs/{}.md", guide.slug));
            assert!(mirror.starts_with("---\ntitle: "), "{}", guide.slug);
            assert_eq!(
                mirror.matches("\n> Canonical HTML: ").count(),
                1,
                "{}",
                guide.slug
            );
            assert!(
                full.contains(&mirror),
                "{} mirror missing from llms-full.txt",
                guide.slug
            );
            let html = text(&format!("docs/{}.html", guide.slug));
            assert!(html.contains(&format!(
                "<a href=\"/docs/{}.md\">View as Markdown</a>",
                guide.slug
            )));
        }
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --offline -p xtask -- site::search site::seo::corpus_tests site::build_tests`
Expected: FAIL to compile, with ``error[E0425]: cannot find function `index` in this scope`` and the same error for `snippet`, `mirror`, `landing_markdown`, `llms_txt`, and `llms_full` (plus ``cannot find type `Guide` in this scope``, since `seo.rs` does not import `Guide` until Step 3).

- [ ] **Step 3: Write the implementation**

In `xtask/src/site/search.rs`, insert the following between the three `//!` lines at the top and the `#[cfg(test)]` line, with one blank line on each side:

```rust
use super::guides::{self, Guide};
use super::markdown::Page;
use serde_json::{Value, json};

/// Longest snippet, in characters.
const SNIPPET_CHARS: usize = 200;

/// The search index as compact JSON with a final newline.
pub fn index(pages: &[(&Guide, &Page)]) -> String {
    let mut entries = Vec::new();
    for (guide, page) in pages {
        let url = guides::url(guide.slug);
        entries.push(json!({
            "u": url,
            "n": guide.number.map(|n| format!("§{n}")).unwrap_or_default(),
            "h": page.title,
            "p": guide.title,
            "t": snippet(&page.lead),
        }));
        for section in &page.sections {
            entries.push(json!({
                "u": format!("{url}#{}", section.id),
                "n": section.number,
                "h": section.heading,
                "p": guide.title,
                "t": snippet(&section.text),
            }));
        }
    }
    let mut out = Value::Array(entries).to_string();
    out.push('\n');
    out
}

/// The first 200 characters of `text` with whitespace collapsed, cut at a
/// character boundary, with no trailing space.
fn snippet(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = collapsed.chars().take(SNIPPET_CHARS).collect();
    cut.trim_end().to_owned()
}
```

In `xtask/src/site/seo.rs`, find the two import lines

```rust
use super::guides::{GUIDES, SITE, url};
use super::html::esc;
```

and replace them with

```rust
use super::guides::{GUIDES, Guide, SITE, url};
use super::html::esc;
use super::links::rewrite_markdown_links;
```

Then insert the following immediately above the `#[cfg(test)]` line that opens Task 4's `mod tests` (the first `#[cfg(test)]` in the file, just below Task 10's `head`), with one blank line on each side:

```rust
/// The markdown mirror of a guide (`site/docs/{slug}.md`): front matter, a
/// note naming the canonical HTML page, then the source with every link
/// made absolute.
pub fn mirror(guide: &Guide, md: &str) -> String {
    let html = format!("{SITE}{}", url(guide.slug));
    format!(
        "{}> Canonical HTML: {html}\n> This markdown mirror is for agents and plain-text readers.\n\n{}\n",
        front_matter(
            guide.title,
            guide.description,
            &html,
            &format!("{SITE}/docs/{}.md", guide.slug)
        ),
        rewrite_markdown_links(md).trim_end()
    )
}

/// `site/index.md`: the landing page as markdown.
pub fn landing_markdown() -> String {
    let mut docs = String::new();
    for g in GUIDES {
        let (label, file) = if g.number.is_some() {
            (g.title, format!("{}.md", g.slug))
        } else {
            ("Docs hub", "docs/index.md".to_owned())
        };
        docs.push_str(&format!(
            "- [{label}]({SITE}{}) · [{file}]({SITE}/docs/{}.md)\n",
            url(g.slug),
            g.slug
        ));
    }
    format!(
        concat!(
            "{front}",
            "# Fidryn (FID-rin)\n\n",
            "Fidryn is a **programming language for legal instruments**: precise where law is mechanical, explicit where judgment enters, and incapable of hiding authority, discretion, or ambiguity inside a Boolean.\n\n",
            "**Research fixture.** Not legal advice, not an operative instrument, and not a complete statement of any jurisdiction's law.\n\n",
            "## What it is\n\n",
            "- Source files use the `.fr` extension.\n",
            "- You write modules, queries, and duties; the reference interpreter checks them and evaluates queries against case records.\n",
            "- It never invents a completion when the model still has open branches.\n",
            "- Determinate results only when invariant across every still-admissible resolution — or a competent authority has already determined them.\n\n",
            "## Install\n\n",
            "```bash\ncargo install --git https://github.com/BeeGass/fidryn --locked\n```\n\n",
            "Requires Rust 1.98+. Local mill: `fidryn ui --no-open` (loopback only, default `127.0.0.1:8751`). This public site does **not** expose live filing or the mill API.\n\n",
            "## Documentation\n\n",
            "{docs}\n",
            "## Agent maps\n\n",
            "- [llms.txt]({site}/llms.txt)\n",
            "- [llms-full.txt]({site}/llms-full.txt)\n",
            "- [sitemap.xml]({site}/sitemap.xml)\n\n",
            "## Source\n\n",
            "https://github.com/BeeGass/fidryn\n",
        ),
        front = front_matter(
            LANDING_TITLE,
            LANDING_DESCRIPTION,
            &format!("{SITE}/"),
            &format!("{SITE}/index.md")
        ),
        docs = docs,
        site = SITE,
    )
}

/// `site/llms.txt`: the curated map of public pages for agents.
pub fn llms_txt() -> String {
    let mut guides = String::new();
    for g in GUIDES.iter().filter(|g| g.number.is_some()) {
        guides.push_str(&format!(
            "- [{}]({SITE}/docs/{}.md): {}\n  - HTML: {SITE}{}\n",
            g.title,
            g.slug,
            g.description,
            url(g.slug)
        ));
    }
    format!(
        concat!(
            "# Fidryn\n\n",
            "> {description}\n\n",
            "This file follows the llms.txt convention: a curated map of public pages, with clean markdown mirrors for agents.\n",
            "Human-facing HTML is unchanged; prefer `text/markdown` URLs below when you need the full text.\n\n",
            "Site: {site}\n",
            "Full corpus: {site}/llms-full.txt\n",
            "Source: https://github.com/BeeGass/fidryn\n\n",
            "## Primary pages\n\n",
            "- [Home]({site}/): Overview — programming language for legal instruments\n",
            "  - Markdown: {site}/index.md\n",
            "- [Docs hub]({site}/docs/): Learner documentation index\n",
            "  - Markdown: {site}/docs/index.md\n\n",
            "## Learner guides\n\n",
            "{guides}\n",
            "## Notes for agents\n\n",
            "- Research fixture — not legal advice.\n",
            "- Per-page markdown mirrors use the `.md` suffix and `Content-Type: text/markdown`.\n",
            "- HTML pages advertise the mirror via `rel=alternate` / `type=text/markdown`.\n",
            "- The local mill (`fidryn ui`) binds loopback only and is not exposed on this site.\n",
        ),
        description = LANDING_DESCRIPTION,
        site = SITE,
        guides = guides,
    )
}

/// `site/llms-full.txt`: a header, the guide index, then every mirror in
/// full, each under a banner naming its URL.
pub fn llms_full(mirrors: &[(&Guide, String)]) -> String {
    let mut out = format!(
        concat!(
            "# Fidryn — full public documentation corpus\n\n",
            "Source: {site}\n",
            "Prefer per-page .md URLs from {site}/llms.txt when possible.\n\n",
            "Research fixture — not legal advice.\n\n",
            "---\n\n",
            "## Guide index\n\n",
            "| Guide | HTML | Markdown |\n",
            "| --- | --- | --- |\n",
            "| Home | {site}/ | {site}/index.md |\n",
        ),
        site = SITE
    );
    for (g, _) in mirrors {
        out.push_str(&format!(
            "| {} | {SITE}{} | {SITE}/docs/{}.md |\n",
            g.title,
            url(g.slug),
            g.slug
        ));
    }
    for (g, mirror) in mirrors {
        out.push_str(&format!(
            "\n========== {SITE}/docs/{}.md ==========\n\n{mirror}",
            g.slug
        ));
    }
    out
}

/// YAML front matter in the fixed key order the mirrors have always used.
fn front_matter(title: &str, description: &str, url: &str, markdown: &str) -> String {
    let quote = |s: &str| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""));
    format!(
        "---\ntitle: {}\ndescription: {}\nurl: {}\nmarkdown: {}\nauthor: \"Bryan Gass\"\n---\n\n",
        quote(title),
        quote(description),
        quote(url),
        quote(markdown)
    )
}
```

In `xtask/src/site/mod.rs`, find the `build` function Task 4 wrote:

```rust
/// Render every generated file. Reads inputs under `root`; writes nothing.
pub fn build(_root: &Path) -> Result<Vec<OutFile>> {
    Ok(vec![
        OutFile::text("robots.txt", seo::robots()),
        OutFile::text("sitemap.xml", seo::sitemap()),
    ])
}
```

and replace it with:

```rust
/// Render every generated file. Reads inputs under `root`; writes nothing.
pub fn build(root: &Path) -> Result<Vec<OutFile>> {
    let kw = highlight::Keywords::load(root)?;
    let assets = pages::Assets::read(root)?;
    let runs = specimen::runs(root, &kw)?;
    let mut sources = Vec::with_capacity(guides::GUIDES.len());
    for guide in guides::GUIDES {
        let path = root.join("docs").join(guide.file);
        let md = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let page = markdown::render(&md, guide.number, &kw);
        sources.push((guide, md, page));
    }

    let mut files = vec![
        OutFile::text("index.html", pages::landing(&runs, &assets)),
        OutFile::text("index.md", seo::landing_markdown()),
        OutFile::text("404.html", pages::not_found(&assets)),
    ];
    let mut mirrors = Vec::with_capacity(sources.len());
    for (guide, md, page) in &sources {
        let mirror = seo::mirror(guide, md);
        files.push(OutFile::text(
            format!("docs/{}.html", guide.slug),
            pages::doc(guide, page, &assets),
        ));
        files.push(OutFile::text(
            format!("docs/{}.md", guide.slug),
            mirror.clone(),
        ));
        mirrors.push((*guide, mirror));
    }
    let indexed: Vec<_> = sources
        .iter()
        .map(|(guide, _, page)| (*guide, page))
        .collect();
    files.extend([
        OutFile::text("search-index.json", search::index(&indexed)),
        OutFile::text("sitemap.xml", seo::sitemap()),
        OutFile::text("robots.txt", seo::robots()),
        OutFile::text("llms.txt", seo::llms_txt()),
        OutFile::text("llms-full.txt", seo::llms_full(&mirrors)),
    ]);
    Ok(files)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --offline -p xtask -- site::search site::seo::corpus_tests site::build_tests`
Expected: PASS (`test result: ok. 10 passed` for the xtask unit tests).

Run: `cargo test --offline -p xtask site::`
Expected: PASS, every generator test green, and no never-used warnings from the generator any more.

- [ ] **Step 5: Commit**

```bash
git add xtask/src/site/mod.rs xtask/src/site/search.rs xtask/src/site/seo.rs
git -c commit.gpgsign=false commit -m "feat(site): markdown mirrors, llms files, search index, and the full build"
```

### Task 12: Generate the site and retire the Node build

**Files:**
- Create: `xtask/tests/site_output.rs`
- Delete: `site/package.json`, `site/package-lock.json`, `site/scripts/build-docs.mjs` (the whole `site/scripts/` directory), `site/styles.css`, `site/docs.css`
- Modify: `site/.gitignore` (drop `node_modules/`)
- Modify: `site/vercel.json` (`headers`: add `/assets/(.*)` after `/fonts/(.*)`, and `/search-index.json` last)
- Modify: `xtask/src/ci.rs` (`run`: the site check after the schema probes)
- Generated by `cargo xtask site`: `site/index.html`, `site/index.md`, `site/404.html`, `site/docs/{index,getting-started,language,cases-and-time,cli,mill,outcomes,examples,contributing}.{html,md}`, `site/search-index.json`, `site/sitemap.xml`, `site/robots.txt`, `site/llms.txt`, `site/llms-full.txt`
- Test: `xtask/tests/site_output.rs`

**Interfaces:**
- Consumes: `cargo xtask site` and `cargo xtask site --check` (Task 4's `run` and `check`, Task 11's `build`); `crate::site::check(root: &Path) -> anyhow::Result<()>` and `crate::workspace::workspace_root()`.
- Produces: the committed generated site; `xtask/tests/site_output.rs`, which Tasks 17 and 18 run after regenerating; `cargo xtask ci` running the site check.

The integration test reads the committed `site/`. Every internal link in every HTML page (`href` and `src`, including `https://fidryn.onlygass.dev/…` canonical and alternate links, `/assets/…?v=…`, `/fonts/…`, `/favicon.svg`, `.md` mirrors, `/llms.txt`) must resolve to a file the way Vercel's `cleanUrls` serves it, and a `#fragment` must name an `id` in the target page. The same holds for every site URL in `index.md`, `llms.txt`, `llms-full.txt`, `sitemap.xml`, and `robots.txt`, and for every `u` in `search-index.json` (the search dropdown's links). Every page needs a title, a description, and exactly one canonical link; ids must be unique per page; no `{{` may remain outside `<code>` (the Mill guide legitimately shows `{{module}}@{{version}}` in code); and `xtask site --check` must pass.

- [ ] **Step 1: Write the failing test**

Create `xtask/tests/site_output.rs`:

```rust
//! Integrity of the committed site under `site/`: every internal link and
//! anchor resolves, every page carries its metadata, and a fresh render
//! matches what is committed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const ORIGIN: &str = "https://fidryn.onlygass.dev";

/// Generated text files whose absolute site URLs must resolve.
const TEXT_FILES: &[&str] = &[
    "index.md",
    "llms.txt",
    "llms-full.txt",
    "sitemap.xml",
    "robots.txt",
];

fn site() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("site")
}

/// Paths of every file under `dir`, relative to it, with `/` separators.
fn files_under(dir: &Path, rel: &str, out: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        if entry.file_type().unwrap().is_dir() {
            files_under(&entry.path(), &path, out);
        } else {
            out.push(path);
        }
    }
}

/// The site's HTML pages, keyed by path relative to `site/`.
fn pages() -> BTreeMap<String, String> {
    let mut all = Vec::new();
    files_under(&site(), "", &mut all);
    all.into_iter()
        .filter(|p| p.ends_with(".html"))
        .map(|p| {
            let html = fs::read_to_string(site().join(&p)).unwrap();
            (p, html)
        })
        .collect()
}

/// Values of every `name="…"` attribute in `html`, entity `&amp;` decoded.
fn attrs(html: &str, name: &str) -> Vec<String> {
    let needle = format!(" {name}=\"");
    html.match_indices(&needle)
        .map(|(i, _)| {
            let rest = &html[i + needle.len()..];
            rest[..rest.find('"').expect("attribute closes")].replace("&amp;", "&")
        })
        .collect()
}

/// The file under `site/` that serves a site path, with Vercel's `cleanUrls`.
fn resolve(path: &str) -> Option<String> {
    let rel = path.trim_start_matches('/');
    let candidates = if rel.is_empty() || rel.ends_with('/') {
        vec![format!("{rel}index.html")]
    } else {
        vec![rel.to_owned(), format!("{rel}.html")]
    };
    candidates.into_iter().find(|c| site().join(c).is_file())
}

/// Check one link found in `from`. Returns a problem, if any.
fn check_link(from: &str, href: &str, ids: &BTreeMap<String, BTreeSet<String>>) -> Option<String> {
    let local = if let Some(rest) = href.strip_prefix(ORIGIN) {
        if rest.is_empty() {
            "/".to_owned()
        } else {
            rest.to_owned()
        }
    } else if (href.starts_with('/') && !href.starts_with("//")) || href.starts_with('#') {
        href.to_owned()
    } else if href.contains("://") || href.starts_with("mailto:") {
        return None;
    } else {
        return Some(format!("{from}: relative link {href}"));
    };
    let (path, fragment) = match local.split_once('#') {
        Some((p, f)) => (p, Some(f)),
        None => (local.as_str(), None),
    };
    let path = path.split('?').next().unwrap_or_default();
    let target = if path.is_empty() {
        from.to_owned()
    } else {
        match resolve(path) {
            Some(t) => t,
            None => return Some(format!("{from}: {href} does not resolve to a file")),
        }
    };
    match (fragment, ids.get(&target)) {
        (Some(""), _) => Some(format!("{from}: empty fragment in {href}")),
        (Some(id), Some(set)) if !set.contains(id) => {
            Some(format!("{from}: {href} has no id=\"{id}\" in {target}"))
        }
        // A fragment into a markdown or text file cannot be checked.
        _ => None,
    }
}

#[test]
fn every_internal_link_and_anchor_resolves() {
    let pages = pages();
    assert!(
        pages.contains_key("index.html") && pages.contains_key("404.html"),
        "{:?}",
        pages.keys()
    );
    let ids: BTreeMap<String, BTreeSet<String>> = pages
        .iter()
        .map(|(p, html)| (p.clone(), attrs(html, "id").into_iter().collect()))
        .collect();
    let mut problems = Vec::new();
    let mut checked = 0;
    for (page, html) in &pages {
        for href in attrs(html, "href").into_iter().chain(attrs(html, "src")) {
            checked += 1;
            problems.extend(check_link(page, &href, &ids));
        }
    }
    for name in TEXT_FILES {
        let text = fs::read_to_string(site().join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        for (i, _) in text.match_indices(ORIGIN) {
            let url: String = text[i..]
                .chars()
                .take_while(|c| !c.is_whitespace() && !matches!(c, ')' | '<' | '"' | '|'))
                .collect();
            checked += 1;
            problems.extend(check_link(name, &url, &ids));
        }
    }
    let index = fs::read_to_string(site().join("search-index.json")).expect("search-index.json");
    let entries: serde_json::Value = serde_json::from_str(&index).expect("search index parses");
    for entry in entries.as_array().expect("array") {
        checked += 1;
        problems.extend(check_link(
            "search-index.json",
            entry["u"].as_str().expect("u"),
            &ids,
        ));
    }
    assert!(
        checked > 500,
        "only {checked} links found; is the site generated?"
    );
    assert!(
        problems.is_empty(),
        "{} broken links:\n{}",
        problems.len(),
        problems.join("\n")
    );
}

#[test]
fn every_page_has_title_description_and_canonical() {
    for (page, html) in pages() {
        let title = html
            .split_once("<title>")
            .and_then(|(_, rest)| rest.split_once("</title>"))
            .map(|(t, _)| t.trim().to_owned());
        assert!(title.is_some_and(|t| !t.is_empty()), "{page}: no title");
        let description = html
            .split_once("<meta name=\"description\" content=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(d, _)| d.to_owned());
        assert!(
            description.is_some_and(|d| !d.is_empty()),
            "{page}: no description"
        );
        assert_eq!(
            html.matches("<link rel=\"canonical\" href=\"https://fidryn.onlygass.dev/")
                .count(),
            1,
            "{page}: needs exactly one canonical link"
        );
    }
}

#[test]
fn ids_are_unique_on_every_page() {
    // A heading slug that collides with a template id (`main`, `drawer`)
    // would break its anchor and the control that points at it.
    for (page, html) in pages() {
        let mut seen = BTreeSet::new();
        for id in attrs(&html, "id") {
            assert!(seen.insert(id.clone()), "{page}: duplicate id=\"{id}\"");
        }
    }
}

#[test]
fn no_template_slot_is_left_outside_code() {
    for (page, html) in pages() {
        // Guides may show `{{module}}` in code; only markup outside code counts.
        let mut rest = html.as_str();
        let mut outside = String::new();
        while let Some(start) = rest.find("<code") {
            outside.push_str(&rest[..start]);
            let end = rest[start..]
                .find("</code>")
                .map(|e| start + e + "</code>".len());
            rest = end.map_or("", |e| &rest[e..]);
        }
        outside.push_str(rest);
        assert!(!outside.contains("{{"), "{page}: unfilled template slot");
    }
}

#[test]
fn generator_check_passes_on_the_committed_site() {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["site", "--check"])
        .output()
        .expect("spawn xtask site --check");
    assert!(
        out.status.success(),
        "xtask site --check failed; run `cargo xtask site`\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
```

- [ ] **Step 2: Run the test to verify it fails against today's site**

Run: `cargo test --offline -p xtask --test site_output`
Expected: FAIL (`test result: FAILED. 3 passed; 2 failed`). `every_internal_link_and_anchor_resolves` panics on its first assertion and prints the pages it found, with no `404.html` among them. `generator_check_passes_on_the_committed_site` panics with ``xtask site --check failed; run `cargo xtask site` `` followed by ``Error: site/ is out of date; run `cargo xtask site`:`` and lines including `index.html differs`, `404.html is missing`, `docs/outcomes.html differs`, `docs/examples.md differs`, `search-index.json is missing`, and `sitemap.xml differs`. `robots.txt` is not listed: Task 4 already reproduces it byte for byte.

- [ ] **Step 3: Retire the Node build**

```bash
git rm site/package.json site/package-lock.json site/styles.css site/docs.css
git rm -r site/scripts
```

In `site/.gitignore`, find

```text
node_modules/
_deploy_files.json
```

and replace it with

```text
_deploy_files.json
```

`site/README.md` still describes `npm run build-docs`; Task 17 rewrites it, so leave it for now.

- [ ] **Step 4: Cache headers for the fingerprinted assets and the search index**

In `site/vercel.json`, find the fonts entry

```json
    {
      "source": "/fonts/(.*)",
      "headers": [
        {
          "key": "Cache-Control",
          "value": "public, max-age=31536000, immutable"
        }
      ]
    },
```

and replace it with the same entry followed by an identical one for `/assets/(.*)` (the pages load `/assets/fidryn.css?v=…` and `/assets/fidryn.js?v=…`, so a changed file gets a new URL):

```json
    {
      "source": "/fonts/(.*)",
      "headers": [
        {
          "key": "Cache-Control",
          "value": "public, max-age=31536000, immutable"
        }
      ]
    },
    {
      "source": "/assets/(.*)",
      "headers": [
        {
          "key": "Cache-Control",
          "value": "public, max-age=31536000, immutable"
        }
      ]
    },
```

Then find the end of the file

```json
    {
      "source": "/llms-full.txt",
      "headers": [
        {
          "key": "Content-Type",
          "value": "text/plain; charset=utf-8"
        },
        {
          "key": "Cache-Control",
          "value": "public, max-age=0, s-maxage=3600"
        }
      ]
    }
  ]
}
```

and replace it with (the search index is regenerated on every deploy, so browsers revalidate it and the edge keeps it for an hour, like the other generated text files):

```json
    {
      "source": "/llms-full.txt",
      "headers": [
        {
          "key": "Content-Type",
          "value": "text/plain; charset=utf-8"
        },
        {
          "key": "Cache-Control",
          "value": "public, max-age=0, s-maxage=3600"
        }
      ]
    },
    {
      "source": "/search-index.json",
      "headers": [
        {
          "key": "Cache-Control",
          "value": "public, max-age=0, s-maxage=3600"
        }
      ]
    }
  ]
}
```

The existing entries stay as they are. Check the file still parses: `python3 -m json.tool site/vercel.json > /dev/null` prints nothing.

- [ ] **Step 5: Run the site check in CI**

In `xtask/src/ci.rs`, find

```rust
    run_schema_probes()?;
```

and replace it with

```rust
    run_schema_probes()?;
    crate::site::check(&crate::workspace::workspace_root())?;
```

- [ ] **Step 6: Generate the site**

Run: `cargo xtask site`
Expected: `wrote 26 files under site/`.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test --offline -p xtask --test site_output`
Expected: PASS (`test result: ok. 5 passed`).

Run: `cargo xtask site --check`
Expected: `site/ is up to date (26 generated files)`.

Run: `cargo clippy --offline -p xtask --all-targets -- -D warnings`
Expected: finishes with no warnings; every generator item is now reachable from `build`.

- [ ] **Step 8: Review the generated files**

Run: `git status --short site`
Expected: the five Node files deleted; `site/.gitignore` and `site/vercel.json` modified; the pages, mirrors, and text files modified; `site/404.html` and `site/search-index.json` new.

Then look at these, in order:

1. `git diff -- site/robots.txt` prints nothing.
2. `git diff -- site/index.md site/llms.txt site/sitemap.xml` shows only the guides reordered (Cases and time now third, after Language) and, in `index.md`, "Cases & time" now "Cases and time". The sitemap keeps its `changefreq` and `priority` values.
3. `head -12 site/docs/examples.md` shows one front-matter block, the two-line canonical note, a blank line, then `# Example corpus`. `site/docs/cli.md`, `outcomes.md`, and `examples.md` are now the full guides (they were short summaries), and every mirror's links start with `https://` or `#`: `grep -o '](\([^h#][^)]*\))' site/docs/*.md` prints nothing.
4. `grep -c '^========== ' site/llms-full.txt` prints `9`.
5. `grep -o '<span class="hn">7.1</span> [^<]*' site/docs/examples.html` prints the 7.1 heading, `How to use an example`: the Examples numbering starts at its first real section.
6. `grep -o 'data-n="\(47\|63\|205\|212\)"' site/index.html | sort | uniq -c` counts each of the four line numbers twice (both trust runs show lines 47 to 63 and 205 to 212).
7. `grep -o '<li class="boundary">[^<]*' site/index.html` prints four lines: two ending `complete_Massachusetts_trust_law.` and two ending `complete_instruments.`.

Serve the site for a quick look: `python3 -m http.server 8000 --directory site` (it does not apply `cleanUrls`, so open the `.html` paths). At `http://127.0.0.1:8000/index.html`: four specimen tabs with the first selected; the Source step numbered 47 to 63, a gap, then 205 to 212; the Case step with the case JSON; the Outcome step with a Contingent stamp over "acting_trustee depends on SuccessorEligibility.", "Under I1 it is Alice.", "Under I2 it is Bob.". Selecting the second tab shows "acting_trustee is Bob.". At `/docs/outcomes.html`: the sidebar with "§6 Outcomes" marked, the kicker "§ 6 · Read results", headings numbered 6.1 onward, the On this page list, and the pager "Previous · §5 Mill" and "Next · §7 Examples". At `/404.html`: "No such provision." Stop the server with Ctrl-C.

- [ ] **Step 9: Commit**

```bash
git add xtask/tests/site_output.rs xtask/src/ci.rs site
git -c commit.gpgsign=false commit -m "feat(site): generate the site with cargo xtask site and retire the Node build"
```

### Task 13: Mill static assets, CSP, and samples

**Files:**
- Modify: `crates/fidryn-cli/src/ui.rs` (axum imports; constants after `const INDEX`; router; `index` handler; new `static_text`, `site_css`, `favicon`, `font`, `Sample`, `SAMPLES`, `samples`; new tests in `mod tests` after `index_returns_html_mill`)
- Modify: `docs/mill.md` (Routes table: new GET rows and a paragraph under the table)
- Test: `crates/fidryn-cli/src/ui.rs` (`mod tests`)

**Interfaces:**
- Consumes: `site/assets/fidryn.css` and `site/favicon.svg`, created by Task 5 earlier in execution; `include_str!` fails the build if either is missing, so Task 5 must be committed first. The existing `site/fonts/fraunces.woff2`, `plex-sans.woff2`, `plex-mono-400.woff2`, `plex-mono-500.woff2`. The fixtures `tests/programs/require-gate.fr`, `tests/programs/late-payment.fr`, `examples/trust/bryan-revocable-trust.fr`, and `examples/trust/cases/{two-certificates-open-eligibility,court-selects-i2,one-certificate}.json`. From Task 3's `ui.rs` tests: `post_json`, `mill_report`, `body_text` (all pre-existing).
- Produces (C3):
  - `GET /` sends `content-type: text/html; charset=utf-8`, `content-security-policy: default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`, `x-content-type-options: nosniff`, `referrer-policy: no-referrer`, `cache-control: no-cache`.
  - `GET /assets/fidryn.css` (`text/css; charset=utf-8`), `GET /favicon.svg` (`image/svg+xml`), `GET /fonts/{name}` (`font/woff2` for the four names, 404 otherwise), all with `cache-control: no-cache`.
  - `GET /api/samples`: `application/json`, the five samples of the C3 table in order, each `{"id","title","blurb","source","case","query","validAt","knownAt","action","expect"}`, `case` as text. Task 15 loads it.
  - Anchors for Tasks 14 and 15: `const SITE_CSS: &str = include_str!("../../../site/assets/fidryn.css");` and `const FAVICON: &str = include_str!("../../../site/favicon.svg");` directly after `const INDEX` (Task 14 adds `const MILL_CSS` on the line after `const FAVICON`); `fn static_text(content_type: &'static str, body: &'static str) -> Response`; the router lines `.route("/assets/fidryn.css", get(site_css))`, `.route("/favicon.svg", get(favicon))`, `.route("/fonts/{name}", get(font))` right after `.route("/", get(index))` (Task 14 adds its route on the line after the `site_css` one), and `.route("/api/samples", get(samples))` right after `/api/health`. `Response` is imported from `axum::response`.
  - Test helpers in `ui.rs` `mod tests` for Tasks 14 and 15: `async fn get_response(uri: &str) -> axum::response::Response`, `fn header_text<'a>(response: &'a axum::response::Response, name: &header::HeaderName) -> &'a str`, `fn repo_bytes(rel: &str) -> Vec<u8>`, `fn repo_text(rel: &str) -> String`, and `async fn assert_static_route(uri: &str, content_type: &str, rel: &str)` (200, the content type, `cache-control: no-cache`, and the bytes of repository file `rel`). Task 14's route test is one line: `assert_static_route("/assets/mill.css", "text/css; charset=utf-8", "web/mill.css").await;`. Put new route tests after `unknown_fonts_and_assets_are_not_found`.

Note for anyone running the mill between this task and Task 15: the current `web/index.html` has an inline `<style>` and `<script>`, which this CSP blocks, so the page renders unstyled and inert in a browser until Task 14 (new page) and Task 15 (`mill.js`) land. The tests are unaffected.

- Note: from this task until Task 14 replaces `web/index.html`, the new CSP blocks the old page's inline `<style>` and `<script>`, so the mill page does not work in a browser in between. Tests are unaffected; Task 14 ships a CSP-compliant page.

- [ ] **Step 1: Write the failing test**

Add the helpers and tests right after `index_returns_html_mill` (which stays unchanged). The expected sample sources and cases are read from the repository at test time, so the test proves the binary embedded the right files.

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
        assert!(html.contains(">Render<"), "{html}");
    }
```

Replace it with:

```rust
        assert!(html.contains(">Render<"), "{html}");
    }

    async fn get_response(uri: &str) -> axum::response::Response {
        router()
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    fn header_text<'a>(
        response: &'a axum::response::Response,
        name: &header::HeaderName,
    ) -> &'a str {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
    }

    /// A repository file, read at test time to compare with what the
    /// binary embedded at compile time.
    fn repo_bytes(rel: &str) -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel);
        std::fs::read(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
    }

    fn repo_text(rel: &str) -> String {
        String::from_utf8(repo_bytes(rel)).expect("UTF-8 repository file")
    }

    /// `uri` answers 200 with `content_type`, `cache-control: no-cache`,
    /// and exactly the bytes of the repository file `rel`.
    async fn assert_static_route(uri: &str, content_type: &str, rel: &str) {
        let response = get_response(uri).await;
        assert_eq!(response.status(), StatusCode::OK, "{uri}");
        assert_eq!(
            header_text(&response, &header::CONTENT_TYPE),
            content_type,
            "{uri}"
        );
        assert_eq!(
            header_text(&response, &header::CACHE_CONTROL),
            "no-cache",
            "{uri}"
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(
            body.as_ref() == repo_bytes(rel).as_slice(),
            "{uri} must serve {rel}"
        );
    }

    #[tokio::test]
    async fn index_sends_the_csp_and_security_headers() {
        let response = get_response("/").await;
        assert_eq!(response.status(), StatusCode::OK);
        for (name, expected) in [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::CACHE_CONTROL, "no-cache"),
        ] {
            assert_eq!(header_text(&response, &name), expected, "{name}");
        }
    }

    #[tokio::test]
    async fn site_stylesheet_and_favicon_are_embedded() {
        assert_static_route(
            "/assets/fidryn.css",
            "text/css; charset=utf-8",
            "site/assets/fidryn.css",
        )
        .await;
        assert_static_route("/favicon.svg", "image/svg+xml", "site/favicon.svg").await;
    }

    #[tokio::test]
    async fn the_four_fonts_are_embedded() {
        for name in [
            "fraunces.woff2",
            "plex-sans.woff2",
            "plex-mono-400.woff2",
            "plex-mono-500.woff2",
        ] {
            assert_static_route(
                &format!("/fonts/{name}"),
                "font/woff2",
                &format!("site/fonts/{name}"),
            )
            .await;
        }
    }

    #[tokio::test]
    async fn unknown_fonts_and_assets_are_not_found() {
        for uri in [
            "/fonts/comic-sans.woff2",
            "/fonts/LICENSE.md",
            "/fonts/fraunces.woff",
            "/assets/site.css",
        ] {
            assert_eq!(
                get_response(uri).await.status(),
                StatusCode::NOT_FOUND,
                "{uri}"
            );
        }
    }

    const EMPTY_CASE_TEXT: &str =
        "{\n  \"schema\": \"fidryn.case-record/v0.1\",\n  \"admissibleCompletions\": {}\n}\n";

    #[tokio::test]
    async fn samples_are_the_five_fixtures_in_order() {
        let response = get_response("/api/samples").await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            header_text(&response, &header::CONTENT_TYPE),
            "application/json"
        );
        let samples: serde_json::Value =
            serde_json::from_str(&body_text(response).await).expect("samples JSON");
        let gate = "2026-09-17T12:00:00Z";
        let trust = "2034-03-01T09:00:00Z";
        let trust_source = repo_text("examples/trust/bryan-revocable-trust.fr");
        let expected = serde_json::json!([
            {
                "id": "require-gate",
                "title": "require-gate",
                "blurb": "q returns 7; r stops at a false require",
                "source": repo_text("tests/programs/require-gate.fr"),
                "case": EMPTY_CASE_TEXT,
                "query": "q",
                "validAt": gate,
                "knownAt": gate,
                "action": "run",
                "expect": "determinate"
            },
            {
                "id": "late-payment",
                "title": "late-payment",
                "blurb": "A duty; paid_on_time needs evidence",
                "source": repo_text("tests/programs/late-payment.fr"),
                "case": EMPTY_CASE_TEXT,
                "query": "due",
                "validAt": gate,
                "knownAt": gate,
                "action": "run",
                "expect": "determinate"
            },
            {
                "id": "trust-open",
                "title": "Trust, open eligibility",
                "blurb": "Two certificates; clause 4.4 unresolved",
                "source": trust_source,
                "case": repo_text("examples/trust/cases/two-certificates-open-eligibility.json"),
                "query": "acting_trustee",
                "validAt": trust,
                "knownAt": trust,
                "action": "run",
                "expect": "contingent"
            },
            {
                "id": "trust-court",
                "title": "Trust, court selects I2",
                "blurb": "A competent authority has decided",
                "source": trust_source,
                "case": repo_text("examples/trust/cases/court-selects-i2.json"),
                "query": "acting_trustee",
                "validAt": trust,
                "knownAt": trust,
                "action": "run",
                "expect": "determinate"
            },
            {
                "id": "trust-one",
                "title": "Trust, one certificate",
                "blurb": "Evidence is still missing",
                "source": trust_source,
                "case": repo_text("examples/trust/cases/one-certificate.json"),
                "query": "acting_trustee",
                "validAt": trust,
                "knownAt": trust,
                "action": "run",
                "expect": "suspended"
            }
        ]);
        assert_eq!(samples, expected);
    }

    #[tokio::test]
    async fn every_sample_yields_its_expected_kind() {
        let response = get_response("/api/samples").await;
        let samples: serde_json::Value =
            serde_json::from_str(&body_text(response).await).expect("samples JSON");
        for sample in samples.as_array().expect("samples array") {
            let id = &sample["id"];
            let case: serde_json::Value =
                serde_json::from_str(sample["case"].as_str().expect("case text"))
                    .unwrap_or_else(|err| panic!("{id}: case JSON: {err}"));
            let body = serde_json::json!({
                "source": sample["source"],
                "query": sample["query"],
                "case": case,
                "validAt": sample["validAt"],
                "knownAt": sample["knownAt"],
            });
            let uri = format!("/api/{}", sample["action"].as_str().expect("action"));
            let (status, json) = post_json(&uri, body).await;
            assert_eq!(status, StatusCode::OK, "{id}: {json}");
            assert_eq!(
                mill_report(&json)["outcomeDocument"]["outcome"]["kind"],
                sample["expect"],
                "{id}: {json}"
            );
        }
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p fidryn-cli --offline --lib ui::tests`
Expected: FAIL (`test result: FAILED. 25 passed; 5 failed`): `index_sends_the_csp_and_security_headers` (`assertion ``left == right`` failed: content-security-policy`, `left: ""`), `site_stylesheet_and_favicon_are_embedded` and `the_four_fonts_are_embedded` (`left: 404`, `right: 200`), `samples_are_the_five_fixtures_in_order` (`left: 404`), and `every_sample_yields_its_expected_kind` (`samples JSON: Error("EOF while parsing a value", line: 1, column: 0)`). `unknown_fonts_and_assets_are_not_found` already passes and must keep passing.

- [ ] **Step 3: Write the implementation**

Imports:

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
use axum::extract::{Json, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse};
```

Replace it with:

```rust
use axum::extract::{Json, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
```

Constants. `SITE_CSS` and `FAVICON` go directly after `INDEX`; the CSP and the font table go after `MILL_CPU_SLOTS`:

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
const INDEX: &str = include_str!("../../../web/index.html");
const DEFAULT_PORT: u16 = 8751;
/// Concurrent check / run / explore / render workers. Extra requests wait
/// on the semaphore; they do not occupy extra blocking threads.
const MILL_CPU_SLOTS: usize = 4;
```

Replace it with:

```rust
const INDEX: &str = include_str!("../../../web/index.html");
const SITE_CSS: &str = include_str!("../../../site/assets/fidryn.css");
const FAVICON: &str = include_str!("../../../site/favicon.svg");
const DEFAULT_PORT: u16 = 8751;
/// Concurrent check / run / explore / render workers. Extra requests wait
/// on the semaphore; they do not occupy extra blocking threads.
const MILL_CPU_SLOTS: usize = 4;
/// Sent with `GET /`: scripts, styles, and fonts from this origin only (no
/// inline script or style), images from this origin or data URIs, no
/// plugins, no `<base>`, and no framing.
const CSP: &str = "default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'";
/// The site's self-hosted fonts, served at `/fonts/{name}`.
const FONTS: &[(&str, &[u8])] = &[
    (
        "fraunces.woff2",
        include_bytes!("../../../site/fonts/fraunces.woff2"),
    ),
    (
        "plex-sans.woff2",
        include_bytes!("../../../site/fonts/plex-sans.woff2"),
    ),
    (
        "plex-mono-400.woff2",
        include_bytes!("../../../site/fonts/plex-mono-400.woff2"),
    ),
    (
        "plex-mono-500.woff2",
        include_bytes!("../../../site/fonts/plex-mono-500.woff2"),
    ),
];
```

Routes, in the order C3 fixes:

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
        .route("/", get(index))
        .route("/api/health", get(health))
```

Replace it with:

```rust
        .route("/", get(index))
        .route("/assets/fidryn.css", get(site_css))
        .route("/favicon.svg", get(favicon))
        .route("/fonts/{name}", get(font))
        .route("/api/health", get(health))
        .route("/api/samples", get(samples))
```

Handlers. `index` gains the security headers; the asset, font, and sample handlers follow it. The sample table is fixed by C3; sources and cases are the repository files, embedded at compile time.

In `crates/fidryn-cli/src/ui.rs`, find:

```rust
async fn index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        Html(INDEX),
    )
}

async fn health() -> &'static str {
    "ok"
}
```

Replace it with:

```rust
async fn index() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CONTENT_SECURITY_POLICY, CSP),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        Html(INDEX),
    )
        .into_response()
}

/// A compile-time embedded text asset, revalidated on every load.
fn static_text(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

async fn site_css() -> Response {
    static_text("text/css; charset=utf-8", SITE_CSS)
}

async fn favicon() -> Response {
    static_text("image/svg+xml", FAVICON)
}

/// One of [`FONTS`] by file name. Any other name is 404; nothing is read
/// from disk.
async fn font(Path(name): Path<String>) -> Response {
    match FONTS.iter().find(|(file, _)| *file == name) {
        Some(&(_, bytes)) => (
            [
                (header::CONTENT_TYPE, "font/woff2"),
                (header::CACHE_CONTROL, "no-cache"),
            ],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn health() -> &'static str {
    "ok"
}

/// One built-in example on the mill's Samples rail (`GET /api/samples`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    id: &'static str,
    title: &'static str,
    blurb: &'static str,
    /// Module source text.
    source: &'static str,
    /// Case record JSON text, exactly as the editor shows it.
    case: &'static str,
    query: &'static str,
    valid_at: &'static str,
    known_at: &'static str,
    /// The action the rail's stamp describes: `run` or `explore`.
    action: &'static str,
    /// The outcome kind `action` returns for this sample.
    expect: &'static str,
}

/// The empty case record, pretty-printed for the editor.
const EMPTY_CASE: &str =
    "{\n  \"schema\": \"fidryn.case-record/v0.1\",\n  \"admissibleCompletions\": {}\n}\n";
const TRUST_SOURCE: &str = include_str!("../../../examples/trust/bryan-revocable-trust.fr");

/// Samples in rail order. Sources and cases are the repository fixtures,
/// embedded at compile time.
const SAMPLES: &[Sample] = &[
    Sample {
        id: "require-gate",
        title: "require-gate",
        blurb: "q returns 7; r stops at a false require",
        source: include_str!("../../../tests/programs/require-gate.fr"),
        case: EMPTY_CASE,
        query: "q",
        valid_at: "2026-09-17T12:00:00Z",
        known_at: "2026-09-17T12:00:00Z",
        action: "run",
        expect: "determinate",
    },
    Sample {
        id: "late-payment",
        title: "late-payment",
        blurb: "A duty; paid_on_time needs evidence",
        source: include_str!("../../../tests/programs/late-payment.fr"),
        case: EMPTY_CASE,
        query: "due",
        valid_at: "2026-09-17T12:00:00Z",
        known_at: "2026-09-17T12:00:00Z",
        action: "run",
        expect: "determinate",
    },
    Sample {
        id: "trust-open",
        title: "Trust, open eligibility",
        blurb: "Two certificates; clause 4.4 unresolved",
        source: TRUST_SOURCE,
        case: include_str!("../../../examples/trust/cases/two-certificates-open-eligibility.json"),
        query: "acting_trustee",
        valid_at: "2034-03-01T09:00:00Z",
        known_at: "2034-03-01T09:00:00Z",
        action: "run",
        expect: "contingent",
    },
    Sample {
        id: "trust-court",
        title: "Trust, court selects I2",
        blurb: "A competent authority has decided",
        source: TRUST_SOURCE,
        case: include_str!("../../../examples/trust/cases/court-selects-i2.json"),
        query: "acting_trustee",
        valid_at: "2034-03-01T09:00:00Z",
        known_at: "2034-03-01T09:00:00Z",
        action: "run",
        expect: "determinate",
    },
    Sample {
        id: "trust-one",
        title: "Trust, one certificate",
        blurb: "Evidence is still missing",
        source: TRUST_SOURCE,
        case: include_str!("../../../examples/trust/cases/one-certificate.json"),
        query: "acting_trustee",
        valid_at: "2034-03-01T09:00:00Z",
        known_at: "2034-03-01T09:00:00Z",
        action: "run",
        expect: "suspended",
    },
];

async fn samples() -> Json<&'static [Sample]> {
    Json(SAMPLES)
}
```

Update the mill guide's Routes table:

In `docs/mill.md`, find:

```markdown
| `GET` | `/` | none | `web/index.html` (`text/html; charset=utf-8`) |
| `GET` | `/api/health` | none | plain text `ok` |
```

Replace it with:

```markdown
| `GET` | `/` | none | `web/index.html` (`text/html; charset=utf-8`), with the security headers below |
| `GET` | `/assets/fidryn.css` | none | `site/assets/fidryn.css`, the design system the site also uses (`text/css; charset=utf-8`) |
| `GET` | `/favicon.svg` | none | `site/favicon.svg` (`image/svg+xml`) |
| `GET` | `/fonts/{name}` | none | `fraunces.woff2`, `plex-sans.woff2`, `plex-mono-400.woff2`, or `plex-mono-500.woff2` from `site/fonts/` (`font/woff2`); any other name is 404 |
| `GET` | `/api/health` | none | plain text `ok` |
| `GET` | `/api/samples` | none | JSON array of the built-in samples, each `{id, title, blurb, source, case, query, validAt, knownAt, action, expect}` |
```

and add the paragraph under the table:

In `docs/mill.md`, find:

```markdown
There is no `POST /api/file`, `/api/filing`, `/api/submit`, or
`/api/live`. Those paths return 404.
```

Replace it with:

```markdown
The page, stylesheet, favicon, fonts, and samples are compiled into the
`fidryn` binary; the mill reads no files at runtime. The page,
stylesheet, favicon, and fonts are sent with `Cache-Control: no-cache`.
`GET /` also sends
`Content-Security-Policy: default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`,
`X-Content-Type-Options: nosniff`, and `Referrer-Policy: no-referrer`,
so the browser runs no inline script or style on the page and will not
show it inside a frame. In a sample, `source` and `case` are the text of
a repository fixture (`case` is JSON text, not a parsed object), and
`expect` is the outcome kind that `action` returns for it.

There is no `POST /api/file`, `/api/filing`, `/api/submit`, or
`/api/live`. Those paths return 404.
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p fidryn-cli --offline --lib ui::tests`
Expected: PASS (`test result: ok. 30 passed; 0 failed`)

Run: `cargo test -p fidryn-cli --offline`
Expected: PASS; the suites report `76 passed` (lib), `0 passed` (main), `13 passed` (`cli_smoke`), `9 passed` (`run_report_text`), `0 passed` (doc-tests).

Run: `cargo clippy -p fidryn-cli --offline --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: `Finished` with no warnings, and no output from `cargo fmt`.

Check the real server once:

```bash
cargo build -q -p fidryn-cli --offline
./target/debug/fidryn ui --no-open --port 18751 & MILL=$!
sleep 1
curl -sI http://127.0.0.1:18751/
curl -s -o /dev/null -w '%{http_code} %{content_type}\n' http://127.0.0.1:18751/fonts/plex-sans.woff2
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:18751/fonts/LICENSE.md
curl -s http://127.0.0.1:18751/api/samples | python3 -c 'import json,sys; print([s["id"] for s in json.load(sys.stdin)])'
kill $MILL
```

Expected: the `HEAD /` response lists `content-security-policy: default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`, `x-content-type-options: nosniff`, `referrer-policy: no-referrer`, and `cache-control: no-cache`; then `200 font/woff2`; then `404`; then `['require-gate', 'late-payment', 'trust-open', 'trust-court', 'trust-one']`.

- [ ] **Step 5: Commit**

```bash
git add crates/fidryn-cli/src/ui.rs docs/mill.md
git -c commit.gpgsign=false commit -m "feat(mill): serve embedded assets, fonts, and samples under a strict CSP"
```

### Task 14: Mill page and styles

**Files:**
- Modify: `web/index.html` (replace the whole file)
- Create: `web/mill.css`
- Modify: `crates/fidryn-cli/src/ui.rs` (`const MILL_CSS` on the line after `const FAVICON`; `async fn mill_css` after `async fn site_css`; the `/assets/mill.css` route after the `/assets/fidryn.css` route; one router test before `mill_binds_loopback_only` in `mod tests`)
- Create: `xtask/tests/mill_page.rs`
- Test: `xtask/tests/mill_page.rs`; `ui::tests::mill_css_route_serves_the_embedded_stylesheet` in `crates/fidryn-cli/src/ui.rs`

**Interfaces:**
- Consumes: from Task 13 (C3), in `crates/fidryn-cli/src/ui.rs`: the line `const FAVICON: &str = include_str!("../../../site/favicon.svg");`, `fn static_text(content_type: &'static str, body: &'static str) -> Response`, `async fn site_css() -> Response` exactly as C3 prints it, the router line `.route("/assets/fidryn.css", get(site_css))`, the `axum::response::Response` import, and the CSP header on `GET /`. From Task 5 (C7): the tokens, the `@font-face` rules, `[hidden]`, and the shared classes `site-head`, `brand`, `mono`, `wordmark`, `label`, `btn`, `pri`, `sec`, `sm`, `actions`, `stamp` (with `det con sus nc oc inc`), `code`, `copy`, `table-wrap`, `table`/`th`/`td`, `theme-toggle`, `skip`, `sr-only`.
- Produces: `web/index.html` exactly per C9 (every C9 id exactly once; `button[data-buffer="module|case|template"]` with `role="tab"` and `aria-selected`; `button[data-view="opinion|table|json"]` with `aria-pressed`; the theme toggle ships `hidden`). Private hooks that `web/mill.js` (Tasks 15 and 16) relies on: `#confirm-text` (the Replace question), `.mill-health-text` inside `#health` (the status word, after a `.mill-dot`), `.mill-host` (the `localhost 127.0.0.1` text, visually hidden on phones), `.mill-mod` (modifier-key labels, rewritten to the Command sign on Macs), `#editor-help`. `#editor-wrap[data-hl]` switches the textarea to transparent text over `#hl`; without that attribute (until Task 16 sets it) the textarea shows plain text, `#hl` is hidden, and an empty `#gutter` takes no space. `web/mill.css` styles every `mill-` class used by Tasks 15 and 16 (`.mill-gutter-in`, `.mill-ln`, `.has-diag`, `.mill-sq`, `.mill-pop`, `.mill-status-main`, `.mill-status-hint`, `.mill-doc*`, `.mill-table`, `.mill-diag-*`, `.mill-error*`, `.mill-hist*`, `.mill-sample`). In `ui.rs`: `const MILL_CSS: &str`, `async fn mill_css() -> Response`, and `GET /assets/mill.css` answering `text/css; charset=utf-8` with `cache-control: no-cache`.

Layout, from the picks: L13 list rail (samples with abbreviated stamps, then history), L15 segmented Module / Case JSON / Template switch over a borderless editor, L14 result with an Opinion / Table / JSON switch, L17 phone stack with a fixed Check / Run / Explore / Render bar, L18 first run shows "Nothing has run yet." with the Run shortcut. Desktop (1200px and up) is a 210px rail, the editor, and the query/result column; tablet (720–1199px) and phone (below 720px) are one column in the order samples, editor, query and result, history; samples become chips (wrapping on tablets, a horizontal scroller on phones).

- [ ] **Step 1: Write the failing tests**

Create `xtask/tests/mill_page.rs`:

```rust
//! `web/index.html` is served under the mill's Content-Security-Policy
//! (`default-src 'self'`), so it may not carry inline script, inline style,
//! or handler attributes. It must also keep the strings the router tests
//! look for and every element id `web/mill.js` reads.

use std::collections::BTreeMap;
use std::path::PathBuf;

fn page() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/index.html");
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

/// One start tag: lowercase name and attributes in source order
/// (`None` for a bare attribute such as `hidden`).
struct Tag {
    name: String,
    attrs: Vec<(String, Option<String>)>,
}

impl Tag {
    fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_deref().unwrap_or(""))
    }
}

/// Every start tag in `html`, skipping comments, end tags, and the doctype.
/// Quoted attribute values may hold spaces, `>`, and `=`.
fn start_tags(html: &str) -> Vec<Tag> {
    let bytes = html.as_bytes();
    let mut tags = Vec::new();
    let mut i = 0;
    while let Some(offset) = html[i..].find('<') {
        let at = i + offset;
        if html[at..].starts_with("<!--") {
            i = html[at..]
                .find("-->")
                .map_or(html.len(), |end| at + end + 3);
            continue;
        }
        if !bytes.get(at + 1).is_some_and(u8::is_ascii_alphabetic) {
            i = at + 1;
            continue;
        }
        let mut j = at + 1;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
            j += 1;
        }
        let name = html[at + 1..j].to_ascii_lowercase();
        let mut attrs = Vec::new();
        loop {
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j >= bytes.len() {
                break;
            }
            if bytes[j] == b'>' {
                j += 1;
                break;
            }
            if bytes[j] == b'/' {
                j += 1;
                continue;
            }
            let key_start = j;
            while j < bytes.len()
                && !bytes[j].is_ascii_whitespace()
                && !matches!(bytes[j], b'=' | b'>' | b'/')
            {
                j += 1;
            }
            let key = html[key_start..j].to_ascii_lowercase();
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let mut value = None;
            if j < bytes.len() && bytes[j] == b'=' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && matches!(bytes[j], b'"' | b'\'') {
                    let quote = bytes[j];
                    let value_start = j + 1;
                    j = value_start;
                    while j < bytes.len() && bytes[j] != quote {
                        j += 1;
                    }
                    value = Some(html[value_start..j].to_owned());
                    j += 1;
                } else {
                    let value_start = j;
                    while j < bytes.len() && !bytes[j].is_ascii_whitespace() && bytes[j] != b'>' {
                        j += 1;
                    }
                    value = Some(html[value_start..j].to_owned());
                }
            }
            attrs.push((key, value));
        }
        tags.push(Tag { name, attrs });
        i = j;
    }
    tags
}

#[test]
fn page_keeps_the_strings_the_mill_is_known_by() {
    let html = page();
    for needle in [
        "<title>fidryn mill</title>",
        "<h1 class=\"sr-only\">fidryn mill</h1>",
        "localhost 127.0.0.1",
        "Live filing is not available from the UI",
        ">Check<",
        ">Run<",
        ">Explore<",
        ">Render<",
    ] {
        assert!(html.contains(needle), "web/index.html lacks {needle:?}");
    }
}

#[test]
fn head_loads_the_shared_styles_the_favicon_and_mill_js() {
    let html = page();
    let head_end = html.find("</head>").expect("web/index.html has a </head>");
    for needle in [
        "<link rel=\"stylesheet\" href=\"/assets/fidryn.css\">",
        "<link rel=\"stylesheet\" href=\"/assets/mill.css\">",
        "<link rel=\"icon\" href=\"/favicon.svg\" type=\"image/svg+xml\">",
        "<script src=\"/assets/mill.js\"></script>",
    ] {
        let at = html
            .find(needle)
            .unwrap_or_else(|| panic!("web/index.html lacks {needle:?}"));
        assert!(at < head_end, "{needle:?} must be inside <head>");
    }
}

#[test]
fn page_has_no_inline_script_style_or_handlers() {
    let html = page();
    assert!(
        !html.contains("<style"),
        "inline <style> is blocked by the CSP"
    );
    for tag in start_tags(&html) {
        for (key, _) in &tag.attrs {
            assert_ne!(key, "style", "<{}> has a style attribute", tag.name);
            assert!(
                !key.starts_with("on"),
                "<{}> has a handler attribute {key}",
                tag.name
            );
        }
        if tag.name == "script" {
            assert!(tag.attr("src").is_some(), "every <script> must load a file");
        }
    }
    let mut rest = html.as_str();
    while let Some(at) = rest.find("<script") {
        let after = &rest[at..];
        let close = after.find('>').expect("<script> tag is closed");
        assert!(
            after[close + 1..].starts_with("</script>"),
            "a <script> element has an inline body"
        );
        rest = &after[close + 1..];
    }
}

#[test]
fn page_has_every_element_id_mill_js_reads_once() {
    let tags = start_tags(&page());
    let mut ids: BTreeMap<String, usize> = BTreeMap::new();
    for tag in &tags {
        if let Some(id) = tag.attr("id") {
            *ids.entry(id.to_owned()).or_default() += 1;
        }
    }
    for (id, count) in &ids {
        assert_eq!(*count, 1, "id {id:?} appears {count} times");
    }
    for id in [
        "health",
        "samples",
        "confirm",
        "confirm-replace",
        "confirm-cancel",
        "history",
        "buffers",
        "buffer-status",
        "editor-wrap",
        "gutter",
        "hl",
        "editor",
        "diag-pop",
        "query",
        "query-names",
        "validAt",
        "validAt-err",
        "knownAt",
        "knownAt-err",
        "check",
        "run",
        "explore",
        "render",
        "views",
        "result-body",
    ] {
        assert!(ids.contains_key(id), "web/index.html lacks id {id:?}");
    }
    let editor = tags
        .iter()
        .find(|t| t.attr("id") == Some("editor"))
        .expect("#editor");
    assert_eq!(editor.name, "textarea");
    let names = tags
        .iter()
        .find(|t| t.attr("id") == Some("query-names"))
        .expect("#query-names");
    assert_eq!(names.name, "datalist");
}

#[test]
fn buffer_tabs_and_view_buttons_carry_their_data_attributes() {
    let tags = start_tags(&page());
    let buffers: Vec<&Tag> = tags
        .iter()
        .filter(|t| t.attr("data-buffer").is_some())
        .collect();
    let names: Vec<&str> = buffers
        .iter()
        .filter_map(|t| t.attr("data-buffer"))
        .collect();
    assert_eq!(names, ["module", "case", "template"]);
    for tab in &buffers {
        assert_eq!(tab.name, "button");
        assert_eq!(tab.attr("role"), Some("tab"));
        assert!(
            tab.attr("aria-selected").is_some(),
            "buffer tab without aria-selected"
        );
    }
    let views: Vec<&str> = tags.iter().filter_map(|t| t.attr("data-view")).collect();
    assert_eq!(views, ["opinion", "table", "json"]);
}

#[test]
fn page_reuses_the_shared_design_classes() {
    let tags = start_tags(&page());
    let classes: Vec<&str> = tags
        .iter()
        .filter_map(|t| t.attr("class"))
        .flat_map(str::split_whitespace)
        .collect();
    for class in [
        "site-head",
        "brand",
        "mono",
        "wordmark",
        "label",
        "btn",
        "pri",
        "sec",
        "sm",
        "actions",
        "theme-toggle",
        "skip",
        "sr-only",
    ] {
        assert!(
            classes.contains(&class),
            "web/index.html does not use .{class}"
        );
    }
    for class in &classes {
        let shared = [
            "site-head",
            "brand",
            "mono",
            "wordmark",
            "label",
            "btn",
            "pri",
            "sec",
            "sm",
            "actions",
            "theme-toggle",
            "skip",
            "sr-only",
            "page-mill",
        ];
        assert!(
            shared.contains(class) || class.starts_with("mill-"),
            "class {class:?} is neither shared nor prefixed mill-"
        );
    }
    let toggle = tags
        .iter()
        .find(|t| {
            t.attr("class")
                .is_some_and(|c| c.split_whitespace().any(|c| c == "theme-toggle"))
        })
        .expect(".theme-toggle");
    assert!(toggle.attr("data-theme-toggle").is_some());
    assert!(
        toggle.attr("hidden").is_some(),
        "the theme toggle ships hidden until mill.js runs"
    );
}
```

In `crates/fidryn-cli/src/ui.rs`, inside `mod tests`, add a router test for the new route next to Task 13's static-route tests.

Find:

```rust
    #[tokio::test]
    async fn mill_binds_loopback_only() {
```

Replace it with:

```rust
    #[tokio::test]
    async fn mill_css_route_serves_the_embedded_stylesheet() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/assets/mill.css")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/css; charset=utf-8"
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
        let css = body_text(response).await;
        assert!(
            css.contains(".mill-ed"),
            "mill.css styles the editor: {css}"
        );
        assert!(
            css.contains(".mill-doc"),
            "mill.css styles the opinion view"
        );
    }

    #[tokio::test]
    async fn mill_binds_loopback_only() {
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p xtask --offline --test mill_page`
Expected: FAIL with `test result: FAILED. 0 passed; 6 failed`. The panics name what the current page lacks, for example `web/index.html lacks "<h1 class=\"sr-only\">fidryn mill</h1>"`, `web/index.html lacks id "samples"`, `web/index.html lacks "<link rel=\"stylesheet\" href=\"/assets/fidryn.css\">"`, and `inline <style> is blocked by the CSP`.

Run: `cargo test -p fidryn-cli --offline --lib mill_css_route`
Expected: FAIL in `mill_css_route_serves_the_embedded_stylesheet` with ``assertion `left == right` failed``, `left: 404`, `right: 200` (no route yet).

- [ ] **Step 3: Write the page, the styles, and the route**

Replace the whole of `web/index.html` with:

```html
<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>fidryn mill</title>
<meta name="description" content="Check, run, explore, and render a Fidryn module on this machine. The mill never files.">
<meta name="robots" content="noindex">
<meta name="theme-color" content="#f7f2e8" media="(prefers-color-scheme: light)">
<meta name="theme-color" content="#15120e" media="(prefers-color-scheme: dark)">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="preload" href="/fonts/plex-sans.woff2" as="font" type="font/woff2" crossorigin>
<link rel="preload" href="/fonts/plex-mono-400.woff2" as="font" type="font/woff2" crossorigin>
<link rel="stylesheet" href="/assets/fidryn.css">
<link rel="stylesheet" href="/assets/mill.css">
<script src="/assets/mill.js"></script>
</head>
<body class="page-mill">
<a class="skip" href="#main">Skip to content</a>
<header class="site-head mill-head">
  <a class="brand" href="/"><span class="mono" aria-hidden="true">F</span><span class="wordmark">Fidryn</span></a>
  <span class="mill-tag">mill</span>
  <p id="health" class="mill-health" data-state="checking"><span class="mill-dot" aria-hidden="true"></span><span class="mill-host">localhost 127.0.0.1</span> <span class="mill-health-text" role="status">checking</span></p>
  <p class="mill-nofile">Live filing is not available from the UI</p>
  <a class="mill-guide" href="https://fidryn.onlygass.dev/docs/mill">Mill guide</a>
  <button class="theme-toggle" type="button" data-theme-toggle aria-label="Switch color theme" hidden><span aria-hidden="true"></span></button>
</header>
<main id="main" class="mill-page">
  <h1 class="sr-only">fidryn mill</h1>
  <noscript><p class="mill-noscript">The mill needs JavaScript to talk to the fidryn ui server on 127.0.0.1.</p></noscript>
  <nav class="mill-samples" aria-labelledby="samples-title">
    <h2 id="samples-title" class="label">Samples</h2>
    <ul id="samples" class="mill-list"></ul>
    <div id="confirm" class="mill-confirm" role="group" aria-labelledby="confirm-text" hidden>
      <p id="confirm-text">Replace your edits with this sample?</p>
      <div class="actions">
        <button id="confirm-replace" class="btn pri sm" type="button">Replace</button>
        <button id="confirm-cancel" class="btn sec sm" type="button">Cancel</button>
      </div>
    </div>
  </nav>
  <section class="mill-editor" aria-label="Editor">
    <div class="mill-editor-head">
      <div id="buffers" class="mill-seg" role="tablist" aria-label="Input">
        <button id="tab-module" type="button" role="tab" data-buffer="module" aria-selected="true" aria-controls="editor-wrap">Module</button>
        <button id="tab-case" type="button" role="tab" data-buffer="case" aria-selected="false" aria-controls="editor-wrap" tabindex="-1">Case JSON</button>
        <button id="tab-template" type="button" role="tab" data-buffer="template" aria-selected="false" aria-controls="editor-wrap" tabindex="-1">Template</button>
      </div>
      <p id="buffer-status" class="mill-status"></p>
    </div>
    <div id="editor-wrap" class="mill-ed" role="tabpanel" aria-labelledby="tab-module">
      <div id="gutter" class="mill-gutter" aria-hidden="true"></div>
      <div class="mill-code">
        <pre id="hl" class="mill-hl" aria-hidden="true"></pre>
        <textarea id="editor" class="mill-input" spellcheck="false" autocapitalize="off" autocomplete="off" autocorrect="off" wrap="off" aria-label="Module source" aria-describedby="editor-help buffer-status"></textarea>
        <div id="diag-pop" class="mill-pop" role="status" hidden></div>
      </div>
    </div>
    <p id="editor-help" class="sr-only">Tab indents and Shift+Tab outdents. Press Escape, then Tab, to leave the editor.</p>
  </section>
  <section class="mill-side" aria-label="Query and result">
    <div class="mill-params">
      <div class="mill-field mill-field-query">
        <label class="label" for="query">Query</label>
        <input id="query" type="text" list="query-names" spellcheck="false" autocapitalize="off" autocomplete="off">
        <datalist id="query-names"></datalist>
      </div>
      <div class="mill-field">
        <label class="label" for="validAt">validAt</label>
        <input id="validAt" type="text" spellcheck="false" autocapitalize="off" autocomplete="off" aria-describedby="validAt-err">
        <p id="validAt-err" class="mill-field-err" hidden></p>
      </div>
      <div class="mill-field">
        <label class="label" for="knownAt">knownAt</label>
        <input id="knownAt" type="text" spellcheck="false" autocapitalize="off" autocomplete="off" aria-describedby="knownAt-err">
        <p id="knownAt-err" class="mill-field-err" hidden></p>
      </div>
    </div>
    <div class="actions mill-actions" role="group" aria-label="Actions">
      <button id="check" class="btn sec sm" type="button">Check</button>
      <button id="run" class="btn pri sm" type="button">Run</button>
      <button id="explore" class="btn sec sm" type="button">Explore</button>
      <button id="render" class="btn sec sm" type="button">Render</button>
    </div>
    <p class="mill-keys"><kbd class="mill-mod">Ctrl</kbd>+<kbd>Enter</kbd> runs &middot; <kbd class="mill-mod">Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Enter</kbd> explores</p>
    <section class="mill-result" aria-labelledby="result-title">
      <div class="mill-result-head">
        <h2 id="result-title" class="label">Result</h2>
        <div id="views" class="mill-seg" role="group" aria-label="Result view">
          <button type="button" data-view="opinion" aria-pressed="true">Opinion</button>
          <button type="button" data-view="table" aria-pressed="false">Table</button>
          <button type="button" data-view="json" aria-pressed="false">JSON</button>
        </div>
      </div>
      <div id="result-body" class="mill-result-body" aria-live="polite">
        <div class="mill-empty">
          <p>Nothing has run yet.</p>
          <p class="mill-hint">Press Run (<kbd class="mill-mod">Ctrl</kbd>+<kbd>Enter</kbd>) or pick a sample.</p>
        </div>
      </div>
    </section>
  </section>
  <section class="mill-history" aria-labelledby="history-title">
    <h2 id="history-title" class="label">History</h2>
    <ol id="history" class="mill-list"><li class="mill-none">Nothing run yet</li></ol>
  </section>
</main>
</body>
</html>
```

Create `web/mill.css`:

```css
/* The localhost mill. Layered on /assets/fidryn.css, which supplies the
   tokens, fonts, buttons, stamps, code frames, and tables; every class here
   starts with `mill-`. Phone below 720px, tablet 720-1199px, desktop 1200px+. */

/* ---------- header ---------- */
.mill-head { display: flex; flex-wrap: wrap; align-items: center; gap: 8px 16px; }
.mill-tag { font: 600 12px/1 var(--sans); color: var(--rubric); }
.mill-health { display: inline-flex; align-items: center; gap: 8px; margin: 0; font: 12px/1.4 var(--mono); color: var(--ink-2); }
.mill-dot { flex: none; width: 8px; height: 8px; border-radius: 50%; background: var(--ink-3); }
.mill-health[data-state="ok"] .mill-dot { background: var(--det); }
.mill-health[data-state="down"] .mill-dot { background: var(--inc); }
.mill-health-text::before { content: "\00b7  "; color: var(--ink-3); }
.mill-health[data-state="down"] .mill-health-text { color: var(--inc); }
.mill-nofile { margin: 0 0 0 auto; padding: 4px 8px; border: 1px solid var(--rule); border-radius: var(--r); font: 600 12px/1.3 var(--sans); color: var(--ink-3); }
.mill-guide { font: 600 12px/1.3 var(--sans); color: var(--ink); text-decoration: underline; text-decoration-color: var(--rubric); text-underline-offset: 3px; }
.mill-noscript { margin: 0; padding: 12px 14px; border: 1px solid var(--inc); border-radius: var(--r); color: var(--ink); }

/* ---------- page grid ---------- */
.mill-page {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  grid-template-areas: "samples" "editor" "side" "history";
  gap: 20px;
  align-items: start;
  padding-block: 16px 48px;
  padding-inline: max(var(--gutter), env(safe-area-inset-left)) max(var(--gutter), env(safe-area-inset-right));
}
.mill-samples { grid-area: samples; min-width: 0; }
.mill-editor { grid-area: editor; min-width: 0; }
.mill-side { grid-area: side; min-width: 0; display: grid; gap: 14px; align-content: start; }
.mill-history { grid-area: history; min-width: 0; }
.mill-samples > .label, .mill-history > .label { margin: 0 0 8px; }

/* ---------- samples and history ---------- */
.mill-list { list-style: none; margin: 0; padding: 0; }
.mill-samples .mill-list { display: flex; gap: 6px; overflow-x: auto; padding-bottom: 4px; scroll-snap-type: x proximity; }
.mill-sample { display: inline-flex; align-items: center; gap: 8px; min-height: 36px; padding: 0 12px; border: 1px solid var(--ink); border-radius: var(--r); background: transparent; color: var(--ink); font: 600 12px/1.2 var(--sans); white-space: nowrap; cursor: pointer; scroll-snap-align: start; }
.mill-sample:hover { background: var(--paper-2); }
.mill-sample[aria-current="true"] { background: var(--ink); color: var(--paper); }
.mill-sample[aria-current="true"] .stamp { color: var(--paper); }
.mill-list .stamp { font-size: 9px; padding: 3px 5px; }
.mill-history .mill-list { border-top: 1px solid var(--ink); }
.mill-hist { display: grid; grid-template-columns: minmax(0, 1fr) auto; grid-template-areas: "what stamp" "time stamp"; align-items: center; gap: 0 10px; width: 100%; min-height: 36px; padding: 6px 0; border: 0; border-bottom: 1px solid var(--rule); background: none; color: var(--ink-2); font: 14px/1.35 var(--serif); text-align: left; cursor: pointer; }
.mill-hist:hover { color: var(--ink); }
.mill-hist[aria-current="true"] { color: var(--rubric); }
.mill-hist .stamp { grid-area: stamp; }
.mill-hist-time { grid-area: time; font: 12px/1.3 var(--mono); color: var(--ink-3); }
.mill-hist-what { grid-area: what; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mill-none { padding: 8px 0; border-bottom: 1px solid var(--rule); color: var(--ink-3); font-size: 14px; }
.mill-confirm { margin-top: 10px; padding: 10px 12px; border: 1px solid var(--rubric); border-radius: var(--r); background: color-mix(in srgb, var(--rubric) 6%, var(--paper)); }
.mill-confirm p { margin: 0 0 8px; color: var(--ink); font-size: 14px; }

/* ---------- segmented controls ---------- */
.mill-seg { display: inline-flex; }
.mill-seg button { min-height: 32px; padding: 0 14px; border: 1px solid var(--ink); background: transparent; color: var(--ink-2); font: 600 12px/1 var(--sans); cursor: pointer; }
.mill-seg button + button { border-left: 0; }
.mill-seg button:first-child { border-radius: var(--r) 0 0 var(--r); }
.mill-seg button:last-child { border-radius: 0 var(--r) var(--r) 0; }
.mill-seg button[aria-selected="true"], .mill-seg button[aria-pressed="true"] { background: var(--ink); color: var(--paper); }
.mill-seg button:disabled { color: var(--ink-3); border-color: var(--rule); background: transparent; cursor: default; }

/* ---------- editor ---------- */
.mill-editor-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 12px; margin-bottom: 10px; }
.mill-status { margin: 0; font: 600 12px/1.3 var(--sans); color: var(--ink-3); }
.mill-status[data-state="ok"] .mill-status-main { color: var(--det); }
.mill-status[data-state="err"] .mill-status-main { color: var(--inc); }
.mill-status-main:not(:empty) + .mill-status-hint::before { content: " \00b7  "; }
.mill-ed { --mill-lh: 22px; position: relative; display: grid; grid-template-columns: auto minmax(0, 1fr); border-top: 1px solid var(--ink); border-bottom: 1px solid var(--rule); background: var(--paper); }
.mill-ed:focus-within { outline: 2px solid var(--rubric); outline-offset: 2px; }
/* The line numbers are positioned out of flow so a long file cannot stretch the editor row. */
.mill-gutter { position: relative; overflow: hidden; width: calc(4ch + 22px); background: var(--paper-2); border-right: 1px solid var(--rule); color: var(--ink-3); font: 13px/var(--mill-lh) var(--mono); text-align: right; user-select: none; }
.mill-gutter:empty { display: none; }
.mill-gutter-in { position: absolute; top: 0; left: 0; right: 0; padding-block: 12px; }
.mill-ln { height: var(--mill-lh); padding: 0 10px 0 4px; white-space: nowrap; }
.mill-ln.has-diag { color: var(--inc); }
.mill-ln.has-diag::before { content: "\25CF"; margin-right: 4px; font-size: 9px; vertical-align: 1px; }
.mill-code { grid-column: 2; position: relative; overflow: hidden; min-width: 0; }
.mill-hl, .mill-input { box-sizing: border-box; margin: 0; padding: 12px 14px; border: 0; border-radius: 0; font: 13px/var(--mill-lh) var(--mono); font-variant-ligatures: none; letter-spacing: 0; word-spacing: 0; tab-size: 4; white-space: pre; overflow-wrap: normal; word-break: normal; }
.mill-hl { position: absolute; top: 0; left: 0; min-width: 100%; color: var(--ink); pointer-events: none; }
.mill-input { position: relative; display: block; width: 100%; height: clamp(320px, 60vh, 760px); resize: vertical; overflow: auto; background: transparent; color: var(--ink); caret-color: var(--ink); outline: none; }
.mill-ed:not([data-hl]) .mill-hl { display: none; }
.mill-ed[data-hl] .mill-input { color: transparent; -webkit-text-fill-color: transparent; }
.mill-input::selection { background: color-mix(in srgb, var(--con) 26%, transparent); }
.mill-sq { text-decoration: underline wavy var(--inc); text-decoration-skip-ink: none; text-underline-offset: 3px; }
.mill-pop { position: absolute; left: 10px; right: 10px; z-index: 2; padding: 6px 10px; border: 1px solid var(--inc); border-left-width: 3px; border-radius: var(--r); background: color-mix(in srgb, var(--inc) 6%, var(--paper)); box-shadow: 0 8px 18px -10px rgba(28, 24, 19, .45); color: var(--ink); font: 13px/1.4 var(--sans); pointer-events: none; }
.mill-pop p { margin: 0; }
.mill-pop p + p { margin-top: 4px; }
.mill-pop code { margin-right: 6px; font: 500 12px var(--mono); color: var(--inc); }

/* ---------- query, clocks, actions ---------- */
.mill-params { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: 10px 12px; }
.mill-field-query { grid-column: 1 / -1; }
.mill-field label { display: block; margin-bottom: 4px; }
.mill-field input { box-sizing: border-box; width: 100%; min-height: 36px; padding: 6px 10px; border: 1px solid var(--ink); border-radius: var(--r); background: var(--paper); color: var(--ink); font: 13px/1.4 var(--mono); }
.mill-field input[aria-invalid="true"] { border-color: var(--inc); box-shadow: inset 0 0 0 1px var(--inc); }
.mill-field-err { margin: 4px 0 0; font: 600 12px/1.3 var(--sans); color: var(--inc); }
.mill-actions { display: flex; flex-wrap: wrap; gap: 8px; margin: 0; }
.mill-actions .btn[aria-disabled="true"] { opacity: .5; cursor: default; }
.mill-keys { margin: -4px 0 0; font: 12px/1.4 var(--sans); color: var(--ink-3); }
.mill-keys kbd, .mill-hint kbd { padding: 0 4px; border: 1px solid var(--rule); border-radius: 3px; background: var(--paper-2); font: 11px/1.5 var(--mono); color: var(--ink-2); }

/* ---------- result ---------- */
.mill-result { display: grid; gap: 10px; min-width: 0; }
.mill-result-head { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 12px; }
.mill-result-head .label { margin: 0; }
.mill-result-body { min-width: 0; }
.mill-result-body[aria-busy="true"] > * { opacity: .55; }
.mill-empty { padding: 28px 20px; border: 1px dashed var(--rule); border-radius: var(--r); color: var(--ink-3); text-align: center; }
.mill-empty p { margin: 0; font-size: 15px; }
.mill-empty .mill-hint { margin-top: 6px; font: 13px/1.5 var(--sans); }
.mill-busy { margin: 0; padding: 20px 0; font: 600 12px/1.3 var(--sans); color: var(--ink-3); }
.mill-from { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 10px; padding: 8px 10px; border: 1px solid var(--rule); border-radius: var(--r); background: var(--paper-2); }
.mill-from .label { margin: 0; }

/* The opinion view and rendered text: a printed page. */
.mill-doc { padding: 22px 26px; border: 1px solid var(--rule); background: var(--paper); box-shadow: 0 1px 2px rgba(28, 24, 19, .08), 0 16px 30px -14px rgba(28, 24, 19, .28); color: var(--ink-2); font-size: 15px; }
.mill-doc p { margin: 0 0 10px; }
.mill-doc .mill-doc-cap { margin: 0 0 14px; padding-bottom: 8px; border-bottom: 3px double var(--ink); font: 600 12px/1.3 var(--sans); color: var(--ink-3); text-align: center; }
.mill-doc-title { display: flex; align-items: center; gap: 10px; margin: 0 0 10px; font: 400 24px/1.15 var(--serif); font-variation-settings: "opsz" 48; color: var(--ink); }
.mill-mark { flex: none; width: 10px; height: 10px; background: var(--ink-3); }
.mill-mark.det { background: var(--det); }
.mill-mark.con { background: var(--con); }
.mill-mark.sus { background: var(--sus); }
.mill-mark.nc { background: var(--nc); }
.mill-mark.oc { background: var(--oc); }
.mill-mark.inc { background: var(--inc); }
.mill-doc .mill-doc-boundary { font-size: 13.5px; color: var(--ink-3); }
.mill-doc-sig { display: flex; flex-wrap: wrap; justify-content: space-between; gap: 4px 16px; margin-top: 14px; padding-top: 8px; border-top: 1px solid var(--rule); font: 12px/1.4 var(--mono); color: var(--ink-3); }
.mill-rendered-text { margin: 0; font: 13px/1.6 var(--mono); color: var(--ink); white-space: pre-wrap; overflow-wrap: anywhere; }

/* The table view. */
.mill-table { width: 100%; table-layout: fixed; }
.mill-table th { width: 9.5rem; text-align: left; vertical-align: top; font: 600 12px/1.4 var(--sans); color: var(--ink-3); }
.mill-table td { font: 12.5px/1.5 var(--mono); color: var(--ink); overflow-wrap: break-word; }
.mill-copy { display: inline-flex; flex-wrap: wrap; align-items: center; gap: 8px; }

/* Check results, errors. */
.mill-ok { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 12px 14px; border: 1px solid var(--rule); border-radius: var(--r); }
.mill-ok p { margin: 0; }
.mill-res-head { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; margin: 0 0 8px; }
.mill-res-head .label { margin: 0; }
.mill-diag-list { list-style: none; margin: 0; padding: 0; border-top: 1px solid var(--ink); }
.mill-diag-list li { display: grid; grid-template-columns: auto minmax(0, 1fr) auto auto; align-items: center; gap: 4px 10px; padding: 8px 0; border-bottom: 1px solid var(--rule); }
.mill-diag-code { font: 500 12px/1.4 var(--mono); color: var(--inc); }
.mill-diag-msg { color: var(--ink); font-size: 14px; overflow-wrap: anywhere; }
.mill-diag-at { font: 12px/1.4 var(--mono); color: var(--ink-3); }
.mill-diag-hint { grid-column: 2 / -1; color: var(--ink-3); font-size: 13px; }
.mill-error { padding: 12px 14px; border: 1px solid var(--inc); border-left-width: 3px; border-radius: var(--r); background: color-mix(in srgb, var(--inc) 5%, var(--paper)); }
.mill-error-title { margin: 0 0 4px; font: 600 13px/1.3 var(--sans); color: var(--inc); }
.mill-error-msg { margin: 0; color: var(--ink); font-size: 14.5px; white-space: pre-wrap; overflow-wrap: anywhere; }
.mill-error .actions { margin-top: 10px; }

/* ---------- tablet ---------- */
@media (min-width: 720px) and (max-width: 1199px) {
  .mill-samples .mill-list { flex-wrap: wrap; overflow: visible; }
}

/* ---------- desktop: rail, editor, query and result ---------- */
@media (min-width: 1200px) {
  .mill-page {
    grid-template-columns: 210px minmax(0, 1.15fr) minmax(0, 1fr);
    grid-template-rows: auto 1fr;
    grid-template-areas: "samples editor side" "history editor side";
    column-gap: 22px;
  }
  .mill-history { margin-top: 4px; }
  .mill-samples .mill-list { display: block; overflow: visible; padding: 0; border-top: 1px solid var(--ink); }
  .mill-sample { display: flex; width: 100%; min-height: 36px; padding: 6px 0; border: 0; border-bottom: 1px solid var(--rule); border-radius: 0; background: none; color: var(--ink-2); font: 400 14px/1.35 var(--serif); text-align: left; white-space: normal; }
  .mill-sample .stamp { margin-left: auto; }
  .mill-sample:hover { background: none; color: var(--ink); }
  .mill-sample[aria-current="true"] { padding-left: 10px; background: none; color: var(--rubric); box-shadow: inset 3px 0 0 var(--rubric); }
  .mill-sample[aria-current="true"] .stamp { color: inherit; }
  .mill-sample[aria-current="true"] .stamp.det { color: var(--det); }
  .mill-sample[aria-current="true"] .stamp.con { color: var(--con); }
  .mill-sample[aria-current="true"] .stamp.sus { color: var(--sus); }
  .mill-input { height: clamp(360px, calc(100vh - 230px), 900px); }
}

/* ---------- phone: one column and a sticky action bar ---------- */
@media (max-width: 719px) {
  .mill-head { gap: 6px 12px; }
  .mill-host { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
  .mill-head .theme-toggle { order: 4; margin-left: auto; }
  .mill-nofile { order: 5; margin-left: 0; }
  .mill-guide { order: 6; }
  .mill-page { gap: 16px; padding-bottom: calc(96px + env(safe-area-inset-bottom)); }
  .mill-samples .stamp { display: none; }
  .mill-sample, .mill-hist, .mill-none { min-height: 44px; }
  .mill-editor-head { display: grid; justify-content: stretch; }
  .mill-seg { display: flex; width: 100%; }
  .mill-seg button { flex: 1; min-height: 44px; padding: 0 8px; }
  .mill-ed { --mill-lh: 24px; }
  .mill-gutter, .mill-hl, .mill-input { font-size: 16px; }
  .mill-input { height: 52vh; }
  .mill-params { grid-template-columns: minmax(0, 1fr); }
  .mill-field input { min-height: 44px; font-size: 16px; }
  .mill-keys, .mill-status-hint { display: none; }
  .mill-actions {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 30;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 6px;
    padding: 10px max(var(--gutter), env(safe-area-inset-right)) calc(10px + env(safe-area-inset-bottom)) max(var(--gutter), env(safe-area-inset-left));
    border-top: 1px solid var(--ink);
    background: var(--paper);
  }
  .mill-actions .btn { min-height: 44px; padding: 0 4px; }
  .mill-result-body .btn, .mill-confirm .btn, .mill-from .btn, .mill-result .mill-seg button { min-height: 44px; }
  .mill-doc { padding: 16px; }
  .mill-diag-list li { grid-template-columns: auto minmax(0, 1fr); }
  .mill-diag-list .btn { justify-self: start; }
  .mill-diag-hint { grid-column: 1 / -1; }
}
```

In `crates/fidryn-cli/src/ui.rs`, embed the stylesheet.

Find:

```rust
const FAVICON: &str = include_str!("../../../site/favicon.svg");
```

Replace it with:

```rust
const FAVICON: &str = include_str!("../../../site/favicon.svg");
const MILL_CSS: &str = include_str!("../../../web/mill.css");
```

Add its handler after Task 13's `site_css`.

Find:

```rust
async fn site_css() -> Response {
    static_text("text/css; charset=utf-8", SITE_CSS)
}
```

Replace it with:

```rust
async fn site_css() -> Response {
    static_text("text/css; charset=utf-8", SITE_CSS)
}

async fn mill_css() -> Response {
    static_text("text/css; charset=utf-8", MILL_CSS)
}
```

Route it right after the shared stylesheet in `router()`.

Find:

```rust
        .route("/assets/fidryn.css", get(site_css))
```

Replace it with:

```rust
        .route("/assets/fidryn.css", get(site_css))
        .route("/assets/mill.css", get(mill_css))
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xtask --offline --test mill_page`
Expected: PASS (`test result: ok. 6 passed; 0 failed`).

Run: `cargo test -p fidryn-cli --offline --lib ui::`
Expected: PASS; every `ui::tests::…` test passes, including the unchanged `index_returns_html_mill` (the page keeps `fidryn mill`, `localhost`, `127.0.0.1`, `Live filing is not available`, `>Run<`, `>Explore<`, `>Render<`) and the new `mill_css_route_serves_the_embedded_stylesheet`.

Run: `cargo fmt -p fidryn-cli -p xtask`
Expected: no output (the new Rust code is already formatted).

- [ ] **Step 5: Look at the page**

Run `cargo run -p fidryn-cli --offline -- ui --no-open` and open `http://127.0.0.1:8751`. `web/mill.js` arrives in Task 15, so the browser console shows one 404 for `/assets/mill.js`; that is expected now. There must be no Content-Security-Policy violation in the console. At a 1440px-wide window check:
  - the header: the F monogram with its rubric offset shadow, `Fidryn`, `mill` in rubric, a grey dot with `localhost 127.0.0.1 · checking`, the bordered notice `Live filing is not available from the UI` pushed to the right, `Mill guide` underlined in rubric, and a double rule under the header; the theme toggle stays hidden without JavaScript;
  - three columns: `Samples` and `History` in a 210px rail (`Nothing run yet` under History), the editor (the Module / Case JSON / Template switch with Module filled in ink, then an empty textarea under a single ink rule, no line-number column), and on the right `Query` at full width, `validAt` and `knownAt` side by side, `Check`, `Run` (filled), `Explore`, `Render`, the keyboard hint line, and `Result` with the Opinion / Table / JSON switch over the dashed `Nothing has run yet.` box;
  - at 820px (DevTools device toolbar): one column in the order samples, editor, parameters, buttons, result, history;
  - at 390px: the notice and `Mill guide` wrap to a second header row, `localhost 127.0.0.1` is visually hidden (the dot and the status word stay), the four buttons form a bar fixed to the bottom of the viewport with 44px targets, and the page never scrolls sideways.

Headless Chrome on macOS lays pages out at least 500px wide whatever `--window-size` says, so check 390px with the device toolbar (or CDP device metrics), not with `--window-size=390,…`.

- [ ] **Step 6: Commit**

```bash
git add web/index.html web/mill.css xtask/tests/mill_page.rs crates/fidryn-cli/src/ui.rs
git -c commit.gpgsign=false commit -m "feat(mill): statute mill page and styles"
```

### Task 15: Mill app

**Files:**
- Create: `web/mill.js`
- Create: `web/tests/mill.test.js`
- Modify: `crates/fidryn-cli/src/ui.rs` (`const MILL_JS` after `const MILL_CSS`; `async fn mill_js` after `async fn mill_css`; the `/assets/mill.js` route after the `/assets/mill.css` route; one router test before `mill_binds_loopback_only`)
- Test: `web/tests/mill.test.js`; `ui::tests::mill_js_route_serves_the_embedded_script` in `crates/fidryn-cli/src/ui.rs`

**Interfaces:**
- Consumes: Task 14's page (the C9 ids, `#confirm-text`, `.mill-health-text`, `.mill-mod`) and its `mill_css` handler and route line; the C3 API: `GET /api/health` answers `ok`; `GET /api/samples` answers `[{id, title, blurb, source, case, query, validAt, knownAt, action, expect}]`; `POST /api/check` `{source}`; `POST /api/run` and `/api/explore` `{source, query, case, validAt, knownAt}` whose success carries `opinion: string[]` beside `ok` and `report`; `POST /api/render` `{source, template}`; the error shapes (`check failed` with diagnostics, `invalid case: …` / `validAt: …` / `knownAt: …`, `{kind: "engineError", error, message}`).
- Produces, in `web/mill.js` (one IIFE, `"use strict"`):
  - exported when `document` is undefined (C8): `esc(text) -> string`; `isRfc3339(text) -> boolean`; `valueText(value) -> string`; `requestText(request) -> string` (both follow the C2 wording); `loadState(storage, fallback) -> state` where `storage` has `getItem` and `fallback` is the first sample's state; `postJson(path, body, fetchFn?) -> Promise<{network: boolean, status: number, data: object | array | null, text: string}>`, which never rejects.
  - state in `localStorage` key `fidryn-mill`: `{v: 1, module, case, template, query, validAt, knownAt, buffer, sample, view}`; theme in `fidryn-theme`, applied as soon as the script runs in `<head>`.
  - private helpers Task 16 builds on: `byteToIndex(text, offset)`, `lineCol(text, index) -> {line, col}` (Task 16 exports and tests them), `parseCase(text) -> {ok: true, value} | {ok: false, index, line, col, message}`, `JSON_NUMBER`, `str`, `isObject`, `h(tag, props, kids)`, `clear(node)`, `el` (elements by camel-cased id: `el.editorWrap`, `el.bufferStatus`, `el.diagPop`, `el.queryNames`, …), `state`, `diagnostics = {source, list, version}`, `noteDiagnostics(source, data)`, `serverAnswered()`, `networkDown()`, `codeFrame(lang, source)`.
  - hooks Task 16 replaces: `editorChanged(reason)`, called with `"load"`, `"edit"`, `"buffer"`, `"caret"`, or `"diagnostics"`, and `updateBufferStatus()`; `start()` calls `wireEvents();` on a line of its own, after which Task 16 adds `installEditor();`.
  - in `ui.rs`: `const MILL_JS: &str`, `async fn mill_js() -> Response`, and `GET /assets/mill.js` answering `text/javascript; charset=utf-8` with `cache-control: no-cache`.

What the page does after this task (the editor is still a plain textarea holding one of three buffers): the header dot turns green or red from `GET /api/health`, checked on load and polled every 3 seconds after a failure until the server answers; samples load from `/api/samples` with the stamp of their `expect`; the first visit (or unreadable saved state) opens the first sample, `require-gate`, with query `q` and nothing run; picking a sample loads it and performs its `action` (Run), but over edited inputs it first asks inline, Replace or Cancel; both clocks are checked as RFC 3339 while typing; Run and Explore parse the case JSON first and report line and column when it is not JSON; Ctrl or Cmd+Enter runs and Ctrl or Cmd+Shift+Enter explores; a result shows as Opinion (caption `Evaluation report · {executionMode} · {sourceTrust}`, the stamp label as title, the server's `opinion` sentences, a footer with the module and `asOf.validTime`), Table (every field, trace with Copy), or JSON (pretty report with Copy), and the choice is remembered; Check shows `No diagnostics.` or a list of code, message, `line:column`, and Jump; Render shows the text as a paper document; engine errors, invalid input, and a stopped server (`Is fidryn ui still running? The mill could not reach 127.0.0.1.`) show inline; History keeps the last 20 actions, reopens one on click, and offers Restore inputs.

- [ ] **Step 1: Write the failing tests**

Create `web/tests/mill.test.js` (test names and assertion messages stay ASCII: Node 18's test runner stops with `ERR_TAP_LEXER_ERROR` when a reported name contains a character such as `§`):

```js
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
```

In `crates/fidryn-cli/src/ui.rs`, inside `mod tests`, add the router test for the script.

Find:

```rust
    #[tokio::test]
    async fn mill_binds_loopback_only() {
```

Replace it with:

```rust
    #[tokio::test]
    async fn mill_js_route_serves_the_embedded_script() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/assets/mill.js")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-cache");
        let js = body_text(response).await;
        assert!(js.contains("\"use strict\""), "{js}");
        assert!(js.contains("/api/samples"), "mill.js loads the samples");
    }

    #[tokio::test]
    async fn mill_binds_loopback_only() {
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `node --test web/tests/mill.test.js`
Expected: FAIL with `Error: Cannot find module '../mill.js'` and `# fail 1`.

Run: `cargo test -p fidryn-cli --offline --lib mill_js_route`
Expected: FAIL in `mill_js_route_serves_the_embedded_script` with `left: 404` / `right: 200`.

- [ ] **Step 3: Write the app and its route**

Create `web/mill.js`:

```js
// The Fidryn mill page: check, run, explore, and render a module through the
// fidryn ui server on 127.0.0.1. Pure helpers come first so `node --test` can
// load this file without a DOM; the page code after them runs in a browser.
(function () {
  "use strict";

  var STATE_KEY = "fidryn-mill";
  var THEME_KEY = "fidryn-theme";
  var BUFFERS = ["module", "case", "template"];
  var VIEWS = ["opinion", "table", "json"];
  var TEXT_FIELDS = ["module", "case", "template", "query", "validAt", "knownAt"];

  // ------------------------------------------------------------ pure helpers

  /** Escape text for HTML element content and quoted attributes. */
  function esc(text) {
    return String(text)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  var RFC3339 = /^(\d{4})-(\d{2})-(\d{2})[Tt ](\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:[Zz]|[+-](\d{2}):(\d{2}))$/;
  var MONTH_DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

  /**
   * True when `text` is an RFC 3339 date-time with a zone (`Z` or `+hh:mm`),
   * the form the server accepts for validAt and knownAt.
   */
  function isRfc3339(text) {
    var m = typeof text === "string" ? RFC3339.exec(text) : null;
    if (!m) return false;
    var year = Number(m[1]);
    var month = Number(m[2]);
    var day = Number(m[3]);
    var leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
    var days = month === 2 && leap ? 29 : MONTH_DAYS[month - 1];
    if (!days || day < 1 || day > days) return false;
    if (Number(m[4]) > 23 || Number(m[5]) > 59 || Number(m[6]) > 60) return false;
    return m[7] === undefined || (Number(m[7]) <= 23 && Number(m[8]) <= 59);
  }

  function isObject(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value);
  }

  /** Compact JSON: the text for any shape the templates below do not know. */
  function compact(value) {
    var text = JSON.stringify(value);
    return text === undefined ? String(value) : text;
  }

  /** A string as itself, anything else as compact JSON. */
  function str(value) {
    return typeof value === "string" ? value : compact(value);
  }

  /**
   * Plain text for a tagged runtime value (`{"kind", "data"}`), written the
   * way the server's opinion sentences write it.
   */
  function valueText(value) {
    if (!isObject(value) || typeof value.kind !== "string") return compact(value);
    var data = value.data;
    switch (value.kind) {
      case "int":
      case "decimal":
      case "bool":
      case "instant":
      case "duration":
        return str(data);
      case "string":
        return typeof data === "string" ? "\"" + data + "\"" : compact(value);
      case "entity":
        return typeof data === "string" ? data : compact(value);
      case "unit":
        return "unit";
      case "ctor":
        return ctorText(value);
      case "set":
        return Array.isArray(data) ? "{" + data.map(valueText).join(", ") + "}" : compact(value);
      case "map":
        if (!isObject(data)) return compact(value);
        return "{" + Object.keys(data).sort().map(function (key) {
          return key + ": " + valueText(data[key]);
        }).join(", ") + "}";
      case "option":
        return data === null || data === undefined ? "none" : valueText(data);
      default:
        return compact(value);
    }
  }

  function ctorText(value) {
    var data = value.data;
    if (!isObject(data) || typeof data.name !== "string") return compact(value);
    var fields = isObject(data.fields) ? data.fields : {};
    var keys = Object.keys(fields).sort();
    if (keys.length === 0) return data.name;
    var positional = keys.every(function (key) { return key.charAt(0) === "_"; });
    return data.name + "(" + keys.map(function (key) {
      return (positional ? "" : key + ": ") + valueText(fields[key]);
    }).join(", ") + ")";
  }

  /** `list` when it is a non-empty array of strings, otherwise null. */
  function stringList(list) {
    var ok = Array.isArray(list) && list.length > 0 &&
      list.every(function (item) { return typeof item === "string"; });
    return ok ? list : null;
  }

  function doctrineNames(list) {
    if (!Array.isArray(list)) return [];
    return list.map(function (item) {
      if (typeof item === "string") return item;
      return isObject(item) && typeof item.name === "string" ? item.name : null;
    }).filter(function (name) { return name !== null; });
  }

  /**
   * Plain text for one request of a suspended outcome, written the way the
   * server's opinion sentences write it.
   */
  function requestText(request) {
    if (!isObject(request) || typeof request.kind !== "string") return compact(request);
    var among;
    switch (request.kind) {
      case "needCustom":
        return typeof request.payload === "string"
          ? request.payload + " (" + str(request.effect) + ")"
          : str(request.effect);
      case "needEvidence":
        return "evidence matching " + str(request.schema);
      case "needInterpretation":
        return "an interpretation of " + str(request.family) +
          (typeof request.source === "string" && request.source !== "" ? " under " + request.source : "");
      case "needJudgment":
        return "a determination under " + str(request.protocol);
      case "needChoice":
        among = stringList(request.options);
        return "a decision under " + str(request.protocol) + (among ? " among " + among.join(", ") : "");
      case "needApplicableLaw":
        among = stringList(request.candidates);
        return "applicable law" + (among ? " among " + among.join(", ") : "");
      case "needConflict":
        among = doctrineNames(request.doctrines);
        return "one applicable conflict doctrine" + (among.length ? " among " + among.join(", ") : "");
      default:
        return request.kind;
    }
  }

  /** A complete mill state from any object, filling gaps with defaults. */
  function copyState(source) {
    var from = isObject(source) ? source : {};
    var out = { v: 1 };
    TEXT_FIELDS.forEach(function (key) {
      out[key] = typeof from[key] === "string" ? from[key] : "";
    });
    out.buffer = BUFFERS.indexOf(from.buffer) >= 0 ? from.buffer : "module";
    out.sample = typeof from.sample === "string" ? from.sample : null;
    out.view = VIEWS.indexOf(from.view) >= 0 ? from.view : "opinion";
    return out;
  }

  /**
   * The saved mill state from `storage` (localStorage or a stand-in), or a
   * copy of `fallback` when nothing usable is saved: storage that is missing
   * or throws, text that is not JSON, another version, or a text field that
   * is missing or not a string. An unknown buffer or view is reset alone.
   */
  function loadState(storage, fallback) {
    var base = copyState(fallback);
    var saved;
    try {
      saved = JSON.parse(storage.getItem(STATE_KEY));
    } catch (err) {
      return base;
    }
    if (!isObject(saved) || saved.v !== 1) return base;
    for (var i = 0; i < TEXT_FIELDS.length; i++) {
      if (typeof saved[TEXT_FIELDS[i]] !== "string") return base;
    }
    TEXT_FIELDS.forEach(function (key) { base[key] = saved[key]; });
    base.buffer = BUFFERS.indexOf(saved.buffer) >= 0 ? saved.buffer : "module";
    base.sample = typeof saved.sample === "string" ? saved.sample : null;
    base.view = VIEWS.indexOf(saved.view) >= 0 ? saved.view : "opinion";
    return base;
  }

  function parseJson(text) {
    try {
      var value = JSON.parse(text);
      return value !== null && typeof value === "object" ? value : null;
    } catch (err) {
      return null;
    }
  }

  /**
   * Fetch `path` and read the body. Never rejects: a failed request resolves
   * to `{ network: true, status: 0, data: null, text: "" }`; any response
   * resolves to `{ network: false, status, data, text }`, where `data` is the
   * parsed JSON object or array (null when the body is not JSON).
   */
  function send(path, init, fetchFn) {
    return new Promise(function (resolve) {
      resolve((fetchFn || fetch)(path, init));
    }).then(function (response) {
      return Promise.resolve(response.text()).then(function (body) {
        return { network: false, status: response.status, data: parseJson(body), text: String(body) };
      });
    }).catch(function () {
      return { network: true, status: 0, data: null, text: "" };
    });
  }

  /**
   * POST `body` as JSON. Resolves like `send`: HTTP errors such as a 400 with
   * a JSON body come back as data, and a server that is gone comes back as
   * `network: true` instead of a rejected promise.
   */
  function postJson(path, body, fetchFn) {
    return send(path, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body)
    }, fetchFn);
  }

  /** UTF-8 byte length of the code point `cp`. */
  function utf8Length(cp) {
    return cp < 0x80 ? 1 : cp < 0x800 ? 2 : cp < 0x10000 ? 3 : 4;
  }

  /**
   * The string index of UTF-8 byte offset `offset` in `text` (diagnostic
   * spans are byte offsets). An offset inside a character gives that
   * character's start; offsets past the end give `text.length`.
   */
  function byteToIndex(text, offset) {
    var bytes = 0;
    var i = 0;
    while (i < text.length) {
      var cp = text.codePointAt(i);
      var size = utf8Length(cp);
      if (bytes + size > offset) return i;
      bytes += size;
      i += cp > 0xffff ? 2 : 1;
    }
    return text.length;
  }

  /** 1-based line and column of string index `index`; columns count characters. */
  function lineCol(text, index) {
    var end = Math.max(0, Math.min(index, text.length));
    var line = 1;
    var col = 1;
    var i = 0;
    while (i < end) {
      var cp = text.codePointAt(i);
      if (cp === 10) {
        line += 1;
        col = 1;
      } else {
        col += 1;
      }
      i += cp > 0xffff ? 2 : 1;
    }
    return { line: line, col: col };
  }

  var JSON_NUMBER = /-?(?:0|[1-9]\d*)(?:\.\d+)?(?:[eE][+-]?\d+)?/y;

  /**
   * Where and why `text` is not JSON, as `{ index, message }`, or null when it
   * is. Browsers word and place JSON.parse errors differently, so the mill
   * finds the first error itself.
   */
  function jsonError(text) {
    var i = 0;
    function fail(message) {
      throw { jsonIndex: i, message: message };
    }
    function space() {
      while (i < text.length && " \t\n\r".indexOf(text.charAt(i)) >= 0) i += 1;
    }
    function word(w) {
      if (text.slice(i, i + w.length) !== w) return false;
      i += w.length;
      return true;
    }
    function string() {
      i += 1;
      while (i < text.length) {
        var c = text.charAt(i);
        if (c === "\"") {
          i += 1;
          return;
        }
        if (c === "\\") {
          var e = text.charAt(i + 1);
          if (e === "u" && /^[0-9a-fA-F]{4}$/.test(text.slice(i + 2, i + 6))) {
            i += 6;
            continue;
          }
          if (e === "" || "\"\\/bfnrt".indexOf(e) < 0) fail("Invalid escape in a string");
          i += 2;
          continue;
        }
        if (c < " ") fail("Line break or control character in a string");
        i += 1;
      }
      fail("Unterminated string");
    }
    function value() {
      space();
      if (i >= text.length) fail("Unexpected end of JSON");
      var c = text.charAt(i);
      if (c === "{") {
        i += 1;
        space();
        if (word("}")) return;
        for (;;) {
          space();
          if (text.charAt(i) !== "\"") fail("Expected a property name in double quotes");
          string();
          space();
          if (!word(":")) fail("Expected : after the property name");
          value();
          space();
          if (word(",")) continue;
          if (word("}")) return;
          fail("Expected , or } after the value");
        }
      }
      if (c === "[") {
        i += 1;
        space();
        if (word("]")) return;
        for (;;) {
          value();
          space();
          if (word(",")) continue;
          if (word("]")) return;
          fail("Expected , or ] after the value");
        }
      }
      if (c === "\"") return string();
      if (c === "-" || (c >= "0" && c <= "9")) {
        JSON_NUMBER.lastIndex = i;
        var m = JSON_NUMBER.exec(text);
        if (!m) fail("Invalid number");
        i += m[0].length;
        return;
      }
      if (word("true") || word("false") || word("null")) return;
      fail("Unexpected " + JSON.stringify(String.fromCodePoint(text.codePointAt(i))));
    }
    try {
      value();
      space();
      if (i < text.length) fail("Unexpected text after the JSON value");
      return null;
    } catch (err) {
      if (err && typeof err.jsonIndex === "number") return { index: err.jsonIndex, message: err.message };
      throw err;
    }
  }

  /** The case JSON to send, or where it fails to parse. Blank text sends `{}`. */
  function parseCase(text) {
    if (text.trim() === "") return { ok: true, value: {} };
    try {
      return { ok: true, value: JSON.parse(text) };
    } catch (err) {
      var found = jsonError(text) || { index: text.length, message: String(err && err.message) };
      var at = lineCol(text, found.index);
      return { ok: false, index: found.index, line: at.line, col: at.col, message: found.message };
    }
  }

  if (typeof document === "undefined") {
    module.exports = {
      esc: esc,
      isRfc3339: isRfc3339,
      valueText: valueText,
      requestText: requestText,
      loadState: loadState,
      postJson: postJson
    };
    return;
  }

  // ------------------------------------------------ theme, before first paint

  function savedTheme() {
    try {
      var theme = window.localStorage.getItem(THEME_KEY);
      return theme === "light" || theme === "dark" ? theme : "";
    } catch (err) {
      return "";
    }
  }

  var initialTheme = savedTheme();
  document.documentElement.classList.add("js");
  if (initialTheme) document.documentElement.dataset.theme = initialTheme;

  // ---------------------------------------------------------------- the page

  var NETWORK_MESSAGE = "Is fidryn ui still running? The mill could not reach 127.0.0.1.";
  var CLOCK_HINT = "Use RFC 3339, for example 2026-09-17T12:00:00Z.";
  var DEFAULT_TEMPLATE = "{{module}}@{{version}}\noutside: {{#each outside_scope}}{{item}} {{/each}}\n";
  var HISTORY_LIMIT = 20;
  var HEALTH_RETRY_MS = 3000;
  var BUSY_DELAY_MS = 400;
  var ACTIONS = ["check", "run", "explore", "render"];
  var BUSY_TEXT = { check: "Checking…", run: "Running…", explore: "Exploring…", render: "Rendering…" };
  var BUFFER_LABELS = { module: "Module source", case: "Case JSON", template: "Template" };
  var SAMPLE_FIELDS = ["id", "title", "blurb", "source", "case", "query", "validAt", "knownAt", "action", "expect"];
  var KINDS = {
    determinate: { cls: "det", label: "Determinate", short: "Det" },
    contingent: { cls: "con", label: "Contingent", short: "Cont" },
    suspended: { cls: "sus", label: "Suspended", short: "Susp" },
    normConflict: { cls: "nc", label: "NormConflict", short: "Conf" },
    outsideCompetence: { cls: "oc", label: "OutsideCompetence", short: "Out" },
    inconsistent: { cls: "inc", label: "Inconsistent", short: "Inc" }
  };

  var el = {};
  var state = null;
  var samples = [];
  var samplesFailed = false;
  var historyEntries = [];
  var entrySeq = 0;
  var shown = null;
  var shownFromHistory = false;
  var pendingSample = null;
  var busy = false;
  var busyTimer = 0;
  var healthTimer = 0;
  var editorBuffer = null;
  var bufferViews = {};
  var diagnostics = { source: null, list: [], version: 0 };

  document.addEventListener("DOMContentLoaded", start);

  function start() {
    [
      "health", "samples", "confirm", "confirm-replace", "confirm-cancel", "history", "buffers",
      "buffer-status", "editor-wrap", "gutter", "hl", "editor", "diag-pop", "query", "query-names",
      "validAt", "validAt-err", "knownAt", "knownAt-err", "check", "run", "explore", "render",
      "views", "result-body"
    ].forEach(function (id) {
      el[id.replace(/-([a-z])/g, function (_, c) { return c.toUpperCase(); })] = document.getElementById(id);
    });
    setupTheme();
    showPlatformKeys();
    wireEvents();
    renderHistory();
    checkHealth();
    loadSamples().then(function () {
      var first = samples[0];
      state = loadState(storage(), first ? sampleState(first, null) : copyState({ template: DEFAULT_TEMPLATE }));
      applyInputs();
      setView(state.view);
      markSample();
      editorChanged("load");
      saveState();
    });
  }

  // ------------------------------------------------------------------- theme

  function currentTheme() {
    var theme = document.documentElement.dataset.theme;
    if (theme === "light" || theme === "dark") return theme;
    return window.matchMedia && window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function setupTheme() {
    var button = document.querySelector("[data-theme-toggle]");
    if (!button) return;
    function label() {
      button.setAttribute("aria-label", currentTheme() === "dark" ? "Switch to light theme" : "Switch to dark theme");
    }
    button.hidden = false;
    label();
    button.addEventListener("click", function () {
      var next = currentTheme() === "dark" ? "light" : "dark";
      document.documentElement.dataset.theme = next;
      try {
        window.localStorage.setItem(THEME_KEY, next);
      } catch (err) {
        // Private windows may refuse storage; the theme still applies now.
      }
      label();
    });
  }

  function showPlatformKeys() {
    var mac = /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent || "");
    if (!mac) return;
    Array.prototype.forEach.call(document.querySelectorAll(".mill-mod"), function (key) {
      key.textContent = "⌘";
      key.title = "Command";
    });
  }

  // ------------------------------------------------------------- DOM helpers

  /** Build an element: `props` become attributes (`text` sets textContent), `kids` are nodes or strings. */
  function h(tag, props, kids) {
    var node = document.createElement(tag);
    Object.keys(props || {}).forEach(function (key) {
      var value = props[key];
      if (value === null || value === undefined || value === false) return;
      if (key === "text") node.textContent = value;
      else if (key === "className") node.className = value;
      else node.setAttribute(key, value === true ? "" : String(value));
    });
    (kids || []).forEach(function (kid) {
      if (kid === null || kid === undefined || kid === false) return;
      node.appendChild(typeof kid === "string" ? document.createTextNode(kid) : kid);
    });
    return node;
  }

  function clear(node) {
    while (node.firstChild) node.removeChild(node.firstChild);
  }

  /** `{ cls, label, short }` for an outcome kind; unknown kinds get a plain stamp. */
  function kindInfo(kind) {
    if (typeof kind === "string" && Object.prototype.hasOwnProperty.call(KINDS, kind)) return KINDS[kind];
    var name = typeof kind === "string" && kind ? kind : "Outcome";
    return { cls: "", label: name, short: name };
  }

  /** A stamp showing `label`; when `full` differs it becomes the accessible name. */
  function stampEl(cls, label, full) {
    var node = h("span", { className: cls ? "stamp " + cls : "stamp" });
    if (full && full !== label) {
      node.title = full;
      node.appendChild(h("span", { "aria-hidden": "true", text: label }));
      node.appendChild(h("span", { className: "sr-only", text: full }));
    } else {
      node.textContent = label;
    }
    return node;
  }

  function clock(date) {
    function two(n) { return (n < 10 ? "0" : "") + n; }
    return two(date.getHours()) + ":" + two(date.getMinutes()) + ":" + two(date.getSeconds());
  }

  // ------------------------------------------------------------------ events

  function wireEvents() {
    el.samples.addEventListener("click", function (event) {
      var button = event.target.closest("[data-sample]");
      if (button) pickSample(button.getAttribute("data-sample"));
    });
    el.confirmReplace.addEventListener("click", function () {
      var sample = hideConfirm();
      if (!sample) return;
      useSample(sample, true);
      focusSample(sample.id);
    });
    el.confirmCancel.addEventListener("click", function () {
      var sample = hideConfirm();
      if (sample) focusSample(sample.id);
    });
    el.confirm.addEventListener("keydown", function (event) {
      if (event.key !== "Escape") return;
      var sample = hideConfirm();
      if (sample) focusSample(sample.id);
    });
    el.buffers.addEventListener("click", function (event) {
      var tab = event.target.closest("[data-buffer]");
      if (tab && state) showBuffer(tab.getAttribute("data-buffer"));
    });
    el.buffers.addEventListener("keydown", onTabKey);
    el.editor.addEventListener("input", function () {
      if (!state) return;
      state[state.buffer] = el.editor.value;
      saveState();
      editorChanged("edit");
    });
    el.query.addEventListener("input", function () { setField("query"); });
    ["validAt", "knownAt"].forEach(function (name) {
      el[name].addEventListener("input", function () {
        setField(name);
        checkClock(name);
      });
    });
    ACTIONS.forEach(function (action) {
      el[action].addEventListener("click", function () { runAction(action); });
    });
    el.views.addEventListener("click", function (event) {
      var button = event.target.closest("[data-view]");
      if (button && !button.disabled && state) setView(button.getAttribute("data-view"));
    });
    el.history.addEventListener("click", function (event) {
      var button = event.target.closest("[data-history]");
      var entry = button && findEntry(button.getAttribute("data-history"));
      if (entry) showEntry(entry, true);
    });
    el.resultBody.addEventListener("click", onResultClick);
    document.addEventListener("keydown", function (event) {
      if (event.key !== "Enter" || !(event.ctrlKey || event.metaKey) || event.altKey) return;
      event.preventDefault();
      runAction(event.shiftKey ? "explore" : "run");
    });
  }

  function onTabKey(event) {
    var tabs = Array.prototype.slice.call(el.buffers.querySelectorAll("[data-buffer]"));
    var at = tabs.indexOf(document.activeElement);
    if (at < 0 || !state) return;
    var next = -1;
    if (event.key === "ArrowRight") next = (at + 1) % tabs.length;
    else if (event.key === "ArrowLeft") next = (at + tabs.length - 1) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    if (next < 0) return;
    event.preventDefault();
    tabs[next].focus();
    showBuffer(tabs[next].getAttribute("data-buffer"));
  }

  function onResultClick(event) {
    var copy = event.target.closest("[data-copy]");
    if (copy) {
      copyFrom(copy);
      return;
    }
    var restore = event.target.closest("[data-restore]");
    if (restore) {
      var entry = findEntry(restore.getAttribute("data-restore"));
      if (entry) restoreInputs(entry);
      return;
    }
    var jump = event.target.closest("[data-jump]");
    if (jump) {
      var parts = jump.getAttribute("data-jump").split(":");
      jumpTo(parts[0], Number(parts[1]), Number(parts[2]));
    }
  }

  // ------------------------------------------------------------------- state

  function storage() {
    try {
      return window.localStorage;
    } catch (err) {
      return null;
    }
  }

  function saveState() {
    var store = storage();
    if (!state || !store) return;
    try {
      store.setItem(STATE_KEY, JSON.stringify(state));
    } catch (err) {
      // Storage can be full or refused; the page keeps working without it.
    }
  }

  function sampleState(sample, template) {
    return copyState({
      module: sample.source,
      case: sample.case,
      template: template === null ? DEFAULT_TEMPLATE : template,
      query: sample.query,
      validAt: sample.validAt,
      knownAt: sample.knownAt,
      sample: sample.id
    });
  }

  function setField(name) {
    if (!state) return;
    state[name] = el[name].value;
    saveState();
  }

  function snapshot() {
    var inputs = { sample: state.sample };
    TEXT_FIELDS.forEach(function (key) { inputs[key] = state[key]; });
    return inputs;
  }

  /** Put the saved inputs into the fields and the editor. */
  function applyInputs() {
    el.query.value = state.query;
    el.validAt.value = state.validAt;
    el.knownAt.value = state.knownAt;
    checkClock("validAt");
    checkClock("knownAt");
    editorBuffer = null;
    bufferViews = {};
    showBuffer(state.buffer);
  }

  function checkClock(name) {
    var ok = isRfc3339(el[name].value.trim());
    var err = el[name + "Err"];
    el[name].setAttribute("aria-invalid", ok ? "false" : "true");
    err.textContent = ok ? "" : CLOCK_HINT;
    err.hidden = ok;
    return ok;
  }

  // ----------------------------------------------------------------- samples

  function isSample(value) {
    return isObject(value) && SAMPLE_FIELDS.every(function (key) { return typeof value[key] === "string"; });
  }

  function loadSamples() {
    return send("/api/samples", { cache: "no-store" }).then(function (res) {
      if (res.network) {
        samplesFailed = true;
        networkDown();
        renderSamples();
        return;
      }
      samples = Array.isArray(res.data) ? res.data.filter(isSample) : [];
      samplesFailed = samples.length === 0;
      renderSamples();
    });
  }

  function findSample(id) {
    for (var i = 0; i < samples.length; i++) {
      if (samples[i].id === id) return samples[i];
    }
    return null;
  }

  function renderSamples() {
    clear(el.samples);
    if (samples.length === 0) {
      el.samples.appendChild(h("li", { className: "mill-none", text: samplesFailed ? "Samples are not available." : "Loading samples…" }));
      return;
    }
    samples.forEach(function (sample) {
      var kind = kindInfo(sample.expect);
      el.samples.appendChild(h("li", null, [
        h("button", { type: "button", className: "mill-sample", "data-sample": sample.id, title: sample.blurb }, [
          h("span", { text: sample.title }),
          stampEl(kind.cls, kind.short, kind.label)
        ])
      ]));
    });
    markSample();
  }

  /** Mark the sample the inputs came from. */
  function markSample() {
    Array.prototype.forEach.call(el.samples.querySelectorAll("[data-sample]"), function (button) {
      if (state && button.getAttribute("data-sample") === state.sample) button.setAttribute("aria-current", "true");
      else button.removeAttribute("aria-current");
    });
  }

  function focusSample(id) {
    Array.prototype.forEach.call(el.samples.querySelectorAll("[data-sample]"), function (button) {
      if (button.getAttribute("data-sample") === id) button.focus();
    });
  }

  /** True when loading a sample now would throw away the user's edits. */
  function isDirty() {
    if (!state) return false;
    var sample = findSample(state.sample);
    if (!sample) return state.module.trim() !== "" || state.case.trim() !== "";
    return state.module !== sample.source || state.case !== sample.case || state.query !== sample.query ||
      state.validAt !== sample.validAt || state.knownAt !== sample.knownAt;
  }

  function pickSample(id) {
    var sample = findSample(id);
    if (!sample || !state) return;
    if (!isDirty()) {
      useSample(sample, true);
      return;
    }
    pendingSample = sample;
    document.getElementById("confirm-text").textContent = "Replace your edits with " + sample.title + "?";
    el.confirm.hidden = false;
    el.confirmCancel.focus();
  }

  /** Close the Replace or Cancel question; returns the sample it was about. */
  function hideConfirm() {
    var sample = pendingSample;
    pendingSample = null;
    el.confirm.hidden = true;
    return sample;
  }

  function useSample(sample, runIt) {
    hideConfirm();
    state = copyState(Object.assign(sampleState(sample, state.template), { buffer: state.buffer, view: state.view }));
    saveState();
    applyInputs();
    markSample();
    editorChanged("load");
    if (runIt) runAction(sample.action === "explore" ? "explore" : "run");
  }

  // ----------------------------------------------------------------- buffers

  function showBuffer(name) {
    if (BUFFERS.indexOf(name) < 0) name = "module";
    if (editorBuffer) {
      bufferViews[editorBuffer] = {
        start: el.editor.selectionStart,
        end: el.editor.selectionEnd,
        top: el.editor.scrollTop,
        left: el.editor.scrollLeft
      };
    }
    state.buffer = name;
    editorBuffer = name;
    el.editor.value = state[name];
    el.editor.setAttribute("aria-label", BUFFER_LABELS[name]);
    Array.prototype.forEach.call(el.buffers.querySelectorAll("[data-buffer]"), function (tab) {
      var on = tab.getAttribute("data-buffer") === name;
      tab.setAttribute("aria-selected", on ? "true" : "false");
      tab.setAttribute("tabindex", on ? "0" : "-1");
    });
    el.editorWrap.setAttribute("aria-labelledby", "tab-" + name);
    var view = bufferViews[name];
    if (view) {
      el.editor.setSelectionRange(view.start, view.end);
      el.editor.scrollTop = view.top;
      el.editor.scrollLeft = view.left;
    } else {
      el.editor.setSelectionRange(0, 0);
      el.editor.scrollTop = 0;
      el.editor.scrollLeft = 0;
    }
    saveState();
    editorChanged("buffer");
  }

  /** Select `start`..`end` in `buffer` and bring it into view. */
  function jumpTo(buffer, start, end) {
    if (!state) return;
    showBuffer(buffer);
    var length = el.editor.value.length;
    var from = Math.max(0, Math.min(start || 0, length));
    var to = Math.max(from, Math.min(end || from, length));
    el.editor.focus();
    el.editor.setSelectionRange(from, to);
    var style = window.getComputedStyle(el.editor);
    var lineHeight = parseFloat(style.lineHeight) || 20;
    var top = (lineCol(el.editor.value, from).line - 1) * lineHeight;
    if (top < el.editor.scrollTop || top > el.editor.scrollTop + el.editor.clientHeight - 2 * lineHeight) {
      el.editor.scrollTop = Math.max(0, top - el.editor.clientHeight / 3);
    }
    el.editorWrap.scrollIntoView({ block: "nearest" });
    editorChanged("caret");
  }

  /** Called after the editor's text, buffer, or caret changes and after new diagnostics. */
  function editorChanged() {
    updateBufferStatus();
  }

  function updateBufferStatus() {
    var text = "";
    var mood = "";
    if (state.buffer === "module" && diagnostics.source === state.module) {
      var n = diagnostics.list.length;
      text = n === 0 ? "No problems" : n === 1 ? "1 problem" : n + " problems";
      mood = n === 0 ? "ok" : "err";
    } else if (state.buffer === "template") {
      text = "Used by Render";
    }
    clear(el.bufferStatus);
    el.bufferStatus.appendChild(h("span", { className: "mill-status-main", text: text }));
    el.bufferStatus.setAttribute("data-state", mood);
  }

  /** Remember the diagnostics of a check of `source`. */
  function noteDiagnostics(source, data) {
    if (!isObject(data) || typeof data.ok !== "boolean") return;
    diagnostics = {
      source: source,
      list: data.ok ? [] : (Array.isArray(data.diagnostics) ? data.diagnostics.filter(isObject) : []),
      version: diagnostics.version + 1
    };
    editorChanged("diagnostics");
  }

  // ------------------------------------------------------------------ health

  function setHealth(mood) {
    el.health.setAttribute("data-state", mood);
    var text = el.health.querySelector(".mill-health-text");
    if (text) text.textContent = mood === "ok" ? "ok" : mood === "down" ? "not responding" : "checking";
  }

  function checkHealth() {
    clearTimeout(healthTimer);
    return send("/api/health", { cache: "no-store" }).then(function (res) {
      var ok = !res.network && res.status === 200 && res.text.trim() === "ok";
      setHealth(ok ? "ok" : "down");
      if (!ok) healthTimer = setTimeout(checkHealth, HEALTH_RETRY_MS);
      else if (samplesFailed) loadSamples();
    });
  }

  /** The server answered a request: it is up. */
  function serverAnswered() {
    clearTimeout(healthTimer);
    setHealth("ok");
    if (samplesFailed) loadSamples();
  }

  /** The server stopped answering: turn the status red and poll until it is back. */
  function networkDown() {
    setHealth("down");
    clearTimeout(healthTimer);
    healthTimer = setTimeout(checkHealth, HEALTH_RETRY_MS);
  }

  // ----------------------------------------------------------------- actions

  function setBusy(action, on) {
    busy = on;
    clearTimeout(busyTimer);
    ACTIONS.forEach(function (name) {
      if (on) el[name].setAttribute("aria-disabled", "true");
      else el[name].removeAttribute("aria-disabled");
    });
    if (on) el[action].setAttribute("aria-busy", "true");
    else el[action].removeAttribute("aria-busy");
    el.resultBody.setAttribute("aria-busy", on ? "true" : "false");
    if (on) {
      busyTimer = setTimeout(function () {
        showNodes([h("p", { className: "mill-busy", text: BUSY_TEXT[action] })]);
      }, BUSY_DELAY_MS);
    }
  }

  /** The request for `action`, or `{ error }` when an input is not ready to send. */
  function buildRequest(action, inputs) {
    if (action === "check") return { path: "/api/check", body: { source: inputs.module } };
    if (action === "render") return { path: "/api/render", body: { source: inputs.module, template: inputs.template } };
    var clocks = ["validAt", "knownAt"];
    for (var i = 0; i < clocks.length; i++) {
      if (!checkClock(clocks[i])) return { error: errorBox("Invalid input", clocks[i] + ": " + CLOCK_HINT) };
    }
    var parsed = parseCase(inputs.case);
    if (!parsed.ok) {
      return {
        error: errorBox("Invalid case JSON", "Line " + parsed.line + ", column " + parsed.col + ": " + parsed.message,
          h("div", { className: "actions" }, [
            h("button", { type: "button", className: "btn sec sm", "data-jump": "case:" + parsed.index + ":" + parsed.index, text: "Show in Case JSON" })
          ]))
      };
    }
    return {
      path: "/api/" + action,
      body: {
        source: inputs.module,
        query: inputs.query.trim(),
        case: parsed.value,
        validAt: inputs.validAt.trim(),
        knownAt: inputs.knownAt.trim()
      }
    };
  }

  function runAction(action) {
    if (busy || !state) return;
    var inputs = snapshot();
    var request = buildRequest(action, inputs);
    if (request.error) {
      showMessage(request.error);
      return;
    }
    setBusy(action, true);
    postJson(request.path, request.body).then(function (res) {
      if (res.network) {
        networkDown();
        showMessage(errorBox("No connection", NETWORK_MESSAGE));
        return;
      }
      serverAnswered();
      var entry = makeEntry(action, inputs, res);
      if (action === "check") noteDiagnostics(inputs.module, res.data);
      historyEntries.unshift(entry);
      if (historyEntries.length > HISTORY_LIMIT) historyEntries.length = HISTORY_LIMIT;
      renderHistory();
      showEntry(entry, false);
      revealResult();
    }).catch(function (err) {
      showMessage(errorBox("The page could not show this result", String(err && err.message ? err.message : err)));
    }).then(function () {
      setBusy(action, false);
    });
  }

  /** Show a message that is not a history entry (an input problem or a lost connection). */
  function showMessage(node) {
    shown = null;
    setViewsEnabled(true);
    showNodes([node]);
    markHistory();
    revealResult();
  }

  /** On a phone the result sits below the editor; scroll to it when it is out of sight. */
  function revealResult() {
    var top = el.resultBody.getBoundingClientRect().top;
    if (top > window.innerHeight - 120) el.resultBody.scrollIntoView({ block: "start" });
  }

  /** Sort a response into what the result area shows for it. */
  function entryKind(action, res) {
    var data = isObject(res.data) ? res.data : null;
    if (!data) return "unexpected";
    if (action === "check") return typeof data.ok === "boolean" ? "check" : "unexpected";
    if (action === "render") {
      if (data.ok === true && typeof data.text === "string") return "rendered";
      return data.ok === false && typeof data.error === "string" ? "renderError" : "unexpected";
    }
    if (data.ok === true && isObject(data.report)) return "report";
    if (data.kind === "engineError") return "engine";
    if (data.error === "check failed" && Array.isArray(data.diagnostics)) return "diagnostics";
    return data.ok === false && typeof data.error === "string" ? "invalid" : "unexpected";
  }

  function makeEntry(action, inputs, res) {
    var entry = {
      id: String(++entrySeq),
      action: action,
      time: new Date(),
      inputs: inputs,
      query: action === "run" || action === "explore" ? inputs.query.trim() : "",
      status: res.status,
      data: res.data,
      text: res.text,
      kind: entryKind(action, res)
    };
    entry.stamp = entryStamp(entry);
    return entry;
  }

  /** `{ cls, label, short }` of the stamp an entry shows in the history list. */
  function entryStamp(entry) {
    switch (entry.kind) {
      case "report":
        return kindInfo(outcomeOf(entry).kind);
      case "check":
        return entry.data.ok ? { cls: "det", label: "ok", short: "ok" } : { cls: "inc", label: "Check failed", short: "Fail" };
      case "diagnostics":
        return { cls: "inc", label: "Check failed", short: "Fail" };
      case "rendered":
        return { cls: "det", label: "Rendered", short: "ok" };
      case "renderError":
        return { cls: "inc", label: "Render failed", short: "Fail" };
      default:
        return { cls: "inc", label: "Error", short: "Error" };
    }
  }

  function outcomeOf(entry) {
    var doc = entry.data.report.outcomeDocument;
    return isObject(doc) && isObject(doc.outcome) ? doc.outcome : {};
  }

  function entryLabel(entry) {
    return entry.action + (entry.query ? " " + entry.query : "");
  }

  function findEntry(id) {
    for (var i = 0; i < historyEntries.length; i++) {
      if (historyEntries[i].id === id) return historyEntries[i];
    }
    return null;
  }

  // ----------------------------------------------------------------- history

  function renderHistory() {
    clear(el.history);
    if (historyEntries.length === 0) {
      el.history.appendChild(h("li", { className: "mill-none", text: "Nothing run yet" }));
      return;
    }
    historyEntries.forEach(function (entry) {
      el.history.appendChild(h("li", null, [
        h("button", { type: "button", className: "mill-hist", "data-history": entry.id }, [
          h("time", { className: "mill-hist-time", datetime: entry.time.toISOString(), text: clock(entry.time) }),
          h("span", { className: "mill-hist-what", text: entryLabel(entry) }),
          stampEl(entry.stamp.cls, entry.stamp.short, entry.stamp.label)
        ])
      ]));
    });
    markHistory();
  }

  function markHistory() {
    Array.prototype.forEach.call(el.history.querySelectorAll("[data-history]"), function (button) {
      var on = shown !== null && button.getAttribute("data-history") === shown.id;
      if (on) button.setAttribute("aria-current", "true");
      else button.removeAttribute("aria-current");
    });
  }

  function restoreInputs(entry) {
    hideConfirm();
    state = copyState(Object.assign({}, entry.inputs, { buffer: state.buffer, view: state.view }));
    saveState();
    applyInputs();
    markSample();
    editorChanged("load");
  }

  // ------------------------------------------------------------------ result

  function showNodes(nodes) {
    clearTimeout(busyTimer);
    clear(el.resultBody);
    nodes.forEach(function (node) { el.resultBody.appendChild(node); });
  }

  function setView(view) {
    state.view = VIEWS.indexOf(view) >= 0 ? view : "opinion";
    saveState();
    Array.prototype.forEach.call(el.views.querySelectorAll("[data-view]"), function (button) {
      button.setAttribute("aria-pressed", button.getAttribute("data-view") === state.view ? "true" : "false");
    });
    if (shown && shown.kind === "report") showEntry(shown, shownFromHistory);
  }

  /** The view switch only applies to evaluation reports. */
  function setViewsEnabled(on) {
    Array.prototype.forEach.call(el.views.querySelectorAll("[data-view]"), function (button) {
      button.disabled = !on;
    });
  }

  function showEntry(entry, fromHistory) {
    shown = entry;
    shownFromHistory = fromHistory;
    setViewsEnabled(entry.kind === "report");
    var nodes = [];
    if (fromHistory) {
      nodes.push(h("div", { className: "mill-from" }, [
        h("p", { className: "label", text: "From history · " + clock(entry.time) + " · " + entryLabel(entry) }),
        h("button", { type: "button", className: "btn sec sm", "data-restore": entry.id, text: "Restore inputs" })
      ]));
    }
    nodes.push(entryBody(entry));
    showNodes(nodes);
    markHistory();
  }

  function entryBody(entry) {
    var data = entry.data;
    switch (entry.kind) {
      case "report":
        if (state.view === "table") return tableView(data.report);
        if (state.view === "json") return codeFrame("json", JSON.stringify(data.report, null, 2));
        return opinionView(data);
      case "check":
        if (data.ok) return h("div", { className: "mill-ok" }, [stampEl("det", "ok"), h("p", { text: "No diagnostics." })]);
        return diagnosticsView(data.diagnostics, entry.inputs.module);
      case "diagnostics":
        return diagnosticsView(data.diagnostics, entry.inputs.module);
      case "engine":
        return errorBox("Engine error · " + str(data.error), str(data.message));
      case "invalid":
        return errorBox("Invalid input", data.error);
      case "rendered":
        return h("article", { className: "mill-doc", "aria-label": "Rendered text" }, [
          h("p", { className: "mill-doc-cap", text: "Rendered template" }),
          h("pre", { className: "mill-rendered-text", text: data.text })
        ]);
      case "renderError":
        return errorBox("Render failed", data.error);
      default:
        return errorBox("Unexpected response", "The mill answered HTTP " + entry.status +
          (entry.text ? ": " + entry.text.slice(0, 300) : "."));
    }
  }

  function errorBox(title, message, extra) {
    return h("div", { className: "mill-error", role: "alert" }, [
      h("p", { className: "mill-error-title", text: title }),
      h("p", { className: "mill-error-msg", text: message }),
      extra || null
    ]);
  }

  /** The printed-opinion view: caption, outcome title, the server's sentences, then module and valid time. */
  function opinionView(data) {
    var report = data.report;
    var doc = isObject(report.outcomeDocument) ? report.outcomeDocument : {};
    var outcome = isObject(doc.outcome) ? doc.outcome : {};
    var asOf = isObject(doc.asOf) ? doc.asOf : {};
    var kind = kindInfo(outcome.kind);
    var sentences = Array.isArray(data.opinion) ? data.opinion.filter(function (s) { return typeof s === "string"; }) : [];
    var article = h("article", { className: "mill-doc", "aria-label": "Opinion" }, [
      h("p", { className: "mill-doc-cap", text: "Evaluation report · " + str(report.executionMode) + " · " + str(report.sourceTrust) }),
      h("h3", { className: "mill-doc-title" }, [h("span", { className: "mill-mark " + kind.cls, "aria-hidden": "true" }), kind.label])
    ]);
    sentences.forEach(function (sentence, i) {
      var boundary = i === sentences.length - 1 && sentence.indexOf("Outside scope: ") === 0;
      article.appendChild(h("p", { className: boundary ? "mill-doc-boundary" : null, text: sentence }));
    });
    if (sentences.length === 0) {
      article.appendChild(h("p", { className: "mill-doc-boundary", text: "The server sent no sentences for this report; see Table or JSON." }));
    }
    article.appendChild(h("footer", { className: "mill-doc-sig" }, [
      h("span", { text: str(doc.module) }),
      h("span", { text: "as of " + str(asOf.validTime) })
    ]));
    return article;
  }

  /** JSON on one line with a space after each comma and colon, so table cells can wrap. */
  function spaced(value) {
    var text = JSON.stringify(value, null, 1);
    return text === undefined ? String(value) : text.replace(/\n\s*/g, " ");
  }

  function completionLabel(key) {
    return key.replace(/^[a-z]:/, "").split("=").join(" = ");
  }

  function pivotNames(pivots) {
    var names = [];
    (Array.isArray(pivots) ? pivots : []).forEach(function (pivot) {
      if (!isObject(pivot)) return;
      var name = typeof pivot.family === "string" ? pivot.family
        : typeof pivot.protocol === "string" ? pivot.protocol : str(pivot.kind);
      if (names.indexOf(name) < 0) names.push(name);
    });
    return names;
  }

  /** The compact-table view: every field of the report, one row each. */
  function tableView(report) {
    var doc = isObject(report.outcomeDocument) ? report.outcomeDocument : {};
    var outcome = isObject(doc.outcome) ? doc.outcome : {};
    var boundary = isObject(doc.modelBoundary) ? doc.modelBoundary : {};
    var asOf = isObject(doc.asOf) ? doc.asOf : {};
    var kind = kindInfo(outcome.kind);
    var rows = [["kind", stampEl(kind.cls, kind.label)]];
    var done = { kind: true, trace: true };
    function row(label, value) { rows.push([label, value]); }
    if ("value" in outcome) {
      row("value", valueText(outcome.value));
      done.value = true;
    }
    if (isObject(outcome.alternatives)) {
      Object.keys(outcome.alternatives).sort().forEach(function (key) {
        row(completionLabel(key), valueText(outcome.alternatives[key]));
      });
      done.alternatives = true;
    }
    if (Array.isArray(outcome.pivots)) {
      row("pivots", pivotNames(outcome.pivots).join(", ") || "none");
      done.pivots = true;
    }
    if (Array.isArray(outcome.requests)) {
      outcome.requests.forEach(function (request) { row("request", requestText(request)); });
      done.requests = true;
    }
    if ("request" in outcome) {
      row("request", requestText(outcome.request));
      done.request = true;
    }
    if (Array.isArray(outcome.doctrines)) {
      row("doctrines", doctrineNames(outcome.doctrines).join(", ") || "none");
      done.doctrines = true;
    }
    if (Array.isArray(outcome.core)) {
      row("core", outcome.core.map(str).join(", ") || "none");
      done.core = true;
    }
    Object.keys(outcome).sort().forEach(function (key) {
      if (!done[key]) row(key, typeof outcome[key] === "string" ? outcome[key] : spaced(outcome[key]));
    });
    row("query", str(doc.query));
    row("module", str(doc.module));
    row("sourceSnapshot", str(doc.sourceSnapshot));
    row("asOf", "valid " + str(asOf.validTime) + " · record " + str(asOf.recordTime));
    row("outsideScope", Array.isArray(boundary.outsideScope) && boundary.outsideScope.length
      ? boundary.outsideScope.map(str).join(", ") : "none");
    row("admissibleCompletions", spaced(boundary.admissibleCompletions));
    row("mode / trust", str(report.executionMode) + " · " + str(report.sourceTrust));
    row("verificationMethod", str(report.verificationMethod));
    row("coverage", spaced(report.coverage));
    row("assumptions", Array.isArray(report.assumptions) && report.assumptions.length
      ? spaced(report.assumptions) : "none");
    row("trace", h("span", { className: "mill-copy" }, [
      h("code", { text: str(outcome.trace) }),
      h("button", { type: "button", className: "copy", "data-copy": "", text: "Copy" })
    ]));
    return h("div", { className: "table-wrap", tabindex: "0", role: "region", "aria-label": "Report fields" }, [
      h("table", { className: "mill-table" }, [
        h("tbody", null, rows.map(function (r) {
          return h("tr", null, [h("th", { scope: "row", text: r[0] }), h("td", null, [r[1]])]);
        }))
      ])
    ]);
  }

  /** A framed code block with a Copy button, as on the site. */
  function codeFrame(lang, source) {
    var body = h("code");
    body.textContent = source;
    return h("figure", { className: "code", "data-lang": lang }, [
      h("figcaption", null, [
        h("span", { text: lang }),
        h("button", { type: "button", className: "copy", "data-copy": "", text: "Copy" })
      ]),
      h("pre", null, [body])
    ]);
  }

  /** A failed check: each diagnostic with its code, message, line:column, and a Jump button. */
  function diagnosticsView(list, source) {
    var items = Array.isArray(list) ? list.filter(isObject) : [];
    var ol = h("ol", { className: "mill-diag-list" });
    items.forEach(function (d) {
      var span = isObject(d.primary_span) ? d.primary_span : null;
      var at = null;
      if (span && typeof span.start === "number") {
        var from = byteToIndex(source, span.start);
        var to = byteToIndex(source, typeof span.end === "number" ? span.end : span.start);
        at = { from: from, to: Math.max(from, to), pos: lineCol(source, from) };
      }
      ol.appendChild(h("li", null, [
        h("code", { className: "mill-diag-code", text: str(d.code) }),
        h("span", { className: "mill-diag-msg", text: str(d.message) }),
        at ? h("span", { className: "mill-diag-at", text: at.pos.line + ":" + at.pos.col }) : null,
        at ? h("button", { type: "button", className: "btn sec sm", "data-jump": "module:" + at.from + ":" + at.to, text: "Jump" }) : null,
        typeof d.suggestion === "string" && d.suggestion ? h("span", { className: "mill-diag-hint", text: d.suggestion }) : null
      ]));
    });
    return h("div", { className: "mill-diags" }, [
      h("p", { className: "mill-res-head" }, [
        stampEl("inc", "Check failed"),
        h("span", { className: "label", text: items.length === 1 ? "1 problem" : items.length + " problems" })
      ]),
      ol
    ]);
  }

  // -------------------------------------------------------------------- copy

  function copyFrom(button) {
    var box = button.closest(".code, .mill-copy");
    var code = box && box.querySelector("code");
    if (!code) return;
    writeClipboard(code.textContent).then(function (ok) {
      if (!ok) return;
      button.setAttribute("data-copied", "");
      button.textContent = "Copied";
      setTimeout(function () {
        button.removeAttribute("data-copied");
        button.textContent = "Copy";
      }, 1400);
    });
  }

  function writeClipboard(text) {
    if (navigator.clipboard && navigator.clipboard.writeText) {
      return navigator.clipboard.writeText(text).then(function () { return true; }, function () { return legacyCopy(text); });
    }
    return Promise.resolve(legacyCopy(text));
  }

  function legacyCopy(text) {
    var active = document.activeElement;
    var area = h("textarea", { className: "sr-only", "aria-hidden": "true", tabindex: "-1" });
    area.value = text;
    document.body.appendChild(area);
    area.select();
    var ok = false;
    try {
      ok = document.execCommand("copy");
    } catch (err) {
      ok = false;
    }
    document.body.removeChild(area);
    if (active && active.focus) active.focus();
    return ok;
  }
})();
```

In `crates/fidryn-cli/src/ui.rs`, embed the script.

Find:

```rust
const MILL_CSS: &str = include_str!("../../../web/mill.css");
```

Replace it with:

```rust
const MILL_CSS: &str = include_str!("../../../web/mill.css");
const MILL_JS: &str = include_str!("../../../web/mill.js");
```

Add its handler after `mill_css`.

Find:

```rust
async fn mill_css() -> Response {
    static_text("text/css; charset=utf-8", MILL_CSS)
}
```

Replace it with:

```rust
async fn mill_css() -> Response {
    static_text("text/css; charset=utf-8", MILL_CSS)
}

async fn mill_js() -> Response {
    static_text("text/javascript; charset=utf-8", MILL_JS)
}
```

Route it after the mill stylesheet.

Find:

```rust
        .route("/assets/mill.css", get(mill_css))
```

Replace it with:

```rust
        .route("/assets/mill.css", get(mill_css))
        .route("/assets/mill.js", get(mill_js))
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `node --check web/mill.js`
Expected: no output, exit status 0.

Run: `node --test web/tests/mill.test.js`
Expected: PASS; all 18 subtests print `ok` and the summary shows `# fail 0` (Node 18 counts the file as one test, `# pass 1`; newer Node prints `# pass 18`).

Run: `cargo test -p fidryn-cli --offline --lib ui::`
Expected: PASS; every `ui::tests::…` test passes, including `mill_js_route_serves_the_embedded_script`.

- [ ] **Step 5: Use the page**

Run `cargo run -p fidryn-cli --offline -- ui --no-open` and open `http://127.0.0.1:8751` in a fresh private window. Check:
  - no console errors and no CSP violations; the header dot is green with `localhost 127.0.0.1 · ok`; the theme toggle is visible and switches the page between cream and Ink, and a reload keeps the choice;
  - first run: the editor holds `tests/programs/require-gate.fr`, Query is `q`, both clocks are `2026-09-17T12:00:00Z`, `require-gate` is marked in the rail, and the result says `Nothing has run yet.` with the Run shortcut (Command sign on a Mac);
  - Ctrl or Cmd+Enter: History gets `run q` with a Det stamp and the Opinion view shows `Determinate` and `q is 7.`; Table lists kind, value `7`, query, module `Programs.RequireGate@0.1.0`, asOf, outsideScope, and the trace with Copy; JSON shows the pretty report with Copy; the chosen view survives a reload;
  - pick `Trust, open eligibility`: it runs and shows `Contingent` with `Under I1 it is Alice.` and `Under I2 it is Bob.`; Explore (Ctrl or Cmd+Shift+Enter) shows `Under SuccessorEligibility = I1 it is Alice.`;
  - type in the editor, then pick another sample: the rail asks `Replace your edits with …?`; Cancel keeps the edits, Replace loads the sample;
  - switch to Case JSON, delete a closing brace, press Run: `Invalid case JSON` with `Line …, column …` and a `Show in Case JSON` button that selects the spot; set validAt to `2026-09-17`: the field turns red with `Use RFC 3339, for example 2026-09-17T12:00:00Z.`;
  - add a line `colour blue` inside the module and press Check: `Check failed`, `` E100 unknown declaration `colour` ``, the line:column, and Jump, which selects `colour` in the editor;
  - stop the server with Ctrl-C and press Run: `Is fidryn ui still running? The mill could not reach 127.0.0.1.` inline and the dot turns red with `not responding`; start it again and the dot turns green within about three seconds;
  - click an older History entry: its result returns under `From history · …` with `Restore inputs`, which puts that entry's inputs back.

- [ ] **Step 6: Commit**

```bash
git add web/mill.js web/tests/mill.test.js crates/fidryn-cli/src/ui.rs
git -c commit.gpgsign=false commit -m "feat(mill): mill app with samples, history, and result views"
```

### Task 16: Mill editor

**Files:**
- Modify: `web/mill.js` (six edits: the editor helpers before the export gate; the export list; `installEditor();` in `start`; `editorChanged` and `updateBufferStatus` replaced; the JSON view highlighted in `codeFrame`; the editor section before the closing `})();`)
- Modify: `web/tests/mill.test.js` (append the editor tests)
- Create: `xtask/tests/mill_keywords.rs`
- Test: `web/tests/mill.test.js`, `xtask/tests/mill_keywords.rs`

**Interfaces:**
- Consumes: Task 15's `web/mill.js`: the anchors quoted in Step 3, the private helpers `byteToIndex`, `lineCol`, `parseCase`, `JSON_NUMBER`, `esc`, `str`, `isObject`, `h`, `clear`, `el`, `state`, `diagnostics`, `noteDiagnostics`, `serverAnswered`, `networkDown`, `postJson`; Task 14's `#editor-wrap[data-hl]` switch and the `.mill-gutter-in`, `.mill-ln`, `.has-diag`, `.mill-sq`, `.mill-pop`, `.mill-status-main`, `.mill-status-hint` styles; the C1 keyword rule; `grammar.ebnf`.
- Produces: the final export list `{ esc, isRfc3339, valueText, requestText, loadState, postJson, byteToIndex, lineCol, tokenizeFr, tokenizeJson, applyRanges, segmentsToHtml, KEYWORDS, editorKey }` (C8's thirteen names plus `editorKey`, see the contract notes). Signatures: `byteToIndex(text, offset) -> index` (UTF-8 byte offset to string index; inside a character gives its start, past the end gives `text.length`); `lineCol(text, index) -> {line, col}` (1-based, columns count characters); `tokenizeFr(text)` and `tokenizeJson(text) -> Array<{text, cls}>` where `cls` is `""` or `tk-kw`, `tk-ty`, `tk-st`, `tk-nu`, `tk-co`, `tk-pu` (C5 classes) and joining the texts gives the input back; `applyRanges(segments, ranges: Array<{start, end, cls}>) -> segments`; `segmentsToHtml(segments) -> string` (every text escaped); `editorKey(key, mods: {shift, ctrl, meta, alt, composing}, escaped) -> {action: "indent" | "outdent" | "newline" | "leave" | "", escaped}`; `KEYWORDS: string[]`, one JSON string per line between `// KEYWORDS-BEGIN` and `// KEYWORDS-END`, sorted, each followed by a comma. `xtask/tests/mill_keywords.rs` keeps that list equal to the grammar.

What the editor does: a highlighted `<pre id="hl">` sits under the transparent textarea with the same font, padding, line height, tab size, and no wrapping, and follows its scroll (a final newline gets a trailing space so the layers keep the same height); `#gutter` numbers the lines and follows the vertical scroll; Tab indents to the next stop (4 spaces in the module, 2 in the case and template) and indents every selected line, Shift-Tab outdents, Enter keeps the line's indentation, all as undoable edits; Escape then Tab (or Shift-Tab) leaves the editor once, with `Esc then Tab to leave the editor` shown in `#buffer-status` while the editor has focus; Check runs 700 ms after typing stops, drops stale answers (a sequence number plus the diagnostics version), and never enters History; each diagnostic's UTF-8 span becomes a wavy underline (an empty span underlines the character at it, or the one before it at a line end), a gutter mark, and a message box right under the line while the caret or the pointer is on it; the case buffer is validated as you type with `Line L, column C: …` in `#buffer-status` and the same underline; `#query-names` offers the module's `query` names; the JSON result view is highlighted.

- [ ] **Step 1: Write the failing tests**

Append to the end of `web/tests/mill.test.js`:

```js

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

test("byteToIndex counts 2-, 2-, and 4-byte characters exactly", () => {
  assert.equal(mill.byteToIndex("§x", 2), 1);
  assert.equal(mill.byteToIndex("éx", 2), 1);
  assert.equal(mill.byteToIndex("𝔽x", 4), 2);
  assert.equal(mill.byteToIndex("a𝔽b", 5), 3);
  assert.equal(mill.byteToIndex("\"§é𝔽\"", 1 + 2 + 2 + 4), 5);
  let bytes = 0;
  const text = "a§é𝔽\n// 𝔽§\nz";
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
    ["module", 0, "tk-kw"], ["Programs", 0, "tk-ty"], [".RequireGate", 0, "tk-ty"], ["version", 0, "tk-kw"],
    ["\"0.1.0\"", 0, "tk-st"], ["{", 0, "tk-pu"], ["// r must", 0, "tk-co"],
    ["import", 0, "tk-kw"], ["MA", 0, "tk-ty"], ["TrustLaw", 0, "tk-ty"], ["Fixture", 0, "tk-ty"],
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
  const source = "module A version \"1\" {\n    // § é 𝔽\n    colour blue\n}\n";
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
```

Create `xtask/tests/mill_keywords.rs`:

```rust
//! The mill highlights `.fr` keywords from a list inside `web/mill.js`. That
//! list must stay equal to the grammar's keywords: every quoted terminal in
//! `grammar.ebnf` made only of `[a-z_]` and longer than one character.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn grammar_keywords(grammar: &str) -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    let mut rest = grammar;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else { break };
        let term = &after[..close];
        if term.len() > 1 && term.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
            words.insert(term.to_owned());
        }
        rest = &after[close + 1..];
    }
    words
}

/// The entries between `// KEYWORDS-BEGIN` and `// KEYWORDS-END`, in file
/// order. Each line must be one JSON string followed by a comma.
fn mill_keywords(js: &str) -> Vec<String> {
    let begin = js
        .find("// KEYWORDS-BEGIN")
        .expect("web/mill.js has // KEYWORDS-BEGIN");
    let end = js
        .find("// KEYWORDS-END")
        .expect("web/mill.js has // KEYWORDS-END");
    assert!(begin < end, "KEYWORDS-BEGIN must come before KEYWORDS-END");
    js[begin..end]
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let entry = line
                .strip_suffix(',')
                .unwrap_or_else(|| panic!("keyword line {line:?} must end with a comma"));
            entry
                .strip_prefix('"')
                .and_then(|word| word.strip_suffix('"'))
                .filter(|word| !word.contains('"') && !word.contains('\\'))
                .unwrap_or_else(|| panic!("keyword line {line:?} must be one JSON string"))
                .to_owned()
        })
        .collect()
}

#[test]
fn grammar_rule_finds_the_keywords_and_skips_other_terminals() {
    let words = grammar_keywords(
        r#"A ::= "module" QName "{" "outside_scope" "UniqueOccupant" "+inf" "as" "_" "#,
    );
    let expected: BTreeSet<String> = ["as", "module", "outside_scope"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(words, expected);
}

#[test]
fn mill_keyword_list_equals_the_grammar_keywords() {
    let grammar = grammar_keywords(&read("grammar.ebnf"));
    let mill: BTreeSet<String> = mill_keywords(&read("web/mill.js")).into_iter().collect();
    let missing: Vec<&String> = grammar.difference(&mill).collect();
    let extra: Vec<&String> = mill.difference(&grammar).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "web/mill.js KEYWORDS differs from grammar.ebnf\n  missing: {missing:?}\n  not in the grammar: {extra:?}"
    );
    for word in ["module", "query", "require", "return", "outside_scope"] {
        assert!(mill.contains(word), "{word}");
    }
}

#[test]
fn mill_keyword_list_is_sorted_without_duplicates() {
    let list = mill_keywords(&read("web/mill.js"));
    let mut sorted = list.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(list, sorted, "keep the KEYWORDS lines sorted and unique");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `node --test web/tests/mill.test.js`
Expected: FAIL; subtests 1–18 still print `ok`, subtests 19–35 print `not ok` with errors such as `mill.byteToIndex is not a function` and `mill.tokenizeFr is not a function`.

Run: `cargo test -p xtask --offline --test mill_keywords`
Expected: FAIL with `test result: FAILED. 1 passed; 2 failed`; both failures panic with `web/mill.js has // KEYWORDS-BEGIN`.

- [ ] **Step 3: Add the editor to `web/mill.js`**

Make these six edits to `web/mill.js`; each quoted text occurs exactly once.

1. Put the highlighting helpers and the keyword list in front of the export gate.

Find this line:

```js
  if (typeof document === "undefined") {
```

Replace it with the helpers followed by the same line:

```js
  // ------------------------------------------------------ editor highlighting

  // Every quoted terminal of grammar.ebnf made only of [a-z_] and longer than
  // one character. xtask/tests/mill_keywords.rs fails when this drifts.
  var KEYWORDS = [
    // KEYWORDS-BEGIN
    "acquired_by",
    "activate",
    "active_while",
    "admissibility",
    "alias",
    "alternative",
    "amendment",
    "and",
    "artifact",
    "as",
    "as_to",
    "ascending",
    "assert",
    "assume",
    "assuming",
    "at",
    "attaches",
    "authenticate",
    "authority",
    "automatic",
    "bearer",
    "bounds",
    "burden_of_persuasion",
    "by",
    "calc",
    "calendar_days",
    "cardinality",
    "choice_space",
    "choices",
    "citation",
    "claimant",
    "clause",
    "competence",
    "competent_when",
    "conflict_doctrine",
    "constitute",
    "constitutive",
    "constitutive_effect",
    "content",
    "counted_days",
    "court",
    "court_finding",
    "create",
    "days",
    "decides",
    "decision",
    "decision_rule",
    "defeat",
    "defects",
    "defines",
    "derive",
    "descending",
    "digest",
    "discharge_by",
    "does_not_establish",
    "due",
    "duration",
    "duty",
    "each",
    "effect",
    "effective",
    "effective_at",
    "enforcement",
    "entity",
    "establish",
    "establishes",
    "every",
    "evidence_type",
    "exclude_from_count",
    "exercise_by",
    "exercises",
    "exists",
    "fn",
    "for",
    "for_all",
    "from",
    "fuel",
    "goal",
    "holder",
    "hours",
    "import",
    "in",
    "interpretation_family",
    "interpretations",
    "judgment",
    "judgments",
    "jurisdiction",
    "kind",
    "legal_act",
    "lost_by",
    "make",
    "may_include",
    "minutes",
    "module",
    "must_consider",
    "must_not_consider",
    "nomination",
    "not",
    "observation",
    "occupied_by",
    "office",
    "on",
    "options",
    "or",
    "order_by",
    "otherwise",
    "output",
    "outside_scope",
    "performer",
    "performers",
    "physical_effect",
    "power",
    "prescriptive",
    "primary_authority",
    "proposition",
    "provision_intervals",
    "purport_to",
    "purports_to",
    "query",
    "rank",
    "reason",
    "recipient",
    "record",
    "record_time",
    "record_type",
    "recorded_at",
    "reject",
    "request",
    "require",
    "requires",
    "retrieved_at",
    "return",
    "returns",
    "review",
    "rule",
    "scenario",
    "seconds",
    "select",
    "selector",
    "source",
    "source_manifest",
    "source_snapshot",
    "standard",
    "subject",
    "subject_to",
    "suspend",
    "suspended_by",
    "terminate",
    "then",
    "to",
    "transaction",
    "type",
    "under",
    "unique",
    "using",
    "valid_time",
    "validity",
    "verify",
    "version",
    "violation_when",
    "when",
    "where",
    "working_days",
    // KEYWORDS-END
  ];
  var KEYWORD_SET = Object.create(null);
  KEYWORDS.forEach(function (word) { KEYWORD_SET[word] = true; });
  var DURATION_UNITS = Object.create(null);
  ["days", "working_days", "counted_days", "calendar_days", "hours", "minutes", "seconds"].forEach(function (unit) {
    DURATION_UNITS[unit] = true;
  });
  var FR_OPERATORS = ["->", "=>", "==", "!=", "<=", ">=", "..", "{", "}", "(", ")", "[", "]", ",", ".", ":", ";", "!", "=", "<", ">", "+", "-", "*", "/", "|"];
  var FR_DATE = /\d{4}-\d{2}-\d{2}(?:T\d{2}:\d{2}:\d{2}(?:Z|[+-]\d{2}:\d{2})?)?/y;

  function isDigit(c) {
    return c >= "0" && c <= "9";
  }

  function isIdentStart(c) {
    return (c >= "a" && c <= "z") || (c >= "A" && c <= "Z") || c === "_";
  }

  /** Append a segment, merging it into the previous one when the class is the same. */
  function pushSegment(out, text, cls) {
    if (!text) return;
    var last = out[out.length - 1];
    if (last && last.cls === cls) last.text += text;
    else out.push({ text: text, cls: cls });
  }

  /** End of the number, date, or date-time starting at `i`, as the lexer reads it. */
  function frNumberEnd(text, i) {
    FR_DATE.lastIndex = i;
    var date = FR_DATE.exec(text);
    if (date) return i + date[0].length;
    var j = i;
    while (j < text.length && isDigit(text.charAt(j))) j += 1;
    if (text.charAt(j) === "." && isDigit(text.charAt(j + 1))) {
      j += 1;
      while (j < text.length && isDigit(text.charAt(j))) j += 1;
    }
    return j;
  }

  /** The operator or punctuation at `i`, longest first (`+inf` and `-inf` included). */
  function frOperator(text, i) {
    var sign = text.charAt(i);
    if ((sign === "+" || sign === "-") && text.slice(i + 1, i + 4) === "inf") {
      var after = text.charAt(i + 4);
      if (!isIdentStart(after) && !isDigit(after)) return sign + "inf";
    }
    for (var k = 0; k < FR_OPERATORS.length; k++) {
      if (text.startsWith(FR_OPERATORS[k], i)) return FR_OPERATORS[k];
    }
    return "";
  }

  /**
   * Highlight segments `{ text, cls }` for `.fr` source, read the way the
   * fidryn-syntax lexer reads it: grammar keywords; capitalized names after
   * `->`, `:`, or `<` and module paths after `module` or `import` as types;
   * strings; numbers, dates, durations, `true`, `false`, and infinities as
   * literals; comments; punctuation. Joining the texts gives `text` back.
   */
  function tokenizeFr(text) {
    var out = [];
    var i = 0;
    var prev = "";
    var path = 0; // 1: after `module` or `import`; 2: after a path name; 3: after a path dot
    while (i < text.length) {
      var start = i;
      var c = text.charAt(i);
      var cls = "";
      if (c === " " || c === "\t" || c === "\r" || c === "\n") {
        while (i < text.length && " \t\r\n".indexOf(text.charAt(i)) >= 0) i += 1;
        if (path > 1) path = 0;
      } else if (c === "/" && text.charAt(i + 1) === "/") {
        var eol = text.indexOf("\n", i);
        i = eol < 0 ? text.length : eol;
        cls = "tk-co";
        if (path > 1) path = 0;
      } else if (c === "\"") {
        i += 1;
        while (i < text.length) {
          var d = text.charAt(i);
          i += d === "\\" ? 2 : 1;
          if (d === "\"") break;
        }
        i = Math.min(i, text.length);
        cls = "tk-st";
        prev = "str";
        path = 0;
      } else if (isDigit(c)) {
        i = frNumberEnd(text, i);
        cls = "tk-nu";
        prev = "num";
        path = 0;
      } else if (isIdentStart(c)) {
        while (i < text.length && (isIdentStart(text.charAt(i)) || isDigit(text.charAt(i)))) i += 1;
        var word = text.slice(start, i);
        if (path === 1 || path === 3) {
          cls = "tk-ty";
          path = 2;
        } else {
          path = 0;
          if (word === "true" || word === "false") cls = "tk-nu";
          else if (DURATION_UNITS[word] && prev === "num") cls = "tk-nu";
          else if (KEYWORD_SET[word]) cls = "tk-kw";
          else if (c >= "A" && c <= "Z" && (prev === "->" || prev === ":" || prev === "<")) cls = "tk-ty";
          if (cls === "tk-kw" && (word === "module" || word === "import")) path = 1;
        }
        prev = "word";
      } else {
        var op = frOperator(text, i);
        if (op) {
          i += op.length;
          if (path === 2 && op === ".") {
            cls = "tk-ty";
            path = 3;
          } else {
            cls = op === "+inf" || op === "-inf" ? "tk-nu" : "tk-pu";
            path = 0;
          }
          prev = op;
        } else {
          i += text.codePointAt(i) > 0xffff ? 2 : 1;
          prev = "";
          path = 0;
        }
      }
      pushSegment(out, text.slice(start, i), cls);
    }
    return out;
  }

  /**
   * Highlight segments for JSON: object keys as types, strings, numbers,
   * `true`, `false`, and `null` as literals, and punctuation. Text that is not
   * JSON stays plain, so joining the texts always gives `text` back.
   */
  function tokenizeJson(text) {
    var out = [];
    var i = 0;
    while (i < text.length) {
      var start = i;
      var c = text.charAt(i);
      var cls = "";
      if (" \t\r\n".indexOf(c) >= 0) {
        while (i < text.length && " \t\r\n".indexOf(text.charAt(i)) >= 0) i += 1;
      } else if (c === "\"") {
        i += 1;
        while (i < text.length && text.charAt(i) !== "\n") {
          var d = text.charAt(i);
          i += d === "\\" ? 2 : 1;
          if (d === "\"") break;
        }
        i = Math.min(i, text.length);
        var next = i;
        while (next < text.length && " \t\r\n".indexOf(text.charAt(next)) >= 0) next += 1;
        cls = text.charAt(next) === ":" ? "tk-ty" : "tk-st";
      } else if (c === "-" || isDigit(c)) {
        JSON_NUMBER.lastIndex = i;
        var number = JSON_NUMBER.exec(text);
        i += number ? number[0].length : 1;
        cls = number ? "tk-nu" : "";
      } else if (text.startsWith("true", i) || text.startsWith("null", i) || text.startsWith("false", i)) {
        i += c === "f" ? 5 : 4;
        cls = "tk-nu";
      } else if ("{}[],:".indexOf(c) >= 0) {
        i += 1;
        cls = "tk-pu";
      } else {
        i += text.codePointAt(i) > 0xffff ? 2 : 1;
      }
      pushSegment(out, text.slice(start, i), cls);
    }
    return out;
  }

  /**
   * Split `segments` at the edges of `ranges` (`{ start, end, cls }` in string
   * indices) and add each range's class to the text inside it. Overlapping
   * ranges add both classes; empty ranges change nothing.
   */
  function applyRanges(segments, ranges) {
    var live = (ranges || []).filter(function (r) { return r && r.end > r.start; });
    if (live.length === 0) return segments.slice();
    var cuts = [];
    live.forEach(function (r) { cuts.push(r.start, r.end); });
    cuts.sort(function (a, b) { return a - b; });
    var out = [];
    var pos = 0;
    segments.forEach(function (seg) {
      var from = pos;
      var to = pos + seg.text.length;
      var edges = [from];
      cuts.forEach(function (cut) {
        if (cut > from && cut < to && edges[edges.length - 1] !== cut) edges.push(cut);
      });
      edges.push(to);
      for (var k = 0; k + 1 < edges.length; k++) {
        var a = edges[k];
        var b = edges[k + 1];
        var classes = seg.cls ? [seg.cls] : [];
        live.forEach(function (r) {
          if (r.start <= a && b <= r.end && classes.indexOf(r.cls) < 0) classes.push(r.cls);
        });
        if (b > a) out.push({ text: seg.text.slice(a - from, b - from), cls: classes.join(" ") });
      }
      pos = to;
    });
    return out;
  }

  /** HTML for highlight segments; every segment's text is escaped. */
  function segmentsToHtml(segments) {
    return segments.map(function (seg) {
      return seg.cls ? "<span class=\"" + esc(seg.cls) + "\">" + esc(seg.text) + "</span>" : esc(seg.text);
    }).join("");
  }

  /**
   * What a key press in the editor does. `escaped` is the one-shot flag set by
   * Escape: the next Tab or Shift-Tab then moves focus instead of indenting.
   * Modifier keys keep the flag; any other key clears it. Returns the action
   * (`indent`, `outdent`, `newline`, `leave`, or "" for the browser default)
   * and the flag's next value.
   */
  function editorKey(key, mods, escaped) {
    var m = mods || {};
    if (key === "Escape") return { action: "", escaped: true };
    if (key === "Shift" || key === "Control" || key === "Alt" || key === "Meta") {
      return { action: "", escaped: Boolean(escaped) };
    }
    if (key === "Tab" && !m.ctrl && !m.meta && !m.alt) {
      if (escaped) return { action: "leave", escaped: false };
      return { action: m.shift ? "outdent" : "indent", escaped: false };
    }
    if (key === "Enter" && !m.shift && !m.ctrl && !m.meta && !m.alt && !m.composing) {
      return { action: "newline", escaped: false };
    }
    return { action: "", escaped: false };
  }

  if (typeof document === "undefined") {
```

2. Export the new helpers.

Find:

```js
    module.exports = {
      esc: esc,
      isRfc3339: isRfc3339,
      valueText: valueText,
      requestText: requestText,
      loadState: loadState,
      postJson: postJson
    };
```

Replace it with:

```js
    module.exports = {
      esc: esc,
      isRfc3339: isRfc3339,
      valueText: valueText,
      requestText: requestText,
      loadState: loadState,
      postJson: postJson,
      byteToIndex: byteToIndex,
      lineCol: lineCol,
      tokenizeFr: tokenizeFr,
      tokenizeJson: tokenizeJson,
      applyRanges: applyRanges,
      segmentsToHtml: segmentsToHtml,
      KEYWORDS: KEYWORDS,
      editorKey: editorKey
    };
```

3. Install the editor when the page starts.

Find:

```js
    wireEvents();
```

Replace it with:

```js
    wireEvents();
    installEditor();
```

4. Replace the plain-textarea status with the editor's.

Find:

```js
  /** Called after the editor's text, buffer, or caret changes and after new diagnostics. */
  function editorChanged() {
    updateBufferStatus();
  }

  function updateBufferStatus() {
    var text = "";
    var mood = "";
    if (state.buffer === "module" && diagnostics.source === state.module) {
      var n = diagnostics.list.length;
      text = n === 0 ? "No problems" : n === 1 ? "1 problem" : n + " problems";
      mood = n === 0 ? "ok" : "err";
    } else if (state.buffer === "template") {
      text = "Used by Render";
    }
    clear(el.bufferStatus);
    el.bufferStatus.appendChild(h("span", { className: "mill-status-main", text: text }));
    el.bufferStatus.setAttribute("data-state", mood);
  }
```

Replace it with:

```js
  /** Called after the editor's text, buffer, or caret changes and after new diagnostics. */
  function editorChanged(reason) {
    if (!state) return;
    if (reason === "caret") {
      updatePop();
      return;
    }
    if (reason === "load") {
      queryKey = null;
      scheduleCheck(0);
    } else if (reason === "edit" && state.buffer === "module") {
      scheduleCheck(AUTO_CHECK_MS);
    }
    paintEditor();
    updateBufferStatus();
  }

  function updateBufferStatus() {
    var text = "";
    var mood = "";
    if (state.buffer === "module") {
      if (checkTimer || checkInFlight) {
        text = "Checking…";
      } else if (diagnostics.source === state.module) {
        var n = diagnostics.list.length;
        text = n === 0 ? "No problems" : n === 1 ? "1 problem" : n + " problems";
        mood = n === 0 ? "ok" : "err";
      }
    } else if (state.buffer === "case") {
      var parsed = caseResult();
      if (parsed.ok) {
        text = state.case.trim() === "" ? "Empty: sends {}" : "Valid JSON";
        mood = "ok";
      } else {
        text = "Line " + parsed.line + ", column " + parsed.col + ": " + parsed.message;
        mood = "err";
      }
    } else {
      text = "Used by Render";
    }
    clear(el.bufferStatus);
    el.bufferStatus.appendChild(h("span", { className: "mill-status-main", text: text }));
    if (editorFocused) el.bufferStatus.appendChild(h("span", { className: "mill-status-hint", text: LEAVE_HINT }));
    el.bufferStatus.setAttribute("data-state", mood);
  }
```

5. Highlight the JSON result view (every token is escaped by `segmentsToHtml`).

Find:

```js
    body.textContent = source;
```

Replace it with:

```js
    body.innerHTML = segmentsToHtml(lang === "json" ? tokenizeJson(source) : [{ text: source, cls: "" }]);
```

6. Append the editor section before the closing `})();` of the file.

Find:

```js
    document.body.removeChild(area);
    if (active && active.focus) active.focus();
    return ok;
  }
})();
```

Replace it with:

```js
    document.body.removeChild(area);
    if (active && active.focus) active.focus();
    return ok;
  }

  // ------------------------------------------------------------------ editor

  var AUTO_CHECK_MS = 700;
  var LEAVE_HINT = "Esc then Tab to leave the editor";
  var metrics = { line: 22, top: 12 };
  var gutterLines = null;
  var gutterKey = "";
  var lineMarks = {};
  var popLine = 0;
  var pointerLine = 0;
  var editorFocused = false;
  var escaped = false;
  var checkTimer = 0;
  var checkSeq = 0;
  var checkInFlight = false;
  var queryKey = null;
  var caseCache = { text: null, result: null };

  /** Put the highlighted layer, the line numbers, and the key handling on the textarea. */
  function installEditor() {
    gutterLines = h("div", { className: "mill-gutter-in" });
    el.gutter.appendChild(gutterLines);
    el.editorWrap.setAttribute("data-hl", "");
    el.editor.addEventListener("scroll", syncScroll);
    el.editor.addEventListener("keydown", onEditorKey);
    el.editor.addEventListener("keyup", updatePop);
    el.editor.addEventListener("click", updatePop);
    el.editor.addEventListener("mousemove", onPointer);
    el.editor.addEventListener("mouseleave", function () {
      pointerLine = 0;
      updatePop();
    });
    el.editor.addEventListener("focus", function () {
      editorFocused = true;
      if (state) updateBufferStatus();
      updatePop();
    });
    el.editor.addEventListener("blur", function () {
      editorFocused = false;
      escaped = false;
      if (state) updateBufferStatus();
      updatePop();
    });
    document.addEventListener("selectionchange", function () {
      if (document.activeElement === el.editor) updatePop();
    });
    window.addEventListener("resize", function () {
      measure();
      syncScroll();
    });
    measure();
  }

  /** Line height and top padding of the textarea; the overlay and the gutter use the same. */
  function measure() {
    var style = window.getComputedStyle(el.editor);
    metrics.line = parseFloat(style.lineHeight) || metrics.line;
    metrics.top = parseFloat(style.paddingTop) || 0;
  }

  /** Highlight the current buffer, mark its problems, and number its lines. */
  function paintEditor() {
    var text = el.editor.value;
    var segments = state.buffer === "module" ? tokenizeFr(text)
      : state.buffer === "case" ? tokenizeJson(text) : [{ text: text, cls: "" }];
    var ranges = [];
    lineMarks = {};
    problems(text).forEach(function (p) {
      ranges.push({ start: p.start, end: p.end, cls: "mill-sq" });
      var line = lineCol(text, p.start).line;
      (lineMarks[line] = lineMarks[line] || []).push(p);
    });
    // A final newline needs a character after it, or the layer is a line short.
    el.hl.innerHTML = segmentsToHtml(applyRanges(segments, ranges)) + (text.slice(-1) === "\n" ? " " : "");
    paintGutter(text.split("\n").length);
    popLine = 0;
    syncScroll();
  }

  function caseResult() {
    if (caseCache.text !== state.case) caseCache = { text: state.case, result: parseCase(state.case) };
    return caseCache.result;
  }

  /** Problems to mark in the current buffer: `{ start, end, code, message }` in string indices. */
  function problems(text) {
    if (state.buffer === "module") {
      if (diagnostics.source !== text) return [];
      return diagnostics.list.map(function (d) {
        var span = isObject(d.primary_span) ? d.primary_span : null;
        if (!span || typeof span.start !== "number") return null;
        var start = byteToIndex(text, span.start);
        var end = byteToIndex(text, typeof span.end === "number" ? span.end : span.start);
        var range = visibleRange(text, start, Math.max(start, end));
        return { start: range.start, end: range.end, code: str(d.code), message: str(d.message) };
      }).filter(function (p) { return p !== null; });
    }
    if (state.buffer === "case") {
      var parsed = caseResult();
      if (parsed.ok) return [];
      var at = visibleRange(text, parsed.index, parsed.index);
      return [{ start: at.start, end: at.end, code: "JSON", message: parsed.message }];
    }
    return [];
  }

  /** Widen an empty span to the character at it, or the one before it at a line end, so it can be underlined. */
  function visibleRange(text, start, end) {
    if (end > start) return { start: start, end: end };
    if (start < text.length && text.charAt(start) !== "\n") {
      return { start: start, end: start + (text.codePointAt(start) > 0xffff ? 2 : 1) };
    }
    if (start > 0 && text.charAt(start - 1) !== "\n") {
      var low = text.charCodeAt(start - 1);
      return { start: start - (low >= 0xdc00 && low <= 0xdfff && start > 1 ? 2 : 1), end: start };
    }
    return { start: start, end: start };
  }

  function paintGutter(count) {
    var key = count + "|" + Object.keys(lineMarks).join(",");
    if (key === gutterKey) return;
    gutterKey = key;
    clear(gutterLines);
    for (var n = 1; n <= count; n++) {
      gutterLines.appendChild(h("div", { className: lineMarks[n] ? "mill-ln has-diag" : "mill-ln", text: String(n) }));
    }
  }

  /** Move the highlighted layer and the line numbers with the textarea's scroll. */
  function syncScroll() {
    var x = el.editor.scrollLeft;
    var y = el.editor.scrollTop;
    el.hl.style.transform = "translate(" + -x + "px, " + -y + "px)";
    gutterLines.style.transform = "translateY(" + -y + "px)";
    updatePop();
  }

  /** Show the problems of the line under the pointer, or else of the caret's line, right under that line. */
  function updatePop() {
    var line = pointerLine && lineMarks[pointerLine] ? pointerLine : 0;
    if (!line && editorFocused) {
      var caret = lineCol(el.editor.value, el.editor.selectionStart).line;
      if (lineMarks[caret]) line = caret;
    }
    var below = metrics.top + line * metrics.line - el.editor.scrollTop;
    if (!line || below < 0 || below > el.editor.clientHeight) {
      el.diagPop.hidden = true;
      popLine = 0;
      return;
    }
    if (line !== popLine) {
      clear(el.diagPop);
      lineMarks[line].forEach(function (p) {
        el.diagPop.appendChild(h("p", null, [h("code", { text: p.code }), p.message]));
      });
      popLine = line;
    }
    el.diagPop.hidden = false;
    var height = el.diagPop.offsetHeight;
    var above = below - metrics.line - height;
    el.diagPop.style.top = (below + height > el.editor.clientHeight && above >= 0 ? above : below) + "px";
  }

  function onPointer(event) {
    var rect = el.editor.getBoundingClientRect();
    var y = event.clientY - rect.top - metrics.top + el.editor.scrollTop;
    var line = y < 0 ? 0 : Math.floor(y / metrics.line) + 1;
    if (line === pointerLine) return;
    pointerLine = line;
    updatePop();
  }

  function onEditorKey(event) {
    var step = editorKey(event.key, {
      shift: event.shiftKey,
      ctrl: event.ctrlKey,
      meta: event.metaKey,
      alt: event.altKey,
      composing: event.isComposing || event.keyCode === 229
    }, escaped);
    escaped = step.escaped;
    if (step.action === "indent") indent();
    else if (step.action === "outdent") outdent();
    else if (step.action === "newline") newline();
    else return;
    event.preventDefault();
  }

  function indentUnit() {
    return state.buffer === "module" ? "    " : "  ";
  }

  function lineStart(text, index) {
    return text.lastIndexOf("\n", index - 1) + 1;
  }

  /** The selection and the whole lines it touches (`from`..`to`, without the last newline). */
  function selectedLines() {
    var text = el.editor.value;
    var start = el.editor.selectionStart;
    var end = el.editor.selectionEnd;
    var last = end > start && text.charAt(end - 1) === "\n" ? end - 1 : end;
    var eol = text.indexOf("\n", last);
    return { text: text, start: start, end: end, from: lineStart(text, start), to: eol < 0 ? text.length : eol };
  }

  function indent() {
    var unit = indentUnit();
    var sel = selectedLines();
    if (sel.text.slice(sel.start, sel.end).indexOf("\n") < 0) {
      var pad = unit.slice((sel.start - sel.from) % unit.length);
      replaceRange(sel.start, sel.end, pad, sel.start + pad.length, sel.start + pad.length);
      return;
    }
    var lines = sel.text.slice(sel.from, sel.to).split("\n");
    var added = 0;
    var out = lines.map(function (line) {
      if (!line) return line;
      added += unit.length;
      return unit + line;
    }).join("\n");
    replaceRange(sel.from, sel.to, out, sel.start + (lines[0] ? unit.length : 0), sel.end + added);
  }

  function outdent() {
    var unit = indentUnit();
    var sel = selectedLines();
    var lines = sel.text.slice(sel.from, sel.to).split("\n");
    var firstCut = 0;
    var removed = 0;
    var out = lines.map(function (line, i) {
      var cut = line.charAt(0) === "\t" ? 1 : Math.min(/^ */.exec(line)[0].length, unit.length);
      if (i === 0) firstCut = cut;
      removed += cut;
      return line.slice(cut);
    }).join("\n");
    if (removed === 0) return;
    var start = Math.max(sel.from, sel.start - firstCut);
    replaceRange(sel.from, sel.to, out, start, Math.max(start, sel.end - removed));
  }

  /** Enter keeps the current line's indentation. */
  function newline() {
    var text = el.editor.value;
    var start = el.editor.selectionStart;
    var insert = "\n" + /^[ \t]*/.exec(text.slice(lineStart(text, start), start))[0];
    replaceRange(start, el.editor.selectionEnd, insert, start + insert.length, start + insert.length);
  }

  /** Replace `from`..`to` with `text` as one undoable edit, then select `selStart`..`selEnd`. */
  function replaceRange(from, to, text, selStart, selEnd) {
    el.editor.setSelectionRange(from, to);
    var done = false;
    try {
      done = document.execCommand("insertText", false, text);
    } catch (err) {
      done = false;
    }
    if (!done) {
      el.editor.setRangeText(text, from, to, "end");
      el.editor.dispatchEvent(new Event("input", { bubbles: true }));
    }
    el.editor.setSelectionRange(selStart, selEnd);
  }

  function scheduleCheck(delay) {
    clearTimeout(checkTimer);
    checkTimer = setTimeout(autoCheck, delay);
  }

  /** Check the module in the background. Stale answers are dropped; auto-checks never enter History. */
  function autoCheck() {
    checkTimer = 0;
    var source = state.module;
    var seq = ++checkSeq;
    var version = diagnostics.version;
    checkInFlight = true;
    updateQueryNames();
    updateBufferStatus();
    postJson("/api/check", { source: source }).then(function (res) {
      if (seq !== checkSeq) return;
      checkInFlight = false;
      if (res.network) {
        networkDown();
      } else {
        serverAnswered();
        if (source === state.module && diagnostics.version === version) noteDiagnostics(source, res.data);
      }
      updateBufferStatus();
    });
  }

  /** Offer the module's query names as suggestions for the Query field. */
  function updateQueryNames() {
    var names = queryNames(state.module);
    var key = names.join("\n");
    if (key === queryKey) return;
    queryKey = key;
    clear(el.queryNames);
    names.forEach(function (name) { el.queryNames.appendChild(h("option", { value: name })); });
  }

  /** Names declared with `query` in `source`, ignoring comments and strings. */
  function queryNames(source) {
    var code = tokenizeFr(source).map(function (seg) {
      return /(^| )tk-(co|st)( |$)/.test(seg.cls) ? " " : seg.text;
    }).join("");
    var decl = /(^|[^A-Za-z0-9_])query\s+(?:automatic\s+)?([A-Za-z_][A-Za-z0-9_]*)/g;
    var names = [];
    var m;
    while ((m = decl.exec(code)) !== null) {
      if (names.indexOf(m[2]) < 0) names.push(m[2]);
    }
    return names;
  }
})();
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `node --check web/mill.js`
Expected: no output, exit status 0.

Run: `node --test web/tests/mill.test.js`
Expected: PASS; all 35 subtests print `ok` and the summary shows `# fail 0`.

Run: `cargo test -p xtask --offline --test mill_keywords --test mill_page`
Expected: PASS (`test result: ok. 3 passed` and `test result: ok. 6 passed`).

Run: `cargo test -p fidryn-cli --offline --lib ui::`
Expected: PASS (the embedded script changed; the route tests still pass).

Run: `cargo clippy --offline -p xtask --tests -- -D warnings`
Expected: `Finished` with no warnings.

- [ ] **Step 5: Use the editor**

Run `cargo run -p fidryn-cli --offline -- ui --no-open`, open `http://127.0.0.1:8751`, and check at 1440px, 820px, and 390px, in both themes:
  - `require-gate` is highlighted: `module`, `version`, `query`, `require`, `return` in rubric; `Programs.RequireGate` and `Int` in the type blue; the strings green; `2026-09-17`, `true`, `7` in the number color; line numbers in the gutter line up with the text;
  - select text with the mouse and scroll the trust sample to its end and sideways: the selection sits exactly on the glyphs, and the gutter and the highlighting move with the textarea (a debugging tip: `document.getElementById("editor").style.webkitTextFillColor = "rgba(230,0,0,.5)"` in the console shows the real textarea text; it must overlap the highlighted text exactly);
  - put the caret at the end of the `query q() -> Int { … }` line and press Enter: the new line starts with four spaces; Tab moves to the next multiple of four; Shift-Tab takes it back; select two lines and press Tab and Shift-Tab; Cmd/Ctrl+Z undoes each step;
  - press Escape and then Tab: focus moves to the Query field (the hint `Esc then Tab to leave the editor` shows beside the status while the editor has focus);
  - inside the module add the line `// § 4.4 é 𝔽` and under it `colour blue`, then stop typing: within a second `colour` gets a wavy underline, the gutter shows a red mark on its line, the status reads `1 problem`, and with the caret on that line a box right under it reads `` E100 unknown declaration `colour` ``; nothing is added to History;
  - Case JSON: remove a comma and the status reads `Line …, column …: …` with the underline at that spot; the Query field suggests `q` and `r`;
  - the JSON result view is highlighted; at 390px the editor text is 16px (no zoom on focus in iOS Safari) and the page never scrolls sideways.

- [ ] **Step 6: Commit**

```bash
git add web/mill.js web/tests/mill.test.js xtask/tests/mill_keywords.rs
git -c commit.gpgsign=false commit -m "feat(mill): highlighting editor with inline diagnostics"
```

### Task 17: Docs and the decision record

**Files:**
- Modify: `docs/mill.md` (the section `## The HTML page`, and two rows of the Routes table)
- Modify: `docs/contributing.md` (new `## Website` section after `## Checks`)
- Modify: `docs/outcomes.md` (the "What it is not." paragraph becomes a blockquote, so it renders as a callout)
- Modify: `site/README.md` (full rewrite)
- Create: `.agents/adr/0001-rust-site-generator.md`
- Modify: `.agents/ADR.md` (index row), `.agents/ARCHITECTURE.md` (top-level table)
- Regenerate: `site/docs/mill.*`, `site/docs/contributing.*`, `site/search-index.json`, `site/llms-full.txt` (by `cargo xtask site`)

**Interfaces:**
- Consumes: `cargo xtask site` and `--check` (Tasks 4–12); the finished mill page (Tasks 14–16), which the new "The HTML page" text describes.
- Produces: nothing other tasks call.

- [ ] **Step 1: Rewrite "The HTML page" in `docs/mill.md`**

Replace everything from the line `## The HTML page` up to, but not including, the line `## See also` with:

````markdown
## The HTML page

`GET /` serves `web/index.html`, which loads `/assets/fidryn.css` (the
site's design system), `/assets/mill.css`, and `/assets/mill.js`. Those
files, the fonts, and the samples are compiled into the binary, and the
page sends requests only to 127.0.0.1. The response carries a strict
Content-Security-Policy, so the page runs no inline script.

The page has three columns on a wide screen and one column on a phone:

1. **Header.** The Fidryn monogram, `mill`, the health status
   (`localhost 127.0.0.1 · ok`, or a warning when the server cannot be
   reached), the notice "Live filing is not available from the UI", a
   link to this guide, and the theme toggle. The theme follows the
   system until you choose light or dark.
2. **Samples and history.** Samples come from `GET /api/samples`, each
   with the outcome kind its default action produces: `require-gate`,
   `late-payment`, and three runs of the trust fixture (open
   eligibility, court selects I2, one certificate). Choosing a sample
   fills the module, case, query, and both clocks, then runs its
   default action; if you have unsaved edits, the rail asks before
   replacing them. The first visit loads `require-gate` without
   running it. History lists the last
   20 actions of this session. Choosing one reopens its result, and
   "Restore inputs" puts its inputs back.
3. **Editor.** A segmented control switches between Module, Case JSON,
   and Template. The editor has line numbers and syntax highlighting;
   Tab and Shift-Tab indent and outdent, and Enter keeps the
   indentation; press Escape, then Tab, to move focus out of the editor. About 700 ms after you stop typing in the module, the
   page posts it to `/api/check`: each diagnostic gets a wavy underline
   and a mark in the gutter, and its message appears under the line
   while the cursor or pointer is on it. Case JSON is checked as you
   type, with the line and column of the first error.
4. **Query, clocks, and actions.** The query field suggests the
   module's `query` names. `validAt` and `knownAt` must be RFC 3339.
   `Check`, `Run`, `Explore`, and `Render` post to the routes above.
   Ctrl or Cmd with Enter runs; adding Shift explores.
5. **Result.** Run and Explore results have three views: Opinion (the
   `opinion` sentences set as a printed page, with the execution mode,
   source trust, module, and `asOf`), Table (every field of the outcome
   and the report envelope), and JSON (the raw report, with Copy).
   Check shows `ok` or the diagnostics, each with a button that jumps to
   its line. Render shows the rendered text. Engine errors, invalid
   input, and an unreachable server are shown in place of a result.

Inputs, the chosen sample, and the chosen result view are kept in the
browser's local storage, so a reload keeps your work. `Explore` sends
the same body as `Run` and no separate `bounds` field; put finite
completions on the case JSON.

````

Then, in the Routes table near the top of `docs/mill.md`, insert these two rows directly after the `/assets/fidryn.css` row:

```markdown
| `GET` | `/assets/mill.css` | none | `web/mill.css`, the mill's own styles (`text/css; charset=utf-8`) |
| `GET` | `/assets/mill.js` | none | `web/mill.js`, the mill app (`text/javascript; charset=utf-8`) |
```

- [ ] **Step 2: Add a Website section to `docs/contributing.md` and mark the Outcomes callout**

Insert after the line `Do not weaken no-false-determinacy to make a test pass.` (the last line of `## Checks`), with one blank line before it:

````markdown
## Website

The public site in [`site/`](../site/) is generated from these guides
by the workspace task runner. After editing a guide in `docs/`, run:

```
cargo xtask site
```

and commit the markdown together with the regenerated files under
`site/`. `cargo xtask site --check` fails when the committed site is
stale, and `cargo xtask ci` runs that check. The landing page's
specimen is evaluated by the real interpreter at build time, so a
change in semantics also makes the site stale.

The styles and scripts in `site/assets/` are written by hand and are
shared with the mill, whose page lives in [`web/`](../web/). When Node
is installed, `cargo xtask ci` also runs the JavaScript unit tests in
`web/tests/`; Node is never needed to build or serve anything.
````

In `docs/outcomes.md`, replace the paragraph

```markdown
**What it is not.** Listing one alternative first is not determination.
Hashing the open issues is not covering. Shape-complete `examined ==
total` with empty or fabricated `branches` is not covering.
```

with the same words as a blockquote, which the site styles as a callout:

```markdown
> **What it is not.** Listing one alternative first is not determination.
> Hashing the open issues is not covering. Shape-complete `examined ==
> total` with empty or fabricated `branches` is not covering.
```

- [ ] **Step 3: Confirm the site is now stale**

Run: `cargo xtask site --check`
Expected: FAIL with `site/ is out of date; run \`cargo xtask site\`:` followed by lines including `docs/mill.html differs`, `docs/mill.md differs`, `docs/contributing.html differs`, `docs/contributing.md differs`, `docs/outcomes.html differs`, `docs/outcomes.md differs`, `search-index.json differs`, and `llms-full.txt differs`.

- [ ] **Step 4: Regenerate and verify**

Run: `cargo xtask site && cargo xtask site --check && cargo test -p xtask --offline --test site_output`
Expected: `wrote 26 files under site/`, then `site/ is up to date (26 generated files)`, then `test result: ok`. The `../site/` and `../web/` links in the new section render as `https://github.com/BeeGass/fidryn/tree/main/site` and `…/tree/main/web` (check with `grep -o 'tree/main/[a-z]*' site/docs/contributing.html`).

- [ ] **Step 5: Rewrite `site/README.md`**

Replace the whole file with:

````markdown
# fidryn.onlygass.dev

Static site for Fidryn: the landing page and the learner docs.

## Build

The site is generated by the Rust workspace. From the repository root:

```bash
cargo xtask site          # render docs/*.md and the landing page into site/
cargo xtask site --check  # fail if the committed site is out of date (CI runs this)
```

`docs/*.md` is canonical. Edit a guide there, run `cargo xtask site`,
and commit the markdown and the regenerated files together. The
landing page's specimen is computed by the interpreter at build time,
so a change in semantics also shows up as a stale site.

## Files

| Path | What it is |
| --- | --- |
| `assets/fidryn.css`, `assets/fidryn.js` | Hand-written design system and behavior, shared with the mill |
| `fonts/`, `favicon.svg` | Self-hosted Fraunces and IBM Plex; the F monogram |
| `index.html`, `404.html`, `docs/*.html` | Generated pages |
| `index.md`, `docs/*.md` | Generated markdown mirrors for agents |
| `search-index.json` | Generated search index |
| `llms.txt`, `llms-full.txt`, `sitemap.xml`, `robots.txt` | Generated |
| `vercel.json` | Deploy config: clean URLs, headers, caching |

Never edit generated files by hand; the next run overwrites them.

## Preview

```bash
python3 .agents/scripts/shoot.py serve   # http://127.0.0.1:8752 with clean URLs
```

## Deploy

- Host: Vercel, project root `site`. `vercel.json` sets the framework,
  install, and build commands to null, so a deploy serves the committed
  files as they are.
- Production domain: `fidryn.onlygass.dev` (CNAME `fidryn` →
  `cname.vercel-dns.com` on the `onlygass.dev` zone).

The localhost mill (`fidryn ui`, sources in `web/`) stays on 127.0.0.1.
It is never deployed here, and this host exposes no mill API and no
filing.
````

- [ ] **Step 6: Record the decision**

Create `.agents/adr/0001-rust-site-generator.md`:

````markdown
# 0001: Generate the site with a Rust xtask

Date: 2026-09-29. Status: accepted.

## Context

The learner docs under `site/docs/` came from a Node script
(`site/scripts/build-docs.mjs`, markdown-it). The script had been
reduced to a stub, so the committed HTML was patched by hand and
drifted from `docs/*.md`. The project wants to stay on the Rust stack,
with no Node build step, and the site and the mill were being
redesigned together.

## Decision

`cargo xtask site` renders the whole site (landing page, docs, markdown
mirrors, search index, SEO files) from `docs/*.md`. It uses
pulldown-cmark, the `fidryn-syntax` lexer for `.fr` highlighting, and
`fidryn-cli` to evaluate the landing specimen, so the page shows real
interpreter output. `--check` fails when the committed output is
stale, and `cargo xtask ci` runs it. The design system
(`site/assets/fidryn.css`, `fidryn.js`) is hand-written and shared with
the mill, which embeds it.

## Consequences

- A docs change needs `cargo xtask site` and a commit of the
  regenerated files; CI catches a forgotten regeneration.
- A change in evaluation semantics makes the site stale until it is
  regenerated, which is intended.
- `xtask` now depends on `fidryn-cli` (a heavier build) and
  `pulldown-cmark`.
- Node is optional: it only runs `node --test` over the JavaScript
  helpers.

Alternatives considered: a separate `fidryn-site` crate (no second
consumer, so no gain) and askama templates (new proc-macro
dependencies for four templates).
````

In `.agents/ADR.md`, replace the line `No decisions are recorded yet.` with nothing (delete it and the blank line after it), and add this row after the table header lines:

```markdown
| 0001 | Generate the site with a Rust xtask | [adr/0001-rust-site-generator.md](adr/0001-rust-site-generator.md) | Accepted |
```

In `.agents/ARCHITECTURE.md`, in the `## Top level` table, replace the row

```markdown
| `site/` | Static site for fidryn.onlygass.dev |
```

with

```markdown
| `site/` | Static site for fidryn.onlygass.dev. `cargo xtask site` generates the pages, mirrors, and search index from `docs/*.md`; `assets/`, `fonts/`, `favicon.svg`, and `vercel.json` are hand-written |
| `web/` | The mill page (`index.html`, `mill.css`, `mill.js`), embedded in the `fidryn` binary by `fidryn-cli/src/ui.rs` |
| `xtask/` | Workspace task runner: `test`, `bench`, `ci`, and `site` (see `.agents/adr/0001-rust-site-generator.md`) |
```

and in the row that lists directories with no stated role, remove `` `web/` `` and `` `xtask/` `` from the list.

- [ ] **Step 7: Verify and commit**

Run: `cargo xtask site --check`
Expected: `site/ is up to date (26 generated files)` (the README and `.agents/` files are not generated).

```bash
git add docs/mill.md docs/contributing.md docs/outcomes.md site/README.md site/docs site/search-index.json site/llms-full.txt .agents/adr/0001-rust-site-generator.md .agents/ADR.md .agents/ARCHITECTURE.md
git -c commit.gpgsign=false commit -m "docs: describe the new mill page, the site build, and ADR 0001"
```

### Task 18: Visual QA and final verification

**Files:**
- Create: `.agents/scripts/shoot.py`
- Modify: `.agents/scripts/README.md`
- Modify, only when the review below finds a defect: `site/assets/fidryn.css`, `site/assets/fidryn.js`, `web/mill.css`, `web/mill.js`, `web/index.html` (and the regenerated `site/`)

**Interfaces:**
- Consumes: the generated site (Task 12), the running mill (Tasks 13–16), the page ids and classes in contract C5, C8, C9.
- Produces: `.agents/scripts/shoot.py`, a durable tool: `serve`, `site`, and `mill` subcommands, exit status 1 on sideways scrolling, JavaScript errors, or failed interaction checks.

- [ ] **Step 1: Create the screenshot and audit tool**

Create `.agents/scripts/shoot.py` (standard library only; Python 3.12+; it drives Chrome over the DevTools protocol with its own small WebSocket client):

```python
#!/usr/bin/env python3
"""Screenshot and audit the Fidryn site and the mill in headless Chrome.

Standard library only (Python 3.12+):

    python3 .agents/scripts/shoot.py serve [--port 8752]
    python3 .agents/scripts/shoot.py site --out DIR [--widths 390,820,1440]
                                                    [--themes light,dark]
    python3 .agents/scripts/shoot.py mill --url http://127.0.0.1:8751 --out DIR

`site` serves `site/` with clean URLs, screenshots every page at each width
and theme, fails when a page scrolls sideways or throws a JavaScript error,
and runs keyboard, drawer, theme, and specimen checks. `mill` screenshots a
running `fidryn ui` in its main states and fails on JavaScript errors or
sideways scrolling. Chrome runs with a throwaway profile and background
downloads disabled; the profile is deleted when the run ends.
"""

from __future__ import annotations

import argparse
import base64
import functools
import http.server
import json
import os
import shutil
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
CHROME_FLAGS = [
    "--headless=new",
    "--disable-gpu",
    "--hide-scrollbars",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-component-update",
    "--disable-background-networking",
    "--disable-sync",
    "--disable-default-apps",
    "--disable-extensions",
    "--disable-features=OptimizationGuideModelDownloading,OptimizationHintsFetching,"
    "OptimizationTargetPrediction,OptimizationHints,MediaRouter",
]
SITE_PAGES = [
    "/",
    "/docs/",
    "/docs/getting-started",
    "/docs/language",
    "/docs/cases-and-time",
    "/docs/cli",
    "/docs/mill",
    "/docs/outcomes",
    "/docs/examples",
    "/docs/contributing",
    "/no-such-page",
]
BAD_MODULE = (
    'module Examples.T version "0.1.0" {\n'
    "    jurisdiction Test\n"
    "    effective_at 2026-09-17\n"
    "    outside_scope { complete_instruments }\n"
    "\n"
    "    query q() -> Bool {\n"
    "        colour blue\n"
    "        goal Evaluate { true }\n"
    "    }\n"
    "}\n"
)


@dataclass(frozen=True)
class Ok[T]:
    value: T


@dataclass(frozen=True)
class Err[E]:
    error: E


type Result[T, E] = Ok[T] | Err[E]


@dataclass
class Report:
    """What a run found: screenshots written and failures to act on."""

    shots: list[Path] = field(default_factory=list)
    failures: list[str] = field(default_factory=list)


class WebSocket:
    """A minimal RFC 6455 client: masked text frames out, fragmented frames in."""

    def __init__(self, sock: socket.socket) -> None:
        self._sock = sock
        self._buf = bytearray()

    @classmethod
    def connect(cls, url: str, timeout: float = 60.0) -> WebSocket:
        hostport, _, path = url.removeprefix("ws://").partition("/")
        host, _, port = hostport.partition(":")
        sock = socket.create_connection((host, int(port)), timeout=timeout)
        key = base64.b64encode(os.urandom(16)).decode()
        sock.sendall(
            (
                f"GET /{path} HTTP/1.1\r\nHost: {hostport}\r\nUpgrade: websocket\r\n"
                f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
                "Sec-WebSocket-Version: 13\r\n\r\n"
            ).encode()
        )
        ws = cls(sock)
        status = ws._read_until(b"\r\n\r\n").split(b"\r\n", 1)[0]
        if b" 101 " not in status:
            raise ConnectionError(f"websocket handshake refused: {status!r}")
        return ws

    def close(self) -> None:
        self._sock.close()

    def send_text(self, text: str) -> None:
        self._send_frame(0x1, text.encode())

    def recv_text(self) -> str:
        parts = bytearray()
        while True:
            b0, b1 = self._read_exact(2)
            fin, opcode, size = b0 & 0x80, b0 & 0x0F, b1 & 0x7F
            if size == 126:
                (size,) = struct.unpack("!H", self._read_exact(2))
            elif size == 127:
                (size,) = struct.unpack("!Q", self._read_exact(8))
            mask = self._read_exact(4) if b1 & 0x80 else b""
            payload = self._read_exact(size)
            if mask:
                payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
            if opcode == 0x8:
                raise ConnectionError("websocket closed by Chrome")
            if opcode == 0x9:
                self._send_frame(0xA, payload)
                continue
            if opcode in (0x0, 0x1, 0x2):
                parts += payload
                if fin:
                    return parts.decode()

    def _send_frame(self, opcode: int, payload: bytes) -> None:
        header = bytearray([0x80 | opcode])
        size = len(payload)
        if size < 126:
            header.append(0x80 | size)
        elif size < 65536:
            header.append(0x80 | 126)
            header += struct.pack("!H", size)
        else:
            header.append(0x80 | 127)
            header += struct.pack("!Q", size)
        mask = os.urandom(4)
        header += mask
        self._sock.sendall(
            bytes(header) + bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
        )

    def _read_exact(self, size: int) -> bytes:
        while len(self._buf) < size:
            chunk = self._sock.recv(max(65536, size - len(self._buf)))
            if not chunk:
                raise ConnectionError("websocket closed")
            self._buf += chunk
        data = bytes(self._buf[:size])
        del self._buf[:size]
        return data

    def _read_until(self, marker: bytes) -> bytes:
        while marker not in self._buf:
            chunk = self._sock.recv(65536)
            if not chunk:
                raise ConnectionError("websocket closed during the handshake")
            self._buf += chunk
        end = self._buf.index(marker)
        head = bytes(self._buf[:end])
        del self._buf[: end + len(marker)]
        return head


class Cdp:
    """One Chrome DevTools Protocol session on a page target."""

    def __init__(self, ws: WebSocket) -> None:
        self._ws = ws
        self._next_id = 0
        self.events: list[dict[str, Any]] = []

    def call(self, method: str, **params: object) -> dict[str, Any]:
        self._next_id += 1
        msg_id = self._next_id
        self._ws.send_text(
            json.dumps({"id": msg_id, "method": method, "params": params})
        )
        while True:
            msg: dict[str, Any] = json.loads(self._ws.recv_text())
            if msg.get("id") != msg_id:
                self.events.append(msg)
                continue
            if "error" in msg:
                raise RuntimeError(f"{method}: {msg['error']}")
            result: dict[str, Any] = msg.get("result", {})
            return result

    def run_js(self, expression: str, *, await_promise: bool = False) -> Any:
        """Evaluate one of this script's own fixed expressions in the page."""
        res = self.call(
            "Runtime.evaluate",
            expression=expression,
            returnByValue=True,
            awaitPromise=await_promise,
        )
        if "exceptionDetails" in res:
            detail = res["exceptionDetails"].get("exception", {}).get("description", "")
            raise RuntimeError(f"page script failed: {expression[:70]!r}: {detail}")
        return res.get("result", {}).get("value")

    def key(self, key: str, code: str, key_code: int, text: str = "") -> None:
        down: dict[str, object] = {
            "type": "keyDown",
            "key": key,
            "code": code,
            "windowsVirtualKeyCode": key_code,
        }
        if text:
            down["text"] = text
        self.call("Input.dispatchKeyEvent", **down)
        self.call(
            "Input.dispatchKeyEvent",
            type="keyUp",
            key=key,
            code=code,
            windowsVirtualKeyCode=key_code,
        )

    def take_errors(self) -> list[str]:
        errors = [
            str(
                e["params"]["exceptionDetails"]
                .get("exception", {})
                .get("description", "error")
            )
            for e in self.events
            if e.get("method") == "Runtime.exceptionThrown"
        ]
        self.events.clear()
        return errors


class CleanUrlHandler(http.server.SimpleHTTPRequestHandler):
    """Serve `site/` like Vercel with `cleanUrls`: `/docs/cli` is `docs/cli.html`."""

    extensions_map = {  # noqa: RUF012 (the base class declares it per instance)
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".woff2": "font/woff2",
        ".md": "text/markdown; charset=utf-8",
        ".json": "application/json",
        ".svg": "image/svg+xml",
    }

    def translate_path(self, path: str) -> str:
        base = Path(super().translate_path(path))
        html = base.with_name(base.name + ".html")
        if not base.exists() and html.is_file():
            return str(html)
        return str(base)

    def send_error(
        self, code: int, message: str | None = None, explain: str | None = None
    ) -> None:
        page = Path(self.directory) / "404.html"
        if code == 404 and page.is_file():
            body = page.read_bytes()
            self.send_response(404)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        super().send_error(code, message, explain)

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002 (stdlib signature)
        return


@contextmanager
def serve_site(root: Path, port: int = 0) -> Iterator[str]:
    handler = functools.partial(CleanUrlHandler, directory=str(root))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_address[1]}"
    finally:
        server.shutdown()
        server.server_close()


def fetch_devtools_port(profile: Path, timeout: float) -> Result[int, str]:
    marker = profile / "DevToolsActivePort"
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if marker.is_file():
            first = marker.read_text().splitlines()[:1]
            if first and first[0].isdigit():
                return Ok(int(first[0]))
        time.sleep(0.1)
    return Err(f"Chrome did not open a DevTools port within {timeout:.0f}s")


@contextmanager
def launch_chrome() -> Iterator[int]:
    profile = Path(tempfile.mkdtemp(prefix="fidryn-shoot-"))
    proc = subprocess.Popen(
        [
            CHROME,
            *CHROME_FLAGS,
            "--remote-debugging-port=0",
            f"--user-data-dir={profile}",
            "about:blank",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    try:
        match fetch_devtools_port(profile, timeout=30.0):
            case Ok(value=port):
                yield port
            case Err(error=message):
                raise RuntimeError(message)
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
        for _ in range(10):
            shutil.rmtree(profile, ignore_errors=True)
            if not profile.exists():
                break
            time.sleep(0.5)


def fetch_page_ws(port: int, timeout: float) -> Result[str, str]:
    """The debugger URL of Chrome's first page; retries while Chrome starts up."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(
                f"http://127.0.0.1:{port}/json/list", timeout=5
            ) as resp:
                targets: list[dict[str, Any]] = json.load(resp)
        except OSError:
            time.sleep(0.5)
            continue
        for target in targets:
            if target.get("type") == "page" and "webSocketDebuggerUrl" in target:
                return Ok(str(target["webSocketDebuggerUrl"]))
        time.sleep(0.5)
    return Err(f"Chrome listed no page target within {timeout:.0f}s")


@contextmanager
def open_session(port: int) -> Iterator[Cdp]:
    match fetch_page_ws(port, timeout=90.0):
        case Ok(value=url):
            ws = WebSocket.connect(url)
        case Err(error=message):
            raise RuntimeError(message)
    cdp = Cdp(ws)
    cdp.call("Page.enable")
    cdp.call("Runtime.enable")
    try:
        yield cdp
    finally:
        ws.close()


def wait_for(cdp: Cdp, condition: str, timeout: float = 15.0) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if cdp.run_js(f"Boolean({condition})"):
            return True
        time.sleep(0.1)
    return False


def load(cdp: Cdp, url: str, width: int, theme: str) -> None:
    cdp.call(
        "Emulation.setDeviceMetricsOverride",
        width=width,
        height=900,
        deviceScaleFactor=1,
        mobile=width < 720,
    )
    cdp.call(
        "Emulation.setEmulatedMedia",
        features=[
            {"name": "prefers-color-scheme", "value": theme},
            {"name": "prefers-reduced-motion", "value": "reduce"},
        ],
    )
    cdp.call("Page.navigate", url=url)
    time.sleep(0.2)
    wait_for(cdp, "document.readyState === 'complete'", timeout=30.0)
    cdp.run_js("document.fonts.ready.then(() => true)", await_promise=True)
    time.sleep(0.25)


def save_shot(cdp: Cdp, path: Path, width: int) -> Path:
    metrics = cdp.call("Page.getLayoutMetrics")
    height = min(int(metrics["cssContentSize"]["height"]), 16000)
    shot = cdp.call(
        "Page.captureScreenshot",
        format="png",
        captureBeyondViewport=True,
        clip={"x": 0, "y": 0, "width": width, "height": max(height, 1), "scale": 1},
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(base64.b64decode(shot["data"]))
    return path


OVERFLOW_JS = r"""
(() => {
  const root = document.documentElement;
  const overflow = root.scrollWidth - root.clientWidth;
  const offenders = [];
  if (overflow > 0) {
    for (const el of document.body.querySelectorAll('*')) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.right <= root.clientWidth + 1) continue;
      let clipped = false;
      for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
        const ox = getComputedStyle(a).overflowX;
        if (ox !== 'visible') { clipped = true; break; }
      }
      if (clipped) continue;
      const cls = typeof el.className === 'string' && el.className.trim()
        ? '.' + el.className.trim().split(/\s+/).join('.') : '';
      offenders.push(el.tagName.toLowerCase() + (el.id ? '#' + el.id : '') + cls);
      if (offenders.length >= 5) break;
    }
  }
  return JSON.stringify({overflow, offenders});
})()
"""


def calc_overflow(cdp: Cdp) -> Result[None, str]:
    data = json.loads(str(cdp.run_js(OVERFLOW_JS)))
    if int(data["overflow"]) > 0:
        return Err(
            f"scrolls sideways by {data['overflow']}px ({', '.join(data['offenders'])})"
        )
    return Ok(None)


def slug_for(page: str) -> str:
    name = page.strip("/").replace("/", "-")
    return name or "home"


def check_page(cdp: Cdp, label: str, report: Report) -> None:
    match calc_overflow(cdp):
        case Err(error=message):
            report.failures.append(f"{label}: {message}")
        case Ok():
            pass
    for error in cdp.take_errors():
        report.failures.append(f"{label}: JavaScript error: {error}")


def run_site_checks(cdp: Cdp, base: str, report: Report, out: Path) -> None:
    """Keyboard search, drawer focus handling, theme toggle, and specimen tabs."""

    def expect(ok: bool, what: str) -> None:
        if not ok:
            report.failures.append(f"interaction: {what}")

    load(cdp, f"{base}/docs/outcomes", 1440, "light")
    cdp.key("/", "Slash", 191, "/")
    expect(
        bool(
            cdp.run_js(
                "document.activeElement && document.activeElement.id === 'site-search'"
            )
        ),
        "'/' focuses the search field",
    )
    cdp.call("Input.insertText", text="outc")
    expect(
        wait_for(
            cdp, "document.querySelectorAll('#search-results [role=option]').length > 0"
        ),
        "typing 'outc' lists results",
    )
    report.shots.append(save_shot(cdp, out / "interaction-search.png", 1440))
    cdp.key("ArrowDown", "ArrowDown", 40)
    expect(
        bool(
            cdp.run_js(
                "document.querySelectorAll("
                "'#search-results [aria-selected=true]').length === 1"
            )
        ),
        "ArrowDown keeps exactly one selected result",
    )
    cdp.key("Escape", "Escape", 27)
    expect(
        bool(cdp.run_js("document.getElementById('search-results').hidden")),
        "Escape closes the results",
    )

    load(cdp, f"{base}/docs/outcomes", 390, "light")
    cdp.run_js("document.querySelector('[data-drawer-open]').click()")
    expect(
        wait_for(cdp, "document.body.classList.contains('drawer-open')"),
        "Menu opens the drawer",
    )
    expect(
        bool(
            cdp.run_js(
                "document.getElementById('drawer').contains(document.activeElement)"
            )
        ),
        "opening the drawer moves focus into it",
    )
    report.shots.append(save_shot(cdp, out / "interaction-drawer.png", 390))
    cdp.key("Escape", "Escape", 27)
    expect(
        bool(
            cdp.run_js(
                "!document.body.classList.contains('drawer-open') && "
                "document.activeElement === "
                "document.querySelector('[data-drawer-open]')"
            )
        ),
        "Escape closes the drawer and returns focus to Menu",
    )

    load(cdp, f"{base}/", 1440, "light")
    cdp.run_js("document.querySelector('[data-theme-toggle]').click()")
    expect(
        bool(cdp.run_js("document.documentElement.dataset.theme === 'dark'")),
        "the theme toggle switches to dark",
    )
    cdp.run_js(
        "localStorage.removeItem('fidryn-theme');"
        " delete document.documentElement.dataset.theme"
    )
    cdp.run_js("document.querySelector('[role=tab]').focus()")
    cdp.key("ArrowRight", "ArrowRight", 39)
    expect(
        bool(
            cdp.run_js(
                "(() => { const tabs = [...document.querySelectorAll('[role=tab]')];"
                " const panels = tabs.map(t =>"
                " document.getElementById(t.getAttribute('aria-controls')));"
                " return tabs[1].getAttribute('aria-selected') === 'true'"
                " && !panels[1].hidden"
                " && panels.filter(p => !p.hidden).length === 1; })()"
            )
        ),
        "ArrowRight on the specimen tabs shows exactly the second run",
    )
    for error in cdp.take_errors():
        report.failures.append(f"interaction: JavaScript error: {error}")


def run_site(
    out: Path, widths: list[int], themes: list[str], with_checks: bool
) -> Report:
    report = Report()
    with (
        serve_site(REPO / "site") as base,
        launch_chrome() as port,
        open_session(port) as cdp,
    ):
        for theme in themes:
            for width in widths:
                for page in SITE_PAGES:
                    load(cdp, base + page, width, theme)
                    label = f"{theme} {width} {page}"
                    report.shots.append(
                        save_shot(
                            cdp, out / f"{theme}-{width}-{slug_for(page)}.png", width
                        )
                    )
                    check_page(cdp, label, report)
        if with_checks:
            run_site_checks(cdp, base, report, out)
    return report


MILL_STATES: list[tuple[str, str, str]] = [
    (
        "first",
        "",
        "document.querySelectorAll('#samples [data-sample]').length >= 5"
        " && document.getElementById('editor').value.includes('RequireGate')",
    ),
    (
        "run",
        "document.getElementById('run').click()",
        "document.querySelector('#result-body')"
        " && document.querySelector('#result-body').children.length > 0"
        " && !document.getElementById('run').hasAttribute('aria-busy')",
    ),
    ("table", "document.querySelector('#views [data-view=table]').click()", "true"),
    ("json", "document.querySelector('#views [data-view=json]').click()", "true"),
    (
        "contingent",
        "(() => { document.querySelector('#views [data-view=opinion]').click();"
        " document.querySelector('#samples [data-sample=\"trust-open\"]').click();"
        " })()",
        "document.getElementById('result-body').textContent.includes('Alice')",
    ),
    (
        "diagnostics",
        "(() => { const ed = document.getElementById('editor');"
        " document.querySelector('[data-buffer=module]').click();"
        f" ed.value = {json.dumps(BAD_MODULE)};"
        " ed.dispatchEvent(new Event('input', {bubbles: true}));"
        " setTimeout(() => { const at = ed.value.indexOf('colour') + 2; ed.focus();"
        " ed.setSelectionRange(at, at);"
        " ed.dispatchEvent(new Event('keyup', {bubbles: true}));"
        " ed.dispatchEvent(new Event('click', {bubbles: true}));"
        " document.dispatchEvent(new Event('selectionchange')); }, 1500); })()",
        "document.getElementById('diag-pop')"
        " && !document.getElementById('diag-pop').hidden",
    ),
]


def run_mill(url: str, out: Path, widths: list[int], themes: list[str]) -> Report:
    report = Report()
    with launch_chrome() as port, open_session(port) as cdp:
        for theme in themes:
            for width in widths:
                load(cdp, url, width, theme)
                cdp.run_js("localStorage.removeItem('fidryn-mill')")
                load(cdp, url, width, theme)
                for name, action, ready in MILL_STATES:
                    if action:
                        cdp.run_js(action)
                    if not wait_for(cdp, ready, timeout=20.0):
                        report.failures.append(
                            f"{theme} {width} mill {name}: never became ready"
                        )
                    time.sleep(0.3)
                    report.shots.append(
                        save_shot(cdp, out / f"{theme}-{width}-mill-{name}.png", width)
                    )
                    check_page(cdp, f"{theme} {width} mill {name}", report)
                cdp.run_js("localStorage.removeItem('fidryn-mill')")
    return report


def parse_list(text: str) -> list[str]:
    return [part.strip() for part in text.split(",") if part.strip()]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)
    serve = sub.add_parser("serve", help="serve site/ with clean URLs")
    serve.add_argument("--port", type=int, default=8752)
    for name in ("site", "mill"):
        p = sub.add_parser(name, help=f"screenshot the {name}")
        p.add_argument("--out", type=Path, required=True)
        p.add_argument("--widths", default="390,820,1440")
        p.add_argument("--themes", default="light,dark")
    sub.choices["site"].add_argument(
        "--no-checks", action="store_true", help="skip the interaction checks"
    )
    sub.choices["mill"].add_argument("--url", default="http://127.0.0.1:8751")
    args = parser.parse_args(argv)

    if args.command == "serve":
        with serve_site(REPO / "site", args.port) as base:
            print(f"serving site/ at {base} (Ctrl-C stops)")
            try:
                while True:
                    time.sleep(3600)
            except KeyboardInterrupt:
                return 0

    widths = [int(w) for w in parse_list(args.widths)]
    themes = parse_list(args.themes)
    run: Callable[[], Report]
    if args.command == "site":
        run = functools.partial(run_site, args.out, widths, themes, not args.no_checks)
    else:
        run = functools.partial(run_mill, args.url, args.out, widths, themes)
    report = run()
    print(f"{len(report.shots)} screenshots in {args.out}")
    for failure in report.failures:
        print(f"FAIL {failure}")
    print("ok" if not report.failures else f"{len(report.failures)} failures")
    return 0 if not report.failures else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
```

- [ ] **Step 2: Lint and type-check it**

Run: `uvx ruff check --target-version py312 --select E,F,W,I,UP,B,A,N,SIM,RUF .agents/scripts/shoot.py && uvx mypy --strict --python-version 3.12 .agents/scripts/shoot.py`
Expected: `All checks passed!` then `Success: no issues found in 1 source file`.

- [ ] **Step 3: Document it**

Append to `.agents/scripts/README.md`:

```markdown

## `shoot.py`

Screenshots and audits the site and the mill in headless Chrome. Standard library only (Python 3.12+).

- `python3 .agents/scripts/shoot.py serve` serves `site/` on http://127.0.0.1:8752 with clean URLs.
- `python3 .agents/scripts/shoot.py site --out .agents/scratchpad/shots/site` screenshots every page at 390, 820, and 1440 pixels in light and dark, and fails on sideways scrolling, JavaScript errors, or a failed search, drawer, theme, or specimen check.
- `python3 .agents/scripts/shoot.py mill --out .agents/scratchpad/shots/mill` does the same for a running `fidryn ui` (default `http://127.0.0.1:8751`) in its first-run, run, table, JSON, contingent, and diagnostics states.

Chrome runs with a throwaway profile and background downloads turned off, and the profile is deleted afterwards.
```

- [ ] **Step 4: Audit the site**

Run: `python3 .agents/scripts/shoot.py site --out .agents/scratchpad/shots/site`
Expected: `68 screenshots in .agents/scratchpad/shots/site` then `ok` (11 pages × 3 widths × 2 themes, plus the search and drawer interaction shots). Any `FAIL` line names the page, width, and theme, and for sideways scrolling the first offending elements; fix those before going on (Step 7).

- [ ] **Step 5: Audit the mill**

Run:

```bash
cargo build -p fidryn-cli --offline
target/debug/fidryn ui --port 8751 --no-open > .agents/scratchpad/mill.log 2>&1 &
sleep 2
python3 .agents/scripts/shoot.py mill --out .agents/scratchpad/shots/mill
```

Expected: `36 screenshots in .agents/scratchpad/shots/mill` then `ok` (6 states × 3 widths × 2 themes). Leave the mill running for Step 6.

- [ ] **Step 6: Review the screenshots**

Open each file named below and check each point. Anything that does not match is a defect for Step 7.

- `light-1440-home.png`: one header row (the F monogram with its rubric shadow, `Fidryn`, Docs, Examples, GitHub, the search field with its `/` hint, the theme toggle) over a double rule; the hero headline "No false determinacy." in large Fraunces on the left, the lede, the ink `Get started` and outlined `Read the docs` buttons, and the install command with Copy on the right; the specimen full width with four tabs (the first inked), and three steps: Source with real line numbers starting at 47 and a `…` gap before 205, the framed Case JSON, and the Outcome with a Contingent stamp and its sentences; the research-fixture note; `§ 2 — What it is` in three columns; `§ 3 — Six honest outcomes` as six question rows, each ending in its stamp; `§ 4 — Contents` in two columns with dotted leaders; the footer under a double rule.
- `light-390-home.png`: brand, theme toggle, and Menu on one row with the search field full width beneath; the hero stacked; the specimen as swipeable cards, each showing its outcome first and its source below, with the dots under them; nothing cut off at the right edge.
- `light-1440-docs-outcomes.png`: three columns. On the left, the grouped guides with `§` numbers and one-line descriptions, Outcomes marked with a rubric bar. In the middle, the kicker `§ 6 · Read results`, the title, headings numbered `6.1`, `6.3`, `6.3.1` in rubric, full-grid tables with tinted headers, framed code blocks with a caption bar and Copy, and callouts with a tinted rubric bar. On the right, the "On this page" rail. At the end, previous and next in two columns under a double rule, then the edit and markdown links.
- `light-820-docs-outcomes.png`: two columns, with "On this page" as a disclosure above the text.
- `light-390-docs-examples.png` and `light-390-docs-cli.png`: wide tables and long commands scroll inside their frames; the page does not.
- Every `dark-*` file: the Ink palette (warm near-black ground, cream text, coral accents), readable stamps and code tokens, and a barely visible grain.
- `light-1440-no-such-page.png`: `§ 404`, "No such provision.", the sentence about the declared model, and the two buttons.
- `interaction-search.png`: the dropdown under the field, with matched prefixes highlighted and one row selected. `interaction-drawer.png`: the guides drawer over a dimmed page.
- Mill `*-first.png`: five samples with stamps (Determinate, Determinate, Contingent, Determinate, Suspended), the segmented Module / Case JSON / Template control, the highlighted editor with line numbers, the query and clock fields, the four actions, and the empty result message.
- Mill `*-run.png`: the Opinion view as a paper document (caption, a Determinate title, `q is 7.`, the outside-scope sentence, the module and `asOf` footer). `*-table.png`: a dense table of every field. `*-json.png`: highlighted JSON with Copy. `*-contingent.png`: `Under I1 it is Alice.` and `Under I2 it is Bob.`.
- Mill `*-diagnostics.png`: a wavy underline under `colour`, a gutter mark on line 7, and the message box directly under line 7.
- Mill `*-390-*`: one column, the samples as a chip row, and the Check / Run / Explore / Render bar pinned to the bottom.

- [ ] **Step 7: Fix what the review found**

For each defect: add a failing check first where one fits (a node test in `web/tests/`, an assertion in `xtask/tests/site_output.rs` or `xtask/tests/mill_page.rs`, or the audit itself), fix it in the owning file, run `cargo xtask site` if a file under `site/assets/` changed (the pages embed asset hashes), re-run the audit from Step 4 or Step 5, and commit each fix separately:

```bash
git add <changed files> site
git -c commit.gpgsign=false commit -m "fix(site): <what was wrong>"
```

Repeat Steps 4–6 until both audits print `ok` and the review finds nothing.

- [ ] **Step 8: Keyboard pass on the mill**

With the mill open in a desktop browser at http://127.0.0.1:8751, check by hand: the skip link appears on the first Tab; Tab reaches the samples, the segmented control, the editor, the fields, and the actions in that order; inside the editor Tab indents, and Escape followed by Tab moves focus on to the next control; Ctrl or Cmd with Enter runs from anywhere; the result view buttons and the history entries work with Enter and Space; every focused control shows the rubric focus ring. Treat anything else as a Step 7 defect.

- [ ] **Step 9: Full verification**

Run:

```bash
kill %1 2>/dev/null || pkill -f "fidryn ui --port 8751"
cargo fmt --all -- --check
cargo xtask ci
git status --short
```

Expected: `cargo fmt` prints nothing; `cargo xtask ci` finishes with the workspace tests passing, clippy clean with `-D warnings`, the schema probes passing, `site/ is up to date (26 generated files)`, and the JS tests passing (`# fail 0`); `git status --short` prints nothing except files under `.agents/scratchpad/` (ignored).

- [ ] **Step 10: Commit the tool**

```bash
git add .agents/scripts/shoot.py .agents/scripts/README.md
git -c commit.gpgsign=false commit -m "chore(agents): add headless screenshot and layout audit tool"
```

---

## Notes from plan review

The tasks were written in parallel against the shared contract and each section was validated by replaying its own steps in a throwaway worktree. These are the cross-section decisions made while assembling; the task texts already reflect them.

- `schemas/mill-evaluation-response-v0.1.json` gains an optional `opinion` array in Task 3, so real `/api/run` and `/api/explore` responses stay valid against the repository's own transport schema.
- From Task 13 until Task 14 replaces `web/index.html`, the new CSP blocks the old page's inline code, so the mill is not usable in a browser in between. Tests are unaffected.
- Explore completion keys with several bindings read `A = I1, B = absent` (each binding loses its `x:` prefix), covered by a Task 2 test.
- Task 8 strips a stale mirror header (YAML front matter and a "Canonical HTML" note) from the top of `docs/examples.md` in its own `docs:` commit, so every guide starts with its `# ` title.
- `xtask` is a binary-only crate: generator items read only by later tasks show dead-code warnings until Task 11 wires `build`. After Task 11, clippy with `-D warnings` is clean; `Keywords::iter` is allowed as test-only.
- Tests in `xtask/tests/` cannot reach `xtask` modules, so `mill_keywords.rs` re-implements the grammar keyword rule (C1) itself.
- Task 6 removes `"type": "module"` from `site/package.json` so Node can `require` the site script in tests; Task 12 deletes the file.
- The Outcomes page's pager neighbors are §5 Mill and §7 Examples (grouped order), and the contract example says so.
- Phone specimen cards carry a `.spec-label` title (Task 10 markup, Task 5 CSS), shown only below 720px where the tabs are hidden.
- The one "What it is not." note in `docs/outcomes.md` becomes a blockquote in Task 17, so it renders as the tinted-bar callout the design picked.
- Choosing a mill sample loads it and runs its default action; the first visit only loads `require-gate`. Escape then Tab leaves the editor (`editorKey`, Task 16). Task 17's docs describe both.
- Node 18.14's test runner fails on non-ASCII test names or assertion messages, so `web/tests/*.test.js` keep those ASCII.
- Headless Chrome on macOS will not lay out narrower than 500px from `--window-size`; `shoot.py` uses CDP device-metrics emulation, which gives true 390px layouts.
