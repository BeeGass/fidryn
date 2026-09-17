//! Bounded completion explorer and invariant checks.

use fidryn_core::ir::CoreModule;
use fidryn_core::{
    CaseRecord, CoverageWitness, EngineError, EvidenceItem, Instant, LegalState, OpenRequest,
    Outcome, QueryName, RunContext, TraceId, Value, VerificationBounds,
};
use fidryn_eval::{evaluate, evaluate_scenario, seed_initial_occupancy};
use fidryn_handlers::{CaseFile, ExplorationBounds, Explore, Skeptical, aggregate};
use fidryn_solve::{
    Assignment, Constraint, Domain, SearchBudget, SearchEvent, SmtAnswer, smt_check, stream,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Exhaustive search cap. Hitting it means coverage is incomplete, so a
/// determinate answer must not be finalized.
const MAX_COMPLETIONS: usize = 4096;

const INTERPRETATION_NS: &str = "i:";
const EVIDENCE_NS: &str = "e:";
const CHOICE_NS: &str = "c:";

/// Structured result of checking a declared `CoreVerify` formula.
///
/// Only [`PropertyVerdict::Proved`] is success. Unknown and invalid must
/// not be treated as a verified property.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub enum PropertyVerdict {
    Proved {
        name: String,
        bounds: VerificationBounds,
    },
    Counterexample {
        name: String,
        detail: String,
    },
    Unknown {
        name: String,
        reason: String,
        bounds: VerificationBounds,
    },
    InvalidProperty {
        diagnostics: String,
    },
}

impl PropertyVerdict {
    pub fn is_proved(&self) -> bool {
        matches!(self, Self::Proved { .. })
    }

    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::InvalidProperty { .. })
    }
}

impl fmt::Display for PropertyVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proved { name, bounds } => write!(
                f,
                "proved {name} (persons={}, events={}, time_points={})",
                bounds.persons, bounds.events, bounds.time_points
            ),
            Self::Counterexample { name, detail } => {
                write!(f, "counterexample for {name}: {detail}")
            }
            Self::Unknown {
                name,
                reason,
                bounds,
            } => write!(
                f,
                "unknown {name}: {reason} (persons={}, events={}, time_points={})",
                bounds.persons, bounds.events, bounds.time_points
            ),
            Self::InvalidProperty { diagnostics } => f.write_str(diagnostics),
        }
    }
}

/// How much of the declared completion space was examined.
///
/// `incomplete` is true when some admissible region was truncated, left
/// unresolved, or lies outside the enumerated dimensions. Agreement among
/// examined worlds is then [`Determinacy::Unknown`], not convergent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    pub examined: usize,
    pub total: usize,
    pub incomplete: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
#[allow(clippy::large_enum_variant)]
pub enum Determinacy {
    Convergent {
        value: Value,
        coverage: Coverage,
    },
    Counterexample {
        left: String,
        right: String,
        va: Value,
        vb: Value,
        coverage: Coverage,
    },
    Unknown {
        reason: String,
    },
    Suspended {
        requests: BTreeSet<OpenRequest>,
    },
    Other(Outcome<Value>),
}

impl fmt::Display for Determinacy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Convergent { value, coverage } => {
                write!(
                    f,
                    "convergent {} (examined {}/{})",
                    value.display_label(),
                    coverage.examined,
                    coverage.total
                )
            }
            Self::Counterexample {
                left,
                right,
                va,
                vb,
                coverage: _,
            } => write!(
                f,
                "counterexample {left}={} vs {right}={}",
                va.display_label(),
                vb.display_label()
            ),
            Self::Unknown { reason } => write!(f, "unknown: {reason}"),
            Self::Suspended { requests } => {
                write!(f, "suspended ({} requests)", requests.len())
            }
            Self::Other(outcome) => write!(f, "other {outcome:?}"),
        }
    }
}

