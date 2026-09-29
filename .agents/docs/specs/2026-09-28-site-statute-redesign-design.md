# Site and mill redesign ("Statute") — design

Date: 2026-09-28, revised 2026-09-29 with the option-board picks.
Branch: `feat/site-statute-redesign`. Status: pending review of this
written spec.

Board picks (the source of every visual decision below):
`S1A S2A S3A S4A S5A S6C S7A S8B S9A S10A S11B S12B S13B S14D S15A S16C ·
L1A L2D L3A L4C L5B L6C L7A L8A L9B L10A L11C L12A L13A L14B L15C L16B
L17A L18A L19A`, plus: the mill result offers both the printed-opinion
view (L14B) and the compact-table view (L14C).

## Goal

Overhaul the look and the experience of every Fidryn web surface so it
reads well on desktop and on phones, while staying on the Rust stack:

1. The public landing page (`site/index.html`).
2. The learner docs (`site/docs/*`), rendered from `docs/*.md`.
3. The localhost mill (`web/index.html`, served by `fidryn ui` through axum).

"Rust stack" means: no JS framework, no Node build step, no vendored
editor library. Pages are static HTML, CSS, and small vanilla JS. All
build tooling is Rust.

## Decisions

| Topic | Decision |
| --- | --- |
| Scope | All three surfaces, one shared design system |
| Docs build | `cargo xtask site` replaces the stubbed Node script; `docs/*.md` is canonical |
| Direction | **Statute**: warm paper, black ink, rubric red |
| Dark theme | **Ink**: warm near-black, cream text, coral accent |
| Search | Build-time JSON index, dropdown results under the search field |
| Generator | Approach A: a `site` module inside the existing `xtask` crate |
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
  favicon.svg                  F monogram (source)
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
  specimen.rs   evaluates the landing runs with the real interpreter
  pages.rs      landing, docs, 404; sidebar; prev/next
  seo.rs        head meta, JSON-LD, sitemap, robots, llms*.txt, search index
  templates/    base.html, landing.html, doc.html, 404.html
web/
  index.html    mill page (rewritten)
  mill.css      mill components, layered on site/assets/fidryn.css
  mill.js       mill app
crates/fidryn-cli/src/ui.rs    new static routes, /api/samples, `opinion` in eval responses
crates/fidryn-cli/src/opinion.rs   report-to-sentences templates (shared with the site)
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

| Token | Light (cream paper) | Dark (Ink) | Use |
| --- | --- | --- | --- |
| `--paper` | `#f7f2e8` | `#15120e` | page ground |
| `--paper-2` | `#efe7d6` | `#1e1a15` | code header, table header, insets |
| `--paper-3` | `#e6dbc4` | `#28221b` | hover, selected |
| `--ink` | `#1c1813` | `#efe6d6` | headings, strong text |
| `--ink-2` | `#3d362c` | `#cfc3ae` | body text |
| `--ink-3` | `#6f6454` | `#9a8e7a` | muted labels |
| `--rule` | `#d6c9b1` | `#3a3228` | hairlines, grid lines |
| `--rubric` | `#a3281d` | `#e0725f` | accent, marks, links, keywords, focus |
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

Every text pair was measured at WCAG AA or better (4.5:1): body, muted,
rubric, the six stamps on `--paper`, and every code token on the code
background, in both themes. A test enforces this (section 7).

Theme: light by default; `prefers-color-scheme: dark` selects Ink; a toggle
writes `data-theme="light|dark"` on `<html>` and remembers it in
`localStorage`. A tiny inline head script applies the stored choice before
first paint on the site. The mill does the same from `mill.js` (no inline
script there; see CSP).

### Style picks

| Pick | Decision | Detail |
| --- | --- | --- |
| S1A | Warm cream paper | tokens above |
| S2A | Rubric red accent | links underline in rubric; keywords take the accent |
| S3A | Ink night palette | tokens above |
| S4A | Fraunces book display | weight 400, `opsz` 144, tracking -0.02em |
| S5A | Fraunces body text | `opsz` 14, 17px in docs, 16px elsewhere, line-height 1.6 |
| S6C | Sentence-case labels | Plex Sans 600, 12px, normal case, for nav, labels, and buttons |
| S7A | Double rule | double rule under the header and above the footer, single rules between sections |
| S8B | Soft corners | 6px on buttons, fields, stamps, code frames, cards |
| S9A | Ink buttons | primary is ink on paper; secondary is outlined in ink |
| S10A | Bordered stamps | small caps, 1px border in the kind's color, square mark |
| S11B | Framed code | 1px `--rule` border, a `--paper-2` header bar with the language and a Copy button |
| S12B | Inline section marks | `§ 2 —` leads each landing heading in rubric |
| S13B | Paper grain | a tiled SVG noise texture embedded in the CSS as a data URI, at about 6% opacity (3% in dark); no effect on contrast tokens |
| S14D | F monogram | a square Fraunces F with an offset rubric shadow, beside the wordmark; also the favicon |
| S15A | Comfortable density | rows 30px on desktop; every touch target at least 44px on phones |
| S16C | Full-grid tables | every cell boxed, a tinted header row |

