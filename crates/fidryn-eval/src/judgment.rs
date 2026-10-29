//! Evaluation of declared evidence policies against the frozen case view.

use crate::trace::{DerivationStep, record_step};
use crate::worklist::{DerivedWorld, canonical_ground_term};
use fidryn_core::ir::{
    CoreDecl, CoreJudgment, JudgmentAction, JudgmentAuthority, JudgmentFallback,
};
use fidryn_core::{
    CaseRecord, EngineError, FrozenCaseView, OpenRequest, PropPattern, PropTerm, RunContext, Term,
    Type, Value,
};
use std::collections::{BTreeMap, BTreeSet};

type Environment = BTreeMap<String, Value>;

pub(crate) struct Finding {
    pub proposition: PropTerm,
    pub result: Option<bool>,
    pub operative: bool,
    pub protocol: String,
    pub requests: BTreeSet<OpenRequest>,
    pub trace: String,
}

pub(crate) fn evaluate(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
    ctx: &RunContext,
    args: &Environment,
    world: &DerivedWorld,
    consumed: &mut u64,
    trace: &mut Vec<DerivationStep>,
) -> Result<Vec<Finding>, EngineError> {
    let mut findings = Vec::new();
    for declaration in &module.declarations {
        let CoreDecl::Judgment(declaration) = declaration else {
            continue;
        };
        if !declaration.meta.valid_time.contains(ctx.valid_time)
            || !declaration.meta.record_time.contains(ctx.record_time)
        {
            continue;
        }
        if !matches!(
            declaration.authority_policy,
            Some(JudgmentAuthority::Concurrence { .. })
        ) {
            continue;
        }
        for environment in bindings(module, declaration, case, args)? {
            spend(consumed, 1, world.operation_limit())?;
            let proposition = instantiate_prop(&declaration.decides, &environment);
            let mut requests = BTreeSet::new();
            let mut parents = Vec::new();
            let mut established = false;
            let mut rejected = false;
            if !declaration.unsupported_clauses.is_empty() || declaration.standard.is_some() {
                requests.insert(OpenRequest::NeedCustom {
                    effect: "JudgmentPolicy".into(),
                    payload: format!(
                        "{} requires unsupported policy clauses {:?} / standard {:?}",
                        declaration.name, declaration.unsupported_clauses, declaration.standard
                    ),
                });
            } else {
                let environments = record_bindings(
                    module,
                    declaration,
                    &proposition,
                    environment,
                    case,
                    ctx,
                    &mut requests,
                    consumed,
                    world.operation_limit(),
                )?;
                for environment in environments {
                    let JudgmentAuthority::Concurrence {
                        minimum,
                        distinct,
                        issuer_type,
                    } = declaration
                        .authority_policy
                        .as_ref()
                        .expect("concurrence filtered")
                    else {
                        unreachable!()
                    };
                    let issuers: Vec<_> = declaration
                        .record_requires
                        .iter()
                        .filter_map(|(name, _)| environment.get(name))
                        .filter_map(record_fields)
                        .filter_map(|fields| fields.get("issuer"))
                        .filter_map(value_name)
                        .collect();
                    let eligible=issuers.iter().all(|issuer| module.declarations.iter().any(|decl| matches!(decl, CoreDecl::Entity(entity) if entity.name == *issuer && fidryn_core::types::is_subtype(&entity.ty, issuer_type))));
                    let count = if *distinct {
                        issuers.iter().collect::<BTreeSet<_>>().len()
                    } else {
                        issuers.len()
                    };
                    let authority = eligible && count >= *minimum as usize && *minimum > 0;
                    let authority_trace = record_step(
                        trace,
                        "judgment_authority",
                        format!(
                            "judgment={}; issue={proposition:?}; policy={:?}; issuers={issuers:?}; admitted={authority}",
                            declaration.name, declaration.authority_policy
                        ),
                        Vec::new(),
                    );
                    parents.push(authority_trace);
                    if !authority {
                        continue;
                    }
                    let mut accepted = true;
                    for condition in &declaration.constraints {
                        let value = crate::eval_judgment_expression(
                            module,
                            condition,
                            &environment,
                            case,
                            ctx,
                            world,
                            consumed,
                            trace,
                        )?;
                        if value != Some(Value::Bool(true)) {
                            accepted = false;
                            break;
                        }
                    }
                    if !accepted {
                        continue;
                    }
                    if let Some(condition) = &declaration.establish_when {
                        established |= crate::eval_judgment_expression(
                            module,
                            condition,
                            &environment,
                            case,
                            ctx,
                            world,
                            consumed,
                            trace,
                        )? == Some(Value::Bool(true));
                    }
                    if let Some(condition) = &declaration.reject_when {
                        rejected |= crate::eval_judgment_expression(
                            module,
                            condition,
                            &environment,
                            case,
                            ctx,
                            world,
                            consumed,
                            trace,
                        )? == Some(Value::Bool(true));
                    }
                }
            }
            if !established && !rejected {
                if let Some(JudgmentFallback::Suspend { issue, protocol }) = &declaration.otherwise
                {
                    requests.insert(OpenRequest::NeedJudgment {
                        issue: instantiate_prop(
                            issue,
                            &bindings_for_prop(declaration, &proposition),
                        ),
                        protocol: term_label(&instantiate_term(
                            protocol,
                            &bindings_for_prop(declaration, &proposition),
                        )),
                    });
                }
                if requests.is_empty() {
                    requests.insert(OpenRequest::NeedJudgment {
                        issue: proposition.clone(),
                        protocol: declaration.name.clone(),
                    });
                }
            }
            if established && rejected {
                requests.insert(OpenRequest::NeedConflict {
                    graph: vec![format!(
                        "opposed admissible evidence sets for {proposition:?}"
                    )],
                    doctrines: Vec::new(),
                });
            }
            let result = match (established, rejected) {
                (true, false) => Some(true),
                (false, true) => Some(false),
                _ => None,
            };
            let operative=declaration.on_established.iter().any(|action| match action { JudgmentAction::MakeOperative { issue, .. } => matches!(issue, Term::Ident(name) | Term::Binder(name) if name == "issue") || matches!(crate::worklist::term_as_proposition(issue), Some(ref issue) if instantiate_prop(issue, &bindings_for_prop(declaration, &proposition)) == proposition) });
            let step = record_step(
                trace,
                "judgment",
                format!(
                    "declaration={}; issue={proposition:?}; result={result:?}; operative={operative}; review={:?}; requests={requests:?}",
                    declaration.name, declaration.review
                ),
                parents,
            );
            findings.push(Finding {
                proposition,
                result,
                operative,
                protocol: declaration.name.clone(),
                requests,
                trace: step,
            });
        }
    }
    Ok(findings)
}

