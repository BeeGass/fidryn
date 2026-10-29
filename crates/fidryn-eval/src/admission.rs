//! Type-directed admission of external values and source constructors.
use fidryn_core::ir::{CoreDecl, CoreModule, TypeDefinition};
use fidryn_core::{PrimitiveType as P, Sort, Term, Type, Value};
use std::collections::BTreeMap;

pub(crate) fn admit(module: &CoreModule, value: &Value, ty: &Type) -> Option<Value> {
    admit_at(module, value, ty, 0)
}
fn admit_at(module: &CoreModule, value: &Value, ty: &Type, depth: usize) -> Option<Value> {
    if depth >= 128 {
        return None;
    }
    if let Type::Applied { ctor, args } = ty
        && ctor == "Union"
    {
        return args
            .iter()
            .find_map(|ty| admit_at(module, value, ty, depth + 1));
    }
    let scalar = match ty {
        Type::Primitive(P::Bool) if matches!(value, Value::Bool(_)) => Some(value.clone()),
        Type::Primitive(P::Int) if matches!(value, Value::Int(_)) => Some(value.clone()),
        Type::Primitive(P::Decimal) if matches!(value, Value::Decimal(_) | Value::Int(_)) => {
            Some(value.clone())
        }
        Type::Primitive(P::String) if matches!(value, Value::String(_)) => Some(value.clone()),
        Type::Primitive(P::Time | P::Date) => {
            crate::judgment::as_instant(value).map(Value::Instant)
        }
        Type::Primitive(P::Money { currency }) => match value {
            Value::Int(_) | Value::Decimal(_) => Some(Value::Ctor {
                name: currency.clone(),
                fields: BTreeMap::from([("_0".into(), value.clone())]),
            }),
            Value::Ctor { name, fields }
                if name == currency
                    && fields.len() == 1
                    && matches!(fields.get("_0"), Some(Value::Int(_) | Value::Decimal(_))) =>
            {
                Some(value.clone())
            }
            _ => None,
        },
        Type::Primitive(P::Duration { calendar }) => match value {
            Value::Duration(duration)
                if format!("{:?}", duration.kind)
                    .replace('_', "")
                    .eq_ignore_ascii_case(&calendar.replace('_', "")) =>
            {
                Some(value.clone())
            }
            _ => None,
        },
        Type::Primitive(P::Option { inner }) => match value {
            Value::Unit => Some(Value::Option(None)),
            Value::Option(None) => Some(value.clone()),
            Value::Option(Some(value)) => Some(Value::Option(Some(Box::new(admit_at(
                module,
                value,
                inner,
                depth + 1,
            )?)))),
            _ => None,
        },
        Type::Primitive(P::FiniteSet { inner } | P::NonEmptySet { inner }) => match value {
            Value::Set(values)
                if !matches!(ty, Type::Primitive(P::NonEmptySet { .. })) || !values.is_empty() =>
            {
                Some(Value::Set(
                    values
                        .iter()
                        .map(|value| admit_at(module, value, inner, depth + 1))
                        .collect::<Option<_>>()?,
                ))
            }
            _ => None,
        },
        Type::Primitive(P::Map {
            key,
            value: element,
        }) => match value {
            Value::Map(fields) => {
                let values = fields
                    .iter()
                    .map(|(name, value)| {
                        admit_at(module, &Value::String(name.clone()), key, depth + 1)?;
                        Some((name.clone(), admit_at(module, value, element, depth + 1)?))
                    })
                    .collect::<Option<_>>()?;
                Some(Value::Map(values))
            }
            _ => None,
        },
        Type::Sort(Sort::ClauseRef) => {
            let clause_name = match value {
                Value::String(name) | Value::Entity(name) => name.as_str(),
                Value::ClauseRef {
                    module: owner,
                    clause,
                    arguments,
                    ..
                } if owner == &module.name && arguments.is_empty() => clause.as_str(),
                _ => return None,
            };
            let clause_name = clause_name
                .strip_prefix(&format!("{}::", module.name))
                .unwrap_or(clause_name);
            let declaration =
                module
                    .declarations
                    .iter()
                    .find_map(|declaration| match declaration {
                        CoreDecl::Clause(clause) if clause.name == clause_name => Some(clause),
                        _ => None,
                    })?;
            let digest =
                fidryn_core::NodeId::of(fidryn_core::canonical_json(declaration).ok()?.as_bytes());
            if matches!(value,Value::ClauseRef{digest:actual,..} if *actual != digest) {
                return None;
            }
            Some(Value::ClauseRef {
                module: module.name.clone(),
                clause: declaration.name.clone(),
                arguments: Vec::new(),
                digest,
            })
        }
        Type::Sort(Sort::Prop) if matches!(value, Value::Prop(_)) => Some(value.clone()),
        Type::Sort(Sort::Nominal(name)) if name == "Unit" && matches!(value, Value::Unit) => {
            Some(value.clone())
        }
        Type::Sort(Sort::Nominal(name)) if name == "DutyStatus" => match value {
            Value::Ctor { name, fields }
                if fields.is_empty()
                    && matches!(
                        name.as_str(),
                        "Unresolved"
                            | "NotAttached"
                            | "Attached"
                            | "Breached"
                            | "Performed"
                            | "Cured"
                            | "Discharged"
                    ) =>
            {
                Some(value.clone())
            }
            _ => None,
        },
        Type::Sort(Sort::Nominal(name)) if name == "DutyState" => match value {
            Value::Map(fields)
                if ["name", "status", "breached", "bearer", "instance"]
                    .iter()
                    .all(|name| fields.contains_key(*name))
                    && fields.keys().all(|name| {
                        matches!(
                            name.as_str(),
                            "name" | "status" | "breached" | "bearer" | "instance" | "claimant"
                        )
                    }) =>
            {
                crate::duty::parse_duty_state(value, "").map(|_| value.clone())
            }
            _ => None,
        },
        _ => None,
    };
    if scalar.is_some() || matches!(ty, Type::Primitive(_) | Type::Sort(Sort::Prop)) {
        return scalar;
    }
    let name = match ty {
        Type::Sort(Sort::Nominal(name)) | Type::Applied { ctor: name, .. } => Some(name.as_str()),
        _ => None,
    };
    if let Some(name) = name {
        if name.contains('|') {
            let variant = match value {
                Value::String(name) | Value::Entity(name) => name.as_str(),
                Value::Ctor { name, fields } if fields.is_empty() => name.as_str(),
                _ => return None,
            };
            return name
                .split('|')
                .map(str::trim)
                .any(|member| member == variant)
                .then(|| Value::Ctor {
                    name: variant.into(),
                    fields: BTreeMap::new(),
                });
        }
        if let Some(record) = module.declarations.iter().find_map(|decl| match decl {
            CoreDecl::RecordType(record) if record.name == name => Some(record),
            _ => None,
        }) {
            let fields = match value {
                Value::Map(fields) => fields,
                Value::Ctor { name: ctor, fields } if ctor == name => fields,
                _ => return None,
            };
            if fields.len() != record.fields.len() {
                return None;
            }
            let fields = record
                .fields
                .iter()
                .map(|(name, ty)| {
                    Some((
                        name.clone(),
                        admit_at(module, fields.get(name)?, ty, depth + 1)?,
                    ))
                })
                .collect::<Option<BTreeMap<_, _>>>()?;
            return Some(Value::Map(fields));
        }
        if let Some(declaration) = module.declarations.iter().find_map(|decl| match decl {
            CoreDecl::Type(declaration) if declaration.name == name => Some(declaration),
            _ => None,
        }) {
            let arguments = match ty {
                Type::Applied { args, .. } => args.as_slice(),
                _ => &[],
            };
            if declaration.parameters.len() != arguments.len() {
                return None;
            }
            let substitution: BTreeMap<_, _> = declaration
                .parameters
                .iter()
                .cloned()
                .zip(arguments.iter().cloned())
                .collect();
            return match &declaration.definition {
                TypeDefinition::Alias(ty) => admit_at(
                    module,
                    value,
                    &substitute_type(ty, &substitution),
                    depth + 1,
                ),
                TypeDefinition::Sum(constructors) => {
                    let owned;
                    let (ctor, fields) = match value {
                        Value::Ctor { name, fields } => (name.as_str(), fields),
                        Value::String(name) | Value::Entity(name) => {
                            let issue = crate::worklist::parse_prop_issue(name);
                            owned = (
                                issue.predicate,
                                issue
                                    .arguments
                                    .iter()
                                    .enumerate()
                                    .map(|(index, term)| {
                                        Some((format!("_{index}"), term_value(term)?))
                                    })
                                    .collect::<Option<BTreeMap<_, _>>>()?,
                            );
                            (owned.0.as_str(), &owned.1)
                        }
                        _ => return None,
                    };
                    let constructor = constructors
                        .iter()
                        .find(|constructor| constructor.name == ctor)?;
                    let expected: Vec<_> = if constructor.fields.is_empty() {
                        constructor
                            .arguments
                            .iter()
                            .enumerate()
                            .map(|(index, ty)| (format!("_{index}"), ty))
                            .collect()
                    } else {
                        constructor
                            .fields
                            .iter()
                            .map(|(name, ty)| (name.clone(), ty))
                            .collect()
                    };
                    if expected.len() != fields.len() {
                        return None;
                    }
                    let fields = expected
                        .into_iter()
                        .map(|(name, ty)| {
                            Some((
                                name.clone(),
                                admit_at(
                                    module,
                                    fields.get(&name)?,
                                    &substitute_type(ty, &substitution),
                                    depth + 1,
                                )?,
                            ))
                        })
                        .collect::<Option<_>>()?;
                    Some(Value::Ctor {
                        name: ctor.into(),
                        fields,
                    })
                }
            };
        }
        if let Some(constructor) = module.declarations.iter().find_map(|decl| match decl {
            CoreDecl::Type(declaration) => match &declaration.definition {
                TypeDefinition::Sum(constructors) => constructors
                    .iter()
                    .find(|constructor| constructor.name == name),
                _ => None,
            },
            _ => None,
        }) {
            let empty = BTreeMap::new();
            let fields = match value {
                Value::Ctor { name: ctor, fields } if ctor == name => fields,
                Value::String(variant) | Value::Entity(variant)
                    if variant == name
                        && constructor.arguments.is_empty()
                        && constructor.fields.is_empty() =>
                {
                    &empty
                }
                _ => return None,
            };
            let expected: Vec<_> = if constructor.fields.is_empty() {
                constructor
                    .arguments
                    .iter()
                    .enumerate()
                    .map(|(index, ty)| (format!("_{index}"), ty))
                    .collect()
            } else {
                constructor
                    .fields
                    .iter()
                    .map(|(name, ty)| (name.clone(), ty))
                    .collect()
            };
            if expected.len() != fields.len() {
                return None;
            }
            let fields = expected
                .into_iter()
                .map(|(name, ty)| {
                    Some((
                        name.clone(),
                        admit_at(module, fields.get(&name)?, ty, depth + 1)?,
                    ))
                })
                .collect::<Option<_>>()?;
            return Some(Value::Ctor {
                name: name.into(),
                fields,
            });
        }
    }
    let entity_name = match value {
        Value::Entity(name) | Value::String(name) => name,
        _ => return None,
    };
    let environment: fidryn_core::types::TypeEnvironment = module
        .declarations
        .iter()
        .filter_map(|declaration| match declaration {
            CoreDecl::Type(declaration) => Some((
                declaration.name.clone(),
                (
                    declaration.parameters.clone(),
                    declaration.definition.clone(),
                ),
            )),
            _ => None,
        })
        .collect();
    module.declarations.iter().any(|decl|matches!(decl,CoreDecl::Entity(entity) if entity.name == *entity_name && fidryn_core::types::resolve_aliases(&entity.ty,&environment).ok().is_some_and(|entity_type|fidryn_core::types::is_subtype(&entity_type,ty)))).then(||Value::Entity(entity_name.clone()))
}
fn substitute_type(ty: &Type, substitution: &BTreeMap<String, Type>) -> Type {
    match ty {
        Type::Sort(Sort::Nominal(name)) => substitution
            .get(name)
            .cloned()
            .unwrap_or_else(|| ty.clone()),
        Type::Applied { ctor, args } => Type::Applied {
            ctor: ctor.clone(),
            args: args
                .iter()
                .map(|ty| substitute_type(ty, substitution))
                .collect(),
        },
        Type::Primitive(P::Option { inner }) => Type::Primitive(P::Option {
            inner: Box::new(substitute_type(inner, substitution)),
        }),
        Type::Primitive(P::FiniteSet { inner }) => Type::Primitive(P::FiniteSet {
            inner: Box::new(substitute_type(inner, substitution)),
        }),
        Type::Primitive(P::NonEmptySet { inner }) => Type::Primitive(P::NonEmptySet {
            inner: Box::new(substitute_type(inner, substitution)),
        }),
        _ => ty.clone(),
    }
}
fn term_value(term: &Term) -> Option<Value> {
    match term {
        Term::Ident(name) => Some(Value::Entity(name.clone())),
        Term::String(name) => Some(Value::String(name.clone())),
        Term::Int(value) => Some(Value::Int(*value)),
        Term::Decimal(value) => Some(Value::Decimal(*value)),
        Term::Bool(value) => Some(Value::Bool(*value)),
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => Some(Value::Ctor {
            name: ctor.clone(),
            fields: args
                .iter()
                .enumerate()
                .map(|(index, term)| Some((format!("_{index}"), term_value(term)?)))
                .collect::<Option<_>>()?,
        }),
        _ => None,
    }
}

