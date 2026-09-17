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
`crates/fidryn-cli/tests/adversarial_regressions.rs`. Those 16 tests
pass. That is a regression suite, not a proof of soundness. Covering
certificates and artifact-byte authentication now have named tests.
A claims digest is still not covering. A `fixture` digest is still not
byte-verified.

| Obligation | Status | Tests | Remaining limit |
| --- | --- | --- | --- |
| Require preservation | Landed | `requirement_is_not_discarded_from_query`; `requirement_is_not_discarded_from_evaluate_goal`; `goal_result_type_is_checked` | Query/Evaluate `require` lowers to `seq(require, …)`. Rule-body `require` and `using` names are not stored. False require suspends rather than a dedicated failed-guard outcome. The complete `require` contract is §2 (not this row). |
| Handler identity | Landed | `remembered_judgments_distinguish_subjects_in_one_run`; `snapshot_change_does_not_retain_unvalidated_old_answers` | Keys are `Debug` of the full `OpenRequest`. Case-identity change discards `answered`. Residual is still the full term/plan, not a call-frame continuation. |
| Succession aggregation | Landed | eval unit tests for suspended alternatives; UniqueOccupant tests | UniqueOccupant is still a specialized plan. Offices without a family still rank accepted nominees only. |
| Decimal wire | Landed | `decimal_case_round_trip_preserves_nominal_variant`; `decimal_case_and_text_case_have_distinct_semantic_encodings`; `cached_case_result_agrees_with_uncached_result` | Canonical case JSON uses tagged `{"kind":"decimal","data":"1.25"}`. Friendly bare strings stay strings. RFC 8785 conformance is not claimed. |
| Effects | Landed | `automatic_queries_cannot_hide_transitive_effects`; `evaluate_int_as_bool_query_is_e210` | Recursive Γ ⊢ e : τ ! ε is not a full inference engine. Money vs Decimal remains compatible for tax closed forms. |
| Evidence subject | Landed | `evidence_does_not_match_subject_by_incidental_issuer_field` | Designated fields are subject/person/candidate/occupant/holder. `CaseDetermination.recorded_at` is optional. |
| Worklist deltas | Landed | `worklist_replacement_with_equal_cardinality_is_not_quiescence` | Truncated substitutions are an engine error, not a covering certificate. |
| Digest / program identity | Landed | `module_body_edit_invalidates_execution_cache`; `declared_empty_completion_domain_cannot_be_reopened_by_recorded_selection` | Run key hashes canonical CoreModule JSON. `ModuleId` is still name-derived. |
| Artifact-byte authentication | Landed | `matching_blake3_hex_authenticates_required_import`; `mismatched_blake3_hex_is_e200`; `hex_digest_without_bytes_is_e200`; `fixture_digest_authenticates_required_import` | `"fixture"` is `TrustProfile::Fixture`, never `ByteVerified`. Hex without a readable file is E200. |
| Verify pipeline (named literals) | Landed | `declared_true_property_survives_the_entire_compiler_pipeline` | Named `verify Trivial { assert true }` lowers. Only exact `true`/`false` literals are decided. |
| Certificate covering | Landed | kernel `test_accept_covering_with_complete_witness_returns_covering_certificate`; `test_reject_digest_as_covering_with_claims_digest_returns_true`; core `Outcome::determinate` rejects digest-only certs with ignored issues | `verified` remains a claims digest (`is_covering() == false`). Ignoring issues requires `verified_covering` plus a complete `CoverageWitness`. Kernel does not generate proofs. |
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
| `seq` / `require` Core eval | Landed | `independent_program_require_true_is_determinate_seven`; `independent_program_require_false_is_not_determinate_seven`; eval seq/require unit tests | Rule-body `require` is not stored. Nested seq inside a non-seq residual still uses RememberingHandler. |
| Tax builtin | Landed | eval: missing-body helper is only `ordinary_income_tax`; other missing bodies are `Unsupported` | Closed-form `.fr` calc still used when a body exists. |
| Records vs tagged values | Landed | `{"kind":"bool","data":false}` is Bool; `{"kind":"record","data":{…}}` is Map | Untagged objects still become `Value::Map`. RFC 8785 is not claimed. |

### 3. Institutional state

| Item | Status | Evidence | Remaining |
| --- | --- | --- | --- |
| Duty status machine | Landed | eval `duty_step` tests: late perform keeps `breached`; illegal discharge commits nothing | Surface `duty` declarations are not yet the eval state machine. History is in bindings/`duty:{name}`, not a full event ledger query API. |
| Authority grants | Landed | `AuthorityGrant` + `covers`; eval `require_authority` suspends without a grant | Occupancy is still a separate UniqueOccupant path. No full delegation/revocation language. |

### 4. Reasoning / proofs

| Item | Status | Evidence | Remaining |
| --- | --- | --- | --- |
| Covering certificates | Landed | `fidryn-kernel` `accept_covering`; `reject_digest_as_covering`; `Outcome::determinate` requires `is_covering()` when issues are ignored | Kernel does not re-run the evaluator. A complete witness is supplied by the caller. Digest is not covering. |
| Streaming search | Landed | `stream_budget_one_on_two_by_two_exceeds_without_full_product`; `budget_exhaustion_is_unknown_not_convergent`; `counterexample_returns_before_remaining_space` | `enumerate` still collects a stream with a huge budget for existing tests. No SMT backend. |
| Finite quantifiers | Partial | `for_all_over_closed_positive_set_is_true`; `for_all_open_ident_domain_without_closure_suspends` | Nested quantifiers and open-world proofs are not done. Quantifiers as a general language (not only `Term::Apply` over a closed `Value::Set`) remain incomplete. |

