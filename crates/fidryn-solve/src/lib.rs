//! Bounded exhaustive SAT over declared finite domains, plus SMT-lite.
//!
//! The solver never invents a domain. Every assignment is a member of the
//! caller-supplied completion space. Explore uses [`dpll`] conceptually to
//! prune incompatible completions; the search remains exhaustive on that
//! declared space.
//!
//! [`smt_check`] is Fidryn SMT-lite: propositional SAT and finite-domain
//! equalities/disequalities, decided by DPLL. It is **not Z3** (no SMT-LIB,
//! no bitvectors, no `z3-sys`).

mod smt;

use fidryn_core::Value;
use std::collections::BTreeMap;
use std::iter::FusedIterator;

pub use smt::{Atom, Constraint, SmtAnswer, smt_check};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Domain {
    pub name: String,
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub bindings: BTreeMap<String, Value>,
}

/// Cap on assignments emitted by [`stream`]. Hitting it is incomplete
/// coverage, not a determinate product.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchBudget {
    pub max_assignments: usize,
}

/// One step of a budgeted cartesian product. Terminal events are last.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchEvent {
    Assignment(Assignment),
    Exhausted,
    BudgetExceeded { emitted: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamPhase {
    Next,
    Exhausted,
    Done,
}

/// Odometer over `domains`. Assignments are built one at a time; the
/// full cartesian product is never materialized.
#[derive(Debug)]
pub struct StreamSearch<'a> {
    domains: &'a [Domain],
    indices: Vec<usize>,
    budget: usize,
    emitted: usize,
    built: usize,
    phase: StreamPhase,
}

/// Stream the cartesian product of `domains` until exhaustion or budget.
#[must_use]
pub fn stream(domains: &[Domain], budget: SearchBudget) -> StreamSearch<'_> {
    StreamSearch::new(domains, budget)
}

impl<'a> StreamSearch<'a> {
    fn new(domains: &'a [Domain], budget: SearchBudget) -> Self {
        let empty_product = domains.iter().any(|domain| domain.values.is_empty());
        Self {
            domains,
            indices: vec![0; domains.len()],
            budget: budget.max_assignments,
            emitted: 0,
            built: 0,
            phase: if empty_product {
                StreamPhase::Exhausted
            } else {
                StreamPhase::Next
            },
        }
    }

    /// Assignments yielded so far. Never the full product size once the
    /// budget has cut generation.
    #[must_use]
    pub fn emitted(&self) -> usize {
        self.emitted
    }

    /// Assignment values constructed. Equal to [`Self::emitted`] because a
    /// binding map is allocated only when an assignment is yielded.
    #[must_use]
    pub fn built(&self) -> usize {
        self.built
    }

    fn current_assignment(&self) -> Assignment {
        let mut bindings = BTreeMap::new();
        for (domain, &index) in self.domains.iter().zip(&self.indices) {
            bindings.insert(domain.name.clone(), domain.values[index].clone());
        }
        Assignment { bindings }
    }

    /// Advance the odometer. False when the last assignment was current.
    fn advance(&mut self) -> bool {
        if self.indices.is_empty() {
            return false;
        }
        for i in (0..self.indices.len()).rev() {
            self.indices[i] += 1;
            if self.indices[i] < self.domains[i].values.len() {
                return true;
            }
            self.indices[i] = 0;
        }
        false
    }
}

impl Iterator for StreamSearch<'_> {
    type Item = SearchEvent;

    fn next(&mut self) -> Option<Self::Item> {
        match self.phase {
            StreamPhase::Done => None,
            StreamPhase::Exhausted => {
                self.phase = StreamPhase::Done;
                Some(SearchEvent::Exhausted)
            }
            StreamPhase::Next if self.emitted >= self.budget => {
                self.phase = StreamPhase::Done;
                Some(SearchEvent::BudgetExceeded {
                    emitted: self.emitted,
                })
            }
            StreamPhase::Next => {
                let assignment = self.current_assignment();
                self.built += 1;
                self.emitted += 1;
                self.phase = if self.advance() {
                    StreamPhase::Next
                } else {
                    StreamPhase::Exhausted
                };
                Some(SearchEvent::Assignment(assignment))
            }
        }
    }
}

