//! Bounded completion explorer and invariant checks.

mod property;

pub use property::{VerificationBudget, verify_property, verify_property_with_budget};

use fidryn_core::ir::CoreModule;
use fidryn_core::{
    CaseRecord, CompletionAdmissionError, CompletionBranch, CompletionSearchEvidence,
    CoverageWitness, EngineError, OpenRequest, Outcome, QueryName, RunContext, TraceId,
    ValidatedCompletionModel, Value, VerificationBounds,
};
use fidryn_eval::{
    DerivationStep, EvalSession, evaluate, evaluate_scenario_traced, evaluate_traced,
};
use fidryn_handlers::{CaseFile, ExplorationBounds, Explore, Skeptical, aggregate_completions};
use fidryn_solve::{Assignment, Domain, SearchBudget, SearchEvent, stream};
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
    Ok(explore_query_traced(module, query, case, ctx)?.outcome)
}

/// Actual evaluated branches and their content-addressed derivations.
#[derive(Clone, Debug)]
pub struct ExplorationSession {
    pub outcome: Outcome<Value>,
    pub derivation: Vec<DerivationStep>,
    pub coverage: Coverage,
    pub evidence: CompletionSearchEvidence,
}

/// Explore once, retaining each executed branch's derivation and exact
/// coverage. The combined graph and branch count have independent limits.
pub fn explore_query_traced(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<ExplorationSession, EngineError> {
    if module.query(query.as_str()).is_none() {
        return Err(EngineError::UnknownQuery(query.as_str().to_owned()));
    }
    let model = match ValidatedCompletionModel::admit(case) {
        Ok(model) => model,
        Err(error) => {
            let outcome = admission_outcome(error)?;
            return finish_session(
                module,
                query,
                case,
                ctx,
                outcome,
                Vec::new(),
                Vec::new(),
                CompletionSearchEvidence {
                    examined: 0,
                    total: 0,
                    incomplete: false,
                    branches: Vec::new(),
                },
            );
        }
    };
    let total = model.product_size().map_err(EngineError::InvalidInput)?;
    let domains = model
        .slots()
        .iter()
        .map(|slot| Domain {
            name: slot.name.clone(),
            values: slot.values.clone(),
        })
        .collect::<Vec<_>>();
    let mut derivation = Vec::new();
    let mut ids = BTreeSet::new();
    let mut roots = Vec::new();
    let mut branches = Vec::new();
    let mut evidence_branches = Vec::new();
    let mut incomplete = false;
    for event in stream(
        &domains,
        SearchBudget {
            max_assignments: MAX_COMPLETIONS,
        },
    ) {
        match event {
            SearchEvent::Assignment(assignment) => {
                let session = eval_assignment_traced(module, query, case, ctx, &assignment)?;
                for step in session.derivation {
                    if ids.insert(step.id.clone()) {
                        push_derivation(&mut derivation, step)?;
                    }
                }
                let label = assignment_identity(&assignment);
                let root = session.outcome.trace().hex();
                if !ids.contains(&root) {
                    return Err(EngineError::Internal(
                        "evaluated completion has no derivation root".into(),
                    ));
                }
                let parents = vec![root];
                let detail = fidryn_core::canonical_json(&serde_json::json!({ "assignment":assignment.bindings, "outcome":session.outcome })).map_err(|error| EngineError::Internal(error.to_string()))?;
                let id = TraceId::of(
                    format!("fidryn.completion-assignment/v0.1\0{detail}\0{parents:?}").as_bytes(),
                )
                .hex();
                if ids.insert(id.clone()) {
                    push_derivation(
                        &mut derivation,
                        DerivationStep {
                            id: id.clone(),
                            kind: "completion_assignment".into(),
                            detail,
                            parents,
                            retained_bytes: 0,
                        },
                    )?;
                }
                roots.push(id);
                evidence_branches.push(CompletionBranch {
                    bindings: assignment.bindings,
                    outcome: session.outcome.clone(),
                });
                branches.push((label, session.outcome));
            }
            SearchEvent::BudgetExceeded { .. } => incomplete = true,
            SearchEvent::Exhausted => {}
        }
    }
    let evidence = CompletionSearchEvidence {
        examined: branches.len(),
        total,
        incomplete,
        branches: evidence_branches,
    };
    let outcome = aggregate_completions(branches, incomplete)?;
    finish_session(
        module, query, case, ctx, outcome, derivation, roots, evidence,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_session(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
    mut outcome: Outcome<Value>,
    mut derivation: Vec<DerivationStep>,
    mut roots: Vec<String>,
    evidence: CompletionSearchEvidence,
) -> Result<ExplorationSession, EngineError> {
    let coverage = Coverage {
        examined: evidence.examined,
        total: evidence.total,
        incomplete: evidence.incomplete,
    };
    roots.sort();
    roots.dedup();
    let detail = fidryn_core::canonical_json(&serde_json::json!({
        "program":module.program_digest().map_err(EngineError::Internal)?, "case":fidryn_core::CaseSnapshotIdentity::of(case).map_err(EngineError::Internal)?,
        "query":query, "validAt":ctx.valid_time, "knownAt":ctx.record_time,
        "examined":coverage.examined, "total":coverage.total, "incomplete":coverage.incomplete, "outcome":outcome,
    })).map_err(|error| EngineError::Internal(error.to_string()))?;
    let trace =
        TraceId::of(format!("fidryn.completion-search/v0.1\0{detail}\0{roots:?}").as_bytes());
    push_derivation(
        &mut derivation,
        DerivationStep {
            id: trace.hex(),
            kind: "completion_search".into(),
            detail,
            parents: roots,
            retained_bytes: 0,
        },
    )?;
    match &mut outcome {
        Outcome::Determinate { trace: id, .. }
        | Outcome::Contingent { trace: id, .. }
        | Outcome::Suspended { trace: id, .. }
        | Outcome::NormConflict { trace: id, .. }
        | Outcome::OutsideCompetence { trace: id, .. }
        | Outcome::Inconsistent { trace: id, .. } => *id = trace,
    }
    Ok(ExplorationSession {
        outcome,
        derivation,
        coverage,
        evidence,
    })
}

fn push_derivation(
    derivation: &mut Vec<DerivationStep>,
    mut step: DerivationStep,
) -> Result<(), EngineError> {
    let prior = derivation.last().map_or(0, |step| step.retained_bytes);
    let bytes = step
        .id
        .len()
        .saturating_add(step.kind.len())
        .saturating_add(step.detail.len())
        .saturating_add(step.parents.iter().map(String::len).sum::<usize>());
    let retained = prior.saturating_add(bytes);
    if derivation.len() >= 100_000 || step.detail.len() > 1024 * 1024 || retained > 16 * 1024 * 1024
    {
        return Err(EngineError::FuelExhausted { remaining: 0 });
    }
    step.retained_bytes = retained;
    derivation.push(step);
    Ok(())
}

/// Two-phase search: one admissible `v`, then a witness of `≠ v`.
///
/// Domains are exactly `case.admissible_completions`. Completions are
/// streamed under the fixed completion budget; the full product is never built
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
    let model = match ValidatedCompletionModel::admit(case) {
        Ok(model) => model,
        Err(error) => return Ok(Determinacy::Other(admission_outcome(error)?)),
    };
    let domains = model
        .slots()
        .iter()
        .map(|slot| Domain {
            name: slot.name.clone(),
            values: slot.values.clone(),
        })
        .collect::<Vec<_>>();
    let mut scan = DetScan {
        total: model.product_size().map_err(EngineError::InvalidInput)?,
        ..DetScan::default()
    };
    for event in stream(&domains, budget) {
        match event {
            SearchEvent::Assignment(assignment) => {
                scan.examined += 1;
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
    let state = case.into_state();
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

fn eval_assignment(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
    assignment: &Assignment,
) -> Result<Outcome<Value>, EngineError> {
    Ok(eval_assignment_traced(module, query, case, ctx, assignment)?.outcome)
}

fn eval_assignment_traced(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
    assignment: &Assignment,
) -> Result<EvalSession, EngineError> {
    let model = ValidatedCompletionModel::admit(case)
        .map_err(|error| EngineError::InvalidInput(error.to_string()))?;
    let branched = model
        .overlay(case, &assignment.bindings, ctx)
        .map_err(EngineError::InvalidInput)?;
    let mut handler = CaseFile {
        record: branched.clone(),
        known_at: Some(ctx.record_time),
    };
    let state = branched.into_state();
    // Nonempty case.assumptions must change the explored answer, not only
    // the CLI/mill envelope. Operative `evaluate` ignores that overlay.
    if branched.assumptions.is_empty() {
        evaluate_traced(
            module,
            query,
            &BTreeMap::new(),
            &state,
            ctx,
            &mut handler,
            &branched,
        )
    } else {
        evaluate_scenario_traced(
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

#[derive(Default)]
struct DetScan {
    witness: Option<(String, Value)>,
    engine: Option<String>,
    suspended: BTreeSet<OpenRequest>,
    saw_suspended: bool,
    other: Option<Outcome<Value>>,
    examined: usize,
    total: usize,
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
            total: self.total,
            incomplete: self.incomplete || self.unresolved_outside || self.examined < self.total,
        }
    }

    fn absorb(
        &mut self,
        label: &str,
        outcome: Outcome<Value>,
        domains: &[Domain],
    ) -> Option<Determinacy> {
        match outcome {
            Outcome::Determinate { value, .. } => self.record(label.to_owned(), value),
            contingent @ Outcome::Contingent { .. } => {
                // Inner alternatives have not become determinate admitted
                // assignments merely by sharing an outer completion.
                self.other.get_or_insert(contingent);
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
                || self.examined != self.total
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

fn admission_outcome(error: CompletionAdmissionError) -> Result<Outcome<Value>, EngineError> {
    match error {
        CompletionAdmissionError::EmptyDomain { .. } => Ok(Outcome::Inconsistent {
            core: vec!["empty completion set".into()],
            trace: TraceId::of(b"empty"),
        }),
        CompletionAdmissionError::OutsideDomain { request, selection } => {
            Ok(Outcome::OutsideCompetence {
                request,
                reason: format!("recorded selection `{selection}` is not in the admissible set"),
                trace: TraceId::of(b"completion-admission"),
            })
        }
        CompletionAdmissionError::InvalidModel(reason) => Err(EngineError::InvalidInput(reason)),
    }
}

fn assignment_identity(assignment: &Assignment) -> String {
    fidryn_core::canonical_json(&assignment.bindings).expect("completion assignment is encodable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::EvidenceItem;
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
        module.declarations.push(CoreDecl::Judgment(Box::new(
            fidryn_core::ir::CoreJudgment {
                id: NodeId::of(b"Incapacity"),
                name: "Incapacity".into(),
                decides: PropTerm::new(
                    "Incapacitated",
                    vec![
                        Term::Binder("person".into()),
                        Term::Binder("purpose".into()),
                    ],
                ),
                authority: Term::Ident("Court".into()),
                record_requires: vec![(
                    "certificate".into(),
                    Type::Sort(fidryn_core::Sort::Nominal("PhysicianCertificate".into())),
                )],
                binders: Vec::new(),
                source: None,
                authority_policy: None,
                record_patterns: Vec::new(),
                standard: None,
                constraints: Vec::new(),
                establish_when: None,
                reject_when: None,
                otherwise: None,
                on_established: Vec::new(),
                review: None,
                unsupported_clauses: Vec::new(),
                meta: node_meta("Incapacity"),
            },
        )));
        module
    }

    fn certificate(issuer: &str) -> Value {
        Value::Map(BTreeMap::from([
            ("issuer".into(), Value::String(issuer.into())),
            ("subject".into(), Value::String("Bryan".into())),
            ("purpose".into(), Value::String("TrusteeOf(BRT)".into())),
            ("conclusion".into(), Value::String("Supports".into())),
        ]))
    }

    fn occupancy() -> Value {
        Value::Map(BTreeMap::from([
            ("person".into(), Value::String("Bryan".into())),
            ("office".into(), Value::String("TrusteeOf(BRT)".into())),
        ]))
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
            assertions: Vec::new(),
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
        case.evidence.push(EvidenceItem {
            schema: "OccupancyRecord".into(),
            value: occupancy(),
            observed_at: t,
        });
        case.facts
            .insert("Accepted(Alice,TrusteeOf(BRT))".into(), Value::Bool(true));
        case.facts
            .insert("Accepted(Bob,TrusteeOf(BRT))".into(), Value::Bool(true));
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: certificate("Doctor1"),
            observed_at: t,
        });
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: certificate("Doctor2"),
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
            .insert("Accepted(Alice,TrusteeOf(BRT))".into(), Value::Bool(true));
        case.facts
            .insert("Accepted(Bob,TrusteeOf(BRT))".into(), Value::Bool(true));
        case.evidence.push(EvidenceItem {
            schema: "OccupancyRecord".into(),
            value: occupancy(),
            observed_at: observed,
        });
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: certificate("Doctor1"),
            observed_at: observed,
        });
        case.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: certificate("Doctor2"),
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
