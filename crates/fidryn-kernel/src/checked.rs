//! Invocation-bound qualification of evaluation reports.

use crate::check_branches_with_args;
use fidryn_core::{
    CaseRecord, CompletionAdmissionError, CompletionSearchEvidence, CoreModule, CoverageMethod,
    EvaluationReport, ExecutionMode, ExecutionOperation, ExecutionRequest, Outcome, QueryName,
    RunContext, TraceId, ValidatedCompletionModel, Value, VerifiedSourceBundle,
};
use fidryn_eval::{
    ExecutionBudget, evaluate_scenario_traced_with_budget, evaluate_traced_with_budget,
};
use fidryn_handlers::{CaseFile, aggregate_completions};
use std::collections::{BTreeMap, BTreeSet};

const MAX_REPLAY_BRANCHES: usize = 4096;

/// A report whose invocation and result claims were checked.
/// The fields cannot be supplied or changed by an artifact reader.
#[derive(Clone, Debug)]
pub struct CheckedEvaluation {
    report: EvaluationReport,
    exploration_evidence: Option<CompletionSearchEvidence>,
}

impl CheckedEvaluation {
    /// Bind a run report to its actual invocation and replay its result.
    /// Covering claims additionally require exact finite branch replay.
    #[allow(clippy::too_many_arguments)]
    pub fn validate(
        module: &CoreModule,
        query: &QueryName,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
        mode: ExecutionMode,
        source_bundle: Option<&VerifiedSourceBundle>,
        report: EvaluationReport,
    ) -> Result<Self, String> {
        let request = validate_envelope(
            module,
            query,
            case,
            ctx,
            args,
            mode,
            ExecutionOperation::Run,
            source_bundle,
            &report,
        )?;
        let covering = validate_covering(module, query, case, ctx, args, mode, &request, &report)?;
        if !covering {
            let replayed = replay(module, query, case, ctx, args, mode, request.budget)?;
            if report.outcome != replayed {
                return Err(
                    "run report outcome does not match execution of the admitted invocation".into(),
                );
            }
        }
        Ok(Self {
            report,
            exploration_evidence: None,
        })
    }

    /// Replay submitted exploration branches, check exact completion
    /// membership, and reconstruct the reported aggregate. Partial coverage
    /// can qualify only the shared incomplete-search suspension.
    #[allow(clippy::too_many_arguments)]
    pub fn validate_exploration(
        module: &CoreModule,
        query: &QueryName,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
        mode: ExecutionMode,
        source_bundle: Option<&VerifiedSourceBundle>,
        evidence: &CompletionSearchEvidence,
        report: EvaluationReport,
    ) -> Result<Self, String> {
        let request = validate_envelope(
            module,
            query,
            case,
            ctx,
            args,
            mode,
            ExecutionOperation::Explore,
            source_bundle,
            &report,
        )?;
        if request.budget.is_some() {
            return Err("custom exploration budgets require budgeted branch generation".into());
        }
        if evidence.branches.len() > MAX_REPLAY_BRANCHES {
            return Err("exploration evidence exceeds replay branch budget".into());
        }
        let model = match ValidatedCompletionModel::admit(case) {
            Ok(model) => model,
            Err(error) => {
                if evidence.examined != 0
                    || evidence.total != 0
                    || evidence.incomplete
                    || !evidence.branches.is_empty()
                {
                    return Err(
                        "inadmissible completion model cannot have executed branches".into(),
                    );
                }
                let expected = match error {
                    CompletionAdmissionError::EmptyDomain { .. } => Outcome::Inconsistent {
                        core: vec!["empty completion set".into()],
                        trace: TraceId::of(b"empty"),
                    },
                    CompletionAdmissionError::OutsideDomain { request, selection } => {
                        Outcome::OutsideCompetence {
                            request,
                            reason: format!(
                                "recorded selection `{selection}` is not in the admissible set"
                            ),
                            trace: TraceId::of(b"completion-admission"),
                        }
                    }
                    CompletionAdmissionError::InvalidModel(reason) => return Err(reason),
                };
                if semantic_outcome(&report.outcome) != semantic_outcome(&expected) {
                    return Err("exploration report hides a completion admission refusal".into());
                }
                validate_covering(module, query, case, ctx, args, mode, &request, &report)?;
                return Ok(Self {
                    report,
                    exploration_evidence: Some(evidence.clone()),
                });
            }
        };
        let total = model.product_size()?;
        if evidence.total != total
            || evidence.examined != evidence.branches.len()
            || evidence.incomplete != (evidence.examined < total)
        {
            return Err(
                "exploration coverage counters do not match executed declared worlds".into(),
            );
        }
        let assignments = evidence
            .branches
            .iter()
            .map(|branch| &branch.bindings)
            .collect::<Vec<_>>();
        model.admit_assignments(&assignments, !evidence.incomplete)?;
        let mut replayed = Vec::with_capacity(evidence.branches.len());
        for branch in &evidence.branches {
            let branched_case = model.overlay(case, &branch.bindings, ctx)?;
            let actual = replay(module, query, &branched_case, ctx, args, mode, None)?;
            if actual != branch.outcome {
                return Err("exploration branch outcome does not match its admitted world".into());
            }
            let label = fidryn_core::canonical_json(&model.canonical_assignment(&branch.bindings)?)
                .map_err(|error| error.to_string())?;
            replayed.push((label, actual));
        }
        let expected = aggregate_completions(replayed, evidence.incomplete)
            .map_err(|error| error.to_string())?;
        let covering = validate_covering(module, query, case, ctx, args, mode, &request, &report)?;
        let mut claimed = semantic_outcome(&report.outcome);
        if covering
            && let Outcome::Determinate {
                convergence_certificate,
                ignored_open_issues,
                ..
            } = &mut claimed
        {
            *convergence_certificate = None;
            ignored_open_issues.clear();
        }
        if claimed != semantic_outcome(&expected) {
            return Err(
                "exploration report outcome does not match aggregation of replayed worlds".into(),
            );
        }
        Ok(Self {
            report,
            exploration_evidence: Some(evidence.clone()),
        })
    }

