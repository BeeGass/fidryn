use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "fidryn-temporal-records-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("module.fr"), source).unwrap();
        Self(path)
    }
    fn run(&self, facts: Value) -> Output {
        fs::write(
            self.0.join("case.json"),
            json!({"schema":"fidryn.case-record/v0.1","facts":facts,"admissibleCompletions":{}})
                .to_string(),
        )
        .unwrap();
        Command::new(env!("CARGO_BIN_EXE_fidryn"))
            .arg("run")
            .arg(self.0.join("module.fr"))
            .args(["--query", "evaluate", "--case"])
            .arg(self.0.join("case.json"))
            .args([
                "--valid-at",
                "2026-01-01T00:00:00Z",
                "--known-at",
                "2026-01-01T00:00:00Z",
            ])
            .env("FIDRYN_TRACE_DIR", self.0.join("traces"))
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn value(output: Output) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["outcomeDocument"]["outcome"]["kind"], "determinate");
    report["outcomeDocument"]["outcome"]["value"].clone()
}
fn instant(text: &str) -> Value {
    json!({"kind":"instant","data":text})
}

#[test]
fn unfamiliar_time_queries_order_instants_and_normalize_offsets() {
    for (operator, earlier, equal, later) in [
        ("<", true, false, false),
        ("<=", true, true, false),
        (">", false, false, true),
        (">=", false, true, true),
    ] {
        let fixture = Fixture::new(&format!(
            "module Chronology version \"0.1.0\" {{ query automatic evaluate(a: Time, b: Time) -> Bool {{ return a {operator} b }} }}"
        ));
        for (a, expected) in [
            ("2025-12-31T23:59:59Z", earlier),
            ("2026-01-01T01:00:00+01:00", equal),
            ("2026-01-01T00:00:01Z", later),
        ] {
            assert_eq!(
                value(fixture.run(json!({"a":instant(a),"b":instant("2026-01-01T00:00:00Z")})))["data"],
                expected
            );
        }
        let wrong_type =
            fixture.run(json!({"a":"not-an-instant","b":instant("2026-01-01T00:00:00Z")}));
        assert!(!wrong_type.status.success());
        assert!(wrong_type.stdout.is_empty());
    }
}

#[test]
fn named_record_constructors_execute_through_nested_functions() {
    let fixture = Fixture::new(
        r#"
module UnfamiliarRecords version "0.1.0" {
    record_type Measurement { amount: Int; accepted: Bool }
    calc pack(x: Int) -> Measurement { return Measurement(accepted = true, amount = x + 1) }
    query automatic evaluate(x: Int) -> Measurement { return pack(x) }
}
"#,
    );
    let result = value(fixture.run(json!({"x":41})));
    assert_eq!(result["data"]["amount"]["data"], 42);
    assert_eq!(result["data"]["accepted"]["data"], true);
    let wrong = fixture.run(json!({"x":"41"}));
    assert!(!wrong.status.success());
}

#[test]
fn unknown_calls_and_bad_record_fields_still_fail() {
    for expression in [
        "Unknown(amount = 1)",
        "Measurement(amount = true, accepted = true)",
        "Measurement(amount = 1, extra = true)",
        "Measurement(amount = 1, amount = 2)",
    ] {
        let fixture = Fixture::new(&format!(
            "module InvalidRecords version \"0.1.0\" {{ record_type Measurement {{ amount: Int; accepted: Bool }} query evaluate() -> Measurement {{ return {expression} }} }}"
        ));
        let output = fixture.run(json!({}));
        assert!(!output.status.success(), "accepted {expression}");
        assert!(output.stdout.is_empty());
    }
}