Stamps keep their small caps (S10A) even though other labels are in
sentence case (S6C): they are status marks, not labels.

Breakpoints: phone below 720px, tablet 720–1199px, desktop 1200px and up.
Every layout keeps at least a 16px side gutter; only code and tables scroll
horizontally, inside their own containers. Safe-area insets are respected.

## 3. Public site

### Guide order and numbering

The docs sidebar groups the guides (L6C), and the § numbers follow that
order everywhere (sidebar, headings, prev/next, landing contents):

| Group | Guides |
| --- | --- |
| Start | §1 Getting started |
| Write | §2 Language, §3 Cases and time |
| Run | §4 CLI, §5 Mill |
| Read results | §6 Outcomes, §7 Examples |
| Contribute | §8 Contributing |

The Overview (`/docs/`) is unnumbered.

### Landing (`index.html`)

1. Header: monogram and wordmark; Docs, Examples, GitHub; search field;
   theme toggle. Phones keep the monogram, search, and a Menu button.
2. Hero (L1A): eyebrow "A programming language for legal instruments" and
   the headline "No false determinacy." on the left; the README lede, Get
   started / Read the docs, and the install command with Copy on the right.
3. Specimen (L2D), full width under the hero: a run selector, then three
   steps side by side: **Source** (the module or a trimmed excerpt, with
   line numbers from the real file), **Case** (the exact case JSON used),
   **Outcome** (stamp, then value, alternatives, or requests, then
   `outsideScope`). All four runs are computed at build time by
   `specimen.rs` through the same library path as `fidryn run`:
   - trust fixture, `two-certificates-open-eligibility`: Contingent
     (I1 gives Alice, I2 gives Bob). Selected by default.
   - trust fixture, `court-selects-i2`: Determinate Bob.
   - `require-gate` query `q` with the empty case: Determinate `7`.
   - `require-gate` query `r` with the empty case: Suspended.
   Without JS all four render one after another.
4. Below the hero (L3A): the research-fixture note; `§ 2 — What it is`
   three-up (instruments, honest outcomes, the local mill); `§ 3 — Six
   honest outcomes`; `§ 4 — Contents`; the footer.
5. Six-outcomes legend (L4C): each kind as the question it answers, with
   its stamp. The questions are taken from the "When" clauses in
   `docs/outcomes.md` and do not imply an evaluation order. Each stamp
   links to its section of the Outcomes guide.
6. Phone (L5B): the hero stacks; the specimen becomes swipeable cards (CSS
   scroll-snap, no JS needed), one per run, each with its stamp, key
   values, and a source excerpt, and a dot row showing position.

### Docs (`docs/*.html`)

- Top bar: monogram and wordmark, breadcrumb, search field, theme toggle.
- Left column (L6C): the groups above, each guide with its title and a
  one-line description; the current guide has a rubric bar. Its § number
  appears beside the title so `6.1`-style heading numbers stay traceable.
  Then Implementers links to GitHub (Architecture, Implementation status,
  Obligations, contracts).
- Centre: kicker (`§ 6 · Read results`), title, body. `h2` and `h3` carry
  inline rubric numbers (L7A: `6.1`, `6.3.1`) and a hover `#` link that
  copies the URL. Code uses the framed style with a Copy button and
  build-time highlighting. Tables use the full grid and scroll inside a
  wrapper. Blockquotes and "What it is not" notes use the tinted bar
  (L9B). End of page (L10A): previous and next in two columns under a
  double rule, then "Edit on GitHub", "View as Markdown", and the
  not-legal-advice line.
- Right rail (L8A, 1200px and up): "On this page" for `h2`/`h3`; the
  current section is marked as you scroll.
- Tablet: two columns; "On this page" becomes a disclosure at the top.
- Phone (L12A): a left drawer with search and contents (backdrop, focus
  trap, Escape to close, focus returns to the Menu button) and the "On
  this page" disclosure.

### Search (L11C)

