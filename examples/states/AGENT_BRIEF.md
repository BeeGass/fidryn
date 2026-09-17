# State-module brief

You are writing **full, true, complete** Fidryn modules. Read
`examples/states/florida/homestead.fr` as the quality bar (length,
structure, scenarios, honest judgment).

## Must

1. Look up **current** citation, day counts, dollar caps, acreage, and
   exceptions with a web search. Do not invent numbers.
2. One directory per state under `examples/states/<slug>/`.
3. Files: `<topic>.fr` plus `sources/<citation-slug>.txt` fixture excerpt
   ending "Not an official edition."
4. **No JSON case files. No JSON manifests.** Put facts in `scenario`
   blocks inside the `.fr` file. Omit `source_manifest`.
5. Header: `module State.Area.Topic version "0.1.0"`, `jurisdiction`,
   `source_snapshot "2026-09-17-..."`, `effective_at 2026-09-17`,
   `recorded_at` with the state's offset, `outside_scope { ... }`.
6. At least two `source` blocks if a constitution and a statute both
   matter; otherwise one statute/regulation with `digest fixture` and
   `effective [date, +inf)`.
7. Entities, propositions, at least one `observation` or `legal_act` or
   `duty`, at least two `calc` or `automatic` queries for mechanical
   numbers, at least one `! {Determine}` or `! {Observe}` query, at
   least two `scenario` blocks, one `verify` that automatic queries have
   empty effect rows and judgment queries are never automatic.
8. Target **120–250 lines**. Thin 40-line stubs fail the assignment.
9. Identifiers: ASCII letters, digits, underscore. No hyphens in idents.
10. Money: `USD(1000.00)`. Durations: `21 counted_days` or `5 working_days`.
11. Allowed introducers only: module, jurisdiction, source_snapshot,
    effective_at, recorded_at, outside_scope, source, import, entity,
    type, record_type, evidence_type, office, proposition, observation,
    nomination, fn, calc, rule, power, duty, judgment, decision,
    legal_act, clause, interpretation_family, conflict_doctrine, query,
    scenario, verify, effect.
12. Not legal advice. Name the incompleteness in `outside_scope`.
13. After writing, run:
    `PATH="/opt/homebrew/bin:$HOME/.cargo/bin:$PATH" cargo run -p fidryn-cli --quiet -- check examples/states/<slug>/<file>.fr`
    Fix parse errors before finishing.

## Timezone offsets

Atlantic-ish East: `-04:00`. Central: `-05:00`. Mountain: `-06:00`
(AZ: `-07:00`, no DST). Pacific: `-07:00`. Alaska: `-08:00`. Hawaii: `-10:00`.
