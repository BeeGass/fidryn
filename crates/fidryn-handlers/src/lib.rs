//! CaseFile, Scenario, Explore, and Skeptical handlers.

use fidryn_core::outcome::OpenRequest;
use fidryn_core::{
    CaseRecord, HaltReason, Handler, HandlerResult, Outcome, SuspensionReason, Value,
};
use fidryn_eval::resolve_conflict;
use std::collections::{BTreeMap, BTreeSet};

pub struct CaseFile {
    pub record: CaseRecord,
}

impl Handler for CaseFile {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedEvidence { schema, .. } => {
                if let Some(item) = self.record.evidence.iter().find(|e| e.schema == *schema) {
                    HandlerResult::Resume {
                        value: item.value.clone(),
                        trace_fragment: format!("observe:{schema}"),
                    }
                } else {
                    let mut requests = BTreeSet::new();
                    requests.insert(request.clone());
                    HandlerResult::Suspend {
                        requests,
                        reason: SuspensionReason::MissingRecord,
                        trace_fragment: format!("need-evidence:{schema}"),
                    }
                }
            }
            _ => self.refuse(request),
        }
    }

    fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedJudgment { protocol, .. } => {
                if let Some(d) = self
                    .record
                    .determinations
                    .iter()
                    .find(|d| d.protocol == *protocol)
                {
                    if d.established {
                        HandlerResult::Resume {
                            value: Value::Bool(true),
                            trace_fragment: format!("determine:{protocol}"),
                        }
                    } else {
                        HandlerResult::Halt {
                            reason: HaltReason::OutsideCompetence {
                                request: request.clone(),
                                reason: "determination present but not established".into(),
                            },
                            trace_fragment: format!("invalid-determination:{protocol}"),
                        }
                    }
                } else {
                    let mut requests = BTreeSet::new();
                    requests.insert(request.clone());
                    HandlerResult::Suspend {
                        requests,
                        reason: SuspensionReason::MissingDetermination,
                        trace_fragment: format!("need-judgment:{protocol}"),
                    }
                }
            }
            _ => self.refuse(request),
        }
    }

    fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedChoice { protocol, options } => {
                if let Some(choice) = self.record.decisions.get(protocol) {
                    if options.is_empty() || options.iter().any(|o| o == choice) {
                        HandlerResult::Resume {
                            value: Value::String(choice.clone()),
                            trace_fragment: format!("choose:{protocol}"),
                        }
                    } else {
                        HandlerResult::Halt {
                            reason: HaltReason::OutsideCompetence {
                                request: request.clone(),
                                reason: "choice not in option space".into(),
                            },
                            trace_fragment: format!("invalid-choice:{protocol}"),
                        }
                    }
                } else {
                    let mut requests = BTreeSet::new();
                    requests.insert(request.clone());
                    HandlerResult::Suspend {
                        requests,
                        reason: SuspensionReason::MissingChoice,
                        trace_fragment: format!("need-choice:{protocol}"),
                    }
                }
            }
            _ => self.refuse(request),
        }
    }

    fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedInterpretation { family, .. } => {
                if let Some(alt) = self.record.interpretations.get(family) {
                    let admitted = self
                        .record
                        .admissible_completions
                        .interpretations
                        .get(family)
                        .map(|v| v.iter().any(|a| a == alt))
                        .unwrap_or(true);
                    if admitted {
                        HandlerResult::Resume {
                            value: Value::String(alt.clone()),
                            trace_fragment: format!("interpret:{family}:{alt}"),
                        }
                    } else {
                        HandlerResult::Halt {
                            reason: HaltReason::OutsideCompetence {
                                request: request.clone(),
                                reason: "inadmissible interpretation".into(),
                            },
                            trace_fragment: format!("inadmissible:{family}"),
                        }
                    }
                } else {
                    let mut requests = BTreeSet::new();
                    requests.insert(request.clone());
                    HandlerResult::Suspend {
                        requests,
                        reason: SuspensionReason::MissingInterpretation,
                        trace_fragment: format!("need-interpret:{family}"),
                    }
                }
            }
            _ => self.refuse(request),
        }
    }

    fn handle_law(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedApplicableLaw { issue, candidates } => {
                if let Some(selected) = recorded_law(&self.record, issue) {
                    return HandlerResult::Resume {
                        value: selected,
                        trace_fragment: format!("applicable-law:{issue}"),
                    };
                }
                if candidates.len() == 1 {
                    return HandlerResult::Resume {
                        value: Value::String(candidates[0].clone()),
                        trace_fragment: format!("applicable-law-unique:{}", candidates[0]),
                    };
                }
                let mut requests = BTreeSet::new();
                requests.insert(request.clone());
                HandlerResult::Suspend {
                    requests,
                    reason: SuspensionReason::OpenBranch,
                    trace_fragment: "need-applicable-law".into(),
                }
            }
            _ => self.refuse(request),
        }
    }

    fn handle_conflict(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedConflict { graph, doctrines } => {
                if let Some(choice) = recorded_conflict(&self.record, graph, doctrines) {
                    return HandlerResult::Resume {
                        value: Value::String(choice.clone()),
                        trace_fragment: format!("conflict-resolution:{choice}"),
                    };
                }
                match resolve_conflict(doctrines, graph) {
                    Ok(name) => HandlerResult::Resume {
                        value: Value::String(name.clone()),
                        trace_fragment: format!("conflict-unique:{name}"),
                    },
                    Err(_) => {
                        let mut requests = BTreeSet::new();
                        requests.insert(request.clone());
                        HandlerResult::Suspend {
                            requests,
                            reason: SuspensionReason::OpenBranch,
                            trace_fragment: "need-conflict".into(),
                        }
                    }
                }
            }
            _ => self.refuse(request),
        }
    }
}

