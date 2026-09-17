//! Closed value fragment checker.
//!
//! Independent of the trusted evaluator: no handlers, duty, Observe, or adapt.
//! Supports literals ([`Term::Bool`], [`Term::Int`], [`Term::Decimal`],
//! [`Term::String`]), [`Term::Ident`] (assignment / facts overlay of admitted
//! slots), Boolean `not` / `||` / `&&`, integer `==` / `!=` / `<` / `<=` /
//! `>` / `>=` / `+` / `-` / `*`, `if`, `seq` / `transaction` of fragment
//! steps, `require`, and [`Term::Field`] on maps/records in the assignment.
//! Covering of other plans still uses the trusted evaluator.

use fidryn_core::{BinOp, Term, Value};
use std::collections::BTreeMap;

/// Whether `term` is in the closed Boolean/ident fragment.
///
/// This is the Boolean subset of [`is_pure_fragment`]: [`Term::Bool`],
/// [`Term::Ident`], and `not` / `||` / `&&`. Integer arithmetic, `if`,
/// `seq`, `require`, and field access are pure but not Boolean.
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
        Term::Call { callee, args } if is_not_ctor(callee) && args.len() == 1 => {
            is_boolean_fragment(&args[0])
        }
        Term::Call { callee, args } if is_and_ctor(callee) || is_or_ctor(callee) => {
            args.len() == 2 && args.iter().all(is_boolean_fragment)
        }
        _ => false,
    }
}

/// Whether `term` is in the closed value fragment (no handlers, duty, or Observe).
pub fn is_pure_fragment(term: &Term) -> bool {
    match term {
        Term::Bool(_) | Term::Int(_) | Term::Decimal(_) | Term::String(_) | Term::Ident(_) => true,
        Term::If { cond, then, else_ } => {
            is_pure_fragment(cond) && is_pure_fragment(then) && is_pure_fragment(else_)
        }
        Term::Field { base, .. } => is_pure_fragment(base),
        Term::Binary { op, left, right } if is_pure_binop(*op) => {
            is_pure_fragment(left) && is_pure_fragment(right)
        }
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => is_pure_apply(ctor, args),
        _ => false,
    }
}

/// Evaluate a closed value-fragment term against `assignment`.
///
/// `assignment` is the admitted-slot overlay (slot keys, stripped names,
/// and original facts that were not overwritten). Does not consult
/// handlers, duty state, or adapt. `require false` is an error so covering
/// cannot claim a determinate answer for a failed guard.
pub fn eval_fragment(term: &Term, assignment: &BTreeMap<String, Value>) -> Result<Value, String> {
    if !is_pure_fragment(term) {
        return Err("term is not in the closed value fragment".into());
    }
    eval_closed(term, assignment)
}

fn is_pure_apply(ctor: &str, args: &[Term]) -> bool {
    if is_not_ctor(ctor) && args.len() == 1 {
        return is_pure_fragment(&args[0]);
    }
    if fragment_binop_ctor(ctor).is_some() {
        return args.len() == 2 && args.iter().all(is_pure_fragment);
    }
    if is_if_ctor(ctor) && args.len() == 3 {
        return args.iter().all(is_pure_fragment);
    }
    if is_field_ctor(ctor) && args.len() == 2 {
        return is_pure_fragment(&args[0]) && is_field_name(&args[1]);
    }
    if is_seq(ctor) || is_transaction(ctor) {
        return !args.is_empty() && args.iter().all(is_pure_fragment);
    }
    if is_require(ctor) {
        return !args.is_empty() && is_pure_fragment(&args[0]);
    }
    false
}

fn eval_closed(term: &Term, assignment: &BTreeMap<String, Value>) -> Result<Value, String> {
    match term {
        Term::Bool(b) => Ok(Value::Bool(*b)),
        Term::Int(i) => Ok(Value::Int(*i)),
        Term::Decimal(d) => Ok(Value::Decimal(*d)),
        Term::String(s) => Ok(Value::String(s.clone())),
        Term::Ident(name) => lookup_ident(name, assignment),
        Term::If { cond, then, else_ } => eval_if(cond, then, else_, assignment),
        Term::Field { base, name } => eval_field(base, name, assignment),
        Term::Binary { op, left, right } => {
            let op = binop_name(*op)?;
            let left = eval_closed(left, assignment)?;
            let right = eval_closed(right, assignment)?;
            eval_binop(op, &left, &right)
        }
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
            eval_apply(ctor, args, assignment)
        }
        _ => Err("term is not in the closed value fragment".into()),
    }
}

