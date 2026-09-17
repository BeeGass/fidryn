//! Proposed regressions for the 2026-09-17 08:25 source export.
//! NOT compiled or executed in the review environment (Rust unavailable).
//! Copy into crates/fidryn-cli/tests/adversarial_regressions.rs.
//! These assert intended contracts and are expected to expose current defects.
//! No helper repairs compiled Core or weakens assertions on failure.

use fidryn_cli::compile_source;
use fidryn_core::{
    CaseRecord, Consequence, CoreDecl, CoreEffect, CoreModule, CoreRule, EffectId, EngineError,
    Guard, Handler, HandlerResult, Instant, NodeId, OpenRequest, Outcome, PropTerm, QueryName,
    RuleKind, RunContext, SourceManifest, SuspensionReason, Term, Value, canonical_json,
};
use fidryn_driver::Driver;
use fidryn_eval::{DerivedWorld, evaluate, evaluate_session, resume};
use fidryn_handlers::CaseFile;
use fidryn_verify::verify_property;
use std::collections::{BTreeMap, BTreeSet};

fn compile(src: &str) -> CoreModule {
    compile_source(src, &SourceManifest::default()).expect("test program must compile")
}
fn source(body: &str) -> String {
    format!("module Regression version \"0.1.0\" {{\n{body}\n}}")
}
fn context() -> RunContext {
    let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
    RunContext::new(t, t)
}
fn execute(module: &CoreModule, case: &CaseRecord) -> Result<Outcome<Value>, EngineError> {
    let ctx = context();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    evaluate(
        module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &case.into_state(),
        &ctx,
        &mut handler,
        case,
    )
}
fn answer(out: Result<Outcome<Value>, EngineError>) -> Value {
    match out.expect("execution must succeed") {
        Outcome::Determinate { value, .. } => value,
        other => panic!("expected a determinate answer: {other:?}"),
    }
}
fn decimal() -> Value {
    serde_json::from_value(serde_json::json!({"kind":"decimal", "data":"1.25"})).unwrap()
}
fn case_with(value: Value) -> CaseRecord {
    let mut case = CaseRecord::default();
    case.facts.insert("x".into(), value);
    case
}

#[test]
fn requirement_is_not_discarded_from_query() {
    let text = source("query q() -> Bool { require false; return true }");
    // Rejecting a statically false requirement is also acceptable.
    if let Ok(module) = compile_source(&text, &SourceManifest::default()) {
        assert!(!matches!(
            execute(&module, &CaseRecord::default()),
            Ok(Outcome::Determinate {
                value: Value::Bool(true),
                ..
            })
        ));
    }
}

#[test]
fn requirement_is_not_discarded_from_evaluate_goal() {
    let text = source("query q() -> Bool { goal Evaluate { require false; return true } }");
    if let Ok(module) = compile_source(&text, &SourceManifest::default()) {
        assert!(!matches!(
            execute(&module, &CaseRecord::default()),
            Ok(Outcome::Determinate {
                value: Value::Bool(true),
                ..
            })
        ));
    }
}

#[test]
fn goal_result_type_is_checked() {
    let text = source("query q() -> Bool { goal Evaluate { 7 } }");
    assert!(
        compile_source(&text, &SourceManifest::default()).is_err(),
        "Evaluate-goal expressions need the same result checking as return expressions"
    );
}

#[test]
fn automatic_queries_cannot_hide_transitive_effects() {
    let text = source(
        r#"
        proposition P(x: NaturalPerson)
        entity A : NaturalPerson
        fn f() -> Bool ! {Determine} { determined(P(A)) }
        query automatic q() -> Bool { return f() }
    "#,
    );
    assert!(
        compile_source(&text, &SourceManifest::default()).is_err(),
        "An omitted effect annotation cannot hide a called function's effect"
    );
}

#[test]
fn ordinary_string_returning_function_executes() {
    let module = compile(&source(
        r#"
        fn text() -> String { "hello" }
        query q() -> String { return text() }
    "#,
    ));
    assert_eq!(
        answer(execute(&module, &CaseRecord::default())),
        Value::String("hello".into())
    );
}

#[test]
fn function_arity_is_not_filled_from_caller_bindings() {
    let text = source(
        r#"
        fn identity(x: Int) -> Int { x }
        query q(x: Int) -> Int { return identity() }
    "#,
    );
    if let Ok(module) = compile_source(&text, &SourceManifest::default()) {
        let args = BTreeMap::from([("x".into(), Value::Int(7))]);
        let out =
            Driver::new().run_with_args(&module, "q", &CaseRecord::default(), &context(), &args);
        assert!(
            out.is_err(),
            "missing argument must not be captured from the caller: {out:?}"
        );
    }
}

