//! Semantic regression tests for intended Fidryn contracts.
//!
//! Installed from the review kit. Assertions describe the desired
//! semantics, not currently buggy evaluator or checker behavior.

use fidryn_cli::{compile_module, compile_source, snapshot_names_from_module};
use fidryn_core::case::CaseDetermination;
use fidryn_core::{
    CaseRecord, CheckedCertificate, CoreDecl, CoreModule, EvidenceItem, Handler, HandlerResult,
    Instant, ModuleId, OpenRequest, Outcome, PropTerm, QueryName, RunContext, SourceManifest,
    SourceSnapshotId, Term, TraceId, Type, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::{CaseFile, aggregate};
use fidryn_syntax::parse_file;
use fidryn_trace::render_outcome;
use fidryn_verify::{explore_query, verify_property};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile(src: &str) -> CoreModule {
    compile_source(src, &SourceManifest::default()).expect("test module should compile")
}

fn constant_query(name: &str, expression: &str) -> CoreModule {
    compile(&format!(
        "module Regression version \"0.1.0\" {{\n\
         query {name}() -> Bool {{\n goal Evaluate {{ {expression} }}\n }}\n }}"
    ))
}

fn at() -> Instant {
    Instant::parse("2033-01-01T00:00:00Z").expect("timestamp")
}

/// Accept `evaluate` as either `Outcome<Value>` or `Result<Outcome<Value>, E>`.
trait IntoEvalOutcome {
    fn into_eval_outcome(self) -> Outcome<Value>;
}

impl IntoEvalOutcome for Outcome<Value> {
    fn into_eval_outcome(self) -> Self {
        self
    }
}

#[allow(dead_code)]
impl<E: std::fmt::Debug> IntoEvalOutcome for Result<Outcome<Value>, E> {
    fn into_eval_outcome(self) -> Outcome<Value> {
        self.expect("evaluate should succeed for this regression input")
    }
}

fn eval_outcome<T: IntoEvalOutcome>(result: T) -> Outcome<Value> {
    result.into_eval_outcome()
}

fn run(module: &CoreModule, name: &str, case: &CaseRecord) -> Outcome<Value> {
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(at()),
    };
    eval_outcome(evaluate(
        module,
        &QueryName::from(name),
        &BTreeMap::new(),
        &state,
        &RunContext::new(at(), at()),
        &mut handler,
        case,
    ))
}

fn answer(out: Outcome<Value>) -> Value {
    match out {
        Outcome::Determinate { value, .. } => value,
        other => panic!("expected determinate result, got {other:?}"),
    }
}

fn det(value: Value) -> Outcome<Value> {
    Outcome::Determinate {
        value,
        trace: TraceId::of(b"test"),
        convergence_certificate: None,
        ignored_open_issues: BTreeSet::new(),
    }
}

/// `Ok(())`, `Valid`, and `Holds` are success. `Err`, `Invalid`, and `Unknown` fail.
fn is_property_verified(verdict: impl std::fmt::Debug) -> bool {
    let text = format!("{verdict:?}");
    text == "Ok(())"
        || text == "Valid"
        || text == "Holds"
        || text == "Verified"
        || text.starts_with("Ok(Valid")
        || text.starts_with("Ok(Holds")
        || text.starts_with("Ok(Verified")
}

#[test]
fn literal_query_evaluates_its_body() {
    let module = constant_query("q", "true");
    assert_eq!(
        answer(run(&module, "q", &CaseRecord::default())),
        Value::Bool(true)
    );
}

#[test]
fn changing_literal_body_changes_answer() {
    let yes = constant_query("q", "true");
    let no = constant_query("q", "false");
    let case = CaseRecord::default();
    assert_ne!(answer(run(&yes, "q", &case)), answer(run(&no, "q", &case)));
}

#[test]
fn renaming_query_does_not_change_its_computation() {
    let ordinary = constant_query("q", "true");
    let renamed = constant_query("tax_on", "true");
    let case = CaseRecord::default();
    assert_eq!(
        answer(run(&ordinary, "q", &case)),
        answer(run(&renamed, "tax_on", &case)),
    );
}

