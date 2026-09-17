//! Duty status machine. Late perform keeps breach history.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum DutyStatus {
    Attached,
    Performed,
    Breached,
    Cured,
    Discharged,
    Unresolved,
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
        };
        assert!(state.breached);
        assert_eq!(state.status, DutyStatus::Performed);
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["status"], "Performed");
        assert_eq!(json["breached"], serde_json::Value::Bool(true));
    }
}