#[test]
fn module_body_edit_invalidates_execution_cache() {
    let mut driver = Driver::new();
    let yes = driver
        .check_source(
            &source("query q() -> Bool { return true }"),
            &SourceManifest::default(),
        )
        .unwrap();
    let no = driver
        .check_source(
            &source("query q() -> Bool { return false }"),
            &SourceManifest::default(),
        )
        .unwrap();
    let case = CaseRecord::default();
    assert_eq!(
        answer(driver.run(&yes, "q", &case, &context())),
        Value::Bool(true)
    );
    assert_eq!(
        answer(driver.run(&no, "q", &case, &context())),
        Value::Bool(false)
    );
}

#[test]
fn decimal_case_round_trip_preserves_nominal_variant() {
    let before = case_with(decimal());
    let text = serde_json::to_string(&before).unwrap();
    let after: CaseRecord = serde_json::from_str(&text).unwrap();
    assert_eq!(before, after, "wire format lost a value's type: {text}");
}

#[test]
fn decimal_case_and_text_case_have_distinct_semantic_encodings() {
    let number = case_with(decimal());
    let text = case_with(Value::String("1.25".into()));
    assert_ne!(number, text);
    assert_ne!(
        canonical_json(&number).unwrap(),
        canonical_json(&text).unwrap(),
        "a digest input must preserve distinctions that evaluation can observe"
    );
}

#[test]
fn cached_case_result_agrees_with_uncached_result() {
    let module = compile(&source("query q() -> Decimal { return x }"));
    let mut driver = Driver::new();
    let number = case_with(decimal());
    let text = case_with(Value::String("1.25".into()));
    let _ = driver.run(&module, "q", &number, &context());
    assert_eq!(
        driver.run(&module, "q", &text, &context()),
        execute(&module, &text)
    );
}

fn pending(request: &OpenRequest) -> HandlerResult {
    HandlerResult::Suspend {
        requests: BTreeSet::from([request.clone()]),
        reason: SuspensionReason::MissingDetermination,
        trace_fragment: "test-pending".into(),
    }
}
#[derive(Default)]
struct SubjectHandler {
    calls: usize,
}
impl Handler for SubjectHandler {
    fn handle_observe(&mut self, r: &OpenRequest) -> HandlerResult {
        pending(r)
    }
    fn handle_choose(&mut self, r: &OpenRequest) -> HandlerResult {
        pending(r)
    }
    fn handle_interpret(&mut self, r: &OpenRequest) -> HandlerResult {
        pending(r)
    }
    fn handle_determine(&mut self, r: &OpenRequest) -> HandlerResult {
        self.calls += 1;
        let OpenRequest::NeedJudgment { issue, .. } = r else {
            return pending(r);
        };
        let value = issue.arguments.first() == Some(&Term::Ident("A".into()));
        HandlerResult::Resume {
            value: Value::Bool(value),
            trace_fragment: "by-subject".into(),
        }
    }
}

#[test]
fn remembered_judgments_distinguish_subjects_in_one_run() {
    let module = compile(&source(
        r#"
        proposition P(x: NaturalPerson)
        entity A : NaturalPerson
        entity B : NaturalPerson
        query q() -> Bool ! {Determine} {
            return determined(P(A)) == determined(P(B))
        }
    "#,
    ));
    let case = CaseRecord::default();
    let mut handler = SubjectHandler::default();
    let out = evaluate(
        &module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &case.into_state(),
        &context(),
        &mut handler,
        &case,
    );
    assert_eq!(handler.calls, 2, "second subject must reach the handler");
    assert_eq!(answer(out), Value::Bool(false));
}

struct PhaseHandler {
    changed: bool,
}
impl Handler for PhaseHandler {
    fn handle_observe(&mut self, r: &OpenRequest) -> HandlerResult {
        pending(r)
    }
    fn handle_choose(&mut self, r: &OpenRequest) -> HandlerResult {
        pending(r)
    }
    fn handle_interpret(&mut self, r: &OpenRequest) -> HandlerResult {
        pending(r)
    }
    fn handle_determine(&mut self, r: &OpenRequest) -> HandlerResult {
        let OpenRequest::NeedJudgment { issue, .. } = r else {
            return pending(r);
        };
        let value = match issue.predicate.as_str() {
            "P" => !self.changed,
            "Q" if self.changed => true,
            _ => return pending(r),
        };
        HandlerResult::Resume {
            value: Value::Bool(value),
            trace_fragment: "phase".into(),
        }
    }
}

