---
title: "Mill"
description: "Localhost Fidryn mill UI on 127.0.0.1 — checks modules; does not live-file."
url: "https://fidryn.onlygass.dev/docs/mill"
markdown: "https://fidryn.onlygass.dev/docs/mill.md"
author: "Bryan Gass"
---

> Canonical HTML: https://fidryn.onlygass.dev/docs/mill
> This markdown mirror is for agents and plain-text readers.

# Fidryn mill

The mill is a loopback-only web UI for checking, running, exploring, and
rendering Fidryn source on this machine. Start it with the CLI:

```
fidryn ui [--port N] [--no-open]
```

Default port is 8751. The process listens on `127.0.0.1` only. It never
binds `0.0.0.0` or other interfaces. There is no filing route. Live
filing is not available from the mill.

See [getting started](https://fidryn.onlygass.dev/docs/getting-started.md) for a first module, and the
[CLI](https://fidryn.onlygass.dev/docs/cli.md) for path-based `check` / `run` / `explore` / `render` /
`file`.

## Start

From a clone:

```
cargo run -p fidryn-cli -- ui --no-open
```

Or, after `cargo install --path crates/fidryn-cli`:

```
fidryn ui --port 8751 --no-open
```

On success, stderr prints:

```
fidryn mill on http://127.0.0.1:8751
127.0.0.1 only. live filing is disabled.
```

Without `--no-open`, a further line says browser auto-open is not linked
and you should open the URL locally. This build does not launch a
browser. Ctrl-C shuts the server down. If the port is taken, the process
exits 1 with
`127.0.0.1:PORT is already in use. Stop that process or pass --port.`

## Routes

| Method | Path | Body | Success |
| --- | --- | --- | --- |
| `GET` | `/` | none | `web/index.html` (`text/html; charset=utf-8`), with the security headers below |
| `GET` | `/assets/fidryn.css` | none | `site/assets/fidryn.css`, the design system the site also uses (`text/css; charset=utf-8`) |
| `GET` | `/assets/mill.css` | none | `web/mill.css`, the mill's own styles (`text/css; charset=utf-8`) |
| `GET` | `/assets/mill.js` | none | `web/mill.js`, the mill app (`text/javascript; charset=utf-8`) |
| `GET` | `/favicon.svg` | none | `site/favicon.svg` (`image/svg+xml`) |
| `GET` | `/fonts/{name}` | none | `fraunces.woff2`, `plex-sans.woff2`, `plex-mono-400.woff2`, or `plex-mono-500.woff2` from `site/fonts/` (`font/woff2`); any other name is 404 |
| `GET` | `/api/health` | none | plain text `ok` |
| `GET` | `/api/samples` | none | JSON array of the built-in samples, each `{id, title, blurb, source, case, query, validAt, knownAt, action, expect}` |
| `POST` | `/api/check` | JSON `CheckRequest` | JSON `{ok, diagnostics}` |
| `POST` | `/api/run` | JSON `EvalRequest` | `{ "ok": true, "report": <evaluation-report>, "opinion": [<sentence>, ...] }` |
| `POST` | `/api/explore` | JSON `EvalRequest` | same transport as `/api/run` |
| `POST` | `/api/render` | JSON `RenderRequest` | JSON `{ok, text?, error?}` |

The page, stylesheet, favicon, fonts, and samples are compiled into the
`fidryn` binary; the mill reads no files at runtime. The page,
stylesheet, favicon, and fonts are sent with `Cache-Control: no-cache`.
`GET /` also sends
`Content-Security-Policy: default-src 'self'; img-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'`,
`X-Content-Type-Options: nosniff`, and `Referrer-Policy: no-referrer`,
so the browser runs no inline script or style on the page and will not
show it inside a frame. In a sample, `source` is the text of a
repository fixture and `case` is the text of a case file, or the empty
case record for the first two samples (`case` is JSON text, not a
parsed object); `expect` is the outcome kind that `action` returns
for it.

There is no `POST /api/file`, `/api/filing`, `/api/submit`, or
`/api/live`. Those paths return 404.

## JSON field names

Names below are the serde fields the mill actually reads. `EvalRequest`
uses camelCase. `CheckRequest` and `RenderRequest` do not.

### `POST /api/check`

```
{"source": "<fidryn module text>"}
```

| Field | Type | Required |
| --- | --- | --- |
| `source` | string | yes |

HTTP status is 200 in both cases. Success is `{"ok": true, "diagnostics": []}`.
A failed check is `{"ok": false, "diagnostics": [...]}`. Each diagnostic
object has `code`, `message`, `primary_span`, `related_spans`, and
`suggestion` (Rust field names, snake_case).

### `POST /api/run` and `POST /api/explore`

```
{
  "source": "<fidryn module text>",
  "query": "q",
  "case": {},
  "bounds": null,
  "validAt": "2033-01-01T00:00:00Z",
  "knownAt": "2033-01-01T00:00:00Z"
}
```

| Field | Type | Required |
| --- | --- | --- |
| `source` | string | yes |
| `query` | string | yes |
| `case` | JSON value | no (omitted, `{}`, or `null` is an empty case) |
| `bounds` | JSON value or omitted | no; used only by `/api/explore` |
| `validAt` | RFC 3339 string | yes |
| `knownAt` | RFC 3339 string | yes |

`case` must be a `fidryn.case-record/v0.1` object, `{}`, or JSON null.
`bounds` follows the same merge rules as CLI `--bounds`: an
`AdmissibleCompletions` object (`interpretations`, `evidence`, `choices`)
or a wrapper with `admissibleCompletions` / `admissible_completions`.
`/api/run` ignores `bounds`. `/api/explore` still needs a nonempty
completion space on the case, on `bounds`, or both; otherwise the
outcome is an empty completion set.

A nonempty `case.assumptions` array puts mill `/api/run` and
`/api/explore` into scenario mode (`executionMode: scenario` on the
report). There is no separate mill `--scenario` flag; the overlay is
inferred from the case. An empty or omitted `assumptions` list is
operative. CLI path `run` still requires explicit `--scenario` to apply
the same overlay.

`validAt` and `knownAt` accept a `Z` suffix or a numeric offset, the same
as CLI `--valid-at` and `--known-at`.

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

On check failure, HTTP 400:

```
{"ok": false, "error": "check failed", "diagnostics": [...]}
```

Invalid case, bounds, or timestamps are HTTP 400 with `ok` false, an
`error` string (`invalid case: ...`, the bounds merge message, or
`validAt:` / `knownAt:` plus the parse error), and `diagnostics: []`.

Unknown queries and other engine failures are HTTP 400 (HTTP 500 only
when the engine kind is `Internal`):

```
{"kind": "engineError", "error": "UnknownQuery", "message": "...", "ok": false}
```

`error` here is the engine kind (`UnknownQuery`, `Unsupported`,
`FuelExhausted`, `InvalidInput`, `Internal`), not a legal outcome kind.
It is not rewritten as `inconsistent`.

### `POST /api/render`

```
{"source": "<fidryn module text>", "template": "{{module}}@{{version}}"}
```

| Field | Type | Required |
| --- | --- | --- |
| `source` | string | yes |
| `template` | string | yes |

HTTP status is 200 in both cases. Success is `{"ok": true, "text": "..."}`.
Failure (check diagnostics joined by newlines, or a missing template key)
is `{"ok": false, "error": "..."}`. Template variables are `module`,
`version`, and `outside_scope`. Missing keys fail closed.

## In-memory compile versus path compile

Mill `source` is compiled with `compile_source` against
`SourceManifest::default()`. A `source_manifest` header in the pasted
text is not loaded. No artifact files are read. Hex import digests are
not byte-authenticated. No `.fr` or manifest is written to disk.

CLI `fidryn check PATH` (and `run` / `explore` / `verify` / `render` on a
path) uses `Driver::check_path`. That authenticates hex import digests
against files in the module directory. The digest `"fixture"` remains a
test trust profile, not that byte check. Details are in [CLI](https://fidryn.onlygass.dev/docs/cli.md)
under source checking and trust.

CLI path compile loads the declared manifest and, for hex digests, hashes
files next to the module. Mill pasted source does neither. A module that
`fidryn check` accepts can fail in the mill if its imports require a
digest the empty default manifest does not supply. Mill check of a
snippet never proves that on-disk artifacts are intact. Mill must not
gain filesystem access from paste.

## No live filing

The mill does not call filing adapters. `fidryn file` on the [CLI](https://fidryn.onlygass.dev/docs/cli.md)
is the only submit path, and even there live HTTP needs `--live` and
`FIDRYN_ALLOW_LIVE_FILING=1`. A transport receipt is still not `Filed`.
The HTML page states that live filing is not available from the UI.

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

## See also

- [Getting started](https://fidryn.onlygass.dev/docs/getting-started.md)
- [CLI](https://fidryn.onlygass.dev/docs/cli.md)
