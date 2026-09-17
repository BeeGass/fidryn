# Review obligations and evidence

Workspace tests are regressions, not closure of the language. Independently
written Fidryn programs with unfamiliar names must control their own
execution without Rust evaluator changes. That is not yet true for every
construct.

The 2026-09-17 review is **open**. Targeted Section 1 counterexamples
and the workstream contract in [`WORKSTREAM-CONTRACT.md`](WORKSTREAM-CONTRACT.md)
have regressions. Green tests do not close the review. A claims digest
is not covering. A `fixture` digest is not byte-verified.

## 1. Immediate defects (regressions, not closure)

Section-1 items are recorded as
`tests/integration/adversarial_regressions.rs`. Those 16 tests
pass. That is a regression suite, not a proof of soundness. Covering
certificates and artifact-byte authentication now have named tests.
A claims digest is still not covering. A `fixture` digest is still not
byte-verified.

| Obligation | Status | Tests | Remaining limit |
| --- | --- | --- | --- |
| Require preservation | Landed | `requirement_is_not_discarded_from_query`; `requirement_is_not_discarded_from_evaluate_goal`; `goal_result_type_is_checked` | Query/Evaluate `require` lowers to `seq(require, …)`. Rule-body `require` and `using` names are not stored. False require suspends rather than a dedicated failed-guard outcome. The complete `require` contract is §2 (not this row). |
| Handler identity | Landed | `remembered_judgments_distinguish_subjects_in_one_run`; `snapshot_change_does_not_retain_unvalidated_old_answers` | Keys are `Debug` of the full `OpenRequest`. Case-identity change discards `answered`. Residual is still the full term/plan, not a call-frame continuation. |
| Succession aggregation | Landed | eval unit tests for suspended alternatives; UniqueOccupant tests | UniqueOccupant is still a specialized plan. Offices without a family still rank accepted nominees only. |
| Decimal wire | Landed | `decimal_case_round_trip_preserves_nominal_variant`; `decimal_case_and_text_case_have_distinct_semantic_encodings`; `cached_case_result_agrees_with_uncached_result` | Canonical case JSON uses tagged `{"kind":"decimal","data":"1.25"}`. Friendly bare strings stay strings. Covering JSON is `fidryn.canonical/v0.2+rfc8785`. |
| Effects | Landed | `automatic_queries_cannot_hide_transitive_effects`; `evaluate_int_as_bool_query_is_e210` | Recursive Γ ⊢ e : τ ! ε is not a full inference engine. Money vs Decimal remains compatible for tax closed forms. |
| Evidence subject | Landed | `evidence_does_not_match_subject_by_incidental_issuer_field` | Designated fields are subject/person/candidate/occupant/holder. `CaseDetermination.recorded_at` is optional. |
| Worklist deltas | Landed | `worklist_replacement_with_equal_cardinality_is_not_quiescence` | Truncated substitutions are an engine error, not a covering certificate. |
| Digest / program identity | Landed | `module_body_edit_invalidates_execution_cache`; `declared_empty_completion_domain_cannot_be_reopened_by_recorded_selection` | Run key hashes canonical CoreModule JSON. `ModuleId` is still name-derived. |
| Artifact-byte authentication | Landed | `matching_blake3_hex_authenticates_required_import`; `mismatched_blake3_hex_is_e200`; `hex_digest_without_bytes_is_e200`; `fixture_digest_authenticates_required_import` | `"fixture"` is `TrustProfile::Fixture`, never `ByteVerified`. Hex without a readable file is E200. |
| Verify pipeline (named literals) | Landed | `declared_true_property_survives_the_entire_compiler_pipeline`; `verify_property_proves_forall_true_over_people` | Named `verify Trivial { assert true }` lowers. Bounded `for_all`/`exists` over People with true/false bodies are decided. `TrusteeContinuity` stays Unknown. |
| Certificate covering | Implemented (this suite) | kernel fabricated-witness / duplicate-world / tautology tests; `structural_accept_covering_cannot_authorize_ignored_issues`; `finite_replay_accept_covering_eval_can_authorize_ignored_issues`; `empty_coverage_witness_cannot_cover_via_accept_covering_eval`; `accept_covering_eval_does_not_mutate_caller_case` | Shape-only `accept_covering` is Structural (`!is_covering()`). FiniteReplay is `accept_covering_eval`. Pure-value covering uses `eval_fragment`; duty and handlers still call `evaluate`. |
| String / arity | Landed | `ordinary_string_returning_function_executes`; `function_arity_is_not_filled_from_caller_bindings` | Callee env starts empty; arity mismatch is `InvalidInput`. |

