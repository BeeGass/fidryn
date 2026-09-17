//! Fidryn SMT-lite over caller-supplied finite domains.
//!
//! This is **not Z3**. There is no SMT-LIB frontend, no bitvector or
//! linear-arithmetic theory, and no `z3-sys` / C solver. The fragment is
//! propositional SAT plus finite-domain equalities and disequalities,
//! decided by DPLL on the declared space.

use crate::{Assignment, Clause, Domain, dpll_first};
use fidryn_core::Value;
use std::collections::BTreeMap;
use std::fmt;

/// Finite-domain equality `x = v` or disequality `x ≠ v`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Atom {
    Eq { var: String, value: Value },
    Ne { var: String, value: Value },
}

/// SMT-lite constraint. The slice passed to [`smt_check`] is a conjunction.
///
/// A [`Constraint::Nogood`] is a conjunction of equalities that must **not**
/// all hold (the existing DPLL clause).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Constraint {
    Eq { var: String, value: Value },
    Ne { var: String, value: Value },
    Nogood(Clause),
}

impl Constraint {
    #[must_use]
    pub fn eq(var: impl Into<String>, value: Value) -> Self {
        Self::Eq {
            var: var.into(),
            value,
        }
    }

    #[must_use]
    pub fn ne(var: impl Into<String>, value: Value) -> Self {
        Self::Ne {
            var: var.into(),
            value,
        }
    }

    #[must_use]
    pub fn nogood(pairs: Clause) -> Self {
        Self::Nogood(pairs)
    }

    fn mentioned_vars(&self) -> Vec<&str> {
        match self {
            Self::Eq { var, .. } | Self::Ne { var, .. } => vec![var.as_str()],
            Self::Nogood(pairs) => pairs.iter().map(|(var, _)| var.as_str()).collect(),
        }
    }
}

impl From<Atom> for Constraint {
    fn from(atom: Atom) -> Self {
        match atom {
            Atom::Eq { var, value } => Self::Eq { var, value },
            Atom::Ne { var, value } => Self::Ne { var, value },
        }
    }
}

/// One SMT-lite answer. [`SmtAnswer::Unknown`] is not Unsat: the fragment
/// cannot decide the query (missing domain, unsupported theory).
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub enum SmtAnswer {
    Sat(Assignment),
    Unsat,
    Unknown { reason: String },
}

impl SmtAnswer {
    pub fn is_sat(&self) -> bool {
        matches!(self, Self::Sat(_))
    }

    pub fn is_unsat(&self) -> bool {
        matches!(self, Self::Unsat)
    }
}

impl fmt::Display for SmtAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sat(assignment) => write!(f, "sat {assignment:?}"),
            Self::Unsat => f.write_str("unsat"),
            Self::Unknown { reason } => write!(f, "unknown: {reason}"),
        }
    }
}

/// Decide propositional SAT and finite-domain equalities/disequalities.
///
/// Not a general SMT solver. Variables named in `constraints` but absent
/// from `domains` yield [`SmtAnswer::Unknown`]; the solver never invents
/// a sort. An empty domain, or an equality whose value is not a member of
/// the declared domain, is [`SmtAnswer::Unsat`].
pub fn smt_check(domains: &[Domain], constraints: &[Constraint]) -> SmtAnswer {
    if let Some(reason) = unbound_reason(domains, constraints) {
        return SmtAnswer::Unknown { reason };
    }

    let mut remaining: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    for domain in domains {
        remaining.insert(domain.name.clone(), domain.values.clone());
    }
    let mut clauses: Vec<Clause> = Vec::new();

    for constraint in constraints {
        match constraint {
            Constraint::Eq { var, value } => {
                let Some(values) = remaining.get_mut(var) else {
                    return SmtAnswer::Unknown {
                        reason: format!("variable `{var}` is not in the declared finite domains"),
                    };
                };
                values.retain(|item| item == value);
                if values.is_empty() {
                    return SmtAnswer::Unsat;
                }
            }
            Constraint::Ne { var, value } => {
                clauses.push(vec![(var.clone(), value.clone())]);
            }
            Constraint::Nogood(pairs) => {
                clauses.push(pairs.clone());
            }
        }
    }

    let filtered: Vec<Domain> = domains
        .iter()
        .map(|domain| Domain {
            name: domain.name.clone(),
            values: remaining.get(&domain.name).cloned().unwrap_or_default(),
        })
        .collect();
    if filtered.iter().any(|domain| domain.values.is_empty()) {
        return SmtAnswer::Unsat;
    }

    match dpll_first(&filtered, &clauses) {
        Some(assignment) => SmtAnswer::Sat(assignment),
        None => SmtAnswer::Unsat,
    }
}

