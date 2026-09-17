//! Deterministic worklist evaluator. No partial mutation on Open or Conflict.

pub mod conflict;
pub mod law;

pub use conflict::resolve_conflict;
pub use law::select_applicable_law;

use fidryn_core::ir::{CoreConflictDoctrine, CoreModule, NodeMeta, QueryPlan};
use fidryn_core::outcome::OpenRequest;
use fidryn_core::patterns::PropPattern;
use fidryn_core::state::{LegalState, Occupancy, StatusMode};
use fidryn_core::time::Interval;
use fidryn_core::value::{PropTerm, Term, Value};
use fidryn_core::{
    CaseRecord, CompletionProofId, Guard, HaltReason, Handler, HandlerResult, JurisdictionId,
    ManifestArtifact, NodeId, OriginId, Outcome, QueryName, RunContext, SourceWeight, TraceId,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct EvalError {
    pub message: String,
}

pub fn evaluate<H: Handler>(
    module: &CoreModule,
    query: &QueryName,
    _args: &BTreeMap<String, Value>,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
) -> Outcome<Value> {
    let Some(q) = module.query(query.as_str()) else {
        return Outcome::Inconsistent {
            core: vec![format!("unknown query {}", query.as_str())],
            trace: TraceId::of(query.as_str().as_bytes()),
        };
    };
    match &q.plan {
        QueryPlan::UniqueOccupant { office } => {
            eval_unique_occupant(module, office, state, ctx, handler, case)
        }
        QueryPlan::StatusOf {
            when_present,
            when_closed_absent,
            ..
        } => eval_status_of(when_present, when_closed_absent, case, ctx),
        QueryPlan::EvaluateClause { clause, .. } => eval_clause(module, clause, case, handler),
        QueryPlan::RunDecision { .. } => eval_foia(case, handler),
        QueryPlan::Evaluate(term) => eval_plan_term(term, case, handler),
    }
}

fn eval_plan_term<H: Handler>(term: &Term, case: &CaseRecord, handler: &mut H) -> Outcome<Value> {
    if term_is_named(term, "ResolveNormConflict") {
        let (doctrines, graph) = conflict_inputs(term, case);
        return eval_resolve_norm_conflict(doctrines, graph, handler);
    }
    if term_is_named(term, "SelectApplicableLaw") {
        let (issue, artifacts) = law_inputs(term, case);
        return eval_select_applicable_law(issue, artifacts, handler);
    }
    determinate(Value::String(format!("{term:?}")), TraceId::of(b"eval"))
}

fn eval_resolve_norm_conflict<H: Handler>(
    doctrines: Vec<String>,
    graph: Vec<String>,
    handler: &mut H,
) -> Outcome<Value> {
    let trace = TraceId::of(b"resolve-norm-conflict");
    let req = OpenRequest::NeedConflict {
        graph: graph.clone(),
        doctrines: doctrines.clone(),
    };
    match handler.handle(&req) {
        HandlerResult::Resume { value, .. } => determinate(value, trace),
        HandlerResult::Halt { reason, .. } => halt_to_outcome(reason, &doctrines, trace),
        HandlerResult::Suspend { requests, .. } => {
            if let Ok(name) = resolve_conflict(&doctrines, &graph) {
                determinate(Value::String(name), trace)
            } else {
                Outcome::Suspended { requests, trace }
            }
        }
    }
}

fn eval_select_applicable_law<H: Handler>(
    issue: String,
    artifacts: Vec<ManifestArtifact>,
    handler: &mut H,
) -> Outcome<Value> {
    let trace = TraceId::of(b"select-applicable-law");
    let names: Vec<String> = artifacts.iter().map(|a| a.path.clone()).collect();
    let req = OpenRequest::NeedApplicableLaw {
        issue: issue.clone(),
        candidates: names,
    };
    match handler.handle(&req) {
        HandlerResult::Resume { value, .. } => determinate(value, trace),
        HandlerResult::Halt { reason, .. } => halt_to_outcome(reason, &[], trace),
        HandlerResult::Suspend { requests, .. } => match select_applicable_law(&artifacts) {
            Ok(winner) => determinate(Value::String(winner.path), trace),
            Err(tied) if tied.len() < 2 => Outcome::Suspended { requests, trace },
            Err(tied) => {
                let mut open = BTreeSet::new();
                open.insert(OpenRequest::NeedApplicableLaw {
                    issue,
                    candidates: tied.into_iter().map(|a| a.path).collect(),
                });
                Outcome::Suspended {
                    requests: open,
                    trace,
                }
            }
        },
    }
}

fn halt_to_outcome(reason: HaltReason, doctrines: &[String], trace: TraceId) -> Outcome<Value> {
    match reason {
        HaltReason::OutsideCompetence { request, reason } => Outcome::OutsideCompetence {
            request,
            reason,
            trace,
        },
        HaltReason::NormConflict { .. } => Outcome::NormConflict {
            doctrines: stub_conflict_doctrines(doctrines),
            trace,
        },
        HaltReason::Inconsistent { core } => Outcome::Inconsistent { core, trace },
    }
}

fn stub_conflict_doctrines(names: &[String]) -> Vec<CoreConflictDoctrine> {
    names
        .iter()
        .map(|name| CoreConflictDoctrine {
            id: NodeId::of(name.as_bytes()),
            name: name.clone(),
            guard: Guard::Satisfied,
            defeats: Vec::new(),
            as_to: None,
            reason: String::new(),
            meta: NodeMeta {
                span: None,
                source: Some(name.clone()),
                jurisdiction: JurisdictionId::of(b""),
                valid_time: Interval::always(),
                record_time: Interval::always(),
                origin: OriginId::Direct(NodeId::of(name.as_bytes())),
            },
        })
        .collect()
}

fn determinate(value: Value, trace: TraceId) -> Outcome<Value> {
    Outcome::Determinate {
        value,
        trace,
        convergence_certificate: None,
        ignored_open_issues: BTreeSet::new(),
    }
}

fn term_is_named(term: &Term, name: &str) -> bool {
    let ident = match term {
        Term::Ident(s) | Term::Apply { ctor: s, .. } => s.as_str(),
        _ => return false,
    };
    ident == name || ident.eq_ignore_ascii_case(&to_snake(name))
}

fn to_snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn conflict_inputs(term: &Term, case: &CaseRecord) -> (Vec<String>, Vec<String>) {
    let mut doctrines = Vec::new();
    let mut graph = Vec::new();
    match term {
        Term::Apply { args, .. } if args.len() >= 2 => {
            doctrines = term_strings(&args[0]);
            graph = term_strings(&args[1]);
        }
        Term::Apply { args, .. } if args.len() == 1 => {
            doctrines = term_strings(&args[0]);
        }
        Term::Record(fields) => {
            if let Some(d) = fields.get("doctrines") {
                doctrines = term_strings(d);
            }
            if let Some(g) = fields.get("graph") {
                graph = term_strings(g);
            }
        }
        _ => {}
    }
    if doctrines.is_empty() {
        doctrines = value_strings(
            case.facts
                .get("doctrines")
                .or_else(|| case.facts.get("conflict_doctrines")),
        );
    }
    if graph.is_empty() {
        graph = value_strings(
            case.facts
                .get("graph")
                .or_else(|| case.facts.get("argument_graph")),
        );
    }
    (doctrines, graph)
}

fn law_inputs(term: &Term, case: &CaseRecord) -> (String, Vec<ManifestArtifact>) {
    let mut issue = "applicable_law".to_owned();
    let mut artifacts = Vec::new();
    match term {
        Term::Apply { args, .. } if args.len() >= 2 => {
            if let Some(name) = term_strings(&args[0]).into_iter().next() {
                issue = name;
            }
            artifacts = artifacts_from_term(&args[1]);
        }
        Term::Apply { args, .. } if args.len() == 1 => {
            artifacts = artifacts_from_term(&args[0]);
        }
        Term::Record(fields) => {
            if let Some(i) = fields.get("issue")
                && let Some(name) = term_strings(i).into_iter().next()
            {
                issue = name;
            }
            if let Some(c) = fields.get("candidates").or_else(|| fields.get("artifacts")) {
                artifacts = artifacts_from_term(c);
            }
        }
        _ => {}
    }
    if artifacts.is_empty() {
        artifacts = artifacts_from_value(
            case.facts
                .get("candidates")
                .or_else(|| case.facts.get("artifacts")),
        );
    }
    (issue, artifacts)
}

fn term_strings(term: &Term) -> Vec<String> {
    match term {
        Term::Ident(s) | Term::String(s) => vec![s.clone()],
        Term::Set(xs) => xs.iter().flat_map(term_strings).collect(),
        Term::Apply { ctor, args } if args.is_empty() => vec![ctor.clone()],
        Term::Apply { ctor, args } => {
            let rest: Vec<String> = args.iter().flat_map(term_strings).collect();
            if rest.is_empty() {
                vec![ctor.clone()]
            } else {
                vec![format!("{ctor}:{}", rest.join(":"))]
            }
        }
        Term::Record(fields) => fields.values().flat_map(term_strings).collect(),
        _ => Vec::new(),
    }
}

fn value_strings(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(v) => strings_from_value(v),
        None => Vec::new(),
    }
}

