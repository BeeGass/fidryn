//! Runtime values and proposition terms. No `From<PropTerm> for bool`.

use crate::ids::NodeId;
use crate::time::{FidrynDuration, Instant};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Term {
    Bool(bool),
    Int(i64),
    Decimal(Decimal),
    String(String),
    Instant(Instant),
    Duration(FidrynDuration),
    Ident(String),
    Apply { ctor: String, args: Vec<Term> },
    Set(Vec<Term>),
    Record(BTreeMap<String, Term>),
    Wildcard,
    Binder(String),
}

impl Term {
    pub fn unit_ctor(name: impl Into<String>) -> Self {
        Term::Apply {
            ctor: name.into(),
            args: Vec::new(),
        }
    }
}

/// A ground proposition application. Never a Boolean.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PropTerm {
    pub predicate: String,
    pub arguments: Vec<Term>,
}

impl PropTerm {
    pub fn new(predicate: impl Into<String>, arguments: Vec<Term>) -> Self {
        Self {
            predicate: predicate.into(),
            arguments,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Unit,
    Bool(bool),
    Int(i64),
    Decimal(Decimal),
    String(String),
    Instant(Instant),
    Duration(FidrynDuration),
    Prop(PropTerm),
    Entity(String),
    Ctor {
        name: String,
        fields: BTreeMap<String, Value>,
    },
    Set(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Option(Option<Box<Value>>),
    ClauseRef {
        module: String,
        clause: String,
        arguments: Vec<Value>,
        digest: NodeId,
    },
}

impl Value {
    pub fn display_label(&self) -> String {
        match self {
            Value::Entity(n) | Value::String(n) => n.clone(),
            Value::Ctor { name, .. } => name.clone(),
            Value::Prop(p) => p.predicate.clone(),
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            other => format!("{other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prop_term_is_not_bool() {
        let p = PropTerm::new("Incapacitated", vec![Term::Ident("Bryan".into())]);
        assert_ne!(Value::Prop(p), Value::Bool(true));
    }
}
