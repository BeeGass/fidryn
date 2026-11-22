use fidryn_core::{
    BranchClaim, CaseRecord, CheckedCertificate, CompletionProofId, CoreModule, CoverageMethod,
    CoverageWitness, EvaluationReport, ExecutionMode, ExecutionOperation, ExecutionRequest,
    Instant, Outcome, QueryName, ReplayIssuance, RunContext, SourceManifest, TraceId, Value,
};
use fidryn_kernel::{CheckedEvaluation, accept_covering_eval, check_branches};
use std::collections::{BTreeMap, BTreeSet};

fn compile(body: &str) -> CoreModule {
    let parsed = fidryn_syntax::parse_file(&format!(
        "module Kernel.Source version \"0.1.0\" {{ {body} }}"
    ));
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let manifest = SourceManifest::default();
    let hir = fidryn_hir::elaborate(&parsed, &manifest).unwrap();
    fidryn_check::check(&hir, &manifest).unwrap()
}

fn context() -> RunContext {
    let at = Instant::parse("2026-10-29T00:00:00Z").unwrap();
    RunContext::new(at, at)
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

fn certificate_id(
    module: &CoreModule,
    case: &CaseRecord,
    value: &Value,
    witness: &CoverageWitness,
) -> CompletionProofId {
    let ctx = context();
    CheckedCertificate::covering_claims_id_with_identity(
        module.id,
        module.snapshot,
        module.content_fingerprint().unwrap(),
        case,
        &QueryName::from("q"),
        ctx.valid_time,
        ctx.record_time,
        &BTreeSet::new(),
        value,
        witness,
        &BTreeMap::new(),
        ExecutionMode::Operative,
    )
    .unwrap()
}

#[test]
fn source_string_values_cannot_be_certified_as_booleans() {
    let module = compile("query q(x: String) -> String { return x }");
    let mut case = CaseRecord::default();
    case.facts.insert("x".into(), Value::String("true".into()));
    let query = QueryName::from("q");
    assert!(
        check_branches(
            &module,
            &query,
            &case,
            &context(),
            &witness(Value::Bool(true)),
            &Value::Bool(true)
        )
        .is_err()
    );
    let correct = Value::String("true".into());
    check_branches(
        &module,
        &query,
        &case,
        &context(),
        &witness(correct.clone()),
        &correct,
    )
    .unwrap();
    let cert = accept_covering_eval(
        certificate_id(&module, &case, &correct, &witness(correct.clone())),
        module.id,
        module.snapshot,
        &case,
        &query,
        &module,
        &context(),
        &BTreeSet::new(),
        &correct,
        witness(correct.clone()),
    )
    .unwrap();
    assert!(cert.is_covering());
}

#[test]
fn source_value_variants_agree_between_execution_and_covering() {
    for (ty, value) in [
        ("Bool", Value::Bool(true)),
        ("Bool", Value::Bool(false)),
        ("Int", Value::Int(7)),
        ("String", Value::String("false".into())),
        ("String", Value::String("7".into())),
    ] {
        let module = compile(&format!("query q(x: {ty}) -> {ty} {{ return x }}"));
        let case = CaseRecord {
            facts: BTreeMap::from([("x".into(), value.clone())]),
            ..Default::default()
        };
        let ctx = context();
        let mut handler = fidryn_handlers::CaseFile {
            record: case.clone(),
            known_at: Some(ctx.record_time),
        };
        let outcome = fidryn_eval::evaluate(
            &module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &case.into_state(),
            &ctx,
            &mut handler,
            &case,
        )
        .unwrap();
        assert!(
            matches!(outcome, Outcome::Determinate { value: ref actual, .. } if actual == &value),
            "{outcome:?}"
        );
        check_branches(
            &module,
            &QueryName::from("q"),
            &case,
            &ctx,
            &witness(value.clone()),
            &value,
        )
        .unwrap();
    }
}

#[test]
fn raw_replay_stamp_cannot_replace_kernel_report_validation() {
    let module = compile("query q() -> Bool { return false }");
    let case = CaseRecord::default();
    let ctx = context();
    let query = QueryName::from("q");
    let value = Value::Bool(true);
    let witness = witness(value.clone());
    let args = BTreeMap::new();
    let ignored = BTreeSet::new();
    let cert = CheckedCertificate::unverified_replay_claim(
        certificate_id(&module, &case, &value, &witness),
        ReplayIssuance {
            program: module.id,
            snapshot: module.snapshot,
            fingerprint: module.content_fingerprint().unwrap(),
            case: &case,
            query: &query,
            valid: ctx.valid_time,
            known: ctx.record_time,
            constraints: &ignored,
            answer: &value,
            witness: &witness,
            args: &args,
            execution_mode: ExecutionMode::Operative,
        },
    )
    .unwrap();
    let mut report = EvaluationReport::from_outcome(Outcome::Determinate {
        value,
        trace: TraceId::of(b"raw"),
        convergence_certificate: Some(cert),
        ignored_open_issues: ignored,
    });
    report.request = Some(
        ExecutionRequest::from_eval(&module, "q", &args, &case, &ctx, ExecutionMode::Operative)
            .unwrap(),
    );
    report.coverage = Some(witness);
    report.verification_method = CoverageMethod::FiniteReplay;
    let error = CheckedEvaluation::validate(
        &module,
        &query,
        &case,
        &ctx,
        &args,
        ExecutionMode::Operative,
        None,
        report,
    )
    .unwrap_err();
    assert!(error.contains("fabricated"), "{error}");
}

#[test]
fn certificate_consumption_rejects_each_changed_invocation_component() {
    let module = compile("query q() -> Bool { return true }");
    let case = CaseRecord::default();
    let ctx = context();
    let query = QueryName::from("q");
    let args = BTreeMap::new();
    let ignored = BTreeSet::new();
    let value = Value::Bool(true);
    let witness = witness(value.clone());
    let cert = accept_covering_eval(
        certificate_id(&module, &case, &value, &witness),
        module.id,
        module.snapshot,
        &case,
        &query,
        &module,
        &ctx,
        &ignored,
        &value,
        witness,
    )
    .unwrap();
    let request =
        ExecutionRequest::from_eval(&module, "q", &args, &case, &ctx, ExecutionMode::Operative)
            .unwrap();
    cert.validate_invocation(&request, &value, &ignored)
        .unwrap();
    let mut requests = Vec::new();
    let mut changed = request.clone();
    changed.operation = ExecutionOperation::Explore;
    requests.push(changed);
    let mut changed = request.clone();
    changed.semantics_version = "old-semantics".into();
    requests.push(changed);
    let mut changed = request.clone();
    changed.budget = Some(0);
    requests.push(changed);
    let mut changed = request.clone();
    changed.query = QueryName::from("other");
    requests.push(changed);
    let mut changed = request.clone();
    changed.args.insert("x".into(), Value::Int(1));
    requests.push(changed);
    let mut changed = request.clone();
    changed.mode = ExecutionMode::Scenario;
    requests.push(changed);
    let mut changed = request.clone();
    changed.valid_at = Instant::parse("2026-10-28T00:00:00Z").unwrap();
    requests.push(changed);
    let mut changed = request.clone();
    changed.known_at = Instant::parse("2026-10-28T00:00:00Z").unwrap();
    requests.push(changed);
    let mut changed_case = case.clone();
    changed_case.outside_scope.push("new exclusion".into());
    requests.push(
        ExecutionRequest::from_eval(
            &module,
            "q",
            &args,
            &changed_case,
            &ctx,
            ExecutionMode::Operative,
        )
        .unwrap(),
    );
    for (index, mut changed_module) in [
        module.clone(),
        module.clone(),
        module.clone(),
        module.clone(),
    ]
    .into_iter()
    .enumerate()
    {
        match index {
            0 => changed_module.outside_scope.push("outside".into()),
            1 => changed_module.snapshot = fidryn_core::SourceSnapshotId::of(b"changed"),
            2 => changed_module.manifest = fidryn_core::SourceManifestId::of(b"changed"),
            _ => changed_module.jurisdiction = fidryn_core::JurisdictionId::of(b"changed"),
        }
        requests.push(
            ExecutionRequest::from_eval(
                &changed_module,
                "q",
                &args,
                &case,
                &ctx,
                ExecutionMode::Operative,
            )
            .unwrap(),
        );
    }
    for changed in requests {
        assert!(
            cert.validate_invocation(&changed, &value, &ignored)
                .is_err(),
            "{changed:?}"
        );
    }
}

#[test]
fn a_matching_run_envelope_cannot_qualify_a_fabricated_direct_answer() {
    let module = compile("query q() -> Bool { return false }");
    let case = CaseRecord::default();
    let ctx = context();
    let args = BTreeMap::new();
    let mut report = EvaluationReport::from_outcome(Outcome::Determinate {
        value: Value::Bool(true),
        trace: TraceId::of(b"fabricated"),
        convergence_certificate: None,
        ignored_open_issues: BTreeSet::new(),
    });
    report.request = Some(
        ExecutionRequest::from_eval(&module, "q", &args, &case, &ctx, ExecutionMode::Operative)
            .unwrap(),
    );
    let error = CheckedEvaluation::validate(
        &module,
        &QueryName::from("q"),
        &case,
        &ctx,
        &args,
        ExecutionMode::Operative,
        None,
        report,
    )
    .unwrap_err();
    assert!(
        error.contains("outcome does not match execution"),
        "{error}"
    );
}

#[test]
fn covering_admission_uses_normalized_and_recorded_completion_worlds() {
    let module = compile("query q() -> Bool { return true }");
    let mut case = CaseRecord::default();
    case.admissible_completions
        .choices
        .insert("D".into(), vec!["A".into(), "A".into(), "B".into()]);
    case.decisions.insert("D".into(), "A".into());
    let value = Value::Bool(true);
    let witness = CoverageWitness {
        examined: 1,
        total: 1,
        incomplete: false,
        answer: value.clone(),
        branches: vec![BranchClaim {
            bindings: BTreeMap::from([("c:D".into(), Value::String("A".into()))]),
            answer: value.clone(),
        }],
    };
    check_branches(
        &module,
        &QueryName::from("q"),
        &case,
        &context(),
        &witness,
        &value,
    )
    .unwrap();
}

#[test]
fn pure_covering_cannot_bypass_typed_query_argument_admission() {
    let module = compile("query q(x: Bool) -> Bool { return x }");
    let case = CaseRecord {
        facts: BTreeMap::from([("x".into(), Value::String("true".into()))]),
        ..Default::default()
    };
    assert!(
        check_branches(
            &module,
            &QueryName::from("q"),
            &case,
            &context(),
            &witness(Value::String("true".into())),
            &Value::String("true".into())
        )
        .is_err()
    );
}

#[test]
fn literal_covering_cannot_bypass_institutional_conflicts() {
    let add = "rule Add : constitutive { when true then establish P() }";
    let cut = "rule Cut : constitutive { when true then terminate P() }";
    for rules in [format!("{add}\n{cut}"), format!("{cut}\n{add}")] {
        let module = compile(&format!(
            "proposition P()\n{rules}\nquery q() -> Bool {{ return true }}"
        ));
        let error = check_branches(
            &module,
            &QueryName::from("q"),
            &CaseRecord::default(),
            &context(),
            &witness(Value::Bool(true)),
            &Value::Bool(true),
        )
        .unwrap_err();
        assert!(
            error.contains("conflicting institutional effects"),
            "{error}"
        );
    }
}

#[test]
fn report_qualification_rejects_changed_metadata_and_source_bundle() {
    use fidryn_core::{Assumption, OpenRequest, TrustProfile, VerifiedSourceBundle};
    let module = compile("query q() -> Bool { return true }");
    let case = CaseRecord::default();
    let ctx = context();
    let args = BTreeMap::new();
    let query = QueryName::from("q");
    let mut handler = fidryn_handlers::CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    let outcome = fidryn_eval::evaluate_traced(
        &module,
        &query,
        &args,
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
    )
    .unwrap()
    .outcome;
    let bundle = VerifiedSourceBundle::from_manifest(&SourceManifest::default())
        .with_root_source(b"compiled source identity");
    let mut report = EvaluationReport::from_outcome(outcome);
    report.request = Some(
        ExecutionRequest::from_eval(&module, "q", &args, &case, &ctx, ExecutionMode::Operative)
            .unwrap(),
    );
    report.source_bundle = Some(bundle.identity().hex());
    report.trust = bundle.trust_summary();
    let validate = |candidate| {
        CheckedEvaluation::validate(
            &module,
            &query,
            &case,
            &ctx,
            &args,
            ExecutionMode::Operative,
            Some(&bundle),
            candidate,
        )
    };
    validate(report.clone()).unwrap();
    let mut changed = report.clone();
    changed.execution_mode = ExecutionMode::Scenario;
    assert!(validate(changed).is_err());
    let mut changed = report.clone();
    changed.trust = TrustProfile::ByteVerified;
    assert!(validate(changed).is_err());
    let mut changed = report.clone();
    changed.source_bundle = Some("different source bundle".into());
    assert!(validate(changed).is_err());
    let mut changed = report.clone();
    changed.provenance_root = TraceId::of(b"unrelated graph");
    assert!(validate(changed).is_err());
    let mut changed = report.clone();
    changed.unresolved.insert(OpenRequest::NeedCustom {
        effect: "injected".into(),
        payload: String::new(),
    });
    assert!(validate(changed).is_err());
    let mut changed = report.clone();
    changed.assumptions.push(Assumption {
        id: "H".into(),
        payload: Value::Bool(true),
    });
    assert!(validate(changed).is_err());
    let mut changed = report.clone();
    changed.request.as_mut().unwrap().known_at = Instant::parse("2026-10-28T00:00:00Z").unwrap();
    assert!(validate(changed).is_err());
    let other_bundle = bundle.with_root_source(b"changed source bytes");
    assert!(
        CheckedEvaluation::validate(
            &module,
            &query,
            &case,
            &ctx,
            &args,
            ExecutionMode::Operative,
            Some(&other_bundle),
            report
        )
        .is_err()
    );
}

#[test]
fn report_qualification_executes_the_claimed_operation_budget() {
    let module = compile("query q() -> Bool { return true }");
    let case = CaseRecord::default();
    let ctx = context();
    let query = QueryName::from("q");
    let args = BTreeMap::new();
    let mut handler = fidryn_handlers::CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    let outcome = fidryn_eval::evaluate_traced_with_budget(
        &module,
        &query,
        &args,
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
        fidryn_eval::ExecutionBudget { operations: 100 },
    )
    .unwrap()
    .outcome;
    let mut report = EvaluationReport::from_outcome(outcome);
    report.request = Some(
        ExecutionRequest::from_eval(&module, "q", &args, &case, &ctx, ExecutionMode::Operative)
            .unwrap()
            .with_budget(Some(100)),
    );
    let validate = |candidate| {
        CheckedEvaluation::validate(
            &module,
            &query,
            &case,
            &ctx,
            &args,
            ExecutionMode::Operative,
            None,
            candidate,
        )
    };
    validate(report.clone()).unwrap();
    report.request.as_mut().unwrap().budget = Some(0);
    assert!(
        validate(report).is_err(),
        "zero budget cannot qualify an executed answer"
    );
}