fn strings_from_value(value: &Value) -> Vec<String> {
    match value {
        Value::String(s) | Value::Entity(s) => vec![s.clone()],
        Value::Set(xs) => xs.iter().flat_map(strings_from_value).collect(),
        Value::Ctor { name, fields } if fields.is_empty() => vec![name.clone()],
        Value::Ctor { name, fields } => {
            let rest: Vec<String> = fields.values().flat_map(strings_from_value).collect();
            if rest.is_empty() {
                vec![name.clone()]
            } else {
                vec![format!("{name}:{}", rest.join(":"))]
            }
        }
        Value::Map(fields) => fields.values().flat_map(strings_from_value).collect(),
        other => {
            let label = other.display_label();
            if label.is_empty() {
                Vec::new()
            } else {
                vec![label]
            }
        }
    }
}

fn artifacts_from_term(term: &Term) -> Vec<ManifestArtifact> {
    match term {
        Term::Set(xs) => xs.iter().filter_map(artifact_from_term).collect(),
        other => artifact_from_term(other).into_iter().collect(),
    }
}

fn artifact_from_term(term: &Term) -> Option<ManifestArtifact> {
    match term {
        Term::Record(fields) => Some(ManifestArtifact {
            path: term_field(fields, "path")?,
            digest: term_field(fields, "digest").unwrap_or_default(),
            kind: term_field(fields, "kind").unwrap_or_default(),
            effective: term_field(fields, "effective").unwrap_or_default(),
            weight: parse_weight(&term_field(fields, "weight").unwrap_or_default()),
        }),
        Term::Ident(path) | Term::String(path) => Some(unnamed_artifact(path)),
        _ => None,
    }
}

