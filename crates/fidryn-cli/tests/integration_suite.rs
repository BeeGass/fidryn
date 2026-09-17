//! Integration suite for the remaining 2026-09-17 boundaries.
//!
//! Does not reimplement fabricated-witness, source-driven due, nested seq,
//! or CLI `check_path` byte-auth. Those crate tests remain the evidence.
//! This file closes the public-API boundaries in `docs/INTEGRATION-SUITE.md`.
//!
//! Six obligations:
//! 1. Verification strength (Structural vs FiniteReplay; empty branches)
//! 2. Replay isolation (accept_covering_eval does not mutate the case)
//! 3. Assumption isolation (operative vs scenario, public envelope)
//! 4. Public report fidelity (`fidryn.evaluation-report/v0.1`)
//! 5. Duty instance isolation (`duty_status(Def, Instance)`)
//! 6. Transaction atomicity (`Apply("transaction", …)` rollback)

use fidryn_cli::compile_source;
use fidryn_core::case::CaseDetermination;
use fidryn_core::{
    Assumption, BranchClaim, CaseRecord, CheckedCertificate, CoverageMethod, CoverageWitness,
    EngineError, EvaluationReport, EvidenceItem, ExecutionMode, Instant, Interval, LedgerEvent,
    OpenRequest, Outcome, QueryName, RunContext, SourceManifest, TraceId, Value,
};
use fidryn_eval::{evaluate, evaluate_scenario, evaluate_session, report_from_scenario, resume};
use fidryn_handlers::CaseFile;
use fidryn_kernel::{accept_covering, accept_covering_eval};
use fidryn_trace::{reject_lossy_export, render_report};
use std::collections::{BTreeMap, BTreeSet};

fn compile(src: &str) -> fidryn_core::CoreModule {
    compile_source(src, &SourceManifest::default()).expect("test program must compile")
}

fn source(body: &str) -> String {
    format!("module Regression version \"0.1.0\" {{\n{body}\n}}")
}

fn context() -> RunContext {
    let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
    RunContext::new(t, t)
}

fn execute(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
) -> Result<Outcome<Value>, EngineError> {
    execute_query(module, "q", case)
}

fn execute_query(
    module: &fidryn_core::CoreModule,
    query: &str,
    case: &CaseRecord,
) -> Result<Outcome<Value>, EngineError> {
    let ctx = context();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    evaluate(
        module,
        &QueryName::from(query),
        &BTreeMap::new(),
        &case.into_state(),
        &ctx,
        &mut handler,
        case,
    )
}

fn execute_scenario(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
) -> Result<Outcome<Value>, EngineError> {
    let ctx = context();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    evaluate_scenario(
        module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &case.into_state(),
        &ctx,
        &mut handler,
        case,
    )
}

fn execute_session(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
) -> Result<fidryn_eval::EvalSession, EngineError> {
    let ctx = context();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    evaluate_session(
        module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &case.into_state(),
        &ctx,
        &mut handler,
        case,
    )
}

fn report_json(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
    report: &EvaluationReport,
) -> serde_json::Value {
    let ctx = context();
    let text = render_report(
        module,
        &QueryName::from("q"),
        ctx.valid_time,
        ctx.record_time,
        case,
        report,
    );
    serde_json::from_str(&text).expect("report json")
}

fn performed_assumption() -> Assumption {
    Assumption {
        id: "performed".into(),
        payload: Value::Ctor {
            name: "Performed".into(),
            fields: BTreeMap::new(),
        },
    }
}

/// Scenario reports must keep qualifications on the envelope.
fn assert_not_outcome_only_export(json: &serde_json::Value) {
    assert_eq!(
        json["schema"], "fidryn.evaluation-report/v0.1",
        "lossy outcome-only export is not the public report: {json}"
    );
    assert_ne!(json["schema"], "fidryn.outcome/v0.1");
    assert!(
        json.get("outcomeDocument").is_some(),
        "report envelope must nest outcomeDocument: {json}"
    );
    assert!(
        json.get("executionMode").is_some(),
        "report envelope must carry executionMode: {json}"
    );
    assert!(
        json.get("assumptions").is_some(),
        "report envelope must carry assumptions: {json}"
    );
    assert!(
        json.get("sourceTrust").is_some(),
        "report envelope must carry sourceTrust: {json}"
    );
}

