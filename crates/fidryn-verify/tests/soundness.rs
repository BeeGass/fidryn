//! Production-path regressions for verification and completion admission.

use fidryn_core::{CaseRecord, CoreModule, Outcome, QueryName, RunContext, Value};
use fidryn_driver::Driver;
use fidryn_verify::{
    Determinacy, PropertyVerdict, VerificationBudget, check_determinacy, explore_query,
    explore_query_traced, verify_property, verify_property_with_budget,
};

fn compile(body: &str) -> CoreModule {
    Driver::new()
        .check_source(
            &format!("module Adversarial.Proof version \"0.1.0\" {{ {body} }}"),
            &Default::default(),
        )
        .unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:?}"))
}

fn context() -> RunContext {
    let at = fidryn_core::Instant::parse("2026-10-29T00:00:00Z").unwrap();
    RunContext::new(at, at)
}

#[test]
fn quantified_binders_are_resolved_on_both_sides() {
    for (operator, proved) in [("==", true), ("!=", false)] {
        let module = compile(&format!(
            "verify Reflexivity {{ bounds {{ persons 2 events 1 time_points 1 }} assert for_all p in People: p {operator} p }}"
        ));
        let verdict = verify_property(&module, "Reflexivity");
        assert_eq!(verdict.is_proved(), proved, "{operator}: {verdict:?}");
        if !proved {
            assert!(matches!(verdict, PropertyVerdict::Counterexample { .. }));
        }
    }
}

#[test]
fn every_assertion_contributes_to_the_property() {
    for assertions in ["assert true; assert false", "assert false; assert true"] {
        let module = compile(&format!("verify Both {{ {assertions} }}"));
        assert_eq!(module.verifications[0].assertions.len(), 2);
        assert!(matches!(
            verify_property(&module, "Both"),
            PropertyVerdict::Counterexample { .. }
        ));
    }
}

#[test]
fn unsupported_assertion_cannot_be_dropped_from_a_true_property() {
    let module = compile("verify Mixed { assert true; assert always occupied(Trustee) }");
    assert!(matches!(
        verify_property(&module, "Mixed"),
        PropertyVerdict::Unknown { .. }
    ));
}

#[test]
fn huge_verification_domains_refuse_before_allocation() {
    let module = compile(
        "verify Huge { bounds { persons 1000000000 events 0 time_points 0 } assert for_all p in People: true }",
    );
    let verdict = verify_property(&module, "Huge");
    assert!(
        matches!(verdict, PropertyVerdict::Unknown { .. }),
        "{verdict:?}"
    );
}

#[test]
fn verification_step_budget_cannot_be_mistaken_for_a_proof() {
    let module = compile(
        "verify Bounded { bounds { persons 3 events 1 time_points 1 } assert for_all p in People: p == p }",
    );
    let verdict = verify_property_with_budget(
        &module,
        "Bounded",
        VerificationBudget {
            max_steps: 2,
            ..Default::default()
        },
    );
    assert!(
        matches!(verdict, PropertyVerdict::Unknown { .. }),
        "{verdict:?}"
    );
}

fn choice_module() -> CoreModule {
    compile(
        "decision D() -> String {} query q() -> String ! {Determine} { goal RunDecision { decision D } }",
    )
}

#[test]
fn completion_labels_cannot_collapse_distinct_worlds() {
    let module = choice_module();
    let mut case = CaseRecord::default();
    case.admissible_completions
        .choices
        .insert("D".into(), vec!["X,c:b=Y".into(), "X".into()]);
    case.admissible_completions
        .choices
        .insert("b".into(), vec!["Z".into(), "Y,c:b=Z".into()]);
    let result = explore_query(&module, &QueryName::from("q"), &case, &context()).unwrap();
    let Outcome::Contingent { alternatives, .. } = result else {
        panic!("{result:?}")
    };
    assert_eq!(
        alternatives.len(),
        4,
        "every admitted assignment must survive"
    );
    assert_eq!(
        alternatives
            .values()
            .filter(|value| **value == Value::String("X".into()))
            .count(),
        2
    );
}

