# Proof, completion, report and review implementation audit

Scope: the shared Fidryn checkout on 2026-10-29. This records implemented boundaries and exact regressions, not completion of the language's general legal semantics or an independent source-to-Core proof.

## Implemented behavior

- `CoreVerify.assertions` preserves all source assertions as structured `Term` values. The property evaluator resolves bound variables on both sides of comparisons, respects lexical scope, checks the conjunction, and returns Unknown for unsupported operations, empty quantifier bounds, excessive domains, depth or operation exhaustion. Default verification budget: 65,536 steps, 4,096 domain elements, depth 128. No SMT/temporal theorem is inferred from a property name or a matching string fragment.
- `fidryn_core::ValidatedCompletionModel` is the shared normalization/admission path for execution, search and kernel replay. Declared empty domains refuse; recorded off-list selections refuse; recorded valid choices narrow domains; duplicate labels normalize before counting. Assignment identities use canonical typed JSON, not delimiter concatenation. Search examines at most 4,096 assignments and reports the actual examined/total/incomplete counters.
- `CompletionSearchEvidence` retains each executed binding map and full outcome. `explore_query_traced` retains actual branch derivations and content-addressed assignment/search roots. It rejects missing roots, duplicate node/world identities, incomplete determinate aggregation and oversized graphs. Merged trace limits are 100,000 nodes, 1 MiB per detail and 16 MiB retained bytes. Nested contingent outcomes remain contingent with their pivots even when their displayed answers agree.
- Covering replay preserves Bool/String/Entity distinctions. Domain labels are not injected as judgment facts. Kernel admission uses normalized/narrowed completion cardinality and exact membership rather than raw vector lengths. Pure/institutional shortcuts validate query input admission and global institutional consistency; independent fragment construction has default operation/depth limits.
- Core certificate objects are explicitly claims. `unverified_replay_claim` does not verify execution. The compatibility `kernel-issue` Cargo feature grants no authority. Sealed finite-replay consumption binds the complete program fingerprint, case identity, query, arguments, clocks, execution mode, operation, semantics version and budget. Actual checked report qualification is a kernel-owned private `CheckedEvaluation` handle.
- Direct run report qualification reruns the admitted invocation. Budgeted requests consume their recorded operation budget through bounded runtime APIs. Exploration qualification validates exact branch membership and coverage, reruns each branch, and reconstructs the aggregate. Both reject changed trust/source bundle/hypotheses/unresolved metadata/provenance. Qualification of finite covering claims additionally rechecks their branch witness.
- `Value::Set` equality is extensional and its serialized identity sorts and deduplicates canonical elements, including nested sets. Runtime variants remain distinct. Shared value digest normalization additionally canonicalizes decimal scales.
- Explicit monetary constructors retain currencies through same-currency addition/subtraction/comparison, scalar multiplication (both orders), and scalar division. Cross-currency operations, illegal units, numeric String substitution, overflow and division by zero refuse. Source tax examples and case fixtures now use typed USD quantities instead of helper-injected arithmetic or parsed money strings.
- Acceptance tests compile actual repository `.fr` files through the Driver. They no longer rewrite compiled Core, inject functions/clauses/manifests, rewrite calls, or synthesize incumbent state. Trust fixtures provide explicit scoped occupancy/acceptance and physician metadata; the fresh August 2026 source policy is exercised alongside a stale March 2034 refusal. Other case fixtures provide actual typed request, proceeding, record and clause inputs.
- `fidryn_cli::review::build_review` exports a fresh portable JSON review archive, not an imported report upgrade. It contains exact frozen root/dependency/package/lock bytes and hashes, missing observations, exact evaluated case bytes, a clearly labeled normalized semantic manifest, immutable source observations, the checked v0.2 report and actual trace, source declaration spans/text, an exact executable fingerprint and a rerun argument vector. Source/case identities and archive contents can be restored to reproduce the same invocation and source-bundle identities. Case/template reads are bounded to 8 MiB; frozen archive inputs are bounded to 64 MiB; report/trace serialization has a 64 MiB bound. Templates are attached reviewer material; review status starts `not_assessed` and remains separate from automatic checks.

