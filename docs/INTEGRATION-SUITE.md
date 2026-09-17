# Integration suite (finite remaining obligations)

Do not reimplement fabricated-witness, source-driven due, nested seq, or
CLI `check_path` byte-auth. Those crate tests remain the evidence. This
file lists the six finite public-API obligations and the named tests in
[`crates/fidryn-cli/tests/integration_suite.rs`](../crates/fidryn-cli/tests/integration_suite.rs).

Command: `cargo test -p fidryn-cli --test integration_suite --offline`.

Public APIs used: `compile_source`, `evaluate`, `evaluate_scenario`,
`report_from_scenario`, `evaluate_session`, `resume`, `accept_covering`,
`accept_covering_eval`, `Outcome::determinate`, `render_report`,
`reject_lossy_export`.

PATH `/opt/homebrew/bin:$HOME/.cargo/bin:/usr/bin:/bin`,
DEVELOPER_DIR=/Library/Developer/CommandLineTools.

The 2026-09-17 review stays **open**. Green tests here are regressions,
not language closure. Remaining after this gate: independent kernel
without `evaluate`, packages, Salsa, SMT, surface transaction syntax,
mill must not gain filesystem from paste. The next gate beyond this
six-obligation suite is
[`crates/fidryn-cli/tests/boundary_review_20260917.rs`](../crates/fidryn-cli/tests/boundary_review_20260917.rs)
(2026-09-17 11:35 boundary review).

## 1. Verification strength

`CheckedCertificate` carries `CoverageMethod`: `None` (claims digest),
`Structural` (shape only), `FiniteReplay` (evaluate each branch).
`is_covering()` is true **only** for `FiniteReplay`.

- `verified` → method None, covering false
- kernel `accept_covering` → Structural, covering false
- kernel `accept_covering_eval` → FiniteReplay, covering true

`Outcome::determinate` with ignored issues requires `is_covering()`.
Structural evidence cannot authorize ignored issues.
`CoverageWitness::complete()` is shape-only with empty `branches`; that
cannot pass `accept_covering_eval`. Core `verified_covering` also
rejects empty branches (`require_replay_witness`).

Replay uses `CaseFile` only. No `fidryn-adapt`. If `admissibleCompletions`
declares a nonempty product, expected `total` is that product’s size, not
an unchecked witness-supplied total.

Named tests:

- `structural_accept_covering_cannot_authorize_ignored_issues`
- `finite_replay_accept_covering_eval_can_authorize_ignored_issues`
- `empty_coverage_witness_cannot_cover_via_accept_covering_eval`

## 2. Replay isolation

`accept_covering_eval` re-evaluates each claimed world on a cloned case,
a fresh `CaseFile`, and `LegalState::new()`. The caller’s `CaseRecord`
must equal a clone taken before the call: no mutation, no published
events.

Named test: `accept_covering_eval_does_not_mutate_caller_case`.

## 3. Assumption isolation

`kind: assumption` must **not** make operative `duty_status` Performed.

`CaseRecord.assumptions: Vec<Assumption>` (`serde default`) is
`{ id, payload }`. `evaluate` is operative: ignore assumption events and
the assumptions vec for committed status.

`evaluate_scenario(module, query, args, state, ctx, handler, case)`
applies `case.assumptions` as an overlay. Result
`EvaluationReport.execution_mode = Scenario` and `assumptions` copied
onto the report. The original `case.events` is not mutated.
`render_report` of that report must show `executionMode: scenario` and a
nonempty `assumptions` array on the public JSON envelope.

Named tests (same records, no authority grant):

- `operative_duty_performed_event_without_grant_is_not_performed`
- `operative_evaluate_does_not_treat_assumptions_as_performed`
- `scenario_assumption_can_report_performed_without_mutating_events`
- `scenario_render_report_includes_execution_mode_and_assumptions`

## 4. Public report fidelity

Schema `fidryn.evaluation-report/v0.1`:

```
schema, executionMode, assumptions, sourceTrust, verificationMethod,
coverage, outcomeDocument
```

`outcomeDocument` is the existing `fidryn.outcome/v0.1` object (keep
that schema unchanged as a projection). Qualifications that cannot fit
the old outcome live on the envelope. A scenario report must not be
reduced to an outcome-only export. This suite asserts JSON keys on
`render_report` output and calls `fidryn_trace::reject_lossy_export`
when that symbol is present.

CLI `run`/`explore` and mill `/api/run` `/api/explore` print/return the
report envelope. Mill pasted compile sets `sourceTrust: unauthenticated`.
Path compile may be `fixture` or `byteVerified`. In-memory
`compile_source` (the mill paste path) is unauthenticated.

Named tests:

- `render_report_schema_is_evaluation_report_v0_1`
- `scenario_report_is_not_outcome_only_export`
- `mill_pasted_run_is_unauthenticated_evaluation_report`

## 5. Duty instance isolation

`duty_status(Def, Instance)` (second arg optional; default `"default"`).
State key `duty:{def}:{instance}`. Payment/evidence for one instance
does not discharge another.

Attach guard unknown is `Unresolved`. A determination with
`established: false` is `NotAttached`, not `Unresolved` and not
`Attached`.

Named tests:

- `two_duty_instances_isolation`
- `attach_guard_unknown_is_unresolved_established_false_is_not_attached`

## 6. Transaction atomicity

`Apply("transaction", [step, ...])`: evaluate steps against a binding
overlay; on EngineError, Suspended, or Halt, discard overlay (no
institutional commit). On full Determinate, commit overlay. Two
`duty_step`s in one transaction where the second is illegal: first
attach does not persist. A suspend in the second step
(`require_authority` without a grant) likewise rolls back the prefix;
`evaluate_session` / `resume` must not mutate the
caller’s case.

Surface transaction syntax is Remaining. Resume-once for nested seq
lives in eval unit tests (`nested_seq_under_add_skips_completed_attach_on_resume`);
this suite does not reimplement nested seq.

Named tests:

- `transaction_rollback`
- `transaction_suspend_does_not_commit_prefix`

## Mill trust

Pasted source: `sourceTrust` unauthenticated. Do not read arbitrary paths
from mill. Missing hex artifacts remain E200 on path compile only.
In-memory `compile_source` (the mill paste path) is asserted
unauthenticated in `render_report_schema_is_evaluation_report_v0_1` and
`mill_pasted_run_is_unauthenticated_evaluation_report`. Mill must not
gain filesystem from paste.
