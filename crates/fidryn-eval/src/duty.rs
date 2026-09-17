//! Duty status machine. Illegal or unauthorized transitions commit nothing.

use fidryn_core::time::Instant;
use fidryn_core::value::Value;
use fidryn_core::{CaseRecord, DutyState, DutyStatus, EngineError};
use std::collections::BTreeMap;

pub const DUTY_KEY_PREFIX: &str = "duty:";

pub fn duty_fact_key(name: &str) -> String {
    format!("{DUTY_KEY_PREFIX}{name}")
}

pub fn status_name(status: DutyStatus) -> &'static str {
    match status {
        DutyStatus::Attached => "Attached",
        DutyStatus::Performed => "Performed",
        DutyStatus::Breached => "Breached",
        DutyStatus::Cured => "Cured",
        DutyStatus::Discharged => "Discharged",
        DutyStatus::Unresolved => "Unresolved",
    }
}

pub fn status_value(status: DutyStatus) -> Value {
    Value::Ctor {
        name: status_name(status).to_owned(),
        fields: BTreeMap::new(),
    }
}

pub fn duty_state_value(state: &DutyState) -> Value {
    let mut fields = BTreeMap::from([
        ("name".into(), Value::String(state.name.clone())),
        (
            "status".into(),
            Value::String(status_name(state.status).into()),
        ),
        ("breached".into(), Value::Bool(state.breached)),
        ("bearer".into(), Value::String(state.bearer.clone())),
    ]);
    if let Some(claimant) = &state.claimant {
        fields.insert("claimant".into(), Value::String(claimant.clone()));
    }
    Value::Map(fields)
}

pub fn parse_duty_state(value: &Value, name: &str) -> Option<DutyState> {
    let fields = match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => fields,
        _ => return None,
    };
    let status = fields.get("status").and_then(parse_status_value)?;
    let breached = match fields.get("breached") {
        Some(Value::Bool(flag)) => *flag,
        None => false,
        _ => return None,
    };
    let bearer = match fields.get("bearer") {
        Some(Value::String(s) | Value::Entity(s)) => s.clone(),
        Some(Value::Unit) | None => String::new(),
        _ => return None,
    };
    let stored_name = match fields.get("name") {
        Some(Value::String(s) | Value::Entity(s)) => s.clone(),
        _ => name.to_owned(),
    };
    let claimant = match fields.get("claimant") {
        Some(Value::String(s) | Value::Entity(s)) => Some(s.clone()),
        Some(Value::Option(Some(inner))) => match inner.as_ref() {
            Value::String(s) | Value::Entity(s) => Some(s.clone()),
            _ => None,
        },
        _ => None,
    };
    Some(DutyState {
        name: stored_name,
        status,
        breached,
        bearer,
        claimant,
    })
}

fn parse_status_value(value: &Value) -> Option<DutyStatus> {
    match value {
        Value::String(name) | Value::Entity(name) => parse_status_name(name),
        Value::Ctor { name, .. } => parse_status_name(name),
        _ => None,
    }
}

fn parse_status_name(name: &str) -> Option<DutyStatus> {
    if name.eq_ignore_ascii_case("Attached") {
        Some(DutyStatus::Attached)
    } else if name.eq_ignore_ascii_case("Performed") {
        Some(DutyStatus::Performed)
    } else if name.eq_ignore_ascii_case("Breached") {
        Some(DutyStatus::Breached)
    } else if name.eq_ignore_ascii_case("Cured") {
        Some(DutyStatus::Cured)
    } else if name.eq_ignore_ascii_case("Discharged") {
        Some(DutyStatus::Discharged)
    } else if name.eq_ignore_ascii_case("Unresolved") {
        Some(DutyStatus::Unresolved)
    } else {
        None
    }
}

pub fn apply_duty_action(
    current: Option<&DutyState>,
    name: &str,
    action: &str,
    person: Option<String>,
) -> Result<DutyState, EngineError> {
    let action = parse_duty_action(action)
        .ok_or_else(|| EngineError::InvalidInput(format!("unknown duty action `{action}`")))?;
    let bearer = match (person, current) {
        (Some(person), _) => person,
        (None, Some(state)) => state.bearer.clone(),
        (None, None) => String::new(),
    };
    let claimant = current.and_then(|state| state.claimant.clone());
    let from = current.map(|state| state.status);
    let breached = current.map(|state| state.breached).unwrap_or(false);
    let next = match (from, action) {
        (None | Some(DutyStatus::Unresolved), DutyAction::Attach) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Attached,
            breached: false,
            bearer,
            claimant,
        },
        (Some(DutyStatus::Attached), DutyAction::Perform) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Performed,
            breached: false,
            bearer,
            claimant,
        },
        (Some(DutyStatus::Attached), DutyAction::Breach) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Breached,
            breached: true,
            bearer,
            claimant,
        },
        (Some(DutyStatus::Breached), DutyAction::Perform) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Performed,
            breached: true,
            bearer,
            claimant,
        },
        (Some(DutyStatus::Breached), DutyAction::Cure) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Cured,
            breached: true,
            bearer,
            claimant,
        },
        (
            Some(DutyStatus::Performed | DutyStatus::Cured | DutyStatus::Breached),
            DutyAction::Discharge,
        ) => DutyState {
            name: name.to_owned(),
            status: DutyStatus::Discharged,
            breached,
            bearer,
            claimant,
        },
        (from, action) => {
            let from = from.map_or("unattached", status_name);
            return Err(EngineError::InvalidInput(format!(
                "illegal duty transition: {} from {from}",
                action.as_str()
            )));
        }
    };
    Ok(next)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DutyAction {
    Attach,
    Perform,
    Breach,
    Cure,
    Discharge,
}

