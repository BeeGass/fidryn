# Site and mill redesign ("Statute") — design

Date: 2026-09-28. Branch: `feat/site-statute-redesign`. Status: approved in
conversation, pending review of this written spec.

## Goal

Overhaul the look and the experience of every Fidryn web surface so it
reads well on desktop and on phones, while staying on the Rust stack:

1. The public landing page (`site/index.html`).
2. The learner docs (`site/docs/*`), rendered from `docs/*.md`.
3. The localhost mill (`web/index.html`, served by `fidryn ui` through axum).

"Rust stack" means: no JS framework, no Node build step. Pages are static
HTML, CSS, and small vanilla JS. All build tooling is Rust.

## Decisions (from the brainstorm)

| Topic | Decision |
| --- | --- |
| Scope | All three surfaces, one shared design system |
| Docs build | `cargo xtask site` replaces the stubbed Node script; `docs/*.md` is canonical |
| Visual direction | **Statute**: warm paper, black ink, rubric red; Fraunces for headings and body, IBM Plex Sans for labels, IBM Plex Mono for code |
| Landing layout | **Demo first**: hero, then a tabbed specimen of real runs |
| Docs layout | **Three columns**: numbered contents, text, "On this page" rail |
| Mill layout | **Three panes**: samples and history, editor, result |
| Dark theme | **Ink**: warm near-black, cream text, coral accent |
| Search | Yes: build-time JSON index plus a keyboard palette |
| Generator shape | Approach A: a `site` module inside the existing `xtask` crate |
| Workflow | Branch in this checkout, commit every milestone |

## Non-goals

- No hosted mill, no live filing from any page, no analytics, no external
  requests (fonts stay self-hosted, no CDNs).
- No new workspace crate and no templating engine.
- No change to evaluation semantics, schemas, or CLI behavior.
- Implementer docs (`ARCHITECTURE.md`, `OBLIGATIONS.md`, contracts, status
  matrix) stay on GitHub; the site links out to them.

## 1. Architecture and files

```
site/                          deploy root (Vercel), committed
  assets/fidryn.css            hand-written design system (source)
  assets/fidryn.js             hand-written enhancement (source)
  fonts/*.woff2                existing Fraunces / Plex (unchanged)
  favicon.svg                  new section-mark favicon (source)
  vercel.json, README.md       hand-maintained
  index.html, index.md, 404.html                      generated
  docs/*.html, docs/*.md                              generated
  search-index.json, llms.txt, llms-full.txt,
  sitemap.xml, robots.txt                             generated
  (removed: styles.css, docs.css, scripts/, package.json, package-lock.json)
xtask/src/site/
  mod.rs        `cargo xtask site [--check]`
  markdown.rs   pulldown-cmark to HTML: ids, numbering, links, TOC, tables
  highlight.rs  fr (fidryn-syntax lexer + grammar.ebnf keywords), json, shell
  specimen.rs   evaluates the landing samples with the real interpreter
  pages.rs      landing, docs, 404; sidebar; prev/next
  seo.rs        head meta, JSON-LD, sitemap, robots, llms*.txt, search index
  templates/    base.html, landing.html, doc.html, 404.html
web/
  index.html    mill page (rewritten)
  mill.css      mill components, layered on site/assets/fidryn.css
  mill.js       mill app
crates/fidryn-cli/src/ui.rs    new static routes and /api/samples
```

Rules:

- Every generated file is overwritten on each run. Hand-written sources are
  never touched by the generator.
- Output is deterministic: no timestamps, stable ordering, so a second run
  produces identical bytes. `--check` regenerates in memory and fails,
  listing paths, when any committed file differs. `cargo xtask ci` runs it.
- Asset URLs carry a content hash (`/assets/fidryn.css?v=<8 hex>`), so
  `vercel.json` can cache `/assets/*` and `/fonts/*` for a year.
- Pages are fully readable with JS disabled. JS only enhances.
- The mill includes `site/assets/fidryn.css` so site and mill share one
  system. It gains no runtime filesystem access: every asset and sample is
  embedded at compile time with `include_str!` / `include_bytes!`.

Dependencies added to `xtask`: `pulldown-cmark` (already in the offline
registry cache), and path dependencies on `fidryn-syntax` and the CLI
library (`fidryn-cli`) for highlighting and specimen evaluation.

## 2. Design system (`site/assets/fidryn.css`)

### Tokens

