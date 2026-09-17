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
are [`schemas/case-record-v0.1.json`](../schemas/case-record-v0.1.json)
and [`schemas/outcome-v0.1.json`](../schemas/outcome-v0.1.json).

