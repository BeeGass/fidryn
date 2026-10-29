# Fidryn implementation audit and evidence ledger

Baseline: eb072a296b987e3f350b0c4348c6755d95540e4b. The two audit passes
reproduced failures through the normal CLI and public compiler/kernel APIs.
The baseline full gate passed 1,011 Rust tests plus its supporting checks;
that is regression evidence, not closure of the language.

## Branch recovery

The original f918bc4 and eb072a2 commits are preserved on remote dev.
Local recovery/accidental-main-20261029 retains eb072a2. Local main points
at origin/main c5c5f45. Remote main has not been rewritten.

## Required findings and owning contracts

| Area | Reproduced failure | Required acceptance |
|---|---|---|
| Guards | invalid terms lower to satisfied | typed guards, no fail-open conversion |
| Statements | fn return drops require/effect prefix; specialized goal drops require | all prefixes execute before result |
| Continuations | sibling seq/calls share progress | location/invocation/iteration-specific frames |
| Proof values | kernel certifies Bool true for actual String true | accepted replay agrees with execution |
| Properties | p != p proved; later assertions dropped | scoped structured assertions, conjunction |
| Types | Int function returns String through open names | declared typed inputs and result preservation |
| Calls | named label invokes unrelated function | named arguments bind signatures only |
| Resolution | binder mistaken for recursive/effectful callee | lexical declaration/binder identities |
| Metadata | comments/strings change fuel and effects | AST-only semantics |
| Literals | overflow becomes zero; JSON strings not decoded | checked ranges, JSON escapes, formatter preservation |
| Collections | set/record become seq/call; set equality is list equality | distinct forms, extensional set identity |
| Declarations | duplicates overwrite; generic type swallows query; semicolon enters type | conservation, duplicate diagnostics, typed signatures |
| Generics | QName suffix instantiates wrong module; currency/calendar not substituted | exact identity, complete checked substitution |
| Worklist | circular support survives withdrawal; declaration order decides conflict | grounded supports and staged effects |
| Modal facts | derived becomes determined; protocol/prefix creates unrelated proposition | proof-kind and issue/context isolation |
| Evidence | wrong payment satisfies duty; duplicate LunchReceipt triggers succession | content/subject/issuer/identity admission |
| Authority | expired/missing grant and undeclared helper authorize transition | scoped temporal declaration-based authority |
| State | duty_status misses staged transition | one staged institutional view |
| Time | future performance/occupancy/assumption admitted; deadline disappears/overflows | shared bitemporal admission and typed deadlines |
| Worlds | run ignores empty/off-list model; labels overwrite worlds | shared validated model, typed identities |
| Coverage | examined count becomes total; duplicate model differs across subsystems | accurate cardinality and normalization |
| Resources | parser stack abort; huge verifier allocation; final binder truncated | preallocation budgets and honest incomplete/error |
| Imports | explicit digest mismatch falls back to path/fixture | digest is mandatory admission constraint |
| Sources | trust inherits cache order; nested closure missing; root escapes reread | artifact-bound complete observed dependency bundle |
| Cases | typo erases domain/exclusion; unsupported schema accepted | shared strict versioned admission |
| Mode | CLI run auto-applies hypotheses | explicit operative/scenario invocation |
| Reports | case/program identity absent; diff ignores clocks/model/coverage | checked invocation-bound v0.2 report |
| Traces | explain misses envelope; wrapper replaces actual derivation | persisted actual parented records |
| Presentation | scenario summary unconditional; string lists split names | mandatory qualifications, typed parsed rendering |
| Qualification | probes cannot fail; tests rewrite compiled Core | explicit controls and production compilation |

## Implementation evidence rules

A fix is closed only by a named regression plus applicable integration checks.
Core-only tests may isolate a machine, but source acceptance cannot repair Core.
Counterexamples, incomplete coverage, fixture trust and independent checking
remain separate qualifications. Failed legacy fixture assumptions must be
replaced with explicit declarations and evidence rather than weaker assertions.

