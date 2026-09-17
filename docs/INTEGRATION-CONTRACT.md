# Integration contract (connect existing machinery)

Do not reimplement helpers. Wire them into ordinary compile, eval, and
CLI paths. No-false-determinacy. PATH `/opt/homebrew/bin:$HOME/.cargo/bin:/usr/bin:/bin`,
DEVELOPER_DIR=/Library/Developer/CommandLineTools.

## Kernel meaning

`CoverageWitness` gains `branches: Vec<BranchClaim>` (`#[serde(default)]`)
where `BranchClaim { bindings: BTreeMap<String, Value>, answer: Value }`.

Shape completeness is not enough. `fidryn-kernel` must **evaluate** each
branch against the claimed program and query:

1. Shape: complete, nonempty, `examined == total == branches.len()`, unique bindings.
2. For each branch, bind `bindings` into a case clone (facts by name, and
   interpretation/choice keys if prefixed `i:` / `c:`), `evaluate` the query.
3. Determinate result must equal that branch’s claimed answer.
4. All branch answers must equal `witness.answer`.
5. A digest-only certificate remains not covering.

Reject: fabricated `false → true` for `return b`; omitted world with
duplicate of the other; witness for a different query/program (hash bind
already does program/query; also re-eval will fail).

Accept: `return b || !b` over `{false, true}` both true.

Kernel may depend on `fidryn-eval`. It does not search; it checks supplied
branch derivations.

## Surface duty

`duty Name { bearer; claimant; attaches when …; content; due … }` must
lower to `CoreDuty` and control `duty_status(Name)` / UniqueOccupant-style
queries **without** `duty_step` in the source.

Status from declaration + case:
- attach guard not held → Unresolved
- guard held, no performance, deadline not passed → Attached
- deadline passed, no performance → Breached (`breached: true`)
- performance after deadline → Performed, `breached` remains true
- performance before deadline → Performed, `breached` false

Changing `due` in the `.fr` file changes the lifecycle without Rust edits.

## Authority on ingestion

`CaseRecord.events` with kind `duty` / `authority` / institutional do not
enter `into_state` / eval projections unless an `AuthorityGrant` covers
the action at `record_time`. Assumptions (`kind: "assumption"`) skip that
gate. Missing grant: drop the event from committed state (or leave it
unprojected). Do not let a submitted “Performed” event bypass the duty
machine.

## Continuation frames

`10 + seq(A, await B, 2)` after answering B yields 12, A not replayed.
Implement a frame stack so nested seq under Binary/Call/If is not only
RememberingHandler. Sequence position lives on the seq frame.

## Frontends

`Driver::check_path` must call `check_with_sources(..., Some(parent_dir))`.
CLI `compile_module` uses that path. `explore` already uses verify stream;
keep it. Prefer `run_report` for CLI/server JSON when emitting outcomes
so trust/coverage/unresolved survive.

## Programs

Extend or add `tests/programs/` with a declared duty (not duty_step),
deadline interpretation, PaymentRecord suspend, late vs on-time.
Conformance tests in `fidryn-verify/tests/domains.rs`.