impl FusedIterator for StreamSearch<'_> {}

fn collect_assignments(domains: &[Domain], budget: SearchBudget) -> Vec<Assignment> {
    stream(domains, budget)
        .filter_map(|event| match event {
            SearchEvent::Assignment(assignment) => Some(assignment),
            SearchEvent::Exhausted | SearchEvent::BudgetExceeded { .. } => None,
        })
        .collect()
}

/// A nogood: the conjunction of these `(variable, value)` pairs must not
/// all hold.
///
/// `[(x, a), (y, b)]` forbids `x = a ∧ y = b` (equivalently requires
/// `x ≠ a ∨ y ≠ b`). This is the dual of a SAT clause over equality
/// literals. An empty clause is the empty conjunction, which always holds,
/// so it forbids every assignment.
pub type Clause = Vec<(String, Value)>;

/// Cartesian product of finite domains, in slice order.
///
/// Collects [`stream`] with an unbounded budget so existing callers keep
/// an exhaustive `Vec`. Prefer [`stream`] when the product may be large.
pub fn enumerate(domains: &[Domain]) -> Vec<Assignment> {
    collect_assignments(
        domains,
        SearchBudget {
            max_assignments: usize::MAX,
        },
    )
}

/// DPLL-style search: unit-prefer bindings that satisfy `ok`.
pub fn search<F>(domains: &[Domain], mut ok: F) -> Vec<Assignment>
where
    F: FnMut(&Assignment) -> bool,
{
    stream(
        domains,
        SearchBudget {
            max_assignments: usize::MAX,
        },
    )
    .filter_map(|event| match event {
        SearchEvent::Assignment(assignment) if ok(&assignment) => Some(assignment),
        SearchEvent::Assignment(_)
        | SearchEvent::Exhausted
        | SearchEvent::BudgetExceeded { .. } => None,
    })
    .collect()
}

/// Bounded DPLL over caller-supplied finite domains and nogood clauses.
///
/// Returns every total assignment drawn from `domains` that violates none
/// of `clauses`. Variables mentioned in a clause but absent from `domains`
/// cannot take the forbidden value, so that nogood cannot fire.
pub fn dpll(domains: &[Domain], clauses: &[Clause]) -> Vec<Assignment> {
    let mut out = dpll_collect(domains, clauses, usize::MAX);
    sort_assignments(&mut out);
    out
}

/// First SAT model in DPLL search order, or `None` if unsat.
pub(crate) fn dpll_first(domains: &[Domain], clauses: &[Clause]) -> Option<Assignment> {
    dpll_collect(domains, clauses, 1).into_iter().next()
}

fn dpll_collect(domains: &[Domain], clauses: &[Clause], limit: usize) -> Vec<Assignment> {
    let mut remaining = BTreeMap::new();
    for domain in domains {
        remaining.insert(domain.name.clone(), domain.values.clone());
    }
    let mut out = Vec::new();
    search_dpll(remaining, BTreeMap::new(), clauses, &mut out, limit);
    out
}

fn sort_assignments(assignments: &mut [Assignment]) {
    assignments.sort_by_key(assignment_key);
}

fn assignment_key(assignment: &Assignment) -> String {
    format!("{:?}", assignment.bindings)
}