Outcome schema `fidryn.outcome/v0.1` (`schemas/outcome-v0.1.json`) now
rejects, independently of each other:

- `determinate` without `value`, and a non-hex `trace`
- nonempty `ignoredOpenIssues` without a hex `convergenceCertificate`
- tagged `{ "kind": "bool", "data": ... }` where `data` is not a boolean

There are no hand-authored `fidryn.outcome/v0.1` fixtures under
`examples/` or `prelude/`. Informal snapshot JSON in CLI tests (`trace`
`"aa"`) is not validated against this schema. Optional probe:
`python3 conformance/probe_outcome_schema.py` (not invoked from cargo
tests). Schema presence of a certificate id is not covering proof.

## 2–5 workstreams

Acceptance is [`WORKSTREAM-CONTRACT.md`](WORKSTREAM-CONTRACT.md). Status
is **Landed** only when a named test exists for that obligation.
**Partial** means some syntax, IR, or tests exist and the contract is
incomplete. **Remaining** means not implemented. **In progress** is
used only where a sketch exists but the correctness obligation is unmet.

### 2. Compiler / execution

`seq` and `require` are Core operations (`Term::Apply` ctors). The
complete contract is:

| Program | Result |
| --- | --- |
| `require true; return 7` | Determinate 7 |
| `require false; return 7` | requirement-failure (`Suspended` NeedCustom require); 7 does not run |
| `require unresolved; return 7` | Suspended with that issue; 7 does not run |

False require is a failed guard for this invocation, not a compile error
and not Determinate false unless a later declared result says so.

| Item | Status | Evidence | Remaining |
| --- | --- | --- | --- |
| `seq` / `require` Core eval | Implemented | `independent_program_require_true_is_determinate_seven`; `independent_program_require_false_is_not_determinate_seven`; `nested_seq_under_add_skips_completed_attach_on_resume` | Nested seq is implemented in eval. This suite does not reimplement it. Rule-body `require` is not stored. RememberingHandler still used for reusable observations. |
| Tax builtin | Landed | eval: missing-body helper is only `ordinary_income_tax`; other missing bodies are `Unsupported` | Closed-form `.fr` calc still used when a body exists. |
| Records vs tagged values | Landed | `{"kind":"bool","data":false}` is Bool; `{"kind":"record","data":{…}}` is Map | Untagged objects still become `Value::Map`. Covering JSON is `fidryn.canonical/v0.2+rfc8785`. |

### 3. Institutional state

| Item | Status | Evidence | Remaining |
| --- | --- | --- | --- |
| Duty status machine | Landed | eval `duty_step` tests: late perform keeps `breached`; illegal discharge commits nothing | History is in bindings/`duty:{name}`, not a full event ledger query API. |
| Surface duty integration | Implemented (this suite) | `duty_status(PayInvoice)` from `.fr`; `operative_duty_performed_event_without_grant_is_not_performed`; `scenario_assumption_can_report_performed_without_mutating_events`; `scenario_render_report_includes_execution_mode_and_assumptions`; `two_duty_instances_isolation`; `transaction_rollback`; `transaction_suspend_does_not_commit_prefix`; `attach_guard_unknown_is_unresolved_established_false_is_not_attached`; `transaction-atomic.fr` | Source-driven due is implemented. Scenario overlay is `evaluate_scenario`, not operative `evaluate`. Surface `transaction { … }` lowers to `Apply("transaction", steps)`. |
| Authority grants | Landed | `AuthorityGrant` + `covers`; eval `require_authority` suspends without a grant; revoked grant does not authorize; unrevoked `delegate_of` does | Occupancy is still a separate UniqueOccupant path. No appeal or supersession graph. |

### 4. Reasoning / proofs

