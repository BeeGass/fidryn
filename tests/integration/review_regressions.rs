//! Review regressions for the 2026-09-17 14:45 source export.
//!
//! These assert intended stronger contracts. The review stays open: green
//! tests are regressions, not closure. Do not weaken assertions to preserve
//! a bypass.
//!
//! `CheckedCertificate::issue_finite_replay` is not a covering factory.
//! Without feature `kernel-issue` it always returns `Err` after digest
//! checks. `#[doc(hidden)]` is not access control. The consumer-facing
//! probe is also a rustdoc test on that method in fidryn-core.

use fidryn_cli::{compile_source, snapshot_names_from_module};
use fidryn_core::case::CaseDetermination;
use fidryn_core::{
    BranchClaim, CaseRecord, CheckedCertificate, CoreModule, CoverageWitness, ExecutionMode,
    Instant, Outcome, PropTerm, QueryName, QueryPlan, ReplayIssuance, RunContext, SourceManifest,
    Term, Value,
};
use fidryn_driver::Driver;
use fidryn_eval::{DerivedWorld, evaluate_session, resume};
use fidryn_handlers::CaseFile;
use std::collections::{BTreeMap, BTreeSet};

fn source(body: &str) -> String {
    format!("module Review version \"0.1.0\" {{\n{body}\n}}")
}

fn compile(body: &str) -> CoreModule {
    compile_source(&source(body), &SourceManifest::default()).expect("valid review program")
}

fn ctx() -> RunContext {
    let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
    RunContext::new(t, t)
}

fn apply(name: &str, args: Vec<Term>) -> Term {
    Term::Apply {
        ctor: name.into(),
        args,
    }
}

fn witness(value: Value) -> CoverageWitness {
    CoverageWitness {
        examined: 1,
        total: 1,
        incomplete: false,
        answer: value.clone(),
        branches: vec![BranchClaim {
            bindings: BTreeMap::new(),
            answer: value,
        }],
    }
}

#[test]
fn false_rule_guard_does_not_establish_a_proposition() {
    let module = compile(
        r#"
        proposition P()
        rule R : derive { when false then derive P() }
        query q() -> Bool { return true }
    "#,
    );
    let world =
        DerivedWorld::compute(&module, &CaseRecord::default(), &ctx(), &BTreeMap::new()).unwrap();
    assert!(!world.holds(&PropTerm::new("P", vec![])));
}

#[test]
fn otherwise_is_not_an_additional_then_consequence() {
    let module = compile(
        r#"
        proposition P()
        proposition Q()
        rule R : derive { when true then derive P() otherwise derive Q() }
        query q() -> Bool { return true }
    "#,
    );
    let world =
        DerivedWorld::compute(&module, &CaseRecord::default(), &ctx(), &BTreeMap::new()).unwrap();
    assert!(world.holds(&PropTerm::new("P", vec![])));
    assert!(!world.holds(&PropTerm::new("Q", vec![])));
}

#[test]
fn declared_function_parameters_constrain_call_arguments() {
    let src = source(
        r#"
        fn identity(x: Int) -> Int { x }
        query q() -> Int { return identity("wrong type") }
    "#,
    );
    assert!(compile_source(&src, &SourceManifest::default()).is_err());
}

#[test]
fn incompatible_branches_are_not_an_inference_escape_hatch() {
    let src = source(r#"query q() -> Int { if false { 1 } else { "wrong type" } }"#);
    assert!(compile_source(&src, &SourceManifest::default()).is_err());
}

#[test]
fn resuming_a_nested_sequence_without_new_evidence_stays_suspended() {
    // This test intentionally constructs a Core expression to isolate the machine.
    // The preceding parser/checker tests use source without repairing the Core.
    let mut module = compile("proposition Gate()\nquery q() -> Int { return 0 }");
    module
        .queries
        .iter_mut()
        .find(|q| q.name == "q")
        .unwrap()
        .plan = QueryPlan::Evaluate(apply(
        "seq",
        vec![
            apply("seq", vec![Term::Int(7)]),
            apply(
                "seq",
                vec![
                    apply(
                        "require",
                        vec![apply("determined", vec![apply("Gate", vec![])])],
                    ),
                    Term::Int(9),
                ],
            ),
        ],
    ));
    let case = CaseRecord::default();
    let state = case.into_state();
    let context = ctx();
    let name = QueryName::from("q");
    let args = BTreeMap::new();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(context.record_time),
    };
    let session =
        evaluate_session(&module, &name, &args, &state, &context, &mut handler, &case).unwrap();
    assert!(matches!(session.outcome, Outcome::Suspended { .. }));
    let resumed = resume(
        session,
        &module,
        &name,
        &args,
        &state,
        &context,
        &mut handler,
        &case,
    )
    .unwrap();
    assert!(
        matches!(resumed.outcome, Outcome::Suspended { .. }),
        "{:?}",
        resumed.outcome
    );
}

#[test]
fn a_rebase_runs_the_new_query_not_the_old_residual() {
    // This adopts the currently documented rebase behavior. An explicit rebase
    // API can host this test after separating resume from rebase.
    let old = compile(
        r#"
        proposition Gate()
        query q() -> Int ! {Determine} { require determined(Gate()); return 1 }
    "#,
    );
    let new = compile("query q() -> Int { return 2 }");
    let case = CaseRecord::default();
    let state = case.into_state();
    let context = ctx();
    let args = BTreeMap::new();
    let name = QueryName::from("q");
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(context.record_time),
    };
    let session =
        evaluate_session(&old, &name, &args, &state, &context, &mut handler, &case).unwrap();
    assert!(matches!(session.outcome, Outcome::Suspended { .. }));
    let resumed = resume(
        session,
        &new,
        &name,
        &args,
        &state,
        &context,
        &mut handler,
        &case,
    )
    .unwrap();
    assert!(matches!(
        resumed.outcome,
        Outcome::Determinate {
            value: Value::Int(2),
            ..
        }
    ));
}

