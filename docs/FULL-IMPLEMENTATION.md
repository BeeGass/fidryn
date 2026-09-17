# Fidryn: required subsystems

The originating essay deferred several subsystems. They remain **language
requirements** for v0.1. Nothing in this list is an accepted deferral.

A `.fr` module is not an executable specification. A feature is
implemented only when parse, resolution, checking, evaluation, uncertainty,
provenance, and tests agree. Evidence is
[`implementation-status.md`](implementation-status.md). This review-fix
pass closes the holes in [`REVIEW-FIX-CONTRACT.md`](REVIEW-FIX-CONTRACT.md)
(term evaluation, real plans, matching, aggregation, certificates, manifests).
It does **not** make general rule evaluation, fuel-SCC termination, or
finite quantification complete.

## Governing rule

A legal computation may return one determinate result only when that
result is invariant across every still-admissible resolution of the
unresolved issues, or when a competent authority has already made a
determination that is operative in the relevant context.

`run` never chooses a completion. Uncertified open issues are
`Suspended`. An empty exhaustive completion set is `Inconsistent` or
`Suspended`, never a vacuous `Determinate`.

## Required subsystems

1. **Conflict oracle** (`ResolveNormConflict`)
   - Build an argument graph from staged effects and named doctrines.
   - Apply finite, stratified doctrines; compatible single doctrine defeats;
     incompatible multiple doctrines or none applicable stay `NormConflict`
     with the graph, not a silent pick.
   - CaseFile may resume from a recorded `ConflictResolution` in the case
     record (`decisions["conflict:<id>"]` or `facts`).
   - Explore branches over admissible doctrines.
   - **Evidence (Partial):** unique named doctrine resumes; ties suspend.
     Matching is label-level, not a full argument-graph reasoner. See matrix.

2. **SelectApplicableLaw**
   - Candidates are `SourceVersion` ids from the manifest hierarchy.
   - Ranking: binding > controlling > persuasive > explanatory, then
     later-in-time, then specific-over-general when a doctrine says so.
   - If ranking is unique, resume with that set. If not, suspend
     `NeedApplicableLaw` or return `Contingent` under Explore.
   - **Evidence (Partial):** weight then later-in-time; list order is never
     a tie-break. Specific-over-general is not a general calculus this pass.

3. **Source hierarchy**
   - Manifest artifacts have `weight`: binding | controlling | persuasive | explanatory.
   - Imports resolve against the snapshot; missing digest is E200.
   - **This pass:** CLI loads the path declared by `source_manifest`, else
     `sources/manifest.json`. Missing/malformed is a diagnostic, not
     `SourceManifest::default()` when the module declared a manifest.
     `digest fixture` is a fixture profile, not authentication.
     `check_imports` matches the exact artifact path (or import name) and
     that artifact’s digest.

4. **User-defined effects**
   - Surface: `effect Name { request(params) -> Type }`
   - Core stores `CoreEffectDecl`. Calls become `NeedCustom`.
   - Handlers in `.fr` or Rust `Handler::handle_custom`.

5. **Recursion**
   - `calc` remains total and non-recursive.
   - `fn` may recurse with an explicit `fuel = N` or a finite-domain
     structural argument checked by `fidryn-check` (E530 if unguarded).
   - Evaluator **decrements** fuel; exhaustion is `EngineError::FuelExhausted`,
     never a hang and never `Outcome::Inconsistent`.
   - **Evidence (Partial):** `fuel = N` parses; E530 is ident-in-body, not
     an SCC. Decrement of a real call stack is only as complete as body
     execution. Full SCC termination is not this pass.

6. **Quantification**
   - `for_all x in Domain: phi` and `exists x in Domain: phi`.
   - External domains require a closure record or `W610`.
   - Required: evaluator over finite closed sets.
   - **Evidence (Partial / executed No):** syntax and W610 exist. The
     evaluator does not yet quantify over finite domains. That is still
     required; it is not this pass. See matrix row “Finite quantification”.

7. **Court procedure**
   - Prelude `prelude/procedure.fr`: Proceeding, Pleading, Burden,
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
   - States: `DryRun` / `Submitted` / `Rejected`.

10. **Tax engine** (`examples/tax` + calc)
    - Dated federal BOI exemption module (effective 2026-08-14).
    - Closed-form bracket calc: `calc tax_on(amount: Money<USD>) -> Money<USD>`.
    - Open-textured credits remain `Determine`.
    - **Evidence (Partial):** the 22% slab is included and missing money is
      `InvalidInput`. Name-dispatch on `tax_on` is not execution of the
      calc body. Source that returns a string is not arithmetic.

11. **Solver** (`fidryn-solve`)
    - Bounded DPLL over declared finite domains for Explore/Skeptical.
    - Used by `fidryn-verify` instead of only nested for-loops when the
      domain product is large; still exhaustive on the declared space.
    - Not an SMT solver and not a counterexample certificate to
      determinacy (matrix: SMT row is No).

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
    - Parent edges on the DAG are this pass; a full proof DAG for every
      rule application is Partial.

14. **Mill UI**
    - `fidryn ui [--port N] [--no-open]` on 127.0.0.1 only.
    - Check, run, explore, render. No live filing from the UI.
    - Outcome JSON uses the mill/CLI evaluation-report envelope (`fidryn.evaluation-report/v0.1` with nested `fidryn.outcome/v0.1`
      fields, camelCase, hex ids, tagged `Value`).

15. **General worklist evaluator**
    - Apply constitutive/derive/prescriptive rules from Core, not only
      fixture-specialized query plans. Existing acceptance tests must
      still pass.
    - **Evidence (Partial):** this pass evaluates `QueryPlan::Evaluate`
      terms, `CoreFunction.body`, and real rule guards/consequences in
      Core. Name-dispatch on query names is not execution. UniqueOccupant,
      StatusOf, and RunDecision remain specialized plans (correctly
      extracted, not substring FOIA). A worklist that fires every Core
      rule is still required and not complete.

## Crate ownership for parallel work

See [`REVIEW-FIX-CONTRACT.md`](REVIEW-FIX-CONTRACT.md). Docs own only
this file, [`implementation-status.md`](implementation-status.md), and
[`ARCHITECTURE.md`](ARCHITECTURE.md).

Do not weaken no-false-determinacy. Rust 1.98. `cargo fmt`,
`clippy -D warnings`, `cargo test --workspace`.
