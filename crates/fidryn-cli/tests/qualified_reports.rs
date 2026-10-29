use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "fidryn-qualified-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(
            path.join("module.fr"),
            "module Qualified version \"0.1.0\" { query q(x: Int) -> Int { return x } }",
        )
        .unwrap();
        Self(path)
    }
    fn run(&self, case: Value, scenario: bool) -> Output {
        self.run_with_budget(case, scenario, None)
    }
    fn run_with_budget(&self, case: Value, scenario: bool, budget: Option<u32>) -> Output {
        fs::write(self.0.join("case.json"), case.to_string()).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_fidryn"));
        command
            .args(["run"])
            .arg(self.0.join("module.fr"))
            .args(["--query", "q", "--case"])
            .arg(self.0.join("case.json"))
            .args([
                "--valid-at",
                "2026-01-01T00:00:00Z",
                "--known-at",
                "2026-01-01T00:00:00Z",
            ])
            .env("FIDRYN_TRACE_DIR", self.0.join("traces"));
        if scenario {
            command.arg("--scenario");
        }
        if let Some(budget) = budget {
            command.arg("--budget").arg(budget.to_string());
        }
        command.output().unwrap()
    }
}

#[test]
fn operation_budget_is_consumed_and_bound_to_the_checked_report() {
    let fixture = Fixture::new();
    let case =
        json!({"schema":"fidryn.case-record/v0.1","facts":{"x":7},"admissibleCompletions":{}});
    let exhausted = fixture.run_with_budget(case.clone(), false, Some(0));
    assert!(!exhausted.status.success());
    assert!(exhausted.stdout.is_empty());
    assert!(String::from_utf8_lossy(&exhausted.stderr).contains("FuelExhausted"));
    let bounded = report(fixture.run_with_budget(case.clone(), false, Some(1000)));
    let default = report(fixture.run(case, false));
    assert_eq!(bounded["invocation"]["budget"], 1000);
    assert_ne!(bounded["invocationId"], default["invocationId"]);
    assert_eq!(
        bounded["outcomeDocument"]["outcome"]["value"],
        default["outcomeDocument"]["outcome"]["value"]
    );
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn report(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn default_run_does_not_apply_hypotheses_and_scenario_discloses_them() {
    let fixture = Fixture::new();
    let case = json!({"schema":"fidryn.case-record/v0.1","facts":{"x":1},"assumptions":[{"id":"H","payload":{"x":99}}],"admissibleCompletions":{}});
    let operative = report(fixture.run(case.clone(), false));
    let scenario = report(fixture.run(case, true));
    assert_eq!(operative["schema"], "fidryn.evaluation-report/v0.2");
    assert_eq!(operative["executionMode"], "operative");
    assert_eq!(operative["outcomeDocument"]["outcome"]["value"]["data"], 1);
    assert_eq!(scenario["executionMode"], "scenario");
    assert_eq!(scenario["outcomeDocument"]["outcome"]["value"]["data"], 99);
    assert_eq!(scenario["assumptions"].as_array().unwrap().len(), 1);
    assert_ne!(operative["invocationId"], scenario["invocationId"]);
    let text = fidryn_cli::opinion::sentences(&scenario).join("\n");
    assert!(text.contains("scenario") && text.contains("H") && text.contains("unauthenticated"));
}

#[test]
fn different_case_snapshots_have_different_report_identities() {
    let fixture = Fixture::new();
    let a=report(fixture.run(json!({"schema":"fidryn.case-record/v0.1","facts":{"x":1,"extra":1},"admissibleCompletions":{}}),false));
    let b=report(fixture.run(json!({"schema":"fidryn.case-record/v0.1","facts":{"x":1,"extra":2},"admissibleCompletions":{}}),false));
    assert_eq!(
        a["outcomeDocument"]["outcome"]["value"],
        b["outcomeDocument"]["outcome"]["value"]
    );
    assert_ne!(a["invocation"]["case"], b["invocation"]["case"]);
    assert_ne!(a["invocationId"], b["invocationId"]);
}

#[test]
fn semantic_case_typos_fail_before_evaluation() {
    let fixture = Fixture::new();
    for case in [
        json!({"schema":"invalid/v9","facts":{"x":1},"admissibleCompletions":{}}),
        json!({"schema":"fidryn.case-record/v0.1","facts":{"x":1},"admissibleCompletions":{"choice":{"Impossible":[]}}}),
    ] {
        let output = fixture.run(case, false);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn diffs_include_clocks_model_and_coverage() {
    let fixture = Fixture::new();
    let before = report(fixture.run(
        json!({"schema":"fidryn.case-record/v0.1","facts":{"x":1},"admissibleCompletions":{}}),
        false,
    ));
    for pointer in [
        "/outcomeDocument/asOf/validTime",
        "/outcomeDocument/modelBoundary/outsideScope",
        "/coverage",
    ] {
        let mut after = before.clone();
        *after.pointer_mut(pointer).unwrap() = match pointer {
            "/coverage" => json!({"examined":1,"total":2,"incomplete":true}),
            "/outcomeDocument/modelBoundary/outsideScope" => json!(["creditors"]),
            _ => json!("2030-01-01T00:00:00Z"),
        };
        let diff = fidryn_cli::named_diff(&before, &after, "q");
        assert!(
            !diff.result.changed.is_empty() || !diff.assurance.changed.is_empty(),
            "ignored {pointer}"
        );
    }
}

#[test]
fn saved_report_explains_actual_graph_after_the_run_process_exits() {
    let fixture = Fixture::new();
    let document = report(fixture.run(
        json!({"schema":"fidryn.case-record/v0.1","facts":{"x":7},"admissibleCompletions":{}}),
        false,
    ));
    let trace = &document["traceDocument"];
    assert!(trace["nodes"].as_array().unwrap().len() > 1);
    assert!(
        trace["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["kind"] == "Invocation")
    );
    let saved = fixture.0.join("report.json");
    fs::write(&saved, document.to_string()).unwrap();
    fs::remove_file(fixture.0.join("module.fr")).unwrap();
    fs::remove_file(fixture.0.join("case.json")).unwrap();
    let explained = Command::new(env!("CARGO_BIN_EXE_fidryn"))
        .arg("explain")
        .arg(saved)
        .args(["--format", "json"])
        .output()
        .unwrap();
    let graph = report(explained);
    assert_eq!(graph["trace"], trace["trace"]);
    assert_eq!(graph["nodes"], trace["nodes"]);
}

#[test]
fn cli_render_keeps_typed_records_and_names_with_commas() {
    let fixture = Fixture::new();
    let template = fixture.0.join("template.txt");
    let data = fixture.0.join("data.json");
    fs::write(&template, "{{#each data.people}}{{name}}|{{/each}}").unwrap();
    fs::write(
        &data,
        json!({"people":[{"name":"Ada, Esq."},{"name":"Ben"}]}).to_string(),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_fidryn"))
        .arg("render")
        .arg(fixture.0.join("module.fr"))
        .arg("--template")
        .arg(template)
        .arg("--data")
        .arg(data)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "Ada, Esq.|Ben|");
}
