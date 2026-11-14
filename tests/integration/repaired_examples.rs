//! Example modules repaired by typing-only edits.
//!
//! The first test is the list. The others pin what the repaired constructs
//! do at run time, so the guides can describe them.

use fidryn_cli::{CaseInput, RunRequest, compile_module, run_report_text};
use serde_json::Value;
use std::path::PathBuf;

const REPAIRED: [&str; 34] = [
    "examples/california/abc-test/abc-test.fr",
    "examples/california/final-pay/final-pay.fr",
    "examples/california/lemon/lemon.fr",
    "examples/california/noncompete/noncompete.fr",
    "examples/delaware/dgcl-220/dgcl-220.fr",
    "examples/everyday/final-paycheck/final-paycheck.fr",
    "examples/everyday/minimum-wage/minimum-wage.fr",
    "examples/states/arkansas/homestead.fr",
    "examples/states/colorado/famli.fr",
    "examples/states/florida/homestead.fr",
    "examples/states/idaho/noncompete.fr",
    "examples/states/kansas/homestead.fr",
    "examples/states/louisiana/community-property.fr",
    "examples/states/michigan/no-fault-pip.fr",
    "examples/states/south-dakota/wage-payment.fr",
    "examples/texas/homestead/homestead.fr",
    "examples/ucc/battle-of-forms/battle-of-forms.fr",
    "examples/us-federal/accredited-investor/accredited-investor.fr",
    "examples/us-federal/ada-service-animal/ada-service-animal.fr",
    "examples/us-federal/bankruptcy-stay/bankruptcy-stay.fr",
    "examples/us-federal/diversity/diversity.fr",
    "examples/us-federal/dmca/dmca.fr",
    "examples/us-federal/fair-housing/fair-housing.fr",
    "examples/us-federal/flsa/flsa.fr",
    "examples/us-federal/fmla/fmla.fr",
    "examples/us-federal/garnishment/garnishment.fr",
    "examples/us-federal/hearsay/hearsay.fr",
    "examples/us-federal/means-test/means-test.fr",
    "examples/us-federal/pslf/pslf.fr",
    "examples/us-federal/scra/scra.fr",
    "examples/us-federal/ssa-fra/ssa-fra.fr",
    "examples/us-federal/ssi/ssi.fr",
    "examples/us-federal/tcpa/tcpa.fr",
    "examples/us-federal/unlawful-presence/unlawful-presence.fr",
];

const AT: &str = "2026-09-17T19:00:00Z";
const NO_FACTS: &str =
    r#"{"schema":"fidryn.case-record/v0.1","facts":{},"admissibleCompletions":{}}"#;
const FINAL_PAY: &str = "examples/california/final-pay/final-pay.fr";
const BATTLE_OF_FORMS: &str = "examples/ucc/battle-of-forms/battle-of-forms.fr";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The outcome object of a run, or the text `fidryn run` writes to stderr.
fn outcome(module: &str, query: &str, case: &str, args: &[String]) -> Result<Value, String> {
    let path = root().join(module);
    let text = run_report_text(&RunRequest {
        path: &path,
        query,
        case: CaseInput::Json(case),
        valid_at: AT,
        known_at: AT,
        args,
        scenario: false,
    })?;
    let report: Value = serde_json::from_str(&text).expect("report is JSON");
    Ok(report["outcomeDocument"]["outcome"].clone())
}

fn final_pay_case(determinations: &str) -> String {
    format!(
        r#"{{"schema":"fidryn.case-record/v0.1","facts":{{"discharged":true}},"determinations":{determinations},"admissibleCompletions":{{}}}}"#
    )
}

fn willful(established: bool) -> String {
    final_pay_case(&format!(
        r#"[{{"issue":"WillfulFailureToPay(Employer)","protocol":"Determine","established":{established},"decider":"Court"}}]"#
    ))
}

