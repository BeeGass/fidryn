//! Checked patterns. `_` is illegal in ordinary expressions.

use crate::value::{PropTerm, Term};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PropPattern {
    Ground(PropTerm),
    Match {
        predicate: String,
        arguments: Vec<TermPattern>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TermPattern {
    Exact(Term),
    Bind(String),
    Wildcard,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LegalStatusPattern {
    PropositionStatus {
        proposition: PropPattern,
        context: ContextPattern,
    },
    InstitutionalStatus {
        constructor: String,
        arguments: Vec<TermPattern>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LegalEffectPattern {
    Establish(LegalStatusPattern),
    Terminate(LegalStatusPattern),
    Suspend(LegalStatusPattern),
    CreatePosition {
        position_kind: String,
        arguments: Vec<TermPattern>,
    },
    Affect(LegalSubjectPattern),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PositionPattern {
    MatchPosition {
        position_kind: String,
        arguments: Vec<TermPattern>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LegalSubjectPattern {
    Exact(String),
    Bind(String),
    Wildcard,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ContextPattern {
    CurrentContext,
    Exact(String),
    AnyContext,
}
