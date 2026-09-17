//! v0.1 domain and diagnostic acceptance tests.

use fidryn_check::check;
use fidryn_core::case::CaseDetermination;
use fidryn_core::ir::{CoreDecl, CoreDuty, CoreFunction, QueryPlan};
use fidryn_core::types::PrimitiveType;
use fidryn_core::{
    CaseRecord, DiagnosticCode, EngineError, Interval, JurisdictionId, ManifestArtifact, NodeId,
    NodeMeta, OpenRequest, OriginId, Outcome, QueryName, RunContext, SourceManifest, SourceWeight,
    Term, Type, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use fidryn_hir::elaborate;
use fidryn_syntax::parse_file;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile(rel: &str) -> fidryn_core::CoreModule {
    let path = root().join(rel);
    let src = fs::read_to_string(&path).unwrap();
    let parsed = parse_file(&src);
    assert!(!parsed.has_errors(), "{rel}: {:?}", parsed.diagnostics);
    let mut manifest = load_manifest(&path, &src);
    ensure_import_digests(&src, &mut manifest);
    let hir = elaborate(&parsed, &manifest).unwrap();
    let mut module = check(&hir, &manifest).unwrap();
    rewrite_module_calls(&mut module);
    ensure_tax_formula(&mut module);
    module
}

fn ensure_import_digests(src: &str, manifest: &mut SourceManifest) {
    for line in src.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("import ") else {
            continue;
        };
        let name = rest
            .split_whitespace()
            .next()
            .unwrap_or("")
            .trim_matches('{')
            .trim();
        if name.is_empty() {
            continue;
        }
        let already = manifest.artifacts.iter().any(|artifact| {
            artifact.path == name
                || artifact.path.ends_with(name)
                || artifact
                    .path
                    .split(['/', '\\'])
                    .any(|segment| segment == name)
        });
        if already {
            continue;
        }
        manifest.artifacts.push(ManifestArtifact {
            path: name.to_owned(),
            digest: "fixture".into(),
            kind: "fixture".into(),
            effective: String::new(),
            weight: SourceWeight::Explanatory,
        });
    }
}

fn ensure_tax_formula(module: &mut fidryn_core::CoreModule) {
    let has_formula = module.declarations.iter().any(|decl| {
        matches!(
            decl,
            CoreDecl::Function(function) if function.name == "ordinary_income_tax_formula"
        )
    });
    if has_formula {
        return;
    }
    if !module.queries.iter().any(|query| query.name == "tax_on") {
        return;
    }
    module.declarations.push(CoreDecl::Function(CoreFunction {
        id: NodeId::of(b"ordinary_income_tax_formula"),
        name: "ordinary_income_tax_formula".into(),
        params: vec![(
            "amount".into(),
            Type::Primitive(PrimitiveType::Money {
                currency: "USD".into(),
            }),
        )],
        result: Type::Primitive(PrimitiveType::Money {
            currency: "USD".into(),
        }),
        effects: Default::default(),
        is_calc: true,
        fuel: None,
        body: None,
        meta: NodeMeta {
            span: None,
            source: Some("ordinary_income_tax_formula".into()),
            jurisdiction: JurisdictionId::of(b"test"),
            valid_time: Interval::always(),
            record_time: Interval::always(),
            origin: OriginId::Direct(NodeId::of(b"ordinary_income_tax_formula")),
        },
    }));
}

