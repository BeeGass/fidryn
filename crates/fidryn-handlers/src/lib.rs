//! CaseFile, Scenario, Explore, and Skeptical handlers.

use fidryn_core::case::CaseDetermination;
use fidryn_core::ir::CoreConflictDoctrine;
use fidryn_core::outcome::OpenRequest;
use fidryn_core::patterns::{PropPattern, TermPattern};
use fidryn_core::{
    CaseRecord, EvidenceItem, HaltReason, Handler, HandlerResult, Instant, Outcome, PropTerm,
    SuspensionReason, Term, TraceId, Value,
};
use fidryn_eval::resolve_conflict;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct CaseFile {
    pub record: CaseRecord,
    /// Extra temporal filter for observe. `None` does not filter by time.
    pub known_at: Option<Instant>,
}

impl CaseFile {
    pub fn new(record: CaseRecord) -> Self {
        Self {
            record,
            known_at: None,
        }
    }
}

impl Handler for CaseFile {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
        match request {
            OpenRequest::NeedEvidence { schema, issue } => {
                if let Some(item) =
                    select_evidence(&self.record.evidence, schema, issue, self.known_at)
                {
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
            OpenRequest::NeedJudgment { protocol, issue } => {
                match matching_determination(
                    &self.record.determinations,
                    protocol,
                    issue,
                    self.known_at,
                ) {
                    Some(d) if d.established => HandlerResult::Resume {
                        value: Value::Bool(true),
                        trace_fragment: format!("determine:{protocol}"),
                    },
                    Some(_) => HandlerResult::Resume {
                        value: Value::Bool(false),
                        trace_fragment: format!("determine-rejected:{protocol}"),
                    },
                    None => {
                        let mut requests = BTreeSet::new();
                        requests.insert(request.clone());
                        HandlerResult::Suspend {
                            requests,
                            reason: SuspensionReason::MissingDetermination,
                            trace_fragment: format!("need-judgment:{protocol}"),
                        }
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

fn select_evidence<'a>(
    evidence: &'a [EvidenceItem],
    schema: &str,
    issue: &PropPattern,
    known_at: Option<Instant>,
) -> Option<&'a EvidenceItem> {
    evidence.iter().find(|item| {
        item.schema == schema
            && observed_by_known_at(item.observed_at, known_at)
            && evidence_fits_issue(&item.value, issue)
    })
}

fn observed_by_known_at(observed_at: Instant, known_at: Option<Instant>) -> bool {
    match known_at {
        None => true,
        Some(known) => observed_at <= known,
    }
}

fn evidence_fits_issue(value: &Value, issue: &PropPattern) -> bool {
    let subjects = pattern_subjects(issue);
    if subjects.is_empty() {
        return true;
    }
    subjects
        .iter()
        .any(|subject| evidence_matches_subject(value, subject, issue))
}

fn evidence_matches_subject(value: &Value, subject: &str, issue: &PropPattern) -> bool {
    match value {
        Value::String(name) | Value::Entity(name) => names_eq(name, subject),
        Value::Option(Some(inner)) => evidence_matches_subject(inner, subject, issue),
        Value::Set(items) => items
            .iter()
            .any(|item| evidence_matches_subject(item, subject, issue)),
        Value::Map(_) | Value::Ctor { .. } => designated_subject_matches(value, subject, issue),
        _ => false,
    }
}

fn designated_subject_matches(value: &Value, subject: &str, issue: &PropPattern) -> bool {
    let mut names = BTreeSet::new();
    let mut saw_subject_field = false;
    collect_designated_subject_names(value, issue, &mut names, &mut saw_subject_field);
    saw_subject_field && !names.is_empty() && names.iter().all(|name| names_eq(name, subject))
}

fn collect_designated_subject_names(
    value: &Value,
    issue: &PropPattern,
    names: &mut BTreeSet<String>,
    saw_subject_field: &mut bool,
) {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => {
            for (key, nested) in fields {
                if is_designated_subject_key(key, issue) {
                    *saw_subject_field = true;
                    collect_person_names(nested, names);
                } else {
                    collect_designated_subject_names(nested, issue, names, saw_subject_field);
                }
            }
        }
        Value::Set(items) => {
            for nested in items {
                collect_designated_subject_names(nested, issue, names, saw_subject_field);
            }
        }
        Value::Option(Some(inner)) => {
            collect_designated_subject_names(inner, issue, names, saw_subject_field);
        }
        _ => {}
    }
}

fn collect_person_names(value: &Value, names: &mut BTreeSet<String>) {
    match value {
        Value::String(name) | Value::Entity(name) => {
            names.insert(name.clone());
        }
        Value::Option(Some(inner)) => collect_person_names(inner, names),
        Value::Set(items) => {
            for nested in items {
                collect_person_names(nested, names);
            }
        }
        Value::Map(fields) | Value::Ctor { fields, .. } => {
            for (key, nested) in fields {
                if DESIGNATED_SUBJECT_FIELDS
                    .iter()
                    .any(|field| field.eq_ignore_ascii_case(key))
                {
                    collect_person_names(nested, names);
                }
            }
        }
        _ => {}
    }
}

/// Person slots on evidence maps. Role fields count only when the issue is
/// about that role (`Issuer(A)` may read `issuer`; `Certified(A)` must not).
const DESIGNATED_SUBJECT_FIELDS: &[&str] =
    &["subject", "person", "candidate", "occupant", "holder"];
const ROLE_SUBJECT_FIELDS: &[&str] = &["issuer", "signer", "author", "decider"];

fn is_designated_subject_key(key: &str, issue: &PropPattern) -> bool {
    designated_keys_for_issue(issue)
        .iter()
        .any(|field| field.eq_ignore_ascii_case(key))
}

fn designated_keys_for_issue(issue: &PropPattern) -> Vec<&'static str> {
    let roles: Vec<&'static str> = ROLE_SUBJECT_FIELDS
        .iter()
        .copied()
        .filter(|role| issue_is_about_role(issue, role))
        .collect();
    if roles.is_empty() {
        DESIGNATED_SUBJECT_FIELDS.to_vec()
    } else {
        roles
    }
}

fn issue_is_about_role(issue: &PropPattern, role: &str) -> bool {
    ident_tokens(issue_predicate(issue))
        .iter()
        .any(|token| token_matches_role(token, role))
}

fn issue_predicate(issue: &PropPattern) -> &str {
    match issue {
        PropPattern::Ground(prop) => prop.predicate.as_str(),
        PropPattern::Match { predicate, .. } => predicate.as_str(),
    }
}

fn token_matches_role(token: &str, role: &str) -> bool {
    token.eq_ignore_ascii_case(role)
        || (role.eq_ignore_ascii_case("issuer") && token.eq_ignore_ascii_case("issued"))
}

fn ident_tokens(name: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for c in name.chars() {
        if c == '_' || c == '-' {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current).to_ascii_lowercase());
            }
            continue;
        }
        if c.is_ascii_uppercase() && !current.is_empty() {
            tokens.push(std::mem::take(&mut current).to_ascii_lowercase());
        }
        current.push(c);
    }
    if !current.is_empty() {
        tokens.push(current.to_ascii_lowercase());
    }
    tokens
}