#[test]
fn declared_result_type_survives_lowering() {
    let module = constant_query("q", "true");
    assert_eq!(module.query("q").expect("q").result_type, Type::bool());
}

#[test]
fn incompatible_query_result_is_a_compile_error() {
    let src = r#"module Regression version "0.1.0" {
        query q() -> Bool { return 123 }
    }"#;
    assert!(compile_source(src, &SourceManifest::default()).is_err());
}

#[test]
fn mutually_recursive_functions_need_a_termination_argument() {
    let src = r#"module Regression version "0.1.0" {
        fn first(n: Int) -> Int { second(n) }
        fn second(n: Int) -> Int { first(n) }
    }"#;
    assert!(compile_source(src, &SourceManifest::default()).is_err());
}

#[test]
fn rule_consequences_survive_lowering() {
    let module = compile(
        r#"module Regression version "0.1.0" {
        proposition P()
        proposition Q()
        rule R : derive {
            when operative P()
            then derive Q()
        }
    }"#,
    );
    let rule = module
        .declarations
        .iter()
        .find_map(|d| match d {
            CoreDecl::Rule(rule) if rule.name == "R" => Some(rule),
            _ => None,
        })
        .expect("rule R");
    assert!(!rule.consequences.is_empty());
}

#[test]
fn unknown_fields_inside_declarations_are_parse_errors() {
    let parsed = parse_file(
        r#"module Regression version "0.1.0" {
        source S {
            artifact "source.txt"
            completely_unknown_field true
        }
    }"#,
    );
    assert!(parsed.has_errors());
}

#[test]
fn parser_rejects_trailing_nontrivia_after_module() {
    let parsed = parse_file(r#"module Regression version "0.1.0" {} unexpected_tokens"#);
    assert!(parsed.has_errors());
}

#[test]
fn non_ascii_identifier_is_rejected_without_panicking() {
    // The supplied grammar permits ASCII identifiers only. Invalid input
    // still needs valid UTF-8 token boundaries and recoverable diagnostics.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        parse_file("module é version \"0.1.0\" {}")
    }));
    assert!(result.is_ok(), "parser panicked on a valid UTF-8 string");
    assert!(result.expect("no panic").has_errors());
}

#[test]
fn value_roundtrip_preserves_nominal_entity_variant() {
    let original = Value::Entity("Alice".into());
    let encoded = serde_json::to_string(&original).expect("serialize");
    let decoded: Value = serde_json::from_str(&encoded).expect("deserialize");
    assert_eq!(original, decoded);
}

#[test]
fn same_constructor_different_fields_remains_contingent() {
    let award = |amount| Value::Ctor {
        name: "Award".into(),
        fields: BTreeMap::from([("amount".into(), Value::Int(amount))]),
    };
    let out = aggregate(vec![det(award(100)), det(award(200))]);
    assert!(matches!(out, Outcome::Contingent { .. }), "{out:?}");
}

#[test]
fn nested_contingency_is_not_discarded() {
    let branch = Outcome::Contingent {
        alternatives: BTreeMap::from([("A".into(), Value::Int(2)), ("B".into(), Value::Int(3))]),
        pivots: BTreeSet::new(),
        trace: TraceId::of(b"contingent"),
    };
    let out = aggregate(vec![det(Value::Int(1)), branch]);
    assert!(!out.is_determinate(), "{out:?}");
}

#[test]
fn missing_occupancy_and_unknown_coverage_do_not_produce_a_certificate() {
    let module = compile(
        r#"module Regression version "0.1.0" {
        query q() -> LegalPerson ! {Observe, Determine, Interpret} {
            goal UniqueOccupant { office TrusteeOf(BRT) }
        }
    }"#,
    );
    let out = run(&module, "q", &CaseRecord::default());
    assert!(
        !out.is_determinate(),
        "no occupant or proof was supplied: {out:?}"
    );
}