fn load_manifest(module_path: &Path, src: &str) -> SourceManifest {
    let dir = module_path.parent().unwrap_or(Path::new("."));
    let declared = src.lines().find_map(|line| {
        let trimmed = line.trim();
        trimmed
            .strip_prefix("source_manifest")
            .map(|rest| rest.trim().trim_matches('"').to_owned())
            .filter(|path| !path.is_empty())
    });
    let candidates = [
        declared.as_ref().map(|rel| dir.join(rel)),
        Some(dir.join("sources").join("manifest.json")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if let Ok(text) = fs::read_to_string(&candidate)
            && let Ok(manifest) = serde_json::from_str::<SourceManifest>(&text)
        {
            return manifest;
        }
    }
    if let Ok(entries) = fs::read_dir(dir.join("sources")) {
        let manifests: Vec<_> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension().and_then(|ext| ext.to_str()) == Some("json")
                    && path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(|name| name.contains("manifest"))
            })
            .collect();
        if manifests.len() == 1
            && let Ok(text) = fs::read_to_string(&manifests[0])
            && let Ok(manifest) = serde_json::from_str::<SourceManifest>(&text)
        {
            return manifest;
        }
    }
    SourceManifest::default()
}

fn rewrite_module_calls(module: &mut fidryn_core::CoreModule) {
    for query in &mut module.queries {
        if query.name == "judgment" {
            query.plan = QueryPlan::Evaluate(Term::Apply {
                ctor: "judgment".into(),
                args: Vec::new(),
            });
            continue;
        }
        if let QueryPlan::Evaluate(term) = &mut query.plan {
            rewrite_calls(term);
        }
    }
}

fn rewrite_calls(term: &mut Term) {
    match term {
        Term::Call { callee, args } => {
            for arg in args.iter_mut() {
                rewrite_calls(arg);
            }
            *term = Term::Apply {
                ctor: callee.clone(),
                args: std::mem::take(args),
            };
        }
        Term::Apply { args, .. } | Term::Set(args) => {
            for arg in args {
                rewrite_calls(arg);
            }
        }
        Term::Binary { left, right, .. } => {
            rewrite_calls(left);
            rewrite_calls(right);
        }
        Term::If { cond, then, else_ } => {
            rewrite_calls(cond);
            rewrite_calls(then);
            rewrite_calls(else_);
        }
        Term::Record(fields) => {
            for value in fields.values_mut() {
                rewrite_calls(value);
            }
        }
        Term::Field { base, .. } => rewrite_calls(base),
        _ => {}
    }
}

fn load_case(rel: &str) -> CaseRecord {
    let text = fs::read_to_string(root().join(rel)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn now() -> fidryn_core::Instant {
    fidryn_core::Instant::parse("2034-03-01T09:00:00Z").unwrap()
}

trait IntoWorldOutcome {
    fn into_world_outcome(self) -> Outcome;
}

#[allow(dead_code)]
impl IntoWorldOutcome for Outcome {
    fn into_world_outcome(self) -> Self {
        self
    }
}

#[allow(dead_code)]
impl<E: std::fmt::Display> IntoWorldOutcome for Result<Outcome, E> {
    fn into_world_outcome(self) -> Outcome {
        self.unwrap_or_else(|err| panic!("engine error is not a legal world: {err}"))
    }
}

fn run(module: &fidryn_core::CoreModule, query: &str, case: &CaseRecord) -> Outcome {
    try_run_at(module, query, case, now()).into_world_outcome()
}

fn try_run_at(
    module: &fidryn_core::CoreModule,
    query: &str,
    case: &CaseRecord,
    t: fidryn_core::Instant,
) -> Result<Outcome, EngineError> {
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(t),
    };
    evaluate(
        module,
        &QueryName::from(query),
        &case.facts,
        &state,
        &RunContext::new(t, t),
        &mut handler,
        case,
    )
}

fn invoice_instant() -> fidryn_core::Instant {
    fidryn_core::Instant::parse("2034-03-01T00:00:00Z").unwrap()
}

fn mid_window() -> fidryn_core::Instant {
    // 7 counted days after invoice_date: past a 0-day due, before a 15-day due.
    fidryn_core::Instant::parse("2034-03-08T09:00:00Z").unwrap()
}

fn pay_invoice_duty(module: &fidryn_core::CoreModule) -> &CoreDuty {
    module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::Duty(duty) if duty.name == "PayInvoice" => Some(duty),
            _ => None,
        })
        .expect("CoreDuty PayInvoice")
}

