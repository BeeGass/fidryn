//! Bounded exhaustive SAT over declared finite domains.
//!
//! The solver never invents a domain. Every assignment is a member of the
//! caller-supplied completion space. Explore uses [`dpll`] conceptually to
//! prune incompatible completions; the search remains exhaustive on that
//! declared space.

use fidryn_core::Value;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Domain {
    pub name: String,
    pub values: Vec<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub bindings: BTreeMap<String, Value>,
}

/// A nogood: the conjunction of these `(variable, value)` pairs must not
/// all hold.
///
/// `[(x, a), (y, b)]` forbids `x = a ∧ y = b` (equivalently requires
/// `x ≠ a ∨ y ≠ b`). This is the dual of a SAT clause over equality
/// literals. An empty clause is the empty conjunction, which always holds,
/// so it forbids every assignment.
pub type Clause = Vec<(String, Value)>;

/// Cartesian product of finite domains, in domain-name order.
pub fn enumerate(domains: &[Domain]) -> Vec<Assignment> {
    if domains.is_empty() {
        return vec![Assignment {
            bindings: BTreeMap::new(),
        }];
    }
    let mut acc = vec![Assignment {
        bindings: BTreeMap::new(),
    }];
    for domain in domains {
        let mut next = Vec::new();
        for prefix in &acc {
            for value in &domain.values {
                let mut bindings = prefix.bindings.clone();
                bindings.insert(domain.name.clone(), value.clone());
                next.push(Assignment { bindings });
            }
        }
        acc = next;
    }
    acc
}

/// DPLL-style search: unit-prefer bindings that satisfy `ok`.
pub fn search<F>(domains: &[Domain], mut ok: F) -> Vec<Assignment>
where
    F: FnMut(&Assignment) -> bool,
{
    enumerate(domains).into_iter().filter(|a| ok(a)).collect()
}

/// Bounded DPLL over caller-supplied finite domains and nogood clauses.
///
/// Returns every total assignment drawn from `domains` that violates none
/// of `clauses`. Variables mentioned in a clause but absent from `domains`
/// cannot take the forbidden value, so that nogood cannot fire.
pub fn dpll(domains: &[Domain], clauses: &[Clause]) -> Vec<Assignment> {
    let mut remaining = BTreeMap::new();
    for domain in domains {
        remaining.insert(domain.name.clone(), domain.values.clone());
    }
    let mut out = Vec::new();
    search_dpll(remaining, BTreeMap::new(), clauses, &mut out);
    sort_assignments(&mut out);
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
) {
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
        search_dpll(remaining.clone(), next_assigned, clauses, out);
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

    #[test]
    fn product_is_exhaustive() {
        let domains = vec![
            domain(
                "I",
                vec![Value::String("I1".into()), Value::String("I2".into())],
            ),
            domain("C", vec![Value::Bool(true), Value::Bool(false)]),
        ];
        assert_eq!(enumerate(&domains).len(), 4);
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