fn names_eq(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

fn pattern_subjects(issue: &PropPattern) -> Vec<String> {
    match issue {
        PropPattern::Ground(prop) => prop.arguments.iter().filter_map(term_subject).collect(),
        PropPattern::Match { arguments, .. } => arguments
            .iter()
            .filter_map(|pattern| match pattern {
                TermPattern::Exact(term) => term_subject(term),
                TermPattern::Bind(_) | TermPattern::Wildcard => None,
            })
            .collect(),
    }
}

fn term_subject(term: &Term) -> Option<String> {
    match term {
        Term::Ident(name) | Term::String(name) => Some(name.clone()),
        Term::Apply { ctor, args } if args.is_empty() => Some(ctor.clone()),
        _ => None,
    }
}

/// Nested mention helper for tests. Observe matching does not use this:
/// an `issuer` field naming A is not evidence that A is the subject.
#[cfg(test)]
fn value_mentions_subject(value: &Value, subject: &str) -> bool {
    match value {
        Value::Entity(name) | Value::String(name) => names_eq(name, subject),
        Value::Prop(prop) => {
            names_eq(&prop.predicate, subject)
                || prop
                    .arguments
                    .iter()
                    .any(|term| term_mentions_subject(term, subject))
        }
        Value::Ctor { name, fields } => {
            names_eq(name, subject) || fields.values().any(|v| value_mentions_subject(v, subject))
        }
        Value::Map(fields) => fields.values().any(|v| value_mentions_subject(v, subject)),
        Value::Set(items) => items.iter().any(|v| value_mentions_subject(v, subject)),
        Value::Option(Some(inner)) => value_mentions_subject(inner, subject),
        Value::ClauseRef { arguments, .. } => {
            arguments.iter().any(|v| value_mentions_subject(v, subject))
        }
        _ => false,
    }
}

#[cfg(test)]
fn term_mentions_subject(term: &Term, subject: &str) -> bool {
    match term {
        Term::Ident(name) | Term::String(name) | Term::Binder(name) => names_eq(name, subject),
        Term::Apply { ctor, args } => {
            names_eq(ctor, subject) || args.iter().any(|term| term_mentions_subject(term, subject))
        }
        Term::Set(items) => items
            .iter()
            .any(|term| term_mentions_subject(term, subject)),
        Term::Record(fields) => fields
            .values()
            .any(|term| term_mentions_subject(term, subject)),
        _ => false,
    }
}

fn matching_determination<'a>(
    determinations: &'a [CaseDetermination],
    protocol: &str,
    issue: &PropTerm,
    known_at: Option<Instant>,
) -> Option<&'a CaseDetermination> {
    determinations.iter().find(|determination| {
        determination.protocol == protocol
            && judgment_issue_matches(&determination.issue, issue)
            && determination_is_known(determination.recorded_at, known_at)
    })
}

