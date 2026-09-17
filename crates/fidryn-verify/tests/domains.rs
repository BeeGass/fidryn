//! v0.1 domain and diagnostic acceptance tests.

use fidryn_check::check;
use fidryn_core::{
    CaseRecord, DiagnosticCode, OpenRequest, Outcome, QueryName, RunContext, SourceManifest, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use fidryn_hir::elaborate;
use fidryn_syntax::parse_file;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile(rel: &str) -> fidryn_core::CoreModule {
    let src = fs::read_to_string(root().join(rel)).unwrap();
    let parsed = parse_file(&src);
    assert!(!parsed.has_errors(), "{rel}: {:?}", parsed.diagnostics);
    let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
    check(&hir, &SourceManifest::default()).unwrap()
}

fn load_case(rel: &str) -> CaseRecord {
    let text = fs::read_to_string(root().join(rel)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn now() -> fidryn_core::Instant {
    fidryn_core::Instant::parse("2034-03-01T09:00:00Z").unwrap()
}

fn run(module: &fidryn_core::CoreModule, query: &str, case: &CaseRecord) -> Outcome {
    let t = now();
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
    };
    evaluate(
        module,
        &QueryName::from(query),
        &Default::default(),
        &state,
        &RunContext::new(t, t),
        &mut handler,
        case,
    )
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
    let module = compile("examples/prenup/ava-noah.fidryn");
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
    let module = compile("examples/prenup/ava-noah.fidryn");
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
    let module = compile("examples/foia/foia-request.fidryn");
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
    let module = compile("examples/massachusetts-llc/harbor-robotics.fidryn");
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
    let module = compile("examples/massachusetts-llc/harbor-robotics.fidryn");
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
    let err = check_file("tests/diagnostics/e410-duplicate-rank.fidryn");
    assert!(
        err.iter().any(|d| d.code == DiagnosticCode::E410),
        "{err:?}"
    );
}

#[test]
fn missing_goal_file_is_e430() {
    let err = check_file("tests/diagnostics/e430-missing-goal.fidryn");
    assert!(
        err.iter().any(|d| d.code == DiagnosticCode::E430),
        "{err:?}"
    );
}

#[test]
fn prop_as_guard_file_is_e310() {
    let err = check_file("tests/diagnostics/e310-prop-as-guard.fidryn");
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
