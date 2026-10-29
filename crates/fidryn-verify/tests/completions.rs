//! v0.1 no-false-determinacy acceptance tests.

use fidryn_core::{CaseRecord, Outcome, QueryName, RunContext, Value};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use fidryn_verify::{
    Determinacy, PropertyVerdict, check_determinacy, explore_query, skeptical, verify_property,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn compile_trust() -> fidryn_core::CoreModule {
    fidryn_driver::Driver::new()
        .check(&root().join("examples/trust/bryan-revocable-trust.fr"))
        .unwrap_or_else(|diagnostics| panic!("trust source failed: {diagnostics:?}"))
}

fn load_case(name: &str) -> CaseRecord {
    let text = fs::read_to_string(root().join("examples/trust/cases").join(name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn at() -> fidryn_core::Instant {
    fidryn_core::Instant::parse("2026-08-24T12:00:00Z").unwrap()
}

trait IntoWorldOutcome {
    fn into_world_outcome(self) -> Outcome<Value>;
}

#[allow(dead_code)]
impl IntoWorldOutcome for Outcome<Value> {
    fn into_world_outcome(self) -> Self {
        self
    }
}

#[allow(dead_code)]
impl<E: std::fmt::Display> IntoWorldOutcome for Result<Outcome<Value>, E> {
    fn into_world_outcome(self) -> Outcome<Value> {
        self.unwrap_or_else(|err| panic!("engine error is not a legal world: {err}"))
    }
}

fn eval_out(module: &fidryn_core::CoreModule, query: &str, case: &CaseRecord) -> Outcome<Value> {
    let t = at();
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(t),
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
    .into_world_outcome()
}

#[test]
fn trust_module_parses() {
    compile_trust();
}

#[test]
fn one_certificate_without_checked_certificate_is_suspended() {
    let module = compile_trust();
    let case = load_case("one-certificate.json");
    let out = eval_out(&module, "acting_trustee", &case);
    assert!(
        matches!(out, Outcome::Suspended { .. }),
        "ignored open issues without a CheckedCertificate must be Suspended: {out:?}"
    );
}

#[test]
fn uncertified_open_branch_is_suspended() {
    let module = compile_trust();
    let case = load_case("bob-plus-open-alice.json");
    let out = eval_out(&module, "acting_trustee", &case);
    assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
}

#[test]
fn two_interpretations_are_contingent() {
    let module = compile_trust();
    let case = load_case("two-certificates-open-eligibility.json");
    let t = at();
    let ctx = RunContext::new(t, t);
    let out =
        explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx).expect("explore");
    match out {
        Outcome::Contingent { alternatives, .. } => {
            let labels: BTreeSet<String> =
                alternatives.values().map(Value::display_label).collect();
            assert!(labels.contains("Alice"), "{alternatives:?}");
            assert!(labels.contains("Bob"), "{alternatives:?}");
            assert_eq!(alternatives.len(), 2, "{alternatives:?}");
        }
        other => panic!("{other:?}"),
    }
    let det = check_determinacy(&module, &QueryName::from("acting_trustee"), &case, &ctx)
        .expect("determinacy");
    match det {
        Determinacy::Counterexample { va, vb, .. } => {
            let labels = [va.display_label(), vb.display_label()];
            assert!(labels.contains(&"Alice".to_owned()), "{labels:?}");
            assert!(labels.contains(&"Bob".to_owned()), "{labels:?}");
            assert_ne!(va, vb);
        }
        Determinacy::Convergent { value, .. } => {
            panic!("two SuccessorEligibility interpretations must not be Convergent: {value:?}")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn court_selects_i2_determinate_bob() {
    let module = compile_trust();
    let case = load_case("court-selects-i2.json");
    let t = at();
    let out = eval_out(&module, "acting_trustee", &case);
    match out {
        Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
        other => panic!("{other:?}"),
    }
    let ctx = RunContext::new(t, t);
    let explored =
        explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx).expect("explore");
    match explored {
        Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
        other => panic!("recorded I2 must constrain explore: {other:?}"),
    }
    let occupancy_case = load_case("court-selects-i2.json");
    assert!(
        occupancy_case
            .evidence
            .iter()
            .any(|item| item.schema == "OccupancyRecord"),
        "court-selects-i2 must supply OccupancyRecord"
    );
    let det = check_determinacy(
        &module,
        &QueryName::from("acting_trustee"),
        &occupancy_case,
        &ctx,
    )
    .expect("determinacy");
    match det {
        Determinacy::Convergent { value, .. } => {
            assert_eq!(value.display_label(), "Bob");
        }
        other => panic!("recorded I2 with OccupancyRecord must be Convergent Bob: {other:?}"),
    }
}

#[test]
fn verify_property_query_name_is_invalid() {
    let module = compile_trust();
    match verify_property(&module, "acting_trustee") {
        PropertyVerdict::InvalidProperty { diagnostics } => {
            assert!(
                diagnostics.contains("acting_trustee") || diagnostics.contains("query"),
                "{diagnostics}"
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn skeptical_preserves_suspended_open_branch() {
    let module = compile_trust();
    let case = load_case("bob-plus-open-alice.json");
    let t = at();
    let out = skeptical(
        &module,
        &QueryName::from("acting_trustee"),
        &case,
        &RunContext::new(t, t),
    )
    .expect("skeptical");
    assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
}

#[test]
fn stale_physician_records_do_not_authorize_later_succession() {
    let module = compile_trust();
    let case = load_case("court-selects-i2.json");
    let at = fidryn_core::Instant::parse("2034-03-01T09:00:00Z").unwrap();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(at),
    };
    let outcome = evaluate(
        &module,
        &QueryName::from("acting_trustee"),
        &Default::default(),
        &case.into_state(),
        &RunContext::new(at, at),
        &mut handler,
        &case,
    )
    .unwrap();
    assert!(
        matches!(outcome, Outcome::Suspended { .. }),
        "expired source-policy evidence must remain unresolved: {outcome:?}"
    );
}