fn bindings(
    module: &fidryn_core::CoreModule,
    judgment: &CoreJudgment,
    case: &CaseRecord,
    args: &Environment,
) -> Result<Vec<Environment>, EngineError> {
    let mut environments = vec![Environment::new()];
    for (name, ty) in &judgment.binders {
        let mut candidates = Vec::new();
        if let Some(value) = args.get(name).or_else(|| case.facts.get(name))
            && crate::runtime_value_matches(module, value, ty)
        {
            candidates.push(value.clone());
        }
        for declaration in &module.declarations {
            if let CoreDecl::Entity(entity) = declaration
                && crate::runtime_value_matches(module, &Value::Entity(entity.name.clone()), ty)
            {
                candidates.push(Value::Entity(entity.name.clone()));
            }
        }
        let mut next = Vec::new();
        for environment in environments {
            for value in &candidates {
                if next.len() >= 256 {
                    return Err(EngineError::Unsupported(
                        "judgment grounding exceeds 256 complete assignments".into(),
                    ));
                }
                let mut row = environment.clone();
                row.insert(name.clone(), value.clone());
                next.push(row);
            }
        }
        environments = next;
    }
    Ok(environments)
}

#[allow(clippy::too_many_arguments)]
fn record_bindings(
    module: &fidryn_core::CoreModule,
    judgment: &CoreJudgment,
    proposition: &PropTerm,
    environment: Environment,
    case: &CaseRecord,
    ctx: &RunContext,
    requests: &mut BTreeSet<OpenRequest>,
    consumed: &mut u64,
    limit: u64,
) -> Result<Vec<Environment>, EngineError> {
    let mut environments = vec![environment];
    for (name, ty) in &judgment.record_requires {
        let schema = match ty {
            Type::Sort(fidryn_core::Sort::Nominal(name)) | Type::Applied { ctor: name, .. } => name,
            _ => {
                return Err(EngineError::InvalidInput(format!(
                    "judgment record `{name}` lacks a declared record type"
                )));
            }
        };
        let records: Vec<_> = FrozenCaseView::from_context(case, ctx)
            .evidence()
            .filter(|record| record.schema == *schema)
            .filter_map(|record| record_admitted(module, schema, &record.value, proposition))
            .collect();
        if records.is_empty() {
            requests.insert(OpenRequest::NeedEvidence {
                issue: PropPattern::Ground(proposition.clone()),
                schema: schema.clone(),
            });
        }
        let mut next = Vec::new();
        for environment in environments {
            for record in &records {
                spend(consumed, 1, limit)?;
                if next.len() >= 256 {
                    return Err(EngineError::Unsupported(
                        "judgment record matching exceeds 256 complete assignments".into(),
                    ));
                }
                let mut row = environment.clone();
                row.insert(name.clone(), record.clone());
                next.push(row);
            }
        }
        environments = next;
    }
    Ok(environments)
}

