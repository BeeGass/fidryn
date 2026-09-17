//! Case records declare their admissible completion space.

use crate::state::LegalState;
use crate::time::{Instant, Interval};
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaseRecord {
    pub schema: String,
    pub module: Option<String>,
    #[serde(default, with = "crate::value::case_value_map")]
    pub facts: BTreeMap<String, Value>,
    #[serde(default)]
    pub evidence: Vec<EvidenceItem>,
    #[serde(default)]
    pub events: Vec<LedgerEvent>,
    #[serde(default)]
    pub determinations: Vec<CaseDetermination>,
    #[serde(default)]
    pub interpretations: BTreeMap<String, String>,
    #[serde(default)]
    pub decisions: BTreeMap<String, String>,
    #[serde(default)]
    pub closures: Vec<ClosureRecord>,
    pub admissible_completions: AdmissibleCompletions,
    #[serde(default)]
    pub outside_scope: Vec<String>,
}

impl Default for CaseRecord {
    fn default() -> Self {
        Self {
            schema: "fidryn.case-record/v0.1".into(),
            module: None,
            facts: BTreeMap::new(),
            evidence: Vec::new(),
            events: Vec::new(),
            determinations: Vec::new(),
            interpretations: BTreeMap::new(),
            decisions: BTreeMap::new(),
            closures: Vec::new(),
            admissible_completions: AdmissibleCompletions::default(),
            outside_scope: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub schema: String,
    #[serde(with = "crate::value::case_value")]
    pub value: Value,
    pub observed_at: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEvent {
    /// `"evidence"` | `"duty"` | `"authority"` | `"correction"` | `"retraction"`
    pub kind: String,
    pub valid_time: Interval,
    pub record_time: Instant,
    #[serde(with = "crate::value::case_value")]
    pub payload: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseDetermination {
    pub issue: String,
    pub protocol: String,
    pub established: bool,
    pub decider: String,
    #[serde(default)]
    pub recorded_at: Option<Instant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureRecord {
    pub domain: String,
    pub closed: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissibleCompletions {
    #[serde(default)]
    pub interpretations: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub evidence: BTreeMap<String, CompletionDomain>,
    #[serde(default)]
    pub choices: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionDomain {
    pub responses: Vec<String>,
    #[serde(default)]
    pub effect_on_valid_time: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelBoundary {
    pub outside_scope: Vec<String>,
    pub admissible_completions: AdmissibleCompletions,
}

impl CaseRecord {
    pub fn model_boundary(&self) -> ModelBoundary {
        ModelBoundary {
            outside_scope: self.outside_scope.clone(),
            admissible_completions: self.admissible_completions.clone(),
        }
    }

    pub fn into_state(&self) -> LegalState {
        let mut state = LegalState::new();
        state.sources.snapshot = self.module.clone().unwrap_or_default();
        for item in &self.evidence {
            state.record.items.push(crate::state::RecordEntry {
                schema: item.schema.clone(),
                value: item.value.clone(),
                observed_at: item.observed_at,
            });
        }
        for event in &self.events {
            if event.kind == "evidence" {
                state.record.items.push(crate::state::RecordEntry {
                    schema: event.kind.clone(),
                    value: event.payload.clone(),
                    observed_at: event.record_time,
                });
            }
        }
        state
    }

    /// Append without rewriting earlier events.
    pub fn append_event(&mut self, event: LedgerEvent) {
        self.events.push(event);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceManifest {
    pub schema: String,
    pub snapshot: String,
    pub jurisdiction: String,
    pub artifacts: Vec<ManifestArtifact>,
}

#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "camelCase")]
pub enum SourceWeight {
    Binding,
    Controlling,
    Persuasive,
    #[default]
    Explanatory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestArtifact {
    pub path: String,
    pub digest: String,
    pub kind: String,
    pub effective: String,
    #[serde(default)]
    pub weight: SourceWeight,
}

impl Default for SourceManifest {
    fn default() -> Self {
        Self {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{Instant, Interval};
    use crate::value::Value;

    #[test]
    fn case_facts_accept_bare_literals_and_tagged_entity() {
        let json = serde_json::json!({
            "schema": "fidryn.case-record/v0.1",
            "facts": {
                "acting_trustee": "Bryan",
                "open_alice_branch": true,
                "year": 2026,
                "who": {"kind": "entity", "data": "Alice"}
            },
            "evidence": [{
                "schema": "PhysicianCertificate",
                "value": "certificate-1",
                "observedAt": "2026-08-23T12:00:00Z"
            }],
            "admissibleCompletions": {}
        });
        let case: CaseRecord = serde_json::from_value(json).unwrap();
        assert_eq!(case.facts["acting_trustee"], Value::String("Bryan".into()));
        assert_eq!(case.facts["open_alice_branch"], Value::Bool(true));
        assert_eq!(case.facts["year"], Value::Int(2026));
        assert_eq!(case.facts["who"], Value::Entity("Alice".into()));
        assert_eq!(
            case.evidence[0].value,
            Value::String("certificate-1".into())
        );
    }

    #[test]
    fn determination_without_recorded_at_deserializes() {
        let json = serde_json::json!({
            "issue": "Incapacitated",
            "protocol": "PhysicianCertificate",
            "established": true,
            "decider": "court"
        });
        let det: CaseDetermination = serde_json::from_value(json).unwrap();
        assert_eq!(det.issue, "Incapacitated");
        assert!(det.recorded_at.is_none());
    }

    #[test]
    fn into_state_projects_evidence_and_evidence_events() {
        let observed = Instant::parse("2026-08-23T12:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: Value::String("certificate-1".into()),
            observed_at: observed,
        });
        let first = LedgerEvent {
            kind: "evidence".into(),
            valid_time: Interval::always(),
            record_time: observed,
            payload: Value::String("event-1".into()),
        };
        let second = LedgerEvent {
            kind: "duty".into(),
            valid_time: Interval::always(),
            record_time: observed,
            payload: Value::String("duty-1".into()),
        };
        case.append_event(first.clone());
        case.append_event(second.clone());
        assert_eq!(case.events[0], first);
        let state = case.into_state();
        assert_eq!(state.record.items.len(), 2);
        assert_eq!(state.record.items[0].schema, "PhysicianCertificate");
        assert_eq!(
            state.record.items[0].value,
            Value::String("certificate-1".into())
        );
        assert_eq!(state.record.items[1].schema, "evidence");
        assert_eq!(state.record.items[1].value, Value::String("event-1".into()));
    }
}