/// Explore `query` under declared finite completions.
///
/// Unknown queries and other [`EngineError`] values are returned as errors.
/// They are not rewritten as `incomplete`, `Suspended`, or another legal
/// outcome.
pub fn explore_query(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<Outcome<Value>, EngineError> {
    if module.query(query.as_str()).is_none() {
        return Err(EngineError::UnknownQuery(query.as_str().to_owned()));
    }
    if let Ok(determinacy) = check_determinacy(module, query, case, ctx) {
        match determinacy {
            Determinacy::Convergent { value, .. } => return Ok(determinate_explored(value)),
            Determinacy::Suspended { requests } => {
                return Ok(Outcome::Suspended {
                    requests,
                    trace: TraceId::of(b"explore"),
                });
            }
            Determinacy::Other(outcome) => return Ok(outcome),
            Determinacy::Counterexample { .. } | Determinacy::Unknown { .. } => {}
        }
    }

    let Some((assignments, incomplete)) = declared_assignments(case) else {
        return Ok(empty_completion_set());
    };
    if assignments.is_empty() {
        return Ok(empty_completion_set());
    }

    let mut labeled_branches: Vec<(String, Outcome<Value>)> = Vec::new();
    for assignment in &assignments {
        let out = eval_assignment(module, query, case, ctx, assignment)?;
        labeled_branches.push((assignment_identity(assignment), out));
    }
    Ok(finalize_exploration(labeled_branches, incomplete))
}

/// Two-phase search: one admissible `v`, then a witness of `≠ v`.
///
/// Domains are exactly `case.admissible_completions`. Completions are
/// streamed under [`MAX_COMPLETIONS`]; the full product is never built
/// and then truncated. Engine errors and budget exhaustion are
/// [`Determinacy::Unknown`], never [`Determinacy::Convergent`].
pub fn check_determinacy(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<Determinacy, EngineError> {
    check_determinacy_with_budget(
        module,
        query,
        case,
        ctx,
        SearchBudget {
            max_assignments: MAX_COMPLETIONS,
        },
    )
}

fn check_determinacy_with_budget(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
    budget: SearchBudget,
) -> Result<Determinacy, EngineError> {
    let domains = completion_domains(case);
    if domains.iter().any(|domain| domain.values.is_empty()) {
        return Ok(Determinacy::Other(empty_completion_set()));
    }

    let mut scan = DetScan::default();
    for event in stream(&domains, budget) {
        match event {
            SearchEvent::Assignment(assignment) => {
                let label = assignment_identity(&assignment);
                match eval_assignment(module, query, case, ctx, &assignment) {
                    Ok(outcome) => {
                        if let Some(counterexample) = scan.absorb(&label, outcome, &domains) {
                            return Ok(counterexample);
                        }
                    }
                    Err(err) => {
                        scan.engine.get_or_insert_with(|| err.to_string());
                    }
                }
            }
            SearchEvent::BudgetExceeded { .. } => {
                scan.incomplete = true;
            }
            SearchEvent::Exhausted => {}
        }
    }
    Ok(scan.finish())
}

pub fn skeptical(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<Outcome<Value>, EngineError> {
    let bounds = ExplorationBounds {
        interpretations: case.admissible_completions.interpretations.clone(),
        evidence: case
            .admissible_completions
            .evidence
            .iter()
            .map(|(schema, domain)| (schema.clone(), domain.responses.clone()))
            .collect(),
    };
    let mut handler = Skeptical {
        inner: Explore {
            bounds,
            branch: case.interpretations.clone(),
        },
    };
    let state = state_from_case(case, ctx);
    let inner = match evaluate(
        module,
        query,
        &BTreeMap::new(),
        &state,
        ctx,
        &mut handler,
        case,
    ) {
        Ok(out) => out,
        Err(_) => return explore_query(module, query, case, ctx),
    };

    let Outcome::Determinate {
        value: inner_value, ..
    } = &inner
    else {
        return Ok(inner);
    };

    let explored = explore_query(module, query, case, ctx)?;
    match explored {
        Outcome::Contingent { .. }
        | Outcome::Suspended { .. }
        | Outcome::NormConflict { .. }
        | Outcome::Inconsistent { .. }
        | Outcome::OutsideCompetence { .. } => Ok(explored),
        Outcome::Determinate {
            value: ref explored_value,
            ..
        } => {
            if inner_value == explored_value {
                Ok(inner)
            } else {
                Ok(explored)
            }
        }
    }
}

/// Verify a declared [`fidryn_core::CoreVerify`] formula.
///
/// Query names are not properties. `TrusteeContinuity` is not special: it
/// must appear in `module.verifications`. A universal with empty bounds is
/// [`PropertyVerdict::Unknown`], not proved.
///
/// Quantified `true` / `false` (and finite-domain `x = v` / `x ≠ v`) over
/// nonempty `People` bounds are decided by Fidryn SMT-lite, not Z3.
/// `assert always occupied(...)` stays Unknown.
pub fn verify_property(module: &CoreModule, name: &str) -> PropertyVerdict {
    let Some(property) = module.verifications.iter().find(|item| item.name == name) else {
        let diagnostics = if module.queries.iter().any(|query| query.name == name) {
            format!("`{name}` is a query, not a declared verification property")
        } else {
            format!("unknown property {name}")
        };
        return PropertyVerdict::InvalidProperty { diagnostics };
    };

    if bounds_are_empty(&property.bounds) && formula_is_universal(&property.formula) {
        return PropertyVerdict::Unknown {
            name: property.name.clone(),
            reason: "W620 BoundedVerification: quantifier has empty bounds".into(),
            bounds: property.bounds.clone(),
        };
    }
    if bounds_are_empty(&property.bounds) && formula_is_existential(&property.formula) {
        return PropertyVerdict::Unknown {
            name: property.name.clone(),
            reason: "bounded check cannot witness an existential over empty bounds".into(),
            bounds: property.bounds.clone(),
        };
    }

    let formula = property.formula.trim().trim_end_matches(';').trim();
    if formula_is_true_literal(formula) {
        return PropertyVerdict::Proved {
            name: property.name.clone(),
            bounds: property.bounds.clone(),
        };
    }
    if formula_is_false_literal(formula) {
        return PropertyVerdict::Counterexample {
            name: property.name.clone(),
            detail: "formula evaluates to false".into(),
        };
    }
    if let Some(verdict) = verify_quantified_lite(&property.name, formula, &property.bounds) {
        return verdict;
    }

    PropertyVerdict::Unknown {
        name: property.name.clone(),
        reason: "bounded check did not obtain a covering proof of the declared formula".into(),
        bounds: property.bounds.clone(),
    }
}

fn formula_is_true_literal(formula: &str) -> bool {
    formula == "true" || formula == "assert true"
}

fn formula_is_false_literal(formula: &str) -> bool {
    formula == "false" || formula == "assert false"
}

/// SMT-lite decision for a single `for_all` / `exists` over a finite bound
/// sort (`People`, `Events`, `TimePoints`) whose body is `true`, `false`,
/// or a finite-domain equality/disequality.
///
/// `assert always occupied(...)` and other uninterpreted bodies stay
/// [`None`] so [`verify_property`] reports Unknown. Empty person bounds on
/// a universal remain W620, never a vacuous proof.
fn verify_quantified_lite(
    name: &str,
    formula: &str,
    bounds: &VerificationBounds,
) -> Option<PropertyVerdict> {
    let quantified = parse_quantified_lite(formula)?;
    let domain = finite_sort_domain(&quantified.binder, &quantified.domain, bounds)?;
    if domain.values.is_empty() {
        let reason = match quantified.kind {
            QuantKind::ForAll => "W620 BoundedVerification: quantifier has empty bounds".to_owned(),
            QuantKind::Exists => {
                "bounded check cannot witness an existential over empty bounds".to_owned()
            }
        };
        return Some(PropertyVerdict::Unknown {
            name: name.to_owned(),
            reason,
            bounds: bounds.clone(),
        });
    }

    let negate = matches!(quantified.kind, QuantKind::ForAll);
    let constraints = lite_body_constraints(&quantified.body, negate);
    match smt_check(std::slice::from_ref(&domain), &constraints) {
        SmtAnswer::Sat(assignment) => match quantified.kind {
            QuantKind::ForAll => Some(PropertyVerdict::Counterexample {
                name: name.to_owned(),
                detail: format!(
                    "formula is false under {}",
                    assignment_identity(&assignment)
                ),
            }),
            QuantKind::Exists => Some(PropertyVerdict::Proved {
                name: name.to_owned(),
                bounds: bounds.clone(),
            }),
        },
        SmtAnswer::Unsat => match quantified.kind {
            QuantKind::ForAll => Some(PropertyVerdict::Proved {
                name: name.to_owned(),
                bounds: bounds.clone(),
            }),
            QuantKind::Exists => Some(PropertyVerdict::Counterexample {
                name: name.to_owned(),
                detail: "no finite witness".into(),
            }),
        },
        SmtAnswer::Unknown { reason } => Some(PropertyVerdict::Unknown {
            name: name.to_owned(),
            reason,
            bounds: bounds.clone(),
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuantKind {
    ForAll,
    Exists,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum LiteBody {
    Bool(bool),
    Eq(String, Value),
    Ne(String, Value),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LiteQuantifier {
    kind: QuantKind,
    binder: String,
    domain: String,
    body: LiteBody,
}

fn parse_quantified_lite(formula: &str) -> Option<LiteQuantifier> {
    let rest = strip_leading_assert(formula);
    // Temporal `always` is not this fragment. TrusteeContinuity stays Unknown.
    if ident_prefix(rest, "always").is_some() {
        return None;
    }
    let (kind, rest) = if let Some(rest) = ident_prefix(rest, "for_all") {
        (QuantKind::ForAll, rest)
    } else {
        let rest = ident_prefix(rest, "exists")?;
        (QuantKind::Exists, rest)
    };
    let (binder, rest) = take_ident(rest.trim_start())?;
    let rest = ident_prefix(rest.trim_start(), "in")?;
    let (domain, rest) = take_ident(rest.trim_start())?;
    let rest = rest.trim_start().strip_prefix(':')?;
    let body_src = rest.trim().trim_end_matches(';').trim();
    if body_src.is_empty() {
        return None;
    }
    let body = parse_lite_body(body_src)?;
    Some(LiteQuantifier {
        kind,
        binder,
        domain,
        body,
    })
}

fn strip_leading_assert(formula: &str) -> &str {
    let trimmed = formula.trim();
    ident_prefix(trimmed, "assert").unwrap_or(trimmed)
}

fn ident_prefix<'a>(src: &'a str, ident: &str) -> Option<&'a str> {
    if src.starts_with(ident) && ident_boundary(src, ident.len()) {
        Some(src[ident.len()..].trim_start())
    } else {
        None
    }
}

fn ident_boundary(src: &str, end: usize) -> bool {
    match src[end..].chars().next() {
        None => true,
        Some(c) => !c.is_ascii_alphanumeric() && c != '_',
    }
}

fn take_ident(src: &str) -> Option<(String, &str)> {
    let mut chars = src.char_indices();
    let (_, first) = chars.next()?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut end = first.len_utf8();
    for (i, c) in chars {
        if c.is_ascii_alphanumeric() || c == '_' {
            end = i + c.len_utf8();
        } else {
            break;
        }
    }
    Some((src[..end].to_owned(), &src[end..]))
}

fn parse_lite_body(src: &str) -> Option<LiteBody> {
    match src {
        "true" => return Some(LiteBody::Bool(true)),
        "false" => return Some(LiteBody::Bool(false)),
        _ => {}
    }
    if let Some((left, right)) = split_once_op(src, "!=").or_else(|| split_once_op(src, "≠")) {
        return Some(LiteBody::Ne(
            parse_lite_var(left)?,
            parse_lite_value(right)?,
        ));
    }
    if let Some((left, right)) = split_once_op(src, "=") {
        return Some(LiteBody::Eq(
            parse_lite_var(left)?,
            parse_lite_value(right)?,
        ));
    }
    None
}

fn split_once_op<'a>(src: &'a str, op: &str) -> Option<(&'a str, &'a str)> {
    let i = src.find(op)?;
    Some((&src[..i], &src[i + op.len()..]))
}

fn parse_lite_var(src: &str) -> Option<String> {
    let src = src.trim();
    let (ident, rest) = take_ident(src)?;
    if rest.trim().is_empty() {
        Some(ident)
    } else {
        None
    }
}

fn is_lite_entity_token(src: &str) -> bool {
    let mut chars = src.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn parse_lite_value(src: &str) -> Option<Value> {
    let src = src.trim();
    match src {
        "true" => Some(Value::Bool(true)),
        "false" => Some(Value::Bool(false)),
        _ => {
            if let Ok(n) = src.parse::<i64>() {
                return Some(Value::Int(n));
            }
            if src.len() >= 2 {
                let bytes = src.as_bytes();
                if (bytes[0] == b'"' && *bytes.last()? == b'"')
                    || (bytes[0] == b'\'' && *bytes.last()? == b'\'')
                {
                    return Some(Value::String(src[1..src.len() - 1].to_owned()));
                }
            }
            if is_lite_entity_token(src) {
                Some(Value::Entity(src.to_owned()))
            } else {
                None
            }
        }
    }
}

fn lite_body_constraints(body: &LiteBody, negate: bool) -> Vec<Constraint> {
    match (body, negate) {
        (LiteBody::Bool(true), false) | (LiteBody::Bool(false), true) => Vec::new(),
        (LiteBody::Bool(true), true) | (LiteBody::Bool(false), false) => {
            vec![Constraint::nogood(Vec::new())]
        }
        (LiteBody::Eq(var, value), false) | (LiteBody::Ne(var, value), true) => {
            vec![Constraint::eq(var.clone(), value.clone())]
        }
        (LiteBody::Eq(var, value), true) | (LiteBody::Ne(var, value), false) => {
            vec![Constraint::ne(var.clone(), value.clone())]
        }
    }
}

fn finite_sort_domain(binder: &str, sort: &str, bounds: &VerificationBounds) -> Option<Domain> {
    let (n, prefix) = match sort {
        "People" | "people" | "Person" | "persons" => (bounds.persons, "person"),
        "Events" | "events" | "Event" => (bounds.events, "event"),
        "Time" | "Times" | "TimePoint" | "TimePoints" | "time_points" => {
            (bounds.time_points, "time")
        }
        _ => return None,
    };
    Some(Domain {
        name: binder.to_owned(),
        values: (0..n)
            .map(|i| Value::Entity(format!("{prefix}-{i}")))
            .collect(),
    })
}

fn eval_assignment(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
    assignment: &Assignment,
) -> Result<Outcome<Value>, EngineError> {
    let branched = apply_assignment(case, assignment, ctx);
    let mut handler = CaseFile {
        record: branched.clone(),
        known_at: Some(ctx.record_time),
    };
    let state = state_from_case(&branched, ctx);
    // Nonempty case.assumptions must change the explored answer, not only
    // the CLI/mill envelope. Operative `evaluate` ignores that overlay.
    if branched.assumptions.is_empty() {
        evaluate(
            module,
            query,
            &BTreeMap::new(),
            &state,
            ctx,
            &mut handler,
            &branched,
        )
    } else {
        evaluate_scenario(
            module,
            query,
            &BTreeMap::new(),
            &state,
            ctx,
            &mut handler,
            &branched,
        )
    }
}

/// Declared finite product, streamed under [`MAX_COMPLETIONS`].
/// `None` when a domain is explicitly empty.
fn declared_assignments(case: &CaseRecord) -> Option<(Vec<Assignment>, bool)> {
    let domains = completion_domains(case);
    if domains.iter().any(|domain| domain.values.is_empty()) {
        return None;
    }
    let mut assignments = Vec::new();
    let mut incomplete = false;
    for event in stream(
        &domains,
        SearchBudget {
            max_assignments: MAX_COMPLETIONS,
        },
    ) {
        match event {
            SearchEvent::Assignment(assignment) => assignments.push(assignment),
            SearchEvent::BudgetExceeded { .. } => incomplete = true,
            SearchEvent::Exhausted => {}
        }
    }
    Some((assignments, incomplete))
}

#[derive(Default)]
struct DetScan {
    witness: Option<(String, Value)>,
    engine: Option<String>,
    suspended: BTreeSet<OpenRequest>,
    saw_suspended: bool,
    other: Option<Outcome<Value>>,
    examined: usize,
    incomplete: bool,
    unresolved_outside: bool,
}

impl DetScan {
    fn record(&mut self, label: String, value: Value) -> Option<Determinacy> {
        match &self.witness {
            None => {
                self.witness = Some((label, value));
                None
            }
            Some((left, va)) if va != &value => {
                let coverage = self.coverage();
                Some(Determinacy::Counterexample {
                    left: left.clone(),
                    right: label,
                    va: va.clone(),
                    vb: value,
                    coverage,
                })
            }
            Some(_) => None,
        }
    }

    fn coverage(&self) -> Coverage {
        Coverage {
            examined: self.examined,
            total: self.examined,
            incomplete: self.incomplete || self.unresolved_outside,
        }
    }

    fn absorb(
        &mut self,
        label: &str,
        outcome: Outcome<Value>,
        domains: &[Domain],
    ) -> Option<Determinacy> {
        self.examined += 1;
        match outcome {
            Outcome::Determinate { value, .. } => self.record(label.to_owned(), value),
            Outcome::Contingent { alternatives, .. } => {
                for (nested, value) in alternatives {
                    let nested_label = if label == "{}" {
                        nested
                    } else {
                        format!("{label}/{nested}")
                    };
                    if let Some(found) = self.record(nested_label, value) {
                        return Some(found);
                    }
                }
                None
            }
            Outcome::Suspended { requests, .. } => {
                self.saw_suspended = true;
                if requests.iter().any(|req| !request_in_domains(req, domains)) {
                    self.unresolved_outside = true;
                }
                self.suspended.extend(requests);
                None
            }
            other => {
                if self.other.is_none() {
                    self.other = Some(other);
                }
                None
            }
        }
    }

    fn finish(self) -> Determinacy {
        if let Some((_, value)) = self.witness {
            if self.incomplete
                || self.engine.is_some()
                || self.saw_suspended
                || self.other.is_some()
                || self.unresolved_outside
            {
                return Determinacy::Unknown {
                    reason: self.engine.unwrap_or_else(|| {
                        if self.unresolved_outside {
                            "unresolved request outside declared completion dimensions".into()
                        } else if self.incomplete {
                            "incomplete completion search".into()
                        } else {
                            "admissible assignments are not uniformly determinate".into()
                        }
                    }),
                };
            }
            let witness = CoverageWitness::complete(self.examined, value.clone());
            debug_assert!(
                witness.is_complete(),
                "Convergent requires a complete covering witness"
            );
            let coverage = Coverage {
                examined: witness.examined,
                total: witness.total,
                incomplete: witness.incomplete,
            };
            return Determinacy::Convergent { value, coverage };
        }
        if let Some(reason) = self.engine {
            return Determinacy::Unknown { reason };
        }
        if self.incomplete {
            return Determinacy::Unknown {
                reason: "incomplete completion search".into(),
            };
        }
        if self.saw_suspended && self.other.is_none() {
            return Determinacy::Suspended {
                requests: self.suspended,
            };
        }
        if let Some(outcome) = self.other {
            return Determinacy::Other(outcome);
        }
        Determinacy::Unknown {
            reason: "no admissible determinate assignment".into(),
        }
    }
}

fn request_in_domains(request: &OpenRequest, domains: &[Domain]) -> bool {
    let key = match request {
        OpenRequest::NeedEvidence { schema, .. } => format!("{EVIDENCE_NS}{schema}"),
        OpenRequest::NeedInterpretation { family, .. } => {
            format!("{INTERPRETATION_NS}{family}")
        }
        OpenRequest::NeedChoice { protocol, .. } => format!("{CHOICE_NS}{protocol}"),
        _ => return false,
    };
    domains.iter().any(|domain| domain.name == key)
}

fn determinate_explored(value: Value) -> Outcome<Value> {
    Outcome::Determinate {
        value,
        trace: TraceId::of(b"explore"),
        convergence_certificate: None,
        ignored_open_issues: BTreeSet::new(),
    }
}

fn state_from_case(case: &CaseRecord, ctx: &RunContext) -> LegalState {
    let mut state = case.into_state();
    if !state.authority.occupancy.is_empty() {
        return state;
    }
    let Some(person) = fact_name(case, "acting_trustee") else {
        return state;
    };
    let office = fact_name(case, "office").unwrap_or_else(|| "TrusteeOf(BRT)".into());
    seed_initial_occupancy(&mut state, &person, &office, ctx.valid_time);
    state
}

fn fact_name(case: &CaseRecord, key: &str) -> Option<String> {
    match case.facts.get(key) {
        Some(Value::Entity(name) | Value::String(name)) => Some(name.clone()),
        _ => None,
    }
}

fn completion_domains(case: &CaseRecord) -> Vec<Domain> {
    let mut domains = Vec::new();
    for (family, alternatives) in &case.admissible_completions.interpretations {
        let values = recorded_or_declared(case.interpretations.get(family), alternatives);
        domains.push(Domain {
            name: format!("{INTERPRETATION_NS}{family}"),
            values,
        });
    }
    for (schema, domain) in &case.admissible_completions.evidence {
        domains.push(Domain {
            name: format!("{EVIDENCE_NS}{schema}"),
            values: domain
                .responses
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        });
    }
    for (protocol, options) in &case.admissible_completions.choices {
        let values = recorded_or_declared(case.decisions.get(protocol), options);
        domains.push(Domain {
            name: format!("{CHOICE_NS}{protocol}"),
            values,
        });
    }
    domains
}

fn recorded_or_declared(recorded: Option<&String>, declared: &[String]) -> Vec<Value> {
    if declared.is_empty() {
        return Vec::new();
    }
    if let Some(value) = recorded {
        if declared.iter().any(|item| item == value) {
            return vec![Value::String(value.clone())];
        }
        return Vec::new();
    }
    declared.iter().cloned().map(Value::String).collect()
}

fn apply_assignment(case: &CaseRecord, assignment: &Assignment, ctx: &RunContext) -> CaseRecord {
    let mut branched = case.clone();
    for (key, value) in &assignment.bindings {
        let text = binding_text(value);
        if let Some(family) = key.strip_prefix(INTERPRETATION_NS) {
            if !branched.interpretations.contains_key(family) {
                branched.interpretations.insert(family.to_owned(), text);
            }
        } else if let Some(schema) = key.strip_prefix(EVIDENCE_NS) {
            apply_evidence_response(&mut branched, case, schema, &text, ctx);
        } else if let Some(protocol) = key.strip_prefix(CHOICE_NS)
            && !branched.decisions.contains_key(protocol)
        {
            branched.decisions.insert(protocol.to_owned(), text);
        }
    }
    branched
}

fn apply_evidence_response(
    branched: &mut CaseRecord,
    original: &CaseRecord,
    schema: &str,
    response: &str,
    ctx: &RunContext,
) {
    if is_absent_response(response) {
        return;
    }
    if original.evidence.iter().any(|item| item.schema == schema) {
        return;
    }
    let effect = original
        .admissible_completions
        .evidence
        .get(schema)
        .and_then(|domain| domain.effect_on_valid_time.as_deref());
    branched.evidence.push(EvidenceItem {
        schema: schema.to_owned(),
        value: Value::String(response.to_owned()),
        observed_at: evidence_observed_at(response, effect, ctx),
    });
}

fn is_absent_response(response: &str) -> bool {
    matches!(response, "absent" | "none" | "missing" | "not_present" | "")
}

fn evidence_observed_at(response: &str, effect: Option<&str>, ctx: &RunContext) -> Instant {
    let prospective = response.contains("prospective")
        || effect.is_some_and(|text| text.contains("after") || text.contains("unchanged"));
    if prospective {
        Instant::parse("2099-01-01T00:00:00Z").unwrap_or(ctx.record_time)
    } else {
        ctx.record_time
    }
}

fn assignment_identity(assignment: &Assignment) -> String {
    if assignment.bindings.is_empty() {
        return "{}".into();
    }
    assignment
        .bindings
        .iter()
        .map(|(family, value)| format!("{family}={}", binding_text(value)))
        .collect::<Vec<_>>()
        .join(",")
}

fn binding_text(value: &Value) -> String {
    match value {
        Value::String(text) | Value::Entity(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Int(n) => n.to_string(),
        other => other.display_label(),
    }
}

fn finalize_exploration(
    branches: Vec<(String, Outcome<Value>)>,
    incomplete: bool,
) -> Outcome<Value> {
    if branches.is_empty() {
        if incomplete {
            return incomplete_search();
        }
        return empty_completion_set();
    }

    let outcomes: Vec<Outcome<Value>> = branches.iter().map(|(_, out)| out.clone()).collect();
    if outcomes.iter().any(|out| {
        matches!(
            out,
            Outcome::Suspended { .. }
                | Outcome::NormConflict { .. }
                | Outcome::OutsideCompetence { .. }
                | Outcome::Inconsistent { .. }
        )
    }) {
        return aggregate(outcomes);
    }

    let mut labeled = BTreeMap::new();
    for (label, out) in &branches {
        match out {
            Outcome::Determinate { value, .. } => {
                labeled.insert(label.clone(), value.clone());
            }
            Outcome::Contingent { alternatives, .. } => {
                for (nested, value) in alternatives {
                    labeled.insert(format!("{label}/{nested}"), value.clone());
                }
            }
            _ => {}
        }
    }
    if labeled.is_empty() {
        return aggregate(outcomes);
    }

    let mut values = labeled.values();
    let Some(first) = values.next() else {
        return aggregate(outcomes);
    };
    let all_equal = values.all(|value| value == first);
    if all_equal {
        if incomplete {
            return incomplete_search();
        }
        return outcomes
            .into_iter()
            .find(|out| matches!(out, Outcome::Determinate { .. }))
            .unwrap_or_else(|| aggregate(branches.into_iter().map(|(_, out)| out).collect()));
    }

    Outcome::Contingent {
        alternatives: labeled,
        pivots: BTreeSet::new(),
        trace: TraceId::of(b"explore"),
    }
}

fn empty_completion_set() -> Outcome<Value> {
    Outcome::Inconsistent {
        core: vec!["empty completion set".into()],
        trace: TraceId::of(b"empty"),
    }
}

fn incomplete_search() -> Outcome<Value> {
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedCustom {
        effect: "Explore".into(),
        payload: "incomplete completion search".into(),
    });
    Outcome::Suspended {
        requests,
        trace: TraceId::of(b"explore-incomplete"),
    }
}

fn bounds_are_empty(bounds: &VerificationBounds) -> bool {
    bounds.persons == 0 && bounds.events == 0 && bounds.time_points == 0
}

fn formula_is_universal(formula: &str) -> bool {
    contains_ident(formula, "for_all") || contains_ident(formula, "always")
}

fn formula_is_existential(formula: &str) -> bool {
    contains_ident(formula, "exists")
}

fn contains_ident(src: &str, ident: &str) -> bool {
    src.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|token| token == ident)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::ir::{
        CoreDecl, CoreInterpretationFamily, CoreNomination, CoreQuery, CoreVerify, QueryPlan,
    };
    use fidryn_core::types::Type;
    use fidryn_core::value::PropTerm;
    use fidryn_core::{
        Assumption, Instant, Interval, JurisdictionId, ModuleId, NodeId, NodeMeta, OriginId,
        SourceManifestId, SourceSnapshotId, Term,
    };

    fn node_meta(name: &str) -> NodeMeta {
        NodeMeta {
            span: None,
            source: None,
            jurisdiction: JurisdictionId::of(b"test"),
            valid_time: Interval::always(),
            record_time: Interval::always(),
            origin: OriginId::Direct(NodeId::of(name.as_bytes())),
        }
    }

    fn module_with_query(name: &str, plan: QueryPlan) -> CoreModule {
        CoreModule {
            id: ModuleId::of(b"test"),
            name: "Test".into(),
            version: "0.1.0".into(),
            snapshot: SourceSnapshotId::of(b"s"),
            manifest: SourceManifestId::of(b"m"),
            jurisdiction: JurisdictionId::of(b"j"),
            outside_scope: Vec::new(),
            declarations: Vec::new(),
            nominations: vec![
                CoreNomination {
                    candidate: "Alice".into(),
                    office: "TrusteeOf(BRT)".into(),
                    rank: 1,
                },
                CoreNomination {
                    candidate: "Bob".into(),
                    office: "TrusteeOf(BRT)".into(),
                    rank: 2,
                },
            ],
            queries: vec![CoreQuery {
                id: NodeId::of(name.as_bytes()),
                name: name.into(),
                binders: Vec::new(),
                result_type: Type::bool(),
                effects: BTreeSet::new(),
                automatic: false,
                plan,
                meta: node_meta(name),
            }],
            verifications: Vec::new(),
            assertions: Vec::new(),
        }
    }

    fn eligible_def(person: &str, office: &str, established: bool) -> (PropTerm, bool) {
        (
            PropTerm::new(
                "Eligible",
                vec![Term::Ident(person.into()), Term::Ident(office.into())],
            ),
            established,
        )
    }

    fn trust_module() -> CoreModule {
        let office = "TrusteeOf(BRT)";
        let mut module = module_with_query(
            "acting_trustee",
            QueryPlan::UniqueOccupant {
                office: Term::Ident(office.into()),
            },
        );
        module
            .declarations
            .push(CoreDecl::InterpretationFamily(CoreInterpretationFamily {
                id: NodeId::of(b"SuccessorEligibility"),
                name: "SuccessorEligibility".into(),
                source: Term::Ident("SuccessorEligibility".into()),
                alternatives: vec![
                    (
                        "I1".into(),
                        vec![
                            eligible_def("Alice", office, true),
                            eligible_def("Bob", office, true),
                        ],
                    ),
                    (
                        "I2".into(),
                        vec![
                            eligible_def("Alice", office, false),
                            eligible_def("Bob", office, true),
                        ],
                    ),
                ],
                meta: node_meta("SuccessorEligibility"),
            }));
        module
    }

    fn bool_module() -> CoreModule {
        module_with_query("q", QueryPlan::Evaluate(Term::Bool(true)))
    }

    fn at() -> Instant {
        Instant::parse("2033-01-01T00:00:00Z").unwrap()
    }

    fn ctx() -> RunContext {
        let t = at();
        RunContext::new(t, t)
    }

    fn push_property(
        module: &mut CoreModule,
        name: &str,
        formula: &str,
        bounds: VerificationBounds,
    ) {
        module.verifications.push(CoreVerify {
            id: NodeId::of(name.as_bytes()),
            name: name.to_owned(),
            bounds,
            formula: formula.to_owned(),
            meta: NodeMeta {
                span: None,
                source: None,
                jurisdiction: JurisdictionId::of(b"test"),
                valid_time: Interval::always(),
                record_time: Interval::always(),
                origin: OriginId::Direct(NodeId::of(name.as_bytes())),
            },
        });
    }

    fn answer_labels(alternatives: &BTreeMap<String, Value>) -> BTreeSet<String> {
        alternatives.values().map(Value::display_label).collect()
    }

    fn two_cert_case() -> CaseRecord {
        let mut case = CaseRecord::default();
        let t = at();
        case.facts
            .insert("acting_trustee".into(), Value::Entity("Bryan".into()));
        case.facts
            .insert("alice_accepted".into(), Value::Bool(true));
        case.facts.insert("bob_accepted".into(), Value::Bool(true));
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: Value::String("c1".into()),
            observed_at: t,
        });
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: Value::String("c2".into()),
            observed_at: t,
        });
        case.admissible_completions.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["I1".into(), "I2".into()],
        );
        case
    }

    fn court_i2_occupancy_case() -> CaseRecord {
        let observed = Instant::parse("2026-08-23T12:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts
            .insert("alice_accepted".into(), Value::Bool(true));
        case.facts.insert("bob_accepted".into(), Value::Bool(true));
        case.evidence.push(EvidenceItem {
            schema: "OccupancyRecord".into(),
            value: Value::String("Bryan".into()),
            observed_at: observed,
        });
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: Value::String("certificate-1".into()),
            observed_at: observed,
        });
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: Value::String("certificate-2".into()),
            observed_at: observed,
        });
        case.interpretations
            .insert("SuccessorEligibility".into(), "I2".into());
        case.admissible_completions.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["I1".into(), "I2".into()],
        );
        case
    }

    #[test]
    fn two_interpretations_are_contingent() {
        let module = trust_module();
        let case = two_cert_case();
        let out = explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx())
            .expect("explore");
        match out {
            Outcome::Contingent { alternatives, .. } => {
                assert_eq!(alternatives.len(), 2);
                let labels = answer_labels(&alternatives);
                assert!(labels.contains("Alice"), "{alternatives:?}");
                assert!(labels.contains("Bob"), "{alternatives:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn two_interpretations_are_counterexample_not_convergent() {
        let module = trust_module();
        let case = two_cert_case();
        let det = check_determinacy(&module, &QueryName::from("acting_trustee"), &case, &ctx())
            .expect("determinacy");
        match det {
            Determinacy::Counterexample { va, vb, .. } => {
                let labels = [va.display_label(), vb.display_label()];
                assert!(labels.contains(&"Alice".to_owned()), "{labels:?}");
                assert!(labels.contains(&"Bob".to_owned()), "{labels:?}");
                assert_ne!(va, vb);
            }
            Determinacy::Convergent { value, .. } => {
                panic!("two SuccessorEligibility interpretations must not be Convergent: {value:?}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn counterexample_returns_before_remaining_space() {
        let module = trust_module();
        let mut case = two_cert_case();
        case.admissible_completions
            .interpretations
            .insert("Padding".into(), (0..32).map(|i| format!("p{i}")).collect());
        let det = check_determinacy(&module, &QueryName::from("acting_trustee"), &case, &ctx())
            .expect("determinacy");
        match det {
            Determinacy::Counterexample {
                va, vb, coverage, ..
            } => {
                let labels = [va.display_label(), vb.display_label()];
                assert!(labels.contains(&"Alice".to_owned()), "{labels:?}");
                assert!(labels.contains(&"Bob".to_owned()), "{labels:?}");
                assert_ne!(va, vb);
                assert!(
                    coverage.examined <= 2,
                    "disagreeing answers must not wait for the padded product: {coverage:?}"
                );
            }
            Determinacy::Convergent { value, .. } => {
                panic!("padded SuccessorEligibility product must not be Convergent: {value:?}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn recorded_i2_with_occupancy_is_convergent_bob() {
        let module = trust_module();
        let case = court_i2_occupancy_case();
        let det = check_determinacy(&module, &QueryName::from("acting_trustee"), &case, &ctx())
            .expect("determinacy");
        match det {
            Determinacy::Convergent { value, coverage } => {
                assert_eq!(value.display_label(), "Bob");
                assert!(!coverage.incomplete);
                assert_eq!(coverage.total, coverage.examined);
                assert!(coverage.examined > 0);
            }
            other => panic!("recorded I2 with OccupancyRecord must be Convergent Bob: {other:?}"),
        }
    }

    #[test]
    fn budget_exhaustion_is_unknown_not_convergent() {
        let module = bool_module();
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("A".into(), vec!["a1".into(), "a2".into()]);
        case.admissible_completions
            .interpretations
            .insert("B".into(), vec!["b1".into(), "b2".into()]);
        let det = super::check_determinacy_with_budget(
            &module,
            &QueryName::from("q"),
            &case,
            &ctx(),
            SearchBudget { max_assignments: 1 },
        )
        .expect("determinacy");
        match det {
            Determinacy::Unknown { reason } => {
                assert!(reason.contains("incomplete"), "{reason}");
            }
            Determinacy::Convergent { value, coverage } => {
                panic!("budget exhaustion must not be Convergent: {value:?} {coverage:?}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn empty_declared_domain_is_not_convergent() {
        let module = bool_module();
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("SuccessorEligibility".into(), Vec::new());
        let det =
            check_determinacy(&module, &QueryName::from("q"), &case, &ctx()).expect("determinacy");
        assert!(
            !matches!(det, Determinacy::Convergent { .. }),
            "empty admissible product must not be Convergent: {det:?}"
        );
        match det {
            Determinacy::Other(Outcome::Inconsistent { .. }) => {}
            other => panic!("empty domain must be Other(Inconsistent), got {other:?}"),
        }
    }

    #[test]
    fn recorded_selection_cannot_reopen_empty_declared_domain() {
        let module = bool_module();
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), Vec::new());
        case.interpretations.insert("I".into(), "outside".into());
        let det =
            check_determinacy(&module, &QueryName::from("q"), &case, &ctx()).expect("determinacy");
        assert!(
            !matches!(det, Determinacy::Convergent { .. }),
            "an invalid recorded value is not an admissible world: {det:?}"
        );
        match det {
            Determinacy::Other(Outcome::Inconsistent { .. }) => {}
            other => panic!("recorded empty domain must stay Other(Inconsistent): {other:?}"),
        }
    }

    #[test]
    fn engine_error_is_unknown_not_convergent() {
        let module = bool_module();
        let case = CaseRecord::default();
        let det = check_determinacy(&module, &QueryName::from("missing"), &case, &ctx())
            .expect("determinacy");
        match det {
            Determinacy::Unknown { reason } => {
                assert!(
                    reason.contains("unknown query") || reason.contains("missing"),
                    "{reason}"
                );
            }
            Determinacy::Convergent { .. } => {
                panic!("engine error must not be treated as Convergent")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn explore_unknown_query_is_engine_error_not_incomplete() {
        let module = bool_module();
        let case = CaseRecord::default();
        let err = explore_query(&module, &QueryName::from("missing"), &case, &ctx())
            .expect_err("unknown query is not a legal outcome");
        match err {
            EngineError::UnknownQuery(name) => assert_eq!(name, "missing"),
            other => panic!("expected UnknownQuery, got {other:?}"),
        }
        let mut empty_domain = CaseRecord::default();
        empty_domain
            .admissible_completions
            .interpretations
            .insert("ClosedFamily".into(), Vec::new());
        let err = explore_query(&module, &QueryName::from("missing"), &empty_domain, &ctx())
            .expect_err("unknown query is not an empty completion set");
        assert!(matches!(err, EngineError::UnknownQuery(_)));
    }

    #[test]
    fn empty_families_are_one_empty_assignment() {
        let module = bool_module();
        let case = CaseRecord::default();
        let out = explore_query(&module, &QueryName::from("q"), &case, &ctx()).expect("explore");
        match out {
            Outcome::Determinate { value, .. } => assert_eq!(value, Value::Bool(true)),
            other => panic!("empty families should evaluate once, got {other:?}"),
        }
    }

    #[test]
    fn empty_declared_domain_is_inconsistent() {
        let module = bool_module();
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("SuccessorEligibility".into(), Vec::new());
        let out = explore_query(&module, &QueryName::from("q"), &case, &ctx()).expect("explore");
        match out {
            Outcome::Inconsistent { core, .. } => {
                assert!(
                    core.iter()
                        .any(|item| item.contains("empty completion set")),
                    "{core:?}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn recorded_interpretation_is_not_overwritten() {
        let module = trust_module();
        let mut case = two_cert_case();
        case.interpretations
            .insert("SuccessorEligibility".into(), "I2".into());
        let out = explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx())
            .expect("explore");
        match out {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn distinct_assignments_keep_distinct_labels() {
        let module = trust_module();
        let mut case = two_cert_case();
        case.admissible_completions
            .interpretations
            .insert("ExtraFamily".into(), vec!["P".into(), "Q".into()]);
        let out = explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx())
            .expect("explore");
        match out {
            Outcome::Contingent { alternatives, .. } => {
                assert_eq!(alternatives.len(), 4, "{alternatives:?}");
                let labels = answer_labels(&alternatives);
                assert!(labels.contains("Alice"), "{alternatives:?}");
                assert!(labels.contains("Bob"), "{alternatives:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_rejects_a_query_name() {
        let module = bool_module();
        match verify_property(&module, "q") {
            PropertyVerdict::InvalidProperty { diagnostics } => {
                assert!(diagnostics.contains('q'), "{diagnostics}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_rejects_undeclared_trustee_continuity() {
        let module = bool_module();
        match verify_property(&module, "TrusteeContinuity") {
            PropertyVerdict::InvalidProperty { diagnostics } => {
                assert!(diagnostics.contains("TrusteeContinuity"), "{diagnostics}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_reports_unknown_for_empty_universal_bounds() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "ClosedWorld",
            "for_all x in People: Eligible(x)",
            VerificationBounds {
                persons: 0,
                events: 0,
                time_points: 0,
            },
        );
        match verify_property(&module, "ClosedWorld") {
            PropertyVerdict::Unknown {
                name,
                reason,
                bounds,
            } => {
                assert_eq!(name, "ClosedWorld");
                assert!(reason.contains("empty bounds"), "{reason}");
                assert_eq!(bounds.persons, 0);
                assert_eq!(bounds.events, 0);
                assert_eq!(bounds.time_points, 0);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_does_not_auto_prove_declared_trustee_continuity() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "TrusteeContinuity",
            "assert always occupied(TrusteeOf(BRT))",
            VerificationBounds {
                persons: 8,
                events: 40,
                time_points: 80,
            },
        );
        match verify_property(&module, "TrusteeContinuity") {
            PropertyVerdict::Proved { .. } => {
                panic!("TrusteeContinuity must not be treated as automatically proved")
            }
            PropertyVerdict::Unknown { name, bounds, .. } => {
                assert_eq!(name, "TrusteeContinuity");
                assert_eq!(bounds.persons, 8);
            }
            PropertyVerdict::InvalidProperty { diagnostics } => {
                panic!("declared TrusteeContinuity must be recognized: {diagnostics}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_proves_true_literal() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "AlwaysOk",
            "assert true",
            VerificationBounds {
                persons: 1,
                events: 1,
                time_points: 1,
            },
        );
        match verify_property(&module, "AlwaysOk") {
            PropertyVerdict::Proved { name, bounds } => {
                assert_eq!(name, "AlwaysOk");
                assert_eq!(bounds.persons, 1);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_counterexample_for_false_literal() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "NeverOk",
            "assert false",
            VerificationBounds {
                persons: 1,
                events: 1,
                time_points: 1,
            },
        );
        match verify_property(&module, "NeverOk") {
            PropertyVerdict::Counterexample { name, detail } => {
                assert_eq!(name, "NeverOk");
                assert!(detail.contains("false"), "{detail}");
            }
            other => panic!("{other:?}"),
        }
    }

    fn people_bounds(persons: u32) -> VerificationBounds {
        VerificationBounds {
            persons,
            events: 1,
            time_points: 1,
        }
    }

    #[test]
    fn verify_property_proves_forall_true_over_people() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "AllTrue",
            "for_all x in People: true",
            people_bounds(3),
        );
        match verify_property(&module, "AllTrue") {
            PropertyVerdict::Proved { name, bounds } => {
                assert_eq!(name, "AllTrue");
                assert_eq!(bounds.persons, 3);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_counterexample_forall_false_over_people() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "AllFalse",
            "for_all x in People: false",
            people_bounds(3),
        );
        match verify_property(&module, "AllFalse") {
            PropertyVerdict::Counterexample { name, detail } => {
                assert_eq!(name, "AllFalse");
                assert!(
                    detail.contains("person-") || detail.contains('x'),
                    "{detail}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_proves_exists_true_over_people() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "Someone",
            "exists x in People: true",
            people_bounds(2),
        );
        match verify_property(&module, "Someone") {
            PropertyVerdict::Proved { name, bounds } => {
                assert_eq!(name, "Someone");
                assert_eq!(bounds.persons, 2);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_exists_false_over_people_is_counterexample() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "Nobody",
            "exists x in People: false",
            people_bounds(2),
        );
        match verify_property(&module, "Nobody") {
            PropertyVerdict::Counterexample { name, .. } => {
                assert_eq!(name, "Nobody");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_forall_true_empty_people_is_w620() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "Vacuous",
            "for_all x in People: true",
            VerificationBounds {
                persons: 0,
                events: 0,
                time_points: 0,
            },
        );
        match verify_property(&module, "Vacuous") {
            PropertyVerdict::Unknown { reason, .. } => {
                assert!(reason.contains("W620"), "{reason}");
                assert!(reason.contains("empty bounds"), "{reason}");
            }
            other => panic!("empty universal true must not be proved: {other:?}"),
        }
    }

    #[test]
    fn verify_property_forall_true_empty_people_sort_is_w620() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "VacuousPeople",
            "for_all x in People: true",
            VerificationBounds {
                persons: 0,
                events: 4,
                time_points: 4,
            },
        );
        match verify_property(&module, "VacuousPeople") {
            PropertyVerdict::Unknown { reason, .. } => {
                assert!(reason.contains("W620"), "{reason}");
                assert!(reason.contains("empty bounds"), "{reason}");
            }
            other => panic!("empty People sort must not be a vacuous proof: {other:?}"),
        }
    }

    #[test]
    fn verify_property_forall_eligible_stays_unknown() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "ClosedWorld",
            "for_all x in People: Eligible(x)",
            people_bounds(4),
        );
        match verify_property(&module, "ClosedWorld") {
            PropertyVerdict::Unknown { name, .. } => {
                assert_eq!(name, "ClosedWorld");
            }
            PropertyVerdict::Proved { .. } => {
                panic!("Eligible is outside SMT-lite; must not auto-prove")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verify_property_exists_equality_witness_over_people() {
        let mut module = bool_module();
        push_property(
            &mut module,
            "NamedPerson",
            "exists x in People: x = person-0",
            people_bounds(3),
        );
        match verify_property(&module, "NamedPerson") {
            PropertyVerdict::Proved { name, bounds } => {
                assert_eq!(name, "NamedPerson");
                assert_eq!(bounds.persons, 3);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn skeptical_preserves_suspended() {
        let module = trust_module();
        let mut case = CaseRecord::default();
        case.admissible_completions.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["I1".into(), "I2".into()],
        );
        let out = skeptical(&module, &QueryName::from("acting_trustee"), &case, &ctx())
            .expect("skeptical");
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    #[test]
    fn skeptical_does_not_keep_determinate_over_empty_completion_set() {
        let module = bool_module();
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("ClosedFamily".into(), Vec::new());
        let out = skeptical(&module, &QueryName::from("q"), &case, &ctx()).expect("skeptical");
        assert!(
            matches!(out, Outcome::Inconsistent { .. }),
            "determinate inner answer must not hide an empty completion set: {out:?}"
        );
    }

    #[test]
    fn explore_with_assumptions_overlays_boolean_fact() {
        let module = module_with_query("q", QueryPlan::Evaluate(Term::Ident("flag".into())));
        let mut case = CaseRecord::default();
        let operative = explore_query(&module, &QueryName::from("q"), &case, &ctx());
        match operative {
            Err(EngineError::Unsupported(message)) => {
                assert!(
                    message.contains("flag"),
                    "unbound flag must surface as engine error: {message}"
                );
            }
            Ok(Outcome::Determinate { value, .. }) => {
                panic!("operative explore must not determine flag: {value:?}")
            }
            other => panic!("unbound flag must not be a fake legal outcome: {other:?}"),
        }

        let mut facts = BTreeMap::new();
        facts.insert("flag".into(), Value::Bool(true));
        case.assumptions.push(Assumption {
            id: "hyp-flag".into(),
            payload: Value::Map(facts),
        });
        let scenario =
            explore_query(&module, &QueryName::from("q"), &case, &ctx()).expect("explore");
        match scenario {
            Outcome::Determinate { value, .. } => {
                assert_eq!(value, Value::Bool(true), "{value:?}");
            }
            other => panic!("scenario overlay must determine flag: {other:?}"),
        }
    }
}