/// Missing `recorded_at` stays visible (legacy records). `known_at = None`
/// does not apply a knowledge filter, matching observe.
fn determination_is_known(recorded_at: Option<Instant>, known_at: Option<Instant>) -> bool {
    match (recorded_at, known_at) {
        (_, None) => true,
        (None, Some(_)) => true,
        (Some(recorded), Some(known)) => recorded <= known,
    }
}

fn judgment_issue_matches(recorded: &str, issue: &PropTerm) -> bool {
    compact_issue(recorded) == compact_issue(&format_prop_term(issue))
}

fn format_prop_term(issue: &PropTerm) -> String {
    if issue.arguments.is_empty() {
        return issue.predicate.clone();
    }
    let args = issue
        .arguments
        .iter()
        .map(format_term)
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}({args})", issue.predicate)
}

fn format_term(term: &Term) -> String {
    match term {
        Term::Ident(name) | Term::String(name) | Term::Binder(name) => name.clone(),
        Term::Bool(value) => value.to_string(),
        Term::Int(value) => value.to_string(),
        Term::Decimal(value) => value.to_string(),
        Term::Apply { ctor, args } if args.is_empty() => ctor.clone(),
        Term::Apply { ctor, args } => {
            let nested = args.iter().map(format_term).collect::<Vec<_>>().join(", ");
            format!("{ctor}({nested})")
        }
        Term::Wildcard => "_".into(),
        other => format!("{other:?}"),
    }
}

