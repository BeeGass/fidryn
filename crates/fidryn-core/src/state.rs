//! Immutable eight-ledger legal state.

use crate::positions::Position;
use crate::time::{Instant, Interval};
use crate::value::{PropTerm, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegalState {
    pub world: WorldLedger,
    pub record: RecordLedger,
    pub legal: LegalStatusLedger,
    pub normative: PositionLedger,
    pub authority: AuthorityLedger,
    pub decisions: DecisionLedger,
    pub interpretations: InterpretationLedger,
    pub sources: SourceLedger,
}

impl LegalState {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldLedger {
    pub entities: BTreeMap<String, String>,
    pub occurrences: Vec<Occurrence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occurrence {
    pub term: crate::value::Term,
    pub valid_time: Interval,
    pub record_time: Instant,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordLedger {
    pub items: Vec<RecordEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordEntry {
    pub schema: String,
    pub value: Value,
    pub observed_at: Instant,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LegalStatusLedger {
    pub statuses: Vec<StatusEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    pub proposition: PropTerm,
    pub context: String,
    pub mode: StatusMode,
    pub valid_time: Interval,
    pub record_time: Instant,
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusMode {
    Established,
    Suspended,
    Terminated,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionLedger {
    pub positions: Vec<Position>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityLedger {
    pub occupancy: Vec<Occupancy>,
    #[serde(default)]
    pub grants: Vec<AuthorityGrant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityGrant {
    pub action: String,
    pub scope: String,
    pub context: String,
    pub valid_time: Interval,
    pub source: String,
}

impl AuthorityGrant {
    pub fn covers(&self, action: &str, at: Instant) -> bool {
        self.action == action && self.valid_time.contains(at)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Occupancy {
    pub person: String,
    pub office: String,
    pub mode: StatusMode,
    pub valid_time: Interval,
    pub record_time: Instant,
}

impl AuthorityLedger {
    pub fn occupant_at(&self, office: &str, valid: Instant, known: Instant) -> Vec<&Occupancy> {
        self.occupancy
            .iter()
            .filter(|o| {
                o.office == office
                    && o.mode == StatusMode::Established
                    && o.valid_time.contains(valid)
                    && o.record_time <= known
            })
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionLedger {
    pub determinations: Vec<DeterminationEntry>,
    pub choices: Vec<ChoiceEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterminationEntry {
    pub issue: PropTerm,
    pub established: bool,
    pub protocol: String,
    pub decider: String,
    pub valid_time: Interval,
    pub record_time: Instant,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceEntry {
    pub protocol: String,
    pub choice: String,
    pub decider: String,
    pub valid_time: Interval,
    pub record_time: Instant,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationLedger {
    pub selected: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLedger {
    pub snapshot: String,
    pub artifacts: BTreeMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Instant;

    #[test]
    fn grant_covers_matching_action_inside_interval() {
        let start = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let end = Instant::parse("2034-01-01T00:00:00Z").unwrap();
        let grant = AuthorityGrant {
            action: "Administer".into(),
            scope: "trust".into(),
            context: "office".into(),
            valid_time: Interval::from_instants(start, Some(end)).unwrap(),
            source: "instrument".into(),
        };
        assert!(grant.covers("Administer", start));
        assert!(!grant.covers("Distribute", start));
        assert!(!grant.covers("Administer", end));
    }
}