#[test]
fn snapshot_change_does_not_retain_unvalidated_old_answers() {
    let module = compile(&source(
        r#"
        proposition P(x: NaturalPerson)
        proposition Q(x: NaturalPerson)
        entity A : NaturalPerson
        query q() -> Bool ! {Determine} { return determined(P(A)) and determined(Q(A)) }
    "#,
    ));
    let mut case = CaseRecord::default();
    let args = BTreeMap::new();
    let name = QueryName::from("q");
    let ctx = context();
    let mut handler = PhaseHandler { changed: false };
    let session = evaluate_session(
        &module,
        &name,
        &args,
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
    )
    .unwrap();
    assert!(matches!(session.outcome, Outcome::Suspended { .. }));
    case.facts.insert("snapshot_revision".into(), Value::Int(2));
    handler.changed = true;
    let resumed = resume(
        session,
        &module,
        &name,
        &args,
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
    )
    .unwrap();
    let fresh = evaluate(
        &module,
        &name,
        &args,
        &case.into_state(),
        &ctx,
        &mut PhaseHandler { changed: true },
        &case,
    )
    .unwrap();
    assert_eq!(
        resumed.outcome, fresh,
        "rebase must not reuse an answer with no validated dependency contract"
    );
}

fn rule(module: &CoreModule, name: &str, guard: Guard, effects: Vec<Consequence>) -> CoreDecl {
    let meta = module.queries[0].meta.clone();
    CoreDecl::Rule(CoreRule {
        id: NodeId::of(name.as_bytes()),
        name: name.into(),
        kind: RuleKind::Constitutive,
        binders: vec![],
        selection: None,
        guard,
        consequences: effects
            .into_iter()
            .enumerate()
            .map(|(i, consequence)| CoreEffect {
                id: EffectId::of(format!("{name}:{i}").as_bytes()),
                consequence,
                meta: meta.clone(),
            })
            .collect(),
        fallback: None,
        meta,
    })
}
fn prop(name: &str) -> PropTerm {
    PropTerm::new(name, vec![])
}

#[test]
fn worklist_replacement_with_equal_cardinality_is_not_quiescence() {
    let mut module = compile(&source("query q() -> Bool { return true }"));
    let first = rule(
        &module,
        "B_to_C",
        Guard::Derived(prop("B")),
        vec![Consequence::Derive(prop("C"))],
    );
    let second = rule(
        &module,
        "A_to_B",
        Guard::Derived(prop("A")),
        vec![
            Consequence::Terminate(prop("A")),
            Consequence::Establish(prop("B")),
        ],
    );
    module.declarations.extend([first, second]);
    let mut case = CaseRecord::default();
    case.facts.insert("A".into(), Value::Bool(true));
    let world = DerivedWorld::compute(&module, &case, &context(), &BTreeMap::new()).unwrap();
    assert!(
        world.holds(&prop("C")),
        "the next pass must propagate the replacement"
    );
}

#[test]
fn declared_true_property_survives_the_entire_compiler_pipeline() {
    let module = compile(&source("verify Trivial { assert true }"));
    let verdict = verify_property(&module, "Trivial");
    assert!(
        verdict.is_proved(),
        "a true property must survive parsing and lowering: {verdict:?}"
    );
}

#[test]
fn evidence_does_not_match_subject_by_incidental_issuer_field() {
    let mut case = CaseRecord::default();
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "Certificate".into(),
        observed_at: context().record_time,
        value: Value::Map(BTreeMap::from([
            ("subject".into(), Value::String("B".into())),
            ("issuer".into(), Value::String("A".into())),
        ])),
    });
    let mut handler = CaseFile {
        record: case,
        known_at: Some(context().record_time),
    };
    let req = OpenRequest::NeedEvidence {
        schema: "Certificate".into(),
        issue: fidryn_core::PropPattern::Ground(PropTerm::new(
            "Certified",
            vec![Term::Ident("A".into())],
        )),
    };
    assert!(
        matches!(handler.handle_observe(&req), HandlerResult::Suspend { .. }),
        "mentioning A as issuer must not make B's certificate evidence about A"
    );
}

#[test]
fn declared_empty_completion_domain_cannot_be_reopened_by_recorded_selection() {
    let module = compile(&source("query q() -> Bool { return true }"));
    let mut case = CaseRecord::default();
    case.admissible_completions
        .interpretations
        .insert("I".into(), vec![]);
    case.interpretations.insert("I".into(), "outside".into());
    let verdict =
        fidryn_verify::check_determinacy(&module, &QueryName::from("q"), &case, &context());
    assert!(
        !matches!(verdict, Ok(fidryn_verify::Determinacy::Convergent { .. })),
        "an invalid recorded value is not an admissible world: {verdict:?}"
    );
}
