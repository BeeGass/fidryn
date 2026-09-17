# Fidryn

Fidryn (pronounced **FID-rin**) is a programming language for legal
instruments: precise where law is mechanical, explicit where judgment
enters, and incapable of hiding authority, discretion, or ambiguity
inside a Boolean.

This repository is the v0.1 **reference interpreter**. It is a research
fixture, not legal advice, not an operative instrument, and not a
complete statement of any jurisdiction's law. Modules under `examples/`
and programs under `tests/` exist to exercise the interpreter.

Source files use the `.fr` extension.

**Documentation:** [docs/README.md](docs/README.md) — start with
[Getting started](docs/getting-started.md).

## No false determinacy

A computation may return one determinate result only when that result
is invariant across every still-admissible resolution of the unresolved
issues, or when a competent authority has already made a determination
that is operative in the relevant context.

`run` never chooses a completion. An empty completion set is not a
vacuous determinate answer. Open branches without a covering certificate
stay `Suspended`. Every outcome carries the declared `modelBoundary`.

## Quick start

Rust 1.98 (`rust-toolchain.toml` pins the patch). From the repository
root:

```
cargo test --workspace --offline
cargo run -p fidryn-cli -- check tests/programs/require-gate.fr
```

Install the binary:

```
cargo install --path crates/fidryn-cli
fidryn check examples/trust/bryan-revocable-trust.fr
```

Run a query against a case record (RFC 3339 times are required):

```
fidryn run examples/tax/federal-tax.fr \
  --query tax_on \
  --case examples/tax/cases/ordinary-income.json \
  --valid-at 2034-03-01T09:00:00Z \
  --known-at 2034-03-01T09:00:00Z
```

Local mill (loopback only, default port 8751):

```
fidryn ui --no-open
```

Then open `http://127.0.0.1:8751`. See [the mill guide](docs/mill.md).

## What to read next

| If you want to… | Read |
| --- | --- |
| Check and run a first module | [Getting started](docs/getting-started.md) |
| Write `.fr` | [Language](docs/language.md) |
| Use every subcommand | [CLI](docs/cli.md) |
| Supply facts and evidence | [Cases and time](docs/cases-and-time.md) |
| Interpret `Determinate` vs `Suspended` | [Outcomes](docs/outcomes.md) |
| Browse fixtures | [Examples](docs/examples.md) |
| Change the interpreter | [Contributing](docs/contributing.md) |

## CLI at a glance

The binary name is `fidryn`. Full flags are in [docs/cli.md](docs/cli.md).

```
fidryn fmt PATH
fidryn check PATH
fidryn run PATH --query NAME --case RECORD.json --valid-at TIME --known-at TIME [--arg KEY=VALUE]
fidryn explore PATH --query NAME --case RECORD.json [--bounds BOUNDS.json] --valid-at TIME --known-at TIME
fidryn explain TRACE_ID --format text|json|dot
fidryn verify PATH --property NAME
fidryn diff OLD_SNAPSHOT NEW_SNAPSHOT --query NAME
fidryn render PATH --template TEMPLATE
fidryn file PACKET.json [--adapter dry-run|ma-corporations] [--live] [--endpoint URL]
fidryn ui [--port N] [--no-open]
```

`--arg provision=...` writes `case.facts["provision"]`. Times are ISO 8601
/ RFC 3339. `explore` searches only the declared finite completion space.
`file` is dry-run unless both `--live` and `FIDRYN_ALLOW_LIVE_FILING=1`
are set; a transport receipt is not a `Filed` fact.

## Examples

Bounded high-impact slices, not the United States Code:

- Trust successor occupancy: `examples/trust/bryan-revocable-trust.fr`
- Federal ordinary-income tax: `examples/tax/federal-tax.fr`
- Fifty-state corpus: `examples/states/` (quality bar: Florida homestead)
- Independent programs: `tests/programs/` (require-gate, late-payment)

Catalog: [docs/examples.md](docs/examples.md) and
[examples/README.md](examples/README.md).

## Crates

| Crate | Role |
| --- | --- |
| `fidryn-syntax` | Lexer, parser, Rowan CST, formatter |
| `fidryn-hir` | Names, imports, elaboration |
| `fidryn-check` | Types, effects, import authentication |
| `fidryn-core` | IR, values, outcomes, legal state |
| `fidryn-eval` | Worklist evaluator |
| `fidryn-handlers` | CaseFile, Explore, Skeptical |
| `fidryn-verify` | Determinacy search |
| `fidryn-kernel` | Covering-certificate checks |
| `fidryn-solve` | Streamed finite-domain search |
| `fidryn-driver` | Compile and run session |
| `fidryn-trace` | Outcome JSON and traces |
| `fidryn-render` | Constrained templates |
| `fidryn-adapt` | Filing adapters |
| `fidryn-cli` | `fidryn` binary and mill |

Pipeline and APIs: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). What is
actually implemented: [docs/implementation-status.md](docs/implementation-status.md).