fn eval_apply(
    ctor: &str,
    args: &[Term],
    assignment: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    if is_not_ctor(ctor) && args.len() == 1 {
        return Ok(Value::Bool(!as_bool(&eval_closed(&args[0], assignment)?)?));
    }
    if let Some(op) = fragment_binop_ctor(ctor)
        && args.len() == 2
    {
        let left = eval_closed(&args[0], assignment)?;
        let right = eval_closed(&args[1], assignment)?;
        return eval_binop(op, &left, &right);
    }
    if is_if_ctor(ctor) && args.len() == 3 {
        return eval_if(&args[0], &args[1], &args[2], assignment);
    }
    if is_field_ctor(ctor) && args.len() == 2 {
        return eval_field(&args[0], field_name(&args[1])?, assignment);
    }
    if is_seq(ctor) || is_transaction(ctor) {
        return eval_seq(args, assignment);
    }
    if is_require(ctor) {
        return eval_require(args, assignment);
    }
    Err("term is not in the closed value fragment".into())
}

fn eval_if(
    cond: &Term,
    then: &Term,
    else_: &Term,
    assignment: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    match eval_closed(cond, assignment)? {
        Value::Bool(true) => eval_closed(then, assignment),
        Value::Bool(false) => eval_closed(else_, assignment),
        other => Err(format!("if condition `{other:?}`")),
    }
}

fn eval_seq(args: &[Term], assignment: &BTreeMap<String, Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("empty seq".into());
    }
    let mut last = None;
    for arg in args {
        last = Some(eval_closed(arg, assignment)?);
    }
    last.ok_or_else(|| "empty seq".into())
}

fn eval_require(args: &[Term], assignment: &BTreeMap<String, Value>) -> Result<Value, String> {
    if args.is_empty() {
        return Err("require expects a condition".into());
    }
    match eval_closed(&args[0], assignment)? {
        Value::Bool(true) => Ok(Value::Unit),
        Value::Bool(false) => Err("requirement failed".into()),
        other => Err(format!("require condition `{other:?}`")),
    }
}

fn eval_field(
    base: &Term,
    name: &str,
    assignment: &BTreeMap<String, Value>,
) -> Result<Value, String> {
    match eval_closed(base, assignment)? {
        Value::Map(fields) | Value::Ctor { fields, .. } => fields
            .get(name)
            .cloned()
            .ok_or_else(|| format!("missing field `{name}`")),
        other => Err(format!("field access on `{other:?}`")),
    }
}

fn eval_binop(op: &str, left: &Value, right: &Value) -> Result<Value, String> {
    match op {
        "+" | "-" | "*" => eval_int_arith(op, left, right),
        "&&" => Ok(Value::Bool(as_bool(left)? && as_bool(right)?)),
        "||" => Ok(Value::Bool(as_bool(left)? || as_bool(right)?)),
        "==" => Ok(Value::Bool(left == right)),
        "!=" => Ok(Value::Bool(left != right)),
        "<" | "<=" | ">" | ">=" => eval_int_cmp(op, left, right),
        _ => Err(format!(
            "operator `{op}` is not in the closed value fragment"
        )),
    }
}

fn eval_int_arith(op: &str, left: &Value, right: &Value) -> Result<Value, String> {
    let a = as_int(left)?;
    let b = as_int(right)?;
    let result = match op {
        "+" => a.checked_add(b),
        "-" => a.checked_sub(b),
        "*" => a.checked_mul(b),
        _ => return Err(format!("arithmetic `{op}`")),
    };
    result
        .map(Value::Int)
        .ok_or_else(|| format!("integer overflow in `{op}`"))
}