impl CaseFile {
    fn refuse(&self, request: &OpenRequest) -> HandlerResult {
        let mut requests = BTreeSet::new();
        requests.insert(request.clone());
        HandlerResult::Suspend {
            requests,
            reason: SuspensionReason::OpenBranch,
            trace_fragment: "refuse".into(),
        }
    }
}

fn recorded_conflict(
    record: &CaseRecord,
    graph: &[String],
    doctrines: &[String],
) -> Option<String> {
    let mut found = BTreeSet::new();
    for key in conflict_record_keys(graph, doctrines) {
        if let Some(choice) = record.decisions.get(&key) {
            found.insert(choice.clone());
        }
        if let Some(value) = record.facts.get(&key) {
            found.insert(value.display_label());
        }
    }
    let mut found = found.into_iter();
    match (found.next(), found.next()) {
        (Some(name), None) => Some(name),
        _ => None,
    }
}

fn conflict_record_keys(graph: &[String], doctrines: &[String]) -> Vec<String> {
    let mut keys = Vec::new();
    for node in graph {
        keys.push(format!("conflict:{node}"));
    }
    for doctrine in doctrines {
        keys.push(format!("conflict:{doctrine}"));
    }
    if !graph.is_empty() {
        keys.push(format!("conflict:{}", graph.join("|")));
    }
    keys.push("conflict".into());
    keys.push("ConflictResolution".into());
    keys
}

fn recorded_law(record: &CaseRecord, issue: &str) -> Option<Value> {
    for key in [
        format!("applicable_law:{issue}"),
        format!("law:{issue}"),
        "applicable_law".into(),
    ] {
        if let Some(choice) = record.decisions.get(&key) {
            return Some(Value::String(choice.clone()));
        }
        if let Some(value) = record.facts.get(&key) {
            return Some(value.clone());
        }
    }
    None
}

pub struct Scenario {
    pub assumptions: BTreeMap<String, Value>,
}

impl Handler for Scenario {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
        self.resume_assumption(request, "observe")
    }
    fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
        self.resume_assumption(request, "determine")
    }
    fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
        self.resume_assumption(request, "choose")
    }
    fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
        self.resume_assumption(request, "interpret")
    }
}

