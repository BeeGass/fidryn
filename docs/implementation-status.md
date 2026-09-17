# Capability matrix

A `.fr` file is not an executable specification. Presence under `examples/`
or `prelude/` does not mean the interpreter implements the feature.

A feature counts as implemented only when syntax, name resolution, checking,
evaluation, uncertainty, provenance, and tests agree
([`REVIEW-FIX-CONTRACT.md`](REVIEW-FIX-CONTRACT.md)).

**Determinate** only when every still-admissible resolution of the open
issues agrees, or a competent authority has already made a determination
that is operative in the relevant context. `run` never chooses a
completion. Uncertified open issues are `Suspended`, never a silent
`Determinate`.

This table is the **evidence-backed status after the review-fix pass**.
It is not a snapshot of the pre-pass tree, which still name-dispatched
many queries. `cargo test --workspace` and `clippy -D warnings` are
green, including the 28 review-kit regressions.

Cells are `Yes` / `Partial` / `No`.

- **Parsed** — grammar and parser produce a structured form (not only a
  source slice that happens to contain the keyword).
- **Resolved** — names, imports, and binders bind against the
  authenticated manifest.
- **Checked** — types, effects, authority, or the relevant diagnostic
  actually fire (not a substring heuristic).
- **Executed** — the evaluator or handler runs Core terms, plans, or
  rules. Selecting a fixture by query name or `Term::Ident(query_name)`
  is **not** execution.
- **Traceable** — a proof-relevant DAG records the step with parent
  edges, not only a content-hashed `TraceId`.
- **Conformance tests** — shared tests under `conformance/` and/or
  `crates/fidryn-cli/tests/semantic_regressions.rs` (crate unit tests
  alone are `Partial`).
- **This pass** — `In progress` if a crate-owned agent is changing the
  row under the review-fix contract; `Not this pass` if the language
  still requires it but this review does not ship it.

Language ambition is unchanged. `No` and `Partial` are evidence, not
deferrals of the requirement. See
[`FULL-IMPLEMENTATION.md`](FULL-IMPLEMENTATION.md) and
[`ARCHITECTURE.md`](ARCHITECTURE.md).

## Matrix

| Feature | Parsed | Resolved | Checked | Executed | Traceable | Conformance tests | This pass |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Expression literals (`true`/`false`) | Yes | Yes | Yes | Yes | Partial | Yes | Done |
| Function/calc bodies | Yes | Yes | Partial | Yes | Partial | Yes | Done |
| Rule guards and consequences | Yes | Partial | Partial | Yes | Partial | Partial | Done |
| Query Evaluate plan | Yes | Yes | Yes | Yes | Partial | Yes | Done |
| UniqueOccupant | Yes | Yes | Partial | Yes | Partial | Yes | Done |
| StatusOf / closed-world absence | Yes | Yes | Partial | Partial | Partial | Yes | Done |
| RunDecision (not always FOIA) | Yes | Yes | Partial | Yes | Partial | Yes | Done |
| Observe matching (schema+subject+time) | Yes | Yes | Partial | Yes | Partial | Yes | Done |
| Determine matching (protocol+issue) | Yes | Yes | Partial | Yes | Partial | Yes | Done |
| Aggregation (full values) | Yes | Yes | Yes | Yes | Partial | Yes | Done |
| Exploration coverage (all domains) | Yes | Yes | Partial | Yes | Partial | Yes | Done |
| Convergence certificates (checked handles) | Yes | Yes | Yes | Partial | Partial | Yes | Done |
| Source manifest loading and import digests | Yes | Yes | Yes | Yes | Partial | Yes | Done |
| Tax bracket arithmetic | Yes | Yes | Partial | Yes | Partial | Yes | Done |
| Conflict oracle | Yes | Partial | Partial | Partial | Partial | Partial | Done |
| SelectApplicableLaw | Yes | Partial | Partial | Partial | Partial | Partial | Done |
| Fuel / recursion SCC | Yes | Partial | Partial | Partial | Partial | Partial | Done |
| Prop vs Bool (E310) | Yes | Yes | Yes | Yes | Partial | Yes | Done |
| JSON schema (`weight`) | Yes | Yes | Partial | Partial | No | Yes | Done |
| Mill outcome envelope | Yes | Yes | Partial | Partial | Partial | Partial | Done |
| Filing adapter states | Yes | Yes | Yes | Yes | Partial | Partial | Done |
| Trace DAG parent edges | Yes | Yes | Partial | Partial | Partial | Partial | Done |
| Rowan CST | Yes | Yes | Partial | Yes | Partial | Yes | Done |
| Resumable suspensions | Yes | Yes | Partial | Yes | Partial | Partial | Done |
| Incremental compilation / Salsa | Yes | Yes | Partial | Yes | Partial | Partial | Done |
| SMT counterexample to determinacy | Yes | Yes | Partial | Yes | Partial | Partial | Done |
| Parameterized modules | Yes | Yes | Yes | Partial | Partial | Partial | Done |
| Finite quantification (`for_all`/`exists`) | Yes | Partial | Partial | Yes | Partial | Partial | Done |