#[test]
fn early_counterexamples_disclose_unexamined_worlds() {
    let module = choice_module();
    let mut case = CaseRecord::default();
    case.admissible_completions
        .choices
        .insert("D".into(), ["A", "B", "C", "D"].map(str::to_owned).to_vec());
    let result = check_determinacy(&module, &QueryName::from("q"), &case, &context()).unwrap();
    let Determinacy::Counterexample { coverage, .. } = result else {
        panic!("{result:?}")
    };
    assert_eq!(coverage.total, 4);
    assert_eq!(coverage.examined, 2);
    assert!(coverage.incomplete);
}

#[test]
fn duplicate_domain_values_do_not_consume_distinct_world_budget() {
    let module = choice_module();
    let mut case = CaseRecord::default();
    case.admissible_completions
        .choices
        .insert("D".into(), vec!["A".into(); 4100]);
    let result = check_determinacy(&module, &QueryName::from("q"), &case, &context()).unwrap();
    let Determinacy::Convergent { value, coverage } = result else {
        panic!("{result:?}")
    };
    assert_eq!(value, Value::String("A".into()));
    assert_eq!(coverage.examined, 1);
    assert_eq!(coverage.total, 1);
    assert!(!coverage.incomplete);
}

#[test]
fn off_list_recorded_choices_are_outside_competence() {
    let module = choice_module();
    let mut case = CaseRecord::default();
    case.admissible_completions
        .choices
        .insert("D".into(), vec!["A".into()]);
    case.decisions.insert("D".into(), "B".into());
    let result = explore_query(&module, &QueryName::from("q"), &case, &context()).unwrap();
    assert!(
        matches!(result, Outcome::OutsideCompetence { .. }),
        "{result:?}"
    );
}

#[test]
fn exploration_retains_real_branch_graphs_and_qualifies_exact_evidence() {
    use fidryn_core::{EvaluationReport, ExecutionMode, ExecutionOperation, ExecutionRequest};
    use fidryn_kernel::CheckedEvaluation;
    use std::collections::{BTreeMap, BTreeSet};

    let module = choice_module();
    let mut case = CaseRecord::default();
    case.admissible_completions
        .choices
        .insert("D".into(), vec!["A".into(), "B".into()]);
    let query = QueryName::from("q");
    let ctx = context();
    let session = explore_query_traced(&module, &query, &case, &ctx).unwrap();
    assert_eq!(session.coverage.examined, 2);
    assert_eq!(session.coverage.total, 2);
    assert!(!session.coverage.incomplete);
    assert_eq!(session.evidence.branches.len(), 2);
    let mut ids = BTreeSet::new();
    for step in &session.derivation {
        assert!(
            step.parents.iter().all(|parent| ids.contains(parent)),
            "missing parent in {step:?}"
        );
        assert!(ids.insert(step.id.clone()), "duplicate graph node");
    }
    assert!(ids.contains(&session.outcome.trace().hex()));
    assert_eq!(
        session
            .derivation
            .iter()
            .filter(|step| step.kind == "decision")
            .count(),
        2
    );
    assert_eq!(
        session
            .derivation
            .iter()
            .filter(|step| step.kind == "completion_assignment")
            .count(),
        2
    );
    let mut report = EvaluationReport::from_outcome(session.outcome.clone());
    let args = BTreeMap::new();
    let mut request =
        ExecutionRequest::from_eval(&module, "q", &args, &case, &ctx, ExecutionMode::Operative)
            .unwrap();
    request.operation = ExecutionOperation::Explore;
    report.request = Some(request);
    let validate = |evidence, candidate| {
        CheckedEvaluation::validate_exploration(
            &module,
            &query,
            &case,
            &ctx,
            &args,
            ExecutionMode::Operative,
            None,
            evidence,
            candidate,
        )
    };
    let checked = validate(&session.evidence, report.clone()).unwrap();
    assert_eq!(checked.exploration_evidence(), Some(&session.evidence));
    let mut forged = session.evidence.clone();
    forged.branches[0].outcome = forged.branches[1].outcome.clone();
    assert!(validate(&forged, report.clone()).is_err());
    let mut duplicate = session.evidence.clone();
    duplicate.branches[0] = duplicate.branches[1].clone();
    assert!(validate(&duplicate, report.clone()).is_err());
    let mut missing = session.evidence.clone();
    missing.branches.pop();
    assert!(validate(&missing, report.clone()).is_err());
    let mut wrong_aggregate = report.clone();
    wrong_aggregate.outcome = Outcome::Determinate {
        value: Value::Bool(true),
        trace: report.outcome.trace(),
        convergence_certificate: None,
        ignored_open_issues: BTreeSet::new(),
    };
    wrong_aggregate.unresolved.clear();
    assert!(validate(&session.evidence, wrong_aggregate).is_err());
    assert!(
        CheckedEvaluation::validate(
            &module,
            &query,
            &case,
            &ctx,
            &args,
            ExecutionMode::Operative,
            None,
            report
        )
        .is_err()
    );
}

