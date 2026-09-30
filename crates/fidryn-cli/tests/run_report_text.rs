//! `run_report_text` is the `fidryn run` path as a library call: the same
//! report text on success and the same stderr text on failure.

use fidryn_cli::{CaseInput, RunRequest, compile_module, parse_instant, run_report_text};
use fidryn_core::CaseRecord;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

const EMPTY_CASE: &str = r#"{"schema":"fidryn.case-record/v0.1","admissibleCompletions":{}}"#;
const GATE_TIME: &str = "2026-09-17T12:00:00Z";
const TRUST_TIME: &str = "2034-03-01T09:00:00Z";
const TIME_HELP: &str =
    "Use ISO 8601 / RFC 3339, for example 2033-01-01T00:00:00Z or 2033-01-01T00:00:00+00:00.";

fn workspace_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

/// `fidryn run MODULE --query QUERY --case CASE --valid-at AT --known-at AT`.
fn request<'a>(
    module: &'a Path,
    query: &'a str,
    case: CaseInput<'a>,
    at: &'a str,
) -> RunRequest<'a> {
    RunRequest {
        path: module,
        query,
        case,
        valid_at: at,
        known_at: at,
        args: &[],
        scenario: false,
    }
}

fn report(text: &str) -> Value {
    let report: Value = serde_json::from_str(text).expect("report JSON");
    assert_eq!(report["schema"], "fidryn.evaluation-report/v0.1", "{text}");
    report
}

#[test]
fn require_gate_q_is_determinate_seven() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let text = run_report_text(&request(
        &module,
        "q",
        CaseInput::Json(EMPTY_CASE),
        GATE_TIME,
    ))
    .expect("run q");
    let report = report(&text);
    assert_eq!(report["outcomeDocument"]["query"], "q", "{text}");
    let outcome = &report["outcomeDocument"]["outcome"];
    assert_eq!(outcome["kind"], "determinate", "{text}");
    assert_eq!(
        outcome["value"],
        json!({"kind": "int", "data": 7}),
        "{text}"
    );
}

#[test]
fn require_gate_r_is_suspended_on_the_failed_require() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let text = run_report_text(&request(
        &module,
        "r",
        CaseInput::Json(EMPTY_CASE),
        GATE_TIME,
    ))
    .expect("run r");
    let outcome = &report(&text)["outcomeDocument"]["outcome"];
    assert_eq!(outcome["kind"], "suspended", "{text}");
    assert_eq!(
        outcome["requests"],
        json!([{"kind": "needCustom", "effect": "require", "payload": "requirement failed"}]),
        "{text}"
    );
}

#[test]
fn trust_case_file_court_selects_i2_is_determinate_bob() {
    let module = workspace_file("examples/trust/bryan-revocable-trust.fr");
    let case = workspace_file("examples/trust/cases/court-selects-i2.json");
    let text = run_report_text(&request(
        &module,
        "acting_trustee",
        CaseInput::File(&case),
        TRUST_TIME,
    ))
    .expect("run trust");
    let report = report(&text);
    assert_eq!(report["sourceTrust"], "fixture", "{text}");
    let outcome = &report["outcomeDocument"]["outcome"];
    assert_eq!(outcome["kind"], "determinate", "{text}");
    assert_eq!(
        outcome["value"],
        json!({"kind": "entity", "data": "Bob"}),
        "{text}"
    );
}

#[test]
fn bad_times_name_the_flag_and_the_accepted_forms() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let parse_err = parse_instant("yesterday").expect_err("not a timestamp");
    let mut req = request(&module, "q", CaseInput::Json(EMPTY_CASE), GATE_TIME);
    req.valid_at = "yesterday";
    assert_eq!(
        run_report_text(&req).expect_err("bad --valid-at"),
        format!("--valid-at: {parse_err}. {TIME_HELP}")
    );
    req.valid_at = GATE_TIME;
    req.known_at = "yesterday";
    assert_eq!(
        run_report_text(&req).expect_err("bad --known-at"),
        format!("--known-at: {parse_err}. {TIME_HELP}")
    );
}

#[test]
fn unknown_query_is_an_engine_failure() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let err = run_report_text(&request(
        &module,
        "no_such_query",
        CaseInput::Json(EMPTY_CASE),
        GATE_TIME,
    ))
    .expect_err("unknown query");
    assert_eq!(err, "UnknownQuery: unknown query no_such_query");
}