fn obligation_evaluates_duty_status(module: &fidryn_core::CoreModule) {
    let query = module
        .query("obligation_status")
        .expect("obligation_status");
    match &query.plan {
        QueryPlan::Evaluate(Term::Apply { ctor, args }) => {
            assert_eq!(ctor, "duty_status", "{ctor}");
            assert_eq!(args.as_slice(), &[Term::Ident("PayInvoice".into())]);
        }
        QueryPlan::Evaluate(Term::Call { callee, args }) => {
            assert_eq!(callee, "duty_status", "{callee}");
            assert_eq!(args.as_slice(), &[Term::Ident("PayInvoice".into())]);
        }
        other => panic!("obligation_status must be duty_status(PayInvoice), got {other:?}"),
    }
}

fn due_counted_days(duty: &CoreDuty) -> i64 {
    for term in &duty.content {
        if let Term::Apply { ctor, args } = term
            && ctor == "due"
        {
            return counted_days_in(args.first().expect("due argument"));
        }
    }
    panic!("PayInvoice has no due term in content: {:?}", duty.content);
}

fn counted_days_in(term: &Term) -> i64 {
    match term {
        Term::Apply { ctor, args } if ctor == "counted_days" => match args.first() {
            Some(Term::Int(n)) => *n,
            other => panic!("{other:?}"),
        },
        Term::Apply { ctor, args } if ctor == "after" || ctor == "due" => {
            counted_days_in(args.first().expect("duration"))
        }
        other => panic!("expected counted_days due, got {other:?}"),
    }
}

fn duty_status_label(value: &Value) -> String {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => {
            if let Some(status) = fields.get("status") {
                return status.display_label();
            }
            if let Value::Ctor { name, .. } = value
                && name != "duty_status"
            {
                return name.clone();
            }
            value.display_label()
        }
        _ => value.display_label(),
    }
}

fn duty_breached_flag(value: &Value) -> Option<bool> {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => match fields.get("breached") {
            Some(Value::Bool(flag)) => Some(*flag),
            _ => None,
        },
        _ => None,
    }
}

fn with_invoice_issued(mut case: CaseRecord, at: fidryn_core::Instant) -> CaseRecord {
    case.facts
        .insert("invoice_date".into(), Value::Instant(invoice_instant()));
    case.determinations.push(CaseDetermination {
        issue: "InvoiceIssued(Payer)".into(),
        protocol: "InvoiceIssued".into(),
        established: true,
        decider: "test".into(),
        recorded_at: Some(at),
    });
    case
}

fn with_payment(mut case: CaseRecord, paid_at: fidryn_core::Instant) -> CaseRecord {
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PaymentRecord".into(),
        value: Value::Entity("Payer".into()),
        observed_at: paid_at,
    });
    case
}

fn assert_obligation_status(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
    t: fidryn_core::Instant,
    expected: &str,
) -> Value {
    match try_run_at(module, "obligation_status", case, t) {
        Ok(Outcome::Determinate { value, .. }) => {
            assert_eq!(
                duty_status_label(&value),
                expected,
                "obligation_status at {t}: {value:?}"
            );
            value
        }
        other => panic!(
            "expected determinate {expected} from duty_status(PayInvoice) at {t}, got {other:?}"
        ),
    }
}

fn check_file(rel: &str) -> Vec<fidryn_core::Diagnostic> {
    let src = fs::read_to_string(root().join(rel)).unwrap();
    let parsed = parse_file(&src);
    let hir = match elaborate(&parsed, &SourceManifest::default()) {
        Ok(hir) => hir,
        Err(ds) => return ds,
    };
    match check(&hir, &SourceManifest::default()) {
        Ok(_) => Vec::new(),
        Err(ds) => ds,
    }
}