Focused regressions and final command receipts are recorded in the worktree's
implementation changes and the final handoff. Broad professional workflows,
full capability obligations and cross-platform CI
remain active requirements until their own acceptance evidence exists.

## Implemented counterexamples and evidence

These are scoped implementation receipts. The final workspace/site/platform gate
and the broad language requirements remain separate obligations.

| Boundary | Production change | Regression evidence |
|---|---|---|
| Frontend syntax/typing | Guards fail closed; statement prefixes and named labels survive lowering; typed records/sets, explicit inputs, aliases and nominal sums | `crates/fidryn-check/tests/frontend_soundness.rs`; `crates/fidryn-syntax/tests/literal_and_structure_regressions.rs` |
| Source conservation | Structured judgment policies, observation validators, selection scope/predicate/order, duty binders and every verification assertion retained | `judgment_policies_and_every_condition_survive_source_compilation`, `observations_preserve_and_typecheck_each_validator_and_record_binding`, `source_selection_preserves_scope_predicate_and_ordering`, `every_verification_assertion_is_preserved` |
| Sources/cache identity | Immutable CompilationArtifact binds root text and complete observed package/lock inputs; mixed/failed checks cannot acquire byte trust | driver `compilation_artifact_provenance_is_independent_of_cache_order`, `nested_package_edit_agrees_on_warm_and_fresh_driver` |
| Filesystem capability | Explicit enclosing source root; manifest and artifact confinement; JSON-decoded AST headers | driver `source_root_is_explicit_and_confines_manifest_resolution`, `source_root_rejects_symlinked_artifacts`, `source_manifest_uses_decoded_headers_and_ignores_line_comments` |
| Case admission | Known case/manifest/lock versions; nested unknown fields refused; intervals validated | `crates/fidryn-core/tests/case_admission.rs`; time `interval_admission_rejects_reversed_or_impossible_bounds` |
| Report identity | Mode, program, case, clocks, operation and actual operation budget bind v0.2 qualified reports | `crates/fidryn-cli/tests/qualified_reports.rs`; kernel `crates/fidryn-kernel/tests/covering_soundness.rs` |
| Persisted actual trace | Run/exploration records form a retained graph with parent closure and invocation root; report is standalone explanation artifact | CLI `saved_report_explains_actual_graph_after_the_run_process_exits`; `conformance/probe_report_schema.py` |
| Typed templates | Parsed arrays/records, bounded work, strict missing/scalar distinctions; interpolated data is never reparsed | `crates/fidryn-render/tests/typed_templates.rs` |
| Portable review | Exact case/root/dependency bytes, normalized manifest explicitly labeled, source spans, checked report/trace and argument-vector rerun | `crates/fidryn-cli/tests/review_bundle.rs` four independent archive/qualification/refusal tests |
| Qualification controls | Probes fail on acceptance mismatch; permissive/reject-all mutation controls; actual run/scenario/explore report schema validation | `conformance/test_schema_controls.py`, `conformance/probe_report_schema.py` |

## Native GitHub governance

The user rejected the custom App/controller approach and chose native GitHub
capabilities. No App registration or private key is required. SSH authenticates
pushes as the account; commit signing is separate verification. Active rulesets
are Protect main 23613847, Main independent review 26163390, Protect dev 23613849,
and Dev contributions 26163393. Main safety and required checks have no bypass
actors; BeeGass 44324535 has an exception only to independent PR review. Main's
required Actions source check validates same-repository dev-to-main PRs. Native
checks are commit-scoped, so this is not an atomic head-branch restriction across
multiple PRs sharing a SHA. New final-candidate checks must pass before promotion.

## General requirements retained

Concrete regression fixes do not establish a full resolved symbol calculus,
complete capture-avoiding module/body substitution, every imported nominal
namespace, arbitrary generic records/sums, the complete court-finding/review DSL,
or every power/legal-act/scenario construct. Rich unsupported judgment clauses
are retained and must remain open/refused. Structured syntax preservation alone
is not execution qualification. Source-to-IR translation is still trusted:
CheckedEvaluation independently validates results for the admitted Core program;
CompilationArtifact provides the production relationship to observed source.
Source trust is byte/digest checking, not legal authority or substantive review.