fn search_dpll(
    mut remaining: BTreeMap<String, Vec<Value>>,
    mut assigned: BTreeMap<String, Value>,
    clauses: &[Clause],
    out: &mut Vec<Assignment>,
    limit: usize,
) {
    if out.len() >= limit {
        return;
    }
    if !propagate(&mut remaining, &mut assigned, clauses) {
        return;
    }
    let Some((var, domain)) = remaining
        .iter()
        .min_by(|(n1, d1), (n2, d2)| d1.len().cmp(&d2.len()).then(n1.cmp(n2)))
        .map(|(n, d)| (n.clone(), d.clone()))
    else {
        out.push(Assignment { bindings: assigned });
        return;
    };
    remaining.remove(&var);
    for value in domain {
        let mut next_assigned = assigned.clone();
        next_assigned.insert(var.clone(), value);
        search_dpll(remaining.clone(), next_assigned, clauses, out, limit);
        if out.len() >= limit {
            return;
        }
    }
}

fn propagate(
    remaining: &mut BTreeMap<String, Vec<Value>>,
    assigned: &mut BTreeMap<String, Value>,
    clauses: &[Clause],
) -> bool {
    loop {
        if remaining.values().any(Vec::is_empty) {
            return false;
        }
        let units: Vec<(String, Value)> = remaining
            .iter()
            .filter(|(_, domain)| domain.len() == 1)
            .map(|(name, domain)| (name.clone(), domain[0].clone()))
            .collect();
        for (name, value) in &units {
            remaining.remove(name);
            assigned.insert(name.clone(), value.clone());
        }
        let mut reduced = false;
        for clause in clauses {
            match nogood_status(clause, assigned, remaining) {
                Nogood::Conflict => return false,
                Nogood::Dead | Nogood::Open => {}
                Nogood::Unit(var, forbid) => {
                    let Some(domain) = remaining.get_mut(&var) else {
                        continue;
                    };
                    let before = domain.len();
                    domain.retain(|value| value != &forbid);
                    if domain.len() != before {
                        reduced = true;
                    }
                    if domain.is_empty() {
                        return false;
                    }
                }
            }
        }
        if units.is_empty() && !reduced {
            return true;
        }
    }
}

enum Nogood {
    Conflict,
    Dead,
    Open,
    Unit(String, Value),
}

