# Review-fix contract (pass 3 — remaining work)

Complete the remaining review items. Do not weaken no-false-determinacy.
Rust 1.98 / edition 2024.
PATH: `/opt/homebrew/bin:$HOME/.cargo/bin:/usr/bin:/bin`

## Ownership

| Agent | Owns |
| --- | --- |
| syntax | `crates/fidryn-syntax/**`, `grammar.ebnf` (module type params + CST) |
| hir-check | `crates/fidryn-hir/**`, `crates/fidryn-check/**` |
| eval | `crates/fidryn-eval/**` |
| verify | `crates/fidryn-verify/**` |
| driver | NEW `crates/fidryn-driver/**` and workspace `Cargo.toml` members/deps only |
| tests | add tests in the crate you own; do not weaken `semantic_regressions.rs` |

## 1. Rowan CST (syntax)

`rowan` is already a dependency. Build a real GreenNode:

- `SyntaxKind` for tokens + `MODULE`, `DECL`, `HEADER`, `EXPR`, … (repr u16, `Language` impl).
- After lex, wrap every token (including trivia) as a leaf. Parser `start_node`/`finish_node` around module, each declaration, each expr if feasible.
- `Parse` exposes `green: GreenNode` and `syntax(): SyntaxNode`. Source text is recoverable from the tree (`syntax.text() == source`).
- Keep the existing typed `ast::Module` (do not break hir). CST is additional, not a full rust-analyzer rewrite of Pratt.
- Test: `parse_file(src).syntax().text() == src` for a small module and for `examples/trust/bryan-revocable-trust.fr`.
- Trivia (whitespace/comments) is not dropped from the green tree.

## 2. Parameterized modules (syntax + hir-check)

Grammar:

```
Module ::= "module" QName TypeParameters? "version" String "{" ModuleItem* "}"
TypeParameters ::= "<" Ident ("," Ident)* ">"
ImportDecl may take TypeArgs: "import" QName TypeArgs? "version" ...
TypeArgs ::= "<" Type ("," Type)* ">"
```

- Parse `module Box<T> version "0.1.0" { entity x: T ... }`
- Parse `import Box<NaturalPerson> version "0.1.0"`
- Hir stores `type_params: Vec<String>` and import `type_args: Vec<String>`
- Check: arity must match (E200/E210). Instantiation substitutes `T` in entity types / signatures when lowering an imported-style instantiation **in the same file** (a second module in the same parse is not required). Minimum: a module with params type-checks when referenced types use the param; a dummy instantiate helper `substitute_type(ty, params, args)` used in check.
- Test: `module Id<T> version "0.1.0" { entity X: T }` parses; `import Id<NaturalPerson>` arity 1 ok; `import Id<A,B>` against one param is an error.

If multi-module files are not supported, implement `fidryn_check::instantiate(module, args) -> CoreModule` that substitutes params.

## 3. Resumable evaluate (eval)

Do **not** break `evaluate(...) -> Result<Outcome<Value>, EngineError>`.

Add:

```
pub struct Continuation { /* residual Term or QueryPlan + bindings + derived props + fuel */ }
pub struct EvalSession {
    pub outcome: Outcome<Value>,
    pub continuation: Option<Continuation>,
}
pub fn evaluate_session(...) -> Result<EvalSession, EngineError>
pub fn resume<H: Handler>(session: EvalSession, handler: &mut H, case: &CaseRecord, ...) -> Result<EvalSession, EngineError>
```

`evaluate` = `evaluate_session` then `.outcome`.

When Suspended, store the residual `QueryPlan`/`Term` and bindings so `resume` after `HandlerResult::Resume` continues **that** computation, not a full re-entry that forgets derived worklist facts. If the case snapshot identity changed, recompute (document that).

Test: query NeedJudgment, first session Suspended with continuation; handler then has the determination; `resume` is Determinate. Do not require the caller to call `evaluate` from scratch for the test (they may, but resume must work).

## 4. General UniqueOccupant / RunDecision (eval)

UniqueOccupant must not hardcode `"Bryan"`, `PhysicianCertificate`, `"Administer"`, `SuccessorEligibility`, or exactly two certificates as the only vacancy rule.

General calculus:

1. Unique established occupant of `office` (state occupancy or OccupancyRecord) → that person if succession is not triggered.
2. Succession triggers when `Incapacitated(occupant)` is derived/determined **or** when concurring evidence for that issue exists. Evidence schema is the request schema (PhysicianCertificate for the trust fixture because that is the evidence_type / NeedEvidence schema), not a substring `"Filing"`.
3. Ranked nominations for that office that have accepted (fact `{name}_accepted` or AcceptOffice evidence).
4. Recorded interpretation for **any** family in `case.interpretations` that is in `admissible_completions.interpretations` selects among ranked nominees (I1 = rank 1, I2 = rank 2, or a value equal to a nominee name).
5. No recorded interpretation and ≥2 accepted nominees → Contingent over those families.
6. Incomplete required evidence → Suspend (no fake certificate).

Trust tests must still pass (two PhysicianCertificates + I2 → Bob, missing occupancy → Suspend, one cert without checked cert → Suspend).

RunDecision: do not special-case FOIA **by name** if `CoreDecision` requirements can be evaluated. If `is_foia_process` remains, it may only run when the decision’s declared requirements match (HarmAnalysis/Segregability) **or** when no CoreDecision exists yet. Prefer generic requirement eval. Temporal filter on evidence stays.

## 5. Counterexample to determinacy (verify + solve)

```
pub enum Determinacy {
    Convergent { value: Value },
    Counterexample { a: Assignment, b: Assignment, va: Value, vb: Value },
    Unknown { reason: String },
    Suspended { requests: ... },
}
pub fn check_determinacy(module, query, case, ctx) -> Result<Determinacy, EngineError>
```

Use declared `admissible_completions` domains only (never invent). Evaluate one admissible assignment to `v`, then search for another with `≠ v`. Two different determinate values → Counterexample (not silent Determinate). All agree → Convergent. Incomplete/engine → Unknown, never Determinate. Empty admissible set → not Convergent.

`explore_query` may call this. Tests: two successor interpretations → Counterexample or existing Contingent; court-selects-i2 with occupancy → Convergent Bob.

No Z3 C dependency. fidryn-solve DPLL/enumerate is the backend (sound on the declared finite space).

## 6. Incremental compilation (driver)

New crate `fidryn-driver`:

- Memoize parse and check by blake3 of source bytes + manifest snapshot/artifacts.
- Memoize evaluate by program id + query + canonical case JSON + valid/known times.
- Changing a comment-only source may change the parse key (byte identity). Changing a query body must miss the check cache.
- `Driver::check(path) -> Result<CoreModule, Vec<Diagnostic>>` and `Driver::run(...)`.
- Second `check` of identical bytes does not re-parse (test with a counter or `std::sync::atomic` parse hook **or** document via cache `hits()`).
- This is Salsa-style dependency memoization. You MAY use the `salsa` crate if it compiles on 1.98 without destabilizing the workspace; if not, an explicit memo map is acceptable and must track the keys above.
- Add workspace member + workspace.dependency. fidryn-cli MAY use Driver for `compile_module` if that is a one-line switch; do not rewrite the CLI.

## Tests and quality

`cargo fmt`, `cargo test -p <crate>`, `cargo clippy -p <crate> -- -D warnings`.
Keep `semantic_regressions` 28 passing. `parses_every_example_fr_file` passing.
