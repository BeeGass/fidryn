---
title: "CLI"
description: "Every fidryn CLI subcommand and flag the reference binary accepts."
url: "https://fidryn.onlygass.dev/docs/cli"
markdown: "https://fidryn.onlygass.dev/docs/cli.md"
author: "Bryan Gass"
---

> Canonical HTML: https://fidryn.onlygass.dev/docs/cli
> This markdown mirror is for agents and plain-text readers.

# Fidryn CLI

The binary name is `fidryn`. It is the 0.2 reference interpreter: parse a
`.fr` module, check it, and evaluate a named query against a case record.
It is not legal advice and does not file anything unless you opt into live
HTTP as described under [`file`](#file).

From a clone of this repository:

```text
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

Success is exit 0. A command line the argument parser rejects, such as an
unknown subcommand or flag, a missing required argument, or `--budget abc`,
is a usage error: the usage message goes to stderr and the exit status is
2. Every other failure is exit 1. Compiler diagnostics, engine
errors, adapter errors, and usage problems go to stderr. Successful
payloads (formatted source, the word `ok`, evaluation-report JSON, rendered text,
diff JSON, filing receipts) go to stdout.

A legal `Outcome` is not a process failure. `run` and `explore` print a
`fidryn.evaluation-report/v0.2` document (with nested
`outcomeDocument` of schema `fidryn.outcome/v0.1`) and exit 0 even when
the nested outcome kind is `suspended`, `contingent`, `normConflict`,
`outsideCompetence`, or `inconsistent`. Unknown queries, invalid
timestamps, missing files, and check errors are engine or compiler
failures and exit 1.

## Source checking and trust

`check`, `inputs`, `run`, `explore`, `review`, `verify`, `diff` (when a snapshot is a `.fr`
file), and `render` compile through `Driver::check_path`. That call loads
the module’s `source_manifest` (or a `sources/` fallback) and then
`check_with_sources` with `source_root` set to the directory that contains
the `.fr` file, or the explicit global `--source-root PATH`. Static paths and
symlinks must remain inside that root. Hex import digests are authenticated against artifact
bytes next to the module:

- a 64-digit hex digest must equal the full blake3 of the file at
  `artifact.path` relative to `source_root`;
- a 32-digit hex digest must equal the first 16 bytes of that hash.

A missing file or a mismatch is diagnostic `E200` for digest-required
imports.

The digest string `"fixture"` is a test trust profile. It permits the
import in this corpus; it is not byte verification of the artifact. Most
example manifests under `examples/` use `"digest": "fixture"`.

Pasted source in the [mill](https://fidryn.onlygass.dev/docs/mill.md) is different: it compiles in memory
with an empty default manifest and does not read artifact files.

## `fmt`

Rewrite a module’s trailing whitespace to stdout. The file on disk is not
modified.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |

**Example**

```sh
fidryn fmt examples/trust/bryan-revocable-trust.fr
```

**Success.** The formatted source is printed to stdout, including a
trailing newline.

**Failure.** Exit 1. Unreadable files print `cannot read PATH: ...` to
stderr. Parse errors print a diagnostic (`E100 ParseError: ...`).

## `check`

Parse, elaborate, and type-check a module, including source-manifest
authentication as described above.

A file whose first two words are `edition 2` is a law module of edition 2,
which holds provisions of law as data
([Law modules](https://github.com/BeeGass/fidryn/blob/main/docs/law-modules.md#edition-2)). `check` reads it with its own
parser and runs the checks of one module on it: its shape, its addresses,
the form of its text, its hashes, its footnote markers and its
definitions, as
[What the checker verifies in one module](https://github.com/BeeGass/fidryn/blob/main/docs/law-modules.md#what-the-checker-verifies-in-one-module)
lists them.

A directory is a workspace of edition 2 modules, with a manifest,
`workspace.json`, at its top ([Workspaces](https://github.com/BeeGass/fidryn/blob/main/docs/law-modules.md#workspaces)).
`check` reads every module of it by itself, and then checks the manifest,
the lock of a generated workspace, where each module stands, and the
containers and their members, as
[What the checker verifies in a workspace](https://github.com/BeeGass/fidryn/blob/main/docs/law-modules.md#what-the-checker-verifies-in-a-workspace)
lists them, and gives every reference its
[outcome](https://github.com/BeeGass/fidryn/blob/main/docs/law-modules.md#references). It does so in two phases, each on
several threads, and its output is the same for any number of them
([How a workspace is checked](https://github.com/BeeGass/fidryn/blob/main/docs/law-modules.md#how-a-workspace-is-checked)).

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file, or to the directory of a workspace |
| `--jobs N` | no | the number of worker threads of the check of a workspace, at least 1; by default as many as the system reports as available |
| `--report FILE` | no | path of the check report of a workspace to write, `fidryn.check-report/v0.1` |
| `--unresolved FILE` | no | path of the list to write of the references of a workspace whose outcome is not `exact`, one JSON object to a line |

`--jobs`, `--report` and `--unresolved` belong to the check of a
workspace; given with a `.fr` file, they are refused with exit 1. The two
files are written whether or not the check finds an error, and each
replaces a file of its name.

**Example**

```sh
fidryn check examples/trust/bryan-revocable-trust.fr
fidryn check crates/fidryn-check/tests/fixtures/e2/unit.fr
fidryn check conformance/fixtures/workspace/demo
fidryn check conformance/fixtures/workspace/demo --jobs 2
```

```text
fidryn check DIR --report report.json --unresolved unresolved.jsonl
```

**Success.** Prints `ok` and exits 0. For a workspace it prints, before
`ok`, one summary line:

```text
modules: 5; from cache: 0; errors: 0; warnings: 0
```

A warning, such as `W760 ToolchainMismatch` when the manifest's
`toolchain` is not the running version, is printed to stderr and does not
change the exit status. Nor does a reference whose target the check does
not find: its outcome is counted in the report and listed in the
unresolved list, and is never a diagnostic.

**Failure.** Exit 1. Each diagnostic is printed to stderr as
`CODE Title: message`, after the file it is in when it is about the text
of a module (for example
`tests/diagnostics/e310-prop-as-guard.fr: E310 PropAsGuard: ...`). A
module that cannot check:

```sh exit=1
fidryn check tests/diagnostics/e310-prop-as-guard.fr
```

A module of edition 2 is reported in the same way, as
`FILE: E710 IntegrityMismatch: ...`. A diagnostic of a workspace names its
file under the directory given, as
`DIR/zz/demo/t1/s2.fr: E730 WorkspaceViolation: ...`; the summary line is
printed to stdout, its errors counted, and `ok` is not. A file of
`--report` or `--unresolved` that cannot be written is reported as
`cannot write FILE: ...`, also with exit 1. A directory with no
`workspace.json` is refused with `E730`:

```sh exit=1
fidryn check conformance/fixtures/workspace
```

Every other command exits 1 on a module of edition 2, with
``E100 ParseError: an edition 2 module holds provisions and is not compiled for evaluation; `fidryn check` reads it``:

```sh exit=1
fidryn fmt crates/fidryn-check/tests/fixtures/e2/unit.fr
```

## `run`

Evaluate one query against one case record at a pair of bitemporal
instants. `run` never chooses a completion. It does not search
`admissibleCompletions`, pick an interpretation, or fill in missing
evidence. `--arg KEY=VALUE` writes a typed fact onto the case (`case.facts[KEY]`): JSON literals and tagged
values are decoded, while unquoted names remain strings.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |
| `--query NAME` | yes | query name (string) |
| `--case RECORD.json` | yes | path to a `fidryn.case-record/v0.1` JSON file |
| `--valid-at TIME` | yes | ISO 8601 / RFC 3339 instant |
| `--known-at TIME` | yes | ISO 8601 / RFC 3339 instant |
| `--arg KEY=VALUE` | no, repeatable | sets `case.facts[KEY]` to decoded JSON or an unquoted name string |
| `--budget N` | no | maximum actual evaluator operations; exhaustion is an engine failure |
| `--scenario` | no | flag; evaluate with `case.assumptions` as a scenario overlay (`executionMode: scenario`) |

`--valid-at` is valid time. `--known-at` is record time. Both accept a
`Z` suffix or a numeric offset (`+00:00`, `-04:00`). Example spellings:
`2033-01-01T00:00:00Z` and `2033-01-01T00:00:00+00:00`. `--arg provision=ChildSupportWaiver`
writes `case.facts["provision"]`. A binding without `=` is an error.
Without `--scenario`, evaluation is operative: `case.assumptions` are
not applied as an overlay. With `--scenario`, the report keeps
`executionMode: scenario` and serializes `assumptions` (an empty array
when the case has none).

**Example**

```sh
fidryn run examples/trust/bryan-revocable-trust.fr \
  --query acting_trustee \
  --case examples/trust/cases/one-certificate.json \
  --valid-at 2026-08-23T12:00:00Z \
  --known-at 2026-08-23T12:00:00Z
```

Override a fact without editing the case file:

```sh
fidryn run examples/prenup/ava-noah.fr \
  --query provision_result \
  --case examples/prenup/cases/divorce-record.json \
  --valid-at 2026-08-23T12:00:00-07:00 \
  --known-at 2026-08-23T12:00:00-07:00 \
  --arg provision=ChildSupportWaiver
```

**Success.** Canonical JSON for schema `fidryn.evaluation-report/v0.2`
on stdout: `schema`, `executionMode` (`operative` or `scenario`),
`sourceTrust`, `verificationMethod`, `assumptions`, optional `coverage`,
`invocation`, `invocationId`, `engineVersion`, `engineArtifact`, source bundle
identity, retained `traceDocument`, and `outcomeDocument`. Exploration also
retains checked branch `searchEvidence`. Nested `outcomeDocument` is the
`fidryn.outcome/v0.1` projection (`module`, `sourceSnapshot`, `query`,
`asOf`, `modelBoundary`, `outcome`). Exit 0. See [Outcomes](https://fidryn.onlygass.dev/docs/outcomes.md).

**Failure.** Exit 1, stderr only. Check diagnostics; `cannot read PATH`;
`invalid case record: ...`;
`--valid-at` / `--known-at` parse errors; or an engine failure such as
`UnknownQuery: unknown query ...`. Those engine errors are not rewritten
as `outcome.kind = inconsistent`.

## `explore`

Evaluate a query under an explicit finite completion space: the case's
`admissibleCompletions`, with any completions from `--bounds` merged in.
When neither supplies a completion, `explore` evaluates the case as it
stands (see below). `explore` does not invent interpretations; it only
examines the declared finite bounds.

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

When neither the case nor `--bounds` supplies any completions, `explore`
evaluates the case as it stands. A query that needs no completion is
answered: with the empty case record `tests/programs/cases/empty.json`,
query `q` of `tests/programs/require-gate.fr` is determinate `7`. A query
that needs an interpretation, evidence or a choice that the case does not
supply is `suspended`, with a request for it.

```sh
fidryn explore tests/programs/require-gate.fr \
  --query q \
  --case tests/programs/cases/empty.json \
  --valid-at 2026-09-17T12:00:00Z \
  --known-at 2026-09-17T12:00:00Z
```

The outcome is `inconsistent`, with `empty completion set` in its core,
when the case or `--bounds` lists an interpretation family or a choice
with no alternatives, such as `"SuccessorEligibility": []`. That happens
whatever the query asks. Each of these is a successful CLI run (exit 0)
with an outcome document on stdout.

**Example** (case already declares `admissibleCompletions`; `--bounds`
merges another copy of the same family from a second record in the repo):

```sh
fidryn explore examples/trust/bryan-revocable-trust.fr \
  --query acting_trustee \
  --case examples/trust/cases/one-certificate.json \
  --bounds examples/trust/cases/two-certificates-open-eligibility.json \
  --valid-at 2026-08-23T12:00:00Z \
  --known-at 2026-08-23T12:00:00Z
```

Omit `--bounds` when the case record is already admissible, as
`examples/trust/cases/one-certificate.json` is.

**Success.** The same `fidryn.evaluation-report/v0.2` envelope as `run`,
on stdout. If `case.assumptions` is nonempty, explore labels the report
`executionMode: scenario` (there is no separate `--scenario` flag on
`explore`).

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
is loaded. A saved v0.2 report from `run` / `explore` retains the actual
parented derivation in `traceDocument`; `explain` uses those nodes. Legacy
outcome-only documents have only the provenance their compatibility projection
retained. If no such file exists, the argument is hashed as a
`TraceId` and the node list is empty. JSON always includes a `nodes`
array; emptiness means no persisted DAG was loaded, not an omitted field.
Unknown `--format` values are treated as `text`.

**Example**

```text
fidryn run examples/trust/bryan-revocable-trust.fr \
  --query acting_trustee \
  --case examples/trust/cases/one-certificate.json \
  --valid-at 2026-08-23T12:00:00Z \
  --known-at 2026-08-23T12:00:00Z \
  > outcome.json

fidryn explain outcome.json --format text
```

The first command writes `outcome.json` in the working directory.

Without a file, the id is hashed and the DAG is empty:

```sh
fidryn explain 00000000000000000000000000000000 --format json
```

**Success.** Formatted trace on stdout. Text with no nodes is
`trace <hex>`. JSON is a canonical `TraceDocument`. DOT is a `digraph`.

**Failure.** Exit 1. `cannot read PATH: ...` or `invalid trace JSON PATH: ...`.

## `verify`

Check a property of a module. A `verify` block declares one under its
name. A `for_all` or an `exists` written in a query, in a `verify` block
or as a declaration of its own is a property as well. It is named after
its kind, its variable and its number among the quantifiers written in
those places, counted from 0 in the order of the text: `for_all:x:0`.
Such a property has no bounds, and its verdict is `unknown`. Query names
are not properties.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |
| `--property NAME` | yes | property name |

**Example**

```sh exit=1
fidryn verify examples/trust/bryan-revocable-trust.fr --property TrusteeContinuity
```

`TrusteeContinuity` is declared on that module. A bounded check that does
not obtain a covering proof is not success: the CLI prints the verdict to
stderr and exits 1. Only a proved verdict is exit 0. The bounded supported
fragment includes structured Boolean/arithmetic assertions and finite quantified
formulas; every assertion in the named block must hold.
[The fields of a verify block](https://fidryn.onlygass.dev/docs/language.md#the-fields-of-a-verify-block)
says which parts of a block are its assertions. Unsupported formulas, a
block with no assertion and resource exhaustion remain `unknown`. Passing a
query name such as `acting_trustee` is invalid.

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
verificationMethod, coverage, invocation/source/executable identity and
exploration evidence). Each is `{added, removed, changed}`. Changing only
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

```sh
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
module are `module`, `version`, typed `outside_scope` and its text
`outside_scope_label`. Optional `--data DATA.json` adds typed values under `data`.
`{{#each data.people}}{{name}}{{/each}}` iterates records without splitting names
on punctuation. Missing keys fail
closed. A directory named `certified` does not approve legal content;
`templates/certified/` files are interpolation fixtures only.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file |
| `--template TEMPLATE` | yes | path to a template file |
| `--data DATA.json` | no | typed JSON under the `data` template key |

**Example**

```sh
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
| `PACKET.json` | yes | JSON file holding one Fidryn value in tagged form (see below) |
| `--adapter NAME` | no | `dry-run` (default), `ma-corporations`, or `massachusetts` |
| `--live` | no | flag; enable live HTTP when the adapter and env allow it |
| `--endpoint URL` | no | live POST URL for the Massachusetts adapter |

`--adapter massachusetts` is an alias of `ma-corporations`. Any other
adapter name uses dry-run. The dry-run adapter refuses `--live` even if
the environment variable is set. The Massachusetts adapter’s default
endpoint is `https://www.sec.state.ma.us/filing`.

A packet is one Fidryn value in its tagged JSON form: an object whose
`kind` names the kind of value, such as `string`, `int`, `bool` or `map`,
and whose `data` holds it, as in `{"kind":"string","data":"x"}`. A `map`
holds named values in the same form. Other JSON, a case record included,
is refused with `invalid packet JSON` and exit 1.
[`tests/programs/packets/dry-run.json`](https://github.com/BeeGass/fidryn/blob/main/tests/programs/packets/dry-run.json)
is a small packet: a `map` of two strings.

Without `--live`, every adapter answers as the dry-run adapter does: it
prints the packet back as `{"kind":"dryRun","packet":...}`, makes no
network request and writes no file.

```sh
fidryn file tests/programs/packets/dry-run.json
```

Live shape (does not establish `Filed`):

```text
FIDRYN_ALLOW_LIVE_FILING=1 fidryn file \
  tests/programs/packets/dry-run.json \
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

The [mill](https://fidryn.onlygass.dev/docs/mill.md) has no filing route.

## `ui`

Serve the localhost mill. See [mill](https://fidryn.onlygass.dev/docs/mill.md) for routes and the HTML
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

```sh
fidryn ui --port 8751 --no-open
```

**Success.** The server runs until Ctrl-C. Stderr includes
`fidryn mill on http://127.0.0.1:PORT` and
`127.0.0.1 only. live filing is disabled.` Exit 0 after shutdown.

**Failure.** Exit 1.
`127.0.0.1:PORT is already in use. Stop that process or pass --port.`
Other bind or runtime errors are printed to stderr.

## `inputs`

`fidryn inputs MODULE --query QUERY` emits JSON Schema for the declared query
argument object. Aliases, sum constructors, records, nominal entities and
collections come from checked declarations. Observation schemas and external
review requests appear as metadata. Unsupported representations refuse instead
of producing an unconstrained input schema.

## `review`

`fidryn review MODULE --query QUERY --case CASE.json --valid-at TIME --known-at
TIME` emits a portable `fidryn.review-bundle/v0.1` JSON archive. It retains exact
source/case/dependency/lock bytes, a labeled normalized manifest, checked report
and actual trace, source spans, executable digest and rerun arguments. Optional
`--scenario`, `--budget N` and `--template FILE` select the disclosed evaluation
or retain reviewer material. Reviewer status is `not_assessed`; the archive does
not constitute legal approval. See [Development and promotion](https://github.com/BeeGass/fidryn/blob/main/docs/development.md).

## See also

- [Getting started](https://fidryn.onlygass.dev/docs/getting-started.md)
- [Mill](https://fidryn.onlygass.dev/docs/mill.md)