#[test]
fn absence_without_closure_is_not_a_determinate_negative_status() {
    let module = compile(
        r#"module Regression version "0.1.0" {
        query q() -> EntityStatus ! {Observe, Determine} {
            goal StatusOf {
                status FormedLLC
                when_closed_absent NotFormedLLC
            }
        }
    }"#,
    );
    let out = run(&module, "q", &CaseRecord::default());
    assert!(
        !out.is_determinate(),
        "no closure record was supplied: {out:?}"
    );
}

#[test]
fn determination_for_one_subject_does_not_discharge_another_subject() {
    let mut record = CaseRecord::default();
    record.determinations.push(CaseDetermination {
        issue: "Eligible(Alice)".into(),
        protocol: "EligibilityDecision".into(),
        established: true,
        decider: "Court".into(),
    });
    let mut handler = CaseFile::new(record);
    let request = OpenRequest::NeedJudgment {
        issue: PropTerm::new("Eligible", vec![Term::Ident("Bob".into())]),
        protocol: "EligibilityDecision".into(),
    };
    let out = handler.handle(&request);
    assert!(!matches!(out, HandlerResult::Resume { .. }), "{out:?}");
}

#[test]
fn future_evidence_cannot_discharge_a_past_query() {
    let (module, _) = compile_module(&root().join("examples/foia/foia-request.fr"))
        .expect("compile FOIA fixture");
    let mut case = CaseRecord::default();
    for schema in ["HarmAnalysis", "SegregabilityAnalysis"] {
        case.evidence.push(EvidenceItem {
            schema: schema.into(),
            value: Value::String("future record".into()),
            observed_at: Instant::parse("2034-01-01T00:00:00Z").expect("future time"),
        });
    }
    let out = run(&module, "disposition", &case);
    assert!(!out.is_determinate(), "records postdate known-at: {out:?}");
}

#[test]
fn transport_receipt_does_not_count_as_official_filing_record() {
    let src = std::fs::read_to_string(root().join("examples/massachusetts-llc/harbor-robotics.fr"))
        .expect("LLC fixture");
    let module = compile(&src);
    let mut case = CaseRecord::default();
    case.facts.insert("transmitted".into(), Value::Bool(true));
    case.evidence.push(EvidenceItem {
        schema: "FilingTransportReceipt".into(),
        value: Value::String("HTTP receipt, not an official record".into()),
        observed_at: at(),
    });
    case.determinations.push(CaseDetermination {
        issue: "FormationComplies".into(),
        protocol: "FormationCompliance".into(),
        established: true,
        decider: "AuthorizedReviewer".into(),
    });
    let out = run(&module, "entity_status", &case);
    assert!(
        !out.is_determinate(),
        "transport receipt promoted to official filing: {out:?}"
    );
}

#[test]
fn verification_requires_a_declared_property_not_just_a_query() {
    let module = constant_query("q", "false");
    let verdict = verify_property(&module, "q");
    assert!(
        !is_property_verified(&verdict),
        "a query name must not count as a verified property: {verdict:?}"
    );
}

#[test]
fn special_property_name_is_not_automatically_verified() {
    let module = constant_query("q", "true");
    let verdict = verify_property(&module, "TrusteeContinuity");
    assert!(
        !is_property_verified(&verdict),
        "TrusteeContinuity must not auto-verify: {verdict:?}"
    );
}

#[test]
fn tax_fixture_does_not_drop_at_third_threshold() {
    // This tests internal arithmetic consistency only, not real-world tax law.
    let (module, _) =
        compile_module(&root().join("examples/tax/federal-tax.fr")).expect("compile tax fixture");
    let amount = |text: &str| {
        let mut case = CaseRecord::default();
        case.facts
            .insert("amount".into(), Value::String(text.into()));
        match answer(run(&module, "tax_on", &case)) {
            Value::Decimal(d) => d,
            other => panic!("expected decimal tax, got {other:?}"),
        }
    };
    let below = amount("103350.00");
    let above = amount("103350.01");
    assert!(above >= below, "tax dropped from {below} to {above}");
}