## What “Executed = Yes” means this pass

`QueryPlan::Evaluate` runs a Core `Term`. `Term::Bool(true)` is the
`Evaluate { true }` body. It is never encoded as `Term::Ident(query_name)`.
Unknown queries, exhausted fuel, and unsupported operations are
`EngineError`, never `Outcome::Inconsistent`.

Observe matches **schema**, issue/arguments (subject), and
`observed_at <= known_at`. Determine matches **protocol and issue**.
`established: false` from a competent authority is a negative
determination, not `OutsideCompetence`. Aggregation compares full
`Value` with `PartialEq`, not `display_label()`. A determinate branch
plus a contingent branch is not determinate. Explore enumerates declared
interpretation, evidence, and choice domains; assignment identity is the
full binding.

## Partial and No (honest limits after this pass)

**Function/calc bodies.** Signatures and `CoreFunction.body` are
populated. `If` / `else` / `else if` parse in calc blocks; `{ amount * 0.10 }`
is a mul. The interpreter runs `Binary`, `Call`, `If`, `Field`, and
decimals. Effectful `fn` bodies are not a general algebraic-effect
worklist. UniqueOccupant / StatusOf / some decisions remain specialized
plans.

**Rule guards and consequences.** `evaluate` of `QueryPlan::Evaluate`
runs a bounded (64) staged worklist over `CoreDecl::Rule`.
`Guard::Operative` / `Derived` hold only when the proposition is in the
staged set; missing is Unknown (may Suspend), not closed-world false.
`operative ExemptFromBOI` is not a date heuristic. Prescriptive duties
and full transaction commit are not this worklist.

**UniqueOccupant.** Unique occupancy from the authority ledger or
`OccupancyRecord`. Succession when `Incapacitated` is derived/determined
or concurring evidence meets `required_concurring` (default 2). Schema
comes from the case/module, not a `"Filing"` substring. Ranked accepted
nominations plus a recorded interpretation (`I1`/`I2` or a nominee
name) select the successor. Unrelated interpretation families do not
steal that choice. No default occupant named Bryan.

**StatusOf / closed-world.** Only schema identity `OfficialFilingRecord`
counts as filing. Substring `"Filing"` must not. Closed-world absence
requires `ClosureRecord { closed: true }` for that domain; otherwise
`Suspended`. This is still a StatusOf evaluator, not a full status
ledger interpreter.

**RunDecision.** Declared `CoreDecision.requirements` run first, with
temporal evidence filters. The FOIA helper is only a fallback when
there is no usable CoreDecision and the name is the FOIA process.

**Convergence certificates.** `CheckedCertificate` has private fields;
only `CheckedCertificate::verified(...)` constructs it.
`CompletionProofId::of(b"P11")` is never a certificate. Ignored open
issues without a checked handle are `Suspended`. There is no independent
proof checker beyond construction discipline.

**Tax bracket arithmetic.** `examples/tax/federal-tax.fr` and
`prelude/tax.fr` define `ordinary_income_tax_formula` as an if/else calc
on 11925 / 48475 / 103350 and 10/12/22/24%. The fourth bracket includes
the completed 22% slab. A Rust helper remains only for a String stub
body. Missing money is `EngineError::InvalidInput`.