## Exact regression evidence

`crates/fidryn-verify/tests/soundness.rs`:

- `quantified_binders_are_resolved_on_both_sides`
- `every_assertion_contributes_to_the_property`
- `unsupported_assertion_cannot_be_dropped_from_a_true_property`
- `huge_verification_domains_refuse_before_allocation`
- `verification_step_budget_cannot_be_mistaken_for_a_proof`
- `completion_labels_cannot_collapse_distinct_worlds`
- `early_counterexamples_disclose_unexamined_worlds`
- `duplicate_domain_values_do_not_consume_distinct_world_budget`
- `off_list_recorded_choices_are_outside_competence`
- `exploration_retains_real_branch_graphs_and_qualifies_exact_evidence`
- `partial_exploration_evidence_cannot_qualify_a_determinate_answer`
- `exploration_graph_byte_exhaustion_is_an_engine_error`

`crates/fidryn-kernel/tests/covering_soundness.rs`:

- `source_string_values_cannot_be_certified_as_booleans`
- `source_value_variants_agree_between_execution_and_covering`
- `raw_replay_stamp_cannot_replace_kernel_report_validation`
- `certificate_consumption_rejects_each_changed_invocation_component`
- `a_matching_run_envelope_cannot_qualify_a_fabricated_direct_answer`
- `covering_admission_uses_normalized_and_recorded_completion_worlds`
- `pure_covering_cannot_bypass_typed_query_argument_admission`
- `literal_covering_cannot_bypass_institutional_conflicts`
- `report_qualification_rejects_changed_metadata_and_source_bundle`
- `report_qualification_executes_the_claimed_operation_budget`

`crates/fidryn-core/tests/set_semantics.rs` exercises order/duplicate-independent equality and wire identity, variant distinctions, nested maps/sets and decimal scale normalization.

`crates/fidryn-eval/tests/money_soundness.rs`:

- `source_money_arithmetic_preserves_currency_and_exact_decimal_amounts`
- `runtime_money_admission_rejects_currency_and_string_substitution`
- `monetary_overflow_and_division_by_zero_are_engine_errors`

`crates/fidryn-handlers/src/exploration.rs`:

- `agreeing_answers_do_not_erase_an_unresolved_nested_contingent`

`crates/fidryn-cli/tests/review_bundle.rs`:

- `portable_review_retains_exact_inputs_graph_spans_and_reruns` also executes the archive's actual CLI argument vector after restoring files and requires identical invocation/source identities.
- `review_supports_nested_manifest_layout_and_retains_missing_observations`
- `review_scenario_and_template_do_not_become_a_legal_assessment`
- `review_refuses_invalid_case_and_insufficient_execution_budget`

`crates/fidryn-cli/src/engine.rs` adds `executable_identity_is_cached_nonempty_and_has_no_local_path`: BLAKE3/byte count/package/semantics/OS/architecture identity is streamed once per process. Linux captures the running inode through `/proc/self/exe`; other platforms label the current executable file method. Review restoration requires the same captured executable identity, using a pinned test binary copy to avoid concurrent build replacement.

`crates/fidryn-verify/tests/completions.rs` and `domains.rs` use source-path production fixtures. `stale_physician_records_do_not_authorize_later_succession` explicitly tests the 90-day policy's stale refusal.

The corrected legacy integration regressions in `tests/integration/{adversarial_regressions,boundary_review_20260917,semantic_regressions}.rs` preserve their adversarial assertions with typed arguments, scoped positive payment/authority inputs and declared duties. `cached_case_result_agrees_with_uncached_result` warms a valid Decimal result and requires String input to be refused identically by warm/cold execution. `worklist_replacement_with_equal_cardinality_is_not_quiescence` now compiles real source rules instead of adding Core rules after compilation. The integration target passed all 85 tests after these corrections.