fn unbound_reason(domains: &[Domain], constraints: &[Constraint]) -> Option<String> {
    for constraint in constraints {
        for var in constraint.mentioned_vars() {
            if !domains.iter().any(|domain| domain.name == var) {
                return Some(format!(
                    "variable `{var}` is not in the declared finite domains"
                ));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enumerate;

    fn domain(name: &str, values: Vec<Value>) -> Domain {
        Domain {
            name: name.into(),
            values,
        }
    }

    fn bool_pair() -> Vec<Domain> {
        vec![
            domain("p", vec![Value::Bool(true), Value::Bool(false)]),
            domain("q", vec![Value::Bool(true), Value::Bool(false)]),
        ]
    }

    #[test]
    fn smt_equality_selects_the_declared_value() {
        let domains = vec![domain("x", vec![Value::Int(1), Value::Int(2)])];
        match smt_check(&domains, &[Constraint::eq("x", Value::Int(2))]) {
            SmtAnswer::Sat(assignment) => {
                assert_eq!(assignment.bindings.get("x"), Some(&Value::Int(2)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smt_disequality_avoids_the_forbidden_value() {
        let domains = vec![domain("x", vec![Value::Int(1), Value::Int(2)])];
        match smt_check(&domains, &[Constraint::ne("x", Value::Int(1))]) {
            SmtAnswer::Sat(assignment) => {
                assert_eq!(assignment.bindings.get("x"), Some(&Value::Int(2)));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smt_equality_outside_domain_is_unsat() {
        let domains = vec![domain("x", vec![Value::Int(1), Value::Int(2)])];
        assert!(
            smt_check(&domains, &[Constraint::eq("x", Value::Int(9))]).is_unsat(),
            "a value not in the declared domain is Unsat, not a new domain"
        );
    }

    #[test]
    fn smt_empty_nogood_is_unsat() {
        let domains = vec![domain("x", vec![Value::Int(1)])];
        assert!(smt_check(&domains, &[Constraint::nogood(Vec::new())]).is_unsat());
    }

    #[test]
    fn smt_unbound_variable_is_unknown_not_unsat() {
        let domains = vec![domain("x", vec![Value::Int(1)])];
        match smt_check(&domains, &[Constraint::eq("z", Value::Int(1))]) {
            SmtAnswer::Unknown { reason } => {
                assert!(reason.contains('z'), "{reason}");
            }
            other => panic!("missing domain must not be decided: {other:?}"),
        }
    }

    #[test]
    fn smt_propositional_clause_is_sat() {
        // (p ∨ q) as the nogood ¬p ∧ ¬q.
        let constraints = [Constraint::nogood(vec![
            ("p".into(), Value::Bool(false)),
            ("q".into(), Value::Bool(false)),
        ])];
        match smt_check(&bool_pair(), &constraints) {
            SmtAnswer::Sat(assignment) => {
                let p = assignment.bindings.get("p");
                let q = assignment.bindings.get("q");
                assert!(
                    p == Some(&Value::Bool(true)) || q == Some(&Value::Bool(true)),
                    "{assignment:?}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smt_propositional_contradiction_is_unsat() {
        let domains = vec![domain("p", vec![Value::Bool(true), Value::Bool(false)])];
        let constraints = [
            Constraint::eq("p", Value::Bool(true)),
            Constraint::ne("p", Value::Bool(true)),
        ];
        assert!(smt_check(&domains, &constraints).is_unsat());
    }

    #[test]
    fn smt_nogood_conjunction_agrees_with_dpll() {
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
        let dpll_models = crate::dpll(&domains, std::slice::from_ref(&forbid));
        match smt_check(&domains, &[Constraint::nogood(forbid.clone())]) {
            SmtAnswer::Sat(assignment) => {
                assert!(
                    dpll_models.contains(&assignment),
                    "smt model {assignment:?} not in dpll {dpll_models:?}"
                );
            }
            SmtAnswer::Unsat => assert!(dpll_models.is_empty()),
            other => panic!("{other:?}"),
        }
        assert_eq!(dpll_models.len(), 3);
    }

    #[test]
    fn smt_no_constraints_matches_first_enumerate() {
        let domains = vec![
            domain("x", vec![Value::Int(1), Value::Int(2)]),
            domain("y", vec![Value::Bool(true), Value::Bool(false)]),
        ];
        let product = enumerate(&domains);
        match smt_check(&domains, &[]) {
            SmtAnswer::Sat(assignment) => {
                assert_eq!(assignment, product[0]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smt_empty_product_is_unsat() {
        let domains = vec![domain("x", Vec::new())];
        assert!(smt_check(&domains, &[]).is_unsat());
    }

    #[test]
    fn smt_no_variables_no_constraints_is_empty_sat() {
        match smt_check(&[], &[]) {
            SmtAnswer::Sat(assignment) => assert!(assignment.bindings.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn smt_lite_does_not_invent_a_bitvector_theory() {
        let domains = vec![domain("x", vec![Value::Int(0), Value::Int(1)])];
        match smt_check(&domains, &[Constraint::eq("bvadd", Value::Int(1))]) {
            SmtAnswer::Unknown { .. } => {}
            other => panic!("bitvector names are not a theory: {other:?}"),
        }
    }
}