fn record_admitted(
    module: &fidryn_core::CoreModule,
    schema: &str,
    value: &Value,
    proposition: &PropTerm,
) -> Option<Value> {
    let value = crate::admission::admit(
        module,
        value,
        &Type::Sort(fidryn_core::Sort::Nominal(schema.into())),
    )?;
    let fields = record_fields(&value)?;
    if let Some(subject) = fields.get("subject")
        && proposition
            .arguments
            .first()
            .is_none_or(|term| value_ground_term(subject) != canonical_ground_term(term))
    {
        return None;
    }
    if let Some(purpose) = fields.get("purpose")
        && proposition
            .arguments
            .get(1)
            .is_none_or(|term| value_ground_term(purpose) != canonical_ground_term(term))
    {
        return None;
    }
    Some(value)
}

pub(crate) fn as_instant(value: &Value) -> Option<fidryn_core::Instant> {
    match value {
        Value::Instant(at) => Some(*at),
        Value::String(text) => fidryn_core::Instant::parse(text).ok(),
        _ => None,
    }
}
fn record_fields(value: &Value) -> Option<&BTreeMap<String, Value>> {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => Some(fields),
        _ => None,
    }
}
fn value_name(value: &Value) -> Option<String> {
    match value {
        Value::String(name) | Value::Entity(name) => Some(name.clone()),
        Value::Ctor { name, fields } if fields.is_empty() => Some(name.clone()),
        _ => None,
    }
}
pub(crate) fn value_ground_term(value: &Value) -> Term {
    match value {
        Value::String(name) => Term::String(name.clone()),
        Value::Entity(name) => Term::Ident(name.clone()),
        Value::Int(n) => Term::Int(*n),
        Value::Decimal(n) => Term::Decimal(*n),
        Value::Bool(value) => Term::Bool(*value),
        Value::Ctor { name, fields } => Term::Apply {
            ctor: name.clone(),
            args: fields.values().map(value_ground_term).collect(),
        },
        Value::Map(fields) => Term::Record(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), value_ground_term(value)))
                .collect(),
        ),
        Value::Set(values) => Term::Set(values.iter().map(value_ground_term).collect()),
        Value::Instant(at) => Term::Instant(*at),
        Value::Duration(duration) => Term::Duration(*duration),
        Value::Option(None) => Term::Apply {
            ctor: "None".into(),
            args: Vec::new(),
        },
        Value::Option(Some(value)) => Term::Apply {
            ctor: "Some".into(),
            args: vec![value_ground_term(value)],
        },
        _ => Term::String(format!("{value:?}")),
    }
}

