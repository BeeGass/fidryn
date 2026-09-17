# Fidryn mill

The mill is a loopback-only web UI for checking, running, exploring, and
rendering Fidryn source on this machine. Start it with the CLI:

```
fidryn ui [--port N] [--no-open]
```

Default port is 8751. The process listens on `127.0.0.1` only. It never
binds `0.0.0.0` or other interfaces. There is no filing route. Live
filing is not available from the mill.

See [getting started](getting-started.md) for a first module, and the
[CLI](cli.md) for path-based `check` / `run` / `explore` / `render` /
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
| `GET` | `/` | none | `web/index.html` (`text/html; charset=utf-8`) |
| `GET` | `/api/health` | none | plain text `ok` |
| `POST` | `/api/check` | JSON `CheckRequest` | JSON `{ok, diagnostics}` |
| `POST` | `/api/run` | JSON `EvalRequest` | `{ "ok": true, "report": <evaluation-report> }` |
| `POST` | `/api/explore` | JSON `EvalRequest` | same transport as `/api/run` |
| `POST` | `/api/render` | JSON `RenderRequest` | JSON `{ok, text?, error?}` |

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
`{ "ok": true, "report": ... }`. `report` is the
`fidryn.evaluation-report/v0.1` envelope the CLI prints
(`schema`, `executionMode`, `assumptions`, `sourceTrust`,
`verificationMethod`, `coverage`, `outcomeDocument`). `ok` is not a
field of that report schema (`additionalProperties` is false). Nested
`outcomeDocument` is the `fidryn.outcome/v0.1` projection. Pasted
compile is `sourceTrust: unauthenticated`. `run` never chooses a
completion.

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
test trust profile, not that byte check. Details are in [CLI](cli.md)
under source checking and trust.

CLI path compile loads the declared manifest and, for hex digests, hashes
files next to the module. Mill pasted source does neither. A module that
`fidryn check` accepts can fail in the mill if its imports require a
digest the empty default manifest does not supply. Mill check of a
snippet never proves that on-disk artifacts are intact. Mill must not
gain filesystem access from paste.

## No live filing

The mill does not call filing adapters. `fidryn file` on the [CLI](cli.md)
is the only submit path, and even there live HTTP needs `--live` and
`FIDRYN_ALLOW_LIVE_FILING=1`. A transport receipt is still not `Filed`.
The HTML page states that live filing is not available from the UI.

## The HTML page

`GET /` serves `web/index.html`. The page is a single column:

1. **Header.** Title `fidryn mill` with subtitle `localhost 127.0.0.1`,
   and a health badge. On load the page `GET`s `/api/health` and shows
   `health: ok (localhost)` or `health: unreachable`.
2. **Deck.** “Check, run, explore, and render a Fidryn module on this
   machine. This mill never files.”
3. **Warning.** “Live filing is not available from the UI.”
4. **Module source.** Textarea `source`, prefilled with a tiny
   `Examples.T` module and query `q`.
5. **Query.** Text input, default `q`.
6. **Times.** `validAt` and `knownAt`, default `2033-01-01T00:00:00Z`.
7. **Case JSON.** Textarea, default `{}`. Completions for Explore belong
   here as `admissibleCompletions` on the case. The page does not send a
   separate `bounds` field.
8. **Template.** Textarea for Render, default
   `{{module}}@{{version}}` plus an `outside_scope` each-block.
9. **Buttons.** `Check` (primary), `Run`, `Explore`, `Render`.
10. **Outcome.** A `<pre>` that starts as `Ready.`

`Check` posts `{source}` to `/api/check`. On `ok` the outcome area shows
the word `ok`. Otherwise it lists `code` and `message` for each
diagnostic.

`Run` posts `{source, query, case, validAt, knownAt}` to `/api/run` and
pretty-prints the JSON response.

`Explore` posts the same body to `/api/explore` (no `bounds` key from the
page). Put finite completions on the case JSON if the query needs them.

`Render` posts `{source, template}` to `/api/render`. On `ok` the outcome
area shows `text`. Otherwise it shows `error`.

Invalid case JSON in the textarea fails in the browser before the
request is sent.

## See also

- [Getting started](getting-started.md)
- [CLI](cli.md)