#[test]
fn false_permission_entry_does_not_grant_authority() {
    let mut case = CaseRecord::default();
    case.facts.insert(
        "authority_grants".into(),
        Value::Map(BTreeMap::from([("perform".into(), Value::Bool(false))])),
    );
    assert!(!fidryn_eval::duty::action_is_granted(
        &case,
        "perform",
        ctx().record_time
    ));
}

#[test]
fn replay_uses_the_same_argument_precedence_as_execution() {
    let module = compile("query q(x: Bool) -> Bool { return x }");
    let mut case = CaseRecord::default();
    case.facts.insert("x".into(), Value::Bool(false));
    let args = BTreeMap::from([("x".into(), Value::Bool(true))]);
    let context = ctx();
    let actual = Driver::new()
        .run_with_args(&module, "q", &case, &context, &args)
        .unwrap();
    assert!(matches!(
        actual,
        Outcome::Determinate {
            value: Value::Bool(true),
            ..
        }
    ));
    let claimed = Value::Bool(false);
    let proof = witness(claimed.clone());
    let name = QueryName::from("q");
    let ignored = BTreeSet::new();
    let id = CheckedCertificate::covering_claims_id_with_identity(
        module.id,
        module.snapshot,
        module.content_fingerprint().unwrap(),
        &case,
        &name,
        context.valid_time,
        context.record_time,
        &ignored,
        &claimed,
        &proof,
        &args,
        ExecutionMode::Operative,
    )
    .unwrap();
    let result = fidryn_kernel::accept_covering_eval_with_args(
        id,
        module.id,
        module.snapshot,
        &case,
        &name,
        &module,
        &context,
        &ignored,
        &claimed,
        proof,
        &args,
    );
    assert!(!result.is_ok_and(|certificate| certificate.is_covering()));
}

#[test]
fn replay_does_not_admit_a_future_determination() {
    let module = compile(
        r#"
        entity A : NaturalPerson
        proposition P(x: NaturalPerson)
        query q() -> Bool ! {Determine} { return determined(P(A)) }
    "#,
    );
    let mut case = CaseRecord::default();
    case.determinations.push(CaseDetermination {
        issue: "P(A)".into(),
        protocol: "P".into(),
        established: true,
        decider: "fixture".into(),
        recorded_at: Some(Instant::parse("2034-01-01T00:00:00Z").unwrap()),
    });
    let context = ctx();
    let actual = Driver::new().run(&module, "q", &case, &context).unwrap();
    assert!(matches!(actual, Outcome::Suspended { .. }));
    let claimed = Value::Bool(true);
    let proof = witness(claimed.clone());
    let name = QueryName::from("q");
    let ignored = BTreeSet::new();
    let args = BTreeMap::new();
    let id = CheckedCertificate::covering_claims_id_with_identity(
        module.id,
        module.snapshot,
        module.content_fingerprint().unwrap(),
        &case,
        &name,
        context.valid_time,
        context.record_time,
        &ignored,
        &claimed,
        &proof,
        &args,
        ExecutionMode::Operative,
    )
    .unwrap();
    let result = fidryn_kernel::accept_covering_eval_with_args(
        id,
        module.id,
        module.snapshot,
        &case,
        &name,
        &module,
        &context,
        &ignored,
        &claimed,
        proof,
        &args,
    );
    assert!(!result.is_ok_and(|certificate| certificate.is_covering()));
}

#[test]
fn function_body_changes_are_not_invisible_to_source_diff() {
    let before = compile("fn f() -> Bool { true }\nquery q() -> Bool { return f() }");
    let after = compile("fn f() -> Bool { false }\nquery q() -> Bool { return f() }");
    assert_ne!(
        snapshot_names_from_module(&before),
        snapshot_names_from_module(&after)
    );
}

#[test]
fn fabricated_answer_cannot_mint_a_replay_certificate() {
    let module = compile_source(
        "module Review version \"0.1.0\" { query q() -> Bool { return false } }",
        &SourceManifest::default(),
    )
    .unwrap();
    let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
    let case = CaseRecord::default();
    let query = QueryName::from("q");
    let args = BTreeMap::new();
    let ignored = BTreeSet::new();
    let answer = Value::Bool(true); // Deliberately false claim about this program.
    let witness = CoverageWitness {
        examined: 1,
        total: 1,
        incomplete: false,
        answer: answer.clone(),
        branches: vec![BranchClaim {
            bindings: BTreeMap::new(),
            answer: answer.clone(),
        }],
    };
    let fingerprint = module.content_fingerprint().unwrap();
    let id = CheckedCertificate::covering_claims_id_with_identity(
        module.id,
        module.snapshot,
        fingerprint,
        &case,
        &query,
        t,
        t,
        &ignored,
        &answer,
        &witness,
        &args,
        ExecutionMode::Operative,
    )
    .unwrap();
    let result = CheckedCertificate::issue_finite_replay(
        id,
        ReplayIssuance {
            program: module.id,
            snapshot: module.snapshot,
            fingerprint,
            case: &case,
            query: &query,
            valid: t,
            known: t,
            constraints: &ignored,
            answer: &answer,
            witness: &witness,
            args: &args,
            execution_mode: ExecutionMode::Operative,
        },
    );
    assert!(!result.is_ok_and(|cert| cert.is_covering()));
}
