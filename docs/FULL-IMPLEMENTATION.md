# Fidryn: no deferred features

The originating essay deferred several subsystems. This tree implements them.
Nothing in this list is a stub that only returns "deferred".

## Required subsystems

1. **Conflict oracle** (`ResolveNormConflict`)
   - Build an argument graph from staged effects and named doctrines.
   - Apply finite, stratified doctrines; compatible single doctrine defeats;
     incompatible multiple doctrines or none applicable stay `NormConflict`
     with the graph, not a silent pick.
   - CaseFile may resume from a recorded `ConflictResolution` in the case
     record (`decisions["conflict:<id>"]` or `facts`).
   - Explore branches over admissible doctrines.

2. **SelectApplicableLaw**
   - Candidates are `SourceVersion` ids from the manifest hierarchy.
   - Ranking: binding > controlling > persuasive > explanatory, then
     later-in-time, then specific-over-general when a doctrine says so.
   - If ranking is unique, resume with that set. If not, suspend
     `NeedApplicableLaw` or return `Contingent` under Explore.

3. **Source hierarchy**
   - Manifest artifacts have `weight`: binding | controlling | persuasive | explanatory.
   - Imports resolve against the snapshot; missing digest is E200.

4. **User-defined effects**
   - Surface: `effect Name { request(params) -> Type }`
   - Core stores `CoreEffectDecl`. Calls become `NeedCustom`.
   - Handlers in `.fidryn` or Rust `Handler::handle_custom`.

5. **Recursion**
   - `calc` remains total and non-recursive.
   - `fn` may recurse with an explicit `fuel = N` or a finite-domain
     structural argument checked by `fidryn-check` (E530 if unguarded).
   - Evaluator decrements fuel; exhaustion is `Inconsistent` not a hang.

6. **Quantification**
   - `for_all x in Domain: phi` and `exists x in Domain: phi`.
   - External domains require a closure record or `W610`.
   - Implemented in the evaluator over finite sets.

7. **Court procedure**
   - Prelude `prelude/procedure.fidryn`: Proceeding, Pleading, Burden,
     Notice, JudgmentEntry, Appeal, Stay, Remand, Finality.
   - Example module that files a complaint, answers, and records a judgment.

8. **Case-law argumentation**
   - `ArgumentGraph` nodes: Citation, Holding, Distinguish, Overrule, Follow.
   - Conflict oracle consumes this graph.

9. **Filing adapters** (`fidryn-adapt`)
   - `FilingAdapter` trait: `submit(packet) -> AdapterResult`.
   - Dry-run is default. Live HTTP only if `--live` AND
     `FIDRYN_ALLOW_LIVE_FILING=1`.
   - Massachusetts Corporations Division adapter with configurable endpoint.
   - Tests use a mock listener. Physical transmission still
     `does_not_establish Filed`.

10. **Tax engine** (`examples/tax` + calc)
    - Dated federal BOI exemption module (effective 2026-08-14).
    - Closed-form bracket calc: `calc tax_on(amount: Money<USD>) -> Money<USD>`.
    - Open-textured credits remain `Determine`.

11. **Solver** (`fidryn-solve`)
    - Bounded DPLL over declared finite domains for Explore/Skeptical.
    - Used by `fidryn-verify` instead of only nested for-loops when the
      domain product is large; still exhaustive on the declared space.

12. **Render / controlled English** (`fidryn-render`)
    - `fidryn render PATH --template T` interpolates Core values into a
      constrained template (`{{path}}`, `{{#each}}`).
    - Templates cannot introduce legal content absent from Core; missing
      keys fail closed. Not claimed semantics-preserving unless the
      template is in `templates/certified/`.

13. **Explain and diff**
    - `explain` prints the trace DAG (text/json/dot) from the last run
      artifact or an explicit trace file.
    - `diff OLD NEW --query Q` compares Core snapshots and outcomes.

14. **Mill UI**
    - `fidryn ui [--port N] [--no-open]` on 127.0.0.1 only.
    - Check, run, explore, render. No live filing from the UI.

15. **General worklist evaluator**
    - Apply constitutive/derive/prescriptive rules from Core, not only
      fixture-specialized query plans. Existing acceptance tests must
      still pass.

## Crate ownership for parallel work

- Agent conflict: `fidryn-eval/src/conflict.rs`, handlers conflict/law, core argument types already in outcome.rs
- Agent lang: grammar.ebnf, fidryn-syntax, fidryn-hir, fidryn-check (effect/fn/quantifier)
- Agent procedure: prelude/procedure.fidryn, prelude/argument.fidryn, examples/procedure, examples/tax
- Agent adapt: crates/fidryn-adapt, crates/fidryn-solve
- Agent surface: crates/fidryn-render, CLI render/diff/ui, web/

Do not weaken no-false-determinacy. Rust 1.98. `cargo fmt`, `clippy -D warnings`, `cargo test --workspace`.
