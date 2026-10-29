//! Bounded evaluation of every structured verification assertion.

use crate::PropertyVerdict;
use fidryn_core::{BinOp, CoreDecl, CoreModule, Term, Value, VerificationBounds};
use std::collections::BTreeMap;

/// Construction and evaluation limits for a property invocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerificationBudget {
    pub max_steps: usize,
    pub max_domain_size: usize,
    pub max_depth: usize,
}

impl Default for VerificationBudget {
    fn default() -> Self {
        Self {
            max_steps: 65_536,
            max_domain_size: 4096,
            max_depth: 128,
        }
    }
}

pub fn verify_property(module: &CoreModule, name: &str) -> PropertyVerdict {
    verify_property_with_budget(module, name, VerificationBudget::default())
}

/// A false assertion disproves the conjunction. Unsupported expressions,
/// empty bounded sorts and exhausted budgets cannot establish a proof.
pub fn verify_property_with_budget(
    module: &CoreModule,
    name: &str,
    budget: VerificationBudget,
) -> PropertyVerdict {
    let Some(property) = module.verifications.iter().find(|item| item.name == name) else {
        let diagnostics = if module.queries.iter().any(|query| query.name == name) {
            format!("`{name}` is a query, not a declared verification property")
        } else {
            format!("unknown property {name}")
        };
        return PropertyVerdict::InvalidProperty { diagnostics };
    };
    let unknown = |reason| PropertyVerdict::Unknown {
        name: property.name.clone(),
        reason,
        bounds: property.bounds.clone(),
    };
    let legacy;
    let assertions = if property.assertions.is_empty() {
        // Historical Core artifacts had only a formula. Decode the supported
        // fragment conservatively; fresh compilation supplies assertions.
        legacy = match legacy_assertions(&property.formula) {
            Some(assertions) => assertions,
            None => return unknown("no supported structured verification assertion".into()),
        };
        &legacy
    } else {
        &property.assertions
    };
    if assertions.is_empty() {
        return unknown("verification contains no assertions".into());
    }
    let mut evaluator = PropertyEval {
        module,
        bounds: &property.bounds,
        budget,
        steps: 0,
        counterexample: None,
    };
    let mut unresolved = None;
    for (index, assertion) in assertions.iter().enumerate() {
        evaluator.counterexample = None;
        match evaluator.eval(assertion, &mut BTreeMap::new(), 0) {
            Ok(Value::Bool(true)) => {}
            Ok(Value::Bool(false)) => {
                return PropertyVerdict::Counterexample {
                    name: property.name.clone(),
                    detail: evaluator
                        .counterexample
                        .take()
                        .unwrap_or_else(|| format!("assertion {} evaluates to false", index + 1)),
                };
            }
            Ok(value) => {
                unresolved.get_or_insert_with(|| {
                    format!("assertion {} is not Boolean: {value:?}", index + 1)
                });
            }
            Err(reason) => {
                unresolved.get_or_insert(reason);
            }
        }
    }
    match unresolved {
        Some(reason) => unknown(reason),
        None => PropertyVerdict::Proved {
            name: property.name.clone(),
            bounds: property.bounds.clone(),
        },
    }
}

struct PropertyEval<'a> {
    module: &'a CoreModule,
    bounds: &'a VerificationBounds,
    budget: VerificationBudget,
    steps: usize,
    counterexample: Option<String>,
}

