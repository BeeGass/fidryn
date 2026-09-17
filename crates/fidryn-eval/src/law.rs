//! Applicable-law oracle. Rank by source weight, then later-in-time.

use fidryn_core::{Instant, ManifestArtifact};

/// Rank candidates: Binding > Controlling > Persuasive > Explanatory, then
/// later-in-time using parsed dates. Unparseable or equal times stay tied.
///
/// A unique winner is `Ok`. Same-rank ties return the tied set; list order is
/// never a tie-break.
pub fn select_applicable_law(
    candidates: &[ManifestArtifact],
) -> Result<ManifestArtifact, Vec<ManifestArtifact>> {
    if candidates.is_empty() {
        return Err(Vec::new());
    }
    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }
    let best_weight = candidates
        .iter()
        .map(|c| c.weight)
        .min()
        .expect("non-empty candidates");
    let mut tied: Vec<ManifestArtifact> = candidates
        .iter()
        .filter(|c| c.weight == best_weight)
        .cloned()
        .collect();
    if tied.len() == 1 {
        return Ok(tied.pop().expect("len == 1"));
    }

    let parsed: Vec<Option<Instant>> = tied.iter().map(|c| parse_effective(&c.effective)).collect();
    if parsed.iter().all(Option::is_some) {
        let best_time = parsed.iter().copied().flatten().max();
        let mut winners: Vec<ManifestArtifact> = tied
            .into_iter()
            .zip(parsed)
            .filter(|(_, t)| *t == best_time)
            .map(|(a, _)| a)
            .collect();
        return unique_or_tied(&mut winners);
    }

    sort_tied(&mut tied);
    Err(tied)
}

fn unique_or_tied(
    winners: &mut Vec<ManifestArtifact>,
) -> Result<ManifestArtifact, Vec<ManifestArtifact>> {
    match winners.len() {
        1 => Ok(winners.pop().expect("len == 1")),
        _ => {
            sort_tied(winners);
            Err(std::mem::take(winners))
        }
    }
}

fn sort_tied(tied: &mut [ManifestArtifact]) {
    tied.sort_by(|a, b| a.path.cmp(&b.path).then(a.digest.cmp(&b.digest)));
}

fn parse_effective(text: &str) -> Option<Instant> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if let Ok(instant) = Instant::parse(text) {
        return Some(instant);
    }
    if !text.contains('T')
        && let Ok(instant) = Instant::parse(&format!("{text}T00:00:00Z"))
    {
        return Some(instant);
    }
    let normalized = normalize_iso_date(text)?;
    Instant::parse(&format!("{normalized}T00:00:00Z")).ok()
}

fn normalize_iso_date(text: &str) -> Option<String> {
    let mut parts = text.split('-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::{OpenRequest, Outcome, SourceWeight, TraceId};
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
    fn unpadded_and_padded_same_date_stay_tied() {
        let a = artifact("a.txt", "2024-1-1", SourceWeight::Controlling);
        let b = artifact("b.txt", "2024-01-01", SourceWeight::Controlling);
        let tied = select_applicable_law(&[b, a]).unwrap_err();
        assert_eq!(tied.len(), 2);
        assert_eq!(tied[0].path, "a.txt");
        assert_eq!(tied[1].path, "b.txt");
    }

    #[test]
    fn parsed_later_date_beats_lexicographically_larger_earlier_date() {
        let jan = artifact("jan.txt", "2024-1-15", SourceWeight::Controlling);
        let feb = artifact("feb.txt", "2024-02-01", SourceWeight::Controlling);
        assert_eq!(
            select_applicable_law(&[jan.clone(), feb.clone()])
                .unwrap()
                .path,
            "feb.txt"
        );
        assert_eq!(select_applicable_law(&[feb, jan]).unwrap().path, "feb.txt");
    }

    #[test]
    fn unparseable_effective_dates_stay_tied() {
        let a = artifact("a.txt", "session-law", SourceWeight::Persuasive);
        let b = artifact("b.txt", "slip-op", SourceWeight::Persuasive);
        let tied = select_applicable_law(&[b, a]).unwrap_err();
        assert_eq!(tied.len(), 2);
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