| Item | Status | Evidence | Remaining |
| --- | --- | --- | --- |
| Covering certificates | Implemented (this suite) | fabricated `false→true` rejected; duplicate-world omission rejected; `b \|\| !b` FiniteReplay accepted; Structural cannot `Outcome::determinate` with ignored issues; `empty_coverage_witness_cannot_cover_via_accept_covering_eval` | Digest is not covering. Pure-value covering uses `eval_fragment`. Top-level `duty_status` / `require_authority` covering uses the institutional fragment, not `evaluate`. UniqueOccupant and handler plans still replay through `evaluate`. |
| Streaming search | Landed | `stream_budget_one_on_two_by_two_exceeds_without_full_product`; `budget_exhaustion_is_unknown_not_convergent`; `counterexample_returns_before_remaining_space`; `smt_check` | `enumerate` still collects a stream with a huge budget for existing tests. SMT-lite is finite-domain equality plus DPLL, not Z3. |
| Finite quantifiers | Partial | `for_all_over_closed_positive_set_is_true`; `nested_for_all_over_closed_sets_with_true_is_true`; `for_all_open_ident_domain_without_closure_suspends` | Nested closed-set `for_all`/`exists` evaluate. Open-world nested proofs and `TrusteeContinuity` stay Unknown. |

A trusted evaluator may establish covering by exhaustive finite search.
`fidryn-kernel` checks supplied branch derivations against `evaluate`;
it does not search. Shape-only `accept_covering` is not that check.
The 2026-09-17 review is still **open**.

### 5. Application / packages

| Item | Status | Evidence | Remaining |
| --- | --- | --- | --- |
| `fidryn-kernel` crate split | Implemented (this suite) | fabricated-witness rejection; Structural vs FiniteReplay determinate gate; `empty_coverage_witness_cannot_cover_via_accept_covering_eval`; `accept_covering_eval_does_not_mutate_caller_case` | Not a proof generator. Pure-value covering is independent of `evaluate`; duty/handler plans are not. |
| `fidryn-driver` crate split | Landed | `run_report`; `check_path_matching_blake3_authenticates_and_tamper_is_e200`; `render_report_schema_is_evaluation_report_v0_1`; `scenario_report_is_not_outcome_only_export` | Mill pasted source still uses `check` without files. Report envelope is `fidryn.evaluation-report/v0.1`; outcome JSON stays a projection (`additionalProperties: false`). Mill must not gain filesystem from paste. |
| CLI `check_path` byte-auth | Implemented (this suite) | `Driver::check_path` → `check_with_sources(..., parent_dir)`; tamper is E200; `render_report_schema_is_evaluation_report_v0_1`; `mill_pasted_run_is_unauthenticated_evaluation_report` | In-memory `compile_source` / mill paste is not byte-verified (`sourceTrust: unauthenticated`). Mill must not gain filesystem from paste. |
| Cross-feature programs | Landed | `late-payment.fr` / `late-payment-extended.fr` / `require-gate.fr` duty_status lifecycle | Not closed through mill explore vs run as one envelope. |
| Packages | Partial | `nested_package_authenticates_and_links`; `matching_package_links_always_true_so_query_type_checks`; mill/`check_source` does not read `packages/` | Path compile authenticates nested `packages/` deps (depth 8; cycle or missing nested digest is E200) and merges unique decls (`always_true` from `Logic.True` via `Std.Core`). No lockfile language in `.fr`, no network store. Mill does not mill nested packages. |
| Salsa | Landed | `fidryn_driver_depends_on_salsa_and_repeated_check_source_hits`; `same_driver_repeated_check_reports_hit` | Driver check/run memos are salsa 0.28 tracked functions. Hits/misses count salsa reuse. Not a whole-compiler query graph. |
| SMT | Partial | `smt_check`; `verify_property_proves_forall_true_over_people` | Finite-domain equality SMT-lite in `fidryn-solve`. Not Z3; no bitvectors; `TrusteeContinuity` stays Unknown. |
| Independent kernel without `evaluate` | Partial | kernel `eval_fragment` / `pure.rs` / `duty.rs`; `1+1`, `if`, `seq(require true, 7)`, top-level `duty_status` covering | Closed value and top-level duty/require_authority covering do not call `evaluate`. UniqueOccupant, Observe, `duty_step`, and nested handler plans still use the trusted evaluator. |

Trust profiles `Fixture`, `ByteVerified`, `PolicyAccepted`, and
`Unauthenticated` exist on `TrustProfile`. Evaluation reports carry a
profile; fixture execution must not be presented as byte-verified.