`search-index.json` has one entry per section: guide number, page title,
section number, heading, URL with anchor, and the first ~300 characters of
the section text. Typing in the header field (or `/`, or Ctrl/Cmd-K to
focus it) opens a dropdown under the field; arrow keys and Enter navigate;
Escape closes. The index is fetched on first focus. Matching is
case-insensitive token prefix over heading (weighted) and text, with the
match highlighted. On phones the field sits on its own row under the
header and results drop down full width. Without JS, submitting the
field opens `/docs/`.

### Other pages and metadata

- `404.html` (L19A): `§ 404`, the headline "No such provision.", "This page
  is outside the declared model. Nothing was invented to fill the gap.",
  then links back to the landing page and the docs.
- `favicon.svg`: the F monogram.
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
- The trust excerpt for the specimen is found by structure (the
  `interpretation_family SuccessorEligibility` block and the
  `acting_trustee` query), keeping the real line numbers.
- Markdown mirrors under `site/docs/*.md` are copies of the sources with
  links rewritten to absolute site or GitHub URLs.

## 5. Mill (`fidryn ui`)

### Layout

- Header: monogram, "Fidryn mill", `localhost 127.0.0.1` with health
  status, the notice "Live filing is not available from the UI", a link to
  the Mill guide, theme toggle.
- Left rail (L13A): Samples, each with the stamp of its default action,
  then History. Samples are embedded at compile time and served by
  `GET /api/samples`:
  - `require-gate` (run `q`: Determinate; run `r`: Suspended)
  - `late-payment` (run `due`: Determinate; run `paid_on_time`: Suspended,
    needs a PaymentRecord)
  - trust, open eligibility (run: Contingent)
  - trust, court selects I2 (run: Determinate)
  - trust, one certificate (run: Suspended)
  Each sample carries source, case, query, validAt, knownAt, and a default
  action. Loading a sample over unsaved edits asks inline (Replace or
  Cancel); no browser dialogs. History keeps the last 20 actions of this
  session with stamp, action, query, and time; selecting one reopens its
  result, and "Restore inputs" puts its inputs back.
- Centre, editor (L15C): a segmented control (Module, Case JSON, Template)
  above a borderless editor with line numbers and a highlighted overlay
  under a transparent textarea (same `fr` and JSON rules as the site).
  Tab and Shift-Tab indent and outdent; Enter keeps the indentation.
- Diagnostics (L16B): Check runs automatically 700 ms after typing stops
  (auto-checks never enter History). The offending span gets a wavy
  underline and the line a gutter mark. The message (code and text)
  appears directly under the line while the caret or pointer is on that
  line or its squiggle. Because the editor is a textarea with an overlay,
  the message floats over the following line instead of pushing lines
  down. Diagnostic spans are UTF-8 byte offsets and are converted before
  placing marks or the caret. Case JSON is validated as you type, with
  line and column.