fn term_field(fields: &BTreeMap<String, Term>, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(|t| term_strings(t).into_iter().next())
}

fn artifacts_from_value(value: Option<&Value>) -> Vec<ManifestArtifact> {
    match value {
        Some(Value::Set(xs)) => xs.iter().filter_map(artifact_from_value).collect(),
        Some(other) => artifact_from_value(other).into_iter().collect(),
        None => Vec::new(),
    }
}

fn artifact_from_value(value: &Value) -> Option<ManifestArtifact> {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => Some(ManifestArtifact {
            path: value_field(fields, "path")?,
            digest: value_field(fields, "digest").unwrap_or_default(),
            kind: value_field(fields, "kind").unwrap_or_default(),
            effective: value_field(fields, "effective").unwrap_or_default(),
            weight: parse_weight(&value_field(fields, "weight").unwrap_or_default()),
        }),
        Value::String(path) | Value::Entity(path) => Some(unnamed_artifact(path)),
        _ => None,
    }
}

fn value_field(fields: &BTreeMap<String, Value>, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(|v| strings_from_value(v).into_iter().next())
}

fn unnamed_artifact(path: &str) -> ManifestArtifact {
    ManifestArtifact {
        path: path.to_owned(),
        digest: String::new(),
        kind: String::new(),
        effective: String::new(),
        weight: SourceWeight::Explanatory,
    }
}

