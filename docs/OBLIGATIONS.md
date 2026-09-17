# Review obligations and evidence

Workspace tests are regressions, not closure of the language. This
matrix maps the follow-up review to implementation and tests.

The next standard is that independently written Fidryn programs with
unfamiliar names control their own execution without Rust evaluator
changes. That is not yet true for every construct.

| Obligation | Where | Tests | Remaining limit |
| --- | --- | --- | --- |
| RunDecision executes the referenced declaration | `eval_run_decision`; FOIA-by-name removed; unknown name is `EngineError::Unsupported` | `process_responsive_record_executes_its_declared_body`; `run_decision_does_not_silently_run_foia`; `independent_process_responsive_record_is_not_foia`; arity `run_decision_rejects_argument_arity_mismatch` | Bodies are `record requires` schemas plus a result ident, not full `when` trees. Options/authority are not yet a general interpreter. |
| UniqueOccupant uses alternative *definitions* | `eval_succession` + `CoreInterpretationFamily`; families matched by `Eligible` office, not I1/I2 aliases | `i1_with_both_accepted_selects_highest_rank_not_the_label`; `i1_with_only_bob_accepted_selects_bob`; `changing_i1_definitions_without_renaming_changes_the_result`; `renamed_alternatives_keep_eligibility_results`; `third_nominee_can_win_from_declared_eligibility`; `two_offices_follow_distinct_succession_families`; independent programs under `tests/programs/` | Offices without a family still rank accepted nominees only. |
| Determinacy: two answers disprove; incomplete is unknown | `check_determinacy` + `Coverage` | existing counterexample/convergent tests; unresolved requests outside declared domains → Unknown | No symbolic covering proof of unexplored regions. Counterexample coverage does not enumerate every possible answer. |
| Resume does not re-ask answered handler work | `RememberingHandler`; case-identity change keeps `answered` | `resume_does_not_replay_answered_observe`; `resume_invalid_response_stays_suspended_or_halts`; `resume_after_case_determination_is_determinate` | Residual is still the full term/plan. Answering a pending request is not a separate pinned-snapshot API from case updates. |
| Cache keys include query arguments | `Driver::run_with_args`; fuel exhaustion is not stored | `different_query_arguments_miss_the_run_cache`; `changed_import_digest_misses_check_cache`; `same_driver_repeated_check_reports_hit` | Handler/profile are not separate key fields (CaseFile is derived from the case). Whole-source hashing; no declaration-level invalidation. |
| Parameterized modules | `instantiate` assigns `Name<Args>` identity | arity E210; nested `Option<T>`; `instantiate_twice_isolates_nominal_identity` (identical args share name/id; different args do not) | No capture-avoiding substitution in terms. Instantiated bodies are not re-checked as a separate module calculus. |
| Source round-trip is not a parse proof | Rowan `syntax().text()` plus Pratt/HIR | `parses_mul_tighter_than_add`; `parses_subtraction_left_associative`; `parse_expr_mul_tighter_than_add`; `malformed_calc_body_is_error_and_later_entity_still_parses` | Not every Pratt node is a CST node. |
| Verification evaluates properties | `verify_property` | `verify_property_proves_true_literal`; `verify_property_counterexample_for_false_literal`; empty-bounds Unknown; TrusteeContinuity is not auto-proved | Only exact `true`/`false` literals are decided. Quantified formulas remain Unknown. |
| Frontends share semantics | CLI `compile_module` thread-local Driver; driver/eval | CLI smoke + driver run cache | Server/session vs CLI agreement is not a dedicated conformance suite. |
| Evidence and authority scoping | Observe schema+subject+time | existing Observe tests | Wrong-authority matrix is still Partial. |
| Source authentication | check imports | `import_not_satisfied_by_unrelated_digest` | Tampered bytes after load are a digest check, not a live rehash of every import file. |

Commands: `cargo test --workspace --offline` and `cargo clippy --workspace -- -D warnings`.