fn eval_int_cmp(op: &str, left: &Value, right: &Value) -> Result<Value, String> {
    let a = as_int(left)?;
    let b = as_int(right)?;
    let result = match op {
        "<" => a < b,
        "<=" => a <= b,
        ">" => a > b,
        ">=" => a >= b,
        _ => return Err(format!("comparator `{op}`")),
    };
    Ok(Value::Bool(result))
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

fn as_int(value: &Value) -> Result<i64, String> {
    match value {
        Value::Int(i) => Ok(*i),
        other => Err(format!("integer operator on non-int `{other:?}`")),
    }
}

fn field_name(term: &Term) -> Result<&str, String> {
    match term {
        Term::Ident(name) | Term::String(name) => Ok(name),
        other => Err(format!("field name `{other:?}`")),
    }
}

fn is_field_name(term: &Term) -> bool {
    matches!(term, Term::Ident(_) | Term::String(_))
}

fn is_pure_binop(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq
            | BinOp::Ne
            | BinOp::Lt
            | BinOp::Le
            | BinOp::Gt
            | BinOp::Ge
            | BinOp::Add
            | BinOp::Sub
            | BinOp::Mul
            | BinOp::And
            | BinOp::Or
    )
}

fn binop_name(op: BinOp) -> Result<&'static str, String> {
    match op {
        BinOp::Add => Ok("+"),
        BinOp::Sub => Ok("-"),
        BinOp::Mul => Ok("*"),
        BinOp::Eq => Ok("=="),
        BinOp::Ne => Ok("!="),
        BinOp::Lt => Ok("<"),
        BinOp::Le => Ok("<="),
        BinOp::Gt => Ok(">"),
        BinOp::Ge => Ok(">="),
        BinOp::And => Ok("&&"),
        BinOp::Or => Ok("||"),
        BinOp::Div => Err("operator `/` is not in the closed value fragment".into()),
    }
}

