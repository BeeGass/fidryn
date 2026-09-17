//! Conflict oracle: a unique applicable doctrine, or the argument graph.
//!
//! List order is never a tie-break. Zero applicable doctrines, or two or more
//! incompatible applicable doctrines, stay unresolved.

use std::collections::BTreeSet;

/// Resolve staged-effect argument-graph nodes against named doctrines.
///
/// Exactly one applicable doctrine yields `Ok(name)`. Otherwise the original
/// `graph` is returned so the caller can produce [`fidryn_core::Outcome::NormConflict`]
/// or suspend [`fidryn_core::OpenRequest::NeedConflict`].
pub fn resolve_conflict(doctrines: &[String], graph: &[String]) -> Result<String, Vec<String>> {
    let mut applicable = applicable_doctrines(doctrines, graph).into_iter();
    match (applicable.next(), applicable.next()) {
        (Some(name), None) => Ok(name),
        _ => Err(graph.to_vec()),
    }
}

fn applicable_doctrines(doctrines: &[String], graph: &[String]) -> BTreeSet<String> {
    let candidates: BTreeSet<String> = doctrines
        .iter()
        .map(|d| d.trim())
        .filter(|d| !d.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    if graph.iter().all(|n| n.trim().is_empty()) {
        return candidates;
    }

    let mut positive = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for doctrine in &candidates {
        for node in graph {
            match mention(node, doctrine) {
                Mention::Positive => {
                    positive.insert(doctrine.clone());
                }
                Mention::Negative => {
                    negative.insert(doctrine.clone());
                }
                Mention::None => {}
            }
        }
    }

    if positive.is_empty() {
        candidates.difference(&negative).cloned().collect()
    } else {
        positive.difference(&negative).cloned().collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mention {
    Positive,
    Negative,
    None,
}

fn mention(node: &str, doctrine: &str) -> Mention {
    let (kind, name) = split_kind(node);
    if name != doctrine {
        return Mention::None;
    }
    match kind {
        Some(kind) if is_negative_kind(kind) => Mention::Negative,
        _ => Mention::Positive,
    }
}

fn split_kind(node: &str) -> (Option<&str>, &str) {
    let node = node.trim();
    for sep in [':', ' ', '('] {
        if let Some((kind, rest)) = node.split_once(sep) {
            let kind = kind.trim();
            if is_graph_kind(kind) {
                return (Some(kind), trim_label(rest));
            }
        }
    }
    (None, trim_label(node))
}

fn trim_label(value: &str) -> &str {
    value
        .trim()
        .trim_matches(|c| c == '(' || c == ')' || c == '"' || c == '\'')
}

fn is_graph_kind(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("Citation")
        || kind.eq_ignore_ascii_case("Holding")
        || kind.eq_ignore_ascii_case("Distinguish")
        || kind.eq_ignore_ascii_case("Overrule")
        || kind.eq_ignore_ascii_case("Follow")
}

fn is_negative_kind(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("Distinguish") || kind.eq_ignore_ascii_case("Overrule")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::{OpenRequest, Outcome, TraceId};
    use std::collections::BTreeSet;

    #[test]
    fn one_listed_doctrine_applies_without_graph_constraints() {
        let doctrines = vec!["ChildSupportCannotBeAdverselyAffected".into()];
        let got = resolve_conflict(&doctrines, &[]).unwrap();
        assert_eq!(got, "ChildSupportCannotBeAdverselyAffected");
    }

    #[test]
    fn graph_selecting_one_of_two_doctrines_is_unique() {
        let doctrines = vec!["LexSpecialis".into(), "LexPosterior".into()];
        let graph = vec![
            "Follow:LexSpecialis".into(),
            "Citation:ChildSupportStatute".into(),
        ];
        assert_eq!(
            resolve_conflict(&doctrines, &graph).unwrap(),
            "LexSpecialis"
        );
    }

    #[test]
    fn two_applicable_doctrines_are_not_resolved_by_list_order() {
        let ab = vec!["LexSpecialis".into(), "LexPosterior".into()];
        let ba = vec!["LexPosterior".into(), "LexSpecialis".into()];
        let graph = vec!["Follow:LexSpecialis".into(), "Holding:LexPosterior".into()];
        let err_ab = resolve_conflict(&ab, &graph).unwrap_err();
        let err_ba = resolve_conflict(&ba, &graph).unwrap_err();
        assert_eq!(err_ab, graph);
        assert_eq!(err_ba, graph);
        assert_ne!(ab[0], ba[0]);
    }

    #[test]
    fn zero_doctrines_return_the_graph() {
        let graph = vec!["Holding:Unrelated".into()];
        assert_eq!(resolve_conflict(&[], &graph).unwrap_err(), graph);
    }

    #[test]
    fn empty_graph_and_empty_doctrines_are_unresolved() {
        assert!(resolve_conflict(&[], &[]).is_err());
    }

    #[test]
    fn duplicate_names_count_as_one_doctrine() {
        let doctrines = vec!["LexSpecialis".into(), "LexSpecialis".into()];
        assert_eq!(resolve_conflict(&doctrines, &[]).unwrap(), "LexSpecialis");
    }

    #[test]
    fn overrule_removes_a_doctrine_and_can_leave_a_unique_winner() {
        let doctrines = vec!["LexSpecialis".into(), "LexPosterior".into()];
        let graph = vec!["Overrule:LexPosterior".into()];
        assert_eq!(
            resolve_conflict(&doctrines, &graph).unwrap(),
            "LexSpecialis"
        );
    }

    #[test]
    fn overrule_of_the_only_doctrine_is_unresolved() {
        let doctrines = vec!["LexPosterior".into()];
        let graph = vec!["Overrule:LexPosterior".into()];
        assert_eq!(resolve_conflict(&doctrines, &graph).unwrap_err(), graph);
    }

    #[test]
    fn distinguish_is_negative() {
        let doctrines = vec!["OldHolding".into(), "CurrentStatute".into()];
        let graph = vec![
            "Distinguish:OldHolding".into(),
            "Follow:CurrentStatute".into(),
        ];
        assert_eq!(
            resolve_conflict(&doctrines, &graph).unwrap(),
            "CurrentStatute"
        );
    }

    #[test]
    fn follow_and_overrule_of_the_same_name_do_not_apply() {
        let doctrines = vec!["LexPosterior".into()];
        let graph = vec!["Follow:LexPosterior".into(), "Overrule:LexPosterior".into()];
        assert!(resolve_conflict(&doctrines, &graph).is_err());
    }

    #[test]
    fn unmentioned_effect_graph_does_not_invent_a_winner_among_two() {
        let doctrines = vec!["A".into(), "B".into()];
        let graph = vec!["Establish:Waived".into()];
        assert_eq!(resolve_conflict(&doctrines, &graph).unwrap_err(), graph);
    }

    #[test]
    fn parenthesized_follow_nodes_parse() {
        let doctrines = vec!["LexSpecialis".into(), "LexPosterior".into()];
        let graph = vec!["Follow(LexSpecialis)".into()];
        assert_eq!(
            resolve_conflict(&doctrines, &graph).unwrap(),
            "LexSpecialis"
        );
    }

    #[test]
    fn caller_turns_err_into_need_conflict_or_norm_conflict() {
        let doctrines = vec!["A".into(), "B".into()];
        let graph = vec!["Follow:A".into(), "Follow:B".into()];
        let err = resolve_conflict(&doctrines, &graph).unwrap_err();
        let request = OpenRequest::NeedConflict {
            graph: err.clone(),
            doctrines: doctrines.clone(),
        };
        assert!(matches!(
            request,
            OpenRequest::NeedConflict { ref graph, .. } if *graph == err
        ));
        let outcome: Outcome<fidryn_core::Value> = Outcome::NormConflict {
            doctrines: Vec::new(),
            trace: TraceId::of(b"conflict"),
        };
        assert!(matches!(outcome, Outcome::NormConflict { .. }));
        let suspended: Outcome<fidryn_core::Value> = Outcome::Suspended {
            requests: BTreeSet::from([request]),
            trace: TraceId::of(b"need-conflict"),
        };
        assert!(matches!(suspended, Outcome::Suspended { .. }));
    }
}
