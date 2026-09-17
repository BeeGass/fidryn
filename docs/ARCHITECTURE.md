# Fidryn v0.1 architecture

Fidryn is a reference interpreter for legal instruments. It is a compiler
plus a partial evaluator: parse a `.fr` module, authenticate a source
manifest, elaborate to Core IR, then evaluate a query under a handler.
The semantics do not depend on Rust. Rust is the implementation language.

This document is the frozen crate contract. When prose in the originating
essay, an example, and Core disagree, Core plus the acceptance tests win.
A `.fr` file is not an executable spec. Evidence of what the pipeline
actually does is [`implementation-status.md`](implementation-status.md).

A computation is determinate only when every still-admissible resolution
agrees, or a competent authority has already made a determination that
is operative in the relevant context.

## How a language is built

A programming language implementation is a pipeline of total functions over
trees, not a single interpreter of source text:

```
bytes
  -> lexer (tokens + trivia)
  -> parser (AST this pass; lossless rowan CST intended)
  -> AST (typed, trivia dropped)
  -> name resolution / imports
  -> elaboration (surface shorthands -> Core)
  -> type and effect checking
  -> Core IR (canonical, immutable)
  -> evaluator (worklist + algebraic effects)
  -> handlers (CaseFile | Scenario | Explore | Skeptical)
  -> Outcome<T> + proof-relevant trace
```

v0.1 is a reference interpreter, not a native-code compiler. There is no
LLVM backend. `calc` functions are total, effect-free, and terminating;
they run in the evaluator, not as machine code.

## Improvements over the essay's gaps

The essay left a grammar sketch, deferred conflict oracles, and noted that
no-false-determinacy is only as honest as the admitted model. v0.1 closes
those as follows:

1. `grammar.ebnf` is closed. Unknown body fields are parse errors, not
   extension points. There is no catch-all `Ident Expr` statement.
2. Keywords that introduce declarations are contextual. `source`, `entity`,
   `office`, `rule`, and similar words are identifiers except in declaration
   position. Reserved words are the small set in the grammar.
3. Case records declare `admissibleCompletions`. Every outcome JSON includes
   `modelBoundary` (`outsideScope` plus that declared space). An omitted
   interpretation cannot silently shrink the model.
4. `Determinate` requires a nonempty exhaustive completion set. An empty
   set is `Inconsistent` or `Suspended`, never a vacuous determinate answer.
5. An `OpenBranch` without a checked convergence certificate is never
   dropped. It yields `Suspended`.
6. Canonical JSON follows RFC 8785 (sorted object keys, no insignificant
   whitespace). Replay is byte-identical.
7. `Parse.green` is a lossless Rowan CST with trivia.
   `syntax().text()` equals the source. Not every Pratt subexpression is
   its own node. The formatter round-trips modules apart from documented
   whitespace normalization.
8. Expressions follow the essay's precedence in `grammar.ebnf`. Chained
   comparisons are rejected. Evaluate/calc/guard bodies lower to Core
   `Term`; that is not a rowan CST and not a claimed Pratt implementation.
9. `LegalState` is an immutable value. Transitions allocate a new state.
10. `Prop` has no conversion to `bool`. Guards must use `operative`,
    `assumed`, `determined`, or `necessarily`.
11. Effect handlers are a trait that returns `HandlerResult<T>`, never a
    Boolean oracle.
12. Durations carry a calendar kind (`days` vs `working_days` vs
    `counted_days`). Money is `USD(500.00)`, never an untyped decimal.
13. Node IDs are blake3 content hashes. Maps and sets use `BTreeMap` /
    `BTreeSet` so serialization is deterministic.
14. `ignoredOpenIssues` is legal only with a `convergenceCertificate`.
    A claims-digest certificate is not covering proof. Covering checks
    belong in `fidryn-kernel`, not in proof search.
15. Conflict and applicable-law oracles run. Unique ranked results resume;
    ties suspend or explore. Silent picks are forbidden.