fn answer(out: Result<Outcome<Value>, EngineError>) -> Value {
    match out.expect("execution must succeed") {
        Outcome::Determinate { value, .. } => value,
        other => panic!("expected a determinate answer: {other:?}"),
    }
}

fn ignored_issue() -> OpenRequest {
    OpenRequest::NeedInterpretation {
        source: "Instrument.clause(\"1\")".into(),
        family: "OpenFamily".into(),
    }
}

fn bool_branch(b: bool, answer: bool) -> BranchClaim {
    let mut bindings = BTreeMap::new();
    bindings.insert("b".into(), Value::Bool(b));
    BranchClaim {
        bindings,
        answer: Value::Bool(answer),
    }
}

fn tautology_module() -> fidryn_core::CoreModule {
    compile(&source("query q() -> Bool { return b or not b }"))
}

fn covering_id(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
    constraints: &BTreeSet<OpenRequest>,
    answer: &Value,
    witness: &CoverageWitness,
) -> fidryn_core::CompletionProofId {
    let ctx = context();
    CheckedCertificate::covering_claims_id(
        module.id,
        module.snapshot,
        case,
        &QueryName::from("q"),
        ctx.valid_time,
        ctx.record_time,
        constraints,
        answer,
        witness,
    )
    .expect("covering claims id")
}

fn structural_id(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
    constraints: &BTreeSet<OpenRequest>,
    answer: &Value,
    witness: &CoverageWitness,
) -> fidryn_core::CompletionProofId {
    let ctx = context();
    CheckedCertificate::structural_claims_id(
        module.id,
        module.snapshot,
        case,
        &QueryName::from("q"),
        ctx.valid_time,
        ctx.record_time,
        constraints,
        answer,
        witness,
    )
    .expect("structural claims id")
}

fn duty_module() -> fidryn_core::CoreModule {
    compile(&source(
        r#"
        entity Payer : NaturalPerson
        entity Payee : NaturalPerson
        proposition InvoiceIssued(person: NaturalPerson)
        duty PayInvoice {
            bearer Payer
            claimant Payee
            attaches when operative InvoiceIssued(Payer)
            content USD(100.00)
            due 30 counted_days after invoice_date
        }
        query q() -> String {
            goal Evaluate { duty_status(PayInvoice) }
        }
    "#,
    ))
}

fn attached_case() -> CaseRecord {
    let t = context().record_time;
    let mut case = CaseRecord::default();
    case.facts.insert("invoice_date".into(), Value::Instant(t));
    case.determinations.push(CaseDetermination {
        issue: "InvoiceIssued(Payer)".into(),
        protocol: "InvoiceIssued".into(),
        established: true,
        decider: "test".into(),
        recorded_at: Some(t),
    });
    case
}

fn performed_event(kind: &str) -> LedgerEvent {
    LedgerEvent {
        kind: kind.into(),
        valid_time: Interval::always(),
        record_time: context().record_time,
        payload: Value::Ctor {
            name: "Performed".into(),
            fields: BTreeMap::new(),
        },
    }
}

fn duty_status_name(value: &Value) -> String {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => match fields.get("status") {
            Some(Value::String(s) | Value::Entity(s)) => s.clone(),
            Some(Value::Ctor { name, .. }) => name.clone(),
            _ => value.display_label(),
        },
        other => other.display_label(),
    }
}

fn is_performed(value: &Value) -> bool {
    duty_status_name(value).eq_ignore_ascii_case("Performed")
}