- Right: Query (suggestions parsed from the module's `query` names),
  validAt, knownAt (RFC 3339 validated inline); Check, Run (primary),
  Explore, Render. Ctrl/Cmd+Enter runs; Ctrl/Cmd+Shift+Enter explores.
- Result, with a view switch **Opinion · Table · JSON**, remembered per
  browser:
  - **Opinion** (L14B, default): the result as a printed paper document.
    A caption line (evaluation report, execution mode, source trust), the
    outcome kind as the title, then sentences filled from report fields
    only, per kind. Determinate: "`acting_trustee` is Bob." Contingent:
    "Under I1 it is Alice. Under I2 it is Bob. No single answer is
    determinate across the admissible completions." Suspended: "It is
    suspended until:" and the requests. Other kinds: their fields as
    sentences where a template exists, otherwise labelled rows. Then
    "Outside scope: …", and a footer with `module@version` and `asOf`.
    Every sentence is a fixed template filled from the report; nothing is
    inferred beyond the data. The sentences come from the server (see
    `opinion` below), so the page only lays them out.
  - **Table** (L14C): every field in one dense table (kind, value or
    alternatives, pivots or requests, module, asOf, outsideScope, mode and
    trust, trace with Copy).
  - **JSON**: the raw report, highlighted, with Copy.
  Check shows "ok" or the diagnostics list (code, message, line:column,
  jump). Render shows the text as a paper document. Engine errors, invalid
  input, and an unreachable server ("is `fidryn ui` still running?") show
  inline in the result area.
- First run (L18A): opens on `require-gate` with query `q` and the result
  area saying nothing has run yet, with the Run shortcut.
- Inputs persist in `localStorage`.
- Phone (L17A): header, samples as a horizontal chip row, editor,
  parameters, result, and a sticky Check / Run / Explore / Render bar.

All dynamic content is written with `textContent` or built DOM nodes, never
`innerHTML` of response or pasted data.

### Server changes (`crates/fidryn-cli/src/ui.rs`)

| Route | Body | Type |
| --- | --- | --- |
| `GET /assets/fidryn.css` | `site/assets/fidryn.css` | `text/css` |
| `GET /assets/mill.css` | `web/mill.css` | `text/css` |
| `GET /assets/mill.js` | `web/mill.js` | `text/javascript` |
| `GET /fonts/{name}.woff2` | the four font files | `font/woff2` |
| `GET /favicon.svg` | `site/favicon.svg` | `image/svg+xml` |
| `GET /api/samples` | JSON array of samples | `application/json` |

Existing routes keep their behavior. The success transport of
`/api/run` and `/api/explore` gains one field beside `ok` and `report`:
`opinion`, an array of plain-text sentences produced by
`fidryn_cli::opinion::sentences(&report)`. It is not part of
`fidryn.evaluation-report/v0.1` (just as `ok` is not). The landing
specimen calls the same function at build time, so the site and the mill
word outcomes identically.

`GET /` adds
`Content-Security-Policy: default-src 'self'; img-src 'self' data:;
object-src 'none'; base-uri 'none'; frame-ancestors 'none'`,
`X-Content-Type-Options: nosniff`, and `Referrer-Policy: no-referrer`. The
page therefore uses no inline scripts and no inline `style` attributes.
Static routes send `Cache-Control: no-cache`. The page keeps the strings
the current tests assert (`fidryn mill`, `localhost`, `127.0.0.1`,
`Live filing is not available`, `>Run<`, `>Explore<`, `>Render<`).

`docs/mill.md` is updated: the Routes table and "The HTML page" section.

## 6. Accessibility

WCAG AA contrast (enforced), visible focus rings in `--rubric`, a skip
link, landmarks, `aria-current` in navigation, labelled controls, the
drawer and search results operable by keyboard, 44px touch targets on
phones, and `prefers-reduced-motion` respected (no animated transitions).

## 7. Testing and verification

Generator (xtask tests):

- Markdown: slug stability and de-duplication; numbering in the grouped
  order; TOC; every link rewrite rule; table wrapping; fence sniffing.
- Highlighter: token classes for `fr`, JSON, shell; escaping of `<`, `>`,
  `&`, quotes; stripping tags from output reproduces the source exactly.
- Specimen: the four runs produce the expected kinds and values; the trust
  excerpt keeps its real line numbers.
- Site: every internal `href` and `#anchor` in the output resolves; every
  page has title, description, canonical; two runs are byte-identical;
  `--check` passes after a run.
- Contrast: parse both token blocks from `fidryn.css` and assert 4.5:1 for
  the pairs in section 2.
- The keyword list in `web/mill.js` equals the `grammar.ebnf` set (an
  xtask test, since xtask already reads the grammar).

JavaScript helpers (pure functions exported for tests) are tested with
`node --test web/tests/` when Node is installed; `cargo xtask ci` runs
them if `node` is on the PATH and skips them otherwise. Node is never
needed to build, serve, or deploy anything.

Mill (`ui.rs` tests):

- Existing tests pass unchanged.
- Each static route returns 200 with the right content type; an unknown
  asset or font returns 404; `/` carries the CSP header.
- Each `/api/samples` entry, sent through the router as the page sends it,
  yields its expected outcome kind.
- `opinion::sentences` has a test per outcome kind (including unknown
  fields falling back to labelled rows), and the run and explore
  transports include `opinion`.

Before calling it done: `cargo fmt`, `cargo xtask ci` (tests and
`clippy -D warnings`), `cargo xtask site --check`; headless Chrome
screenshots of landing, two docs pages, 404, and the mill (each result
view) at 390, 820, and 1440px in light and dark, reviewed; a keyboard pass
over skip link, drawer, search, and mill shortcuts.

## 8. Rollout

- Commits by milestone on `feat/site-statute-redesign` (Conventional
  Commits, per `docs/contributing.md`).
- `site/README.md` and `docs/contributing.md` document `cargo xtask site`
  and `--check`; the Node instructions are removed.
- Record the generator decision as `.agents/adr/0001-rust-site-generator.md`
  and index it in `.agents/ADR.md`.

## Risks

- The trust excerpt is extracted by structure. If the fixture is
  restructured, the specimen test fails loudly rather than rendering
  something wrong.
- Overlay highlighting depends on identical metrics in the textarea and the
  highlighted layer. Both use the embedded Plex Mono with the same padding,
  tab size, and wrapping (none).
- Build-time evaluation ties the landing page to interpreter output. That
  is intended: if semantics change, `--check` flags the page as stale.
- The opinion view turns report fields into sentences. The templates live
  in Rust with a test per outcome kind, so a schema change cannot produce a
  sentence that claims more than the report says.