16. Filing adapters, tax calc modules, guarded recursion, user-defined
    effects, quantification, court procedure, case-law graphs, constrained
    render, a bounded solver, and a localhost mill UI are in-tree. In-tree
    is not executed. See the capability matrix. Live filing still requires
    `--live` and `FIDRYN_ALLOW_LIVE_FILING=1`.

## Crate graph

```
fidryn-syntax          parser, formatter, Rowan CST (not every Pratt node)
fidryn-core            IR, types, Outcome, LegalState, diagnostics
fidryn-hir             names, imports, elaboration  (syntax + core)
fidryn-check           types, effects, authority, time, strata (hir + core)
fidryn-eval            worklist evaluator (core)
fidryn-handlers        CaseFile, Scenario, Explore, Skeptical (core + eval)
fidryn-verify          bounded explorer and invariants (eval + handlers)
fidryn-kernel          covering-check boundary (accept/reject; no proof gen)
fidryn-driver          incremental check/run memo
fidryn-trace           DAG, canonical JSON, source maps (core)
fidryn-render          constrained templates
fidryn-adapt           filing adapters
fidryn-solve           bounded DPLL (eager product today; lazy yield remaining)
fidryn-cli             fidryn binary and mill UI
```

Do not add reverse dependencies. Do not put evaluator logic in `syntax`.
Do not put parser logic in `core`.

## Public APIs

### fidryn-syntax

```rust
pub fn parse_file(source: &str) -> Parse
pub struct Parse {
    pub source: String,
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<fidryn_core::Diagnostic>,
    pub green: rowan::GreenNode, // lossless CST; not every Pratt node
}
impl Parse {
    pub fn module(&self) -> Option<ast::Module>;
    pub fn has_errors(&self) -> bool;
}
pub fn format_module(source: &str) -> Result<String, fidryn_core::Diagnostic>
pub fn lex(source: &str) -> Vec<Lexeme>
```

`Parse` always returns a tree (typed `ast::Module` plus `green`). Recovery wraps a
malformed declaration so later declarations still parse. Diagnostics use
`E100` for parse errors.

### fidryn-core

See the crate itself. The important types are `CoreModule`, `Outcome<T>`,
`LegalState`, `Guard`, `OpenRequest`, `HandlerResult<T>`, `RunContext`,
`CaseRecord`, `SourceManifest`, `Diagnostic`, `EngineError`,
`CheckedCertificate`.

`Outcome::Determinate` may include `ignored_open_issues` only when
`convergence_certificate` is a `CheckedCertificate`.
`CheckedCertificate::verified(...)` is a claims-digest binder, not a
covering proof. Ignoring open issues requires a covering certificate
(`verified_covering` + complete `CoverageWitness`). A hash of the open
issues is not covering. `CompletionProofId::of(b"P11")` is never a
certificate. See [`OBLIGATIONS.md`](OBLIGATIONS.md).

### fidryn-hir

```rust
pub fn elaborate(parse: &fidryn_syntax::Parse, manifest: &SourceManifest)
    -> Result<HirModule, Vec<Diagnostic>>
```

### fidryn-check

```rust
pub fn check(hir: &HirModule, manifest: &SourceManifest)
    -> Result<CoreModule, Vec<Diagnostic>>
```

### fidryn-eval

```rust
pub fn evaluate<H: Handler>(
    module: &CoreModule,
    query: &QueryName,
    args: &BTreeMap<String, Value>,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
) -> Result<Outcome<Value>, EngineError>
```

Unknown queries, exhausted fuel, and unsupported operations are
`EngineError`, never `Outcome::Inconsistent`. `QueryPlan::Evaluate`
runs a `Term`; `Term::Bool(true)` is the `Evaluate { true }` body.
`seq` and `require` are Core `Term::Apply` operations. The required
contract is `require true; return 7` → Determinate 7; false or
unresolved `require` does not run the rest.