/// 1. Structural `accept_covering` is not covering; FiniteReplay is.
#[test]
fn structural_accept_covering_cannot_authorize_ignored_issues() {
    let module = tautology_module();
    let case = CaseRecord::default();
    let ctx = context();
    let query = QueryName::from("q");
    let answer = Value::Bool(true);
    let mut ignored = BTreeSet::new();
    ignored.insert(ignored_issue());
    let witness = CoverageWitness::complete(2, answer.clone());
    let id = structural_id(&module, &case, &ignored, &answer, &witness);
    let cert = accept_covering(
        id,
        module.id,
        module.snapshot,
        &case,
        &query,
        &ctx,
        &ignored,
        &answer,
        witness,
    )
    .expect("shape-only accept_covering");
    assert_eq!(cert.method(), CoverageMethod::Structural);
    assert!(!cert.is_covering(), "Structural is not FiniteReplay");
    let err = Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), ignored)
        .expect_err("structural evidence cannot ignore open issues");
    assert!(err.contains("covering"), "{err}");
}

#[test]
fn finite_replay_accept_covering_eval_can_authorize_ignored_issues() {
    let module = tautology_module();
    let case = CaseRecord::default();
    let ctx = context();
    let query = QueryName::from("q");
    let claimed = Value::Bool(true);
    let mut ignored = BTreeSet::new();
    ignored.insert(ignored_issue());
    let witness = CoverageWitness {
        examined: 2,
        total: 2,
        incomplete: false,
        answer: claimed.clone(),
        branches: vec![bool_branch(false, true), bool_branch(true, true)],
    };
    let id = covering_id(&module, &case, &ignored, &claimed, &witness);
    let cert = accept_covering_eval(
        id,
        module.id,
        module.snapshot,
        &case,
        &query,
        &module,
        &ctx,
        &ignored,
        &claimed,
        witness,
    )
    .expect("tautology FiniteReplay");
    assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
    assert!(cert.is_covering());
    Outcome::determinate(claimed, TraceId::of(b"t"), Some(cert), ignored)
        .expect("FiniteReplay may ignore open issues");
}

/// 1. `CoverageWitness::complete()` has empty branches; that is not covering.
#[test]
fn empty_coverage_witness_cannot_cover_via_accept_covering_eval() {
    let module = tautology_module();
    let case = CaseRecord::default();
    let ctx = context();
    let query = QueryName::from("q");
    let answer = Value::Bool(true);
    let mut ignored = BTreeSet::new();
    ignored.insert(ignored_issue());
    let witness = CoverageWitness::complete(2, answer.clone());
    assert!(
        witness.branches.is_empty(),
        "complete() is shape-only; branches stay empty"
    );
    let id = covering_id(&module, &case, &ignored, &answer, &witness);
    let err = accept_covering_eval(
        id,
        module.id,
        module.snapshot,
        &case,
        &query,
        &module,
        &ctx,
        &ignored,
        &answer,
        witness.clone(),
    )
    .expect_err("empty branches cannot be FiniteReplay covering");
    assert!(
        err.contains("branch") || err.contains("no branches") || err.contains("empty"),
        "{err}"
    );

    let msg = CheckedCertificate::verified_covering(
        id,
        module.id,
        module.snapshot,
        &case,
        &query,
        ctx.valid_time,
        ctx.record_time,
        &ignored,
        &answer,
        witness,
    )
    .expect_err("verified_covering rejects empty branches");
    assert!(
        msg.contains("branch")
            || msg.contains("empty")
            || msg.contains("incomplete")
            || msg.contains("covering"),
        "{msg}"
    );
}

/// 2. FiniteReplay re-evaluates on a clone; the caller's case is unchanged.
#[test]
fn accept_covering_eval_does_not_mutate_caller_case() {
    let module = tautology_module();
    let mut case = CaseRecord::default();
    case.facts.insert("marker".into(), Value::Bool(true));
    case.events.push(LedgerEvent {
        kind: "duty".into(),
        valid_time: Interval::always(),
        record_time: context().record_time,
        payload: Value::Ctor {
            name: "Attached".into(),
            fields: BTreeMap::new(),
        },
    });
    let before = case.clone();
    let ctx = context();
    let query = QueryName::from("q");
    let claimed = Value::Bool(true);
    let ignored = BTreeSet::new();
    let witness = CoverageWitness {
        examined: 2,
        total: 2,
        incomplete: false,
        answer: claimed.clone(),
        branches: vec![bool_branch(false, true), bool_branch(true, true)],
    };
    let id = covering_id(&module, &case, &ignored, &claimed, &witness);
    let cert = accept_covering_eval(
        id,
        module.id,
        module.snapshot,
        &case,
        &query,
        &module,
        &ctx,
        &ignored,
        &claimed,
        witness,
    )
    .expect("tautology FiniteReplay");
    assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
    assert!(cert.is_covering());
    assert_eq!(
        case, before,
        "accept_covering_eval must not mutate the caller's case"
    );
    assert_eq!(
        case.events, before.events,
        "replay must not publish events onto the original case"
    );
}