## Validation commands and local evidence

All local Cargo invocations used `CARGO_TARGET_DIR=/cache/cargo/target` and `--offline`.

- `cargo test -p fidryn-core -p fidryn-kernel -p fidryn-verify -p fidryn-solve --offline`: `/cache/tmp/fidryn-proof-full.log` (core120unit+3case-admission+3set; kernel65unit+10source; solve21unit; verify30unit+8completions+28domains+11source at that run, with the subsequently added twelfth source byte-budget regression passing separately).
- `cargo test -p fidryn-integration-tests --test integration --offline`: `/cache/tmp/fidryn-integration-proof-fixes.log` (85 passed at last completed run).
- `cargo test -p fidryn-cli --test review_bundle --offline`: `/cache/tmp/fidryn-review-tests.log` (4 passed at last completed run).
- `cargo test -p fidryn-eval --test money_soundness --offline`: `/cache/tmp/fidryn-money-regressions.log` (3 passed).
- `cargo clippy -p fidryn-core -p fidryn-kernel -p fidryn-verify -p fidryn-cli -p fidryn-integration-tests --all-targets --offline -- -D warnings`: `/cache/tmp/fidryn-proof-clippy.log` (final all-targets/all-features run clean).

## Explicit remaining boundaries

- Property verification is bounded evaluation of a supported finite expression fragment. General temporal logic, induction, arbitrary legal predicates, symbolic domain proofs and SMT/Lean certificates are not implemented by this change. Unsupported assertions remain Unknown.
- Covering checks of closed pure terms are independent term evaluation with shared admission/foundation checks. Institutional checks reuse runtime foundation/duty helpers; other plans use trusted runtime replay. This is not a general independent kernel or a proof of compiler/source translation correctness.
- Production source/IR correspondence is supplied by the Driver's private admitted compilation artifact and frozen input closure. A caller supplying arbitrary Core plus an unrelated bundle does not thereby obtain an independent translation proof.
- Custom exploration operation budgets are explicitly refused at checked report admission until the branch generator is wired to that custom budget. Current finite-replay issuance is operative Run, semantics 0.2, budget None (the bounded default). Scenario-specific covering issuance and budgeted covering issuance remain separate work; they are not silently generalized.
- The source `CoreClause` identity now preserves actual source text and typed binders. General clause-body semantic elaboration is still incomplete (`rules` remains empty where no elaboration exists); richer court findings, burden/review clauses and unsupported institutional policies must remain unresolved rather than being treated as proved or automatically valid.
- Raw solver collection utilities (`enumerate`, `search`, public `dpll`) still expose unbounded eager results. Production completion search and property evaluation use bounded paths; no global solver-resource qualification is claimed.
- A review archive is integrity-addressed, not signed authority. Replaying it needs a compatible recorded engine. Original manifest formatting is not retained; the archive explicitly supplies the admitted normalized semantic manifest. The export does not implement a legally authoritative review opinion, signed approval, workflow assignment, or an archive import/extraction trust boundary.
- Keeping source/case/trace/evidence together makes a saved review reproducible and explainable. A standalone v0.2 report without the review archive identifies its source/case by hashes but still requires those external inputs for rerun.

## Final handoff update

The complete core/kernel/solver/verifier package test command passed, including all28 production source domain fixtures and all8 trust completion fixtures. The subsequently added merged-graph byte-budget source regression also passed (12 source soundness tests total). All85 integration regressions passed. Review4 integration tests passed with the initial archive generated through the actual CLI and the restored rerun checking the same executable fingerprint; engine identity unit1 passed. Final focused Clippy across all targets and all features passed with `-D warnings`; scoped `git diff --check` is clean. Root owns the final full-workspace qualification and remote governance verification.

Read-only integration review findings and inspected root resolutions are recorded separately at `/cache/tmp/fidryn-root-readonly-review.md`. No Git branches, stage, commits, pushes or remote policy mutations were performed by this delegated agent.
