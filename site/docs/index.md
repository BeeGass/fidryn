---
title: "Documentation"
description: "Fidryn documentation hub — learner guides for the programming language for legal instruments."
url: "https://fidryn.onlygass.dev/docs/"
markdown: "https://fidryn.onlygass.dev/docs/index.md"
author: "Bryan Gass"
---

> Canonical HTML: https://fidryn.onlygass.dev/docs/
> This markdown mirror is for agents and plain-text readers.

# Fidryn documentation

Fidryn (pronounced FID-rin) is a programming language for legal
instruments: precise where law is mechanical, explicit where judgment
enters, and incapable of hiding authority, discretion, or ambiguity
inside a Boolean.

This directory is the human documentation for the 0.3 reference
interpreter. Start at the [product README](https://github.com/BeeGass/fidryn/blob/main/README.md) to clone and
build, then read [Getting started](https://fidryn.onlygass.dev/docs/getting-started.md). The modules,
records, and examples in this repository are research fixtures. They
are not legal advice, not operative instruments, and not a complete
statement of any jurisdiction's law.

## For users

| Guide | What it is for |
| --- | --- |
| [Getting started](https://fidryn.onlygass.dev/docs/getting-started.md) | First hour: build, `check`, `run` |
| [Language](https://fidryn.onlygass.dev/docs/language.md) | `.fr` modules, queries, rules, duties, verify |
| [CLI](https://fidryn.onlygass.dev/docs/cli.md) | Every `fidryn` subcommand and flag |
| [Mill](https://fidryn.onlygass.dev/docs/mill.md) | Localhost UI on 127.0.0.1 |
| [Cases and time](https://fidryn.onlygass.dev/docs/cases-and-time.md) | Case records, valid time, known-at |
| [Outcomes](https://fidryn.onlygass.dev/docs/outcomes.md) | Determinate, Suspended, Contingent, and the rest |
| [Examples](https://fidryn.onlygass.dev/docs/examples.md) | Trust, tax, federal slices, fifty states |
| [Coverage](https://fidryn.onlygass.dev/docs/coverage.md) | What the sample of the United States Code in the repository holds, in counts |

The closed grammar is [`grammar.ebnf`](https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf). Wire formats
are [`schemas/case-record-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/case-record-v0.1.json),
[`schemas/outcome-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/outcome-v0.1.json), and
[`schemas/evaluation-report-v0.2.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/evaluation-report-v0.2.json)
(CLI `run` / `explore` and mill eval responses nest the outcome as
`outcomeDocument`). The files of a workspace of law modules follow
[`schemas/workspace-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/workspace-v0.1.json),
[`schemas/law-lock-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/law-lock-v0.1.json) and
[`schemas/retrieval-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/retrieval-v0.1.json), the
coverage ledger of a generated workspace follows
[`schemas/coverage-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/coverage-v0.1.json), and the
report of the check of a workspace follows
[`schemas/check-report-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/check-report-v0.1.json).

## For implementers

| Document | What it is |
| --- | --- |
| [Contributing](https://fidryn.onlygass.dev/docs/contributing.md) | Toolchain, tests, commits |
| [Development and promotion](https://github.com/BeeGass/fidryn/blob/main/docs/development.md) | Native GitHub protections, required checks, reproducible reports and review bundles |
| [Licensing](https://github.com/BeeGass/fidryn/blob/main/LICENSING.md) | MIT terms and third-party notices |
| [Releases](https://github.com/BeeGass/fidryn/blob/main/docs/releases.md) | Version/changelog checks, native packages and GitHub release publication |
| [Architecture](https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md) | Pipeline and crate contract |
| [Law modules](https://github.com/BeeGass/fidryn/blob/main/docs/law-modules.md) | Addresses of modules, provisions and notes, the file path of a module, the normalization of its text, the model of provisions, notes and modules, their hashes, the syntax of edition 2, what the checker verifies in one module, workspaces: their manifest, their lock and what the checker verifies in one, how a workspace is checked, references and their outcomes, the ingester for the United States Code, the definitions it extracts, the coverage ledger of a generated workspace, and the sample of the Code the repository holds and its check |
| [Implementation status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md) | What the 0.3 interpreter does and does not do, each statement backed by a test or a code location |
| [Internal ledgers](https://github.com/BeeGass/fidryn/blob/main/docs/internal/README.md) | Working ledgers from reviewing and repairing the interpreter; parts describe earlier states of the code |

Mill HTTP routes live in [mill.md](https://fidryn.onlygass.dev/docs/mill.md), not in Architecture.
