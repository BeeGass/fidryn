# Contributing to Fidryn

This file is for people building the v0.1 reference interpreter. It is
not a language tutorial and not legal advice. Product usage lives in
the [README](../README.md) and the [documentation hub](README.md).

Fidryn (FID-rin) is a research fixture. Modules under `examples/` and
records under `tests/` exercise the interpreter. They are not operative
instruments and not a complete statement of any jurisdiction's law.

## Toolchain

Rust 1.98, edition 2024. Workspace `package.rust-version` is 1.98.
[`rust-toolchain.toml`](../rust-toolchain.toml) pins the 1.98.1
channel and the `rustfmt` and `clippy` components.

If `cargo` or `git` pick the wrong tools on this Mac, a working PATH is
`/opt/homebrew/bin:$HOME/.cargo/bin:/usr/bin:/bin`, with
`DEVELOPER_DIR=/Library/Developer/CommandLineTools`.
[`.cargo/config.toml`](../.cargo/config.toml) already points the linker
at Command Line Tools clang because the Xcode license is unsigned.

## Checks

From the repository root:

```
cargo fmt
cargo clippy --workspace -- -D warnings
cargo test --workspace --offline
```

Do not weaken no-false-determinacy to make a test pass.

## Pipeline

Land a change in the crate that owns that stage:

```
syntax → hir → check → core → eval → handlers → verify → driver → cli
```

`fidryn-kernel` sits beside verify: it accepts or rejects covering
claims and does not search. `fidryn-trace`, `fidryn-solve`,
`fidryn-adapt`, and `fidryn-render` support the pipeline; they are not
extra stages.

Do not add reverse dependencies. Do not put evaluator logic in
`syntax`. Do not put parser logic in `core`. `fidryn-driver` must not
depend on `fidryn-cli`.

## Crates

Workspace members are listed in [`Cargo.toml`](../Cargo.toml).

```
fidryn-syntax          lossless CST, parser, formatter
fidryn-core            IR, types, Outcome, LegalState, diagnostics
fidryn-hir             names, imports, elaboration  (syntax + core)
fidryn-check           types, effects, authority, time, strata (hir + core)
fidryn-eval            worklist evaluator (core)
fidryn-handlers        CaseFile, Scenario, Explore, Skeptical (core + eval)
fidryn-verify          bounded explorer and invariants (eval + handlers)
fidryn-kernel          covering-check boundary (accept/reject; no proof gen)
fidryn-driver          incremental check/run memo
fidryn-trace           DAG, canonical JSON, source maps (core)
fidryn-render          constrained templates (missing keys fail closed)
fidryn-adapt           capability-gated filing adapters
fidryn-solve           bounded DPLL over declared finite domains
fidryn-cli             fidryn binary and localhost mill
```

## Tests

Tests live next to the crates they exercise (`#[cfg(test)]` in `src`,
and `crates/<name>/tests/` when a crate has an integration kit).

The adversarial review kit is
[`crates/fidryn-cli/tests/adversarial_regressions.rs`](../crates/fidryn-cli/tests/adversarial_regressions.rs).
Those tests are regressions, not closure of the language.

Independent programs live under [`tests/programs/`](../tests/programs/).
[`crates/fidryn-verify/tests/domains.rs`](../crates/fidryn-verify/tests/domains.rs)
runs them as independently named modules. Do not rewrite compiled Core
to green fingerprint tests. Patching a `CoreModule` so a digest, plan,
or outcome matches is not a passing test.

## Commits

Use Conventional Commits (`feat(syntax): ...`, `fix(eval): ...`,
`test(cli): ...`, `docs: ...`). Prefer atomic commits in pipeline order,
then tests and docs. Do not dump unrelated crates into one commit.

## Grammar

The closed v0.1 grammar is [`grammar.ebnf`](../grammar.ebnf) at the
repository root. Unknown body fields are parse errors, not extension
points.

## Implementer documents

These files are crate contracts and evidence. They are not user
tutorials.

- [ARCHITECTURE.md](ARCHITECTURE.md) — Frozen crate graph, public APIs, and the pipeline of total functions over trees.
- [implementation-status.md](implementation-status.md) — Evidence-backed capability matrix. A `.fr` file is not an executable specification.
- [OBLIGATIONS.md](OBLIGATIONS.md) — Review obligation matrix. Green workspace tests are regressions, not language closure.
- [WORKSTREAM-CONTRACT.md](WORKSTREAM-CONTRACT.md) — Shared types and acceptance for covering certificates, `seq`/`require`, tax, and trust profiles.
- [INTEGRATION-CONTRACT.md](INTEGRATION-CONTRACT.md) — Wire existing machinery into ordinary compile, eval, and CLI paths. Do not reimplement helpers.

Mill HTTP routes are documented in [mill.md](mill.md), not in
ARCHITECTURE.md.

## Not legal advice

This interpreter, its tests, and its example modules are not legal
advice.
