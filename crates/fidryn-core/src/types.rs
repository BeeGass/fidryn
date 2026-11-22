//! Nominal types. `Prop` is a sort, never a Bool.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub type TypeEnvironment = BTreeMap<String, (Vec<String>, crate::ir::TypeDefinition)>;

/// Resolve transparent aliases while preserving nominal sum identities.
pub fn resolve_aliases(ty: &Type, environment: &TypeEnvironment) -> Result<Type, String> {
    resolve_alias_inner(ty, environment, &BTreeMap::new(), &mut BTreeSet::new(), 0)
}

fn resolve_alias_inner(
    ty: &Type,
    environment: &TypeEnvironment,
    bindings: &BTreeMap<String, Type>,
    active: &mut BTreeSet<String>,
    depth: usize,
) -> Result<Type, String> {
    use crate::ir::TypeDefinition;
    if depth >= 128 {
        return Err("type alias expansion exceeds 128 levels".into());
    }
    let (name, arguments) = match ty {
        Type::Sort(Sort::Nominal(name)) => {
            if let Some(bound) = bindings.get(name) {
                return Ok(bound.clone());
            }
            (Some(name), Vec::new())
        }
        Type::Applied { ctor, args } => (
            Some(ctor),
            args.iter()
                .map(|argument| {
                    resolve_alias_inner(argument, environment, bindings, active, depth + 1)
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        _ => (None, Vec::new()),
    };
    if let Some(name) = name
        && let Some((parameters, definition)) = environment.get(name)
    {
        if parameters.len() != arguments.len() {
            return Err(format!(
                "type `{name}` requires {} type arguments, received {}",
                parameters.len(),
                arguments.len()
            ));
        }
        if let TypeDefinition::Alias(alias) = definition {
            if !active.insert(name.clone()) {
                return Err(format!("cyclic type alias `{name}`"));
            }
            let next_bindings = parameters.iter().cloned().zip(arguments).collect();
            let result = resolve_alias_inner(alias, environment, &next_bindings, active, depth + 1);
            active.remove(name);
            return result;
        }
    }
    let mut nested =
        |inner: &Type| resolve_alias_inner(inner, environment, bindings, active, depth + 1);
    Ok(match ty {
        Type::Primitive(primitive) => Type::Primitive(match primitive {
            PrimitiveType::Option { inner } => PrimitiveType::Option {
                inner: Box::new(nested(inner)?),
            },
            PrimitiveType::Interval { inner } => PrimitiveType::Interval {
                inner: Box::new(nested(inner)?),
            },
            PrimitiveType::FiniteSet { inner } => PrimitiveType::FiniteSet {
                inner: Box::new(nested(inner)?),
            },
            PrimitiveType::NonEmptySet { inner } => PrimitiveType::NonEmptySet {
                inner: Box::new(nested(inner)?),
            },
            PrimitiveType::Map { key, value } => PrimitiveType::Map {
                key: Box::new(nested(key)?),
                value: Box::new(nested(value)?),
            },
            other => other.clone(),
        }),
        Type::Applied { ctor, .. } => Type::Applied {
            ctor: ctor.clone(),
            args: arguments,
        },
        other => other.clone(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrimitiveType {
    Bool,
    Int,
    Decimal,
    String,
    Date,
    Time,
    Duration { calendar: String },
    Money { currency: String },
    Interval { inner: Box<Type> },
    Option { inner: Box<Type> },
    FiniteSet { inner: Box<Type> },
    NonEmptySet { inner: Box<Type> },
    Map { key: Box<Type>, value: Box<Type> },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sort {
    Prop,
    NaturalPerson,
    LegalPerson,
    LegalEntity,
    Trust,
    PremaritalAgreement,
    ProposedLlc,
    Asset,
    Office,
    Authority,
    Jurisdiction,
    SourceVersion,
    LegalContext,
    LegalAction,
    LegalEffect,
    Proceeding,
    RecordItem,
    Determination,
    Interpretation,
    ClauseRef,
    Nominal(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Type {
    Primitive(PrimitiveType),
    Sort(Sort),
    Applied { ctor: String, args: Vec<Type> },
}

impl Type {
    pub fn prop() -> Self {
        Type::Sort(Sort::Prop)
    }

    pub fn bool() -> Self {
        Type::Primitive(PrimitiveType::Bool)
    }

    pub fn is_prop(&self) -> bool {
        matches!(self, Type::Sort(Sort::Prop))
    }

    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Primitive(PrimitiveType::Bool))
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Primitive(p) => write!(f, "{p:?}"),
            Type::Sort(s) => write!(f, "{s:?}"),
            Type::Applied { ctor, args } => {
                write!(f, "{ctor}<")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{a}")?;
                }
                write!(f, ">")
            }
        }
    }
}

/// Ordinary nominal subtyping used by v0.1. Offices are not person subtypes.
pub fn is_subtype(child: &Type, parent: &Type) -> bool {
    if child == parent {
        return true;
    }
    matches!(
        (child, parent),
        (
            Type::Sort(Sort::NaturalPerson) | Type::Sort(Sort::LegalEntity),
            Type::Sort(Sort::LegalPerson)
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prop_is_not_bool() {
        assert_ne!(Type::prop(), Type::bool());
        assert!(!is_subtype(&Type::prop(), &Type::bool()));
    }

    #[test]
    fn natural_person_is_legal_person() {
        assert!(is_subtype(
            &Type::Sort(Sort::NaturalPerson),
            &Type::Sort(Sort::LegalPerson)
        ));
    }
}
