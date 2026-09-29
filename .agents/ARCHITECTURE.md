# Architecture

Short map of this checkout, taken from [`README.md`](../README.md) and the top-level names. Human docs win if they disagree. Crate internals are not expanded here. The README points at [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) for the pipeline.

Fidryn is a programming language for legal instruments. The README says this repository is the v0.1 reference interpreter, a research fixture, not legal advice and not an operative instrument. Source files use `.fr`. `run` does not choose a completion; open branches without a covering certificate stay `Suspended`.

## Crates named in the README

| Crate | Role in the README |
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

Those directories are under `crates/`.

## Top level

| Path | Role |
| --- | --- |
| `crates/` | Workspace crates listed above |
| `examples/` | Fixtures that exercise the interpreter |
| `tests/` | Programs that exercise the interpreter |
| `docs/` | Human handbook. Index: `docs/README.md` |
| `site/` | Static site for fidryn.onlygass.dev. `cargo xtask site` generates the pages, mirrors, and search index from `docs/*.md`; `assets/`, `fonts/`, `favicon.svg`, and `vercel.json` are hand-written |
| `web/` | The mill page (`index.html`, `mill.css`, `mill.js`), embedded in the `fidryn` binary by `fidryn-cli/src/ui.rs` |
| `xtask/` | Workspace task runner: `test`, `bench`, `ci`, and `site` (see `.agents/adr/0001-rust-site-generator.md`) |
| `grammar.ebnf` | Present. Not described in the README sections used here |
| `benches/`, `conformance/`, `packages/`, `prelude/`, `schemas/`, `templates/` | Present. Not given a role in those README sections |
| `Cargo.toml` | Workspace manifest |
| `rust-toolchain.toml` | Pins Rust 1.98, as the README states |
| `rustfmt.toml` | Present |
| `README.md` | Overview and quick start |
| `LICENSE` | License |

Quick start from the README: `cargo test --workspace --offline`, then `cargo run -p fidryn-cli -- check tests/programs/require-gate.fr`. The local mill is `fidryn ui --no-open` on port 8751.
