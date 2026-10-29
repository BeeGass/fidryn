//! Outcome aggregation after declared completion admission and evaluation.

use crate::aggregate;
use fidryn_core::{EngineError, OpenRequest, Outcome, TraceId, Value};
use std::collections::{BTreeMap, BTreeSet};

fn nested_identity(outer: &str, inner: &str) -> String {
    fidryn_core::canonical_json(&(outer, inner)).expect("nested completion identity is encodable")
}

/// Preserve world identity, refusal precedence and incomplete coverage.
pub fn aggregate_completions(
    branches: Vec<(String, Outcome<Value>)>,
    incomplete: bool,
) -> Result<Outcome<Value>, EngineError> {
    if branches.is_empty() {
        if incomplete {
            return Ok(incomplete_search());
        }
        return Ok(empty_completion_set());
    }

    if incomplete {
        return Ok(incomplete_search());
    }

    let outcomes: Vec<Outcome<Value>> = branches.iter().map(|(_, out)| out.clone()).collect();
    if outcomes.iter().any(|out| {
        matches!(
            out,
            Outcome::Suspended { .. }
                | Outcome::NormConflict { .. }
                | Outcome::OutsideCompetence { .. }
                | Outcome::Inconsistent { .. }
        )
    }) {
        return Ok(aggregate(outcomes));
    }

    let mut labeled = BTreeMap::new();
    let mut pivots = BTreeSet::new();
    let mut has_contingent = false;
    for (label, out) in &branches {
        match out {
            Outcome::Determinate { value, .. } => {
                if labeled.insert(label.clone(), value.clone()).is_some() {
                    return Err(EngineError::Internal(
                        "duplicate exploration world identity".into(),
                    ));
                }
            }
            Outcome::Contingent {
                alternatives,
                pivots: nested_pivots,
                ..
            } => {
                has_contingent = true;
                pivots.extend(nested_pivots.iter().cloned());
                for (nested, value) in alternatives {
                    if labeled
                        .insert(nested_identity(label, nested), value.clone())
                        .is_some()
                    {
                        return Err(EngineError::Internal(
                            "duplicate nested exploration world identity".into(),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    if labeled.is_empty() {
        return Ok(aggregate(outcomes));
    }

    if has_contingent {
        return Ok(Outcome::Contingent {
            alternatives: labeled,
            pivots,
            trace: TraceId::of(b"explore"),
        });
    }

    let mut values = labeled.values();
    let Some(first) = values.next() else {
        return Ok(aggregate(outcomes));
    };
    let all_equal = values.all(|value| value == first);
    if all_equal {
        if incomplete {
            return Ok(incomplete_search());
        }
        return Ok(outcomes
            .into_iter()
            .find(|out| matches!(out, Outcome::Determinate { .. }))
            .unwrap_or_else(|| aggregate(branches.into_iter().map(|(_, out)| out).collect())));
    }

    Ok(Outcome::Contingent {
        alternatives: labeled,
        pivots: BTreeSet::new(),
        trace: TraceId::of(b"explore"),
    })
}

fn empty_completion_set() -> Outcome<Value> {
    Outcome::Inconsistent {
        core: vec!["empty completion set".into()],
        trace: TraceId::of(b"empty"),
    }
}

fn incomplete_search() -> Outcome<Value> {
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedCustom {
        effect: "Explore".into(),
        payload: "incomplete completion search".into(),
    });
    Outcome::Suspended {
        requests,
        trace: TraceId::of(b"explore-incomplete"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agreeing_answers_do_not_erase_an_unresolved_nested_contingent() {
        let request = OpenRequest::NeedCustom {
            effect: "Interpret".into(),
            payload: "open institutional premise".into(),
        };
        let value = Value::Bool(true);
        let direct = Outcome::Determinate {
            value: value.clone(),
            trace: TraceId::of(b"direct"),
            convergence_certificate: None,
            ignored_open_issues: BTreeSet::new(),
        };
        let nested = Outcome::Contingent {
            alternatives: BTreeMap::from([("inner".into(), value)]),
            pivots: BTreeSet::from([request.clone()]),
            trace: TraceId::of(b"nested"),
        };
        let result = aggregate_completions(
            vec![("direct".into(), direct), ("open".into(), nested)],
            false,
        )
        .unwrap();
        let Outcome::Contingent {
            alternatives,
            pivots,
            ..
        } = result
        else {
            panic!("{result:?}")
        };
        assert_eq!(alternatives.len(), 2);
        assert!(pivots.contains(&request));
    }
}