impl DutyAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Attach => "attach",
            Self::Perform => "perform",
            Self::Breach => "breach",
            Self::Cure => "cure",
            Self::Discharge => "discharge",
        }
    }
}

fn parse_duty_action(name: &str) -> Option<DutyAction> {
    if name.eq_ignore_ascii_case("attach") {
        Some(DutyAction::Attach)
    } else if name.eq_ignore_ascii_case("perform") {
        Some(DutyAction::Perform)
    } else if name.eq_ignore_ascii_case("breach") {
        Some(DutyAction::Breach)
    } else if name.eq_ignore_ascii_case("cure") {
        Some(DutyAction::Cure)
    } else if name.eq_ignore_ascii_case("discharge") {
        Some(DutyAction::Discharge)
    } else {
        None
    }
}

pub fn has_authority_constraint(case: &CaseRecord) -> bool {
    case.facts.contains_key("authority_grants")
        || case
            .evidence
            .iter()
            .any(|item| item.schema.eq_ignore_ascii_case("AuthorityGrant"))
}

pub fn action_is_granted(case: &CaseRecord, action: &str, record_time: Instant) -> bool {
    if let Some(grants) = case.facts.get("authority_grants")
        && value_grants_action(grants, action)
    {
        return true;
    }
    case.evidence.iter().any(|item| {
        item.schema.eq_ignore_ascii_case("AuthorityGrant")
            && item.observed_at <= record_time
            && value_grants_action(&item.value, action)
    })
}

fn value_grants_action(value: &Value, action: &str) -> bool {
    match value {
        Value::String(name) | Value::Entity(name) => name.eq_ignore_ascii_case(action),
        Value::Ctor { name, fields } => {
            name.eq_ignore_ascii_case(action)
                || (name.eq_ignore_ascii_case("AuthorityGrant")
                    && fields
                        .get("action")
                        .is_some_and(|v| value_grants_action(v, action)))
                || fields.values().any(|v| value_grants_action(v, action))
        }
        Value::Set(items) => items.iter().any(|item| value_grants_action(item, action)),
        Value::Map(fields) => {
            if let Some(named) = fields.get("action") {
                return value_grants_action(named, action);
            }
            fields.iter().any(|(key, val)| match val {
                Value::Bool(true) => key.eq_ignore_ascii_case(action),
                _ => key.eq_ignore_ascii_case(action) || value_grants_action(val, action),
            })
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::EvidenceItem;

    fn attached() -> DutyState {
        DutyState {
            name: "pay".into(),
            status: DutyStatus::Attached,
            breached: false,
            bearer: String::new(),
            claimant: None,
        }
    }

    #[test]
    fn attach_from_absent_is_attached() {
        let got = apply_duty_action(None, "pay", "attach", None).unwrap();
        assert_eq!(got.status, DutyStatus::Attached);
        assert!(!got.breached);
        assert_eq!(got.name, "pay");
    }

    #[test]
    fn late_perform_from_breached_keeps_breached() {
        let breached = DutyState {
            name: "pay".into(),
            status: DutyStatus::Breached,
            breached: true,
            bearer: "Alice".into(),
            claimant: None,
        };
        let got = apply_duty_action(Some(&breached), "pay", "perform", None).unwrap();
        assert_eq!(got.status, DutyStatus::Performed);
        assert!(got.breached);
        assert_eq!(got.bearer, "Alice");
    }

    #[test]
    fn discharge_from_attached_is_illegal() {
        let err = apply_duty_action(Some(&attached()), "pay", "discharge", None).unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidInput(ref msg) if msg.contains("discharge")),
            "{err:?}"
        );
    }

    #[test]
    fn grant_set_and_authority_evidence_match_action() {
        let t = Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![Value::String("attach".into())]),
        );
        assert!(action_is_granted(&case, "attach", t));
        assert!(!action_is_granted(&case, "perform", t));

        let mut evidence_only = CaseRecord::default();
        evidence_only.evidence.push(EvidenceItem {
            schema: "AuthorityGrant".into(),
            value: Value::Map(BTreeMap::from([(
                "action".into(),
                Value::String("perform".into()),
            )])),
            observed_at: t,
        });
        assert!(action_is_granted(&evidence_only, "perform", t));
        assert!(!action_is_granted(&evidence_only, "attach", t));
        assert!(has_authority_constraint(&evidence_only));
    }
}