#[test]
fn source_manifest_header_is_actually_loaded() {
    let path = root().join("examples/trust/bryan-revocable-trust.fr");
    let (_, manifest) = compile_module(&path).expect("compile trust fixture");
    assert_eq!(manifest.snapshot, "2026-08-23-ma-trust-fixture");
    assert!(!manifest.artifacts.is_empty());
}

#[test]
fn changing_query_body_changes_snapshot_fingerprint() {
    let yes = constant_query("q", "true");
    let no = constant_query("q", "false");
    assert_ne!(
        snapshot_names_from_module(&yes),
        snapshot_names_from_module(&no)
    );
}

#[test]
fn model_boundary_preserves_module_exclusions_when_case_adds_its_own() {
    let module = compile(
        r#"module Regression version "0.1.0" {
        outside_scope { tax, creditor_priority }
        query q() -> Bool { goal Evaluate { true } }
    }"#,
    );
    let mut case = CaseRecord::default();
    case.outside_scope = vec!["tax".into()];
    let doc = render_outcome(
        &module,
        &QueryName::from("q"),
        at(),
        at(),
        &case,
        &det(Value::Bool(true)),
    );
    let json: serde_json::Value = serde_json::from_str(&doc).expect("outcome document");
    let outside = json["modelBoundary"]["outsideScope"]
        .as_array()
        .expect("outside scope");
    assert!(
        outside
            .iter()
            .any(|v| v.as_str() == Some("creditor_priority"))
    );
}

#[test]
fn trace_ids_serialize_as_schema_strings() {
    let json = serde_json::to_value(det(Value::Bool(true))).expect("serialize outcome");
    assert!(json["trace"].is_string(), "{json}");
}

#[test]
fn outcome_fields_match_documented_camel_case() {
    let case = CaseRecord::default();
    let query = QueryName::from("q");
    let t = at();
    let answer = Value::Bool(true);
    let constraints = BTreeSet::new();
    let id = CheckedCertificate::claims_id(
        ModuleId::of(b"m"),
        SourceSnapshotId::of(b"s"),
        &case,
        &query,
        t,
        t,
        &constraints,
        &answer,
    )
    .expect("claims id");
    let cert = CheckedCertificate::verified(
        id,
        ModuleId::of(b"m"),
        SourceSnapshotId::of(b"s"),
        &case,
        &query,
        t,
        t,
        &constraints,
        &answer,
    )
    .expect("verified");
    let out = Outcome::Determinate {
        value: answer,
        trace: TraceId::of(b"test"),
        convergence_certificate: Some(cert),
        ignored_open_issues: BTreeSet::new(),
    };
    let json = serde_json::to_value(out).expect("serialize outcome");
    assert!(json.get("convergenceCertificate").is_some(), "{json}");
    assert!(json.get("ignoredOpenIssues").is_some(), "{json}");
}

#[test]
fn equal_instants_have_one_canonical_wire_representation() {
    let utc = Instant::parse("2033-01-01T00:00:00Z").expect("UTC");
    let offset = Instant::parse("2032-12-31T19:00:00-05:00").expect("offset");
    assert_eq!(utc, offset);
    assert_eq!(
        serde_json::to_string(&utc).expect("serialize UTC"),
        serde_json::to_string(&offset).expect("serialize offset"),
    );
}

#[test]
fn exploration_respects_an_already_recorded_interpretation() {
    // Counterfactual replacement should be a different, explicitly requested operation.
    let (module, _) = compile_module(&root().join("examples/trust/bryan-revocable-trust.fr"))
        .expect("compile trust fixture");
    let text = std::fs::read_to_string(root().join("examples/trust/cases/court-selects-i2.json"))
        .expect("selected interpretation fixture");
    let case: CaseRecord = serde_json::from_str(&text).expect("case");
    let out = eval_outcome(explore_query(
        &module,
        &QueryName::from("acting_trustee"),
        &case,
        &RunContext::new(at(), at()),
    ));
    assert_eq!(answer(out).display_label(), "Bob");
}
