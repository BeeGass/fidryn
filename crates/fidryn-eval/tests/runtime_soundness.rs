use fidryn_core::{
    CaseRecord, CoreModule, Handler, HandlerResult, Instant, OpenRequest, Outcome, QueryName,
    RunContext, SourceManifest, SuspensionReason, Value,
};
use fidryn_eval::{EngineError, evaluate, evaluate_scenario};
use std::collections::{BTreeMap, BTreeSet};

struct Refusing;
impl Handler for Refusing {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
        HandlerResult::Suspend {
            requests: BTreeSet::from([request.clone()]),
            reason: SuspensionReason::MissingRecord,
            trace_fragment: String::new(),
        }
    }
    fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
        self.handle_observe(request)
    }
    fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
        self.handle_observe(request)
    }
    fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
        self.handle_observe(request)
    }
}
fn compile(body: &str) -> CoreModule {
    let source = format!("module Runtime.Soundness version \"0.1.0\" {{ {body} }}");
    let parsed = fidryn_syntax::parse_file(&source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let manifest = SourceManifest::default();
    let hir = fidryn_hir::elaborate(&parsed, &manifest).expect("elaborate source");
    fidryn_check::check(&hir, &manifest).expect("check source")
}
fn case(fragment: serde_json::Value) -> CaseRecord {
    let mut base =
        serde_json::json!({"schema":"fidryn.case-record/v0.1","admissibleCompletions":{}});
    base.as_object_mut()
        .unwrap()
        .extend(fragment.as_object().unwrap().clone());
    serde_json::from_value(base).unwrap()
}
fn ctx(valid: &str, known: &str) -> RunContext {
    RunContext::new(
        Instant::parse(valid).unwrap(),
        Instant::parse(known).unwrap(),
    )
}
fn now() -> RunContext {
    ctx("2026-01-10T12:00:00Z", "2026-02-01T12:00:00Z")
}
fn run(module: &CoreModule, record: &CaseRecord) -> Result<Outcome<Value>, EngineError> {
    evaluate(
        module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &record.into_state(),
        &now(),
        &mut Refusing,
        record,
    )
}
fn value(outcome: Outcome<Value>) -> Value {
    match outcome {
        Outcome::Determinate { value, .. } => value,
        other => panic!("expected determinate, got {other:?}"),
    }
}
fn grant(action: &str) -> serde_json::Value {
    serde_json::json!({"action":action,"scope":"*","context":"*","source":"grant-record","validTime":{"start":"NegInf","end":"PosInf"}})
}

#[test]
fn sibling_sequences_and_repeated_calls_have_distinct_frames() {
    let module = compile(
        "fn f(x: Int) -> Int { return seq(1, x) }\nquery q() -> Int { return seq(1,2) + seq(3,4) + f(2) + f(4) }",
    );
    assert_eq!(
        value(run(&module, &CaseRecord::default()).unwrap()),
        Value::Int(12)
    );
}

#[test]
fn false_require_in_function_and_specialized_plan_blocks_execution() {
    for source in [
        "fn f() -> Int { require false; return 7 }\nquery q() -> Int { return f() }",
        "decision D() -> Int { returns 7 }\nquery q() -> Int { require false; goal RunDecision {\ndecision D } }",
    ] {
        assert!(matches!(
            run(&compile(source), &CaseRecord::default()).unwrap(),
            Outcome::Suspended { .. }
        ));
    }
}

#[test]
fn named_parameter_is_not_an_executable_function() {
    let module = compile(
        "fn x(v: Int) -> Int { v+1 } fn id(x: Int) -> Int { x }\nquery q() -> Int { return id(x=10) }",
    );
    assert_eq!(
        value(run(&module, &CaseRecord::default()).unwrap()),
        Value::Int(10)
    );
}

#[test]
fn withdrawing_the_last_foundation_retracts_a_support_cycle() {
    let module = compile(
        "proposition P()\nproposition Q()\nproposition R()\nrule Seed : derive { when operative P() then derive Q() }\nrule QR : derive { when operative Q() then derive R() }\nrule RQ : derive { when operative R() then derive Q() }\nrule Cut : constitutive { when true then terminate P() }\nquery q() -> Bool !{Determine} { return operative Q() }",
    );
    let record = case(
        serde_json::json!({"determinations":[{"issue":"P","protocol":"P","established":true,"decider":"Court"}]}),
    );
    assert!(!matches!(
        run(&module, &record).unwrap(),
        Outcome::Determinate {
            value: Value::Bool(true),
            ..
        }
    ));
}

#[test]
fn incompatible_effects_are_independent_of_source_order() {
    let add = "rule Add : constitutive { when true then establish P() }";
    let cut = "rule Cut : constitutive { when true then terminate P() }";
    for rules in [format!("{add}\n{cut}"), format!("{cut}\n{add}")] {
        let module = compile(&format!(
            "proposition P()\n{rules}\nquery q() -> Bool !{{Determine}} {{ return operative P() }}"
        ));
        let outcome = run(&module, &CaseRecord::default()).unwrap();
        assert!(
            matches!(outcome, Outcome::NormConflict { .. }),
            "{outcome:?}; module={module:?}"
        );
    }
}

#[test]
fn derivation_or_protocol_alias_cannot_manufacture_a_determination() {
    let derived = compile(
        "proposition P()\nrule R : derive { when true then derive P() }\nquery q() -> Bool !{Determine} { return determined P() }",
    );
    assert!(matches!(
        run(&derived, &CaseRecord::default()).unwrap(),
        Outcome::Suspended { .. }
    ));
    let wrong = compile(
        "entity Alice : NaturalPerson\nproposition P(x: NaturalPerson)\nproposition Q(x: NaturalPerson)\nquery q() -> Bool !{Determine} { return determined Q(Alice) }",
    );
    let record = case(
        serde_json::json!({"determinations":[{"issue":"P(Alice)","protocol":"Q","established":true,"decider":"Court"}]}),
    );
    assert!(matches!(
        run(&wrong, &record).unwrap(),
        Outcome::Suspended { .. }
    ));
}

#[test]
fn unrelated_fact_prefix_does_not_establish_a_proposition() {
    let module = compile(
        "proposition InvoicePaid()\nquery q() -> Bool !{Determine} { return operative InvoicePaid() }",
    );
    let record = case(serde_json::json!({"facts":{"invoice":true}}));
    assert!(matches!(
        run(&module, &record).unwrap(),
        Outcome::Suspended { .. }
    ));
}

#[test]
fn final_binder_truncation_is_an_error() {
    let entities = (0..257)
        .map(|i| format!("entity E{i:03} : NaturalPerson\n"))
        .collect::<String>();
    let module = compile(&format!(
        "{entities}\nproposition P(x: NaturalPerson)\nrule R(x: NaturalPerson) : derive {{ when true then derive P(x) }}\nquery q() -> Bool !{{Determine}} {{ return operative P(E256) }}"
    ));
    assert!(matches!(
        run(&module, &CaseRecord::default()),
        Err(EngineError::Unsupported(_))
    ));
}

const DUTY: &str = "entity Payer : NaturalPerson\nentity Payee : NaturalPerson\nproposition InvoiceIssued(person: NaturalPerson)\nduty PayInvoice { bearer Payer; claimant Payee; attaches when operative InvoiceIssued(Payer); content USD(100.00); due 15 counted_days after invoice_date }\nquery q() -> DutyState { return duty_status(PayInvoice) }";
fn invoice(mut payment: serde_json::Value) -> CaseRecord {
    payment
        .as_object_mut()
        .unwrap()
        .entry("name")
        .or_insert(serde_json::json!("PayInvoice"));
    payment
        .as_object_mut()
        .unwrap()
        .entry("currency")
        .or_insert(serde_json::json!("USD"));
    case(
        serde_json::json!({"facts":{"invoice_date":"2026-01-01T00:00:00Z"},"determinations":[{"issue":"InvoiceIssued(Payer)","protocol":"Invoice","established":true,"decider":"Court"}],"evidence":[{"schema":"PaymentRecord","observedAt":"2026-01-02T00:00:00Z","value":payment}]}),
    )
}
fn status(value: &Value) -> &Value {
    match value {
        Value::Map(fields) => &fields["status"],
        other => panic!("{other:?}"),
    }
}

#[test]
fn only_matching_occurred_performance_satisfies_the_duty() {
    let module = compile(DUTY);
    for payment in [
        serde_json::json!({"payer":"Other","payee":"Other","amount":1,"occurred_at":"2026-01-02T00:00:00Z"}),
        serde_json::json!({"payer":"Payer","payee":"Payee","amount":100,"occurred_at":"2026-01-20T00:00:00Z"}),
        serde_json::json!({"payer":"Payer","payee":"Payee","amount":100,"currency":"EUR","occurred_at":"2026-01-02T00:00:00Z"}),
    ] {
        let out = value(run(&module, &invoice(payment)).unwrap());
        assert_eq!(status(&out), &Value::String("Attached".into()));
    }
    let record = invoice(
        serde_json::json!({"payer":"Payer","payee":"Payee","amount":100,"occurred_at":"2026-01-02T00:00:00Z"}),
    );
    assert_eq!(
        status(&value(run(&module, &record).unwrap())),
        &Value::String("Performed".into())
    );
}

#[test]
fn hours_execute_and_oversized_deadlines_do_not_panic() {
    let module = compile(&DUTY.replace("15 counted_days", "1 hours"));
    let record = invoice(serde_json::json!({}));
    assert_eq!(
        status(&value(run(&module, &record).unwrap())),
        &Value::String("Breached".into())
    );
    let module = compile(&DUTY.replace("15 counted_days", "9223372036854775807 counted_days"));
    assert!(matches!(
        run(&module, &record),
        Err(EngineError::InvalidInput(_))
    ));
    let module = compile(&DUTY.replace("15 counted_days", "1 working_days"));
    assert!(matches!(
        run(&module, &record),
        Err(EngineError::Unsupported(_))
    ));
}

#[test]
fn undeclared_or_ungranted_duty_transitions_do_not_commit() {
    let undeclared =
        compile("query q() -> DutyStatus { return duty_step(Unregistered, attach, Mallory) }");
    assert!(matches!(
        run(&undeclared, &CaseRecord::default()),
        Err(EngineError::InvalidInput(_))
    ));
    let declared = compile(&DUTY.replace(
        "query q() -> DutyState { return duty_status(PayInvoice) }",
        "query q() -> DutyStatus { return duty_step(PayInvoice, attach) }",
    ));
    assert!(matches!(
        run(&declared, &invoice(serde_json::json!({}))).unwrap(),
        Outcome::Suspended { .. }
    ));
}

#[test]
fn duty_reads_observe_staged_writes() {
    let module = compile(&DUTY.replace("return duty_status(PayInvoice)", "return seq(duty_step(PayInvoice, attach), duty_step(PayInvoice, perform), duty_status(PayInvoice))"));
    let mut record = invoice(serde_json::json!({}));
    record.facts.insert("authority_grants".into(), serde_json::from_value::<CaseRecord>(serde_json::json!({"schema":"fidryn.case-record/v0.1","admissibleCompletions":{},"facts":{"g":[grant("attach"),grant("perform")]}})).unwrap().facts.remove("g").unwrap());
    assert_eq!(
        status(&value(run(&module, &record).unwrap())),
        &Value::String("Performed".into())
    );
}

#[test]
fn expired_and_wrong_scope_authority_does_not_grant() {
    let source = "query q() -> Unit { return require_authority(perform, InvoiceA) }";
    let module = compile(source);
    let mut expired = grant("perform");
    expired["validTime"] = serde_json::json!({"start":{"Inclusive":"2025-01-01T00:00:00Z"},"end":{"Exclusive":"2026-01-01T00:00:00Z"}});
    for g in [
        expired,
        serde_json::json!({"action":"perform","scope":"InvoiceA","context":"OtherContext","source":"G","validTime":{"start":"NegInf","end":"PosInf"}}),
        serde_json::json!({"action":"perform","scope":"Other","context":"*","source":"G","validTime":{"start":"NegInf","end":"PosInf"}}),
    ] {
        let record = case(serde_json::json!({"facts":{"authority_grants":[g]}}));
        assert!(matches!(
            run(&module, &record).unwrap(),
            Outcome::Suspended { .. }
        ));
    }
    let module =
        compile("query q() -> Unit {return require_authority(perform,InvoiceA,OtherContext)}");
    let record = case(
        serde_json::json!({"facts":{"authority_grants":[{"action":"perform","scope":"InvoiceA","context":"OtherContext","source":"G","validTime":{"start":"NegInf","end":"PosInf"}}]}}),
    );
    assert_eq!(value(run(&module, &record).unwrap()), Value::Unit);
}

#[test]
fn future_assumption_events_are_neither_applied_nor_reported() {
    let module = compile("query q(flag: Bool) -> Bool { return flag }");
    let record = case(
        serde_json::json!({"facts":{"flag":false},"events":[{"kind":"assumption","recordTime":"2027-01-01T00:00:00Z","validTime":{"start":{"Inclusive":"2027-01-01T00:00:00Z"},"end":"PosInf"},"payload":{"flag":true}}]}),
    );
    let out = evaluate_scenario(
        &module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &record.into_state(),
        &now(),
        &mut Refusing,
        &record,
    )
    .unwrap();
    assert_eq!(value(out), Value::Bool(false));
    assert!(fidryn_eval::duty::scenario_hypotheses(&record, &now()).is_empty());
}

#[test]
fn decision_results_execute_declared_expressions_without_fact_substitution() {
    let module = compile(
        "decision Compute(x: Int) -> Int { returns x + 1 }\nquery q() -> Int { goal RunDecision { decision Compute; arguments {6} } }",
    );
    assert_eq!(
        value(run(&module, &CaseRecord::default()).unwrap()),
        Value::Int(7)
    );
    let module = compile(
        "decision Text() -> String { returns \"literal\" }\nquery q() -> String { goal RunDecision { decision Text } }",
    );
    let record = case(
        serde_json::json!({"facts":{"literal":"replaced", "proposed_disposition":"different"}}),
    );
    assert_eq!(
        value(run(&module, &record).unwrap()),
        Value::String("literal".into())
    );
}

const OFFICE: &str = "entity Pat : NaturalPerson\nentity Other : NaturalPerson\nentity Bob : NaturalPerson\nentity BRT : Trust\noffice TrusteeOf(trust: Trust) occupied_by LegalPerson { cardinality 0..1 }\nproposition Incapacitated(person: NaturalPerson, office: Office)\nproposition Eligible(person: LegalPerson, office: Office)\nevidence_type LunchReceipt { issuer: String; subject: String; purpose: String; conclusion: String }\ninterpretation_family Eligibility for Instrument.clause(\"1\") { alternative OnlyBob { defines Eligible(Bob, TrusteeOf(BRT)) as established } }\nnomination Bob for TrusteeOf(BRT) rank 1\nquery q() -> LegalPerson !{Observe, Determine, Interpret} { goal UniqueOccupant { office TrusteeOf(BRT) } }";
fn office_case() -> CaseRecord {
    case(
        serde_json::json!({"facts":{"Accepted(Bob,TrusteeOf(BRT))":true}, "evidence":[{"schema":"OccupancyRecord", "observedAt":"2026-01-01T00:00:00Z", "value":{"person":"Pat", "office":"TrusteeOf(BRT)"}}], "admissibleCompletions":{"interpretations":{"Eligibility":["OnlyBob"]}}, "interpretations":{"Eligibility":"OnlyBob"}}),
    )
}

#[test]
fn unrelated_incapacity_and_receipts_do_not_establish_vacancy() {
    let module = compile(OFFICE);
    let mut record = office_case();
    record.facts.insert(
        "Incapacitated(Other,TrusteeOf(BRT))".into(),
        Value::Bool(true),
    );
    for _ in 0..2 {
        record.evidence.push(fidryn_core::EvidenceItem {
            schema: "LunchReceipt".into(),
            observed_at: now().record_time,
            value: Value::Map(BTreeMap::from([
                ("issuer".into(), Value::String("Cafeteria".into())),
                ("subject".into(), Value::String("Pat".into())),
                ("purpose".into(), Value::String("TrusteeOf(BRT)".into())),
                ("conclusion".into(), Value::String("Supports".into())),
            ])),
        });
    }
    assert!(matches!(
        run(&module, &record).unwrap(),
        Outcome::Suspended { .. }
    ));
}

#[test]
fn occupancy_uses_its_valid_interval_and_requires_office_scope() {
    let module = compile(OFFICE);
    for occupancy in [
        serde_json::json!({"person":"Pat", "office":"TrusteeOf(BRT)", "validTime":{"start":{"Inclusive":"2026-02-01T00:00:00Z"},"end":"PosInf"}}),
        serde_json::json!({"person":"Pat", "office":"TrusteeOf(BRT)", "validTime":"invalid"}),
        serde_json::json!({"person":"Pat"}),
    ] {
        let record = case(
            serde_json::json!({"evidence":[{"schema":"OccupancyRecord","observedAt":"2026-01-01T00:00:00Z","value":occupancy}]}),
        );
        let outcome = run(&module, &record).unwrap();
        assert!(
            matches!(outcome, Outcome::Suspended { ref requests, .. } if requests.iter().any(|request| matches!(request, OpenRequest::NeedEvidence {schema, ..} if schema == "OccupancyRecord"))),
            "{outcome:?}"
        );
    }
}

#[test]
fn derivation_is_linked_to_actual_rule_execution_and_input_snapshot() {
    let module = compile(
        "proposition P()\nproposition Q()\nrule R : derive { when operative P() then derive Q() }\nquery q() -> Bool !{Determine} { return operative Q() }",
    );
    let record = case(serde_json::json!({"facts":{"P":true}}));
    let evaluate = |record: &CaseRecord| {
        fidryn_eval::evaluate_traced(
            &module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &record.into_state(),
            &now(),
            &mut Refusing,
            record,
        )
        .unwrap()
    };
    let session = evaluate(&record);
    assert_eq!(value(session.outcome.clone()), Value::Bool(true));
    assert!(
        session
            .derivation
            .iter()
            .any(|step| step.kind == "foundation" && step.detail.contains("P"))
    );
    assert!(
        session
            .derivation
            .iter()
            .any(|step| step.kind == "rule_application" && !step.parents.is_empty())
    );
    let mut seen = BTreeSet::new();
    for step in &session.derivation {
        assert!(
            step.parents.iter().all(|parent| seen.contains(parent)),
            "step {step:?} has a missing or forward parent"
        );
        assert!(seen.insert(step.id.clone()));
    }
    assert!(seen.contains(&session.outcome.trace().to_string()));
    let changed = evaluate(&CaseRecord::default());
    assert_ne!(session.outcome.trace(), changed.outcome.trace());
}

const JUDGMENT: &str = "entity Pat : NaturalPerson\nentity One : NaturalPerson\nentity Two : NaturalPerson\nproposition Approved(person: NaturalPerson)\nevidence_type Certificate { issuer: NaturalPerson; subject: NaturalPerson; conclusion: String; examined_at: Time; signed_at: Time }\njudgment Review(person: NaturalPerson) decides Approved(person) {\nauthority concurrence(2 distinct NaturalPerson)\nrecord requires a: Certificate\nrecord requires b: Certificate\nwhere a.subject == person and b.subject == person and a.issuer != b.issuer and fresh(a, 90 days) and fresh(b, 90 days)\nestablish when a.conclusion == \"yes\" and b.conclusion == \"yes\"\nreject when a.conclusion == \"no\" and b.conclusion == \"no\"\notherwise suspend NeedJudgment { issue: Approved(person); protocol: BoardReview }\non established make issue operative in TestContext\n}\nquery q() -> Bool !{Determine} { return determined Approved(Pat) }";
fn certificate(issuer: &str, conclusion: &str, at: &str) -> serde_json::Value {
    serde_json::json!({"schema":"Certificate","observedAt":"2026-01-02T00:00:00Z","value":{"issuer":issuer,"subject":"Pat","conclusion":conclusion,"examined_at":at,"signed_at":at}})
}
#[test]
fn source_judgment_executes_concurrence_constraints_and_conditions() {
    let module = compile(JUDGMENT);
    let records = vec![
        certificate("One", "yes", "2026-01-01T00:00:00Z"),
        certificate("Two", "yes", "2026-01-01T00:00:00Z"),
    ];
    let record = case(serde_json::json!({"evidence":records}));
    assert_eq!(value(run(&module, &record).unwrap()), Value::Bool(true));
    for records in [
        vec![certificate("One", "yes", "2026-01-01T00:00:00Z")],
        vec![
            certificate("One", "yes", "2026-01-01T00:00:00Z"),
            certificate("One", "yes", "2026-01-01T00:00:00Z"),
        ],
        vec![
            certificate("One", "yes", "2025-01-01T00:00:00Z"),
            certificate("Two", "yes", "2025-01-01T00:00:00Z"),
        ],
        vec![
            certificate("Unlicensed", "yes", "2026-01-01T00:00:00Z"),
            certificate("Two", "yes", "2026-01-01T00:00:00Z"),
        ],
    ] {
        let record =
            case(serde_json::json!({"facts":{"required_concurring":0},"evidence":records}));
        assert!(matches!(
            run(&module, &record).unwrap(),
            Outcome::Suspended { .. }
        ));
    }
    let record = case(
        serde_json::json!({"evidence":[certificate("One","no","2026-01-01T00:00:00Z"),certificate("Two","no","2026-01-01T00:00:00Z")]}),
    );
    assert_eq!(value(run(&module, &record).unwrap()), Value::Bool(false));
    let harder = compile(&JUDGMENT.replace("concurrence(2 distinct", "concurrence(3 distinct"));
    let record = case(serde_json::json!({"evidence":records}));
    assert!(matches!(
        run(&harder, &record).unwrap(),
        Outcome::Suspended { .. }
    ));
}

#[test]
fn shared_operation_allowance_and_resume_do_not_reset_work() {
    let module =
        compile("fn f() -> Int { return seq(1,2,3,4) }\nquery q() -> Int { return f()+f() }");
    let record = CaseRecord::default();
    let invoke = |budget| {
        fidryn_eval::evaluate_traced_with_budget(
            &module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &record.into_state(),
            &now(),
            &mut Refusing,
            &record,
            fidryn_eval::ExecutionBudget { operations: budget },
        )
    };
    let full = invoke(1000).unwrap();
    assert_eq!(value(full.outcome.clone()), Value::Int(8));
    assert!(matches!(
        invoke(full.consumed_operations as u32 - 1),
        Err(EngineError::FuelExhausted { .. })
    ));
    let module = compile("query q() -> Unit { return require_authority(perform) }");
    let invoke = |budget| {
        fidryn_eval::evaluate_traced_with_budget(
            &module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &record.into_state(),
            &now(),
            &mut Refusing,
            &record,
            fidryn_eval::ExecutionBudget { operations: budget },
        )
    };
    let measured = invoke(1000).unwrap();
    let first = invoke(measured.consumed_operations as u32 + 1).unwrap();
    let prior = first.derivation.clone();
    let resumed = fidryn_eval::resume(
        first,
        &module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &record.into_state(),
        &now(),
        &mut Refusing,
        &record,
    );
    match resumed {
        Err(EngineError::FuelExhausted { .. }) => {}
        Ok(session) => {
            assert!(prior.iter().all(|step| session.derivation.contains(step)));
            assert!(session.consumed_operations > measured.consumed_operations);
            assert!(matches!(
                fidryn_eval::resume(
                    session,
                    &module,
                    &QueryName::from("q"),
                    &BTreeMap::new(),
                    &record.into_state(),
                    &now(),
                    &mut Refusing,
                    &record
                ),
                Err(EngineError::FuelExhausted { .. })
            ));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn legal_admission_refusals_have_stored_trace_roots() {
    let module = compile("query q() -> Bool { return true }");
    let record = case(serde_json::json!({"admissibleCompletions":{"choices":{"Route":[]}}}));
    let session = fidryn_eval::evaluate_traced(
        &module,
        &QueryName::from("q"),
        &BTreeMap::new(),
        &record.into_state(),
        &now(),
        &mut Refusing,
        &record,
    )
    .unwrap();
    assert!(matches!(session.outcome, Outcome::Inconsistent { .. }));
    assert!(
        session
            .derivation
            .iter()
            .any(|step| step.kind == "completion_admission")
    );
    assert!(
        session
            .derivation
            .iter()
            .any(|step| step.id == session.outcome.trace().to_string())
    );
}

#[test]
fn actual_trust_judgment_uses_source_policy_and_refuses_stale_certificates() {
    let source = include_str!("../../../examples/trust/bryan-revocable-trust.fr");
    let parsed = fidryn_syntax::parse_file(source);
    let manifest = SourceManifest::default();
    let hir = fidryn_hir::elaborate(&parsed, &manifest).unwrap();
    let module = fidryn_check::check(&hir, &manifest).unwrap();
    let record: CaseRecord = serde_json::from_str(include_str!(
        "../../../examples/trust/cases/court-selects-i2.json"
    ))
    .unwrap();
    for evidence in record
        .evidence
        .iter()
        .filter(|evidence| evidence.schema == "PhysicianCertificate")
    {
        let definition = module
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                fidryn_core::ir::CoreDecl::RecordType(record) if record.name == evidence.schema => {
                    Some(record)
                }
                _ => None,
            })
            .unwrap();
        let Value::Map(fields) = &evidence.value else {
            panic!()
        };
        for (name, ty) in &definition.fields {
            assert!(
                fidryn_eval::runtime_value_matches(&module, &fields[name], ty),
                "{name}: {ty:?}, value={:?}",
                fields[name]
            );
        }
        assert!(fidryn_eval::runtime_value_matches(
            &module,
            &evidence.value,
            &fidryn_core::Type::Sort(fidryn_core::Sort::Nominal(evidence.schema.clone()))
        ));
    }
    let fresh = ctx("2026-08-24T09:00:00Z", "2026-08-24T09:00:00Z");
    let result = evaluate(
        &module,
        &QueryName::from("acting_trustee"),
        &BTreeMap::new(),
        &record.into_state(),
        &fresh,
        &mut Refusing,
        &record,
    )
    .unwrap();
    assert_eq!(value(result), Value::Entity("Bob".into()));
    let stale = ctx("2034-01-01T09:00:00Z", "2034-01-01T09:00:00Z");
    assert!(matches!(
        evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &record.into_state(),
            &stale,
            &mut Refusing,
            &record
        )
        .unwrap(),
        Outcome::Suspended { .. }
    ));
}

#[test]
fn observations_validate_and_instantiate_the_exact_bound_record() {
    let module = compile(
        "entity Pat : NaturalPerson\nentity Bob : NaturalPerson\nproposition Certified(person: NaturalPerson)\nevidence_type Certificate {subject: NaturalPerson; authenticated: Bool}\nobservation Validation(record: Certificate) establishes Certified(record.subject) { authenticate record.authenticated == true }\nquery q() -> Bool !{Determine} {return operative Certified(Pat)}",
    );
    let record = case(
        serde_json::json!({"evidence":[{"schema":"Certificate","observedAt":"2026-01-01T00:00:00Z","value":{"subject":"Pat","authenticated":true}}]}),
    );
    assert_eq!(value(run(&module, &record).unwrap()), Value::Bool(true));
    for (schema, subject, authenticated) in [
        ("Certificate", "Pat", false),
        ("Certificate", "Bob", true),
        ("Validation", "Pat", true),
    ] {
        let record = case(
            serde_json::json!({"evidence":[{"schema":schema,"observedAt":"2026-01-01T00:00:00Z","value":{"subject":subject,"authenticated":authenticated}}]}),
        );
        assert!(matches!(
            run(&module, &record).unwrap(),
            Outcome::Suspended { .. }
        ));
    }
}

#[test]
fn declared_effect_requests_bind_values_and_validate_responses() {
    let module = compile(
        "effect Check { request(input: Int) -> Bool }\nquery q(input: Int) -> Bool !{Check} {return Check(input)}",
    );
    let invoke = |input| {
        let record = case(serde_json::json!({"facts":{"input":input}}));
        run(&module, &record).unwrap()
    };
    let requests = |outcome| match outcome {
        Outcome::Suspended { requests, .. } => requests,
        _ => panic!(),
    };
    assert_ne!(requests(invoke(1)), requests(invoke(2)));
    struct Wrong;
    impl Handler for Wrong {
        fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
            Refusing.handle_observe(request)
        }
        fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
            Refusing.handle_determine(request)
        }
        fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
            Refusing.handle_choose(request)
        }
        fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
            Refusing.handle_interpret(request)
        }
        fn handle_custom(&mut self, _: &OpenRequest) -> HandlerResult {
            HandlerResult::Resume {
                value: Value::String("true".into()),
                trace_fragment: "wrong-type".into(),
            }
        }
    }
    let record = case(serde_json::json!({"facts":{"input":1}}));
    assert!(matches!(
        evaluate(
            &module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &record.into_state(),
            &now(),
            &mut Wrong,
            &record
        ),
        Err(EngineError::InvalidInput(_))
    ));
}

