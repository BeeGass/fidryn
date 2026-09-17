//! v0.1 no-false-determinacy acceptance tests.

use fidryn_check::check;
use fidryn_core::{CaseRecord, Outcome, QueryName, RunContext, SourceManifest, Value};
use fidryn_eval::{evaluate, seed_initial_occupancy};
use fidryn_handlers::CaseFile;
use fidryn_hir::elaborate;
use fidryn_syntax::parse_file;
use fidryn_verify::explore_query;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile_trust() -> fidryn_core::CoreModule {
    let src =
        fs::read_to_string(root().join("examples/trust/bryan-revocable-trust.fidryn")).unwrap();
    let parsed = parse_file(&src);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let hir = elaborate(&parsed, &SourceManifest::default()).unwrap();
    check(&hir, &SourceManifest::default()).unwrap()
}

fn load_case(name: &str) -> CaseRecord {
    let text = fs::read_to_string(root().join("examples/trust/cases").join(name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn at() -> fidryn_core::Instant {
    fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap()
}

#[test]
fn trust_module_parses() {
    compile_trust();
}

#[test]
fn one_certificate_determinate_bryan() {
    let module = compile_trust();
    let case = load_case("one-certificate.json");
    let t = at();
    let mut state = case.into_state();
    seed_initial_occupancy(&mut state, "Bryan", "TrusteeOf(BRT)", t);
    let mut handler = CaseFile {
        record: case.clone(),
    };
    let out = evaluate(
        &module,
        &QueryName::from("acting_trustee"),
        &Default::default(),
        &state,
        &RunContext::new(t, t),
        &mut handler,
        &case,
    );
    match out {
        Outcome::Determinate {
            value,
            ignored_open_issues,
            convergence_certificate,
            ..
        } => {
            assert_eq!(value.display_label(), "Bryan");
            assert!(!ignored_open_issues.is_empty());
            assert!(convergence_certificate.is_some());
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn uncertified_open_branch_is_suspended() {
    let module = compile_trust();
    let case = load_case("bob-plus-open-alice.json");
    let t = at();
    let mut state = case.into_state();
    seed_initial_occupancy(&mut state, "Bryan", "TrusteeOf(BRT)", t);
    let mut handler = CaseFile {
        record: case.clone(),
    };
    let out = evaluate(
        &module,
        &QueryName::from("acting_trustee"),
        &Default::default(),
        &state,
        &RunContext::new(t, t),
        &mut handler,
        &case,
    );
    assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
}

#[test]
fn two_interpretations_are_contingent() {
    let module = compile_trust();
    let case = load_case("two-certificates-open-eligibility.json");
    let t = at();
    let out = explore_query(
        &module,
        &QueryName::from("acting_trustee"),
        &case,
        &RunContext::new(t, t),
    );
    match out {
        Outcome::Contingent { alternatives, .. } => {
            assert_eq!(
                alternatives.get("I1").map(Value::display_label).as_deref(),
                Some("Alice")
            );
            assert_eq!(
                alternatives.get("I2").map(Value::display_label).as_deref(),
                Some("Bob")
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn court_selects_i2_determinate_bob() {
    let module = compile_trust();
    let case = load_case("court-selects-i2.json");
    let t = at();
    let mut state = case.into_state();
    seed_initial_occupancy(&mut state, "Bryan", "TrusteeOf(BRT)", t);
    let mut handler = CaseFile {
        record: case.clone(),
    };
    let out = evaluate(
        &module,
        &QueryName::from("acting_trustee"),
        &Default::default(),
        &state,
        &RunContext::new(t, t),
        &mut handler,
        &case,
    );
    match out {
        Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
        other => panic!("{other:?}"),
    }
}
