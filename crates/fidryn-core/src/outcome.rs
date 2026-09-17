//! Honest evaluator results. Determinate is never a guess.

use crate::effects::OpenOperation;
use crate::ids::{CompletionProofId, TraceId};
use crate::ir::CoreConflictDoctrine;
use crate::patterns::PropPattern;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenRequest {
    NeedEvidence {
        issue: PropPattern,
        schema: String,
    },
    NeedJudgment {
        issue: crate::value::PropTerm,
        protocol: String,
    },
    NeedChoice {
        protocol: String,
        options: Vec<String>,
    },
    NeedInterpretation {
        source: String,
        family: String,
    },
    NeedApplicableLaw {
        issue: String,
        candidates: Vec<String>,
    },
    NeedConflict {
        graph: Vec<String>,
        doctrines: Vec<String>,
    },
    NeedCustom {
        effect: String,
        payload: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Outcome<T = Value> {
    Determinate {
        value: T,
        trace: TraceId,
        #[serde(skip_serializing_if = "Option::is_none")]
        convergence_certificate: Option<CompletionProofId>,
        ignored_open_issues: BTreeSet<OpenRequest>,
    },
    Contingent {
        alternatives: BTreeMap<String, T>,
        pivots: BTreeSet<OpenRequest>,
        trace: TraceId,
    },
    Suspended {
        requests: BTreeSet<OpenRequest>,
        trace: TraceId,
    },
    NormConflict {
        doctrines: Vec<CoreConflictDoctrine>,
        trace: TraceId,
    },
    OutsideCompetence {
        request: OpenRequest,
        reason: String,
        trace: TraceId,
    },
    Inconsistent {
        core: Vec<String>,
        trace: TraceId,
    },
}

impl<T> Outcome<T> {
    /// Construct a determinate result. Nonempty ignored issues require a certificate.
    pub fn determinate(
        value: T,
        trace: TraceId,
        certificate: Option<CompletionProofId>,
        ignored: BTreeSet<OpenRequest>,
    ) -> Result<Self, String> {
        if !ignored.is_empty() && certificate.is_none() {
            return Err("ignored_open_issues requires a checked convergence certificate".into());
        }
        Ok(Self::Determinate {
            value,
            trace,
            convergence_certificate: certificate,
            ignored_open_issues: ignored,
        })
    }

    pub fn is_determinate(&self) -> bool {
        matches!(self, Self::Determinate { .. })
    }

    pub fn trace(&self) -> TraceId {
        match self {
            Self::Determinate { trace, .. }
            | Self::Contingent { trace, .. }
            | Self::Suspended { trace, .. }
            | Self::NormConflict { trace, .. }
            | Self::OutsideCompetence { trace, .. }
            | Self::Inconsistent { trace, .. } => *trace,
        }
    }
}

impl OpenRequest {
    pub fn as_operation(&self) -> OpenOperation {
        match self {
            OpenRequest::NeedEvidence { .. } => OpenOperation::Observe,
            OpenRequest::NeedJudgment { .. } => OpenOperation::Determine,
            OpenRequest::NeedChoice { .. } => OpenOperation::Choose,
            OpenRequest::NeedInterpretation { .. } => OpenOperation::Interpret,
            OpenRequest::NeedApplicableLaw { .. } => OpenOperation::SelectApplicableLaw,
            OpenRequest::NeedConflict { .. } => OpenOperation::ResolveNormConflict,
            OpenRequest::NeedCustom { .. } => OpenOperation::Observe,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::NodeId;
    use crate::patterns::{ContextPattern, PropPattern};
    use crate::value::PropTerm;

    #[test]
    fn ignored_issues_require_certificate() {
        let trace = TraceId::of(b"t");
        let mut ignored = BTreeSet::new();
        ignored.insert(OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(PropTerm::new("P", vec![])),
            schema: "S".into(),
        });
        let err = Outcome::<Value>::determinate(Value::Unit, trace, None, ignored).unwrap_err();
        assert!(err.contains("convergence"));
        let _ = NodeId::of(b"ctx");
        let _ = ContextPattern::CurrentContext;
    }
}
