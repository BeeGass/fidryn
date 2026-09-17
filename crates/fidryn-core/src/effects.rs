//! Algebraic effect operations. Handlers never return a bare Boolean.

use crate::outcome::OpenRequest;
use crate::value::Value;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum EffectName {
    Observe,
    Determine,
    Choose,
    Interpret,
    ResolveNormConflict,
    SelectApplicableLaw,
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
