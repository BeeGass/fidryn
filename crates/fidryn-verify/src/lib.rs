//! Bounded completion explorer and invariant checks.

use fidryn_core::ir::CoreModule;
use fidryn_core::{
    CaseRecord, EngineError, EvidenceItem, Instant, LegalState, OpenRequest, Outcome, QueryName,
    RunContext, TraceId, Value, VerificationBounds,
};
use fidryn_eval::{evaluate, seed_initial_occupancy};
use fidryn_handlers::{CaseFile, ExplorationBounds, Explore, Skeptical, aggregate};
use fidryn_solve::{Assignment, Domain, enumerate};
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

/// Two-phase determinacy over declared `admissible_completions` only.
///
/// Evaluate one admissible assignment to `v`, then search for another
/// assignment whose determinate value is `≠ v`. Agreement is
/// [`Determinacy::Convergent`]; two values are
/// [`Determinacy::Counterexample`]. Engine errors and incomplete coverage
/// are [`Determinacy::Unknown`], never convergent. An explicit empty
/// declared domain is not convergent.
#[derive(Clone, Debug, PartialEq, Eq)]
#[must_use]
pub enum Determinacy {
    Convergent {
        value: Value,
    },
    Counterexample {
        left: String,
        right: String,
        va: Value,
        vb: Value,
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
            Self::Convergent { value } => {
                write!(f, "convergent {}", value.display_label())
            }
            Self::Counterexample {
                left,
                right,
                va,
                vb,
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

/// Map `evaluate`'s `Outcome` or `Result<Outcome, E>` into a world outcome.
/// Engine errors are not legal inconsistency.
trait IntoWorldOutcome {
    fn into_world_outcome(self) -> Result<Outcome<Value>, String>;
}

#[allow(dead_code)]
impl IntoWorldOutcome for Outcome<Value> {
    fn into_world_outcome(self) -> Result<Outcome<Value>, String> {
        Ok(self)
    }
}

#[allow(dead_code)]
impl<E: fmt::Display> IntoWorldOutcome for Result<Outcome<Value>, E> {
    fn into_world_outcome(self) -> Result<Outcome<Value>, String> {
        self.map_err(|err| err.to_string())
    }
}

pub fn explore_query(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Outcome<Value> {
    if let Ok(determinacy) = check_determinacy(module, query, case, ctx) {
        match determinacy {
            Determinacy::Convergent { value } => return determinate_explored(value),
            Determinacy::Suspended { requests } => {
                return Outcome::Suspended {
                    requests,
                    trace: TraceId::of(b"explore"),
                };
            }
            Determinacy::Other(outcome) => return outcome,
            Determinacy::Counterexample { .. } | Determinacy::Unknown { .. } => {}
        }
    }

    let Some((assignments, mut incomplete)) = declared_assignments(case) else {
        return empty_completion_set();
    };
    if assignments.is_empty() {
        return empty_completion_set();
    }

    let mut labeled_branches: Vec<(String, Outcome<Value>)> = Vec::new();
    for assignment in &assignments {
        match eval_assignment(module, query, case, ctx, assignment) {
            Ok(out) => {
                labeled_branches.push((assignment_identity(assignment), out));
            }
            Err(_) => {
                incomplete = true;
            }
        }
    }
    finalize_exploration(labeled_branches, incomplete)
}

/// Two-phase search: one admissible `v`, then a witness of `≠ v`.
///
/// Domains are exactly `case.admissible_completions`. Engine errors are
/// [`Determinacy::Unknown`], never [`Determinacy::Convergent`].
pub fn check_determinacy(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<Determinacy, EngineError> {
    let Some((assignments, incomplete)) = declared_assignments(case) else {
        return Ok(Determinacy::Other(empty_completion_set()));
    };
    if assignments.is_empty() {
        return Ok(Determinacy::Other(empty_completion_set()));
    }

    let mut scan = DetScan::default();
    for assignment in &assignments {
        let label = assignment_identity(assignment);
        match eval_assignment(module, query, case, ctx, assignment) {
            Ok(outcome) => {
                if let Some(counterexample) = scan.absorb(&label, outcome) {
                    return Ok(counterexample);
                }
            }
            Err(err) => {
                scan.engine.get_or_insert_with(|| err.to_string());
            }
        }
    }
    Ok(scan.finish(incomplete))
}

pub fn skeptical(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Outcome<Value> {
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
    )
    .into_world_outcome()
    {
        Ok(out) => out,
        Err(_) => {
            return explore_query(module, query, case, ctx);
        }
    };

    let Outcome::Determinate {
        value: inner_value, ..
    } = &inner
    else {
        return inner;
    };

    let explored = explore_query(module, query, case, ctx);
    match explored {
        Outcome::Contingent { .. }
        | Outcome::Suspended { .. }
        | Outcome::NormConflict { .. }
        | Outcome::Inconsistent { .. }
        | Outcome::OutsideCompetence { .. } => explored,
        Outcome::Determinate {
            value: ref explored_value,
            ..
        } => {
            if inner_value == explored_value {
                inner
            } else {
                explored
            }
        }
    }
}

/// Verify a declared [`fidryn_core::CoreVerify`] formula.
///
/// Query names are not properties. `TrusteeContinuity` is not special: it
/// must appear in `module.verifications`. A universal with empty bounds is
/// [`PropertyVerdict::Unknown`], not proved.
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

    PropertyVerdict::Unknown {
        name: property.name.clone(),
        reason: "bounded check did not obtain a covering proof of the declared formula".into(),
        bounds: property.bounds.clone(),
    }
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
    evaluate(
        module,
        query,
        &BTreeMap::new(),
        &state,
        ctx,
        &mut handler,
        &branched,
    )
}

/// Declared finite product. `None` when a domain is explicitly empty.
fn declared_assignments(case: &CaseRecord) -> Option<(Vec<Assignment>, bool)> {
    let domains = completion_domains(case);
    if domains.iter().any(|domain| domain.values.is_empty()) {
        return None;
    }
    let mut assignments = enumerate(&domains);
    let mut incomplete = false;
    if assignments.len() > MAX_COMPLETIONS {
        assignments.truncate(MAX_COMPLETIONS);
        incomplete = true;
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
}

impl DetScan {
    fn record(&mut self, label: String, value: Value) -> Option<Determinacy> {
        match &self.witness {
            None => {
                self.witness = Some((label, value));
                None
            }
            Some((left, va)) if va != &value => Some(Determinacy::Counterexample {
                left: left.clone(),
                right: label,
                va: va.clone(),
                vb: value,
            }),
            Some(_) => None,
        }
    }

    fn absorb(&mut self, label: &str, outcome: Outcome<Value>) -> Option<Determinacy> {
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

    fn finish(self, incomplete: bool) -> Determinacy {
        if let Some((_, value)) = self.witness {
            if incomplete || self.engine.is_some() || self.saw_suspended || self.other.is_some() {
                return Determinacy::Unknown {
                    reason: self.engine.unwrap_or_else(|| {
                        if incomplete {
                            "incomplete completion search".into()
                        } else {
                            "admissible assignments are not uniformly determinate".into()
                        }
                    }),
                };
            }
            return Determinacy::Convergent { value };
        }
        if let Some(reason) = self.engine {
            return Determinacy::Unknown { reason };
        }
        if incomplete {
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
    if let Some(value) = recorded {
        return vec![Value::String(value.clone())];
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
    use fidryn_core::ir::{CoreNomination, CoreQuery, CoreVerify, QueryPlan};
    use fidryn_core::types::Type;
    use fidryn_core::{
        Instant, Interval, JurisdictionId, ModuleId, NodeId, NodeMeta, OriginId, SourceManifestId,
        SourceSnapshotId, Term,
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

    fn trust_module() -> CoreModule {
        module_with_query(
            "acting_trustee",
            QueryPlan::UniqueOccupant {
                office: Term::Ident("TrusteeOf(BRT)".into()),
            },
        )
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
        let out = explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx());
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
            Determinacy::Convergent { value } => {
                panic!("two SuccessorEligibility interpretations must not be Convergent: {value:?}")
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
            Determinacy::Convergent { value } => {
                assert_eq!(value.display_label(), "Bob");
            }
            other => panic!("recorded I2 with OccupancyRecord must be Convergent Bob: {other:?}"),
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
            Determinacy::Other(Outcome::Inconsistent { .. }) | Determinacy::Unknown { .. } => {}
            other => panic!("{other:?}"),
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
    fn empty_families_are_one_empty_assignment() {
        let module = bool_module();
        let case = CaseRecord::default();
        let out = explore_query(&module, &QueryName::from("q"), &case, &ctx());
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
        let out = explore_query(&module, &QueryName::from("q"), &case, &ctx());
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
        let out = explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx());
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
        let out = explore_query(&module, &QueryName::from("acting_trustee"), &case, &ctx());
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
    fn skeptical_preserves_suspended() {
        let module = trust_module();
        let mut case = CaseRecord::default();
        case.admissible_completions.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["I1".into(), "I2".into()],
        );
        let out = skeptical(&module, &QueryName::from("acting_trustee"), &case, &ctx());
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    #[test]
    fn skeptical_does_not_keep_determinate_over_empty_completion_set() {
        let module = bool_module();
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("ClosedFamily".into(), Vec::new());
        let out = skeptical(&module, &QueryName::from("q"), &case, &ctx());
        assert!(
            matches!(out, Outcome::Inconsistent { .. }),
            "determinate inner answer must not hide an empty completion set: {out:?}"
        );
    }
}
