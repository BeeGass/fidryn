use fidryn_core::CaseRecord;
use serde_json::json;

#[test]
fn case_versions_and_semantic_typos_are_rejected() {
    for value in [
        json!({"schema":"wrong/v999","admissibleCompletions":{}}),
        json!({"schema":"fidryn.case-record/v0.1","admissibleCompletions":{"choice":{"Impossible":[]}}}),
        json!({"schema":"fidryn.case-record/v0.1","admissibleCompletions":{},"outside_scope":["creditors"]}),
        json!({"schema":"fidryn.case-record/v0.1","admissibleCompletions":{},"evidence":[{"schema":"S","value":true,"observedAt":"2026-01-01T00:00:00Z","subjectTypo":"A"}]}),
    ] {
        assert!(
            serde_json::from_value::<CaseRecord>(value.clone()).is_err(),
            "accepted {value}"
        );
    }
}

#[test]
fn supported_hypotheses_and_recorded_time_spelling_are_preserved() {
    let value = json!({"schema":"fidryn.case-record/v0.1","admissibleCompletions":{},
        "assumptions":[{"id":"H","payload":{"flag":true}}],
        "determinations":[{"issue":"P","protocol":"Court","established":true,"decider":"D","recordedAt":"2026-01-01T00:00:00Z"}]});
    let case: CaseRecord = serde_json::from_value(value).unwrap();
    assert_eq!(case.assumptions.len(), 1);
    assert!(case.determinations[0].recorded_at.is_some());
}

#[test]
fn manifest_and_package_versions_and_semantic_typos_are_rejected() {
    let manifest = json!({"schema":"fidryn.source-manifest/v0.1","snapshot":"s","jurisdiction":"J","artifacts":[]});
    assert!(serde_json::from_value::<fidryn_core::SourceManifest>(manifest.clone()).is_ok());
    for (field, value) in [("schema", json!("manifest/v999")), ("artifact", json!([]))] {
        let mut changed = manifest.clone();
        changed[field] = value;
        assert!(serde_json::from_value::<fidryn_core::SourceManifest>(changed).is_err());
    }
    let lock = json!({"schema":"fidryn.package-lock/v0.1","name":"logic","version":"1","digest":"fixture"});
    assert!(serde_json::from_value::<fidryn_core::PackageLock>(lock.clone()).is_ok());
    for (field, value) in [("schema", json!("lock/v999")), ("digset", json!("fixture"))] {
        let mut changed = lock.clone();
        changed[field] = value;
        assert!(serde_json::from_value::<fidryn_core::PackageLock>(changed).is_err());
    }
}