impl Scenario {
    fn resume_assumption(&self, request: &OpenRequest, tag: &str) -> HandlerResult {
        let key = format!("{request:?}");
        if let Some(v) = self
            .assumptions
            .get(&key)
            .or_else(|| self.assumptions.get(tag))
        {
            HandlerResult::Resume {
                value: v.clone(),
                trace_fragment: format!("hypothetical:{tag}"),
            }
        } else {
            let mut requests = BTreeSet::new();
            requests.insert(request.clone());
            HandlerResult::Suspend {
                requests,
                reason: SuspensionReason::OpenBranch,
                trace_fragment: format!("scenario-open:{tag}"),
            }
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ExplorationBounds {
    pub interpretations: BTreeMap<String, Vec<String>>,
    pub evidence: BTreeMap<String, Vec<String>>,
}

pub struct Explore {
    pub bounds: ExplorationBounds,
    pub branch: BTreeMap<String, String>,
}

impl Handler for Explore {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
        self.branch_or_open(request)
    }
    fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
        self.branch_or_open(request)
    }
    fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
        self.branch_or_open(request)
    }
    fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedInterpretation { family, .. } => {
                if let Some(alt) = self.branch.get(family) {
                    HandlerResult::Resume {
                        value: Value::String(alt.clone()),
                        trace_fragment: format!("explore:{family}:{alt}"),
                    }
                } else if self
                    .bounds
                    .interpretations
                    .get(family)
                    .map(|v| v.is_empty())
                    .unwrap_or(true)
                {
                    let mut requests = BTreeSet::new();
                    requests.insert(request.clone());
                    HandlerResult::Suspend {
                        requests,
                        reason: SuspensionReason::OpenBranch,
                        trace_fragment: format!("open-branch:{family}"),
                    }
                } else {
                    let mut requests = BTreeSet::new();
                    requests.insert(request.clone());
                    HandlerResult::Suspend {
                        requests,
                        reason: SuspensionReason::OpenBranch,
                        trace_fragment: format!("unexplored:{family}"),
                    }
                }
            }
            _ => self.branch_or_open(request),
        }
    }

    fn handle_conflict(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedConflict { graph, doctrines } => {
                if let Some(choice) = self.branch.get("conflict") {
                    if doctrines.is_empty() || doctrines.iter().any(|d| d == choice) {
                        return HandlerResult::Resume {
                            value: Value::String(choice.clone()),
                            trace_fragment: format!("explore-conflict:{choice}"),
                        };
                    }
                    return HandlerResult::Halt {
                        reason: HaltReason::OutsideCompetence {
                            request: request.clone(),
                            reason: "explored doctrine is not in the admissible set".into(),
                        },
                        trace_fragment: "inadmissible-doctrine".into(),
                    };
                }
                match resolve_conflict(doctrines, graph) {
                    Ok(name) => HandlerResult::Resume {
                        value: Value::String(name.clone()),
                        trace_fragment: format!("explore-conflict-unique:{name}"),
                    },
                    Err(_) => self.branch_or_open(request),
                }
            }
            _ => self.branch_or_open(request),
        }
    }

    fn handle_law(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedApplicableLaw { issue, candidates } => {
                if let Some(choice) = self
                    .branch
                    .get(issue)
                    .or_else(|| self.branch.get("applicable_law"))
                {
                    if candidates.is_empty() || candidates.iter().any(|c| c == choice) {
                        return HandlerResult::Resume {
                            value: Value::String(choice.clone()),
                            trace_fragment: format!("explore-law:{issue}:{choice}"),
                        };
                    }
                    return HandlerResult::Halt {
                        reason: HaltReason::OutsideCompetence {
                            request: request.clone(),
                            reason: "explored source is not in the candidate set".into(),
                        },
                        trace_fragment: "inadmissible-law".into(),
                    };
                }
                if candidates.len() == 1 {
                    return HandlerResult::Resume {
                        value: Value::String(candidates[0].clone()),
                        trace_fragment: format!("explore-law-unique:{}", candidates[0]),
                    };
                }
                self.branch_or_open(request)
            }
            _ => self.branch_or_open(request),
        }
    }
}

impl Explore {
    fn branch_or_open(&self, request: &OpenRequest) -> HandlerResult {
        let mut requests = BTreeSet::new();
        requests.insert(request.clone());
        HandlerResult::Suspend {
            requests,
            reason: SuspensionReason::OpenBranch,
            trace_fragment: "explore-open".into(),
        }
    }
}

pub struct Skeptical {
    pub inner: Explore,
}

impl Handler for Skeptical {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
        self.inner.handle_observe(request)
    }
    fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
        self.inner.handle_determine(request)
    }
    fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
        self.inner.handle_choose(request)
    }
    fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
        self.inner.handle_interpret(request)
    }
    fn handle_conflict(&mut self, request: &OpenRequest) -> HandlerResult {
        self.inner.handle_conflict(request)
    }
    fn handle_law(&mut self, request: &OpenRequest) -> HandlerResult {
        self.inner.handle_law(request)
    }
}