#[test]
fn partial_exploration_evidence_cannot_qualify_a_determinate_answer() {
    use fidryn_core::{EvaluationReport, ExecutionMode, ExecutionOperation, ExecutionRequest};
    use fidryn_kernel::CheckedEvaluation;
    use std::collections::{BTreeMap, BTreeSet};
    let module = compile("query q() -> Bool { return true }");
    let mut case = CaseRecord::default();
    case.admissible_completions
        .choices
        .insert("D".into(), vec!["A".into(), "B".into()]);
    let query = QueryName::from("q");
    let ctx = context();
    let session = explore_query_traced(&module, &query, &case, &ctx).unwrap();
    let mut evidence = session.evidence;
    evidence.branches.pop();
    evidence.examined = 1;
    evidence.incomplete = true;
    let outcomes = evidence
        .branches
        .iter()
        .map(|branch| {
            (
                fidryn_core::canonical_json(&branch.bindings).unwrap(),
                branch.outcome.clone(),
            )
        })
        .collect();
    let outcome = fidryn_handlers::aggregate_completions(outcomes, true).unwrap();
    assert!(matches!(outcome, Outcome::Suspended { .. }));
    let args = BTreeMap::new();
    let mut request =
        ExecutionRequest::from_eval(&module, "q", &args, &case, &ctx, ExecutionMode::Operative)
            .unwrap();
    request.operation = ExecutionOperation::Explore;
    let mut report = EvaluationReport::from_outcome(outcome);
    report.request = Some(request);
    let validate = |candidate| {
        CheckedEvaluation::validate_exploration(
            &module,
            &query,
            &case,
            &ctx,
            &args,
            ExecutionMode::Operative,
            None,
            &evidence,
            candidate,
        )
    };
    validate(report.clone()).unwrap();
    report.outcome = Outcome::Determinate {
        value: Value::Bool(true),
        trace: report.provenance_root,
        convergence_certificate: None,
        ignored_open_issues: BTreeSet::new(),
    };
    report.unresolved.clear();
    assert!(validate(report).is_err());
}

#[test]
fn exploration_graph_byte_exhaustion_is_an_engine_error() {
    let text = "x".repeat(64 * 1024);
    let module = compile(&format!("query q() -> String {{ return \"{text}\" }}"));
    let mut case = CaseRecord::default();
    case.admissible_completions.choices.insert(
        "D".into(),
        (0..150).map(|index| format!("choice-{index}")).collect(),
    );
    let result = explore_query_traced(&module, &QueryName::from("q"), &case, &context());
    assert!(
        matches!(result, Err(fidryn_core::EngineError::FuelExhausted { .. })),
        "an oversized merged derivation must not produce a legal result"
    );
}