#[test]
fn retained_derivation_bytes_exhaust_honestly() {
    let module = compile("query q(input: String) -> String {return input}");
    let record = case(serde_json::json!({"facts":{"input":"x".repeat(1_100_000)}}));
    assert!(matches!(
        run(&module, &record),
        Err(EngineError::FuelExhausted { .. })
    ));
}

#[test]
fn parameterized_duties_ground_typed_records_and_isolate_instances() {
    let module = compile(
        "entity Payer : NaturalPerson\nentity Payee : NaturalPerson\nrecord_type Invoice { payer: NaturalPerson; payee: NaturalPerson; issued: Time }\nduty Pay(request: Invoice) {bearer request.payer; claimant request.payee; attaches when true; content USD(100); due 30 counted_days after request.issued}\nfn finish(request: Invoice) -> DutyStatus {return seq(duty_step(Pay,attach),duty_step(Pay,perform))}\nquery q(a: Invoice,b: Invoice) -> DutyState {return seq(finish(a),duty_status(Pay(b)))}",
    );
    let record = case(
        serde_json::json!({"facts":{"a":{"payer":"Payer","payee":"Payee","issued":"2026-01-01T00:00:00Z"},"b":{"payer":"Payer","payee":"Payee","issued":"2026-01-02T00:00:00Z"},"authority_grants":[grant("attach"),grant("perform")]}}),
    );
    let result = value(run(&module, &record).unwrap_or_else(|error| {
        panic!(
            "{error:?}; duty={:?}",
            module
                .declarations
                .iter()
                .find(|decl| matches!(decl, fidryn_core::ir::CoreDecl::Duty(_)))
        )
    }));
    assert_eq!(status(&result), &Value::String("Attached".into()));
    let missing = compile(
        "entity Payer : NaturalPerson\nduty Pay(request: Int) {bearer Payer; attaches when true; content USD(100)}\nquery q() -> DutyState {return duty_status(Pay)}",
    );
    assert!(matches!(
        run(&missing, &CaseRecord::default()),
        Err(EngineError::InvalidInput(_))
    ));
}

