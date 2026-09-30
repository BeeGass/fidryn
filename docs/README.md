# Fidryn documentation

Fidryn (pronounced FID-rin) is a programming language for legal
instruments: precise where law is mechanical, explicit where judgment
enters, and incapable of hiding authority, discretion, or ambiguity
inside a Boolean.

This directory is the human documentation for the v0.1 reference
interpreter. Start at the [product README](../README.md) to clone and
build, then read [Getting started](getting-started.md). The modules,
records, and examples in this repository are research fixtures. They
are not legal advice, not operative instruments, and not a complete
statement of any jurisdiction's law.

## For users

| Guide | What it is for |
| --- | --- |
| [Getting started](getting-started.md) | First hour: build, `check`, `run` |
| [Language](language.md) | `.fr` modules, queries, rules, duties, verify |
| [CLI](cli.md) | Every `fidryn` subcommand and flag |
| [Mill](mill.md) | Localhost UI on 127.0.0.1 |
| [Cases and time](cases-and-time.md) | Case records, valid time, known-at |
| [Outcomes](outcomes.md) | Determinate, Suspended, Contingent, and the rest |
| [Examples](examples.md) | Trust, tax, federal slices, fifty states |

The closed grammar is [`grammar.ebnf`](../grammar.ebnf). Wire formats
are [`schemas/case-record-v0.1.json`](../schemas/case-record-v0.1.json),
[`schemas/outcome-v0.1.json`](../schemas/outcome-v0.1.json), and
[`schemas/evaluation-report-v0.1.json`](../schemas/evaluation-report-v0.1.json)
(CLI `run` / `explore` and mill eval responses nest the outcome as
`outcomeDocument`).

## For implementers

| Document | What it is |
| --- | --- |
| [Contributing](contributing.md) | Toolchain, tests, commits |
| [Architecture](ARCHITECTURE.md) | Pipeline and crate contract |
| [Implementation status](implementation-status.md) | Evidence-backed capability matrix |
| [Obligations](OBLIGATIONS.md) | Review obligations; green tests are regressions |
| [Integration contract](INTEGRATION-CONTRACT.md) | How existing machinery must be wired |
| [Workstream contract](WORKSTREAM-CONTRACT.md) | Covering, require, tax, trust profiles |
| [Review-fix contract](REVIEW-FIX-CONTRACT.md) | Historical pass-3 repair brief |
| [Full implementation](FULL-IMPLEMENTATION.md) | Older deferred-work list; status is the matrix |

Mill HTTP routes live in [mill.md](mill.md), not in Architecture.