fn nogood_status(
    clause: &[(String, Value)],
    assigned: &BTreeMap<String, Value>,
    remaining: &BTreeMap<String, Vec<Value>>,
) -> Nogood {
    if clause.is_empty() {
        return Nogood::Conflict;
    }
    let mut pending: Vec<(String, Value)> = Vec::new();
    for (var, forbid) in clause {
        if let Some(value) = assigned.get(var) {
            if value != forbid {
                return Nogood::Dead;
            }
            continue;
        }
        match remaining.get(var) {
            Some(domain) if domain.contains(forbid) => {
                pending.push((var.clone(), forbid.clone()));
            }
            _ => return Nogood::Dead,
        }
    }
    if pending.is_empty() {
        return Nogood::Conflict;
    }
    if pending.len() == 1 {
        let (var, forbid) = pending.swap_remove(0);
        return Nogood::Unit(var, forbid);
    }
    Nogood::Open
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(name: &str, values: Vec<Value>) -> Domain {
        Domain {
            name: name.into(),
            values,
        }
    }

    fn violates(assignment: &Assignment, clause: &Clause) -> bool {
        clause
            .iter()
            .all(|(var, value)| assignment.bindings.get(var) == Some(value))
    }

    fn two_by_two() -> Vec<Domain> {
        vec![
            domain(
                "I",
                vec![Value::String("I1".into()), Value::String("I2".into())],
            ),
            domain("C", vec![Value::Bool(true), Value::Bool(false)]),
        ]
    }

    #[test]
    fn product_is_exhaustive() {
        assert_eq!(enumerate(&two_by_two()).len(), 4);
    }

    #[test]
    fn stream_budget_one_on_two_by_two_exceeds_without_full_product() {
        let domains = two_by_two();
        let mut search = stream(&domains, SearchBudget { max_assignments: 1 });
        let first = search.next();
        assert!(
            matches!(first, Some(SearchEvent::Assignment(_))),
            "{first:?}"
        );
        match search.next() {
            Some(SearchEvent::BudgetExceeded { emitted }) => {
                assert!(emitted <= 1, "emitted={emitted}");
            }
            other => panic!("{other:?}"),
        }
        assert!(search.next().is_none());
        assert!(search.emitted() <= 1, "emitted={}", search.emitted());
        assert!(
            search.built() <= 1,
            "constructed {} assignments; full 2x2 product is 4",
            search.built()
        );
    }

    #[test]
    fn stream_exhausts_two_by_two_within_budget() {
        let domains = two_by_two();
        let events: Vec<_> = stream(&domains, SearchBudget { max_assignments: 4 }).collect();
        let assignments = events
            .iter()
            .filter(|event| matches!(event, SearchEvent::Assignment(_)))
            .count();
        assert_eq!(assignments, 4);
        assert!(matches!(events.last(), Some(SearchEvent::Exhausted)));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, SearchEvent::BudgetExceeded { .. }))
        );
    }

    #[test]
    fn stream_empty_domain_is_exhausted_without_assignments() {
        let domains = vec![domain("I", Vec::new())];
        let events: Vec<_> = stream(&domains, SearchBudget { max_assignments: 8 }).collect();
        assert_eq!(events, vec![SearchEvent::Exhausted]);
    }

    #[test]
    fn stream_no_domains_is_one_empty_assignment() {
        let events: Vec<_> = stream(&[], SearchBudget { max_assignments: 8 }).collect();
        match events.as_slice() {
            [SearchEvent::Assignment(assignment), SearchEvent::Exhausted] => {
                assert!(assignment.bindings.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn dpll_forbids_a_conjunction() {
        let domains = vec![
            domain("x", vec![Value::Int(1), Value::Int(2)]),
            domain(
                "y",
                vec![Value::String("a".into()), Value::String("b".into())],
            ),
        ];
        let forbid: Clause = vec![
            ("x".into(), Value::Int(1)),
            ("y".into(), Value::String("a".into())),
        ];
        let solutions = dpll(&domains, std::slice::from_ref(&forbid));
        assert_eq!(solutions.len(), 3);
        assert!(
            solutions
                .iter()
                .all(|assignment| !violates(assignment, &forbid))
        );
        let forbidden = Assignment {
            bindings: BTreeMap::from([
                ("x".into(), Value::Int(1)),
                ("y".into(), Value::String("a".into())),
            ]),
        };
        assert!(!solutions.contains(&forbidden));
    }

    #[test]
    fn dpll_empty_nogood_is_unsat() {
        let domains = vec![domain("x", vec![Value::Int(1)])];
        let empty: Clause = Vec::new();
        assert!(dpll(&domains, &[empty]).is_empty());
    }

    #[test]
    fn dpll_without_clauses_matches_enumerate() {
        let domains = vec![
            domain(
                "I",
                vec![Value::String("I1".into()), Value::String("I2".into())],
            ),
            domain("C", vec![Value::Bool(true), Value::Bool(false)]),
        ];
        let mut product = enumerate(&domains);
        sort_assignments(&mut product);
        assert_eq!(dpll(&domains, &[]), product);
    }

    #[test]
    fn dpll_matches_filtered_enumerate() {
        let domains = vec![
            domain("p", vec![Value::Bool(true), Value::Bool(false)]),
            domain("q", vec![Value::Bool(true), Value::Bool(false)]),
        ];
        let clauses: Vec<Clause> = vec![
            vec![
                ("p".into(), Value::Bool(true)),
                ("q".into(), Value::Bool(true)),
            ],
            vec![
                ("p".into(), Value::Bool(false)),
                ("q".into(), Value::Bool(false)),
            ],
        ];
        let mut brute = enumerate(&domains);
        brute.retain(|assignment| clauses.iter().all(|clause| !violates(assignment, clause)));
        sort_assignments(&mut brute);
        assert_eq!(dpll(&domains, &clauses), brute);
        assert_eq!(brute.len(), 2);
    }
}