fn fragment_binop_ctor(ctor: &str) -> Option<&'static str> {
    match ctor {
        "+" | "add" | "plus" => Some("+"),
        "-" | "sub" | "minus" => Some("-"),
        "*" | "mul" | "times" => Some("*"),
        "&&" | "and" => Some("&&"),
        "||" | "or" => Some("||"),
        "==" | "eq" | "=" => Some("=="),
        "!=" | "ne" => Some("!="),
        "<" | "lt" => Some("<"),
        "<=" | "le" => Some("<="),
        ">" | "gt" => Some(">"),
        ">=" | "ge" => Some(">="),
        _ => None,
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

fn is_if_ctor(ctor: &str) -> bool {
    ctor == "if" || ctor == "If" || ctor.eq_ignore_ascii_case("cond")
}

fn is_field_ctor(ctor: &str) -> bool {
    ctor == "field" || ctor == "Field"
}

fn is_seq(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("seq")
}

fn is_transaction(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("transaction")
}

fn is_require(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("require")
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

    fn add(left: Term, right: Term) -> Term {
        Term::Binary {
            op: BinOp::Add,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    fn require(cond: Term) -> Term {
        Term::Apply {
            ctor: "require".into(),
            args: vec![cond],
        }
    }

    fn seq(args: Vec<Term>) -> Term {
        Term::Apply {
            ctor: "seq".into(),
            args,
        }
    }

    fn duty_step() -> Term {
        Term::Apply {
            ctor: "duty_step".into(),
            args: vec![Term::Ident("pay".into()), Term::Ident("attach".into())],
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
        assert!(is_pure_fragment(&term));
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
    fn eval_fragment_int_add_is_two() {
        let term = add(Term::Int(1), Term::Int(1));
        assert!(is_pure_fragment(&term));
        assert!(!is_boolean_fragment(&term));
        assert_eq!(
            eval_fragment(&term, &BTreeMap::new()).unwrap(),
            Value::Int(2)
        );
    }

    #[test]
    fn eval_fragment_if_true_then_three() {
        let term = Term::If {
            cond: Box::new(Term::Bool(true)),
            then: Box::new(Term::Int(3)),
            else_: Box::new(Term::Int(4)),
        };
        assert!(is_pure_fragment(&term));
        assert_eq!(
            eval_fragment(&term, &BTreeMap::new()).unwrap(),
            Value::Int(3)
        );
    }

    #[test]
    fn eval_fragment_seq_require_true_is_seven() {
        let term = seq(vec![require(Term::Bool(true)), Term::Int(7)]);
        assert!(is_pure_fragment(&term));
        assert!(!is_boolean_fragment(&term));
        assert_eq!(
            eval_fragment(&term, &BTreeMap::new()).unwrap(),
            Value::Int(7)
        );
    }

    #[test]
    fn eval_fragment_seq_require_false_is_err() {
        let term = seq(vec![require(Term::Bool(false)), Term::Int(7)]);
        assert!(is_pure_fragment(&term));
        let err = eval_fragment(&term, &BTreeMap::new()).expect_err("require false");
        assert!(err.contains("requirement failed"), "{err}");
    }

    #[test]
    fn eval_fragment_transaction_matches_seq_for_pure_values() {
        let term = Term::Apply {
            ctor: "transaction".into(),
            args: vec![require(Term::Bool(true)), Term::Int(5)],
        };
        assert!(is_pure_fragment(&term));
        assert_eq!(
            eval_fragment(&term, &BTreeMap::new()).unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn eval_fragment_field_on_assignment_map() {
        let term = Term::Field {
            base: Box::new(Term::Ident("rec".into())),
            name: "n".into(),
        };
        assert!(is_pure_fragment(&term));
        let env = BTreeMap::from([(
            "rec".into(),
            Value::Map(BTreeMap::from([("n".into(), Value::Int(9))])),
        )]);
        assert_eq!(eval_fragment(&term, &env).unwrap(), Value::Int(9));
    }

    #[test]
    fn eval_fragment_string_and_eq() {
        let term = Term::Binary {
            op: BinOp::Eq,
            left: Box::new(Term::String("a".into())),
            right: Box::new(Term::String("a".into())),
        };
        assert!(is_pure_fragment(&term));
        assert_eq!(
            eval_fragment(&term, &BTreeMap::new()).unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn eval_fragment_int_cmp_and_mul() {
        let lt = Term::Binary {
            op: BinOp::Lt,
            left: Box::new(Term::Int(1)),
            right: Box::new(Term::Int(2)),
        };
        let mul = Term::Binary {
            op: BinOp::Mul,
            left: Box::new(Term::Int(3)),
            right: Box::new(Term::Int(4)),
        };
        assert_eq!(
            eval_fragment(&lt, &BTreeMap::new()).unwrap(),
            Value::Bool(true)
        );
        assert_eq!(
            eval_fragment(&mul, &BTreeMap::new()).unwrap(),
            Value::Int(12)
        );
    }

    #[test]
    fn eval_fragment_rejects_arith_on_duty_map() {
        let term = add(Term::Ident("duty".into()), Term::Int(1));
        assert!(is_pure_fragment(&term));
        let env = BTreeMap::from([(
            "duty".into(),
            Value::Map(BTreeMap::from([(
                "status".into(),
                Value::String("Attached".into()),
            )])),
        )]);
        let err = eval_fragment(&term, &env).expect_err("duty is not an int");
        assert!(err.contains("non-int"), "{err}");
    }

    #[test]
    fn eval_fragment_rejects_duty_observe_and_div() {
        let div = Term::Binary {
            op: BinOp::Div,
            left: Box::new(Term::Int(4)),
            right: Box::new(Term::Int(2)),
        };
        assert!(!is_pure_fragment(&div));
        for ctor in [
            "duty_step",
            "duty_status",
            "observed",
            "Observe",
            "require_authority",
        ] {
            let term = Term::Apply {
                ctor: ctor.into(),
                args: vec![Term::Bool(true)],
            };
            assert!(!is_boolean_fragment(&term), "{ctor}");
            assert!(!is_pure_fragment(&term), "{ctor}");
            let err = eval_fragment(&term, &BTreeMap::new()).expect_err(ctor);
            assert!(err.contains("fragment"), "{err}");
        }
        let err = eval_fragment(&div, &BTreeMap::new()).expect_err("div");
        assert!(err.contains("fragment"), "{err}");
        assert!(!is_pure_fragment(&duty_step()));
        assert!(!is_pure_fragment(&seq(vec![duty_step(), Term::Bool(true)])));
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