| Token | Light (Statute) | Dark (Ink) | Use |
| --- | --- | --- | --- |
| `--paper` | `#f7f2e8` | `#15120e` | page ground |
| `--paper-2` | `#efe7d6` | `#1e1a15` | code, insets |
| `--paper-3` | `#e6dbc4` | `#28221b` | hover, selected |
| `--ink` | `#1c1813` | `#efe6d6` | headings, strong text |
| `--ink-2` | `#3d362c` | `#cfc3ae` | body text |
| `--ink-3` | `#6f6454` | `#9a8e7a` | muted labels |
| `--rule` | `#d6c9b1` | `#3a3228` | hairlines |
| `--rubric` | `#a3281d` | `#e0725f` | accent, section marks, focus |
| `--det` | `#2e6a39` | `#8cc79a` | Determinate |
| `--con` | `#23507f` | `#8fb4e8` | Contingent |
| `--sus` | `#8a5d06` | `#e2b457` | Suspended |
| `--nc` | `#9c3d10` | `#ec9a67` | NormConflict |
| `--oc` | `#6b3a72` | `#cfa1d8` | OutsideCompetence |
| `--inc` | `#b0182b` | `#f07b7b` | Inconsistent |
| `--tok-kw` | `#a3281d` | `#ec8a74` | keywords |
| `--tok-type` | `#23507f` | `#9dbde8` | types, JSON keys |
| `--tok-str` | `#3b6a2c` | `#a8cf95` | strings |
| `--tok-num` | `#86570a` | `#e2b86a` | numbers, dates, literals |
| `--tok-com` | `#5e6652` | `#9aa08a` | comments |
| `--tok-punct` | `#6b6152` | `#a79a84` | punctuation |

All text pairs were measured at WCAG AA or better (4.5:1): body, muted,
rubric, the six stamps on `--paper`, and every code token on `--paper-2`,
in both themes. A test enforces this (section 6).

Theme selection: light by default; `prefers-color-scheme: dark` selects Ink;
a toggle writes `data-theme="light|dark"` on `<html>` and remembers it in
`localStorage`. A tiny inline head script applies the stored choice before
first paint. The mill uses the same mechanism from `mill.js` (no inline
script there; see CSP).

### Type

- Fraunces (variable `opsz` 9–144, `wght` 100–900): display at `opsz` 144,
  body text at `opsz` 14, 17px on docs, 16px elsewhere, line-height 1.6.
- IBM Plex Sans: small-caps-style labels (uppercase, 0.14–0.18em tracking),
  buttons, navigation.
- IBM Plex Mono 400/500: code, commands, identifiers.
- Fluid scale with `clamp()`; the docs measure is about 68 characters.

### Components

Header with double rule; wordmark; buttons (primary ink, secondary outlined,
2px radius); command line with copy; code box (language label, copy button,
rubric left rule); outcome stamps (bordered small caps with a square mark);
tables (small-caps headers, hairline rows, horizontal scroll wrapper);
callouts; section marks (hanging `§ N` on desktop, inline on phones);
contents list with dotted leaders; prev/next pager; drawer; search palette;
focus ring in `--rubric`.

Breakpoints: phone below 720px, tablet 720–1199px, desktop 1200px and up.
Every layout keeps at least a 16px side gutter; only code and tables scroll
horizontally, inside their own containers. Safe-area insets are respected.

## 3. Public site

### Landing (`index.html`)

1. Header: wordmark; Docs, Examples, GitHub; search button (`/`); theme
   toggle. Phones keep Docs, search, and theme; the rest move to the footer.
2. Hero: eyebrow "A programming language for legal instruments"; headline
   "No false determinacy."; lede from the README; Get started / Read the
   docs; install command with copy.
3. Specimen: tabs over four runs, all computed at build time by
   `specimen.rs` through the same library path as `fidryn run`:
   - `require-gate` query `q`: Determinate `7`.
   - `require-gate` query `r`: Suspended (requirement failed).
   - trust fixture, `two-certificates-open-eligibility`: Contingent
     (I1 gives Alice, I2 gives Bob).
   - trust fixture, `court-selects-i2`: Determinate Bob.
   Left: the source (the full `require-gate` module; for the trust fixture a
   trimmed excerpt of the `SuccessorEligibility` family and the
   `acting_trustee` query, with a link to the full file). Right: an outcome
   card (stamp; value, alternatives, or requests; `outsideScope`; `asOf`).
   Without JS all four render stacked.
4. Research-fixture note.
5. "What it is" three-up: instruments, honest outcomes, the local mill.
6. Six-outcomes legend; each stamp links to its section of Outcomes.
7. Contents: the eight guides with dotted leaders.
8. Footer colophon.

### Docs (`docs/*.html`)

