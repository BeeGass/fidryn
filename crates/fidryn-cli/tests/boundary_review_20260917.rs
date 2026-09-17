//! Boundary review regressions for the 2026-09-17 11:35 source export.
//!
//! These assert intended stronger contracts. The review stays open: green
//! tests are regressions, not closure. Do not weaken assertions to preserve
//! a bypass. Covering certificates are issued by kernel replay, not by
//! shape-only public construction.

use fidryn_cli::compile_source;
use fidryn_core::case::CaseDetermination;
use fidryn_core::{
    BranchClaim, CaseRecord, CheckedCertificate, CompletionProofId, CoreModule, CoverageWitness,
    EngineError, EvidenceItem, Instant, Interval, LedgerEvent, ModuleId, OpenRequest, Outcome,
    QueryName, RunContext, SourceManifest, TraceId, TrustProfile, Value,
};
use fidryn_driver::Driver;
use fidryn_eval::{evaluate, evaluate_session, resume};
use fidryn_handlers::CaseFile;
use fidryn_kernel::accept_covering_eval;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

fn compile(body: &str) -> CoreModule {
    let source = format!("module Review version \"0.1.0\" {{\n{body}\n}}");
    compile_source(&source, &SourceManifest::default()).expect("compile review fixture")
}

fn instant(text: &str) -> Instant {
    Instant::parse(text).expect("fixture timestamp")
}

fn context() -> RunContext {
    let t = instant("2033-01-01T00:00:00Z");
    RunContext::new(t, t)
}

