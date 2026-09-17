//! Closed Boolean/ident fragment checker.
//!
//! Independent of the trusted evaluator: no handlers, duty, or adapt.
//! Supports [`Term::Bool`], [`Term::Ident`] (assignment / facts overlay of
//! admitted slots), and Boolean `not` / `||` / `&&` as [`Term::Apply`] or
//! [`Term::Binary`]. General covering still uses the trusted evaluator.

use fidryn_core::{BinOp, Term, Value};
use std::collections::BTreeMap;

/// Whether `term` is in the closed Boolean/ident fragment.
pub fn is_boolean_fragment(term: &Term) -> bool {
    match term {
        Term::Bool(_) | Term::Ident(_) => true,
        Term::Apply { ctor, args } if is_not_ctor(ctor) && args.len() == 1 => {
            is_boolean_fragment(&args[0])
        }
        Term::Apply { ctor, args } if is_and_ctor(ctor) || is_or_ctor(ctor) => {
            args.len() == 2 && args.iter().all(is_boolean_fragment)
        }
        Term::Binary {
            op: BinOp::And | BinOp::Or,
            left,
            right,
        } => is_boolean_fragment(left) && is_boolean_fragment(right),
        _ => false,
    }
}

/// Evaluate a closed Boolean/ident term against `assignment`.
///
/// `assignment` is the admitted-slot overlay (slot keys, stripped names,
/// and original facts that were not overwritten). Does not consult
/// handlers, duty state, or adapt.
pub fn eval_fragment(term: &Term, assignment: &BTreeMap<String, Value>) -> Result<Value, String> {
    if !is_boolean_fragment(term) {
        return Err("term is not in the closed Boolean/ident fragment".into());
    }
    eval_closed(term, assignment)
}

fn eval_closed(term: &Term, assignment: &BTreeMap<String, Value>) -> Result<Value, String> {
    match term {
        Term::Bool(b) => Ok(Value::Bool(*b)),
        Term::Ident(name) => lookup_ident(name, assignment),
        Term::Apply { ctor, args } if is_not_ctor(ctor) && args.len() == 1 => {
            Ok(Value::Bool(!as_bool(&eval_closed(&args[0], assignment)?)?))
        }
        Term::Apply { ctor, args } if is_and_ctor(ctor) && args.len() == 2 => {
            let left = as_bool(&eval_closed(&args[0], assignment)?)?;
            let right = as_bool(&eval_closed(&args[1], assignment)?)?;
            Ok(Value::Bool(left && right))
        }
        Term::Apply { ctor, args } if is_or_ctor(ctor) && args.len() == 2 => {
            let left = as_bool(&eval_closed(&args[0], assignment)?)?;
            let right = as_bool(&eval_closed(&args[1], assignment)?)?;
            Ok(Value::Bool(left || right))
        }
        Term::Binary {
            op: BinOp::And,
            left,
            right,
        } => {
            let left = as_bool(&eval_closed(left, assignment)?)?;
            let right = as_bool(&eval_closed(right, assignment)?)?;
            Ok(Value::Bool(left && right))
        }
        Term::Binary {
            op: BinOp::Or,
            left,
            right,
        } => {
            let left = as_bool(&eval_closed(left, assignment)?)?;
            let right = as_bool(&eval_closed(right, assignment)?)?;
            Ok(Value::Bool(left || right))
        }
        _ => Err("term is not in the closed Boolean/ident fragment".into()),
    }
}

fn lookup_ident(name: &str, assignment: &BTreeMap<String, Value>) -> Result<Value, String> {
    if let Some(value) = assignment.get(name) {
        return Ok(coerce_bool_atom(value));
    }
    match name {
        "true" => Ok(Value::Bool(true)),
        "false" => Ok(Value::Bool(false)),
        _ => Err(format!("unbound identifier `{name}`")),
    }
}

fn coerce_bool_atom(value: &Value) -> Value {
    match as_bool(value) {
        Ok(b) => Value::Bool(b),
        Err(_) => value.clone(),
    }
}

