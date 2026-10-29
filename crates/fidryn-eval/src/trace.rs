//! Content-addressed records of operations actually performed by evaluation.

use fidryn_core::{Outcome, TraceId, Value};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivationStep {
    pub id: String,
    pub kind: String,
    pub detail: String,
    pub parents: Vec<String>,
    /// Cumulative retained bytes used by the evaluator's resource meter.
    pub retained_bytes: usize,
}

pub(crate) fn exhausted(steps: &[DerivationStep]) -> bool {
    steps
        .last()
        .is_some_and(|step| step.retained_bytes == usize::MAX)
}

pub(crate) fn record_step(
    steps: &mut Vec<DerivationStep>,
    kind: &str,
    mut detail: String,
    parents: impl IntoIterator<Item = String>,
) -> String {
    let mut parents: Vec<_> = parents
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut kind = kind;
    let prior = steps.last().map_or(0, |step| step.retained_bytes);
    let bytes = detail
        .len()
        .saturating_add(kind.len())
        .saturating_add(32)
        .saturating_add(parents.iter().map(String::len).sum::<usize>());
    let retained_bytes = if detail.len() > 1_048_576 || prior.saturating_add(bytes) > 16_777_216 {
        kind = "resource_exhaustion";
        detail = format!("derivation byte allowance exhausted: node={bytes}; retained={prior}");
        parents.clear();
        usize::MAX
    } else {
        prior + bytes
    };
    let id =
        TraceId::of(format!("fidryn.derivation/v0.1\0{kind}\0{detail}\0{parents:?}").as_bytes())
            .to_string();
    steps.push(DerivationStep {
        id: id.clone(),
        kind: kind.into(),
        detail,
        parents,
        retained_bytes,
    });
    id
}

pub(crate) fn bind_outcome(outcome: &mut Outcome<Value>, id: &str) {
    let id = TraceId::from_hex(id).expect("derivation ID is a trace hash");
    match outcome {
        Outcome::Determinate { trace, .. }
        | Outcome::Contingent { trace, .. }
        | Outcome::Suspended { trace, .. }
        | Outcome::NormConflict { trace, .. }
        | Outcome::OutsideCompetence { trace, .. }
        | Outcome::Inconsistent { trace, .. } => *trace = id,
    }
}
