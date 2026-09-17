//! Applicable-law oracle. Rank by source weight, then later-in-time.

use fidryn_core::{ManifestArtifact, SourceWeight};
use std::cmp::Reverse;

/// Rank candidates: Binding > Controlling > Persuasive > Explanatory, then
/// lexicographic `effective` as a later-in-time fallback.
///
/// A unique winner is `Ok`. Same-rank ties return the tied set; list order is
/// never a tie-break. Dates are compared as strings, not parsed calendars.
pub fn select_applicable_law(
    candidates: &[ManifestArtifact],
) -> Result<ManifestArtifact, Vec<ManifestArtifact>> {
    if candidates.is_empty() {
        return Err(Vec::new());
    }
    let best = candidates
        .iter()
        .map(rank_key)
        .max()
        .expect("non-empty candidates");
    let mut tied: Vec<ManifestArtifact> = candidates
        .iter()
        .filter(|c| rank_key(c) == best)
        .cloned()
        .collect();
    match tied.len() {
        1 => Ok(tied.pop().expect("len == 1")),
        _ => {
            tied.sort_by(|a, b| a.path.cmp(&b.path).then(a.digest.cmp(&b.digest)));
            Err(tied)
        }
    }
}

fn rank_key(artifact: &ManifestArtifact) -> (Reverse<SourceWeight>, &str) {
    (Reverse(artifact.weight), artifact.effective.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::{OpenRequest, Outcome, TraceId};
    use std::collections::BTreeSet;

    fn artifact(path: &str, effective: &str, weight: SourceWeight) -> ManifestArtifact {
        ManifestArtifact {
            path: path.into(),
            digest: String::new(),
            kind: "statute".into(),
            effective: effective.into(),
            weight,
        }
    }

    #[test]
    fn binding_outranks_controlling_persuasive_and_explanatory() {
        let explanatory = artifact("note.txt", "2026-01-01", SourceWeight::Explanatory);
        let persuasive = artifact("restatement.txt", "2026-01-01", SourceWeight::Persuasive);
        let controlling = artifact("circuit.txt", "2026-01-01", SourceWeight::Controlling);
        let binding = artifact("statute.txt", "1990-01-01", SourceWeight::Binding);
        let got = select_applicable_law(&[
            explanatory,
            persuasive,
            controlling.clone(),
            binding.clone(),
        ])
        .unwrap();
        assert_eq!(got.path, "statute.txt");
        assert_eq!(got.weight, SourceWeight::Binding);
        let got = select_applicable_law(&[
            artifact("c.txt", "2024-01-01", SourceWeight::Controlling),
            artifact("p.txt", "2026-12-31", SourceWeight::Persuasive),
        ])
        .unwrap();
        assert_eq!(got.path, "c.txt");
    }

    #[test]
    fn binding_beats_a_later_controlling_source() {
        let binding = artifact("old-statute.txt", "1990-01-01", SourceWeight::Binding);
        let controlling = artifact("new-case.txt", "2024-12-31", SourceWeight::Controlling);
        assert_eq!(
            select_applicable_law(&[controlling.clone(), binding.clone()])
                .unwrap()
                .path,
            "old-statute.txt"
        );
        assert_eq!(
            select_applicable_law(&[binding, controlling]).unwrap().path,
            "old-statute.txt"
        );
    }

    #[test]
    fn later_effective_breaks_same_weight_not_list_order() {
        let early = artifact("a.txt", "2020-01-01", SourceWeight::Binding);
        let late = artifact("b.txt", "2024-01-01", SourceWeight::Binding);
        assert_eq!(
            select_applicable_law(&[early.clone(), late.clone()])
                .unwrap()
                .path,
            "b.txt"
        );
        assert_eq!(select_applicable_law(&[late, early]).unwrap().path, "b.txt");
    }

    #[test]
    fn same_weight_and_effective_stay_tied() {
        let a = artifact("alpha.txt", "2024-01-01", SourceWeight::Persuasive);
        let b = artifact("beta.txt", "2024-01-01", SourceWeight::Persuasive);
        let tied = select_applicable_law(&[b.clone(), a.clone()]).unwrap_err();
        assert_eq!(tied.len(), 2);
        assert_eq!(tied[0].path, "alpha.txt");
        assert_eq!(tied[1].path, "beta.txt");
        let again = select_applicable_law(&[a, b]).unwrap_err();
        assert_eq!(again[0].path, "alpha.txt");
        assert_eq!(again[1].path, "beta.txt");
    }

    #[test]
    fn empty_candidates_are_tied_empty() {
        assert!(select_applicable_law(&[]).unwrap_err().is_empty());
    }

    #[test]
    fn unique_candidate_wins_regardless_of_weight() {
        let only = artifact("solo.txt", "2010-01-01", SourceWeight::Explanatory);
        assert_eq!(select_applicable_law(&[only]).unwrap().path, "solo.txt");
    }

    #[test]
    fn lexicographic_effective_is_not_a_parsed_calendar() {
        let a = artifact("a.txt", "2024-1-1", SourceWeight::Controlling);
        let b = artifact("b.txt", "2024-01-01", SourceWeight::Controlling);
        assert_eq!(select_applicable_law(&[b, a]).unwrap().path, "a.txt");
    }

    #[test]
    fn caller_turns_a_tie_into_need_applicable_law() {
        let a = artifact("alpha.txt", "2024-01-01", SourceWeight::Binding);
        let b = artifact("beta.txt", "2024-01-01", SourceWeight::Binding);
        let tied = select_applicable_law(&[a, b]).unwrap_err();
        let request = OpenRequest::NeedApplicableLaw {
            issue: "formation".into(),
            candidates: tied.iter().map(|c| c.path.clone()).collect(),
        };
        match request {
            OpenRequest::NeedApplicableLaw { candidates, .. } => {
                assert_eq!(candidates, vec!["alpha.txt", "beta.txt"]);
            }
            other => panic!("{other:?}"),
        }
        let suspended: Outcome<fidryn_core::Value> = Outcome::Suspended {
            requests: BTreeSet::from([OpenRequest::NeedApplicableLaw {
                issue: "formation".into(),
                candidates: vec!["alpha.txt".into(), "beta.txt".into()],
            }]),
            trace: TraceId::of(b"need-law"),
        };
        assert!(matches!(suspended, Outcome::Suspended { .. }));
    }
}