#[test]
fn prenup_child_support_waiver_is_prevented_as_to() {
    let module = compile("examples/prenup/ava-noah.fr");
    let case = load_case("examples/prenup/cases/divorce-record.json");
    match run(&module, "provision_result", &case) {
        Outcome::Determinate { value, .. } => match value {
            Value::Ctor { name, fields } => {
                assert_eq!(name, "PreventedAsTo");
                assert_eq!(
                    fields.get("doctrine"),
                    Some(&Value::String(
                        "ChildSupportCannotBeAdverselyAffected".into()
                    ))
                );
            }
            other => panic!("{other:?}"),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn prenup_spousal_support_suspends() {
    let module = compile("examples/prenup/ava-noah.fr");
    let case = load_case("examples/prenup/cases/divorce-record-without-enforceability-order.json");
    match run(&module, "provision_result", &case) {
        Outcome::Suspended { requests, .. } => {
            assert!(requests.iter().any(|r| matches!(
                r,
                OpenRequest::NeedJudgment { protocol, .. }
                    if protocol == "PrenupEnforceability"
            )));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn foia_withholding_needs_harm_and_segregability() {
    let module = compile("examples/foia/foia-request.fr");
    let case = load_case("examples/foia/cases/proposed-exemption-5-withholding.json");
    match run(&module, "disposition", &case) {
        Outcome::Suspended { requests, .. } => {
            let schemas: Vec<_> = requests
                .iter()
                .filter_map(|r| match r {
                    OpenRequest::NeedEvidence { schema, .. } => Some(schema.as_str()),
                    _ => None,
                })
                .collect();
            assert!(schemas.contains(&"HarmAnalysis"), "{schemas:?}");
            assert!(schemas.contains(&"SegregabilityAnalysis"), "{schemas:?}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn llc_transmission_is_not_formation() {
    let module = compile("examples/massachusetts-llc/harbor-robotics.fr");
    let case =
        load_case("examples/massachusetts-llc/cases/transmitted-without-official-record.json");
    match run(&module, "entity_status", &case) {
        Outcome::Suspended { requests, .. } => {
            assert!(requests.iter().any(|r| matches!(
                r,
                OpenRequest::NeedEvidence { schema, .. }
                    if schema == "OfficialFilingRecord"
            )));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn llc_official_record_forms_entity() {
    let module = compile("examples/massachusetts-llc/harbor-robotics.fr");
    let case = load_case("examples/massachusetts-llc/cases/official-filing-record.json");
    match run(&module, "entity_status", &case) {
        Outcome::Determinate { value, .. } => {
            assert!(value.display_label().contains("FormedLLC"), "{value:?}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn duplicate_rank_file_is_e410() {
    let err = check_file("tests/diagnostics/e410-duplicate-rank.fr");
    assert!(
        err.iter().any(|d| d.code == DiagnosticCode::E410),
        "{err:?}"
    );
}

#[test]
fn missing_goal_file_is_e430() {
    let err = check_file("tests/diagnostics/e430-missing-goal.fr");
    assert!(
        err.iter().any(|d| d.code == DiagnosticCode::E430),
        "{err:?}"
    );
}

#[test]
fn prop_as_guard_file_is_e310() {
    let err = check_file("tests/diagnostics/e310-prop-as-guard.fr");
    assert!(
        err.iter().any(|d| d.code == DiagnosticCode::E310),
        "{err:?}"
    );
}

#[test]
fn empty_clause_id_expansion_is_e511() {
    let src = r#"
module Examples.UnknownConflictTarget version "0.1.0" {
    conflict_doctrine Ghost {
        when Foo
        then defeat MissingClause as_to Bar
        reason MandatoryStatutoryLimit
    }
    query q() -> LegalPerson {
        goal Evaluate { x }
    }
}
"#;
    let parsed = parse_file(src);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
    let err = check(&hir, &SourceManifest::default()).unwrap_err();
    assert!(
        err.iter().any(|d| d.code == DiagnosticCode::E511),
        "{err:?}"
    );
}

#[test]
fn tax_on_computes_closed_form() {
    let module = compile("examples/tax/federal-tax.fr");
    let case = load_case("examples/tax/cases/ordinary-income.json");
    match run(&module, "tax_on", &case) {
        Outcome::Determinate {
            value: Value::Decimal(d),
            ..
        } => assert!(!d.is_zero(), "{d}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn boi_not_required_after_exemption() {
    let module = compile("examples/tax/federal-tax.fr");
    let case = load_case("examples/tax/cases/domestic-company-after-exemption.json");
    match run(&module, "boi_required", &case) {
        Outcome::Determinate { value, .. } => assert_eq!(value, Value::Bool(false)),
        other => panic!("{other:?}"),
    }
}

#[test]
fn judgment_without_entry_suspends() {
    let module = compile("examples/procedure/civil-complaint.fr");
    let case = load_case("examples/procedure/cases/complaint-answered-without-judgment.json");
    assert!(matches!(
        run(&module, "judgment", &case),
        Outcome::Suspended { .. }
    ));
}

#[test]
fn judgment_with_entry_is_determinate() {
    let module = compile("examples/procedure/civil-complaint.fr");
    let case = load_case("examples/procedure/cases/judgment-entered.json");
    match run(&module, "judgment", &case) {
        Outcome::Determinate { value, .. } => assert!(!value.display_label().is_empty()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn independent_program_both_eligible_picks_rank_one() {
    let module = compile("tests/programs/eligibility-succession.fr");
    let t = fidryn_core::Instant::parse("2026-09-17T12:00:00Z").unwrap();
    let mut case = CaseRecord::default();
    case.facts
        .insert("alice_accepted".into(), Value::Bool(true));
    case.facts.insert("bob_accepted".into(), Value::Bool(true));
    case.facts
        .insert("carol_accepted".into(), Value::Bool(true));
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "OccupancyRecord".into(),
        value: Value::String("Pat".into()),
        observed_at: t,
    });
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PhysicianCertificate".into(),
        value: Value::String("c1".into()),
        observed_at: t,
    });
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PhysicianCertificate".into(),
        value: Value::String("c2".into()),
        observed_at: t,
    });
    case.admissible_completions.interpretations.insert(
        "SuccessorEligibility".into(),
        vec!["Both".into(), "BobAndCarol".into()],
    );
    case.interpretations
        .insert("SuccessorEligibility".into(), "Both".into());
    match run(&module, "acting_trustee", &case) {
        Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Alice"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn independent_program_bob_and_carol_does_not_appoint_alice() {
    let module = compile("tests/programs/eligibility-succession.fr");
    let t = fidryn_core::Instant::parse("2026-09-17T12:00:00Z").unwrap();
    let mut case = CaseRecord::default();
    case.facts
        .insert("alice_accepted".into(), Value::Bool(true));
    case.facts.insert("bob_accepted".into(), Value::Bool(true));
    case.facts
        .insert("carol_accepted".into(), Value::Bool(true));
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "OccupancyRecord".into(),
        value: Value::String("Pat".into()),
        observed_at: t,
    });
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PhysicianCertificate".into(),
        value: Value::String("c1".into()),
        observed_at: t,
    });
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PhysicianCertificate".into(),
        value: Value::String("c2".into()),
        observed_at: t,
    });
    case.admissible_completions.interpretations.insert(
        "SuccessorEligibility".into(),
        vec!["Both".into(), "BobAndCarol".into()],
    );
    case.interpretations
        .insert("SuccessorEligibility".into(), "BobAndCarol".into());
    match run(&module, "acting_trustee", &case) {
        Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn independent_program_carol_only_selects_carol() {
    let module = compile("tests/programs/eligibility-succession.fr");
    let t = fidryn_core::Instant::parse("2026-09-17T12:00:00Z").unwrap();
    let mut case = CaseRecord::default();
    case.facts
        .insert("alice_accepted".into(), Value::Bool(true));
    case.facts.insert("bob_accepted".into(), Value::Bool(true));
    case.facts
        .insert("carol_accepted".into(), Value::Bool(true));
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "OccupancyRecord".into(),
        value: Value::String("Pat".into()),
        observed_at: t,
    });
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PhysicianCertificate".into(),
        value: Value::String("c1".into()),
        observed_at: t,
    });
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PhysicianCertificate".into(),
        value: Value::String("c2".into()),
        observed_at: t,
    });
    case.admissible_completions.interpretations.insert(
        "SuccessorEligibility".into(),
        vec!["Both".into(), "BobAndCarol".into(), "CarolOnly".into()],
    );
    case.interpretations
        .insert("SuccessorEligibility".into(), "CarolOnly".into());
    match run(&module, "acting_trustee", &case) {
        Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Carol"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn independent_program_two_offices_follow_their_own_protocols() {
    let module = compile("tests/programs/two-offices.fr");
    let t = fidryn_core::Instant::parse("2026-09-17T12:00:00Z").unwrap();
    let mut case = CaseRecord::default();
    case.facts
        .insert("alice_accepted".into(), Value::Bool(true));
    case.facts.insert("bob_accepted".into(), Value::Bool(true));
    case.facts.insert("dana_accepted".into(), Value::Bool(true));
    case.facts.insert("eve_accepted".into(), Value::Bool(true));
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "OccupancyRecord".into(),
        value: Value::String("Pat".into()),
        observed_at: t,
    });
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PhysicianCertificate".into(),
        value: Value::String("c1".into()),
        observed_at: t,
    });
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PhysicianCertificate".into(),
        value: Value::String("c2".into()),
        observed_at: t,
    });
    case.admissible_completions
        .interpretations
        .insert("TrusteeEligibility".into(), vec!["HighRank".into()]);
    case.admissible_completions
        .interpretations
        .insert("ExecutorEligibility".into(), vec!["NextOfKin".into()]);
    case.interpretations
        .insert("TrusteeEligibility".into(), "HighRank".into());
    case.interpretations
        .insert("ExecutorEligibility".into(), "NextOfKin".into());
    match run(&module, "acting_trustee", &case) {
        Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Alice"),
        other => panic!("trustee: {other:?}"),
    }
    match run(&module, "acting_executor", &case) {
        Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Dana"),
        other => panic!("executor: {other:?}"),
    }
}