    pub fn report(&self) -> &EvaluationReport {
        &self.report
    }

    pub fn exploration_evidence(&self) -> Option<&CompletionSearchEvidence> {
        self.exploration_evidence.as_ref()
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_envelope(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
    args: &BTreeMap<String, Value>,
    mode: ExecutionMode,
    operation: ExecutionOperation,
    source_bundle: Option<&VerifiedSourceBundle>,
    report: &EvaluationReport,
) -> Result<ExecutionRequest, String> {
    if module.query(query.as_str()).is_none() {
        return Err(format!("unknown query `{}`", query.as_str()));
    }
    let mut request = ExecutionRequest::from_eval(module, query.as_str(), args, case, ctx, mode)?;
    request.operation = operation;
    request.budget = report.request.as_ref().and_then(|request| request.budget);
    if report.request.as_ref() != Some(&request) {
        return Err("report belongs to a different or unspecified invocation".into());
    }
    if report.execution_mode != mode {
        return Err("report execution mode does not match invocation".into());
    }
    if mode == ExecutionMode::Operative && !report.assumptions.is_empty() {
        return Err("operative report cannot apply scenario hypotheses".into());
    }
    if mode == ExecutionMode::Scenario
        && report.assumptions != fidryn_eval::duty::scenario_hypotheses(case, ctx)
    {
        return Err("scenario report does not disclose the admitted hypotheses".into());
    }
    let expected_bundle = source_bundle.map(|bundle| bundle.identity().hex());
    if report.source_bundle != expected_bundle {
        return Err("report source bundle does not match admitted artifact".into());
    }
    let expected_trust = source_bundle
        .map(VerifiedSourceBundle::trust_summary)
        .unwrap_or_default();
    if report.trust != expected_trust {
        return Err("report source trust does not match admitted bundle".into());
    }
    let expected_unresolved = match &report.outcome {
        Outcome::Suspended { requests, .. } => requests,
        Outcome::Contingent { pivots, .. } => pivots,
        _ => &BTreeSet::new(),
    };
    if &report.unresolved != expected_unresolved || report.provenance_root != report.outcome.trace()
    {
        return Err("report unresolved issues or provenance root do not match its outcome".into());
    }
    Ok(request)
}

#[allow(clippy::too_many_arguments)]
fn validate_covering(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
    args: &BTreeMap<String, Value>,
    mode: ExecutionMode,
    request: &ExecutionRequest,
    report: &EvaluationReport,
) -> Result<bool, String> {
    if let Outcome::Determinate {
        value,
        convergence_certificate,
        ignored_open_issues,
        ..
    } = &report.outcome
    {
        let covering = convergence_certificate
            .as_ref()
            .is_some_and(|certificate| certificate.is_covering());
        if covering
            || !ignored_open_issues.is_empty()
            || report.verification_method == CoverageMethod::FiniteReplay
        {
            if mode != ExecutionMode::Operative {
                return Err("scenario covering claims require scenario-specific replay".into());
            }
            let certificate = convergence_certificate
                .as_ref()
                .ok_or("covering report has no certificate")?;
            certificate.validate_invocation(request, value, ignored_open_issues)?;
            let witness = report
                .coverage
                .as_ref()
                .ok_or("covering report has no replay witness")?;
            check_branches_with_args(module, query, case, ctx, witness, value, args)?;
            if report.verification_method != CoverageMethod::FiniteReplay {
                return Err("covering report must disclose finite replay verification".into());
            }
            return Ok(true);
        }
    } else if report.verification_method == CoverageMethod::FiniteReplay {
        return Err("finite replay cannot qualify a non-determinate outcome".into());
    }
    if report.coverage.is_some() || report.verification_method != CoverageMethod::None {
        return Err("report has unqualified coverage metadata".into());
    }
    Ok(false)
}

fn replay(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
    args: &BTreeMap<String, Value>,
    mode: ExecutionMode,
    budget: Option<u32>,
) -> Result<Outcome<Value>, String> {
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    let budget = budget
        .map(|operations| ExecutionBudget { operations })
        .unwrap_or_default();
    let session = match mode {
        ExecutionMode::Operative => evaluate_traced_with_budget(
            module,
            query,
            args,
            &state,
            ctx,
            &mut handler,
            case,
            budget,
        ),
        ExecutionMode::Scenario => evaluate_scenario_traced_with_budget(
            module,
            query,
            args,
            &state,
            ctx,
            &mut handler,
            case,
            budget,
        ),
    }
    .map_err(|error| error.to_string())?;
    Ok(session.outcome)
}

fn semantic_outcome(outcome: &Outcome<Value>) -> Outcome<Value> {
    let mut result = outcome.clone();
    match &mut result {
        Outcome::Determinate { trace, .. }
        | Outcome::Contingent { trace, .. }
        | Outcome::Suspended { trace, .. }
        | Outcome::NormConflict { trace, .. }
        | Outcome::OutsideCompetence { trace, .. }
        | Outcome::Inconsistent { trace, .. } => *trace = TraceId::of(b"semantic-outcome"),
    }
    result
}