Guides, numbered in `docs/README.md` order: §1 Getting started, §2 Language,
§3 CLI, §4 Mill, §5 Cases and time, §6 Outcomes, §7 Examples, §8
Contributing. The Overview (`/docs/`) is unnumbered.

- Top bar: wordmark, breadcrumb, search field (`/`), theme toggle.
- Left column: numbered guides with the current one marked, then
  Implementers links to GitHub (Architecture, Implementation status,
  Obligations, contracts).
- Centre: kicker (`§ 6 Guides`), title, body. `h2`/`h3` are numbered
  (`6.1`, `6.3.1`) with a hover `#` link that copies the URL. Code gets a
  language label, copy button, and build-time highlighting. Tables scroll
  inside a wrapper. Blockquotes render as callouts. Footer: prev/next,
  "Edit on GitHub", "View as Markdown", and the not-legal-advice line.
- Right rail (1200px and up): "On this page" for `h2`/`h3` with scrollspy.
- Tablet: two columns; "On this page" becomes a disclosure at the top of the
  article.
- Phone: Contents drawer (backdrop, focus trap, Escape to close, returns
  focus to the button) and the "On this page" disclosure.

### Search

`search-index.json` has one entry per section: guide number, page title,
section number, heading, URL with anchor, and the first ~300 characters of
the section text. `/` or Ctrl/Cmd-K opens a dialog (`role="dialog"`,
`aria-modal`, listbox results); arrow keys and Enter navigate; Escape
closes. The index is fetched on first open. Matching is case-insensitive
token prefix over heading (weighted) and text. Without JS, the search
control is a link to `/docs/`.

### Other pages and metadata

- `404.html` in the same style, with links to the landing page and docs.
- `favicon.svg`: a rubric section mark on paper.
- SEO carries over unchanged in substance: canonical URL, Open Graph,
  Twitter card, JSON-LD, `text/markdown` alternates, `llms.txt`,
  `llms-full.txt`, `sitemap.xml`, `robots.txt`. Per-page descriptions are
  kept in the generator's guide table (seeded from today's pages).

## 4. Generator (`cargo xtask site`)

- Input: `docs/README.md` (Overview) and the eight learner guides; the
  landing sources under `tests/programs/` and `examples/trust/`;
  `grammar.ebnf` for keywords.
- Markdown: pulldown-cmark with tables and heading attributes. Heading ids
  are GitHub-style slugs, de-duplicated with `-1`, `-2`. The first `h1` is
  the page title. Numbering is applied to `h2` and `h3` only.
- Links: `guide.md` or `guide.md#x` becomes `/docs/guide#x`; other `docs/`
  markdown and `../path` links become GitHub `blob/main` URLs (`tree/main`
  for directories); absolute and external links are untouched.
- Code fences: an explicit language wins. Unlabeled fences are sniffed:
  starts with `{` or `[` means JSON; starts with `module` or contains a
  top-level `query` means `fr`; starts with `cargo`, `fidryn`, `cat`, `$`
  means shell; otherwise plain text. All code text is HTML-escaped before
  wrapping tokens in spans.
- `.fr` highlighting uses `fidryn_syntax::lex`. Identifiers that appear as
  quoted lowercase terminals in `grammar.ebnf` are keywords; capitalized
  identifiers after `->`, `:` or `<` are types; `true`/`false`, numbers,
  dates, and durations are literals.
- Markdown mirrors under `site/docs/*.md` are copies of the sources with
  links rewritten to absolute site or GitHub URLs.

## 5. Mill (`fidryn ui`)

### Layout

- Header: "Fidryn mill", `localhost 127.0.0.1` with health status, the
  notice "Live filing is not available from the UI", a link to the Mill
  guide, theme toggle.
- Left rail, Samples, embedded at compile time and served by
  `GET /api/samples`:
  - `require-gate` (run `q`: Determinate; run `r`: Suspended)
  - `late-payment` (run `due`: Determinate; run `paid_on_time`: Suspended,
    needs a PaymentRecord)
  - trust, open eligibility (run: Contingent)
  - trust, court selects I2 (run: Determinate)
  - trust, one certificate (run: Suspended)
  Each sample carries source, case, query, validAt, knownAt, and a default
  action. Loading a sample over unsaved edits asks inline (Replace or
  Cancel); no browser dialogs.
- Left rail, History: the last 20 actions of this session with stamp,
  action, query, and time. Selecting one reopens its result; "Restore
  inputs" puts its inputs back.
