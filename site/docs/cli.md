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

The binary name is `fidryn`. It is the 0.3 reference interpreter: parse a
`.fr` module, check it, and evaluate a named query against a case record.
For [law modules](https://fidryn.onlygass.dev/docs/law-modules.md) of edition 2 it checks a module or a
workspace ([`check`](#check)), imports the United States Code
([`ingest`](#ingest)), prints the text of a workspace ([`text`](#text))
and its coverage ledger ([`coverage`](#coverage)), and says what changed
between two releases ([`changes`](#changes)). It is not legal advice and
does not file anything unless you opt into live HTTP as described under
[`file`](#file).

From a clone of this repository:

```text
cargo run -p fidryn-cli -- <subcommand> ...
```

After install, the same arguments work as `fidryn <subcommand> ...`.
`fidryn --help` (`-h`) and `fidryn <subcommand> --help` print clap usage,
as does the subcommand `help`, as in `fidryn help check` and
`fidryn ingest help uscode`. `fidryn --version`
(`-V`) prints the name and the version of the binary:

```sh
fidryn --version
```

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

Check a module of either edition, or the workspace of edition 2 modules in
a directory, as `fidryn check --help` says. A module of the first edition
is parsed, elaborated and type-checked, including source-manifest
authentication as described above.

A file whose first two words are `edition 2` is a law module of edition 2,
which holds provisions of law as data
([Law modules](https://fidryn.onlygass.dev/docs/law-modules.md#editions)). `check` reads such a module with
its own parser and runs the checks of one module on it: its shape, its
addresses, the form of its text, its hashes, its footnote markers and its
definitions, as
[What the checker verifies in one module](https://fidryn.onlygass.dev/docs/law-modules.md#what-the-checker-verifies-in-one-module)
lists them. The marker is looked for in a regular file only, so a module
of edition 2 is read from a regular file: an input that is no regular
file, such as a pipe or `/dev/stdin`, is read by every command as a file
of the first edition.

A directory is a workspace of edition 2 modules, with a manifest,
`workspace.json`, at its top ([Workspaces](https://fidryn.onlygass.dev/docs/law-modules.md#workspaces)).
`check` reads every module of it by itself, and then checks the manifest,
the lock of a generated workspace, where each module stands, the
containers and their members, and the dependencies of a curated
workspace, as
[What the checker verifies in a workspace](https://fidryn.onlygass.dev/docs/law-modules.md#what-the-checker-verifies-in-a-workspace)
lists them, and gives every reference and every citation its
[outcome](https://fidryn.onlygass.dev/docs/law-modules.md#references). It does so in two phases, each on
several threads, and its output is the same for any number of them
([How a workspace is checked](https://fidryn.onlygass.dev/docs/law-modules.md#how-a-workspace-is-checked)).
It keeps the result of the first phase in a cache, by default a directory
of the user outside every workspace, so that a module file whose bytes it
has checked before is not checked again, and its output is the same as
without the cache, but for the count of the modules read from it, as long
as nothing but a check wrote the cache
([The cache](https://fidryn.onlygass.dev/docs/law-modules.md#the-cache)). A check trusts its cache, so a
cache belongs where only people one trusts can write.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `PATH` | yes | path to a `.fr` file, or to the directory of a workspace |
| `--jobs N` | no | the number of worker threads of the check of a workspace, at least 1; by default as many as the system reports as available |
| `--cache-dir DIR` | no | the directory of the cache of a workspace check; by default the one `FIDRYN_CACHE_DIR` names, else `fidryn` in the cache directory of the user |
| `--no-cache` | no | flag; keep no cache: check every module file, and write no entry of a cache |
| `--report FILE` | no | path of the check report of a workspace to write, `fidryn.check-report/v0.1` |
| `--unresolved FILE` | no | path of the list to write of the references and the citations of a workspace whose outcome is not `exact`, one JSON object to a line |

**Environment**

| Name | Meaning |
| --- | --- |
| `FIDRYN_CACHE_DIR` | the directory of the cache of a workspace check when `--cache-dir` is not given; an empty value is taken as not set |
| `XDG_CACHE_HOME` | on systems other than Windows, the cache directory of the user when `FIDRYN_CACHE_DIR` is not set, if it is an absolute path: the cache is `fidryn` in it |
| `HOME` | on systems other than Windows, when `XDG_CACHE_HOME` is not an absolute path: the cache is `.cache/fidryn` in it, if it is an absolute path |
| `LOCALAPPDATA` | on Windows, the cache directory of the user when `FIDRYN_CACHE_DIR` is not set, if it is an absolute path: the cache is `fidryn` in it |

The cache is outside every workspace unless `--cache-dir` or
`FIDRYN_CACHE_DIR` names a directory inside one; such a directory is used
like any other, and what that means is said under
[The cache](https://fidryn.onlygass.dev/docs/law-modules.md#the-cache). When none of these names a
directory, the check keeps no cache, as with `--no-cache`, and says so once
on stderr.

`--jobs`, `--cache-dir`, `--no-cache`, `--report` and `--unresolved`
belong to the check of a workspace; given with a `.fr` file, they are
refused with exit 1. `--cache-dir` and `--no-cache` are not given
together. The two files are written whether or not the check finds an
error, and each replaces a file of its name.

**Example**

The demo workspace is checked with `--no-cache`, so that the check writes
no cache:

```sh
fidryn check examples/trust/bryan-revocable-trust.fr
fidryn check crates/fidryn-check/tests/fixtures/e2/unit.fr
fidryn check conformance/fixtures/workspace/demo --no-cache
fidryn check conformance/fixtures/workspace/demo --no-cache --jobs 2
```

```text
fidryn check DIR --report report.json --unresolved unresolved.jsonl
fidryn check DIR --cache-dir /var/cache/fidryn
FIDRYN_CACHE_DIR=/var/cache/fidryn fidryn check DIR
```

**Success.** Prints `ok` and exits 0. For a workspace it prints, before
`ok`, one summary line:

```text
modules: 5; from cache: 0; errors: 0; warnings: 0
```

`from cache` counts the modules whose result was read from the cache;
with a cache that holds nothing yet, or with `--no-cache`, it is 0.

A warning, such as `W760 ToolchainMismatch` when the manifest's
`toolchain` is not the running version, or `W750 StaleBinding` when what a
binding of a curated module was written against has changed
([Bindings](https://fidryn.onlygass.dev/docs/law-modules.md#bindings)), is printed to stderr, counted in
`warnings`, and does not change the exit status. Nor does a reference whose target the check does
not find: its outcome is counted in the report and listed in the
unresolved list, and is never a diagnostic. Nor does a cache that cannot
be written: the check writes no further entry, says so once on stderr, as
`the cache could not be written, and the check went on without it: DIR: ...`
with the cache's directory and the error, and its output is what it would
have been. When the identity of the running executable, which the keys of
the cache hold, cannot be read, the check keeps no cache and says so once,
as `the cache could not be used, and the check went on without it: ...`;
so it does when no directory is named for the cache, as
`the cache could not be used, and the check went on without it: no directory is named for it: ...`,
and when the cache's directory is the directory of a dependency of
the workspace, lies inside it or holds it, since nothing is written inside
a dependency but through a symbolic link below the cache's directory that
leads into one
([Depending on another workspace](https://fidryn.onlygass.dev/docs/law-modules.md#depending-on-another-workspace)),
and its output is then that of a check with `--no-cache`.

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
`cannot write FILE: ...`, also with exit 1. A manifest or a lock is read
only from a regular file of at most 64 MiB: a symbolic link, a named pipe,
a device or a directory in its place, and a larger file, are refused with
`E730` before anything is read of them, as are those of a dependency with
`E740`. A directory with no `workspace.json` is refused with `E730`:

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

## `ingest`

`fidryn ingest uscode ARCHIVE_DIR --out DIR` imports the United States
Code from the archives of one release of the official XML release point
published by the Office of the Law Revision Counsel, each verified against
the retrieval record beside them, and writes a generated workspace that
`fidryn check DIR` accepts
([The ingester for the United States Code](https://fidryn.onlygass.dev/docs/law-modules.md#the-ingester-for-the-united-states-code)).
The same archives, the same titles and the same version of the toolchain
give the same bytes on every run and on every host, for any number of
workers.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `ARCHIVE_DIR` | yes | path to the directory of the archives of one release and their retrieval record, `retrieval.json` |
| `--out DIR` | yes | path to the directory of the workspace: one that does not exist, one that holds nothing but entries whose name begins with a dot, or an earlier output of the command |
| `--title T` | no | a title to write, by the publisher's designation, such as `1` or `5a`, its letter in either case; may be given more than once; by default every title of the release |
| `--jobs N` | no | the number of worker threads, at least 1; by default as many as the system reports as available |
| `--verify` | no | flag; write nothing, and report every entry under `DIR` that differs from what the run would write |

A run writes `workspace.json` first, then each module, each file through
a temporary file that is renamed; it removes the modules of an earlier
output that it does not write, and writes `law.lock.json` and then, last,
`coverage.json`, the [coverage ledger](https://fidryn.onlygass.dev/docs/law-modules.md#the-coverage-ledger)
of the workspace, which [`coverage`](#coverage) prints; so a run that is
stopped leaves a directory the next run accepts. The lock does not list
the ledger. With `--title`,
only the modules of those titles are written, and the lock lists the
archives and members whose modules the run wrote. The numbering of
addresses still runs over every member the archive directory holds,
whichever titles are chosen, so an address does not depend on the
selection.

An earlier output of the command is a generated workspace of the prefix
`/us/usc` whose `.fr` files are each a module of edition 2 whose address
lies below the prefix and which stands at the path of its address. A run
reads the head of every `.fr` file of `DIR`, its marker and the address
after `module`, in its first 64 KiB, before it writes anything, and
removes only such modules.

`--verify` compares `workspace.json`, `law.lock.json`, `coverage.json`
and every module file of `DIR` with what the run would write. Every other
entry that a run would refuse is extra: a file other than a module and
those three at the top, a symbolic link or any other entry that is
neither a file nor a directory, and a `.fr` file that is no earlier output,
which is different instead where the run writes a module. Only the entries
whose name begins with a dot are not looked at. It writes nothing, `DIR`
included. A run stopped after the lock and before the ledger leaves a
workspace that `fidryn check` accepts, since no check reads the ledger,
and `--verify` reports it as
`missing coverage.json`, or `different coverage.json` when the ledger is
that of an earlier run.

**Example**

```text
fidryn ingest uscode samples/uscode/archives --out DIR
fidryn ingest uscode samples/uscode/archives --out DIR --title 1 --jobs 4
fidryn check DIR
fidryn ingest uscode samples/uscode/archives --out DIR --verify
```

**Success.** Prints one line and exits 0:

```text
ingested: U units, C containers from A archives (release R)
```

A noun whose count is 1 is in the singular, as in
`ingested: 1 unit, 1 container from 1 archive (release R)`.

With `--verify`, prints `ok` when nothing differs: `DIR` then holds every
file the run would write, with the same bytes, and nothing a run would
refuse, so a run into it is accepted and changes none of its bytes.

**Failure.** Exit 1. A release that is refused, by its retrieval record or
by a reading rule, is reported with the archive, the member and, for a
document, the element, the line it starts on and the nearest identifier.
A title that no member holds is reported, by the designation given,
before anything is written. A directory that holds anything other than an
earlier output of the command is refused with what it holds, and nothing
in it is changed: every `.fr` file that is no earlier output is named, a
file of the first edition, a module of another prefix and a module at
another path among them. With `--verify`, each path under `DIR` that differs is
printed to stdout, relative to `DIR`, in the byte order of the paths,
after the word that says how: `missing`, `extra` or `different`. The exit
status is 1:

```text
extra notes.txt
extra us/usc/link
different us/usc/t99/s1.fr
extra us/usc/t99/s7.fr
missing us/usc/t99a.fr
```

## `text`

`fidryn text DIR` prints the text of the provisions and notes of the
workspace in `DIR`: of every provision, container, note, source credit and
footnote of its modules, read from its module files
([Workspaces](https://fidryn.onlygass.dev/docs/law-modules.md#workspaces)). With `ADDRESS` it prints the
provision, container or note at that address and what lies below it: every
one whose address is `ADDRESS` or continues it at a segment boundary. With
`ADDRESS` and `--hashes` it prints the two hashes that a binding to that
provision records instead ([Bindings](https://fidryn.onlygass.dev/docs/law-modules.md#bindings)). It reads
the modules and checks nothing else of the workspace, which
`fidryn check DIR` does.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `DIR` | yes | path to the directory of a workspace |
| `ADDRESS` | no | the address of a provision, a container or a note of the workspace; with `--hashes`, of a provision, or an alias of a unit |
| `--json` | no | flag; print one text record to a line |
| `--hashes` | no | flag; print the two lines that a binding to `ADDRESS` records; needs `ADDRESS`, and is not given with `--json` |

**Example**

```sh
fidryn text conformance/fixtures/workspace/demo
fidryn text conformance/fixtures/workspace/demo /zz/demo/t1/s2
fidryn text conformance/fixtures/workspace/demo --json
fidryn text conformance/fixtures/workspace/demo /zz/demo/t1/s2 --hashes
fidryn text samples/uscode/workspace /us/usc/t1/s1
```

**Success.** Exit 0. Without `--json` it prints the text for a person,
module by module in the order of their addresses, with a blank line
between two modules:

- A provision is its address on a line of its own, followed by its status
  in parentheses when it has one; then its number and heading on one line,
  the blocks of its text parts and its children, each indented by two
  spaces for each level of depth. The provision printed first is at depth
  0.
- A table is one line for each row, its cells separated by ` | `. The items
  of quoted content are indented one level further. A footnote marker is
  its shown text in brackets, and an image is `[image SOURCE]`.
- After the provisions of a unit come its source credit and its footnotes,
  each footnote after its marker in brackets, and then its notes, each its
  address with its layer in parentheses, its heading and its blocks. A
  container is its address, its number and heading, its credit, its
  footnotes and its notes.

Of the demo workspace, `fidryn text conformance/fixtures/workspace/demo /zz/demo/t1`
prints:

```text
/zz/demo/t1
  Title 1— Invented Things

/zz/demo/t1/ch1
  Chapter 1— Widgets

/zz/demo/t1/s1
  § 1. Definitions
  In this invented chapter, a widget is a small device.

/zz/demo/t1/s2
  § 2. Sizes
  A widget is small or large.

/zz/demo/t1/s3
  § 3. Colors
  A widget may be of any color.
```

With `--json` it prints one text record, `fidryn.text-record/v0.1`, to a
line, the JSON Schema of which is
[schemas/text-record-v0.1.json](https://github.com/BeeGass/fidryn/blob/main/schemas/text-record-v0.1.json). The
records are those of every module, sorted by the bytes of their addresses
across the whole workspace; for one address the provision or container
comes first, then the source credit, then the footnotes by ordinal. Each
is one line of compact JSON with its keys in this order:

| Record | Keys |
| --- | --- |
| a provision | `address`, `layer` (`"operative"`), `kind`, `status` (the word, or `null`), `num`, `heading`, `items`, where an item is a text part `{"part": role, "blocks": [...]}` or a child `{"child": address}`; the top provision of a unit also `aliases`, `within`, `source` and `positive_law` |
| a container | `address`, `layer` (`"operative"`), `kind`, `status`, `num`, `heading`, `aliases`, `within`, `members` |
| a note | `address`, `layer` (`"enacted"` or `"editorial"`), `group`, `topics`, `heading`, `blocks` |
| a source credit | the address of its module, `record` (`"credit"`), and the keys of a paragraph |
| a footnote | the address of its module, `record` (`"footnote"`), `ordinal`, `shown`, `blocks` |

A text is `{"text": plain, "refs": [[start, end, target], ...], "sup":
[[start, end], ...], "sub": [[start, end], ...]}`, followed by `"marks":
[[offset, shown, footnote], ...]` when it holds a footnote marker. Offsets
count the Unicode scalar values of the plain text, an end is exclusive, and
the offset of a marker is the length of the plain text before it. A
paragraph is a text with the key `p` in place of `text`, a subheading one
with the key `subheading`. A heading is `null` or a text, and a number is
`null` or `[value, shown]`, `shown` the plain string when it is words only
and a text otherwise. A table is `{"table": [[group, [[header, colspan,
rowspan, [blocks]], ...]], ...]}`, an image `{"image": source}`, and
quoted content `{"quoted": {"origin": origin, "items": [...]}}`, whose
items are blocks, text parts and levels `{"level": kind, "num": ...,
"heading": ..., "items": [...]}`. The record of `/zz/demo/t1/s2`:

```text
{"address":"/zz/demo/t1/s2","layer":"operative","kind":"section","status":null,"num":["2","§ 2."],"heading":{"text":"Sizes","refs":[],"sup":[],"sub":[]},"items":[{"part":"content","blocks":[{"p":"A widget is small or large.","refs":[],"sup":[],"sub":[]}]}],"aliases":[],"within":"/zz/demo/t1/ch1","source":"demo01.xml","positive_law":true}
```

Both forms depend on the module files alone: neither the host, the
working directory nor the number of threads the system offers changes a
byte. `conformance/uslm_text.py` prints the same records from the
publisher's XML, independently of the ingester, and the check of the
sample of the Code compares the two line for line
([The sample](https://fidryn.onlygass.dev/docs/law-modules.md#the-sample)).

With `--hashes` it prints exactly two lines and nothing else, the two
that a person writes into a binding after its `release`: the tree hash of
the provision at `ADDRESS`, or of the top provision of the unit that
`ADDRESS` is an alias of, and the hash of the enacted layer of that unit,
each as 64 lower-case hexadecimal digits. They come from the
[interfaces](https://fidryn.onlygass.dev/docs/law-modules.md#interfaces) of the modules, the place the check
of a workspace takes them from, so a binding that records them is not
stale against the workspace they were printed from. Of the demo
workspace, `fidryn text conformance/fixtures/workspace/demo /zz/demo/t1/s2 --hashes`
prints:

```text
provision_hash "238c20bcd64758ed0b7f1e2eed5758d78ff4abda212539e8b9e7a07b7301686f"
notes_hash "a396bac23ba81b30cb34dbf808cf06deeaeb4b87e010b66646faadc6accfc6ff"
```

**Failure.** Exit 1. Nothing is printed to stdout, and the reason is
printed to stderr: a directory that holds no workspace (`E730`), an entry
that cannot be read or is neither a file nor a directory (`E730`), a
module file that is not a module of edition 2 that lowers, each with its
diagnostics; a text that is no address; and an address at which the
workspace holds no provision, container or note, as
``the workspace holds no provision, container or note at `/zz/demo/t1/s9` ``:

```sh exit=1
fidryn text conformance/fixtures/workspace/demo /zz/demo/t1/s9
```

With `--hashes`, an address that is no provision and no alias of a unit is
refused in the same way, saying what it is: the address of a note, of a
container or of a curated module, an alias of a container, or the address
of nothing, as
``the workspace holds no provision, container or note at `/zz/demo/t1/s9` ``.
The chapter of the demo workspace is a container:

```sh exit=1
fidryn text conformance/fixtures/workspace/demo /zz/demo/t1/ch1 --hashes
```

`--hashes` without `ADDRESS`, and `--hashes` with `--json`, are usage
errors, exit 2.

## `coverage`

`fidryn coverage DIR` prints the coverage ledger of the generated workspace
in `DIR`, `fidryn.coverage/v0.1`: its units, containers and provisions,
its notes, footnotes and images, its references of each layer by outcome,
the definitions extracted from its provisions by basis and by kind, and
what its importer counted and did not import
([The coverage ledger](https://fidryn.onlygass.dev/docs/law-modules.md#the-coverage-ledger)). It computes the
ledger from the modules as their files stand, the manifest and the lock.
It does not verify the module files against the lock, which is what
`fidryn check DIR` is for. Of a workspace whose files are those its run
wrote, it prints that run's `coverage.json`, byte for byte.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `DIR` | yes | path to the directory of a generated workspace |

**Example**

```sh
fidryn coverage conformance/fixtures/workspace/demo
fidryn coverage samples/uscode/workspace
```

**Success.** Exit 0. It prints the ledger as JSON with two spaces of
indentation and a final line end, its keys in their order, the JSON Schema
of which is [schemas/coverage-v0.1.json](https://github.com/BeeGass/fidryn/blob/main/schemas/coverage-v0.1.json).
Of the demo workspace it prints first:

```text
{
  "schema": "fidryn.coverage/v0.1",
  "prefix": "/zz/demo",
  "release": "42-7",
  "totals": {
    "units": 3,
    "containers": 2,
    "provisions": 3,
    "units_by_kind": {
      "section": 3
    },
```

The output depends on the files of the workspace alone: neither the host,
the working directory nor the number of threads the system offers changes
a byte. The figures of the sample of the Code are on the page
[Coverage](https://fidryn.onlygass.dev/docs/coverage.md).

**Failure.** Exit 1. Nothing is printed to stdout, and the reason is
printed to stderr: a curated workspace, which has no ledger, as
``DIR holds a curated workspace, and the coverage ledger belongs to a generated workspace, whose lock records what its importer counted``;
a directory that holds no workspace (`E730`); a generated workspace
without a lock, with one that cannot be read, or with one whose prefix or
release is not the manifest's (`E730`); an entry that cannot be read or is
neither a file nor a directory (`E730`); and a module file that is not a
module of edition 2 that lowers, with its diagnostics. The curated sample
of the repository is refused:

```sh exit=1
fidryn coverage samples/uscode/curated
```

## `changes`

`fidryn changes OLD_DIR NEW_DIR` prints the change set between two
generated workspaces of one prefix, the imports of two releases of one
corpus: the provisions, notes and containers added, removed, moved or
amended, by layer ([Change sets](https://fidryn.onlygass.dev/docs/law-modules.md#change-sets)). With
`--bindings DIR` it also lists the bindings of the curated workspace in
`DIR` that are stale against `NEW_DIR`, whatever release that workspace is
pinned to: it answers what would go stale if the pin moved there. Each
workspace is read as `fidryn check` reads it, through phase one and the
cache. Only the modules whose hashes differ are read again, and the units
on one side only whose top provision has the kind and the status of a
provision on the other side only, so that the two could be one moved
provision.

**Arguments**

| Name | Required | Type |
| --- | --- | --- |
| `OLD_DIR` | yes | path to the directory of the older generated workspace |
| `NEW_DIR` | yes | path to the directory of the newer generated workspace, of the prefix of `OLD_DIR` |
| `--bindings DIR` | no | path to the directory of a curated workspace whose bindings are judged against `NEW_DIR`; may be given more than once, and a directory given twice is read once |
| `--json` | no | flag; print the change set as JSON, `fidryn.change-set/v0.1` |
| `--jobs N` | no | the number of worker threads, at least 1; by default as many as the system reports as available |
| `--cache-dir DIR` | no | the directory of the cache of every workspace read; by default the one [`check`](#check) keeps, `FIDRYN_CACHE_DIR` or `fidryn` in the cache directory of the user |
| `--no-cache` | no | flag; keep no cache: check every module file, and write no entry |

The cache is chosen, from the same variables, as by [`check`](#check), and
every workspace the command reads, the two sides and each `--bindings`
directory, shares it; nothing is written into any of them unless the
cache is named inside one. A check trusts its cache, so a cache belongs
where only people one trusts can write. `--cache-dir` and `--no-cache` are
not given together.

**Example**

Each of these compares a workspace with itself, with `--no-cache`, so that
no cache is written; each prints a change set in which nothing changed:

```sh
fidryn changes conformance/fixtures/workspace/demo conformance/fixtures/workspace/demo --no-cache
fidryn changes conformance/fixtures/workspace/demo conformance/fixtures/workspace/demo --no-cache --json
fidryn changes samples/uscode/workspace samples/uscode/workspace --no-cache --bindings samples/uscode/curated
```

```text
fidryn changes OLD_DIR NEW_DIR --bindings CURATED_DIR > message.txt
fidryn changes OLD_DIR NEW_DIR --json --jobs 8 > changes.json
```

**Success.** Exit 0. Without `--json` it prints the change set as the text
of a commit message: a first line `<prefix>: <old release> to <new release>`,
a blank line, one line of counts for each of the four groups, and then
each list that is not empty under its name, one entry to a line. Of the
demo workspace compared with itself:

```text
/zz/demo: 42-7 to 42-7

operative: 0 added, 0 removed, 0 amended_text, 0 amended_structure, 0 moved
enacted_notes: 0 added, 0 removed, 0 amended, 0 moved
editorial: 0 added, 0 removed, 0 amended, 0 moved, 0 credits_changed, 0 footnotes_changed
containers: 0 added, 0 removed, 0 changed
```

With `--json` it prints the change set as pretty JSON, every key present,
the JSON Schema of which is
[schemas/change-set-v0.1.json](https://github.com/BeeGass/fidryn/blob/main/schemas/change-set-v0.1.json). Every list
is sorted, and both forms are the same bytes on every run and for every
number of workers. [Change sets](https://fidryn.onlygass.dev/docs/law-modules.md#change-sets) says what puts
an address into each list and shows the text of one.

When the cache of a workspace could not be used or could not be written,
the command says so once on stderr, as [`check`](#check) does, with
`the cache could not be used, and the check went on without it: ...` or
`the cache could not be written, and the check went on without it: ...`,
and the change set is what it would have been.

**Failure.** Exit 1, with nothing on stdout. A directory that holds no
workspace, a curated workspace as `OLD_DIR` or `NEW_DIR`, a generated one
as `--bindings`, and two workspaces of different prefixes are each refused
with one `E730` that says which, as
``this is the manifest of a curated workspace, and a change set is made between two generated workspaces``.
`conformance/fixtures/workspace` holds no `workspace.json`, and is refused
as no workspace:

```sh exit=1
fidryn changes conformance/fixtures/workspace conformance/fixtures/workspace/demo --no-cache
```

A workspace that has an error ends the command with its diagnostics, each
naming its file under the directory given, as `fidryn check` prints them;
for a curated workspace of `--bindings` these are the diagnostics of its
module files by themselves. When the command keeps no cache because no
directory is named for one, or because the identity of the running
executable cannot be read, it says so after the diagnostics of a failure
too, as `the cache could not be used, and the check went on without it: ...`;
what the checks of the workspaces it read found of their cache is said
only when the comparison succeeds.

## See also

- [Getting started](https://fidryn.onlygass.dev/docs/getting-started.md)
- [Mill](https://fidryn.onlygass.dev/docs/mill.md)