## Prior review matrix

| Obligation | Where | Tests | Remaining limit |
| --- | --- | --- | --- |
| RunDecision executes the referenced declaration | `eval_run_decision`; FOIA-by-name removed; unknown name is `EngineError::Unsupported` | `process_responsive_record_executes_its_declared_body`; `run_decision_does_not_silently_run_foia`; `independent_process_responsive_record_is_not_foia`; arity `run_decision_rejects_argument_arity_mismatch` | Bodies are `record requires` schemas plus a result ident, not full `when` trees. Options/authority are not yet a general interpreter. |
| UniqueOccupant uses alternative *definitions* | `eval_succession` + `CoreInterpretationFamily`; families matched by `Eligible` office, not I1/I2 aliases | `i1_with_both_accepted_selects_highest_rank_not_the_label`; `i1_with_only_bob_accepted_selects_bob`; `changing_i1_definitions_without_renaming_changes_the_result`; `renamed_alternatives_keep_eligibility_results`; `third_nominee_can_win_from_declared_eligibility`; `two_offices_follow_distinct_succession_families`; `office_without_succession_family_does_not_rank_accepted_nominees`; independent programs under `tests/programs/` | An office with no family for that office is Suspended, not min-rank of accepted nominees. |
| Determinacy: two answers disprove; incomplete is unknown | `check_determinacy` + `Coverage` | existing counterexample/convergent tests; unresolved requests outside declared domains → Unknown | No covering certificate. `Coverage` is an examination count, not `CoverageWitness`. Counterexample coverage does not enumerate every possible answer. |
| Resume does not re-ask answered handler work | `RememberingHandler`; same-snapshot resume reuses full request keys; case-identity change discards `answered` | `resume_does_not_replay_answered_observe`; `snapshot_change_does_not_retain_unvalidated_old_answers` | Residual is still the full term/plan. Answering a pending request is not a separate pinned-snapshot API from case updates. |
| Cache keys include query arguments | `Driver::run_with_args`; fuel exhaustion is not stored | `different_query_arguments_miss_the_run_cache`; `changed_import_digest_misses_check_cache`; `same_driver_repeated_check_reports_hit` | Handler/profile are not separate key fields (CaseFile is derived from the case). Whole-source hashing; no declaration-level invalidation. |
| Parameterized modules | `instantiate` assigns `Name<Args>` identity | arity E210; nested `Option<T>`; `instantiate_twice_isolates_nominal_identity` (identical args share name/id; different args do not) | No capture-avoiding substitution in terms. Instantiated bodies are not re-checked as a separate module calculus. |
| Source round-trip is not a parse proof | Rowan `syntax().text()` plus Pratt/HIR | `parses_mul_tighter_than_add`; `parses_subtraction_left_associative`; `parse_expr_mul_tighter_than_add`; `malformed_calc_body_is_error_and_later_entity_still_parses` | Not every Pratt node is a CST node. |
| Verification evaluates properties | `verify_property` | `verify_property_proves_true_literal`; `verify_property_counterexample_for_false_literal`; empty-bounds Unknown; TrusteeContinuity is not auto-proved | Only exact `true`/`false` literals are decided. Quantified formulas remain Unknown. |
| Frontends share semantics | CLI `compile_module` thread-local Driver; driver/eval | CLI smoke + driver run cache | Server/session vs CLI agreement is not a dedicated conformance suite. |
| Evidence and authority scoping | Observe schema+subject+time; designated subject fields | `evidence_does_not_match_subject_by_incidental_issuer_field`; existing Observe tests | Wrong-authority matrix is still Partial. Grant calculus is Remaining (§3). |
| Source authentication | check imports | `import_not_satisfied_by_unrelated_digest`; `digest_abc_does_not_authenticate_required_import` | `"fixture"` is a test policy. Tampered bytes after load are not a live rehash of every import file. See §1 artifact-byte authentication. |

The four crate-level items **fabricated-witness**, **source-duty**, **nested seq**,
and **CLI byte-auth** are implemented. This suite does not reimplement them.
Remaining after that gate: Z3, UniqueOccupant covering without `evaluate`,
a full `apps/` crate reorg, and a network package store. Mill must not
gain filesystem from paste. The 2026-09-17 review stays **open**. Green
tests are regressions, not language closure.