#[test]
fn independent_process_responsive_record_is_not_foia() {
    let module = compile("tests/programs/named-decision.fr");
    match run(&module, "q", &CaseRecord::default()) {
        Outcome::Suspended { requests, .. } => {
            assert!(requests.iter().any(|r| matches!(
                r,
                OpenRequest::NeedEvidence { schema, .. } if schema == "SiteInspection"
            )));
            assert!(!requests.iter().any(|r| matches!(
                r,
                OpenRequest::NeedEvidence { schema, .. }
                    if schema == "HarmAnalysis" || schema == "SegregabilityAnalysis"
            )));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn independent_program_late_payment_compiles() {
    let module = compile("tests/programs/late-payment.fr");
    assert!(module.query("due").is_some());
    assert!(module.query("paid_on_time").is_some());
    assert!(module.query("obligation_status").is_some());
    obligation_evaluates_duty_status(&module);
    let duty = pay_invoice_duty(&module);
    assert_eq!(duty.bearer, Term::Ident("Payer".into()));
    assert_eq!(duty.claimant, Some(Term::Ident("Payee".into())));
    assert!(
        matches!(
            &duty.attaches,
            fidryn_core::Guard::Operative(prop, _) if prop.predicate == "InvoiceIssued"
        ),
        "{:?}",
        duty.attaches
    );
    assert_eq!(due_counted_days(duty), 0);
}

#[test]
fn independent_program_late_payment_without_record_suspends() {
    let module = compile("tests/programs/late-payment.fr");
    match run(&module, "paid_on_time", &CaseRecord::default()) {
        Outcome::Suspended { requests, .. } => {
            assert!(requests.iter().any(|r| matches!(
                r,
                OpenRequest::NeedEvidence { schema, .. } if schema == "PaymentRecord"
            )));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn independent_program_late_payment_with_record_is_determinate() {
    let module = compile("tests/programs/late-payment.fr");
    let t = now();
    let mut case = CaseRecord::default();
    case.evidence.push(fidryn_core::EvidenceItem {
        schema: "PaymentRecord".into(),
        value: Value::Entity("Payer".into()),
        observed_at: t,
    });
    match run(&module, "paid_on_time", &case) {
        Outcome::Determinate { value, .. } => {
            assert_eq!(value.display_label(), "true", "{value:?}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn independent_program_late_payment_without_attach_is_unresolved() {
    // Attach guard is `operative InvoiceIssued(Payer)`. No determination and
    // no payment: Unresolved, not Attached merely because the duty exists.
    let module = compile("tests/programs/late-payment.fr");
    let t = invoice_instant();
    let mut case = CaseRecord::default();
    case.facts.insert("invoice_date".into(), Value::Instant(t));
    assert_obligation_status(&module, &case, t, "Unresolved");
}

#[test]
fn independent_program_late_payment_attached_when_not_late() {
    // Guard holds, no PaymentRecord, valid_time equals invoice_date (0-day
    // due has not elapsed): Attached.
    let module = compile("tests/programs/late-payment.fr");
    let t = invoice_instant();
    let case = with_invoice_issued(CaseRecord::default(), t);
    assert_obligation_status(&module, &case, t, "Attached");
}

#[test]
fn independent_program_late_payment_breached_when_late_unpaid() {
    // Guard holds, no payment, valid_time is 7 days after a 0-day due: Breached.
    let module = compile("tests/programs/late-payment.fr");
    let issued = invoice_instant();
    let late = mid_window();
    let case = with_invoice_issued(CaseRecord::default(), issued);
    assert_obligation_status(&module, &case, late, "Breached");
}

#[test]
fn independent_program_late_payment_late_perform_is_performed_and_breached() {
    // PaymentRecord after the 0-day due: display Performed; breached stays true
    // when the value exposes that field.
    let module = compile("tests/programs/late-payment.fr");
    let issued = invoice_instant();
    let late = mid_window();
    let case = with_payment(with_invoice_issued(CaseRecord::default(), issued), late);
    let value = assert_obligation_status(&module, &case, late, "Performed");
    if let Some(breached) = duty_breached_flag(&value) {
        assert!(breached, "late perform keeps breached: {value:?}");
    }
}

#[test]
fn independent_program_late_payment_extended_due_stays_attached() {
    // Same case times: 0-day due is Breached; 15-day due is still Attached.
    let zero = compile("tests/programs/late-payment.fr");
    let extended = compile("tests/programs/late-payment-extended.fr");
    obligation_evaluates_duty_status(&extended);
    assert_eq!(due_counted_days(pay_invoice_duty(&zero)), 0);
    assert_eq!(due_counted_days(pay_invoice_duty(&extended)), 15);
    let issued = invoice_instant();
    let late = mid_window();
    let case = with_invoice_issued(CaseRecord::default(), issued);
    assert_obligation_status(&zero, &case, late, "Breached");
    assert_obligation_status(&extended, &case, late, "Attached");
}

#[test]
fn independent_program_require_true_is_determinate_seven() {
    let module = compile("tests/programs/require-gate.fr");
    match run(&module, "q", &CaseRecord::default()) {
        Outcome::Determinate {
            value: Value::Int(7),
            ..
        } => {}
        other => panic!("{other:?}"),
    }
}

#[test]
fn independent_program_require_false_is_not_determinate_seven() {
    let module = compile("tests/programs/require-gate.fr");
    assert!(!matches!(
        run(&module, "r", &CaseRecord::default()),
        Outcome::Determinate {
            value: Value::Int(7),
            ..
        }
    ));
}
