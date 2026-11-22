use fidryn_cli::review::{ReviewRequest, build_review};
use fidryn_core::{CaseRecord, Instant, Outcome, RunContext, Value as RuntimeValue};
use fidryn_driver::Driver;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "fidryn-review-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn write(&self, path: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn context() -> RunContext {
    let at = Instant::parse("2026-10-29T00:00:00Z").unwrap();
    RunContext::new(at, at)
}
fn decode_hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16).unwrap())
        .collect()
}
fn file<'a>(archive: &'a Value, path: &str) -> &'a Value {
    archive["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == path)
        .unwrap()
}

#[test]
fn portable_review_retains_exact_inputs_graph_spans_and_reruns() {
    let fixture = Fixture::new();
    let executable = fixture.0.join(
        std::path::Path::new(env!("CARGO_BIN_EXE_fidryn"))
            .file_name()
            .unwrap(),
    );
    fs::copy(env!("CARGO_BIN_EXE_fidryn"), &executable).unwrap();
    let source = b"// retain this comment and exact spacing\nmodule Review.Example version \"0.1.0\" {\n  query q(x: Int) -> Int { return x + 1 }\n}\n";
    let case_bytes = b"{\n  \"schema\": \"fidryn.case-record/v0.1\", \"facts\": {\"x\": 6}, \"admissibleCompletions\": {}\n}\n";
    let source_path = fixture.write("module.fr", source);
    let case_path = fixture.write("input.json", case_bytes);
    let ctx = context();
    let output = std::process::Command::new(&executable)
        .arg("review")
        .arg(&source_path)
        .args(["--query", "q", "--case"])
        .arg(&case_path)
        .args([
            "--valid-at",
            "2026-10-29T00:00:00Z",
            "--known-at",
            "2026-10-29T00:00:00Z",
            "--budget",
            "1000",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let archive: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(archive["schema"], "fidryn.review-bundle/v0.1");
    assert_eq!(
        decode_hex(
            file(&archive, "program/module.fr")["bytesHex"]
                .as_str()
                .unwrap()
        ),
        source
    );
    assert_eq!(
        decode_hex(
            file(&archive, "case/case-record.json")["bytesHex"]
                .as_str()
                .unwrap()
        ),
        case_bytes
    );
    assert_eq!(archive["report"]["schema"], "fidryn.evaluation-report/v0.2");
    assert_eq!(
        archive["report"]["sourceBundle"],
        archive["sourceBundle"]["identity"]
    );
    assert!(
        !archive["report"]["traceDocument"]["nodes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(archive["qualification"]["reviewStatus"], "not_assessed");
    let query = archive["sourceLinks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|link| link["name"] == "q")
        .unwrap();
    let start = query["startByte"].as_u64().unwrap() as usize;
    let end = query["endByte"].as_u64().unwrap() as usize;
    assert_eq!(
        query["text"],
        std::str::from_utf8(&source[start..end]).unwrap()
    );
    let mut unsigned = archive.clone();
    let digest = unsigned
        .as_object_mut()
        .unwrap()
        .remove("archiveDigest")
        .unwrap();
    assert_eq!(
        digest,
        blake3::hash(fidryn_core::canonical_json(&unsigned).unwrap().as_bytes())
            .to_hex()
            .to_string()
    );
    let restored = Fixture::new();
    for entry in archive["files"].as_array().unwrap() {
        if let Some(bytes) = entry["bytesHex"].as_str() {
            restored.write(entry["path"].as_str().unwrap(), &decode_hex(bytes));
        }
    }
    let mut driver = Driver::new();
    let (artifact, _) = driver
        .check_path_artifact_with_root(
            &restored.0.join("program/module.fr"),
            &restored.0.join("program"),
        )
        .unwrap();
    let case: CaseRecord = serde_json::from_slice(case_bytes).unwrap();
    let checked = driver
        .run_artifact_checked_with_budget(&artifact, "q", &case, &ctx, false, Some(1000))
        .unwrap();
    assert!(matches!(
        &checked.report().outcome,
        Outcome::Determinate {
            value: RuntimeValue::Int(7),
            ..
        }
    ));
    assert_eq!(
        checked.report().request.as_ref().unwrap().identity().hex(),
        archive["invocationId"]
    );
    assert_eq!(
        artifact.sources().identity().hex(),
        archive["sourceBundle"]["identity"]
    );
    let command = archive["rerun"]["arguments"].as_array().unwrap();
    let output = std::process::Command::new(&executable)
        .args(
            command
                .iter()
                .skip(1)
                .map(|argument| argument.as_str().unwrap()),
        )
        .current_dir(&restored.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rerun: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rerun["invocationId"], archive["invocationId"]);
    assert_eq!(rerun["engineArtifact"], archive["engineArtifact"]);
    assert_eq!(
        archive["report"]["engineArtifact"],
        archive["engineArtifact"]
    );
    assert_eq!(
        archive["rerun"]["engineArtifact"],
        archive["engineArtifact"]
    );
}

#[test]
fn review_supports_nested_manifest_layout_and_retains_missing_observations() {
    let fixture = Fixture::new();
    let manifest = json!({"schema":"fidryn.source-manifest/v0.1","snapshot":"S","jurisdiction":"Test","artifacts":[{"path":"sources/missing.txt","digest":"fixture","kind":"fixture","effective":"","weight":"explanatory"}]});
    fixture.write(
        "manifest.json",
        serde_json::to_string_pretty(&manifest).unwrap().as_bytes(),
    );
    let source_path = fixture.write("nested/module.fr", b"module Review.Nested version \"0.1.0\" { source_manifest \"../manifest.json\"\n query q() -> Bool { return true } }");
    let case_path = fixture.write(
        "case.json",
        b"{\"schema\":\"fidryn.case-record/v0.1\",\"admissibleCompletions\":{}}",
    );
    let ctx = context();
    let archive: Value = serde_json::from_str(
        &build_review(&ReviewRequest {
            path: &source_path,
            query: "q",
            case_path: &case_path,
            context: &ctx,
            scenario: false,
            source_root: Some(&fixture.0),
            template: None,
            budget: None,
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        archive["normalizedManifest"]["path"],
        "program/manifest.json"
    );
    for entry in archive["files"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        assert!(!path.contains('\\'), "nonportable review path: {path}");
        assert!(path.split('/').all(|part| !matches!(part, "" | "." | "..")));
    }
    assert_eq!(archive["rerun"]["modulePath"], "program/nested/module.fr");
    assert_eq!(
        archive["sourceLinks"][0]["path"],
        "program/nested/module.fr"
    );
    let missing = file(&archive, "program/sources/missing.txt");
    assert_eq!(missing["availability"], "missing");
    assert!(missing["bytesHex"].is_null());
    assert_eq!(archive["qualification"]["sourceTrust"], "fixture");
    let restored = Fixture::new();
    for entry in archive["files"].as_array().unwrap() {
        if let Some(bytes) = entry["bytesHex"].as_str() {
            restored.write(entry["path"].as_str().unwrap(), &decode_hex(bytes));
        }
    }
    assert!(!restored.0.join("program/sources/missing.txt").exists());
    let mut driver = Driver::new();
    let (artifact, _) = driver
        .check_path_artifact_with_root(
            &restored.0.join("program/nested/module.fr"),
            &restored.0.join("program"),
        )
        .unwrap();
    let case: CaseRecord =
        serde_json::from_slice(&fs::read(restored.0.join("case/case-record.json")).unwrap())
            .unwrap();
    let checked = driver
        .run_artifact_checked(&artifact, "q", &case, &ctx, false)
        .unwrap();
    assert!(matches!(
        checked.report().outcome,
        Outcome::Determinate {
            value: RuntimeValue::Bool(true),
            ..
        }
    ));
    assert_eq!(
        checked.report().request.as_ref().unwrap().identity().hex(),
        archive["invocationId"]
    );
    assert_eq!(
        artifact.sources().identity().hex(),
        archive["sourceBundle"]["identity"]
    );
}

#[test]
fn review_refuses_platform_dependent_manifest_references() {
    let fixture = Fixture::new();
    let reference = r"sources\manifest.json";
    fixture.write(
        reference,
        br#"{"schema":"fidryn.source-manifest/v0.1","snapshot":"S","jurisdiction":"Test","artifacts":[]}"#,
    );
    let source = format!(
        "module Review.Path version \"0.1.0\" {{ source_manifest {}\n query q() -> Bool {{ return true }} }}",
        serde_json::to_string(reference).unwrap()
    );
    let source_path = fixture.write("module.fr", source.as_bytes());
    let case_path = fixture.write(
        "case.json",
        br#"{"schema":"fidryn.case-record/v0.1","admissibleCompletions":{}}"#,
    );
    let ctx = context();
    let error = build_review(&ReviewRequest {
        path: &source_path,
        query: "q",
        case_path: &case_path,
        context: &ctx,
        scenario: false,
        source_root: Some(&fixture.0),
        template: None,
        budget: None,
    })
    .unwrap_err();
    assert!(error.contains("platform-dependent separators"), "{error}");
}

#[test]
fn review_scenario_and_template_do_not_become_a_legal_assessment() {
    let fixture = Fixture::new();
    let source_path = fixture.write(
        "module.fr",
        b"module Review.Scenario version \"0.1.0\" { query q(x: Int) -> Int { return x } }",
    );
    let case_path = fixture.write("case.json", serde_json::to_string(&json!({"schema":"fidryn.case-record/v0.1","facts":{"x":1},"assumptions":[{"id":"H","payload":{"x":99}}],"admissibleCompletions":{}})).unwrap().as_bytes());
    let template = fixture.write(
        "template.md",
        b"Reviewer notes:\n- assess the operative sources\n",
    );
    let ctx = context();
    let archive: Value = serde_json::from_str(
        &build_review(&ReviewRequest {
            path: &source_path,
            query: "q",
            case_path: &case_path,
            context: &ctx,
            scenario: true,
            source_root: None,
            template: Some(&template),
            budget: None,
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(archive["report"]["executionMode"], "scenario");
    assert_eq!(
        archive["report"]["outcomeDocument"]["outcome"]["value"]["data"],
        99
    );
    assert_eq!(archive["report"]["assumptions"][0]["id"], "H");
    assert_eq!(archive["qualification"]["reviewStatus"], "not_assessed");
    assert_eq!(
        decode_hex(
            file(&archive, "review/template")["bytesHex"]
                .as_str()
                .unwrap()
        ),
        fs::read(template).unwrap()
    );
}

#[test]
fn review_refuses_invalid_case_and_insufficient_execution_budget() {
    let fixture = Fixture::new();
    let source_path = fixture.write(
        "module.fr",
        b"module Review.Refusal version \"0.1.0\" { query q() -> Bool { return true } }",
    );
    let case_path = fixture.write(
        "case.json",
        b"{\"schema\":\"invalid\",\"admissibleCompletions\":{}}",
    );
    let ctx = context();
    let mut request = ReviewRequest {
        path: &source_path,
        query: "q",
        case_path: &case_path,
        context: &ctx,
        scenario: false,
        source_root: None,
        template: None,
        budget: None,
    };
    assert!(build_review(&request).is_err());
    fs::write(
        &case_path,
        b"{\"schema\":\"fidryn.case-record/v0.1\",\"admissibleCompletions\":{}}",
    )
    .unwrap();
    request.budget = Some(0);
    assert!(build_review(&request).is_err());
}
