# Architecture

Short map of this checkout, taken from [`README.md`](../README.md) and the top-level names. Human docs win if they disagree. Crate internals are not expanded here. The README points at [`docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) for the pipeline.

Fidryn is a programming language for legal instruments. The README says Fidryn 0.2 is a research interpreter that checks and runs modules (`.fr` files), that nothing in the repository is legal advice, and that the modules under `examples/` and the programs under `tests/` exist to exercise the interpreter. `run` does not choose a completion; open branches without a covering certificate stay `Suspended`.

## Crates named in the README

| Crate | Role in the README |
| --- | --- |
| `fidryn-syntax` | Lexer, parser, Rowan CST, formatter |
| `fidryn-hir` | Names, imports, elaboration |
| `fidryn-check` | Types, effects, import authentication; lowering and the checks of one module of edition 2. Depends on `fidryn-provision` for the model of law modules |
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
| `fidryn-provision` | The model of law modules and the rules over it, among them addresses, file paths, text normalization, hashes, text records and the canonical text of a module. Depends on no other crate of the workspace |
| `fidryn-ingest-uscode` | Empty. Of the workspace crates, depends on `fidryn-provision` only |
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
| `web/` | The mill page (`index.html`, `mill.css`, `mill.js`), embedded in the `fidryn` binary by `crates/fidryn-cli/src/ui.rs` |
| `xtask/` | Workspace task runner: `test`, `bench`, `ci`, `examples`, `docs`, `site` (see `.agents/adr/0001-rust-site-generator.md`), `governance`, `package`, `release-check` and `release-notes` |
| `grammar.ebnf` | Present. Not described in the README sections used here |
| `benches/`, `conformance/`, `packages/`, `prelude/`, `schemas/`, `templates/` | Present. Not given a role in those README sections |
| `Cargo.toml` | Workspace manifest |
| `rust-toolchain.toml` | Pins Rust 1.98, as the README states |
| `rustfmt.toml` | Present |
| `README.md` | Overview and quick start |
| `LICENSE` | License |

Quick start from the README: `cargo install --path crates/fidryn-cli --locked`, then `fidryn check tests/programs/require-gate.fr` and `fidryn run` on the same file with `tests/programs/cases/empty.json`. The README runs the tests with `cargo test --workspace --locked` and lists the local mill as `fidryn ui [--port N] [--no-open]`.