The full contracts in docs/FULL-IMPLEMENTATION.md, docs/OBLIGATIONS.md and
WORKSTREAM-CONTRACT.md remain obligations. Record exact final receipts and any
unavailable platform/runtime evidence without upgrading scoped tests to closure.

## Runtime acceptance map

`crates/fidryn-eval/tests/runtime_soundness.rs` exercises the actual source path:

| Contract | Named counterexample |
|---|---|
| Grounded truth maintenance | withdrawing_the_last_foundation_retracts_a_support_cycle; incompatible_effects_are_independent_of_source_order |
| Complete bounded bindings | final_binder_truncation_is_an_error |
| Distinct dynamic continuations | sibling_sequences_and_repeated_calls_have_distinct_frames; shared_operation_allowance_and_resume_do_not_reset_work |
| Modal identity | derivation_or_protocol_alias_cannot_manufacture_a_determination; unrelated_fact_prefix_does_not_establish_a_proposition |
| Typed value/effect boundary | decision_results_execute_declared_expressions_without_fact_substitution; declared_effect_requests_bind_values_and_validate_responses |
| Scoped institutional state | undeclared_or_ungranted_duty_transitions_do_not_commit; duty_reads_observe_staged_writes; parameterized_duties_ground_typed_records_and_isolate_instances |
| Performance and authority | only_matching_occurred_performance_satisfies_the_duty; expired_and_wrong_scope_authority_does_not_grant |
| Time and occupancy | occupancy_uses_its_valid_interval_and_requires_office_scope; hours_execute_and_oversized_deadlines_do_not_panic |
| Declared policy execution | source_judgment_executes_concurrence_constraints_and_conditions; observations_validate_and_instantiate_the_exact_bound_record |
| Actual trust policy and stale refusal | actual_trust_judgment_uses_source_policy_and_refuses_stale_certificates |
| No ignored selection constraints | unsupported_source_selection_conditions_remain_explicitly_open |
| Actual bounded derivation | derivation_is_linked_to_actual_rule_execution_and_input_snapshot; legal_admission_refusals_have_stored_trace_roots; retained_derivation_bytes_exhaust_honestly |
| Timed hypotheses disclosed exactly | future_assumption_events_are_neither_applied_nor_reported |

The final runtime scoped receipt passed172 evaluator unit tests,3 currency
source tests,27 runtime source regressions,19 handler tests and85 cross-crate
integration tests. Full workspace qualification is recorded separately.

General selection execution, the complete Actor/court-finding DSL, general clause
body elaboration and new structured position materialization remain open.
Recognized succession selection is limited to the declared nomination index,
Eligible and scoped effective AcceptOffice, ascending unique nomination rank,
and closed/unique/vacancy requirements; other source predicates/orderings suspend.
Duty commit grants require wildcard context until declaration contexts are stored;
require_authority supports an explicit context and otherwise requires wildcard.
Resource defaults are100000 operations,256 evaluation depth,64 worklist rounds,
256 substitutions/record matches,100000 trace nodes,1MiB single trace detail,
and16MiB retained trace bytes. Exhaustion is an explicit engine failure.

Typed input descriptions and editor ranges are covered by
`crates/fidryn-cli/tests/input_schema.rs`. Parser refusal covers8MiB source and
262144 tokens before AST creation. Filesystem capture additionally bounds source,
case, manifest, dependency count/bytes and private snapshot permissions.

The native policy also trusts accepted workflow contents: an Actions provider id
and check name do not authenticate a workflow definition. Owner review is the
trust boundary for workflow/protection edits. Local source capture assumes a
trusted filesystem against hostile concurrent path replacement; static checks
are not descriptor-relative filesystem isolation.