pub(crate) fn parse_type(text: &str) -> Type {
    let text = text.trim();
    match text {
        "Bool" => Type::Primitive(P::Bool),
        "Int" => Type::Primitive(P::Int),
        "Decimal" => Type::Primitive(P::Decimal),
        "String" => Type::Primitive(P::String),
        "Time" => Type::Primitive(P::Time),
        "Date" => Type::Primitive(P::Date),
        "NaturalPerson" => Type::Sort(Sort::NaturalPerson),
        "LegalPerson" => Type::Sort(Sort::LegalPerson),
        "Trust" => Type::Sort(Sort::Trust),
        _ => {
            if let Some(open) = text.find('<')
                && text.ends_with('>')
            {
                let head = &text[..open];
                let inside = &text[open + 1..text.len() - 1];
                return match head {
                    "Money" => Type::Primitive(P::Money {
                        currency: inside.into(),
                    }),
                    "FiniteSet" => Type::Primitive(P::FiniteSet {
                        inner: Box::new(parse_type(inside)),
                    }),
                    "NonEmptySet" => Type::Primitive(P::NonEmptySet {
                        inner: Box::new(parse_type(inside)),
                    }),
                    "Option" => Type::Primitive(P::Option {
                        inner: Box::new(parse_type(inside)),
                    }),
                    _ => Type::Applied {
                        ctor: head.into(),
                        args: inside.split(',').map(parse_type).collect(),
                    },
                };
            }
            Type::Sort(Sort::Nominal(text.into()))
        }
    }
}
