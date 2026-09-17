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

## Source checking and trust

`check`, `run`, `explore`, `verify`, `diff` (when a snapshot is `.fr`), and `render` compile through `Driver::check_path` with source-manifest authentication. Hex digests authenticate artifact bytes next to the module. The digest string `"fixture"` is a test trust profile, not byte verification.

## Subcommands

| Command | Purpose |
| --- | --- |
| `fmt PATH.fr` | Format source to stdout |
| `check PATH.fr` | Elaborate and type-check |
| `run PATH.fr --query NAME --case RECORD.json --valid-at TIME --known-at TIME` | Evaluate a query |
| `explore PATH.fr ... [--bounds BOUNDS.json]` | Search declared admissibleCompletions only |
| `explain TRACE` | Render a trace (`text` / `json` / `dot`) |
| `verify PATH.fr --property NAME` | Check a declared verify property |
| `diff OLD NEW --query NAME` | Compare snapshots |
| `render PATH.fr --template T` | Interpolate a template |
| `file PACKET.json` | Filing adapter (dry-run default) |
| `ui [--port N] [--no-open]` | Localhost mill on 127.0.0.1 |

### `run` flags

- `--valid-at` / `--known-at` — RFC 3339 valid time and record time
- `--arg KEY=VALUE` — sets `case.facts[KEY]`
- `--scenario` — apply `case.assumptions` overlay

Stdout is `fidryn.evaluation-report/v0.1` with nested `outcomeDocument` (`fidryn.outcome/v0.1`).

### `ui`

```
fidryn ui --port 8751 --no-open
```

Binds `127.0.0.1` only. Live filing is disabled.

## See also

- [Getting started](getting-started.md)
- [Outcomes](outcomes.md)
- [Cases and time](cases-and-time.md)
- [Mill](mill.md)

Full flag tables: repository `docs/cli.md`.