fn execute(
    module: &CoreModule,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<Outcome<Value>, EngineError> {
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    evaluate(
        module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &case.into_state(),
        ctx,
        &mut handler,
        case,
    )
}

fn answer(outcome: Result<Outcome<Value>, EngineError>) -> Value {
    match outcome.expect("evaluation succeeds") {
        Outcome::Determinate { value, .. } => value,
        other => panic!("expected determinate answer, got {other:?}"),
    }
}

fn witness(branches: Vec<BTreeMap<String, Value>>, value: bool) -> CoverageWitness {
    CoverageWitness {
        examined: branches.len(),
        total: branches.len(),
        incomplete: false,
        answer: Value::Bool(value),
        branches: branches
            .into_iter()
            .map(|bindings| BranchClaim {
                bindings,
                answer: Value::Bool(value),
            })
            .collect(),
    }
}

fn claim_id(
    module: &CoreModule,
    program: ModuleId,
    case: &CaseRecord,
    ignored: &BTreeSet<OpenRequest>,
    witness: &CoverageWitness,
) -> CompletionProofId {
    let ctx = context();
    CheckedCertificate::covering_claims_id(
        program,
        module.snapshot,
        case,
        &QueryName::from("q"),
        ctx.valid_time,
        ctx.record_time,
        ignored,
        &witness.answer,
        witness,
    )
    .expect("claim digest")
}

#[test]
fn caller_supplied_answers_cannot_mint_finite_replay_without_replay() {
    let module = compile("query q() -> Bool { return false }");
    let case = CaseRecord::default();
    let ctx = context();
    let ignored = BTreeSet::new();
    let fabricated = witness(vec![BTreeMap::new()], true);
    let id = claim_id(&module, module.id, &case, &ignored, &fabricated);
    let result = CheckedCertificate::verified_covering(
        id,
        module.id,
        module.snapshot,
        &case,
        &QueryName::from("q"),
        ctx.valid_time,
        ctx.record_time,
        &ignored,
        &Value::Bool(true),
        fabricated,
    );
    assert!(
        !result.is_ok_and(|certificate| certificate.is_covering()),
        "shape-only public construction must not produce replay authority",
    );
}

#[test]
fn replay_cannot_overwrite_a_fixed_case_fact() {
    let module = compile("query q() -> Bool { return fixed }");
    let mut case = CaseRecord::default();
    case.facts.insert("fixed".into(), Value::Bool(false));
    case.admissible_completions
        .interpretations
        .insert("I".into(), vec!["A".into(), "B".into()]);
    let branches = ["A", "B"]
        .into_iter()
        .map(|alternative| {
            BTreeMap::from([
                ("i:I".into(), Value::String(alternative.into())),
                ("fixed".into(), Value::Bool(true)),
            ])
        })
        .collect();
    let fabricated = witness(branches, true);
    let ignored = BTreeSet::new();
    let id = claim_id(&module, module.id, &case, &ignored, &fabricated);
    assert!(
        accept_covering_eval(
            id,
            module.id,
            module.snapshot,
            &case,
            &QueryName::from("q"),
            &module,
            &context(),
            &ignored,
            &Value::Bool(true),
            fabricated,
        )
        .is_err(),
        "completion bindings may resolve declared slots, not rewrite fixed facts"
    );
}

#[test]
fn matching_branch_count_does_not_replace_domain_membership() {
    let module = compile("query q() -> Bool { return true }");
    let mut case = CaseRecord::default();
    case.admissible_completions
        .interpretations
        .insert("I".into(), vec!["A".into(), "B".into()]);
    // Two different maps are not the two declared worlds I=A and I=B.
    let fabricated = witness(
        vec![
            BTreeMap::from([("noise".into(), Value::Int(0))]),
            BTreeMap::from([("noise".into(), Value::Int(1))]),
        ],
        true,
    );
    let ignored = BTreeSet::new();
    let id = claim_id(&module, module.id, &case, &ignored, &fabricated);
    assert!(
        accept_covering_eval(
            id,
            module.id,
            module.snapshot,
            &case,
            &QueryName::from("q"),
            &module,
            &context(),
            &ignored,
            &Value::Bool(true),
            fabricated,
        )
        .is_err(),
        "exact admitted assignment coverage is required"
    );
}

#[test]
fn replay_rejects_a_different_program_identity() {
    let module = compile("query q() -> Bool { return true }");
    let case = CaseRecord::default();
    let ignored = BTreeSet::new();
    let actual = witness(vec![BTreeMap::new()], true);
    let wrong_program = ModuleId::of(b"different-program");
    let id = claim_id(&module, wrong_program, &case, &ignored, &actual);
    assert!(
        accept_covering_eval(
            id,
            wrong_program,
            module.snapshot,
            &case,
            &QueryName::from("q"),
            &module,
            &context(),
            &ignored,
            &Value::Bool(true),
            actual,
        )
        .is_err(),
        "the replayed program and bound program must agree"
    );
}

#[test]
fn a_real_certificate_for_true_cannot_certify_false() {
    let module = compile("query q() -> Bool { return true }");
    let case = CaseRecord::default();
    let ignored = BTreeSet::from([OpenRequest::NeedInterpretation {
        source: "Review".into(),
        family: "Unneeded".into(),
    }]);
    let actual = witness(vec![BTreeMap::new()], true);
    let id = claim_id(&module, module.id, &case, &ignored, &actual);
    let certificate = accept_covering_eval(
        id,
        module.id,
        module.snapshot,
        &case,
        &QueryName::from("q"),
        &module,
        &context(),
        &ignored,
        &Value::Bool(true),
        actual,
    )
    .expect("a valid certificate for a constant true query");
    assert!(
        Outcome::determinate(
            Value::Bool(false),
            TraceId::of(b"review"),
            Some(certificate),
            ignored,
        )
        .is_err(),
        "certificate consumption must check the certified answer"
    );
}

#[test]
fn future_determinations_are_not_visible_at_an_earlier_known_time() {
    let module = compile(
        r#"
        entity A : NaturalPerson
        proposition P(person: NaturalPerson)
        query q() -> Bool ! {Determine} { return operative(P(A)) }
    "#,
    );
    let mut case = CaseRecord::default();
    case.determinations.push(CaseDetermination {
        issue: "P(A)".into(),
        protocol: "P".into(),
        established: true,
        decider: "Reviewer".into(),
        recorded_at: Some(instant("2034-01-01T00:00:00Z")),
    });
    assert!(
        !matches!(
            execute(&module, &case, &context()),
            Ok(Outcome::Determinate {
                value: Value::Bool(true),
                ..
            }),
        ),
        "future knowledge must not establish a past answer"
    );
}

fn duty_module() -> CoreModule {
    compile(
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
        query q() -> String { goal Evaluate { duty_status(PayInvoice, invoice_a) } }
    "#,
    )
}

fn attached_case() -> CaseRecord {
    let mut case = CaseRecord::default();
    case.facts
        .insert("invoice_date".into(), Value::Instant(context().valid_time));
    case.determinations.push(CaseDetermination {
        issue: "InvoiceIssued(Payer)".into(),
        protocol: "InvoiceIssued".into(),
        established: true,
        decider: "Reviewer".into(),
        recorded_at: Some(context().record_time),
    });
    case
}

fn duty_fields(value: Value) -> BTreeMap<String, Value> {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => fields,
        other => panic!("expected duty record, got {other:?}"),
    }
}

#[test]
fn on_time_payment_remains_unbreached_after_its_deadline() {
    let module = duty_module();
    let mut case = attached_case();
    case.evidence.push(EvidenceItem {
        schema: "PaymentRecord".into(),
        observed_at: instant("2033-01-10T00:00:00Z"),
        value: Value::Map(BTreeMap::from([
            ("name".into(), Value::String("PayInvoice".into())),
            ("instance".into(), Value::String("invoice_a".into())),
        ])),
    });
    let later = instant("2033-02-15T00:00:00Z");
    let fields = duty_fields(answer(execute(
        &module,
        &case,
        &RunContext::new(later, later),
    )));
    assert_eq!(
        fields.get("status"),
        Some(&Value::String("Performed".into()))
    );
    assert_eq!(
        fields.get("breached"),
        Some(&Value::Bool(false)),
        "querying later must not turn timely performance into breach"
    );
}