The only missing-body arithmetic helper is the explicit builtin
`ordinary_income_tax`. Substring `tax` is not a builtin.

### fidryn-kernel

Covering-check boundary, not proof generation. Search stays in
`fidryn-verify` / `fidryn-solve`. This crate only accepts or rejects a
covering claim (`accept_covering`, `digest_is_not_covering`). A digest
certificate is not covering. Types `CoverageWitness` /
`verified_covering` are the contract; they are not yet on
`CheckedCertificate`.

### fidryn-handlers

```rust
pub struct CaseFile { pub record: CaseRecord }
pub struct Scenario { pub assumptions: Vec<Assumption> }
pub struct Explore { pub bounds: ExplorationBounds }
pub struct Skeptical { pub inner: Explore }
```

Aggregation compares full `Value` with `PartialEq`, never `display_label()`.
A determinate branch plus a contingent branch is not determinate. Preserve
nested alternatives and unresolved requests.

Conservative precedence (first match wins):

1. every branch unsatisfiable -> `Inconsistent`
2. uncertified `OutsideCompetence` -> `OutsideCompetence`
3. uncertified open request -> `Suspended`
4. unresolved conflict -> `NormConflict`
5. divergent total answers, or mixed determinate/contingent -> `Contingent`
6. convergent nonempty answers (open branches certified) -> `Determinate`

### fidryn-cli

```
fidryn fmt PATH
fidryn check PATH
fidryn run PATH --query NAME --case RECORD.json --valid-at TIME --known-at TIME
    [--arg KEY=VALUE]
fidryn explore PATH --query NAME --case RECORD.json --bounds BOUNDS.json
    --valid-at TIME --known-at TIME
fidryn explain TRACE_ID --format text|json|dot
fidryn verify PATH --property NAME
fidryn diff OLD_SNAPSHOT NEW_SNAPSHOT --query NAME
fidryn render PATH --template TEMPLATE
```

`run` never chooses a completion. `--arg provision=...` sets
`case.facts["provision"]`. `TIME` is ISO 8601 / RFC 3339 (`Z` or a numeric
offset). Compiler diagnostics print `Diagnostic`'s `Display` and exit 1.
`explore` requires explicit bounds.

## Pipeline

```
parse
  -> authenticate the fixed source manifest
  -> resolve names and imports against that manifest
  -> elaborate surface conveniences
  -> infer and check types/effects
  -> validate sources, authority, and time
  -> stratify derivations
  -> verify finite domains and closure
  -> emit CoreModule
  -> evaluate guards and handler requests
  -> stage named effects and evaluate conflict doctrines
  -> validate authority and constitutive basis
  -> check CoreAssertion obligations
  -> commit consequences atomically
  -> append proof-relevant trace
  -> Outcome<T>
```

A guard that is `Open` or `Conflict` stages no partial mutation. Name-dispatch
on a query name is not evaluation of Core.

## Diagnostic codes

```
E100 ParseError
E200 UnresolvedName
E210 TypeMismatch
E310 PropAsGuard
E320 DirectInstitutionalMutation
E330 MissingConstitutiveBasis
E340 InvalidAuthorityKind
E410 AmbiguousSelection
E420 UnhandledEffect
E430 MissingQueryGoal
E431 InvalidClauseReference
E510 UnjustifiedPriority
E511 UnknownConflictTarget
E520 InvalidBitemporalInterval
E530 NegativeRecursion
E540 MissingLegalSource
W610 OpenUniverse
W620 BoundedVerification
```

Compiler diagnostics are not legal outcomes. A valid program may evaluate
to `Suspended`, `Contingent`, or `NormConflict`.

## Toolchain

Rust 1.98, edition 2024. On this Mac, `.cargo/config.toml` forces
Command Line Tools clang because the Xcode license is unsigned.

Before considering a crate done: `cargo fmt`, `cargo clippy -p CRATE -- -D warnings`,
`cargo test -p CRATE`.