A trusted evaluator may establish covering by exhaustive finite search.
`fidryn-kernel` only accepts or rejects a covering claim.
`fidryn-verify` / `fidryn-solve` generate search; they do not make a
digest covering.

### 5. Application / packages

| Item | Status | Evidence | Remaining |
| --- | --- | --- | --- |
| `fidryn-kernel` crate split | Landed | 8 kernel tests; covering vs digest | Not a proof generator. Does not re-execute the program. |
| `fidryn-driver` crate split | Landed | `run_report`; `function_body_edit_invalidates_execution_cache`; `program_digest` in run key | Handler/profile are not separate key fields. No declaration-level invalidation. |
| Cross-feature programs | Landed | `tests/programs/late-payment.fr`; `tests/programs/require-gate.fr` | Late-payment does not yet run duty breach history through CLI/server/explore together. |
| Packages | Remaining | none | No package language, lock, or authenticated package digest. |

Trust profiles `Fixture`, `ByteVerified`, `PolicyAccepted`, and
`Unauthenticated` exist on `TrustProfile`. Evaluation reports carry a
profile; fixture execution must not be presented as byte-verified.

## Prior review matrix

| Obligation | Where | Tests | Remaining limit |
| --- | --- | --- | --- |
| RunDecision executes the referenced declaration | `eval_run_decision`; FOIA-by-name removed; unknown name is `EngineError::Unsupported` | `process_responsive_record_executes_its_declared_body`; `run_decision_does_not_silently_run_foia`; `independent_process_responsive_record_is_not_foia`; arity `run_decision_rejects_argument_arity_mismatch` | Bodies are `record requires` schemas plus a result ident, not full `when` trees. Options/authority are not yet a general interpreter. |
| UniqueOccupant uses alternative *definitions* | `eval_succession` + `CoreInterpretationFamily`; families matched by `Eligible` office, not I1/I2 aliases | `i1_with_both_accepted_selects_highest_rank_not_the_label`; `i1_with_only_bob_accepted_selects_bob`; `changing_i1_definitions_without_renaming_changes_the_result`; `renamed_alternatives_keep_eligibility_results`; `third_nominee_can_win_from_declared_eligibility`; `two_offices_follow_distinct_succession_families`; independent programs under `tests/programs/` | Offices without a family still rank accepted nominees only. |
| Determinacy: two answers disprove; incomplete is unknown | `check_determinacy` + `Coverage` | existing counterexample/convergent tests; unresolved requests outside declared domains → Unknown | No covering certificate. `Coverage` is an examination count, not `CoverageWitness`. Counterexample coverage does not enumerate every possible answer. |
| Resume does not re-ask answered handler work | `RememberingHandler`; same-snapshot resume reuses full request keys; case-identity change discards `answered` | `resume_does_not_replay_answered_observe`; `snapshot_change_does_not_retain_unvalidated_old_answers` | Residual is still the full term/plan. Answering a pending request is not a separate pinned-snapshot API from case updates. |
| Cache keys include query arguments | `Driver::run_with_args`; fuel exhaustion is not stored | `different_query_arguments_miss_the_run_cache`; `changed_import_digest_misses_check_cache`; `same_driver_repeated_check_reports_hit` | Handler/profile are not separate key fields (CaseFile is derived from the case). Whole-source hashing; no declaration-level invalidation. |
| Parameterized modules | `instantiate` assigns `Name<Args>` identity | arity E210; nested `Option<T>`; `instantiate_twice_isolates_nominal_identity` (identical args share name/id; different args do not) | No capture-avoiding substitution in terms. Instantiated bodies are not re-checked as a separate module calculus. |
| Source round-trip is not a parse proof | Rowan `syntax().text()` plus Pratt/HIR | `parses_mul_tighter_than_add`; `parses_subtraction_left_associative`; `parse_expr_mul_tighter_than_add`; `malformed_calc_body_is_error_and_later_entity_still_parses` | Not every Pratt node is a CST node. |
| Verification evaluates properties | `verify_property` | `verify_property_proves_true_literal`; `verify_property_counterexample_for_false_literal`; empty-bounds Unknown; TrusteeContinuity is not auto-proved | Only exact `true`/`false` literals are decided. Quantified formulas remain Unknown. |
| Frontends share semantics | CLI `compile_module` thread-local Driver; driver/eval | CLI smoke + driver run cache | Server/session vs CLI agreement is not a dedicated conformance suite. |
| Evidence and authority scoping | Observe schema+subject+time; designated subject fields | `evidence_does_not_match_subject_by_incidental_issuer_field`; existing Observe tests | Wrong-authority matrix is still Partial. Grant calculus is Remaining (§3). |
| Source authentication | check imports | `import_not_satisfied_by_unrelated_digest`; `digest_abc_does_not_authenticate_required_import` | `"fixture"` is a test policy. Tampered bytes after load are not a live rehash of every import file. See §1 artifact-byte authentication. |

Commands: `cargo test --workspace --offline` and `cargo clippy --workspace -- -D warnings`.
`cargo test -p fidryn-cli --test adversarial_regressions` is the 16-test kit.