#[test]
fn every_repaired_module_passes_the_checker() {
    for module in REPAIRED {
        if let Err(diagnostics) = compile_module(&root().join(module)) {
            panic!("{module}: {diagnostics:?}");
        }
    }
}

#[test]
fn a_judgment_query_suspends_until_a_determination_is_on_file() {
    let out = outcome(
        FINAL_PAY,
        "waiting_time_penalty",
        &final_pay_case("[]"),
        &[],
    )
    .unwrap();
    assert_eq!(out["kind"], "suspended");
    assert_eq!(out["requests"][0]["kind"], "needJudgment");
    assert_eq!(
        out["requests"][0]["issue"]["predicate"],
        "WillfulFailureToPay"
    );
}

#[test]
fn an_established_determination_makes_the_judgment_query_true() {
    let out = outcome(FINAL_PAY, "waiting_time_penalty", &willful(true), &[]).unwrap();
    assert_eq!(out["kind"], "determinate");
    assert_eq!(out["value"]["data"], true);
}

#[test]
fn a_negative_determination_suspends_and_never_answers_false() {
    let out = outcome(FINAL_PAY, "waiting_time_penalty", &willful(false), &[]).unwrap();
    assert_eq!(out["kind"], "suspended");
    assert_eq!(out["requests"][0]["payload"], "requirement failed");
}

#[test]
fn a_query_with_a_bound_parameter_asks_for_its_argument() {
    assert_eq!(
        outcome(BATTLE_OF_FORMS, "material_alteration", NO_FACTS, &[]).unwrap_err(),
        "InvalidInput: invalid input: missing query argument `term`"
    );
    let args = ["term=arbitration".to_owned()];
    let out = outcome(BATTLE_OF_FORMS, "material_alteration", NO_FACTS, &args).unwrap();
    assert_eq!(out["kind"], "suspended");
    assert_eq!(out["requests"][0]["kind"], "needJudgment");
    assert_eq!(
        out["requests"][0]["issue"]["arguments"][0]["String"],
        "arbitration"
    );
}

#[test]
fn money_facts_in_the_repaired_case_records_are_accepted() {
    for (module, query, case) in [
        (
            "examples/us-federal/accredited-investor/accredited-investor.fr",
            "individual_income_met",
            "examples/us-federal/accredited-investor/cases/income-met.json",
        ),
        (
            "examples/us-federal/diversity/diversity.fr",
            "amount_in_controversy_met",
            "examples/us-federal/diversity/cases/amount-met-diversity-open.json",
        ),
        (
            "examples/us-federal/fair-housing/fair-housing.fr",
            "extra_deposit_not_required",
            "examples/us-federal/fair-housing/cases/no-extra-deposit.json",
        ),
        (
            "examples/us-federal/garnishment/garnishment.fr",
            "within_twenty_five_percent",
            "examples/us-federal/garnishment/cases/under-cap.json",
        ),
        (
            "examples/us-federal/means-test/means-test.fr",
            "above_median",
            "examples/us-federal/means-test/cases/above-median.json",
        ),
        (
            "examples/us-federal/ssi/ssi.fr",
            "resources_within_cap",
            "examples/us-federal/ssi/cases/under-cap.json",
        ),
        (
            "examples/us-federal/tcpa/tcpa.fr",
            "at_least_statutory_floor",
            "examples/us-federal/tcpa/cases/statutory-floor.json",
        ),
    ] {
        let record = std::fs::read_to_string(root().join(case)).expect(case);
        let out = outcome(module, query, &record, &[])
            .unwrap_or_else(|error| panic!("{module}: {error}"));
        assert_eq!(out["kind"], "determinate", "{module}");
        assert_eq!(out["value"]["data"], true, "{module}");
    }
}

#[test]
fn a_module_that_fails_the_checker_cannot_be_run() {
    let error = outcome(
        "examples/us-federal/removal/removal.fr",
        "thirty_day_window",
        NO_FACTS,
        &[],
    )
    .unwrap_err();
    assert!(error.contains("E210"), "{error}");
}
