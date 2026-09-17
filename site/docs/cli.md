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

See the HTML guide at /docs/cli for the full rendered version. This mirror is generated from docs/cli.md.

The binary name is `fidryn`. It is the v0.1 reference interpreter: parse a `.fr` module, check it, and evaluate a named query against a case record. It is not legal advice and does not file anything unless you opt into live HTTP as described under `file`.

```
cargo run -p fidryn-cli -- <subcommand> ...
```

After install: `fidryn <subcommand> ...`.

## Install

```
cargo install --path crates/fidryn-cli
```

Workspace `rust-version` is 1.98.

## Exit status

Success is exit 0. Failures are exit 1. A legal Outcome is not a process failure — `run` / `explore` exit 0 even for suspended/contingent outcomes.

## `check`

```
fidryn check PATH.fr
```

## `run`

```
fidryn run PATH.fr --query NAME --case RECORD.json --valid-at TIME --known-at TIME [--arg KEY=VALUE] [--scenario]
```

`--valid-at` is valid time; `--known-at` is record time. Both are RFC 3339. `--scenario` applies `case.assumptions` as an overlay.

Stdout is `fidryn.evaluation-report/v0.1` with nested `outcomeDocument` (`fidryn.outcome/v0.1`).

## `explore`

```
fidryn explore PATH.fr --query NAME --case RECORD.json --valid-at TIME --known-at TIME [--bounds BOUNDS.json]
```

Searches declared `admissibleCompletions` only. Does not invent completions.

## `explain` / `verify` / `diff` / `render` / `file` / `ui`

See the HTML page for full flag tables. Local mill: `fidryn ui --port 8751 --no-open` (127.0.0.1 only).

## See also

- [Getting started](getting-started.md)
- [Outcomes](outcomes.md)
- [Cases and time](cases-and-time.md)