fn instantiate_prop(prop: &PropTerm, environment: &Environment) -> PropTerm {
    PropTerm::new(
        prop.predicate.clone(),
        prop.arguments
            .iter()
            .map(|term| instantiate_term(term, environment))
            .collect(),
    )
}
fn instantiate_term(term: &Term, environment: &Environment) -> Term {
    match term {
        Term::Ident(name) | Term::Binder(name) => environment
            .get(name)
            .map(value_ground_term)
            .unwrap_or_else(|| term.clone()),
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => Term::Apply {
            ctor: ctor.clone(),
            args: args
                .iter()
                .map(|term| instantiate_term(term, environment))
                .collect(),
        },
        other => other.clone(),
    }
}
fn bindings_for_prop(judgment: &CoreJudgment, proposition: &PropTerm) -> Environment {
    fn bind(pattern: &Term, value: &Term, names: &BTreeSet<&str>, env: &mut Environment) {
        match (pattern, value) {
            (Term::Ident(name) | Term::Binder(name), value) if names.contains(name.as_str()) => {
                env.insert(
                    name.clone(),
                    match value {
                        Term::Ident(name) => Value::Entity(name.clone()),
                        Term::Int(n) => Value::Int(*n),
                        Term::String(text) => Value::String(text.clone()),
                        _ => Value::String(term_label(value)),
                    },
                );
            }
            (
                Term::Apply { args, .. } | Term::Call { args, .. },
                Term::Apply { args: values, .. } | Term::Call { args: values, .. },
            ) => {
                for (pattern, value) in args.iter().zip(values) {
                    bind(pattern, value, names, env);
                }
            }
            _ => {}
        }
    }
    let names = judgment
        .binders
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let mut env = Environment::new();
    for (pattern, value) in judgment
        .decides
        .arguments
        .iter()
        .zip(&proposition.arguments)
    {
        bind(pattern, value, &names, &mut env);
    }
    env
}
pub(crate) fn term_label(term: &Term) -> String {
    match term {
        Term::Ident(name) | Term::Binder(name) | Term::String(name) => name.clone(),
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
            if args.is_empty() {
                ctor.clone()
            } else {
                format!(
                    "{}({})",
                    ctor,
                    args.iter().map(term_label).collect::<Vec<_>>().join(",")
                )
            }
        }
        other => format!("{other:?}"),
    }
}
fn spend(consumed: &mut u64, amount: u64, limit: u64) -> Result<(), EngineError> {
    *consumed = consumed.saturating_add(amount);
    if *consumed >= limit {
        Err(EngineError::FuelExhausted { remaining: 0 })
    } else {
        Ok(())
    }
}

pub(crate) fn observations(
    module: &fidryn_core::CoreModule,
    case: &CaseRecord,
    ctx: &RunContext,
    world: &DerivedWorld,
    consumed: &mut u64,
    trace: &mut Vec<DerivationStep>,
) -> Result<Vec<PropTerm>, EngineError> {
    let mut established = Vec::new();
    for declaration in &module.declarations {
        let CoreDecl::Observation(observation) = declaration else {
            continue;
        };
        if !observation.meta.valid_time.contains(ctx.valid_time)
            || !observation.meta.record_time.contains(ctx.record_time)
        {
            continue;
        }
        let Some(ty) = &observation.record_type else {
            continue;
        };
        let schema = match ty {
            Type::Sort(fidryn_core::Sort::Nominal(name)) | Type::Applied { ctor: name, .. } => name,
            _ => {
                return Err(EngineError::InvalidInput(format!(
                    "observation `{}` requires a declared record schema",
                    observation.name
                )));
            }
        };
        for record in FrozenCaseView::from_context(case, ctx)
            .evidence()
            .filter(|record| record.schema == *schema)
        {
            spend(consumed, 1, world.operation_limit())?;
            let Some(value) = crate::admission::admit(module, &record.value, ty) else {
                record_step(
                    trace,
                    "admission_rejection",
                    format!(
                        "observation={} rejects malformed {} record",
                        observation.name, record.schema
                    ),
                    Vec::new(),
                );
                continue;
            };
            if !observation.unsupported_clauses.is_empty() {
                return Err(EngineError::Unsupported(format!(
                    "observation `{}` has unsupported clauses {:?}",
                    observation.name, observation.unsupported_clauses
                )));
            }
            let environment = BTreeMap::from([(observation.record_binder.clone(), value)]);
            let mut admitted = true;
            for validator in &observation.validator_terms {
                if crate::eval_judgment_expression(
                    module,
                    validator,
                    &environment,
                    case,
                    ctx,
                    world,
                    consumed,
                    trace,
                )? != Some(Value::Bool(true))
                {
                    admitted = false;
                    break;
                }
            }
            for validator in &observation.validators {
                if !world.is_guard_held(validator, case, ctx) {
                    admitted = false;
                    break;
                }
            }
            if !admitted {
                record_step(
                    trace,
                    "observation_rejection",
                    format!("observation={} has an unmet validator", observation.name),
                    Vec::new(),
                );
                continue;
            }
            let mut arguments = Vec::new();
            for argument in &observation.establishes.arguments {
                let Some(value) = crate::eval_judgment_expression(
                    module,
                    argument,
                    &environment,
                    case,
                    ctx,
                    world,
                    consumed,
                    trace,
                )?
                else {
                    admitted = false;
                    break;
                };
                arguments.push(value_ground_term(&value));
            }
            if admitted {
                let proposition =
                    PropTerm::new(observation.establishes.predicate.clone(), arguments);
                record_step(
                    trace,
                    "observation",
                    format!(
                        "observation={} establishes {proposition:?} from schema={schema}",
                        observation.name
                    ),
                    Vec::new(),
                );
                established.push(proposition);
            }
        }
    }
    Ok(established)
}