- Centre, editor: tabs Module, Case JSON, Template; line numbers; a
  highlighted overlay under a transparent textarea (same `fr` and JSON
  rules as the site); Tab and Shift-Tab indent and outdent; Enter keeps the
  indentation. Check runs automatically 700 ms after typing stops and
  paints diagnostics in the gutter; auto-checks never enter History.
  Diagnostic spans are UTF-8 byte offsets and are converted before placing
  the caret. Case JSON is validated as you type, with line and column.
- Right: Query (suggestions parsed from the module's `query` names),
  validAt, knownAt (RFC 3339 validated inline); Check, Run (primary),
  Explore, Render. Ctrl/Cmd+Enter runs; Ctrl/Cmd+Shift+Enter explores.
- Right, result card: stamp, action, query, elapsed time, mode and trust
  badges; body by kind (value; alternatives table with pivots; requests
  list; other kinds as labelled rows, with unknown fields falling back to
  rows so schema growth still displays); always modelBoundary, asOf,
  module, trace (copy), verificationMethod; raw JSON in a disclosure with
  copy. Check shows "ok" or the diagnostics list (code, message,
  line:column, suggestion). Render shows the text as a paper document.
  Engine errors, invalid input, and an unreachable server ("is `fidryn ui`
  still running?") show inline.
- Inputs persist in `localStorage`.
- Phone: header, samples as a horizontal chip row, editor, parameters,
  sticky action bar, result.

All dynamic content is written with `textContent` or built DOM nodes, never
`innerHTML` of response or pasted data.

### Server changes (`crates/fidryn-cli/src/ui.rs`)

| Route | Body | Type |
| --- | --- | --- |
| `GET /assets/fidryn.css` | `site/assets/fidryn.css` | `text/css` |
| `GET /assets/mill.css` | `web/mill.css` | `text/css` |
| `GET /assets/mill.js` | `web/mill.js` | `text/javascript` |
| `GET /fonts/{name}.woff2` | the four font files | `font/woff2` |
| `GET /api/samples` | JSON array of samples | `application/json` |

Existing routes are unchanged. `GET /` adds
`Content-Security-Policy: default-src 'self'; img-src 'self' data:;
object-src 'none'; base-uri 'none'; frame-ancestors 'none'`,
`X-Content-Type-Options: nosniff`, and `Referrer-Policy: no-referrer`.
Static routes send `Cache-Control: no-cache`. The page keeps the strings the
current tests assert (`fidryn mill`, `localhost`, `127.0.0.1`,
`Live filing is not available`, `>Run<`, `>Explore<`, `>Render<`).

`docs/mill.md` is updated: the Routes table and "The HTML page" section.

## 6. Testing and verification

Generator (xtask tests):

- Markdown: slug stability and de-duplication; numbering; TOC; every link
  rewrite rule; table wrapping; fence sniffing.
- Highlighter: token classes for `fr`, JSON, shell; escaping of `<`, `>`,
  `&`, quotes; stripping tags from output reproduces the source exactly.
- Specimen: the four runs produce the expected kinds and values.
- Site: every internal `href` and `#anchor` in the output resolves; every
  page has title, description, canonical; two runs are byte-identical;
  `--check` passes after a run.
- Contrast: parse both token blocks from `fidryn.css` and assert 4.5:1 for
  the pairs in section 2.

Mill (`ui.rs` tests):

- Existing tests pass unchanged.
- Each static route returns 200 with the right content type; an unknown
  asset or font returns 404; `/` carries the CSP header.
- Each `/api/samples` entry, sent through the router as the page sends it,
  yields its expected outcome kind.
- The keyword list in `web/mill.js` equals the `grammar.ebnf` set.

Before calling it done: `cargo fmt`, `cargo xtask ci` (tests and
`clippy -D warnings`), `cargo xtask site --check`; headless Chrome
screenshots of landing, two docs pages, 404, and the mill at 390, 820, and
1440px in light and dark, reviewed; a keyboard pass over skip link, drawer,
search, and mill shortcuts.

## 7. Rollout

- Commits by milestone on `feat/site-statute-redesign` (Conventional
  Commits, per `docs/contributing.md`).
- `site/README.md` and `docs/contributing.md` document `cargo xtask site`
  and `--check`; the Node instructions are removed.
- Record the generator decision as `.agents/adr/0001-rust-site-generator.md`
  and index it in `.agents/ADR.md`.

## Risks

- The trust excerpt is extracted by structure (family block and query
  block). If the fixture is restructured, the specimen test fails loudly
  rather than rendering something wrong.
- Overlay highlighting depends on identical metrics in the textarea and the
  highlighted layer. Both use the embedded Plex Mono with the same padding,
  tab size, and wrapping (none).
- Build-time evaluation ties the landing page to interpreter output. That
  is intended: if semantics change, `--check` flags the page as stale.
