//! Duty status machine. Late perform keeps breach history.

use serde::{Deserialize, Serialize};

fn default_instance() -> String {
    "default".into()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DutyStatus {
    Attached,
    Performed,
    Breached,
    Cured,
    Discharged,
    Unresolved,
    NotAttached,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DutyState {
    pub name: String,
    pub status: DutyStatus,
    /// Remains true after a late perform.
    pub breached: bool,
    pub bearer: String,
    pub claimant: Option<String>,
    #[serde(default = "default_instance")]
    pub instance: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_perform_keeps_breach_history() {
        let state = DutyState {
            name: "GiveNotice".into(),
            status: DutyStatus::Performed,
            breached: true,
            bearer: "Bryan".into(),
            claimant: Some("Alice".into()),
            instance: default_instance(),
        };
        assert!(state.breached);
        assert_eq!(state.status, DutyStatus::Performed);
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["status"], "Performed");
        assert_eq!(json["breached"], serde_json::Value::Bool(true));
        assert_eq!(json["instance"], "default");
    }

    #[test]
    fn not_attached_serializes_pascal_case_and_instance_defaults() {
        let json = serde_json::json!({
            "name": "pay",
            "status": "NotAttached",
            "breached": false,
            "bearer": "",
            "claimant": null
        });
        let state: DutyState = serde_json::from_value(json).unwrap();
        assert_eq!(state.status, DutyStatus::NotAttached);
        assert_ne!(state.status, DutyStatus::Unresolved);
        assert_eq!(state.instance, "default");
        assert_eq!(
            serde_json::to_value(state.status).unwrap(),
            serde_json::Value::String("NotAttached".into())
        );
    }
}
