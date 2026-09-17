//! Algebraic effect operations. Handlers never return a bare Boolean.

use crate::outcome::OpenRequest;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Built-in algebraic effects, or an unresolved custom effect ident.
///
/// Custom names stay on Core function/query effect sets instead of being
/// dropped. Wire form is the ident string (`"Determine"`, `"DocketLookup"`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum EffectName {
    Observe,
    Determine,
    Choose,
    Interpret,
    ResolveNormConflict,
    SelectApplicableLaw,
    Unresolved(String),
}

impl EffectName {
    /// Parse a surface effect-row ident. Unknown names are [`Self::Unresolved`].
    pub fn from_ident(name: &str) -> Self {
        match name {
            "Observe" => Self::Observe,
            "Determine" => Self::Determine,
            "Choose" => Self::Choose,
            "Interpret" => Self::Interpret,
            "ResolveNormConflict" => Self::ResolveNormConflict,
            "SelectApplicableLaw" => Self::SelectApplicableLaw,
            other => Self::Unresolved(other.to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Observe => "Observe",
            Self::Determine => "Determine",
            Self::Choose => "Choose",
            Self::Interpret => "Interpret",
            Self::ResolveNormConflict => "ResolveNormConflict",
            Self::SelectApplicableLaw => "SelectApplicableLaw",
            Self::Unresolved(name) => name,
        }
    }
}

impl From<String> for EffectName {
    fn from(name: String) -> Self {
        Self::from_ident(&name)
    }
}

impl From<EffectName> for String {
    fn from(effect: EffectName) -> Self {
        effect.as_str().to_owned()
    }
}

impl fmt::Display for EffectName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OpenOperation {
    Observe,
    Determine,
    Choose,
    Interpret,
    ResolveNormConflict,
    SelectApplicableLaw,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandlerResult<T = Value> {
    Resume {
        value: T,
        trace_fragment: String,
    },
    Suspend {
        requests: BTreeSet<OpenRequest>,
        reason: SuspensionReason,
        trace_fragment: String,
    },
    Halt {
        reason: HaltReason,
        trace_fragment: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SuspensionReason {
    MissingRecord,
    MissingDetermination,
    MissingChoice,
    MissingInterpretation,
    OpenBranch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HaltReason {
    OutsideCompetence {
        request: OpenRequest,
        reason: String,
    },
    NormConflict {
        graph: Vec<String>,
    },
    Inconsistent {
        core: Vec<String>,
    },
}

/// A handler discharges an open legal operation or refuses honestly.
pub trait Handler {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult;
    fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult;
    fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult;
    fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult;

    fn handle_conflict(&mut self, request: &OpenRequest) -> HandlerResult {
        let mut requests = BTreeSet::new();
        requests.insert(request.clone());
        HandlerResult::Suspend {
            requests,
            reason: SuspensionReason::OpenBranch,
            trace_fragment: "need-conflict".into(),
        }
    }

    fn handle_law(&mut self, request: &OpenRequest) -> HandlerResult {
        let mut requests = BTreeSet::new();
        requests.insert(request.clone());
        HandlerResult::Suspend {
            requests,
            reason: SuspensionReason::OpenBranch,
            trace_fragment: "need-applicable-law".into(),
        }
    }

    fn handle_custom(&mut self, request: &OpenRequest) -> HandlerResult {
        let mut requests = BTreeSet::new();
        requests.insert(request.clone());
        HandlerResult::Suspend {
            requests,
            reason: SuspensionReason::OpenBranch,
            trace_fragment: "need-custom-effect".into(),
        }
    }

    fn handle(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedEvidence { .. } => self.handle_observe(request),
            OpenRequest::NeedJudgment { .. } => self.handle_determine(request),
            OpenRequest::NeedChoice { .. } => self.handle_choose(request),
            OpenRequest::NeedInterpretation { .. } => self.handle_interpret(request),
            OpenRequest::NeedApplicableLaw { .. } => self.handle_law(request),
            OpenRequest::NeedConflict { .. } => self.handle_conflict(request),
            OpenRequest::NeedCustom { .. } => self.handle_custom(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EffectName;

    #[test]
    fn effect_name_keeps_unresolved_ident_on_the_wire() {
        assert_eq!(EffectName::from_ident("Determine"), EffectName::Determine);
        assert_eq!(
            EffectName::from_ident("DocketLookup"),
            EffectName::Unresolved("DocketLookup".into())
        );
        assert_eq!(
            serde_json::to_string(&EffectName::Determine).unwrap(),
            "\"Determine\""
        );
        let custom = EffectName::from_ident("DocketLookup");
        assert_eq!(serde_json::to_string(&custom).unwrap(), "\"DocketLookup\"");
        let back: EffectName = serde_json::from_str("\"DocketLookup\"").unwrap();
        assert_eq!(back, custom);
    }
}
