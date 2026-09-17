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
    #[serde(default)]
    pub principal: Option<String>,
    #[serde(default)]
    pub delegate_of: Option<String>,
    #[serde(default)]
    pub revoked: bool,
    #[serde(default)]
    pub revoked_at: Option<Instant>,
}

impl Default for AuthorityGrant {
    fn default() -> Self {
        Self {
            action: String::new(),
            scope: String::new(),
            context: String::new(),
            valid_time: Interval::always(),
            source: String::new(),
            principal: None,
            delegate_of: None,
            revoked: false,
            revoked_at: None,
        }
    }
}

impl AuthorityGrant {
    pub fn covers(&self, action: &str, at: Instant) -> bool {
        !self.is_revoked_at(at) && self.action == action && self.valid_time.contains(at)
    }

    fn is_revoked_at(&self, at: Instant) -> bool {
        if !self.revoked {
            return false;
        }
        self.revoked_at.is_none_or(|when| at >= when)
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
            ..AuthorityGrant::default()
        };
        assert!(grant.covers("Administer", start));
        assert!(!grant.covers("Distribute", start));
        assert!(!grant.covers("Administer", end));
        assert!(!grant.revoked);
        assert!(grant.principal.is_none());
        assert!(grant.delegate_of.is_none());
        assert!(grant.revoked_at.is_none());
    }

    fn administer_grant(start: Instant, end: Instant) -> AuthorityGrant {
        AuthorityGrant {
            action: "Administer".into(),
            scope: "trust".into(),
            context: "office".into(),
            valid_time: Interval::from_instants(start, Some(end)).unwrap(),
            source: "instrument".into(),
            ..AuthorityGrant::default()
        }
    }

    #[test]
    fn revoked_grant_does_not_cover() {
        let start = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let end = Instant::parse("2034-01-01T00:00:00Z").unwrap();
        let grant = AuthorityGrant {
            revoked: true,
            ..administer_grant(start, end)
        };
        assert!(!grant.covers("Administer", start));
    }

    #[test]
    fn revoked_grant_covers_before_revoked_at() {
        let start = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mid = Instant::parse("2033-06-01T00:00:00Z").unwrap();
        let end = Instant::parse("2034-01-01T00:00:00Z").unwrap();
        let grant = AuthorityGrant {
            revoked: true,
            revoked_at: Some(mid),
            ..administer_grant(start, end)
        };
        assert!(grant.covers("Administer", start));
        assert!(!grant.covers("Administer", mid));
    }

    #[test]
    fn unrevoked_delegated_grant_covers() {
        let start = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let end = Instant::parse("2034-01-01T00:00:00Z").unwrap();
        let grant = AuthorityGrant {
            principal: Some("delegate".into()),
            delegate_of: Some("grantor".into()),
            ..administer_grant(start, end)
        };
        assert!(grant.covers("Administer", start));
        assert!(!grant.covers("Distribute", start));
    }

    #[test]
    fn grant_deserializes_without_delegation_fields() {
        let start = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let end = Instant::parse("2034-01-01T00:00:00Z").unwrap();
        let grant = administer_grant(start, end);
        let mut json = serde_json::to_value(&grant).unwrap();
        let obj = json.as_object_mut().expect("grant object");
        obj.remove("principal");
        obj.remove("delegateOf");
        obj.remove("revoked");
        obj.remove("revokedAt");
        let decoded: AuthorityGrant = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.action, "Administer");
        assert!(!decoded.revoked);
        assert!(decoded.principal.is_none());
        assert!(decoded.delegate_of.is_none());
        assert!(decoded.revoked_at.is_none());
        assert!(decoded.covers("Administer", start));
    }
}