Commands: `cargo test --workspace --offline` and `cargo clippy --workspace -- -D warnings`.
`cargo test -p fidryn-integration-tests --offline` is the cross-crate integration crate.
`cargo test -p fidryn-integration-tests --offline adversarial_regressions` is the 16-test kit.
`cargo test -p fidryn-integration-tests --offline integration_suite` is the integration gate.
`cargo test -p fidryn-integration-tests --offline boundary_review_20260917` is the 2026-09-17 11:35 boundary-review gate.
`cargo test -p fidryn-integration-tests --offline review_regressions` is the 2026-09-17 14:45 review gate.

## 2026-09-17 11:35 boundary review

The review stays **open**. Green tests are regressions, not closure.
This gate is `tests/integration/boundary_review_20260917.rs`
(thirteen tests). Schema probes live at
`conformance/schema_boundary_probes.py`.

| Test | Contract |
| --- | --- |
| `caller_supplied_answers_cannot_mint_finite_replay_without_replay` | A witness with invented answers cannot manufacture a replay-verified capability. |
| `replay_cannot_overwrite_a_fixed_case_fact` | Completions resolve declared unknown slots; they do not replace fixed evidence/facts. |
| `matching_branch_count_does_not_replace_domain_membership` | Equal counts and distinct maps do not establish exact coverage. |
| `replay_rejects_a_different_program_identity` | Bound claims identify the program actually replayed. |
| `a_real_certificate_for_true_cannot_certify_false` | Certificate consumption checks the certified answer, not just its method tag. |
| `future_determinations_are_not_visible_at_an_earlier_known_time` | Every determination-reading path respects knowledge time. |
| `on_time_payment_remains_unbreached_after_its_deadline` | Later observation does not turn timely performance into historical breach. |
| `relabeling_a_performed_payload_as_correction_does_not_authorize_it` | Event kind changes cannot bypass semantic admission. |
| `rolled_back_nested_sequence_is_reexecuted_before_transaction_commit` | Aborting a transaction rolls back its progress markers as well as bindings. |
| `rebase_cannot_reuse_a_require_that_is_now_false` | Changed dependencies invalidate completed guards on rebase, or rebase is rejected. |
| `sequencing_does_not_hide_a_wrong_result_type` | A surrounding sequence does not conceal a wrong result type. |
| `a_declared_function_result_does_not_replace_checking_its_body` | Function bodies must satisfy their declared results. |
| `a_hex_manifest_entry_without_checked_bytes_is_not_byte_verified` | ByteVerified requires checked bytes, not a plausible hex label. |

## 2026-09-17 14:45 review

The review stays **open**. Green tests are regressions, not closure.
This gate is [`tests/integration/review_regressions.rs`](../tests/integration/review_regressions.rs)
(ten semantic tests). Command:
`cargo test -p fidryn-integration-tests --offline review_regressions`.
Do not weaken assertions.

`CheckedCertificate::issue_finite_replay` is still public on core. The
same file asserts a fabricated answer cannot mint covering. If that
factory is removed, convert the probe to a rustdoc `compile_fail`; do
not re-expose minting.

| Test | Contract |
| --- | --- |
| `false_rule_guard_does_not_establish_a_proposition` | A false rule guard does not derive its then-consequence. |
| `otherwise_is_not_an_additional_then_consequence` | `otherwise` is not an extra `then`. |
| `declared_function_parameters_constrain_call_arguments` | Call arguments must match declared parameter types. |
| `incompatible_branches_are_not_an_inference_escape_hatch` | If-branches of incompatible types are a compile error. |
| `resuming_a_nested_sequence_without_new_evidence_stays_suspended` | Nested seq resume without evidence stays Suspended. |
| `a_rebase_runs_the_new_query_not_the_old_residual` | Rebase evaluates the new program, not the old residual. |
| `false_permission_entry_does_not_grant_authority` | `action: false` is not a grant. |
| `replay_uses_the_same_argument_precedence_as_execution` | Replay cannot certify the case fact when args win. |
| `replay_does_not_admit_a_future_determination` | Knowledge time binds replay as well as execution. |
| `function_body_changes_are_not_invisible_to_source_diff` | A function-body edit is visible to source diff. |