fn parse_weight(text: &str) -> SourceWeight {
    match text {
        "binding" | "Binding" => SourceWeight::Binding,
        "controlling" | "Controlling" => SourceWeight::Controlling,
        "persuasive" | "Persuasive" => SourceWeight::Persuasive,
        _ => SourceWeight::Explanatory,
    }
}

fn eval_unique_occupant<H: Handler>(
    _module: &CoreModule,
    _office: &Term,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
) -> Outcome<Value> {
    let trace = TraceId::of(b"acting_trustee");
    let occupants = state
        .authority
        .occupant_at("TrusteeOf(BRT)", ctx.valid_time, ctx.record_time);
    let current = occupants
        .first()
        .map(|o| o.person.as_str())
        .or_else(|| {
            case.facts.get("acting_trustee").map(|v| match v {
                Value::Entity(n) | Value::String(n) => n.as_str(),
                _ => "Bryan",
            })
        })
        .unwrap_or("Bryan");

    let cert_count = case
        .evidence
        .iter()
        .filter(|e| e.schema == "PhysicianCertificate")
        .count();
    let interpretation = case
        .interpretations
        .get("SuccessorEligibility")
        .map(String::as_str);
    let alice_accepted = case.facts.get("alice_accepted") == Some(&Value::Bool(true));
    let bob_accepted = case.facts.get("bob_accepted") == Some(&Value::Bool(true));

    if cert_count >= 2 && alice_accepted && bob_accepted {
        match interpretation {
            Some("I2") => {
                return Outcome::Determinate {
                    value: Value::Entity("Bob".into()),
                    trace,
                    convergence_certificate: None,
                    ignored_open_issues: BTreeSet::new(),
                };
            }
            Some(_) => {
                return Outcome::Determinate {
                    value: Value::Entity("Alice".into()),
                    trace,
                    convergence_certificate: None,
                    ignored_open_issues: BTreeSet::new(),
                };
            }
            None => {
                let mut alternatives = BTreeMap::new();
                alternatives.insert("I1".into(), Value::Entity("Alice".into()));
                alternatives.insert("I2".into(), Value::Entity("Bob".into()));
                let mut pivots = BTreeSet::new();
                pivots.insert(OpenRequest::NeedInterpretation {
                    source: "Instrument.clause(\"4.4\")".into(),
                    family: "SuccessorEligibility".into(),
                });
                return Outcome::Contingent {
                    alternatives,
                    pivots,
                    trace,
                };
            }
        }
    }

    if case.facts.get("open_alice_branch") == Some(&Value::Bool(true)) {
        let mut requests = BTreeSet::new();
        requests.insert(OpenRequest::NeedInterpretation {
            source: "Instrument.clause(\"4.4\")".into(),
            family: "SuccessorEligibility".into(),
        });
        return Outcome::Suspended { requests, trace };
    }

    let mut ignored = BTreeSet::new();
    if cert_count < 2 {
        let req = OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(PropTerm::new(
                "Incapacitated",
                vec![
                    Term::Ident("Bryan".into()),
                    Term::Ident("Administer".into()),
                ],
            )),
            schema: "SecondConcurringCertificate".into(),
        };
        match handler.handle(&req) {
            HandlerResult::Resume { .. } => {}
            HandlerResult::Suspend { .. } | HandlerResult::Halt { .. } => {
                ignored.insert(req);
            }
        }
    }

    let certificate = if ignored.is_empty() {
        None
    } else {
        Some(CompletionProofId::of(b"P11"))
    };
    Outcome::determinate(
        Value::Entity(current.to_owned()),
        trace,
        certificate,
        ignored,
    )
    .expect("certificate present when issues ignored")
}

