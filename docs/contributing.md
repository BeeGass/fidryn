# Contributing to Fidryn

This file is for people building the 0.2 reference interpreter. It is
not a language tutorial and not legal advice. Product usage lives in
the [README](../README.md) and the [documentation hub](README.md).

Fidryn (FID-rin) is a research fixture. Modules under `examples/` and
records under `tests/` exercise the interpreter. They are not operative
instruments and not a complete statement of any jurisdiction's law.

Fidryn is released under the MIT License. Licensing and contribution
rights are described in [Licensing](../LICENSING.md). Contributions are
accepted under the same license; no separate agreement is required.

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
cargo fmt --all
cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings
cargo test --workspace --all-features --locked --offline
```

`cargo xtask ci --fmt` runs formatting, the complete Rust tests and Clippy,
Rustdoc with warnings denied, executable schema probes, Python lint/format/types
and probe controls, required JavaScript tests, generated-site consistency, the
example status and documentation checks described below, and benchmark
compilation. Node and [uv](https://docs.astral.sh/uv/) are required for
qualification. `uv run --locked` uses `.python-version` and `uv.lock`.

GitHub runs this gate for pushes and PRs to `dev` and `main`, plus separate
Linux/macOS/Windows tests, dependency auditing, secret scanning, workflow linting
and CodeQL. Development and native branch protections are described in
[Development and promotion](development.md). Generated environments and build
outputs are not committed.

Do not weaken no-false-determinacy to make a test pass.

## Website

The public site in [`site/`](../site/) is generated from these guides
by the workspace task runner. After editing a guide in `docs/`, run:

```
cargo xtask site
```

and commit the markdown together with the regenerated files under
`site/`. `cargo xtask site --check` fails when the committed site is
stale, and `cargo xtask ci` runs that check. The landing page's
specimen is evaluated by the real interpreter at build time, so a
change in semantics also makes the site stale.

The stylesheet and script in `site/assets/` are written by hand. The
mill, whose page lives in [`web/`](../web/), embeds the stylesheet
`fidryn.css` and has its own script, `web/mill.js`. `cargo xtask ci` requires Node to run the JavaScript unit tests in
`web/tests/`; Node is not needed to build or serve the interpreter.

## Example status and documentation checks

`cargo xtask examples` runs `fidryn check` on every `.fr` file under
`examples/`, `packages/`, `prelude/` and `tests/programs/`, and fails when
the recorded result is out of date. The record is `examples/status.json`
and `examples/STATUS.md`; the status sentence between the
`examples-status` markers in `README.md`, `examples/README.md`,
`examples/states/README.md` and `docs/examples.md`; and the ` (draft)`
mark after each path of a failing file that `README.md`,
`examples/README.md`, `examples/states/README.md` or a Markdown file
directly under `docs/` mentions outside a code block. Do not edit any of these by
hand. `cargo xtask examples --update` rewrites the status files, the
status sentences and the draft marks. Run `cargo xtask site` after it,
because the site is generated from the guides it rewrites.

`cargo xtask docs` reads the Markdown in `README.md`, `LICENSING.md`,
`CHANGELOG.md`, `SECURITY.md`, `docs/`, `examples/`, `packages/`,
`conformance/` and `site/README.md`. It checks that every relative link
resolves, that every untagged or `fidryn`-tagged code block that starts
with `module` passes the checker, and that the `fidryn` commands in
blocks tagged `sh` behave as the tag says. `--skip-commands` leaves out
the commands.

A code block that shows a `fidryn` command takes one of these tags:

- `sh`: the check runs each `fidryn` command in the block from the
  repository root and expects exit status 0. A command is a line, with
  lines ending in `\` joined, that starts with `fidryn` or with
  `cargo run -p fidryn-cli --`; the check runs the built `fidryn` binary
  with the words after that prefix. A line in an `sh` block that is not a
  `fidryn` command, such as `cargo test --workspace --locked`, is not run.
  A command with `ui` as one of its words is taken to start the mill, so
  it is not run either. The check does not use a shell, so a command
  must not use `>`, `>>`, `<`, `|`, `&&`, `||` or `;`. A command that has
  not exited after 60 seconds is stopped and reported.
- `sh exit=N`: the same, but each command must exit with status `N`. A
  tag whose first word is `sh` and which is neither `sh` nor `sh exit=N`
  with `N` an integer, such as `sh exit 1`, is reported.
- `text`: the block is shown and not run. Use it for placeholders such as
  `PATH`, for output redirection, and for commands that cannot run
  unattended.

A block with any other tag, such as `shell`, or with none, must not show
a `fidryn` command. Such a tag is not itself reported, but a block under
it that shows a `fidryn` command is reported as an untagged block would
be.

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
fidryn-provision       law-module model and the rules over it (no workspace crate)
fidryn-ingest-uscode   empty (provision)
fidryn-cli             fidryn binary and localhost mill
```

## Tests

Tests live next to the crates they exercise (`#[cfg(test)]` in `src`,
and `crates/<name>/tests/` when a crate has an integration kit).

The adversarial review kit is
[`tests/integration/adversarial_regressions.rs`](../tests/integration/adversarial_regressions.rs).
Those tests are regressions, not closure of the language.

Independent programs live under [`tests/programs/`](../tests/programs/).
[`crates/fidryn-verify/tests/domains.rs`](../crates/fidryn-verify/tests/domains.rs)
runs them as independently named modules. Do not rewrite compiled Core
to green fingerprint tests. Patching a `CoreModule` so a digest, plan,
or outcome matches is not a passing test.

## Commits

Write each commit subject as an imperative sentence with no type prefix,
for example `Require code-scanning findings to pass before promotion`.
Recent history follows this; older commits used Conventional Commits
prefixes such as `feat(syntax):`, which are no longer used. Prefer atomic
commits in pipeline order, then tests and docs. Do not dump unrelated
crates into one commit.

## Grammar

The grammar is [`grammar.ebnf`](../grammar.ebnf) at the repository root.
It is closed, but the 0.2 parser enforces closed field sets only in
`source` blocks, rule bodies and judgment request blocks. In most other
declarations an unknown field is skipped without a diagnostic; see
[implementation status](implementation-status.md).

## Implementer documents

These files are crate contracts and evidence. They are not user
tutorials.

- [ARCHITECTURE.md](ARCHITECTURE.md) — Frozen crate graph, public APIs, and the pipeline of total functions over trees.
- [implementation-status.md](implementation-status.md) — What the 0.2 interpreter does and does not do, each statement backed by a test or a code location. A `.fr` file is not an executable specification.

The internal working ledgers are indexed in
[internal/README.md](internal/README.md). They are not reader
documentation, and parts of them describe earlier states of the code.

Mill HTTP routes are documented in [mill.md](mill.md), not in
ARCHITECTURE.md.

## Not legal advice

This interpreter, its tests, and its example modules are not legal
advice.
