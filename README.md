# Fidryn

Fidryn (FID-rin) is a programming language for legal instruments: precise
where law is mechanical, explicit where judgment enters, and incapable of
hiding authority, discretion, or ambiguity inside a Boolean.

This repository is the v0.1 reference interpreter. It is a research fixture,
not legal advice, not an operative instrument, and not a complete statement
of any jurisdiction's law. The modules under `examples/` and the records
under `tests/` are fixtures for the interpreter. They are not legal advice.

Source files use the `.fr` extension.

## Governing rule: no false determinacy

A legal computation may return one determinate result only when that result
is invariant across every still-admissible resolution of the unresolved
issues, or when a competent authority has already made a determination that
is operative in the relevant context.

`Determinate` additionally requires a nonempty exhaustive completion set.
An empty set is `Inconsistent` or `Suspended`, never a vacuous determinate
answer. Open branches without a checked convergence certificate yield
`Suspended`. `run` never chooses a completion. Every outcome carries the
declared `modelBoundary` so omitted interpretations cannot silently shrink
the model.

## Build

Rust 1.98 (workspace `rust-version` is 1.98; `rust-toolchain.toml` pins the
patch). Edition 2024. On this Mac, `.cargo/config.toml` points at Command
Line Tools clang because the Xcode license is unsigned.

```
cargo test
cargo run -p fidryn-cli -- check examples/trust/bryan-revocable-trust.fr
```

Example modules live under `examples/` (see `examples/README.md`). They encode
bounded high-impact slices of federal, state, and everyday law. They do
not contain the entire United States Code.

## CLI

The binary name is `fidryn`.

```
fidryn fmt PATH
fidryn check PATH
fidryn run PATH --query NAME --case RECORD.json --valid-at TIME --known-at TIME
    [--arg KEY=VALUE]
fidryn explore PATH --query NAME --case RECORD.json --bounds BOUNDS.json
    --valid-at TIME --known-at TIME
fidryn explain TRACE_ID --format text|json|dot
fidryn verify PATH --property NAME
fidryn diff OLD_SNAPSHOT NEW_SNAPSHOT --query NAME
fidryn render PATH --template TEMPLATE
fidryn file PACKET.json [--adapter dry-run|ma-corporations] [--live] [--endpoint URL]
fidryn ui [--port N] [--no-open]
```

`run` never chooses a completion. `--arg provision=...` sets
`case.facts["provision"]`. `--valid-at` and `--known-at` are ISO 8601 /
RFC 3339 timestamps (`2033-01-01T00:00:00Z` or a numeric offset such as
`+00:00`). `explore` requires explicit finite bounds.

`render` interpolates Core fields (`{{module}}`, `{{version}}`,
`{{outside_scope}}`) into a template. Missing keys fail closed (exit 1).
Templates under `templates/certified/` are the only ones treated as
certified interpolations; they still cannot invent legal content.

`diff` compiles `.fr` snapshots or reads JSON outcomes and prints
canonical JSON `{added, removed, changed}` of query and module names.

`explain` loads `TRACE_ID.json` when that file exists; otherwise it
prints the hashed `TraceId` as text, JSON, or DOT. JSON always includes
a `nodes` array. The array is empty only when no persisted DAG was
loaded.

## Mill

`fidryn ui [--port N] [--no-open]` binds a mill on **127.0.0.1 only**
(default port 8751). It serves `web/index.html` and exposes
`GET /api/health` and `POST /api/check`. Check a module in the browser.
Run, explore, and render remain CLI commands.

The mill does not live-file. There is no filing route. This build does
not link an opener crate; `--no-open` skips any browser launch.

`fidryn file` dry-runs by default. Live HTTP requires both `--live` and
`FIDRYN_ALLOW_LIVE_FILING=1`. A successful transport receipt is not a
`Filed` legal fact.

## Crate layout

```
fidryn-syntax          lossless CST, parser, formatter
fidryn-core            IR, types, Outcome, LegalState, diagnostics
fidryn-hir             names, imports, elaboration  (syntax + core)
fidryn-check           types, effects, authority, time, strata (hir + core)
fidryn-eval            worklist evaluator (core)
fidryn-handlers        CaseFile, Scenario, Explore, Skeptical (core + eval)
fidryn-verify          bounded explorer and invariants (eval + handlers)
fidryn-trace           DAG, canonical JSON, source maps (core)
fidryn-render          constrained templates (missing keys fail closed)
fidryn-adapt           capability-gated filing adapters
fidryn-solve           bounded DPLL over declared finite domains
fidryn-cli             fidryn binary and localhost mill
```

See `docs/ARCHITECTURE.md` and `grammar.ebnf`.