/// Conservative aggregation of explored branch outcomes.
pub fn aggregate(branches: Vec<Outcome<Value>>) -> Outcome<Value> {
    if branches.is_empty() {
        return Outcome::Inconsistent {
            core: vec!["empty completion set".into()],
            trace: fidryn_core::TraceId::of(b"empty"),
        };
    }
    if branches
        .iter()
        .all(|b| matches!(b, Outcome::Inconsistent { .. }))
    {
        return branches.into_iter().next().unwrap();
    }
    if let Some(out) = branches
        .iter()
        .find(|b| matches!(b, Outcome::OutsideCompetence { .. }))
    {
        return out.clone();
    }
    if let Some(out) = branches
        .iter()
        .find(|b| matches!(b, Outcome::Suspended { .. }))
    {
        return out.clone();
    }
    if let Some(out) = branches
        .iter()
        .find(|b| matches!(b, Outcome::NormConflict { .. }))
    {
        return out.clone();
    }
    let values: Vec<_> = branches
        .iter()
        .filter_map(|b| match b {
            Outcome::Determinate { value, .. } => Some(value.display_label()),
            _ => None,
        })
        .collect();
    if values.is_empty() {
        return branches.into_iter().next().unwrap();
    }
    let first = values[0].clone();
    if values.iter().all(|v| *v == first) {
        return branches
            .into_iter()
            .find(|b| matches!(b, Outcome::Determinate { .. }))
            .unwrap();
    }
    let mut alternatives = BTreeMap::new();
    for (i, b) in branches.iter().enumerate() {
        if let Outcome::Determinate { value, .. } = b {
            alternatives.insert(format!("B{i}"), value.clone());
        }
    }
    Outcome::Contingent {
        alternatives,
        pivots: BTreeSet::new(),
        trace: fidryn_core::TraceId::of(b"aggregate"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::TraceId;

    #[test]
    fn empty_completions_are_inconsistent() {
        let out = aggregate(vec![]);
        assert!(matches!(out, Outcome::Inconsistent { .. }));
    }

    #[test]
    fn uncertified_open_prevents_determinate() {
        let det = Outcome::Determinate {
            value: Value::Entity("Bob".into()),
            trace: TraceId::of(b"t"),
            convergence_certificate: None,
            ignored_open_issues: BTreeSet::new(),
        };
        let sus = Outcome::Suspended {
            requests: BTreeSet::new(),
            trace: TraceId::of(b"t2"),
        };
        let out = aggregate(vec![det, sus]);
        assert!(matches!(out, Outcome::Suspended { .. }));
    }

    #[test]
    fn casefile_resumes_recorded_conflict_id() {
        let mut record = CaseRecord::default();
        record
            .decisions
            .insert("conflict:LexSpecialis".into(), "LexSpecialis".into());
        let mut h = CaseFile { record };
        let out = h.handle(&OpenRequest::NeedConflict {
            graph: vec!["Follow:LexSpecialis".into(), "Follow:LexPosterior".into()],
            doctrines: vec!["LexSpecialis".into(), "LexPosterior".into()],
        });
        match out {
            HandlerResult::Resume { value, .. } => {
                assert_eq!(value.display_label(), "LexSpecialis");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn casefile_does_not_pick_the_first_of_two_doctrines() {
        let mut h = CaseFile {
            record: CaseRecord::default(),
        };
        let out = h.handle(&OpenRequest::NeedConflict {
            graph: vec!["Follow:A".into(), "Follow:B".into()],
            doctrines: vec!["A".into(), "B".into()],
        });
        assert!(matches!(out, HandlerResult::Suspend { .. }));
    }

    #[test]
    fn explore_branches_a_named_doctrine_and_does_not_pick_order() {
        let mut open = Explore {
            bounds: ExplorationBounds::default(),
            branch: BTreeMap::new(),
        };
        let req = OpenRequest::NeedConflict {
            graph: vec![],
            doctrines: vec!["A".into(), "B".into()],
        };
        assert!(matches!(
            open.handle_conflict(&req),
            HandlerResult::Suspend { .. }
        ));
        open.branch.insert("conflict".into(), "B".into());
        match open.handle_conflict(&req) {
            HandlerResult::Resume { value, .. } => {
                assert_eq!(value.display_label(), "B");
            }
            other => panic!("{other:?}"),
        }
    }
}
