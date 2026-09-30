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

This directory is the human documentation for the v0.1 reference
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

The closed grammar is [`grammar.ebnf`](https://github.com/BeeGass/fidryn/blob/main/grammar.ebnf). Wire formats
are [`schemas/case-record-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/case-record-v0.1.json),
[`schemas/outcome-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/outcome-v0.1.json), and
[`schemas/evaluation-report-v0.1.json`](https://github.com/BeeGass/fidryn/blob/main/schemas/evaluation-report-v0.1.json)
(CLI `run` / `explore` and mill eval responses nest the outcome as
`outcomeDocument`).

## For implementers

| Document | What it is |
| --- | --- |
| [Contributing](https://fidryn.onlygass.dev/docs/contributing.md) | Toolchain, tests, commits |
| [Architecture](https://github.com/BeeGass/fidryn/blob/main/docs/ARCHITECTURE.md) | Pipeline and crate contract |
| [Implementation status](https://github.com/BeeGass/fidryn/blob/main/docs/implementation-status.md) | Evidence-backed capability matrix |
| [Obligations](https://github.com/BeeGass/fidryn/blob/main/docs/OBLIGATIONS.md) | Review obligations; green tests are regressions |
| [Integration contract](https://github.com/BeeGass/fidryn/blob/main/docs/INTEGRATION-CONTRACT.md) | How existing machinery must be wired |
| [Workstream contract](https://github.com/BeeGass/fidryn/blob/main/docs/WORKSTREAM-CONTRACT.md) | Covering, require, tax, trust profiles |
| [Review-fix contract](https://github.com/BeeGass/fidryn/blob/main/docs/REVIEW-FIX-CONTRACT.md) | Historical pass-3 repair brief |
| [Full implementation](https://github.com/BeeGass/fidryn/blob/main/docs/FULL-IMPLEMENTATION.md) | Older deferred-work list; status is the matrix |

Mill HTTP routes live in [mill.md](https://fidryn.onlygass.dev/docs/mill.md), not in Architecture.