impl PropertyEval<'_> {
    fn eval(
        &mut self,
        term: &Term,
        env: &mut BTreeMap<String, Value>,
        depth: usize,
    ) -> Result<Value, String> {
        if depth > self.budget.max_depth || self.steps >= self.budget.max_steps {
            return Err("verification resource budget exhausted".into());
        }
        self.steps += 1;
        match term {
            Term::Bool(value) => Ok(Value::Bool(*value)),
            Term::Int(value) => Ok(Value::Int(*value)),
            Term::Decimal(value) => Ok(Value::Decimal(*value)),
            Term::String(value) => Ok(Value::String(value.clone())),
            Term::Instant(value) => Ok(Value::Instant(*value)),
            Term::Duration(value) => Ok(Value::Duration(*value)),
            Term::Ident(name) => env
                .get(name)
                .cloned()
                .or_else(|| self.constant(name))
                .ok_or_else(|| format!("unresolved verification identifier `{name}`")),
            Term::Binder(name) => env
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unbound verification binder `{name}`")),
            Term::Binary { op, left, right } => self.binary(*op, left, right, env, depth + 1),
            Term::If { cond, then, else_ } => match self.eval(cond, env, depth + 1)? {
                Value::Bool(true) => self.eval(then, env, depth + 1),
                Value::Bool(false) => self.eval(else_, env, depth + 1),
                _ => Err("verification condition is not Boolean".into()),
            },
            Term::Set(terms) => {
                if terms.len() > self.budget.max_domain_size {
                    return Err("verification domain exceeds resource budget".into());
                }
                terms
                    .iter()
                    .map(|term| self.eval(term, env, depth + 1))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::Set)
            }
            Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
                match (ctor.as_str(), args.as_slice()) {
                    (
                        "for_all" | "exists",
                        [Term::Ident(binder) | Term::Binder(binder), domain, body],
                    ) => self.quantify(ctor == "for_all", binder, domain, body, env, depth + 1),
                    ("not" | "!", [inner]) => match self.eval(inner, env, depth + 1)? {
                        Value::Bool(value) => Ok(Value::Bool(!value)),
                        _ => Err("verification negation is not Boolean".into()),
                    },
                    _ => Err(format!("unsupported verification operation `{ctor}`")),
                }
            }
            _ => Err("unsupported verification expression".into()),
        }
    }

    fn constant(&self, name: &str) -> Option<Value> {
        if self
            .module
            .declarations
            .iter()
            .any(|decl| matches!(decl, CoreDecl::Entity(entity) if entity.name == name))
        {
            return Some(Value::Entity(name.to_owned()));
        }
        for (prefix, count) in [
            ("person", self.bounds.persons),
            ("event", self.bounds.events),
            ("time", self.bounds.time_points),
        ] {
            if let Some(index) = name
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix('-'))
                .and_then(|index| index.parse::<u32>().ok())
                && index < count
            {
                return Some(Value::Entity(name.to_owned()));
            }
        }
        None
    }

    fn binary(
        &mut self,
        op: BinOp,
        left: &Term,
        right: &Term,
        env: &mut BTreeMap<String, Value>,
        depth: usize,
    ) -> Result<Value, String> {
        let a = self.eval(left, env, depth);
        if matches!((&op, &a), (BinOp::And, Ok(Value::Bool(false)))) {
            return Ok(Value::Bool(false));
        }
        if matches!((&op, &a), (BinOp::Or, Ok(Value::Bool(true)))) {
            return Ok(Value::Bool(true));
        }
        let b = self.eval(right, env, depth);
        if matches!((&op, &b), (BinOp::And, Ok(Value::Bool(false)))) {
            return Ok(Value::Bool(false));
        }
        if matches!((&op, &b), (BinOp::Or, Ok(Value::Bool(true)))) {
            return Ok(Value::Bool(true));
        }
        let (a, b) = (a?, b?);
        match op {
            BinOp::Eq => Ok(Value::Bool(a == b)),
            BinOp::Ne => Ok(Value::Bool(a != b)),
            BinOp::And | BinOp::Or => match (a, b) {
                (Value::Bool(a), Value::Bool(b)) => {
                    Ok(Value::Bool(if op == BinOp::And { a && b } else { a || b }))
                }
                _ => Err("verification Boolean operands have incompatible types".into()),
            },
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let ordering = match (a, b) {
                    (Value::Int(a), Value::Int(b)) => a.cmp(&b),
                    (Value::Decimal(a), Value::Decimal(b)) => a.cmp(&b),
                    (Value::Instant(a), Value::Instant(b)) => a.cmp(&b),
                    _ => {
                        return Err(
                            "verification comparison operands have incompatible types".into()
                        );
                    }
                };
                Ok(Value::Bool(match op {
                    BinOp::Lt => ordering.is_lt(),
                    BinOp::Le => ordering.is_le(),
                    BinOp::Gt => ordering.is_gt(),
                    _ => ordering.is_ge(),
                }))
            }
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => match (a, b) {
                (Value::Int(a), Value::Int(b)) => match op {
                    BinOp::Add => a.checked_add(b),
                    BinOp::Sub => a.checked_sub(b),
                    BinOp::Mul => a.checked_mul(b),
                    _ => a.checked_div(b),
                }
                .map(Value::Int)
                .ok_or_else(|| "invalid or overflowing verification arithmetic".into()),
                (Value::Decimal(a), Value::Decimal(b)) => match op {
                    BinOp::Add => a.checked_add(b),
                    BinOp::Sub => a.checked_sub(b),
                    BinOp::Mul => a.checked_mul(b),
                    _ => a.checked_div(b),
                }
                .map(Value::Decimal)
                .ok_or_else(|| "invalid or overflowing verification arithmetic".into()),
                _ => Err("verification arithmetic operands have incompatible types".into()),
            },
        }
    }

    fn quantify(
        &mut self,
        universal: bool,
        binder: &str,
        domain: &Term,
        body: &Term,
        env: &mut BTreeMap<String, Value>,
        depth: usize,
    ) -> Result<Value, String> {
        let finite = match domain {
            Term::Ident(name) => bounded_sort(name, self.bounds),
            _ => None,
        };
        let (size, explicit) = if let Some((count, prefix)) = finite {
            (count as usize, Some(prefix))
        } else {
            let Value::Set(values) = self.eval(domain, env, depth)? else {
                return Err("verification domain is not a declared finite set".into());
            };
            return self.quantify_values(universal, binder, body, env, depth, values.into_iter());
        };
        if size > self.budget.max_domain_size {
            return Err("verification domain exceeds resource budget".into());
        }
        let prefix = explicit.expect("bounded sort has a prefix");
        self.quantify_values(
            universal,
            binder,
            body,
            env,
            depth,
            (0..size).map(|index| Value::Entity(format!("{prefix}-{index}"))),
        )
    }

    fn quantify_values(
        &mut self,
        universal: bool,
        binder: &str,
        body: &Term,
        env: &mut BTreeMap<String, Value>,
        depth: usize,
        values: impl Iterator<Item = Value>,
    ) -> Result<Value, String> {
        let prior = env.get(binder).cloned();
        let mut seen = 0usize;
        let mut unknown = None;
        let result = (|| {
            for value in values {
                if seen >= self.budget.max_domain_size {
                    return Err("verification domain exceeds resource budget".into());
                }
                seen += 1;
                env.insert(binder.to_owned(), value.clone());
                match self.eval(body, env, depth) {
                    Ok(Value::Bool(result)) if result != universal => {
                        if universal {
                            self.counterexample = Some(format!(
                                "{binder} = {value:?} makes the quantified assertion false"
                            ));
                        }
                        return Ok(Value::Bool(!universal));
                    }
                    Ok(Value::Bool(_)) => {}
                    Ok(_) => {
                        unknown.get_or_insert_with(|| {
                            "quantified verification body is not Boolean".into()
                        });
                    }
                    Err(reason) => {
                        unknown.get_or_insert(reason);
                    }
                }
                if self.steps >= self.budget.max_steps {
                    return Err("verification resource budget exhausted".into());
                }
            }
            if seen == 0 {
                return Err("W620 BoundedVerification: quantifier has empty bounds".into());
            }
            match unknown {
                Some(reason) => Err(reason),
                None => Ok(Value::Bool(universal)),
            }
        })();
        match prior {
            Some(value) => {
                env.insert(binder.to_owned(), value);
            }
            None => {
                env.remove(binder);
            }
        }
        result
    }
}

