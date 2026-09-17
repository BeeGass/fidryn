---
title: "Outcomes"
description: "Determinate, Suspended, Contingent, and the rest of the Fidryn outcome envelope."
url: "https://fidryn.onlygass.dev/docs/outcomes"
markdown: "https://fidryn.onlygass.dev/docs/outcomes.md"
author: "Bryan Gass"
---

> Canonical HTML: https://fidryn.onlygass.dev/docs/outcomes
> This markdown mirror is for agents and plain-text readers.

# Outcomes

This guide explains the six honest results a Fidryn query can return. Fidryn is a research interpreter. An outcome is not a court order, not legal advice, and not a filing.

The six-kind result is a `fidryn.outcome/v0.1` object (`schemas/outcome-v0.1.json`). Field names are camelCase. Trust, coverage, execution mode, and assumptions that cannot fit that schema live on the evaluation-report envelope; they are not extra keys on the outcome object.

## Envelope

| Field | Meaning |
| --- | --- |
| `schema` | `fidryn.outcome/v0.1` |
| `module` | Compiled module identity, `Name@version` |
| `sourceSnapshot` | Content identity of the compiled source snapshot |
| `query` | Query name you asked |
| `asOf.validTime` | `--valid-at` |
| `asOf.recordTime` | `--known-at` |
| `modelBoundary.outsideScope` | Union of module and case outside-scope |
| `modelBoundary.admissibleCompletions` | Declared completion space when nonempty |
| `outcome` | One of the six kinds below |

`modelBoundary` is part of the answer. Read it before treating a determinate value as "the" result.

## Outcome kinds

`outcome` is tagged by `kind`. Every kind includes `trace` (32-char hex).

1. **Determinate** — one answer invariant across every still-admissible resolution, or a competent determination already on the case. Requires a nonempty exhaustive completion set.
2. **Contingent** — still-admissible worlds disagree.
3. **Suspended** — missing interpretation, evidence, judgment, or choice; `run` never invents the completion.
4. **NormConflict** — staged effects disagree and no unique doctrine applies.
5. **OutsideCompetence** — recorded selection outside the declared model.
6. **Inconsistent** — admitted model cannot be satisfied (e.g. empty domain).

## Rules

1. Determinate needs nonempty exhaustive agreement or a competent determination.
2. Open branches without a covering certificate stay Suspended.
3. A claims digest is not covering proof.
4. `explore` searches declared completions; `run` never fills them.
5. `modelBoundary` is on the envelope — read it.
6. Prop is not Bool.

## Reading a result

1. Confirm schema `fidryn.outcome/v0.1` (or nested under evaluation-report).
2. Read `modelBoundary` and `asOf` before `outcome.kind`.
3. Branch on `kind`. Only `determinate` has `value` as *the* answer.
4. If `ignoredOpenIssues` is nonempty, demand a covering certificate.
5. For `suspended` / `contingent`, work from `requests` / `pivots`.

See also: [CLI](cli.md), [Cases and time](cases-and-time.md). Full field tables: repository `docs/outcomes.md`.