fn as_bool(value: &Value) -> Result<bool, String> {
    match value {
        Value::Bool(b) => Ok(*b),
        Value::String(text) | Value::Entity(text) => match text.as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!("boolean operator on non-bool `{text}`")),
        },
        other => Err(format!("boolean operator on non-bool `{other:?}`")),
    }
}

fn is_not_ctor(ctor: &str) -> bool {
    ctor == "not" || ctor == "!" || ctor.eq_ignore_ascii_case("Not")
}

fn is_and_ctor(ctor: &str) -> bool {
    ctor == "&&" || ctor == "and"
}

fn is_or_ctor(ctor: &str) -> bool {
    ctor == "||" || ctor == "or"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn or_not_ident(name: &str) -> Term {
        Term::Binary {
            op: BinOp::Or,
            left: Box::new(Term::Ident(name.into())),
            right: Box::new(Term::Apply {
                ctor: "not".into(),
                args: vec![Term::Ident(name.into())],
            }),
        }
    }

    #[test]
    fn eval_fragment_true_is_true() {
        assert_eq!(
            eval_fragment(&Term::Bool(true), &BTreeMap::new()).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn eval_fragment_false_is_false() {
        assert_eq!(
            eval_fragment(&Term::Bool(false), &BTreeMap::new()).unwrap(),
            Value::Bool(false)
        );
    }

    #[test]
    fn eval_fragment_or_not_is_tautology_on_both_assignments() {
        let term = or_not_ident("b");
        assert!(is_boolean_fragment(&term));
        let true_world = BTreeMap::from([("b".into(), Value::Bool(true))]);
        let false_world = BTreeMap::from([("b".into(), Value::Bool(false))]);
        assert_eq!(
            eval_fragment(&term, &true_world).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            eval_fragment(&term, &false_world).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn eval_fragment_or_not_accepts_string_true_false_labels() {
        let term = or_not_ident("b");
        let true_world = BTreeMap::from([("b".into(), Value::String("true".into()))]);
        let false_world = BTreeMap::from([("b".into(), Value::String("false".into()))]);
        assert_eq!(
            eval_fragment(&term, &true_world).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            eval_fragment(&term, &false_world).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn eval_fragment_apply_and_or_match_binary() {
        let apply_or = Term::Apply {
            ctor: "||".into(),
            args: vec![Term::Ident("b".into()), Term::Bool(false)],
        };
        let apply_and = Term::Apply {
            ctor: "and".into(),
            args: vec![Term::Ident("b".into()), Term::Bool(true)],
        };
        let env = BTreeMap::from([("b".into(), Value::Bool(true))]);
        assert_eq!(eval_fragment(&apply_or, &env).unwrap(), Value::Bool(true));
        assert_eq!(eval_fragment(&apply_and, &env).unwrap(), Value::Bool(true));
    }

    #[test]
    fn eval_fragment_rejects_unbound_ident() {
        let err = eval_fragment(&Term::Ident("b".into()), &BTreeMap::new()).expect_err("unbound");
        assert!(err.contains("unbound"), "{err}");
    }

    #[test]
    fn eval_fragment_rejects_seq_require_and_duty() {
        for ctor in ["seq", "require", "duty_step", "duty_status"] {
            let term = Term::Apply {
                ctor: ctor.into(),
                args: vec![Term::Bool(true)],
            };
            assert!(!is_boolean_fragment(&term), "{ctor}");
            let err = eval_fragment(&term, &BTreeMap::new()).expect_err(ctor);
            assert!(err.contains("fragment"), "{err}");
        }
    }

    #[test]
    fn fragment_source_does_not_depend_on_evaluator_or_handlers() {
        let impl_src = include_str!("pure.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("impl");
        assert!(
            !impl_src.contains("fidryn_eval"),
            "fragment checker must not import the trusted evaluator"
        );
        assert!(
            !impl_src.contains("fidryn_handlers"),
            "fragment checker must not import handlers"
        );
        assert!(
            !impl_src.contains("fidryn_adapt"),
            "fragment checker must not import adapt"
        );
    }
}
