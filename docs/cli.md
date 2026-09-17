# Fidryn CLI

The binary name is `fidryn`. It is the v0.1 reference interpreter: parse a
`.fr` module, check it, and evaluate a named query against a case record.
It is not legal advice and does not file anything unless you opt into live
HTTP as described under [`file`](#file).

From a clone of this repository:

```
cargo run -p fidryn-cli -- <subcommand> ...
```

After install, the same arguments work as `fidryn <subcommand> ...`.
`fidryn --help` and `fidryn <subcommand> --help` print clap usage.

## Install

From the repository root:

```
cargo install --path crates/fidryn-cli
```

That places `fidryn` on your Cargo bin path. Workspace `rust-version` is
1.98.

## Exit status

Success is exit 0. Failures are exit 1. Compiler diagnostics, engine
errors, adapter errors, and usage problems go to stderr. Successful
payloads (formatted source, the word `ok`, outcome JSON, rendered text,
diff JSON, filing receipts) go to stdout.

A legal `Outcome` is not a process failure. `run` and `explore` print a
`fidryn.outcome/v0.1` document and exit 0 even when the outcome kind is
`suspended`, `contingent`, `normConflict`, `outsideCompetence`, or
`inconsistent`. Unknown queries, invalid timestamps, missing files, and
check errors are engine or compiler failures and exit 1.

## Source checking and trust

`check`, `run`, `explore`, `verify`, `diff` (when a snapshot is a `.fr`
file), and `render` compile through `Driver::check_path`. That call loads
the module’s `source_manifest` (or a `sources/` fallback) and then
`check_with_sources` with `source_root` set to the directory that contains
the `.fr` file. Hex import digests are authenticated against artifact
bytes next to the module:

- a 64-digit hex digest must equal the full blake3 of the file at
  `artifact.path` relative to `source_root`;
- a 32-digit hex digest must equal the first 16 bytes of that hash.

A missing file or a mismatch is diagnostic `E200` for digest-required
imports.

The digest string `"fixture"` is a test trust profile. It permits the
import in this corpus; it is not byte verification of the artifact. Most
example manifests under `examples/` use `"digest": "fixture"`.

Pasted source in the [mill](mill.md) is different: it compiles in memory
with an empty default manifest and does not read artifact files.

## `fmt`

Rewrite a module’s trailing whitespace to stdout. The file on disk is not
modified.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |

**Example**

```
fidryn fmt examples/trust/bryan-revocable-trust.fr
```

**Success.** The formatted source is printed to stdout, including a
trailing newline.

**Failure.** Exit 1. Unreadable files print `cannot read PATH: ...` to
stderr. Parse errors print a diagnostic (`E100 ParseError: ...`).

## `check`

Parse, elaborate, and type-check a module, including source-manifest
authentication as described above.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |

**Example**

```
fidryn check examples/trust/bryan-revocable-trust.fr
```

**Success.** Prints `ok` and exits 0.

**Failure.** Exit 1. Each diagnostic is printed to stderr as
`CODE Title: message` (for example
`E310 PropAsGuard: ...`). A module that cannot check:

```
fidryn check tests/diagnostics/e310-prop-as-guard.fr
```

## `run`

Evaluate one query against one case record at a pair of bitemporal
instants. `run` never chooses a completion. It does not search
`admissibleCompletions`, pick an interpretation, or fill in missing
evidence. `--arg` only writes string facts onto the case.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |
| `--query NAME` | yes | query name (string) |
| `--case RECORD.json` | yes | path to a `fidryn.case-record/v0.1` JSON file |
| `--valid-at TIME` | yes | ISO 8601 / RFC 3339 instant |
| `--known-at TIME` | yes | ISO 8601 / RFC 3339 instant |
| `--arg KEY=VALUE` | no, repeatable | sets `case.facts[KEY]` to the string `VALUE` |

`--valid-at` is valid time. `--known-at` is record time. Both accept a
`Z` suffix or a numeric offset (`+00:00`, `-04:00`). Example spellings:
`2033-01-01T00:00:00Z` and `2033-01-01T00:00:00+00:00`. `--arg provision=ChildSupportWaiver`
writes `case.facts["provision"]`. Bindings without an `=` are ignored.

**Example**

```
fidryn run examples/trust/bryan-revocable-trust.fr \
  --query acting_trustee \
  --case examples/trust/cases/one-certificate.json \
  --valid-at 2026-08-23T12:00:00Z \
  --known-at 2026-08-23T12:00:00Z
```

Override a fact without editing the case file:

```
fidryn run examples/prenup/ava-noah.fr \
  --query provision_result \
  --case examples/prenup/cases/divorce-record.json \
  --valid-at 2026-08-23T12:00:00-07:00 \
  --known-at 2026-08-23T12:00:00-07:00 \
  --arg provision=ChildSupportWaiver
```

**Success.** Canonical JSON for schema `fidryn.outcome/v0.1` on stdout:
`schema`, `module`, `sourceSnapshot`, `query`, `asOf` (`validTime`,
`recordTime`), `modelBoundary`, and `outcome` (`kind`, `trace`, and
kind-specific fields). Exit 0.

**Failure.** Exit 1, stderr only. Check diagnostics; `cannot read PATH`;
`invalid case record: ...`;
`--valid-at` / `--known-at` parse errors; or an engine failure such as
`UnknownQuery: unknown query ...`. Those engine errors are not rewritten
as `outcome.kind = inconsistent`.

## `explore`

Evaluate a query under an explicit finite completion space. The space
must already be on the case as `admissibleCompletions`, or you must pass
`--bounds` to merge more completions into the case. `explore` still does
not invent interpretations; it only examines the declared finite bounds.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |
| `--query NAME` | yes | query name (string) |
| `--case RECORD.json` | yes | path to a case-record JSON file |
| `--bounds BOUNDS.json` | no | JSON file merged into `case.admissibleCompletions` |
| `--valid-at TIME` | yes | RFC 3339 instant |
| `--known-at TIME` | yes | RFC 3339 instant |

`--bounds` may be an `AdmissibleCompletions` object
(`interpretations`, `evidence`, `choices`) or a wrapper that contains
`admissibleCompletions` / `admissible_completions`. A case-record JSON
file that already carries that object is accepted. Entries are merged
(extended), not replaced wholesale.

If neither the case nor `--bounds` supplies a nonempty completion space,
exploration yields an inconsistent outcome whose core includes
`empty completion set`. That is still a successful CLI run (exit 0) with
an outcome document on stdout.

**Example** (case already declares `admissibleCompletions`; `--bounds`
merges another copy of the same family from a second record in the repo):

```
fidryn explore examples/trust/bryan-revocable-trust.fr \
  --query acting_trustee \
  --case examples/trust/cases/one-certificate.json \
  --bounds examples/trust/cases/two-certificates-open-eligibility.json \
  --valid-at 2026-08-23T12:00:00Z \
  --known-at 2026-08-23T12:00:00Z
```

Omit `--bounds` when the case record is already admissible, as
`examples/trust/cases/one-certificate.json` is.

**Success.** The same `fidryn.outcome/v0.1` envelope as `run`, on stdout.

**Failure.** Exit 1. Same compile, case, and timestamp failures as `run`;
`cannot read` / `invalid bounds JSON` for `--bounds`; or
`bounds JSON must be AdmissibleCompletions or {interpretations, evidence, choices}: ...`.
Engine failures print `kind: message` to stderr.

## `explain`

Render a trace as text, JSON, or Graphviz DOT.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `TRACE_ID` | yes | file path, `PATH.json`, or an opaque id string |
| `--format FORMAT` | no | `text` (default), `json`, or `dot` |

If `TRACE_ID` is an existing file, or `TRACE_ID.json` exists, that JSON
is loaded. An outcome document from a prior `run` / `explore` is wrapped
as a small DAG. If no such file exists, the argument is hashed as a
`TraceId` and the node list is empty. JSON always includes a `nodes`
array; emptiness means no persisted DAG was loaded, not an omitted field.
Unknown `--format` values are treated as `text`.

**Example**

```
fidryn run examples/trust/bryan-revocable-trust.fr \
  --query acting_trustee \
  --case examples/trust/cases/one-certificate.json \
  --valid-at 2026-08-23T12:00:00Z \
  --known-at 2026-08-23T12:00:00Z \
  > /tmp/fidryn-outcome.json

fidryn explain /tmp/fidryn-outcome.json --format text
```

Without a file, the id is hashed and the DAG is empty:

```
fidryn explain 00000000000000000000000000000000 --format json
```

**Success.** Formatted trace on stdout. Text with no nodes is
`trace <hex>`. JSON is a canonical `TraceDocument`. DOT is a `digraph`.

**Failure.** Exit 1. `cannot read PATH: ...` or `invalid trace JSON PATH: ...`.

## `verify`

Check a declared `verify` property on a module. Query names are not
properties. The name must appear in the module’s `verify` blocks.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |
| `--property NAME` | yes | declared verification name |

**Example**

```
fidryn verify examples/trust/bryan-revocable-trust.fr --property TrusteeContinuity
```

`TrusteeContinuity` is declared on that module. A bounded check that does
not obtain a covering proof is not success: the CLI prints the verdict to
stderr and exits 1. Only a proved verdict is exit 0 (today that is a
declared `true` or `assert true` formula). Passing a query name such as
`acting_trustee` is invalid.

**Success.** Stdout, for example
`proved NAME (persons=..., events=..., time_points=...)`.

**Failure.** Exit 1, stderr. Check diagnostics, or a verdict such as
`unknown NAME: ...`, `counterexample for NAME: ...`,
`` `NAME` is a query, not a declared verification property ``, or
`unknown property NAME`.

## `diff`

Compare two snapshots and print canonical JSON with two named
operations: `result` (outcomeDocument / compiled query bodies) and
`assurance` (executionMode, sourceTrust, assumptions,
verificationMethod). Each is `{added, removed, changed}`. Changing only
qualifications is an assurance diff, not a silent equal. A `.fr` path is
compiled; any other path is read as JSON (an evaluation report, mill
`{ok, report}` wrapper, outcome document, or serialized module).

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `OLD_SNAPSHOT` | yes | `.fr` path or JSON file |
| `NEW_SNAPSHOT` | yes | `.fr` path or JSON file |
| `--query NAME` | yes | query name used when JSON has no embedded names |

**Example**

```
fidryn diff \
  examples/trust/bryan-revocable-trust.fr \
  examples/california/final-pay/final-pay.fr \
  --query acting_trustee
```

Identical snapshots (the same file twice) print empty arrays under both
`result` and `assurance`.

**Success.** One canonical JSON object on stdout:

```
{"assurance":{"added":[],"changed":[],"removed":[]},"result":{"added":[],"changed":[],"removed":[]}}
```

Each of `result` and `assurance` has keys `added`, `removed`, and
`changed` (sorted arrays of strings).

**Failure.** Exit 1. Compile diagnostics for `.fr` inputs;
`cannot read PATH`; `invalid snapshot JSON PATH: ...`; or
`cannot encode diff: ...`.

## `render`

Interpolate Core fields into a template. Available keys from the compiled
module are `module`, `version`, and `outside_scope`. Missing keys fail
closed. A directory named `certified` does not approve legal content;
`templates/certified/` files are interpolation fixtures only.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |
| `--template TEMPLATE` | yes | path to a template file |

**Example**

```
fidryn render examples/trust/bryan-revocable-trust.fr \
  --template templates/certified/instrument-outline.txt
```

**Success.** Rendered text on stdout (no extra newline beyond the
template).

**Failure.** Exit 1. Check diagnostics; `cannot read TEMPLATE`; or
`missing template key \`...\``.

## `file`

Submit a JSON filing packet through an adapter. Dry-run is the default.
Live HTTP requires both `--live` and the environment variable
`FIDRYN_ALLOW_LIVE_FILING=1`. A successful transport receipt is not a
`Filed` legal fact.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PACKET.json` | yes | JSON file parsed as the packet body |
| `--adapter NAME` | no | `dry-run` (default), `ma-corporations`, or `massachusetts` |
| `--live` | no | flag; enable live HTTP when the adapter and env allow it |
| `--endpoint URL` | no | live POST URL for the Massachusetts adapter |

`--adapter massachusetts` is an alias of `ma-corporations`. Any other
adapter name uses dry-run. The dry-run adapter refuses `--live` even if
the environment variable is set. The Massachusetts adapter’s default
endpoint is `https://www.sec.state.ma.us/filing`.

There is no dedicated packet fixture in-tree; any JSON value is accepted.
The LLC case record is a realistic dry-run body:

```
fidryn file examples/massachusetts-llc/cases/transmitted-without-official-record.json
```

Live shape (does not establish `Filed`):

```
FIDRYN_ALLOW_LIVE_FILING=1 fidryn file \
  examples/massachusetts-llc/cases/transmitted-without-official-record.json \
  --adapter ma-corporations \
  --live \
  --endpoint https://www.sec.state.ma.us/filing
```

**Success.** Adapter JSON on stdout (`kind` is camelCase): `dryRun` with
the packet echoed; or, for live Massachusetts HTTP, `accepted` (with
`filingId` and `raw`), `rejected`, `transportFailure`, or
`submissionUncertain`. `accepted` is a transport receipt.

**Failure.** Exit 1. `cannot read PACKET`; `invalid packet JSON`; or
`live filing is disabled (set FIDRYN_ALLOW_LIVE_FILING=1 and pass --live)`.

The [mill](mill.md) has no filing route.

## `ui`

Serve the localhost mill. See [mill](mill.md) for routes and the HTML
page. The process binds `127.0.0.1` only and does not live-file.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `--port N` | no | `u16` TCP port on loopback (default 8751) |
| `--no-open` | no | flag; do not attempt to open a browser |

This build does not link an opener crate. Without `--no-open`, stderr
notes that browser auto-open is not linked. With `--no-open`, that line
is omitted.

**Example**

```
fidryn ui --port 8751 --no-open
```

**Success.** The server runs until Ctrl-C. Stderr includes
`fidryn mill on http://127.0.0.1:PORT` and
`127.0.0.1 only. live filing is disabled.` Exit 0 after shutdown.

**Failure.** Exit 1.
`127.0.0.1:PORT is already in use. Stop that process or pass --port.`
Other bind or runtime errors are printed to stderr.

## See also

- [Getting started](getting-started.md)
- [Mill](mill.md)
