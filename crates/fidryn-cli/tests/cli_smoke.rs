use clap::{CommandFactory, Parser};
use fidryn_cli::{
    Cli, Command, SnapshotDiff, apply_run_args, compile_module, compile_source, load_manifest,
    parse_instant, snapshot_names_from_json, snapshot_names_from_module,
};
use fidryn_core::{CaseRecord, DiagnosticCode, SourceManifest, Value, canonical_json};
use fidryn_render::{module_vars, render};
use std::path::PathBuf;

fn workspace_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

#[test]
fn cli_subcommands_exist() {
    let names: Vec<String> = Cli::command()
        .get_subcommands()
        .map(|c| c.get_name().to_string())
        .collect();
    for expected in [
        "fmt", "check", "run", "explore", "explain", "verify", "diff", "render", "file", "ui",
    ] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing subcommand {expected} in {names:?}"
        );
    }
}

#[test]
fn run_parses_provision_arg() {
    let cli = Cli::try_parse_from([
        "fidryn",
        "run",
        "mod.fr",
        "--query",
        "provision_result",
        "--case",
        "case.json",
        "--valid-at",
        "2033-01-01T00:00:00Z",
        "--known-at",
        "2033-01-01T00:00:00+00:00",
        "--arg",
        "provision=ChildSupportWaiver",
    ])
    .expect("parse run");
    match cli.command {
        Command::Run { args, .. } => {
            assert_eq!(args, vec!["provision=ChildSupportWaiver"]);
            let mut case = CaseRecord::default();
            apply_run_args(&mut case, &args);
            assert_eq!(
                case.facts.get("provision"),
                Some(&Value::String("ChildSupportWaiver".into()))
            );
        }
        other => panic!("expected run, got {other:?}"),
    }
}

#[test]
fn parse_instant_accepts_z_and_numeric_offsets() {
    parse_instant("2033-01-01T00:00:00Z").expect("Z suffix");
    parse_instant("2033-01-01T00:00:00+00:00").expect("numeric +00:00");
    parse_instant("2026-08-23T12:00:00-04:00").expect("numeric -04:00");
    assert!(parse_instant("not-a-timestamp").is_err());
}

#[test]
fn compile_source_accepts_a_query_with_a_goal() {
    let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
    compile_source(src, &SourceManifest::default()).expect("compile");
}

#[test]
fn ui_parses_port_and_no_open() {
    let cli =
        Cli::try_parse_from(["fidryn", "ui", "--port", "9000", "--no-open"]).expect("parse ui");
    match cli.command {
        Command::Ui { port, no_open } => {
            assert_eq!(port, Some(9000));
            assert!(no_open);
        }
        other => panic!("expected ui, got {other:?}"),
    }
}

#[test]
fn compile_diagnostics_use_display_and_exit_path() {
    let path = workspace_file("tests/diagnostics/e310-prop-as-guard.fr");
    let err = compile_module(&path).expect_err("E310 module must fail");
    assert!(
        err.iter().any(|d| d.code == DiagnosticCode::E310),
        "expected E310, got {err:?}"
    );
    let text = format!("{}", err[0]);
    assert!(
        text.contains("E310"),
        "Diagnostic Display should include the code: {text}"
    );
}

#[test]
fn render_certified_outline_interpolates_module_vars() {
    let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
    let module = compile_source(src, &SourceManifest::default()).expect("compile");
    let template =
        std::fs::read_to_string(workspace_file("templates/certified/instrument-outline.txt"))
            .expect("certified template");
    let text = render(&template, &module_vars(&module)).expect("render");
    assert!(text.contains("Examples.T"), "{text}");
    assert!(text.contains("0.1.0"), "{text}");
    assert!(matches!(
        render("{{missing}}", &module_vars(&module)),
        Err(fidryn_render::RenderError::MissingKey(_))
    ));
}

#[test]
fn diff_json_outcomes_marks_changed_query() {
    let old = serde_json::json!({
        "module": "Examples.T@0.1.0",
        "query": "q",
        "outcome": {"kind": "determinate", "trace": "aa"}
    });
    let new = serde_json::json!({
        "module": "Examples.T@0.1.0",
        "query": "q",
        "outcome": {"kind": "suspended", "trace": "bb"}
    });
    let diff = SnapshotDiff::from_maps(
        &snapshot_names_from_json(&old, "q"),
        &snapshot_names_from_json(&new, "q"),
    );
    assert_eq!(diff.changed, vec!["q".to_string()]);
    let text = canonical_json(&diff).unwrap();
    assert!(text.contains("\"changed\":[\"q\"]"), "{text}");
}

#[test]
fn snapshot_names_from_compiled_module_include_queries() {
    let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
    let module = compile_source(src, &SourceManifest::default()).expect("compile");
    let names = snapshot_names_from_module(&module);
    assert!(names.contains_key("Examples.T"));
    assert!(names.contains_key("q"));
}

#[test]
fn source_manifest_header_is_actually_loaded() {
    let path = workspace_file("examples/trust/bryan-revocable-trust.fr");
    let src = std::fs::read_to_string(&path).expect("read trust module");
    let loaded = load_manifest(&path, &src).expect("load declared source_manifest");
    let expected: SourceManifest = serde_json::from_str(
        &std::fs::read_to_string(workspace_file(
            "examples/trust/sources/ma-trust-fixture.manifest.json",
        ))
        .expect("read fixture manifest"),
    )
    .expect("parse fixture manifest");
    assert_eq!(loaded, expected);
    assert!(!loaded.snapshot.is_empty());
    assert_eq!(loaded.snapshot, "2026-08-23-ma-trust-fixture");
    assert_eq!(loaded.artifacts[0].digest, "fixture");
    let compiled = compile_module(&path);
    match compiled {
        Ok((_, manifest)) => {
            assert_eq!(
                manifest, expected,
                "compile_module must use the declared manifest"
            );
        }
        Err(diagnostics) => {
            panic!(
                "compiling bryan-revocable-trust.fr should succeed with a loaded manifest; got {diagnostics:?}"
            );
        }
    }
}

#[test]
fn evaluate_true_vs_false_changes_query_fingerprint() {
    let src_true = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
    let src_false = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { false }
    }
}
"#;
    let module_true = compile_source(src_true, &SourceManifest::default()).expect("compile true");
    let module_false =
        compile_source(src_false, &SourceManifest::default()).expect("compile false");
    assert_ne!(
        format!("{:?}", module_true.queries[0].plan),
        format!("{:?}", module_false.queries[0].plan),
        "Evaluate {{ true }} and Evaluate {{ false }} must lower to distinct plans"
    );
    let a = snapshot_names_from_module(&module_true);
    let b = snapshot_names_from_module(&module_false);
    assert_ne!(
        a.get("q"),
        b.get("q"),
        "changing Evaluate {{ true }} to false must change the query fingerprint"
    );
}
