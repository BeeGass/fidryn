# Fidryn documentation

Fidryn (pronounced FID-rin) is a programming language for legal
instruments: precise where law is mechanical, explicit where judgment
enters, and incapable of hiding authority, discretion, or ambiguity
inside a Boolean.

This directory is the human documentation for the 0.3 reference
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
| [Law modules](law-modules.md) | Law modules of edition 2, which hold provisions of law as data: the two editions, addresses and file paths, normalization, provisions, notes and layers, hashes, the syntax, what the checker verifies in one module, workspaces with their manifest and lock, how a workspace is checked and the outcome of each reference, the cache, definitions, curated modules with their names, dependencies and bindings, change sets, the ingester for the United States Code, the coverage ledger and the sample of the Code, and the limits |
| [CLI](cli.md) | Every `fidryn` subcommand and flag |
| [Mill](mill.md) | Localhost UI on 127.0.0.1 |
| [Cases and time](cases-and-time.md) | Case records, valid time, known-at |
| [Outcomes](outcomes.md) | Determinate, Suspended, Contingent, and the rest |
| [Examples](examples.md) | Trust, tax, federal slices, fifty states |
| [Coverage](coverage.md) | What the sample of the United States Code in the repository holds, in counts |

The closed grammar is [`grammar.ebnf`](../grammar.ebnf). Wire formats
are [`schemas/case-record-v0.1.json`](../schemas/case-record-v0.1.json),
[`schemas/outcome-v0.1.json`](../schemas/outcome-v0.1.json), and
[`schemas/evaluation-report-v0.2.json`](../schemas/evaluation-report-v0.2.json)
(CLI `run` / `explore` and mill eval responses nest the outcome as
`outcomeDocument`). The files of a workspace of law modules follow
[`schemas/workspace-v0.1.json`](../schemas/workspace-v0.1.json),
[`schemas/law-lock-v0.1.json`](../schemas/law-lock-v0.1.json) and
[`schemas/retrieval-v0.1.json`](../schemas/retrieval-v0.1.json), the
coverage ledger of a generated workspace follows
[`schemas/coverage-v0.1.json`](../schemas/coverage-v0.1.json), the
report of the check of a workspace follows
[`schemas/check-report-v0.1.json`](../schemas/check-report-v0.1.json), the
text records that `fidryn text --json` prints follow
[`schemas/text-record-v0.1.json`](../schemas/text-record-v0.1.json), and
the change set that `fidryn changes --json` prints follows
[`schemas/change-set-v0.1.json`](../schemas/change-set-v0.1.json).

## For implementers

| Document | What it is |
| --- | --- |
| [Contributing](contributing.md) | Toolchain, tests, commits |
| [Development and promotion](development.md) | Native GitHub protections, required checks, reproducible reports and review bundles |
| [Licensing](../LICENSING.md) | MIT terms and third-party notices |
| [Releases](releases.md) | Version/changelog checks, native packages and GitHub release publication |
| [Architecture](ARCHITECTURE.md) | Pipeline and crate contract |
| [Implementation status](implementation-status.md) | What the 0.3 interpreter does and does not do, each statement backed by a test or a code location |
| [Internal ledgers](internal/README.md) | Working ledgers from reviewing and repairing the interpreter; parts describe earlier states of the code |

Mill HTTP routes live in [mill.md](mill.md), not in Architecture.