fn compact_issue(label: &str) -> String {
    label.chars().filter(|c| !c.is_whitespace()).collect()
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
                if let Some(alt) = self.branch.get(family).cloned() {
                    return self.resume_recorded_interpretation(request, family, alt);
                }
                if self
                    .bounds
                    .interpretations
                    .get(family)
                    .is_none_or(Vec::is_empty)
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
    /// Bind `additions` only for families that are not already recorded.
    /// Ordinary interpret must not replace a recorded selection.
    pub fn fill_unset(&mut self, additions: &BTreeMap<String, String>) {
        for (family, alt) in additions {
            self.branch
                .entry(family.clone())
                .or_insert_with(|| alt.clone());
        }
    }

    fn resume_recorded_interpretation(
        &self,
        request: &OpenRequest,
        family: &str,
        alt: String,
    ) -> HandlerResult {
        let admitted = self
            .bounds
            .interpretations
            .get(family)
            .map(|options| options.iter().any(|option| option == &alt))
            .unwrap_or(true);
        if admitted {
            HandlerResult::Resume {
                value: Value::String(alt.clone()),
                trace_fragment: format!("explore:{family}:{alt}"),
            }
        } else {
            HandlerResult::Halt {
                reason: HaltReason::OutsideCompetence {
                    request: request.clone(),
                    reason: "recorded interpretation is not in the admissible set".into(),
                },
                trace_fragment: format!("inadmissible:{family}"),
            }
        }
    }

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
///
/// Determinate answers are compared with full [`Value`] equality. Nested
/// contingents are flattened. Unresolved requests are unioned rather than
/// dropped. Suspended / conflict / competence beat a lone determinate;
/// a nested contingent or unequal values beat determinate.
pub fn aggregate(branches: Vec<Outcome<Value>>) -> Outcome<Value> {
    if branches.is_empty() {
        return Outcome::Inconsistent {
            core: vec!["empty completion set".into()],
            trace: TraceId::of(b"empty"),
        };
    }
    if branches.iter().all(is_inconsistent) {
        return merge_inconsistent(branches);
    }

    let live: Vec<Outcome<Value>> = branches
        .into_iter()
        .filter(|branch| !is_inconsistent(branch))
        .collect();
    if live.len() <= 1 {
        return live
            .into_iter()
            .next()
            .unwrap_or_else(|| Outcome::Inconsistent {
                core: vec!["empty completion set".into()],
                trace: TraceId::of(b"empty"),
            });
    }

    let mut alternatives = BTreeMap::new();
    let mut pivots = BTreeSet::new();
    let mut requests = BTreeSet::new();
    let mut first_determinate = None;
    let mut competence = Vec::new();
    let mut conflicts = Vec::new();
    let mut has_contingent = false;
    let mut has_suspended = false;

    for (index, branch) in live.into_iter().enumerate() {
        match branch {
            Outcome::Determinate {
                value,
                trace,
                convergence_certificate,
                ignored_open_issues,
            } => {
                insert_alternative(&mut alternatives, format!("B{index}"), value.clone());
                if first_determinate.is_none() {
                    first_determinate = Some(Outcome::Determinate {
                        value,
                        trace,
                        convergence_certificate,
                        ignored_open_issues,
                    });
                }
            }
            Outcome::Contingent {
                alternatives: nested,
                pivots: nested_pivots,
                ..
            } => {
                has_contingent = true;
                pivots.extend(nested_pivots);
                for (key, value) in nested {
                    insert_alternative(&mut alternatives, key, value);
                }
            }
            Outcome::Suspended {
                requests: nested, ..
            } => {
                has_suspended = true;
                requests.extend(nested);
            }
            Outcome::OutsideCompetence {
                request,
                reason,
                trace,
            } => {
                requests.insert(request.clone());
                competence.push(Outcome::OutsideCompetence {
                    request,
                    reason,
                    trace,
                });
            }
            Outcome::NormConflict { doctrines, trace } => {
                requests.insert(conflict_request(&doctrines));
                conflicts.push(Outcome::NormConflict { doctrines, trace });
            }
            Outcome::Inconsistent { .. } => {}
        }
    }

    let divergent = unique_values(&alternatives).len() > 1;
    let combined_trace = TraceId::of(b"aggregate");

    if has_contingent || divergent {
        pivots.extend(requests);
        return Outcome::Contingent {
            alternatives,
            pivots,
            trace: combined_trace,
        };
    }
    if has_suspended {
        return Outcome::Suspended {
            requests,
            trace: combined_trace,
        };
    }
    if !conflicts.is_empty() {
        return merge_conflicts(conflicts);
    }
    if competence.len() == 1 {
        return competence.remove(0);
    }
    if !competence.is_empty() {
        return Outcome::Suspended {
            requests,
            trace: combined_trace,
        };
    }
    if let Some(determinate) = first_determinate {
        return determinate;
    }
    Outcome::Inconsistent {
        core: vec!["empty completion set".into()],
        trace: combined_trace,
    }
}

fn is_inconsistent(outcome: &Outcome<Value>) -> bool {
    matches!(outcome, Outcome::Inconsistent { .. })
}

fn merge_inconsistent(branches: Vec<Outcome<Value>>) -> Outcome<Value> {
    let mut core = Vec::new();
    let mut trace = TraceId::of(b"aggregate");
    for (index, branch) in branches.into_iter().enumerate() {
        if let Outcome::Inconsistent {
            core: lines,
            trace: branch_trace,
        } = branch
        {
            if index == 0 {
                trace = branch_trace;
            }
            for line in lines {
                if !core.contains(&line) {
                    core.push(line);
                }
            }
        }
    }
    Outcome::Inconsistent { core, trace }
}

fn merge_conflicts(mut conflicts: Vec<Outcome<Value>>) -> Outcome<Value> {
    if conflicts.len() == 1 {
        return conflicts.remove(0);
    }
    let mut doctrines: Vec<CoreConflictDoctrine> = Vec::new();
    let mut trace = TraceId::of(b"aggregate");
    for (index, conflict) in conflicts.into_iter().enumerate() {
        if let Outcome::NormConflict {
            doctrines: nested,
            trace: branch_trace,
        } = conflict
        {
            if index == 0 {
                trace = branch_trace;
            }
            for doctrine in nested {
                if doctrines
                    .iter()
                    .all(|seen| seen.id != doctrine.id && seen.name != doctrine.name)
                {
                    doctrines.push(doctrine);
                }
            }
        }
    }
    Outcome::NormConflict { doctrines, trace }
}

fn conflict_request(doctrines: &[CoreConflictDoctrine]) -> OpenRequest {
    OpenRequest::NeedConflict {
        graph: Vec::new(),
        doctrines: doctrines
            .iter()
            .map(|doctrine| doctrine.name.clone())
            .collect(),
    }
}

fn insert_alternative(alternatives: &mut BTreeMap<String, Value>, key: String, value: Value) {
    match alternatives.get(&key) {
        None => {
            alternatives.insert(key, value);
        }
        Some(existing) if existing == &value => {}
        Some(_) => {
            let mut n = 0u32;
            loop {
                let candidate = format!("{key}#{n}");
                match alternatives.get(&candidate) {
                    None => {
                        alternatives.insert(candidate, value);
                        return;
                    }
                    Some(existing) if existing == &value => return,
                    Some(_) => n += 1,
                }
            }
        }
    }
}

fn unique_values(alternatives: &BTreeMap<String, Value>) -> Vec<&Value> {
    let mut unique = Vec::new();
    for value in alternatives.values() {
        if !unique.contains(&value) {
            unique.push(value);
        }
    }
    unique
}

#[cfg(test)]
mod tests {
    use super::*;

    fn award(amount: i64) -> Value {
        Value::Ctor {
            name: "Award".into(),
            fields: BTreeMap::from([("amount".into(), Value::Int(amount))]),
        }
    }

    fn determinate(value: Value) -> Outcome<Value> {
        Outcome::Determinate {
            value,
            trace: TraceId::of(b"t"),
            convergence_certificate: None,
            ignored_open_issues: BTreeSet::new(),
        }
    }

    fn instant(text: &str) -> Instant {
        Instant::parse(text).unwrap()
    }

    fn eligible(person: &str) -> PropTerm {
        PropTerm::new("Eligible", vec![Term::Ident(person.into())])
    }

    fn need_eligible(person: &str) -> OpenRequest {
        OpenRequest::NeedJudgment {
            issue: eligible(person),
            protocol: "Eligibility".into(),
        }
    }

    fn certified(person: &str) -> PropPattern {
        PropPattern::Ground(PropTerm::new("Certified", vec![Term::Ident(person.into())]))
    }

    fn need_certified(person: &str) -> OpenRequest {
        OpenRequest::NeedEvidence {
            issue: certified(person),
            schema: "Certificate".into(),
        }
    }

    fn certificate_map(subject: &str, issuer: &str) -> Value {
        Value::Map(BTreeMap::from([
            ("subject".into(), Value::String(subject.into())),
            ("issuer".into(), Value::String(issuer.into())),
        ]))
    }

    #[test]
    fn empty_completions_are_inconsistent() {
        let out = aggregate(vec![]);
        assert!(matches!(out, Outcome::Inconsistent { .. }));
    }

    #[test]
    fn uncertified_open_prevents_determinate() {
        let det = determinate(Value::Entity("Bob".into()));
        let sus = Outcome::Suspended {
            requests: BTreeSet::new(),
            trace: TraceId::of(b"t2"),
        };
        let out = aggregate(vec![det, sus]);
        assert!(matches!(out, Outcome::Suspended { .. }));
    }

    #[test]
    fn same_constructor_different_fields_are_contingent() {
        let out = aggregate(vec![determinate(award(100)), determinate(award(200))]);
        match out {
            Outcome::Contingent { alternatives, .. } => {
                let values: Vec<&Value> = alternatives.values().collect();
                assert!(values.contains(&&award(100)), "{alternatives:?}");
                assert!(values.contains(&&award(200)), "{alternatives:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nested_contingent_is_not_discarded() {
        let mut nested = BTreeMap::new();
        nested.insert("I1".into(), award(100));
        nested.insert("I2".into(), award(200));
        let contingent = Outcome::Contingent {
            alternatives: nested,
            pivots: BTreeSet::new(),
            trace: TraceId::of(b"c"),
        };
        let out = aggregate(vec![determinate(award(100)), contingent]);
        match out {
            Outcome::Contingent { alternatives, .. } => {
                let values: Vec<&Value> = alternatives.values().collect();
                assert!(values.contains(&&award(100)), "{alternatives:?}");
                assert!(values.contains(&&award(200)), "{alternatives:?}");
            }
            other => panic!("nested contingent discarded: {other:?}"),
        }
    }

    #[test]
    fn all_inconsistent_stays_inconsistent() {
        let a = Outcome::Inconsistent {
            core: vec!["a".into()],
            trace: TraceId::of(b"a"),
        };
        let b = Outcome::Inconsistent {
            core: vec!["b".into()],
            trace: TraceId::of(b"b"),
        };
        assert!(matches!(
            aggregate(vec![a, b]),
            Outcome::Inconsistent { .. }
        ));
    }

    #[test]
    fn casefile_resumes_recorded_conflict_id() {
        let mut record = CaseRecord::default();
        record
            .decisions
            .insert("conflict:LexSpecialis".into(), "LexSpecialis".into());
        let mut h = CaseFile::new(record);
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
        let mut h = CaseFile::new(CaseRecord::default());
        let out = h.handle(&OpenRequest::NeedConflict {
            graph: vec!["Follow:A".into(), "Follow:B".into()],
            doctrines: vec!["A".into(), "B".into()],
        });
        assert!(matches!(out, HandlerResult::Suspend { .. }));
    }

    #[test]
    fn future_determinations_are_not_visible_at_an_earlier_known_time() {
        let mut record = CaseRecord::default();
        record.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: true,
            decider: "Reviewer".into(),
            recorded_at: Some(instant("2034-01-01T00:00:00Z")),
        });
        let mut h = CaseFile {
            record,
            known_at: Some(instant("2033-01-01T00:00:00Z")),
        };
        let req = OpenRequest::NeedJudgment {
            issue: PropTerm::new("P", vec![Term::Ident("A".into())]),
            protocol: "P".into(),
        };
        assert!(
            matches!(h.handle_determine(&req), HandlerResult::Suspend { .. }),
            "future knowledge must not establish a past answer"
        );
    }

    #[test]
    fn determination_at_known_time_still_resumes() {
        let known = instant("2033-01-01T00:00:00Z");
        let mut record = CaseRecord::default();
        record.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: true,
            decider: "Reviewer".into(),
            recorded_at: Some(known),
        });
        let mut h = CaseFile {
            record,
            known_at: Some(known),
        };
        let req = OpenRequest::NeedJudgment {
            issue: PropTerm::new("P", vec![Term::Ident("A".into())]),
            protocol: "P".into(),
        };
        match h.handle_determine(&req) {
            HandlerResult::Resume { value, .. } => assert_eq!(value, Value::Bool(true)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn determination_for_alice_does_not_resume_bob() {
        let mut record = CaseRecord::default();
        record.determinations.push(CaseDetermination {
            issue: "Eligible(Alice)".into(),
            protocol: "Eligibility".into(),
            established: true,
            decider: "Court".into(),
            recorded_at: None,
        });
        let mut h = CaseFile::new(record);
        match h.handle_determine(&need_eligible("Bob")) {
            HandlerResult::Suspend { .. } => {}
            other => panic!("Alice determination resumed Bob: {other:?}"),
        }
        match h.handle_determine(&need_eligible("Alice")) {
            HandlerResult::Resume { value, .. } => assert_eq!(value, Value::Bool(true)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn negative_determination_is_not_outside_competence() {
        let mut record = CaseRecord::default();
        record.determinations.push(CaseDetermination {
            issue: "Eligible(Alice)".into(),
            protocol: "Eligibility".into(),
            established: false,
            decider: "Court".into(),
            recorded_at: None,
        });
        let mut h = CaseFile::new(record);
        match h.handle_determine(&need_eligible("Alice")) {
            HandlerResult::Resume { value, .. } => assert_eq!(value, Value::Bool(false)),
            HandlerResult::Halt { reason, .. } => {
                panic!("established:false halted as {reason:?}")
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn observe_matches_schema_subject_and_known_at() {
        let early = instant("2033-01-01T00:00:00Z");
        let late = instant("2034-01-01T00:00:00Z");
        let mut record = CaseRecord::default();
        record.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: Value::Entity("Bob".into()),
            observed_at: early,
        });
        record.evidence.push(EvidenceItem {
            schema: "PhysicianCertificate".into(),
            value: Value::Entity("Alice".into()),
            observed_at: late,
        });
        let issue = PropPattern::Ground(eligible("Alice"));
        let req = OpenRequest::NeedEvidence {
            issue,
            schema: "PhysicianCertificate".into(),
        };

        let mut by_schema = CaseFile::new(record.clone());
        match by_schema.handle_observe(&req) {
            HandlerResult::Resume { value, .. } => assert_eq!(value, Value::Entity("Alice".into())),
            other => panic!("subject mismatch used first schema hit: {other:?}"),
        }

        let mut too_early = CaseFile {
            record: record.clone(),
            known_at: Some(early),
        };
        assert!(
            matches!(
                too_early.handle_observe(&req),
                HandlerResult::Suspend { .. }
            ),
            "future Alice record must not resume at an earlier known_at"
        );

        let mut known = CaseFile {
            record,
            known_at: Some(late),
        };
        match known.handle_observe(&req) {
            HandlerResult::Resume { value, .. } => assert_eq!(value, Value::Entity("Alice".into())),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn certificate_subject_b_issuer_a_does_not_resume_certified_a() {
        let cert = certificate_map("B", "A");
        assert!(
            value_mentions_subject(&cert, "A"),
            "issuer A is a nested mention; observe must not use that predicate"
        );
        assert!(!evidence_fits_issue(&cert, &certified("A")));
        assert!(evidence_fits_issue(&cert, &certified("B")));

        let mut record = CaseRecord::default();
        record.evidence.push(EvidenceItem {
            schema: "Certificate".into(),
            value: cert.clone(),
            observed_at: instant("2033-01-01T00:00:00Z"),
        });
        let mut h = CaseFile::new(record);

        match h.handle_observe(&need_certified("A")) {
            HandlerResult::Suspend { .. } => {}
            other => panic!("issuer A must not resume Certified(A): {other:?}"),
        }
        match h.handle_observe(&need_certified("B")) {
            HandlerResult::Resume { value, .. } => assert_eq!(value, cert),
            other => panic!("subject B should resume Certified(B): {other:?}"),
        }
    }

    #[test]
    fn evidence_without_designated_subject_field_does_not_resume() {
        let mut record = CaseRecord::default();
        record.evidence.push(EvidenceItem {
            schema: "Certificate".into(),
            value: Value::Map(BTreeMap::from([
                ("issuer".into(), Value::String("A".into())),
                ("signer".into(), Value::String("A".into())),
            ])),
            observed_at: instant("2033-01-01T00:00:00Z"),
        });
        let mut h = CaseFile::new(record);
        match h.handle_observe(&need_certified("A")) {
            HandlerResult::Suspend { .. } => {}
            other => panic!("missing subject field must stay unresolved: {other:?}"),
        }
    }

    #[test]
    fn issuer_issue_reads_issuer_field_not_certificate_subject() {
        let cert = certificate_map("B", "A");
        let about_a = PropPattern::Ground(PropTerm::new("Issuer", vec![Term::Ident("A".into())]));
        let about_b = PropPattern::Ground(PropTerm::new("Issuer", vec![Term::Ident("B".into())]));
        assert!(evidence_fits_issue(&cert, &about_a));
        assert!(!evidence_fits_issue(&cert, &about_b));
    }

    #[test]
    fn explore_fill_unset_does_not_replace_recorded() {
        let mut bounds = ExplorationBounds::default();
        bounds.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["I1".into(), "I2".into()],
        );
        bounds
            .interpretations
            .insert("Other".into(), vec!["X".into(), "Y".into()]);
        let mut explore = Explore {
            bounds,
            branch: BTreeMap::from([("SuccessorEligibility".into(), "I1".into())]),
        };
        let mut additions = BTreeMap::new();
        additions.insert("SuccessorEligibility".into(), "I2".into());
        additions.insert("Other".into(), "X".into());
        explore.fill_unset(&additions);
        assert_eq!(
            explore
                .branch
                .get("SuccessorEligibility")
                .map(String::as_str),
            Some("I1")
        );
        assert_eq!(explore.branch.get("Other").map(String::as_str), Some("X"));

        let req = OpenRequest::NeedInterpretation {
            source: "Instrument.clause(\"4.4\")".into(),
            family: "SuccessorEligibility".into(),
        };
        match explore.handle_interpret(&req) {
            HandlerResult::Resume { value, .. } => {
                assert_eq!(value, Value::String("I1".into()));
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            explore
                .branch
                .get("SuccessorEligibility")
                .map(String::as_str),
            Some("I1")
        );
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