#[test]
fn unsupported_source_selection_conditions_remain_explicitly_open() {
    let source=include_str!("../../../examples/trust/bryan-revocable-trust.fr").replace("and effective AcceptOffice(candidate, TrusteeOf(BRT))","and effective AcceptOffice(candidate, TrusteeOf(BRT))\n              and candidate != Alice");
    let parsed = fidryn_syntax::parse_file(&source);
    let manifest = SourceManifest::default();
    let hir = fidryn_hir::elaborate(&parsed, &manifest).unwrap();
    let module = fidryn_check::check(&hir, &manifest).unwrap();
    let record: CaseRecord = serde_json::from_str(include_str!(
        "../../../examples/trust/cases/court-selects-i2.json"
    ))
    .unwrap();
    let time = ctx("2026-08-24T09:00:00Z", "2026-08-24T09:00:00Z");
    let outcome = evaluate(
        &module,
        &QueryName::from("acting_trustee"),
        &BTreeMap::new(),
        &record.into_state(),
        &time,
        &mut Refusing,
        &record,
    )
    .unwrap();
    assert!(
        matches!(outcome,Outcome::Suspended{requests,..} if requests.iter().any(|request|matches!(request,OpenRequest::NeedCustom{effect,..} if effect == "SelectionPolicy")))
    );
}