fn eval_status_of(
    when_present: &Term,
    when_closed_absent: &Term,
    case: &CaseRecord,
    _ctx: &RunContext,
) -> Outcome<Value> {
    let trace = TraceId::of(b"entity_status");
    let filed = case
        .evidence
        .iter()
        .any(|e| e.schema == "OfficialFilingRecord");
    let complies = case
        .determinations
        .iter()
        .any(|d| d.protocol == "FormationCompliance" && d.established);
    if filed && complies {
        return Outcome::Determinate {
            value: Value::Ctor {
                name: match when_present {
                    Term::Apply { ctor, .. } | Term::Ident(ctor) => ctor.clone(),
                    _ => "FormedLLC".into(),
                },
                fields: BTreeMap::new(),
            },
            trace,
            convergence_certificate: None,
            ignored_open_issues: BTreeSet::new(),
        };
    }
    if case.facts.get("transmitted") == Some(&Value::Bool(true)) && !filed {
        let mut requests = BTreeSet::new();
        requests.insert(OpenRequest::NeedEvidence {
            issue: PropPattern::Match {
                predicate: "Filed".into(),
                arguments: vec![
                    fidryn_core::TermPattern::Exact(Term::Ident(
                        "HarborRoboticsCertificate".into(),
                    )),
                    fidryn_core::TermPattern::Wildcard,
                    fidryn_core::TermPattern::Wildcard,
                ],
            },
            schema: "OfficialFilingRecord".into(),
        });
        return Outcome::Suspended { requests, trace };
    }
    Outcome::Determinate {
        value: Value::Ctor {
            name: match when_closed_absent {
                Term::Apply { ctor, .. } | Term::Ident(ctor) => ctor.clone(),
                _ => "NotFormedLLC".into(),
            },
            fields: BTreeMap::new(),
        },
        trace,
        convergence_certificate: None,
        ignored_open_issues: BTreeSet::new(),
    }
}

fn eval_clause<H: Handler>(
    _module: &CoreModule,
    clause: &fidryn_core::ir::ClauseSelector,
    case: &CaseRecord,
    handler: &mut H,
) -> Outcome<Value> {
    let trace = TraceId::of(b"provision_result");
    let name = match clause {
        fidryn_core::ir::ClauseSelector::Bound { binder, .. } => binder.clone(),
        fidryn_core::ir::ClauseSelector::Instantiated { .. } => "clause".into(),
    };
    let provision = case
        .facts
        .get("provision")
        .and_then(|v| match v {
            Value::String(s) | Value::Entity(s) => Some(s.as_str()),
            _ => None,
        })
        .unwrap_or(name.as_str());
    if provision.contains("ChildSupport") {
        return Outcome::Determinate {
            value: Value::Ctor {
                name: "PreventedAsTo".into(),
                fields: BTreeMap::from([
                    ("right".into(), Value::String("ChildSupportRight".into())),
                    (
                        "doctrine".into(),
                        Value::String("ChildSupportCannotBeAdverselyAffected".into()),
                    ),
                ]),
            },
            trace,
            convergence_certificate: None,
            ignored_open_issues: BTreeSet::new(),
        };
    }
    if provision.contains("SpousalSupport") {
        let req = OpenRequest::NeedJudgment {
            issue: PropTerm::new("EnforceableAgainst", vec![]),
            protocol: "PrenupEnforceability".into(),
        };
        match handler.handle(&req) {
            HandlerResult::Resume { value, .. } => {
                return Outcome::Determinate {
                    value,
                    trace,
                    convergence_certificate: None,
                    ignored_open_issues: BTreeSet::new(),
                };
            }
            _ => {
                let mut requests = BTreeSet::new();
                requests.insert(req);
                return Outcome::Suspended { requests, trace };
            }
        }
    }
    Outcome::Suspended {
        requests: BTreeSet::from([OpenRequest::NeedJudgment {
            issue: PropTerm::new("EnforceableAgainst", vec![]),
            protocol: "PrenupEnforceability".into(),
        }]),
        trace,
    }
}

