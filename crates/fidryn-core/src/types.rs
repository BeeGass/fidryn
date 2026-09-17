//! Nominal types. `Prop` is a sort, never a Bool.

use serde::{Deserialize, Serialize};
use std::fmt;

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