/// 2. Operative `evaluate` does not commit an ungranted duty Performed event.
#[test]
fn operative_duty_performed_event_without_grant_is_not_performed() {
    let module = duty_module();
    let mut case = attached_case();
    let events_before = case.events.clone();
    case.events.push(performed_event("duty"));
    let value = answer(execute(&module, &case));
    assert!(
        !is_performed(&value),
        "ungranted duty Performed event must not be Performed: {value:?}"
    );
    assert_eq!(
        case.events.len(),
        events_before.len() + 1,
        "evaluate must not drop the submitted event from the caller's case"
    );
}

#[test]
fn operative_evaluate_does_not_treat_assumptions_as_performed() {
    let module = duty_module();
    let mut assumed = attached_case();
    assumed.assumptions.push(Assumption {
        id: "performed".into(),
        payload: Value::Ctor {
            name: "Performed".into(),
            fields: BTreeMap::new(),
        },
    });
    let assumed_value = answer(execute(&module, &assumed));
    assert!(
        !is_performed(&assumed_value),
        "operative evaluate must ignore case.assumptions: {assumed_value:?}"
    );

    let mut relabeled = attached_case();
    relabeled.events.push(performed_event("assumption"));
    let events_before = relabeled.events.clone();
    let relabeled_value = answer(execute(&module, &relabeled));
    assert!(
        !is_performed(&relabeled_value),
        "relabeling kind from duty to assumption must not make operative evaluate Performed: {relabeled_value:?}"
    );
    assert_eq!(
        relabeled.events, events_before,
        "operative evaluate must not mutate case.events"
    );
}

#[test]
fn scenario_assumption_can_report_performed_without_mutating_events() {
    let module = duty_module();
    let mut case = attached_case();
    case.assumptions.push(Assumption {
        id: "performed".into(),
        payload: Value::Ctor {
            name: "Performed".into(),
            fields: BTreeMap::new(),
        },
    });
    let events_before = case.events.clone();
    let outcome = execute_scenario(&module, &case).expect("evaluate_scenario");
    let report = report_from_scenario(outcome, case.assumptions.clone());
    assert_eq!(report.execution_mode, ExecutionMode::Scenario);
    assert!(!report.assumptions.is_empty());
    let value = match report.outcome {
        Outcome::Determinate { value, .. } => value,
        other => panic!("expected determinate scenario answer: {other:?}"),
    };
    assert!(
        is_performed(&value),
        "scenario overlay may be Performed: {value:?}"
    );
    assert_eq!(
        case.events, events_before,
        "evaluate_scenario must not mutate case.events"
    );
}

/// 3. Scenario qualifications are visible on the public report envelope.
#[test]
fn scenario_render_report_includes_execution_mode_and_assumptions() {
    let module = duty_module();
    let mut case = attached_case();
    case.assumptions.push(performed_assumption());
    let events_before = case.events.clone();
    let outcome = execute_scenario(&module, &case).expect("evaluate_scenario");
    let report = report_from_scenario(outcome, case.assumptions.clone());
    let json = report_json(&module, &case, &report);
    assert_eq!(json["schema"], "fidryn.evaluation-report/v0.1");
    assert_eq!(json["executionMode"], "scenario");
    let assumptions = json["assumptions"]
        .as_array()
        .expect("assumptions array on the public envelope");
    assert!(
        !assumptions.is_empty(),
        "scenario report must expose a nonempty assumptions array: {json}"
    );
    assert_eq!(json["outcomeDocument"]["schema"], "fidryn.outcome/v0.1");
    assert_eq!(
        case.events, events_before,
        "render_report must not mutate case.events"
    );
}