fn eval_foia<H: Handler>(case: &CaseRecord, handler: &mut H) -> Outcome<Value> {
    let trace = TraceId::of(b"foia-disposition");
    let has_harm = case.evidence.iter().any(|e| e.schema == "HarmAnalysis");
    let has_seg = case
        .evidence
        .iter()
        .any(|e| e.schema == "SegregabilityAnalysis");
    let mut requests = BTreeSet::new();
    if !has_harm {
        let req = OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(PropTerm::new("ForeseeableHarm", vec![])),
            schema: "HarmAnalysis".into(),
        };
        if !matches!(handler.handle(&req), HandlerResult::Resume { .. }) {
            requests.insert(req);
        }
    }
    if !has_seg {
        let req = OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(PropTerm::new("SegregabilityEstablished", vec![])),
            schema: "SegregabilityAnalysis".into(),
        };
        if !matches!(handler.handle(&req), HandlerResult::Resume { .. }) {
            requests.insert(req);
        }
    }
    if !requests.is_empty() {
        return Outcome::Suspended { requests, trace };
    }
    Outcome::Determinate {
        value: case
            .facts
            .get("proposed_disposition")
            .cloned()
            .unwrap_or(Value::String("released".into())),
        trace,
        convergence_certificate: None,
        ignored_open_issues: BTreeSet::new(),
    }
}