**Conflict oracle / SelectApplicableLaw.** Unique ranked results resume;
ties suspend or explore; list order is never a tie-break. Matching is
name-level (doctrine graph labels; weight then later-in-time). Full
argument-graph reasoning and specific-over-general doctrines are not
done.

**Fuel / recursion SCC.** `fuel = N` parses. `calc` must not recurse
(E530). `fn` recursion without fuel is E530. The checker is ident-in-body,
not a call-graph SCC. The evaluator decrements fuel; exhaustion is
`EngineError::FuelExhausted`. That is not structural termination.

**Prop vs Bool (E310).** `Prop` has no conversion to `Bool`. Guards must
use `operative`, `assumed`, `determined`, or `necessarily`. E310 is a
type/effect check, not `src.contains("if Incapacitated(")`. Runtime
values stay disjoint (`Value::Prop` ≠ `Value::Bool`).

**JSON schema (`weight`).** Manifest schema and `SourceWeight` include
`binding | controlling | persuasive | explanatory`. The JSON checker
validates document shape. It does not authenticate artifacts.

**Mill outcome envelope.** Mill stays on `127.0.0.1`. `/api/run` and
`/api/explore` must serialize outcomes with camelCase fields, hex
trace/certificate ids, and internally tagged `Value`. The HTTP wrapper
`{ok, outcome, error}` is transport. The legal document is
`fidryn.outcome/v0.1` (CLI `render_outcome`), including `modelBoundary`.

**Filing adapter states.** `DryRun` / `Submitted` / `Rejected`. Live HTTP
requires `--live` and `FIDRYN_ALLOW_LIVE_FILING=1`. A transport receipt
does not establish `Filed`. No live filing from the mill.

**Trace DAG parent edges.** `TraceNode` gains parent edges this pass.
Eval does not yet emit a parented node for every rule application.
`explain` of a bare hashed id may still have an empty `nodes` array;
emptiness means no persisted DAG, not an omitted field.

**Rowan CST.** `Parse.green` is a lossless `rowan::GreenNode`. Trivia is
leaves. `syntax().text()` equals the source. Typed `ast::Module` is
still built alongside. Not every Pratt subexpression is its own node.

**Resumable suspensions.** `evaluate_session` stores a `Continuation`
(residual term/plan, bindings, derived worklist, fuel). `resume`
continues that residual on the same case snapshot; a changed snapshot
recomputes. JSON outcomes still omit the continuation (not a wire
value).

**Incremental compilation.** `fidryn-driver::Driver` memos parse/check
by blake3(source)+manifest and run by program+query+case+times.
Salsa-style explicit memoization; the `salsa` crate is not a dependency.
Comment-only edits miss the check cache (byte identity).

**Counterexample to determinacy.** `check_determinacy` evaluates one
admissible assignment to `v` then searches for `≠ v` over **declared**
domains only (`fidryn-solve` enumerate/DPLL, no Z3). Two values are
`Counterexample`, never silent Determinate. Engine errors are `Unknown`.

**Parameterized modules.** `module Name<T>` and `import Name<Arg>` parse.
`fidryn_check::instantiate` substitutes `Nominal(T)`. Arity mismatch is
E210. Cross-file module instantiation and value parameters are not done.

**Finite quantification.** `for_all` / `exists` parse and execute over
a `Value::Set`, a named set fact, or a `ClosureRecord { closed: true }`.
An open domain without closure is `Suspended` (`NeedEvidence`
`ClosureRecord`), not determinate false. Nested quantifiers and
open-world proofs are not done.

## Remaining honest limits

- Pratt still sits beside the Rowan tree; not every expr is a CST node.
- `resume` is in-process, not a serialized rest-of-computation on the wire.
- Incremental compilation is a memo table, not the `salsa` crate.
- Determinacy search is exhaustive on **declared** finite domains, not a
  general SMT encoding of evaluation.
- Module parameters are type-name substitution, not a module calculus
  with exports and override points.