#[test]
fn unreadable_case_file_names_the_path() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let missing = workspace_file("examples/trust/cases/no-such-case.json");
    let io_err = std::fs::read_to_string(&missing).expect_err("case file is absent");
    let err = run_report_text(&request(&module, "q", CaseInput::File(&missing), GATE_TIME))
        .expect_err("missing case file");
    assert_eq!(err, format!("cannot read {}: {io_err}", missing.display()));
}

#[test]
fn invalid_case_json_is_an_invalid_case_record() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let bad = r#"{"schema": "#;
    let serde_err = serde_json::from_str::<CaseRecord>(bad).expect_err("truncated JSON");
    let err = run_report_text(&request(&module, "q", CaseInput::Json(bad), GATE_TIME))
        .expect_err("invalid case JSON");
    assert_eq!(err, format!("invalid case record: {serde_err}"));
}

#[test]
fn checks_run_in_the_cli_order() {
    let module = workspace_file("tests/programs/require-gate.fr");
    let orphan = ["orphan".to_owned()];
    let all_bad = RunRequest {
        path: &module,
        query: "no_such_query",
        case: CaseInput::Json("not json"),
        valid_at: "never",
        known_at: "never",
        args: &orphan,
        scenario: false,
    };

    let missing_module = workspace_file("tests/programs/no-such-module.fr");
    let diagnostics = compile_module(&missing_module).expect_err("module is absent");
    let compile_text = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let err = run_report_text(&RunRequest {
        path: &missing_module,
        ..all_bad
    })
    .expect_err("compile fails first");
    assert_eq!(err, compile_text);

    let err = run_report_text(&all_bad).expect_err("case fails second");
    assert!(err.starts_with("invalid case record: "), "{err}");

    let err = run_report_text(&RunRequest {
        case: CaseInput::Json(EMPTY_CASE),
        ..all_bad
    })
    .expect_err("--arg fails third");
    assert_eq!(
        err,
        "--arg `orphan` must be KEY=VALUE (writes case.facts[KEY])"
    );

    let err = run_report_text(&RunRequest {
        case: CaseInput::Json(EMPTY_CASE),
        args: &[],
        ..all_bad
    })
    .expect_err("--valid-at fails fourth");
    assert!(err.starts_with("--valid-at: "), "{err}");

    let err = run_report_text(&RunRequest {
        case: CaseInput::Json(EMPTY_CASE),
        args: &[],
        valid_at: GATE_TIME,
        ..all_bad
    })
    .expect_err("--known-at fails fifth");
    assert!(err.starts_with("--known-at: "), "{err}");

    let err = run_report_text(&RunRequest {
        case: CaseInput::Json(EMPTY_CASE),
        args: &[],
        valid_at: GATE_TIME,
        known_at: GATE_TIME,
        ..all_bad
    })
    .expect_err("evaluation fails last");
    assert!(err.starts_with("UnknownQuery: "), "{err}");
}

#[test]
fn fidryn_run_prints_the_same_text() {
    let module = workspace_file("examples/trust/bryan-revocable-trust.fr");
    let case = workspace_file("examples/trust/cases/court-selects-i2.json");
    let fidryn_run = |valid_at: &str| {
        Command::new(env!("CARGO_BIN_EXE_fidryn"))
            .arg("run")
            .arg(&module)
            .args(["--query", "acting_trustee", "--case"])
            .arg(&case)
            .args(["--valid-at", valid_at, "--known-at", TRUST_TIME])
            .output()
            .expect("spawn fidryn")
    };
    let req = request(
        &module,
        "acting_trustee",
        CaseInput::File(&case),
        TRUST_TIME,
    );

    let text = run_report_text(&req).expect("library run");
    let out = fidryn_run(TRUST_TIME);
    assert!(out.status.success(), "{out:?}");
    assert_eq!(String::from_utf8(out.stdout).unwrap(), format!("{text}\n"));

    let err = run_report_text(&RunRequest {
        valid_at: "yesterday",
        ..req
    })
    .expect_err("library failure");
    let out = fidryn_run("yesterday");
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    assert!(out.stdout.is_empty(), "{out:?}");
    assert_eq!(String::from_utf8(out.stderr).unwrap(), format!("{err}\n"));
}