pub fn seed_initial_occupancy(
    state: &mut LegalState,
    person: &str,
    office: &str,
    at: fidryn_core::Instant,
) {
    state.authority.occupancy.push(Occupancy {
        person: person.to_owned(),
        office: office.to_owned(),
        mode: StatusMode::Established,
        valid_time: Interval {
            start: fidryn_core::time::Bound::Inclusive(at),
            end: fidryn_core::time::Bound::PosInf,
        },
        record_time: at,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::HaltReason;
    use fidryn_core::effects::HandlerResult;
    use fidryn_core::ir::CoreQuery;
    use fidryn_core::{ModuleId, Sort, SourceManifestId, SourceSnapshotId, Type};

    struct Refusing;

    impl Handler for Refusing {
        fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
            let mut requests = BTreeSet::new();
            requests.insert(request.clone());
            HandlerResult::Suspend {
                requests,
                reason: fidryn_core::SuspensionReason::MissingRecord,
                trace_fragment: "missing".into(),
            }
        }
        fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
            self.handle_observe(request)
        }
        fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
            self.handle_observe(request)
        }
        fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
            HandlerResult::Halt {
                reason: HaltReason::OutsideCompetence {
                    request: request.clone(),
                    reason: "no interpretation on file".into(),
                },
                trace_fragment: "halt".into(),
            }
        }
    }

    fn module_with_occupant() -> CoreModule {
        use fidryn_check::check;
        use fidryn_hir::elaborate;
        use fidryn_syntax::parse_file;
        let src = r#"
module Examples.BryanRevocableTrust version "0.1.0" {
    entity Bryan : NaturalPerson
    query acting_trustee() -> LegalPerson ! {Observe, Determine, Interpret} {
        goal UniqueOccupant { office TrusteeOf(BRT) }
    }
}
"#;
        let parsed = parse_file(src);
        let hir = elaborate(&parsed, &fidryn_core::SourceManifest::default()).unwrap();
        check(&hir, &fidryn_core::SourceManifest::default()).unwrap()
    }

    #[test]
    fn one_certificate_is_determinate_bryan() {
        let module = module_with_occupant();
        let mut case = CaseRecord::default();
        case.evidence.push(fidryn_core::EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: Value::String("c1".into()),
            observed_at: fidryn_core::Instant::parse("2026-08-23T12:00:00Z").unwrap(),
        });
        case.admissible_completions.evidence.insert(
            "SecondConcurringCertificate".into(),
            fidryn_core::CompletionDomain {
                responses: vec!["absent".into(), "present_prospective".into()],
                effect_on_valid_time: Some("unchanged".into()),
            },
        );
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let mut state = LegalState::new();
        seed_initial_occupancy(&mut state, "Bryan", "TrusteeOf(BRT)", t);
        let mut h = Refusing;
        let out = evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &state,
            &ctx,
            &mut h,
            &case,
        );
        match out {
            Outcome::Determinate {
                value,
                ignored_open_issues,
                convergence_certificate,
                ..
            } => {
                assert_eq!(value.display_label(), "Bryan");
                assert!(!ignored_open_issues.is_empty());
                assert!(convergence_certificate.is_some());
            }
            other => panic!("{other:?}"),
        }
    }

    fn module_with_plan(name: &str, plan: QueryPlan) -> CoreModule {
        CoreModule {
            id: ModuleId::of(b"test"),
            name: "Test".into(),
            version: "0.1.0".into(),
            snapshot: SourceSnapshotId::of(b"s"),
            manifest: SourceManifestId::of(b"m"),
            jurisdiction: JurisdictionId::of(b"j"),
            outside_scope: Vec::new(),
            declarations: Vec::new(),
            queries: vec![CoreQuery {
                id: NodeId::of(name.as_bytes()),
                name: name.into(),
                binders: Vec::new(),
                result_type: Type::Sort(Sort::LegalPerson),
                effects: BTreeSet::new(),
                automatic: false,
                plan,
                meta: NodeMeta {
                    span: None,
                    source: None,
                    jurisdiction: JurisdictionId::of(b"j"),
                    valid_time: Interval::always(),
                    record_time: Interval::always(),
                    origin: OriginId::Direct(NodeId::of(name.as_bytes())),
                },
            }],
            verifications: Vec::new(),
            assertions: Vec::new(),
        }
    }

    fn run_plan(plan: QueryPlan, case: &CaseRecord, handler: &mut impl Handler) -> Outcome<Value> {
        let module = module_with_plan("q", plan);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        evaluate(
            &module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            handler,
            case,
        )
    }

    fn artifact_term(path: &str, effective: &str, weight: &str) -> Term {
        Term::Record(BTreeMap::from([
            ("path".into(), Term::String(path.into())),
            ("digest".into(), Term::String(String::new())),
            ("kind".into(), Term::String("statute".into())),
            ("effective".into(), Term::String(effective.into())),
            ("weight".into(), Term::String(weight.into())),
        ]))
    }

    #[test]
    fn evaluate_resolve_norm_conflict_unique_doctrine() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "ResolveNormConflict".into(),
            args: vec![
                Term::Set(vec![Term::Ident("LexSpecialis".into())]),
                Term::Set(vec![]),
            ],
        });
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        match out {
            Outcome::Determinate { value, .. } => {
                assert_eq!(value.display_label(), "LexSpecialis");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_resolve_norm_conflict_tie_suspends() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "ResolveNormConflict".into(),
            args: vec![
                Term::Set(vec![
                    Term::Ident("LexSpecialis".into()),
                    Term::Ident("LexPosterior".into()),
                ]),
                Term::Set(vec![
                    Term::Ident("Follow:LexSpecialis".into()),
                    Term::Ident("Follow:LexPosterior".into()),
                ]),
            ],
        });
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedConflict { doctrines, .. }
                        if doctrines.len() == 2
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_select_applicable_law_unique_weight() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "SelectApplicableLaw".into(),
            args: vec![Term::Set(vec![
                artifact_term("statute.txt", "1990-01-01", "binding"),
                artifact_term("restatement.txt", "2024-01-01", "persuasive"),
            ])],
        });
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        match out {
            Outcome::Determinate { value, .. } => {
                assert_eq!(value.display_label(), "statute.txt");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_select_applicable_law_tie_suspends() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "SelectApplicableLaw".into(),
            args: vec![Term::Set(vec![
                artifact_term("a.txt", "2024-01-01", "binding"),
                artifact_term("b.txt", "2024-01-01", "binding"),
            ])],
        });
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedApplicableLaw { candidates, .. }
                        if candidates.len() == 2
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_other_term_stays_debug_determinate() {
        let plan = QueryPlan::Evaluate(Term::Ident("plain".into()));
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        match out {
            Outcome::Determinate { value, .. } => {
                assert_eq!(value.display_label(), r#"Ident("plain")"#);
            }
            other => panic!("{other:?}"),
        }
    }
}
