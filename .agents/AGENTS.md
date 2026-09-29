# Agent contract — Fidryn

Operating contract for agents editing this checkout. Read this file before changing code.

## Do not copy the human handbook

Do not copy human handbooks into `.agents/`. Human docs stay in [`README.md`](../README.md), [`docs/`](../docs/), and [`docs/contributing.md`](../docs/contributing.md). If a human document and a note under `.agents/` disagree, the human document wins.

## Where to look

| Path | What it is |
| --- | --- |
| [`README.md`](../README.md) | What Fidryn is, the quick start, and the crate table |
| [`docs/README.md`](../docs/README.md) | Doc index. The README says to start with [`docs/getting-started.md`](../docs/getting-started.md) |
| [`docs/contributing.md`](../docs/contributing.md) | Contributing notes linked from the README |
| [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) | Pipeline and APIs, as the README labels that file |
| [`docs/implementation-status.md`](../docs/implementation-status.md) | What the README says is actually implemented |
| [`crates/`](../crates/) | Crate workspace. Names are in the README table |
| [`examples/`](../examples/) | Fixtures the README says exercise the interpreter |
| [`tests/`](../tests/) | Programs the README says exercise the interpreter |
| [`site/`](../site/) | Static site. The README points at fidryn.onlygass.dev |
| [`rust-toolchain.toml`](../rust-toolchain.toml) | The README says this pins Rust 1.98 |
| [`Cargo.toml`](../Cargo.toml) | Workspace manifest |
| [`grammar.ebnf`](../grammar.ebnf) | Present at the root. The README sections read here do not describe it |
| [`LICENSE`](../LICENSE) | License |

Other top-level directories are present and are not given a role in the README sections used for this map: `benches/`, `conformance/`, `packages/`, `prelude/`, `schemas/`, `templates/`, `web/`, `xtask/`. Do not invent what they contain.

The README says Fidryn is a programming language for legal instruments and that this repository is the v0.1 reference interpreter: a research fixture, not legal advice and not an operative instrument. Source files use `.fr`.

Commands the README documents:

```bash
cargo test --workspace --offline
cargo run -p fidryn-cli -- check tests/programs/require-gate.fr
cargo install --path crates/fidryn-cli
fidryn check examples/trust/bryan-revocable-trust.fr
fidryn ui --no-open
```

`fidryn run` is also shown, with `--query`, `--case`, `--valid-at`, and `--known-at`. Do not invent a separate lint command.

## Where agent material goes

- Durable facts go in [`MEMORY.md`](MEMORY.md) and notes under [`memory/`](memory/). Session dumps go in [`memory/sessions/`](memory/sessions/).
- Codebase-specific agent documentation goes in [`docs/`](docs/). That directory is not the human [`docs/`](../docs/) tree.
- Decisions are indexed from [`ADR.md`](ADR.md). One decision is one file, `.agents/adr/NNNN-short-title.md`. Do not invent past decisions.
- System shape goes in [`ARCHITECTURE.md`](ARCHITECTURE.md). The human architecture note stays at [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md).
- Durable scripts that are kept and rerun go in [`.agents/scripts/`](scripts/). One-off commands do not land there.
- Reviews, audits, and other working files go in [`scratchpad/`](scratchpad/).

Product code stays in `crates/`, `tests/`, `examples/`, and the other source trees above. Do not relocate it into `.agents/`.