/// 3 and 6. `render_report` is `fidryn.evaluation-report/v0.1` with
/// `outcomeDocument`. In-memory `compile_source` is unauthenticated (mill paste).
#[test]
fn render_report_schema_is_evaluation_report_v0_1() {
    let module = compile(&source("query q() -> Bool { return true }"));
    let case = CaseRecord::default();
    let ctx = context();
    let query = QueryName::from("q");
    let outcome = execute(&module, &case).expect("evaluate");
    let report = EvaluationReport::from_outcome(outcome);
    let text = render_report(
        &module,
        &query,
        ctx.valid_time,
        ctx.record_time,
        &case,
        &report,
    );
    let json: serde_json::Value = serde_json::from_str(&text).expect("report json");
    assert_eq!(json["schema"], "fidryn.evaluation-report/v0.1");
    let doc = json
        .get("outcomeDocument")
        .expect("report envelope contains outcomeDocument");
    assert_eq!(doc["schema"], "fidryn.outcome/v0.1");
    assert_eq!(doc["query"], "q");
    assert_eq!(json["sourceTrust"], "unauthenticated");
    assert_eq!(json["executionMode"], "operative");
    assert_eq!(json["verificationMethod"], "none");
}

/// 4. Scenario reports must not collapse to an outcome-only export.
#[test]
fn scenario_report_is_not_outcome_only_export() {
    let module = duty_module();
    let mut case = attached_case();
    case.assumptions.push(performed_assumption());
    let outcome = execute_scenario(&module, &case).expect("evaluate_scenario");
    let report = report_from_scenario(outcome, case.assumptions.clone());
    let json = report_json(&module, &case, &report);
    assert_not_outcome_only_export(&json);
    assert_eq!(json["executionMode"], "scenario");
    assert!(
        json["assumptions"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "scenario qualifications must stay on the envelope: {json}"
    );
    let err = reject_lossy_export(&report)
        .expect_err("scenario reports must not use an outcome-only export");
    assert!(
        err.contains("executionMode") || err.contains("assumptions") || err.contains("drop"),
        "{err}"
    );
}

/// 4. Payment for one duty instance does not perform another.
#[test]
fn two_duty_instances_isolation() {
    let module = compile(&source(
        r#"
        entity Payer : NaturalPerson
        entity Payee : NaturalPerson
        proposition InvoiceIssued(person: NaturalPerson)
        duty PayInvoice {
            bearer Payer
            claimant Payee
            attaches when operative InvoiceIssued(Payer)
            content USD(100.00)
            due 30 counted_days after invoice_date
        }
        query a() -> String {
            goal Evaluate { duty_status(PayInvoice, invoice_a) }
        }
        query b() -> String {
            goal Evaluate { duty_status(PayInvoice, invoice_b) }
        }
    "#,
    ));
    let mut case = attached_case();
    case.evidence.push(EvidenceItem {
        schema: "PaymentRecord".into(),
        observed_at: context().record_time,
        value: Value::Map(BTreeMap::from([
            ("name".into(), Value::String("PayInvoice".into())),
            ("instance".into(), Value::String("invoice_a".into())),
        ])),
    });
    let paid = answer(execute_query(&module, "a", &case));
    let other = answer(execute_query(&module, "b", &case));
    assert!(
        is_performed(&paid),
        "instance invoice_a should be Performed: {paid:?}"
    );
    assert!(
        !is_performed(&other),
        "instance invoice_b must stay unperformed: {other:?}"
    );
}

/// 5. Illegal second step rolls back the first duty_step; the caller's case is unchanged.
#[test]
fn transaction_rollback() {
    let module = compile(&source(
        r#"
        query q() -> Bool {
            goal Evaluate {
                transaction(duty_step(pay, attach), duty_step(pay, discharge))
            }
        }
    "#,
    ));
    let case = CaseRecord::default();
    let before = case.clone();
    let err = execute(&module, &case).expect_err("illegal discharge must not commit");
    let msg = err.to_string();
    assert!(
        msg.contains("discharge") || matches!(err, EngineError::InvalidInput(_)),
        "{err:?}"
    );
    assert_eq!(case, before, "failed transaction must not mutate case");
}

/// 5. Unknown attach guard vs a negative determination.
///
/// `DutyStatus::NotAttached` is not on the enum yet. Assert via the
/// unknown attach is Unresolved; established:false is NotAttached.
#[test]
fn attach_guard_unknown_is_unresolved_established_false_is_not_attached() {
    let module = duty_module();
    let t = context().record_time;
    let mut unknown = CaseRecord::default();
    unknown
        .facts
        .insert("invoice_date".into(), Value::Instant(t));
    let unknown_name = duty_status_name(&answer(execute(&module, &unknown)));
    assert!(
        unknown_name.eq_ignore_ascii_case("Unresolved"),
        "unknown attach guard must be Unresolved, got {unknown_name}"
    );
    assert!(!unknown_name.eq_ignore_ascii_case("Attached"));
    assert!(!unknown_name.eq_ignore_ascii_case("Performed"));

    let mut denied = unknown.clone();
    denied.determinations.push(CaseDetermination {
        issue: "InvoiceIssued(Payer)".into(),
        protocol: "InvoiceIssued".into(),
        established: false,
        decider: "test".into(),
        recorded_at: Some(t),
    });
    let denied_name = duty_status_name(&answer(execute(&module, &denied)));
    assert!(
        denied_name.eq_ignore_ascii_case("NotAttached"),
        "established:false must be NotAttached, got {denied_name}"
    );
    assert_ne!(denied_name, unknown_name);
    assert!(!denied_name.eq_ignore_ascii_case("Unresolved"));
    assert!(!denied_name.eq_ignore_ascii_case("Attached"));
    assert!(!denied_name.eq_ignore_ascii_case("Performed"));
}

/// 6. Suspended transaction rolls back the prefix; resume does not commit it.
#[test]
fn transaction_suspend_does_not_commit_prefix() {
    let module = compile(&source(
        r#"
        query q() -> Bool {
            goal Evaluate {
                transaction(duty_step(pay, attach), require_authority(missing_grant))
            }
        }
    "#,
    ));
    let case = CaseRecord::default();
    let before = case.clone();
    let session = execute_session(&module, &case).expect("transaction suspends");
    assert!(
        matches!(session.outcome, Outcome::Suspended { .. }),
        "{:?}",
        session.outcome
    );
    if let Some(cont) = session.continuation.as_ref() {
        assert!(
            !cont.bindings.keys().any(|k| k.starts_with("duty:")),
            "rolled-back transaction must not keep duty bindings: {:?}",
            cont.bindings.keys().collect::<Vec<_>>()
        );
    }
    assert_eq!(case, before, "suspended transaction must not mutate case");

    let ctx = context();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    let resumed = resume(
        session,
        &module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
    )
    .expect("resume after rolled-back suspend");
    assert!(
        matches!(resumed.outcome, Outcome::Suspended { .. }),
        "{:?}",
        resumed.outcome
    );
    assert_eq!(case, before, "resume must not mutate the caller's case");
}

/// Mill pasted compile is the same unauthenticated report envelope.
#[tokio::test]
async fn mill_pasted_run_is_unauthenticated_evaluation_report() {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use fidryn_cli::ui::router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let src = source(
        r#"
        query q() -> Bool {
            goal Evaluate { true }
        }
    "#,
    );
    let body = serde_json::json!({
        "source": src,
        "query": "q",
        "case": {},
        "validAt": "2033-01-01T00:00:00Z",
        "knownAt": "2033-01-01T00:00:00Z"
    });
    let response = router()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/run")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["ok"], true, "{json}");
    assert_not_outcome_only_export(&json);
    assert_eq!(json["sourceTrust"], "unauthenticated", "{json}");
    assert_eq!(json["executionMode"], "operative", "{json}");
    assert_eq!(json["outcomeDocument"]["schema"], "fidryn.outcome/v0.1");
}
