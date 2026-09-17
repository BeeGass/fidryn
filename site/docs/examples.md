---
title: "Examples"
description: "Trust, tax, federal slices, and the fifty-state corpus map for Fidryn."
url: "https://fidryn.onlygass.dev/docs/examples"
markdown: "https://fidryn.onlygass.dev/docs/examples.md"
author: "Bryan Gass"
---

> Canonical HTML: https://fidryn.onlygass.dev/docs/examples
> This markdown mirror is for agents and plain-text readers.

# Examples

Fidryn ships fixture modules under `examples/`. They encode bounded
high-impact slices. They are not legal advice, not operative instruments,
and not a complete statement of any jurisdiction's law.

## Catalog

| Area | Path | What it exercises |
| --- | --- | --- |
| Trust | `examples/trust/` | Occupancy, incapacity, successor eligibility |
| Tax | `examples/tax/` | Bracket math with `calc` / `if` |
| Federal slices | `examples/federal/` | Bounded USC-shaped fixtures |
| States | `examples/states/` | Fifty-state corpus map |

## Trust fixture

```
cargo run -p fidryn-cli -- check examples/trust/bryan-revocable-trust.fr
```

Cases live under `examples/trust/cases/`. Open eligibility suspends or
explores; a recorded court selection of I2 answers Bob.

## Running a fixture

```
cargo run -p fidryn-cli -- run PATH.fr \
  --query NAME \
  --case CASE.json \
  --valid-at 2026-09-17T12:00:00Z \
  --known-at 2026-09-17T12:00:00Z
```

See [Getting started](getting-started.md), [Outcomes](outcomes.md), and
[Cases and time](cases-and-time.md). The full catalog with per-fixture
notes is in repository `docs/examples.md`.
