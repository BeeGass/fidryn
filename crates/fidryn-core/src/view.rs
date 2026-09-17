//! Frozen bitemporal projection of a [`CaseRecord`].
//!
//! Lookups of determinations, evidence, events, grants, and closures go
//! through this view so subsystems do not reimplement timestamp filters.
//! Valid time (`valid_at`) is the legal timeline; known time (`known_at`)
//! is record time.

use crate::case::{CaseDetermination, CaseRecord, ClosureRecord, EvidenceItem, LedgerEvent};
use crate::time::{Instant, Interval, RunContext};
use crate::value::Value;
use std::borrow::Cow;

/// Immutable case snapshot at a pair of clocks.
///
/// Holds a borrowed [`CaseRecord`] or an owned clone. The record is never
/// mutated through this view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenCaseView<'a> {
    case: Cow<'a, CaseRecord>,
    valid_at: Instant,
    known_at: Instant,
}

impl<'a> FrozenCaseView<'a> {
    /// Borrow `case` at `valid_at` (legal time) and `known_at` (record time).
    pub fn new(case: &'a CaseRecord, valid_at: Instant, known_at: Instant) -> Self {
        Self {
            case: Cow::Borrowed(case),
            valid_at,
            known_at,
        }
    }

    /// Own a snapshot so the view does not borrow the caller's record.
    pub fn owned(
        case: CaseRecord,
        valid_at: Instant,
        known_at: Instant,
    ) -> FrozenCaseView<'static> {
        FrozenCaseView {
            case: Cow::Owned(case),
            valid_at,
            known_at,
        }
    }

    /// Project `case` at the run context's valid time and record time.
    pub fn from_context(case: &'a CaseRecord, ctx: &RunContext) -> Self {
        Self::new(case, ctx.valid_time, ctx.record_time)
    }

    pub fn case(&self) -> &CaseRecord {
        &self.case
    }

    pub fn valid_at(&self) -> Instant {
        self.valid_at
    }

    pub fn known_at(&self) -> Instant {
        self.known_at
    }

    /// Determinations visible at `known_at`.
    ///
    /// `recorded_at > known_at` is excluded. Missing `recorded_at` stays
    /// visible so legacy records without a knowledge timestamp remain usable.
    pub fn determinations(&self) -> impl Iterator<Item = &CaseDetermination> + '_ {
        let known_at = self.known_at;
        self.case
            .determinations
            .iter()
            .filter(move |det| Self::determination_is_visible_at(det, known_at))
    }

    /// Evidence with `observed_at <= known_at`.
    pub fn evidence(&self) -> impl Iterator<Item = &EvidenceItem> + '_ {
        let known_at = self.known_at;
        self.case
            .evidence
            .iter()
            .filter(move |item| Self::evidence_is_visible_at(item, known_at))
    }

    /// Events admitted-visible at both clocks.
    ///
    /// `record_time` must be `<= known_at`. When the valid interval is not
    /// [`Interval::always`], it must contain `valid_at`.
    pub fn events(&self) -> impl Iterator<Item = &LedgerEvent> + '_ {
        let valid_at = self.valid_at;
        let known_at = self.known_at;
        self.case
            .events
            .iter()
            .filter(move |event| Self::event_is_visible_at(event, valid_at, known_at))
    }

    /// Authority grants visible at this view's clocks.
    ///
    /// Fact `authority_grants` are given and always included. Evidence with
    /// schema `AuthorityGrant` uses the evidence knowledge filter
    /// (`observed_at <= known_at`).
    pub fn grants(&self) -> impl Iterator<Item = &Value> + '_ {
        let from_facts = self.case.facts.get("authority_grants");
        let from_evidence = self.evidence().filter_map(|item| {
            if item.schema.eq_ignore_ascii_case("AuthorityGrant") {
                Some(&item.value)
            } else {
                None
            }
        });
        from_facts.into_iter().chain(from_evidence)
    }

    /// Closure records. They carry no clocks, so the full list is visible.
    pub fn closures(&self) -> impl Iterator<Item = &ClosureRecord> + '_ {
        self.case.closures.iter()
    }

    /// Knowledge filter for a determination at a concrete `known_at`.
    pub fn determination_is_visible_at(det: &CaseDetermination, known_at: Instant) -> bool {
        Self::determination_is_known(det.recorded_at, Some(known_at))
    }

    /// Knowledge filter used by observe/determine.
    ///
    /// Missing `recorded_at` stays visible (legacy records). `known_at = None`
    /// does not apply a knowledge filter, matching observe.
    pub fn determination_is_known(recorded_at: Option<Instant>, known_at: Option<Instant>) -> bool {
        match (recorded_at, known_at) {
            (_, None) => true,
            (None, Some(_)) => true,
            (Some(recorded), Some(known)) => recorded <= known,
        }
    }

    pub fn evidence_is_visible_at(item: &EvidenceItem, known_at: Instant) -> bool {
        Self::evidence_is_known(item.observed_at, Some(known_at))
    }

    /// Evidence knowledge filter. `known_at = None` does not filter by time.
    pub fn evidence_is_known(observed_at: Instant, known_at: Option<Instant>) -> bool {
        match known_at {
            None => true,
            Some(known) => observed_at <= known,
        }
    }

    /// Record-time half of event visibility (`record_time <= known_at`).
    pub fn event_is_known_at(event: &LedgerEvent, known_at: Instant) -> bool {
        event.record_time <= known_at
    }

    /// Both-clock event visibility used by [`Self::events`].
    pub fn event_is_visible_at(event: &LedgerEvent, valid_at: Instant, known_at: Instant) -> bool {
        if !Self::event_is_known_at(event, known_at) {
            return false;
        }
        event.valid_time == Interval::always() || event.valid_time.contains(valid_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn instant(text: &str) -> Instant {
        Instant::parse(text).unwrap()
    }

    fn clocks() -> (Instant, Instant, Instant) {
        (
            instant("2033-01-01T00:00:00Z"),
            instant("2033-06-01T00:00:00Z"),
            instant("2034-01-01T00:00:00Z"),
        )
    }

    #[test]
    fn future_determination_is_not_visible() {
        let (valid, known, future) = clocks();
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: true,
            decider: "Reviewer".into(),
            recorded_at: Some(future),
        });
        let view = FrozenCaseView::new(&case, valid, known);
        assert!(
            view.determinations().next().is_none(),
            "recorded_at after known_at must not be visible"
        );
    }

    #[test]
    fn determination_without_recorded_at_remains_visible() {
        let (valid, known, _) = clocks();
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "InvoiceIssued".into(),
            protocol: "Invoice".into(),
            established: false,
            decider: "Tribunal".into(),
            recorded_at: None,
        });
        let view = FrozenCaseView::new(&case, valid, known);
        let visible: Vec<_> = view.determinations().collect();
        assert_eq!(visible.len(), 1);
        assert!(!visible[0].established);
    }

    #[test]
    fn determination_at_known_at_is_visible() {
        let (valid, known, _) = clocks();
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: true,
            decider: "Reviewer".into(),
            recorded_at: Some(known),
        });
        let view = FrozenCaseView::new(&case, valid, known);
        assert_eq!(view.determinations().count(), 1);
    }

    #[test]
    fn determination_is_known_skips_filter_when_known_at_is_none() {
        let future = instant("2034-01-01T00:00:00Z");
        assert!(FrozenCaseView::determination_is_known(Some(future), None));
        assert!(FrozenCaseView::determination_is_known(None, Some(future)));
        assert!(!FrozenCaseView::determination_is_known(
            Some(future),
            Some(instant("2033-01-01T00:00:00Z"))
        ));
    }

    #[test]
    fn evidence_after_known_at_is_not_visible() {
        let (valid, known, future) = clocks();
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "PaymentRecord".into(),
            value: Value::String("late".into()),
            observed_at: future,
        });
        case.evidence.push(EvidenceItem {
            schema: "PaymentRecord".into(),
            value: Value::String("on-time".into()),
            observed_at: known,
        });
        let view = FrozenCaseView::new(&case, valid, known);
        let visible: Vec<_> = view.evidence().collect();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].value, Value::String("on-time".into()));
    }

    #[test]
    fn evidence_is_known_skips_filter_when_known_at_is_none() {
        let future = instant("2034-01-01T00:00:00Z");
        assert!(FrozenCaseView::evidence_is_known(future, None));
        assert!(!FrozenCaseView::evidence_is_known(
            future,
            Some(instant("2033-01-01T00:00:00Z"))
        ));
    }

    #[test]
    fn event_after_known_at_is_not_visible() {
        let (valid, known, future) = clocks();
        let mut case = CaseRecord::default();
        case.events.push(LedgerEvent {
            kind: "evidence".into(),
            valid_time: Interval::always(),
            record_time: future,
            payload: Value::Bool(true),
        });
        let view = FrozenCaseView::new(&case, valid, known);
        assert!(view.events().next().is_none());
        assert!(!FrozenCaseView::event_is_known_at(&case.events[0], known));
    }

    #[test]
    fn event_outside_valid_interval_is_not_visible() {
        let (valid, known, future) = clocks();
        let mut case = CaseRecord::default();
        case.events.push(LedgerEvent {
            kind: "evidence".into(),
            valid_time: Interval::from_instants(future, None).unwrap(),
            record_time: known,
            payload: Value::Bool(true),
        });
        let view = FrozenCaseView::new(&case, valid, known);
        assert!(
            view.events().next().is_none(),
            "a bounded valid interval that does not contain valid_at is invisible"
        );
    }

    #[test]
    fn always_interval_event_is_visible_at_any_valid_time() {
        let (valid, known, _) = clocks();
        let mut case = CaseRecord::default();
        case.events.push(LedgerEvent {
            kind: "evidence".into(),
            valid_time: Interval::always(),
            record_time: known,
            payload: Value::Bool(true),
        });
        let view = FrozenCaseView::new(&case, valid, known);
        assert_eq!(view.events().count(), 1);
        let earlier = FrozenCaseView::new(&case, instant("2000-01-01T00:00:00Z"), known);
        assert_eq!(earlier.events().count(), 1);
    }

    #[test]
    fn event_at_known_at_with_containing_interval_is_visible() {
        let (valid, known, future) = clocks();
        let mut case = CaseRecord::default();
        case.events.push(LedgerEvent {
            kind: "evidence".into(),
            valid_time: Interval::from_instants(valid, Some(future)).unwrap(),
            record_time: known,
            payload: Value::Bool(true),
        });
        let view = FrozenCaseView::new(&case, valid, known);
        assert_eq!(view.events().count(), 1);
    }

    #[test]
    fn future_grant_evidence_is_not_visible() {
        let (valid, known, future) = clocks();
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![Value::String("attach".into())]),
        );
        case.evidence.push(EvidenceItem {
            schema: "AuthorityGrant".into(),
            value: Value::Map(BTreeMap::from([(
                "action".into(),
                Value::String("perform".into()),
            )])),
            observed_at: future,
        });
        case.evidence.push(EvidenceItem {
            schema: "AuthorityGrant".into(),
            value: Value::Map(BTreeMap::from([(
                "action".into(),
                Value::String("discharge".into()),
            )])),
            observed_at: known,
        });
        let view = FrozenCaseView::new(&case, valid, known);
        let grants: Vec<_> = view.grants().cloned().collect();
        assert_eq!(grants.len(), 2);
        assert_eq!(grants[0], Value::Set(vec![Value::String("attach".into())]));
        match &grants[1] {
            Value::Map(fields) => {
                assert_eq!(
                    fields.get("action"),
                    Some(&Value::String("discharge".into()))
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn closures_are_visible_without_clocks() {
        let (valid, known, _) = clocks();
        let mut case = CaseRecord::default();
        case.closures.push(ClosureRecord {
            domain: "filings".into(),
            closed: true,
        });
        case.closures.push(ClosureRecord {
            domain: "occupancy".into(),
            closed: false,
        });
        let view = FrozenCaseView::new(&case, valid, known);
        let closures: Vec<_> = view.closures().collect();
        assert_eq!(closures.len(), 2);
        assert!(closures[0].closed);
        assert!(!closures[1].closed);
    }

    #[test]
    fn owned_snapshot_matches_borrowed() {
        let (valid, known, _) = clocks();
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "P".into(),
            protocol: "P".into(),
            established: true,
            decider: "Court".into(),
            recorded_at: Some(known),
        });
        case.evidence.push(EvidenceItem {
            schema: "PaymentRecord".into(),
            value: Value::Bool(true),
            observed_at: known,
        });
        let borrowed = FrozenCaseView::new(&case, valid, known);
        let owned = FrozenCaseView::owned(case.clone(), valid, known);
        assert_eq!(
            borrowed.determinations().count(),
            owned.determinations().count()
        );
        assert_eq!(borrowed.evidence().count(), owned.evidence().count());
        assert_eq!(borrowed.valid_at(), valid);
        assert_eq!(owned.known_at(), known);
        assert_eq!(borrowed.case(), owned.case());
    }

    #[test]
    fn from_context_uses_run_clocks() {
        let (valid, known, _) = clocks();
        let case = CaseRecord::default();
        let ctx = RunContext::new(valid, known);
        let view = FrozenCaseView::from_context(&case, &ctx);
        assert_eq!(view.valid_at(), valid);
        assert_eq!(view.known_at(), known);
    }
}