#[test]
fn relabeling_a_performed_payload_as_correction_does_not_authorize_it() {
    let module = duty_module();
    let mut case = attached_case();
    case.events.push(LedgerEvent {
        kind: "correction".into(),
        valid_time: Interval::always(),
        record_time: context().record_time,
        payload: Value::Ctor {
            name: "Performed".into(),
            fields: BTreeMap::new(),
        },
    });
    match execute(&module, &case, &context()) {
        Err(_) => {} // Rejecting an invalid event before evaluation is also safe.
        Ok(outcome) => {
            let fields = duty_fields(answer(Ok(outcome)));
            assert_ne!(
                fields.get("status"),
                Some(&Value::String("Performed".into())),
                "an ungated event label must not admit a performed payload"
            );
        }
    }
}

#[test]
fn rolled_back_nested_sequence_is_reexecuted_before_transaction_commit() {
    let module = compile(
        r#"
        query q() -> Bool {
            goal Evaluate {
                transaction(seq(duty_step(pay, attach), true), require_authority(release), true)
            }
        }
    "#,
    );
    let case = CaseRecord::default();
    let ctx = context();
    let query = QueryName::from("q");
    let args = BTreeMap::new();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    let session = evaluate_session(
        &module,
        &query,
        &args,
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
    )
    .expect("initial evaluation");
    assert!(matches!(session.outcome, Outcome::Suspended { .. }));
    assert!(!session.bindings.contains_key("duty:pay:default"));
    let mut granted = case.clone();
    granted.facts.insert(
        "authority_grants".into(),
        Value::Set(vec![
            Value::String("attach".into()),
            Value::String("release".into()),
        ]),
    );
    let mut handler = CaseFile {
        record: granted.clone(),
        known_at: Some(ctx.record_time),
    };
    let resumed = resume(
        session,
        &module,
        &query,
        &args,
        &granted.into_state(),
        &ctx,
        &mut handler,
        &granted,
    )
    .expect("resume with grant");
    assert!(matches!(resumed.outcome, Outcome::Determinate { .. }));
    assert!(
        resumed.bindings.contains_key("duty:pay:default"),
        "a rolled-back prefix cannot be marked completed and skipped"
    );
}

#[test]
fn rebase_cannot_reuse_a_require_that_is_now_false() {
    let module = compile(
        r#"
        entity A : NaturalPerson
        proposition P(person: NaturalPerson)
        query q() -> Bool ! {Determine} { require gate; return determined(P(A)) }
    "#,
    );
    let mut case = CaseRecord::default();
    case.facts.insert("gate".into(), Value::Bool(true));
    let ctx = context();
    let query = QueryName::from("q");
    let args = BTreeMap::new();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    let session = evaluate_session(
        &module,
        &query,
        &args,
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
    )
    .expect("initial evaluation");
    assert!(matches!(session.outcome, Outcome::Suspended { .. }));
    case.facts.insert("gate".into(), Value::Bool(false));
    case.determinations.push(CaseDetermination {
        issue: "P(A)".into(),
        protocol: "P".into(),
        established: true,
        decider: "Reviewer".into(),
        recorded_at: Some(ctx.record_time),
    });
    let fresh = execute(&module, &case, &ctx).expect("fresh evaluation");
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    if let Ok(resumed) = resume(
        session,
        &module,
        &query,
        &args,
        &case.into_state(),
        &ctx,
        &mut handler,
        &case,
    ) {
        assert_eq!(
            resumed.outcome, fresh,
            "either reject snapshot changes or invalidate dependent completed work"
        );
    }
}

#[test]
fn sequencing_does_not_hide_a_wrong_result_type() {
    let src = r#"module Review version "0.1.0" {
        query q() -> Bool { require true; return 7 }
    }"#;
    assert!(compile_source(src, &SourceManifest::default()).is_err());
}

#[test]
fn a_declared_function_result_does_not_replace_checking_its_body() {
    let src = r#"module Review version "0.1.0" {
        fn wrong() -> Bool { 7 }
        query q() -> Bool { return wrong() }
    }"#;
    assert!(compile_source(src, &SourceManifest::default()).is_err());
}

struct TempRoot(PathBuf);
impl TempRoot {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "fidryn-review-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::create_dir_all(root.join("sources")).expect("temporary source directory");
        Self(root)
    }
}
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_hex_manifest_entry_without_checked_bytes_is_not_byte_verified() {
    let root = TempRoot::new();
    let source = r#"module Review version "0.1.0" {
        source_manifest "sources/manifest.json"
        query q() -> Bool { return true }
    }"#;
    let path = root.0.join("main.fr");
    std::fs::write(&path, source).expect("source");
    let manifest = serde_json::json!({
        "schema": "fidryn.source-manifest/v0.1",
        "snapshot": "review-snapshot", "jurisdiction": "Test",
        "artifacts": [{"path": "never-created.txt", "digest": "ab".repeat(32),
                       "kind": "text", "effective": "2033-01-01", "weight": "explanatory"}],
    });
    std::fs::write(root.0.join("sources/manifest.json"), manifest.to_string()).expect("manifest");
    let mut driver = Driver::new();
    if let Ok((module, _)) = driver.check_path(&path) {
        assert_ne!(
            driver.source_trust_of(&module),
            TrustProfile::ByteVerified,
            "no bytes were available to authenticate this artifact"
        );
    }
}