fn bounded_sort(name: &str, bounds: &VerificationBounds) -> Option<(u32, &'static str)> {
    match name {
        "People" | "people" | "Person" | "persons" => Some((bounds.persons, "person")),
        "Events" | "events" | "Event" => Some((bounds.events, "event")),
        "Time" | "Times" | "TimePoint" | "TimePoints" | "time_points" => {
            Some((bounds.time_points, "time"))
        }
        _ => None,
    }
}

fn legacy_assertions(source: &str) -> Option<Vec<Term>> {
    if source.len() > 16_384 {
        return None;
    }
    source
        .split(';')
        .filter(|part| !part.trim().is_empty())
        .map(|part| legacy_term(part.trim(), 0))
        .collect()
}

fn legacy_term(source: &str, depth: usize) -> Option<Term> {
    if depth > 128 {
        return None;
    }
    let source = source.strip_prefix("assert ").unwrap_or(source).trim();
    for quantifier in ["for_all", "exists"] {
        if let Some(rest) = source
            .strip_prefix(quantifier)
            .and_then(|rest| rest.strip_prefix(char::is_whitespace))
        {
            let (prefix, body) = rest.split_once(':')?;
            let mut words = prefix.split_whitespace();
            let binder = words.next()?;
            if words.next()? != "in" {
                return None;
            }
            let domain = words.next()?;
            if words.next().is_some() {
                return None;
            }
            return Some(Term::Apply {
                ctor: quantifier.into(),
                args: vec![
                    Term::Ident(binder.into()),
                    Term::Ident(domain.into()),
                    legacy_term(body.trim(), depth + 1)?,
                ],
            });
        }
    }
    for (spelling, op) in [
        ("!=", BinOp::Ne),
        ("≠", BinOp::Ne),
        ("==", BinOp::Eq),
        ("=", BinOp::Eq),
    ] {
        if let Some((left, right)) = source.split_once(spelling) {
            return Some(Term::Binary {
                op,
                left: Box::new(legacy_atom(left.trim())?),
                right: Box::new(legacy_atom(right.trim())?),
            });
        }
    }
    legacy_atom(source).or_else(|| {
        Some(Term::Apply {
            ctor: "$unsupported_assertion".into(),
            args: vec![Term::String(source.into())],
        })
    })
}

fn legacy_atom(source: &str) -> Option<Term> {
    match source {
        "true" => Some(Term::Bool(true)),
        "false" => Some(Term::Bool(false)),
        _ => {
            if let Ok(value) = source.parse::<i64>() {
                return Some(Term::Int(value));
            }
            if source.starts_with('"') {
                return serde_json::from_str::<String>(source)
                    .ok()
                    .map(Term::String);
            }
            if !source.is_empty()
                && source
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            {
                Some(Term::Ident(source.into()))
            } else {
                None
            }
        }
    }
}
