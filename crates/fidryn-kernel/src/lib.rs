//! Finite coverage verification by replay.
//!
//! Proof generation lives in `fidryn-verify` / `fidryn-solve`; this crate
//! only accepts or rejects a covering claim. [`accept_covering_eval`]
//! re-evaluates each claimed world with [`fidryn_eval::evaluate`] (trusted
//! evaluator, not an independent Lean kernel). Expected completion size is
//! the checked invocation's [`CaseRecord::admissible_completions`] product.
//! Independent proof checking, Salsa, SMT, and packages remain Remaining.
//!
//! Replay is isolated: [`IsolatedReplay`] wraps a cloned [`CaseFile`] and a
//! fresh [`LegalState::new`]. It never files or publishes. This crate does
//! not import `fidryn-adapt`.

use fidryn_core::{
    BranchClaim, CaseRecord, CheckedCertificate, CompletionProofId, CoreModule, CoverageWitness,
    Handler, HandlerResult, LegalState, ModuleId, OpenRequest, Outcome, QueryName, RunContext,
    SourceSnapshotId, SuspensionReason, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use std::collections::{BTreeMap, BTreeSet};

/// Isolated replay handler: recorded case responses only.
///
/// Wraps a cloned [`CaseFile`]. Never files, never publishes, and never
/// treats NeedCustom `"file"` / `"publish"` as [`HandlerResult::Resume`].
/// The caller's [`CaseRecord`] is not mutated; construct from a clone.
pub struct IsolatedReplay {
    inner: CaseFile,
}

impl IsolatedReplay {
    pub fn new(case: CaseRecord) -> Self {
        Self {
            inner: CaseFile::new(case),
        }
    }

    fn refuse_live(request: &OpenRequest) -> HandlerResult {
        let mut requests = BTreeSet::new();
        requests.insert(request.clone());
        let fragment = match request {
            OpenRequest::NeedCustom { effect, .. } => {
                format!("isolated-replay:{effect}")
            }
            _ => "isolated-replay".into(),
        };
        HandlerResult::Suspend {
            requests,
            reason: SuspensionReason::OpenBranch,
            trace_fragment: fragment,
        }
    }
}

impl Handler for IsolatedReplay {
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

    fn handle_law(&mut self, request: &OpenRequest) -> HandlerResult {
        self.inner.handle_law(request)
    }

    fn handle_conflict(&mut self, request: &OpenRequest) -> HandlerResult {
        self.inner.handle_conflict(request)
    }

    fn handle_custom(&mut self, request: &OpenRequest) -> HandlerResult {
        Self::refuse_live(request)
    }
}

/// Accept a covering witness by shape and bind a structural certificate.
///
/// Completeness here is `examined == total`, `examined > 0`,
/// `incomplete == false`, a matching answer, and (when the case declares a
/// nonempty completion product) `total` and `branches.len()` equal to that
/// product. This does **not** evaluate branch meaning. The result is not
/// covering (`!is_covering()`). Use [`accept_covering_eval`] when a module
/// and query are available. A claims digest is not covering proof.
#[allow(clippy::too_many_arguments)]
pub fn accept_covering(
    id: CompletionProofId,
    program: ModuleId,
    snapshot: SourceSnapshotId,
    case: &CaseRecord,
    query: &QueryName,
    ctx: &RunContext,
    constraints: &BTreeSet<OpenRequest>,
    answer: &Value,
    witness: CoverageWitness,
) -> Result<CheckedCertificate, String> {
    accept_covering_with_args(
        id,
        program,
        snapshot,
        case,
        query,
        ctx,
        constraints,
        answer,
        witness,
        &BTreeMap::new(),
    )
}

/// [`accept_covering`] with query arguments bound into Structural claims.
#[allow(clippy::too_many_arguments)]
pub fn accept_covering_with_args(
    id: CompletionProofId,
    program: ModuleId,
    snapshot: SourceSnapshotId,
    case: &CaseRecord,
    query: &QueryName,
    ctx: &RunContext,
    constraints: &BTreeSet<OpenRequest>,
    answer: &Value,
    witness: CoverageWitness,
    args: &BTreeMap<String, Value>,
) -> Result<CheckedCertificate, String> {
    admit_witness_shape(case, &witness, answer)?;
    CheckedCertificate::verified_structural_with_args(
        id,
        program,
        snapshot,
        case,
        query,
        ctx.valid_time,
        ctx.record_time,
        constraints,
        answer,
        witness,
        args,
    )
}

/// Accept a covering witness after evaluating each claimed branch.
///
/// Shape completeness is not enough: [`check_branches`] must re-evaluate the
/// query under every binding. Then [`CheckedCertificate::verified_covering`]
/// binds a FiniteReplay certificate (`is_covering()`).
#[allow(clippy::too_many_arguments)]
pub fn accept_covering_eval(
    id: CompletionProofId,
    program: ModuleId,
    snapshot: SourceSnapshotId,
    case: &CaseRecord,
    query: &QueryName,
    module: &CoreModule,
    ctx: &RunContext,
    constraints: &BTreeSet<OpenRequest>,
    answer: &Value,
    witness: CoverageWitness,
) -> Result<CheckedCertificate, String> {
    accept_covering_eval_with_args(
        id,
        program,
        snapshot,
        case,
        query,
        module,
        ctx,
        constraints,
        answer,
        witness,
        &BTreeMap::new(),
    )
}

/// [`accept_covering_eval`] with query arguments bound into FiniteReplay claims.
///
/// Finite coverage verification by replay: each branch is re-evaluated
/// with [`evaluate`] under [`IsolatedReplay`]. The caller's `case` is not
/// mutated.
#[allow(clippy::too_many_arguments)]
pub fn accept_covering_eval_with_args(
    id: CompletionProofId,
    program: ModuleId,
    snapshot: SourceSnapshotId,
    case: &CaseRecord,
    query: &QueryName,
    module: &CoreModule,
    ctx: &RunContext,
    constraints: &BTreeSet<OpenRequest>,
    answer: &Value,
    witness: CoverageWitness,
    args: &BTreeMap<String, Value>,
) -> Result<CheckedCertificate, String> {
    check_branches_with_args(module, query, case, ctx, &witness, answer, args)?;
    CheckedCertificate::verified_covering_with_args(
        id,
        program,
        snapshot,
        case,
        query,
        ctx.valid_time,
        ctx.record_time,
        constraints,
        answer,
        witness,
        args,
    )
}

/// Reject a claims-digest certificate as covering evidence for ignored issues.
pub fn reject_digest_as_covering(cert: &CheckedCertificate) -> bool {
    !cert.is_covering()
}

/// Admit only a complete finite-search witness whose answer matches `answer`.
///
/// Fails when the witness is incomplete, examined is not equal to total,
/// examined is zero, or the witnessed answer differs from the claim.
pub fn witness_is_admissible(witness: &CoverageWitness, answer: &Value) -> Result<(), String> {
    if witness.incomplete {
        return Err("coverage witness is incomplete".into());
    }
    if witness.examined == 0 {
        return Err("coverage witness examined no worlds".into());
    }
    if witness.examined != witness.total {
        return Err(format!(
            "coverage witness examined {} of {} worlds",
            witness.examined, witness.total
        ));
    }
    if &witness.answer != answer {
        return Err("coverage witness answer does not match claimed answer".into());
    }
    Ok(())
}

fn admit_witness_shape(
    case: &CaseRecord,
    witness: &CoverageWitness,
    answer: &Value,
) -> Result<(), String> {
    witness_is_admissible(witness, answer)?;
    witness_matches_declared_space(case, witness)
}

/// Cartesian size of declared interpretation, choice, and evidence domains.
///
/// `None` when no domains are listed: that is not an extra constraint.
/// An empty listed domain contributes size 0.
fn declared_completion_product(case: &CaseRecord) -> Result<Option<usize>, String> {
    let completions = &case.admissible_completions;
    let mut sizes: Vec<usize> = Vec::new();
    sizes.extend(completions.interpretations.values().map(Vec::len));
    sizes.extend(completions.choices.values().map(Vec::len));
    sizes.extend(
        completions
            .evidence
            .values()
            .map(|domain| domain.responses.len()),
    );
    if sizes.is_empty() {
        return Ok(None);
    }
    let product = sizes
        .into_iter()
        .try_fold(1usize, |acc, size| acc.checked_mul(size))
        .ok_or_else(|| "declared admissible completion product overflows usize".to_string())?;
    Ok(Some(product))
}

/// Do not trust a claimed total larger than the declared model.
fn witness_matches_declared_space(
    case: &CaseRecord,
    witness: &CoverageWitness,
) -> Result<(), String> {
    let Some(expected) = declared_completion_product(case)? else {
        return Ok(());
    };
    if witness.total > expected {
        return Err(format!(
            "coverage witness total {} exceeds declared completion product {expected}",
            witness.total
        ));
    }
    if expected > 0 && (witness.total != expected || witness.branches.len() != expected) {
        return Err(format!(
            "coverage witness total {} / branches {} does not match declared completion product {expected}",
            witness.total,
            witness.branches.len()
        ));
    }
    Ok(())
}

/// Re-evaluate each claimed world. Shape completeness is not covering.
///
/// Rejects duplicate bindings, a determinate value other than the branch
/// answer (fabricated evaluation), and any suspend or engine error.
pub fn check_branches(
    module: &CoreModule,
    query: &QueryName,
    base: &CaseRecord,
    ctx: &RunContext,
    witness: &CoverageWitness,
    claimed: &Value,
) -> Result<(), String> {
    check_branches_with_args(module, query, base, ctx, witness, claimed, &BTreeMap::new())
}

#[allow(clippy::too_many_arguments)]
fn check_branches_with_args(
    module: &CoreModule,
    query: &QueryName,
    base: &CaseRecord,
    ctx: &RunContext,
    witness: &CoverageWitness,
    claimed: &Value,
    args: &BTreeMap<String, Value>,
) -> Result<(), String> {
    admit_witness_shape(base, witness, claimed)?;
    if witness.branches.is_empty() {
        return Err("coverage witness has no branches".into());
    }
    if witness.branches.len() != witness.total || witness.branches.len() != witness.examined {
        return Err(format!(
            "coverage witness branches length {} does not match examined {} / total {}",
            witness.branches.len(),
            witness.examined,
            witness.total
        ));
    }
    if has_duplicate_worlds(&witness.branches) {
        return Err("duplicate worlds in coverage witness".into());
    }
    for branch in &witness.branches {
        if branch.answer != *claimed || branch.answer != witness.answer {
            return Err("coverage witness branch answer does not match claimed answer".into());
        }
        check_branch_evaluation(module, query, base, ctx, branch, args)?;
    }
    Ok(())
}

fn has_duplicate_worlds(branches: &[BranchClaim]) -> bool {
    branches.iter().enumerate().any(|(index, branch)| {
        branches[..index]
            .iter()
            .any(|prior| prior.bindings == branch.bindings)
    })
}

/// Replay one claimed world against `evaluate`.
///
/// Finite coverage verification by replay. Isolation: a cloned
/// [`CaseRecord`], [`IsolatedReplay`], and [`LegalState::new()`] (never
/// `into_state` on the caller's record). Replay does not import
/// `fidryn-adapt` and does not publish institutional state.
fn check_branch_evaluation(
    module: &CoreModule,
    query: &QueryName,
    base: &CaseRecord,
    ctx: &RunContext,
    branch: &BranchClaim,
    args: &BTreeMap<String, Value>,
) -> Result<(), String> {
    let cloned = apply_branch_bindings(base, &branch.bindings);
    let mut handler = IsolatedReplay::new(cloned.clone());
    let state = LegalState::new();
    match evaluate(module, query, args, &state, ctx, &mut handler, &cloned) {
        Ok(Outcome::Determinate { value, .. }) => {
            if value != branch.answer {
                return Err(format!(
                    "fabricated evaluation: branch answer {:?} but evaluation produced {value:?}",
                    branch.answer
                ));
            }
            Ok(())
        }
        Ok(Outcome::Suspended { .. }) => {
            Err("not a covering evaluation: evaluation suspended".into())
        }
        Ok(other) => Err(format!("not a covering evaluation: {other:?}")),
        Err(err) => Err(format!("not a covering evaluation: {err}")),
    }
}

fn apply_branch_bindings(base: &CaseRecord, bindings: &BTreeMap<String, Value>) -> CaseRecord {
    let mut case = base.clone();
    for (key, value) in bindings {
        case.facts.insert(key.clone(), value.clone());
        if let Some(family) = key.strip_prefix("i:")
            && !family.is_empty()
        {
            case.interpretations
                .insert(family.to_owned(), binding_label(value));
        }
        if let Some(protocol) = key.strip_prefix("c:")
            && !protocol.is_empty()
        {
            case.decisions
                .insert(protocol.to_owned(), binding_label(value));
        }
    }
    case
}

fn binding_label(value: &Value) -> String {
    match value {
        Value::String(s) | Value::Entity(s) => s.clone(),
        other => other.display_label(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::ir::{CoreQuery, QueryPlan};
    use fidryn_core::{
        CoverageMethod, Instant, Interval, JurisdictionId, NodeId, NodeMeta, OriginId,
        PrimitiveType, SourceManifestId, Term, TraceId, Type,
    };

    fn complete_witness(answer: Value) -> CoverageWitness {
        CoverageWitness::complete(3, answer)
    }

    fn at() -> Instant {
        Instant::parse("2033-01-01T00:00:00Z").expect("instant")
    }

    fn run_ctx() -> RunContext {
        let t = at();
        RunContext::new(t, t)
    }

    fn test_meta(name: &str) -> NodeMeta {
        NodeMeta {
            span: None,
            source: None,
            jurisdiction: JurisdictionId::of(b"j"),
            valid_time: Interval::always(),
            record_time: Interval::always(),
            origin: OriginId::Direct(NodeId::of(name.as_bytes())),
        }
    }

    fn module_with_plan(plan: QueryPlan) -> CoreModule {
        CoreModule {
            id: ModuleId::of(b"test"),
            name: "Test".into(),
            version: "0.1.0".into(),
            snapshot: SourceSnapshotId::of(b"s"),
            manifest: SourceManifestId::of(b"m"),
            jurisdiction: JurisdictionId::of(b"j"),
            outside_scope: Vec::new(),
            declarations: Vec::new(),
            nominations: Vec::new(),
            queries: vec![CoreQuery {
                id: NodeId::of(b"q"),
                name: "q".into(),
                binders: Vec::new(),
                result_type: Type::Primitive(PrimitiveType::Bool),
                effects: BTreeSet::new(),
                automatic: false,
                plan,
                meta: test_meta("q"),
            }],
            verifications: Vec::new(),
            assertions: Vec::new(),
        }
    }

    fn return_b_module() -> CoreModule {
        module_with_plan(QueryPlan::Evaluate(Term::Ident("b".into())))
    }

    fn tautology_module() -> CoreModule {
        module_with_plan(QueryPlan::Evaluate(Term::Apply {
            ctor: "||".into(),
            args: vec![
                Term::Ident("b".into()),
                Term::Apply {
                    ctor: "not".into(),
                    args: vec![Term::Ident("b".into())],
                },
            ],
        }))
    }

    fn bool_branch(b: bool, answer: bool) -> BranchClaim {
        let mut bindings = BTreeMap::new();
        bindings.insert("b".into(), Value::Bool(b));
        BranchClaim {
            bindings,
            answer: Value::Bool(answer),
        }
    }

    fn covering_witness(answer: bool, branches: Vec<BranchClaim>) -> CoverageWitness {
        let n = branches.len();
        CoverageWitness {
            examined: n,
            total: n,
            incomplete: false,
            answer: Value::Bool(answer),
            branches,
        }
    }

    fn claims_digest_certificate(answer: &Value) -> CheckedCertificate {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = at();
        let constraints = BTreeSet::new();
        let id = CheckedCertificate::claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            answer,
        )
        .expect("claims id");
        CheckedCertificate::verified(
            id,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            answer,
        )
        .expect("verified digest")
    }

    fn ignored_issue() -> OpenRequest {
        OpenRequest::NeedChoice {
            protocol: "p".into(),
            options: vec!["a".into(), "b".into()],
        }
    }

    fn covering_shape(
        case: &CaseRecord,
        answer: &Value,
        witness: CoverageWitness,
    ) -> Result<CheckedCertificate, String> {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let query = QueryName::from("q");
        let ctx = run_ctx();
        let constraints = BTreeSet::new();
        let id = CheckedCertificate::structural_claims_id(
            program,
            snapshot,
            case,
            &query,
            ctx.valid_time,
            ctx.record_time,
            &constraints,
            answer,
            &witness,
        )
        .expect("structural claims id");
        accept_covering(
            id,
            program,
            snapshot,
            case,
            &query,
            &ctx,
            &constraints,
            answer,
            witness,
        )
    }

    fn covering_eval(
        module: &CoreModule,
        answer: &Value,
        witness: CoverageWitness,
    ) -> Result<CheckedCertificate, String> {
        covering_eval_case(module, &CaseRecord::default(), answer, witness)
    }

    fn covering_eval_case(
        module: &CoreModule,
        case: &CaseRecord,
        answer: &Value,
        witness: CoverageWitness,
    ) -> Result<CheckedCertificate, String> {
        let query = QueryName::from("q");
        let ctx = run_ctx();
        let constraints = BTreeSet::new();
        let id = CheckedCertificate::covering_claims_id(
            module.id,
            module.snapshot,
            case,
            &query,
            ctx.valid_time,
            ctx.record_time,
            &constraints,
            answer,
            &witness,
        )
        .expect("covering claims id");
        accept_covering_eval(
            id,
            module.id,
            module.snapshot,
            case,
            &query,
            module,
            &ctx,
            &constraints,
            answer,
            witness,
        )
    }

    #[test]
    fn test_witness_is_admissible_with_complete_matching_answer_returns_ok() {
        let answer = Value::Int(7);
        let witness = complete_witness(answer.clone());
        assert_eq!(witness_is_admissible(&witness, &answer), Ok(()));
    }

    #[test]
    fn test_witness_is_admissible_with_incomplete_witness_returns_err() {
        let answer = Value::Int(7);
        let witness = CoverageWitness {
            examined: 3,
            total: 3,
            incomplete: true,
            answer: answer.clone(),
            branches: Vec::new(),
        };
        let err = witness_is_admissible(&witness, &answer).expect_err("incomplete");
        assert!(err.contains("incomplete"), "{err}");
    }

    #[test]
    fn test_witness_is_admissible_with_examined_zero_returns_err() {
        let answer = Value::Int(7);
        let witness = CoverageWitness {
            examined: 0,
            total: 0,
            incomplete: false,
            answer: answer.clone(),
            branches: Vec::new(),
        };
        let err = witness_is_admissible(&witness, &answer).expect_err("examined 0");
        assert!(
            err.contains("no worlds") || err.contains("examined"),
            "{err}"
        );
    }

    #[test]
    fn test_witness_is_admissible_with_examined_not_equal_total_returns_err() {
        let answer = Value::Int(7);
        let witness = CoverageWitness {
            examined: 2,
            total: 3,
            incomplete: false,
            answer: answer.clone(),
            branches: Vec::new(),
        };
        let err = witness_is_admissible(&witness, &answer).expect_err("examined != total");
        assert!(err.contains("examined"), "{err}");
    }

    #[test]
    fn test_witness_is_admissible_with_answer_mismatch_returns_err() {
        let claimed = Value::Int(7);
        let witness = complete_witness(Value::Int(1));
        let err = witness_is_admissible(&witness, &claimed).expect_err("mismatch");
        assert!(err.contains("answer"), "{err}");
    }

    #[test]
    fn test_reject_digest_as_covering_with_claims_digest_returns_true() {
        let answer = Value::Bool(true);
        let cert = claims_digest_certificate(&answer);
        assert!(reject_digest_as_covering(&cert));
    }

    #[test]
    fn test_accept_covering_with_complete_witness_is_not_covering() {
        let case = CaseRecord::default();
        let answer = Value::Int(7);
        let witness = complete_witness(answer.clone());
        let cert = covering_shape(&case, &answer, witness).expect("accept covering");
        assert_eq!(cert.method(), CoverageMethod::Structural);
        assert!(!cert.is_covering());
        assert!(reject_digest_as_covering(&cert));
    }

    #[test]
    fn test_accept_covering_with_ignored_issues_cannot_build_determinate() {
        let case = CaseRecord::default();
        let answer = Value::Int(7);
        let witness = complete_witness(answer.clone());
        let cert = covering_shape(&case, &answer, witness).expect("accept covering");
        assert!(!cert.is_covering());
        let mut ignored = BTreeSet::new();
        ignored.insert(ignored_issue());
        let err = Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), ignored)
            .expect_err("structural is not covering");
        assert!(err.contains("covering"), "{err}");
    }

    #[test]
    fn test_accept_covering_with_digest_id_returns_err() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = at();
        let ctx = RunContext::new(t, t);
        let constraints = BTreeSet::new();
        let answer = Value::Int(7);
        let witness = complete_witness(answer.clone());
        let digest_id = CheckedCertificate::claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
        )
        .expect("claims id");
        let err = accept_covering(
            digest_id,
            program,
            snapshot,
            &case,
            &query,
            &ctx,
            &constraints,
            &answer,
            witness,
        )
        .expect_err("digest is not covering");
        assert!(
            err.contains("does not match") || err.contains("covering") || err.contains("digest"),
            "{err}"
        );
    }

    #[test]
    fn test_check_branches_with_fabricated_false_world_returns_err() {
        let module = return_b_module();
        let claimed = Value::Bool(true);
        let witness = covering_witness(
            true,
            vec![bool_branch(false, true), bool_branch(true, true)],
        );
        let err = check_branches(
            &module,
            &QueryName::from("q"),
            &CaseRecord::default(),
            &run_ctx(),
            &witness,
            &claimed,
        )
        .expect_err("false world is not true");
        assert!(err.contains("fabricated"), "{err}");
        let err = covering_eval(&module, &claimed, witness).expect_err("not covering");
        assert!(err.contains("fabricated"), "{err}");
    }

    #[test]
    fn test_check_branches_with_duplicate_false_worlds_claiming_total_two_returns_err() {
        let module = return_b_module();
        let claimed = Value::Bool(false);
        let witness = covering_witness(
            false,
            vec![bool_branch(false, false), bool_branch(false, false)],
        );
        let err = check_branches(
            &module,
            &QueryName::from("q"),
            &CaseRecord::default(),
            &run_ctx(),
            &witness,
            &claimed,
        )
        .expect_err("duplicate worlds");
        assert!(err.contains("duplicate"), "{err}");
        let err = covering_eval(&module, &claimed, witness).expect_err("not covering");
        assert!(err.contains("duplicate"), "{err}");
    }

    #[test]
    fn test_accept_covering_eval_with_tautology_worlds_returns_covering_certificate() {
        let module = tautology_module();
        let claimed = Value::Bool(true);
        let witness = covering_witness(
            true,
            vec![bool_branch(false, true), bool_branch(true, true)],
        );
        let cert = covering_eval(&module, &claimed, witness).expect("tautology covers");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        assert!(!reject_digest_as_covering(&cert));
        let mut ignored = BTreeSet::new();
        ignored.insert(ignored_issue());
        Outcome::determinate(claimed, TraceId::of(b"t"), Some(cert), ignored)
            .expect("finite replay may ignore open issues");
    }

    #[test]
    fn test_accept_covering_with_two_choice_domain_and_total_one_returns_err() {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .choices
            .insert("protocol".into(), vec!["a".into(), "b".into()]);
        let answer = Value::Bool(true);
        let witness = CoverageWitness {
            examined: 1,
            total: 1,
            incomplete: false,
            answer: answer.clone(),
            branches: vec![BranchClaim {
                bindings: BTreeMap::new(),
                answer: answer.clone(),
            }],
        };
        let err = covering_shape(&case, &answer, witness)
            .expect_err("declared two-choice space is not size 1");
        assert!(
            err.contains("declared") || err.contains("product") || err.contains("total"),
            "{err}"
        );
    }

    #[test]
    fn test_reject_digest_as_covering_with_digest_only_certificate_returns_true() {
        let answer = Value::Bool(true);
        let cert = claims_digest_certificate(&answer);
        assert!(reject_digest_as_covering(&cert));
        assert!(!cert.is_covering());
    }

    #[test]
    fn test_accept_covering_eval_does_not_mutate_original_case_record() {
        let module = tautology_module();
        let mut case = CaseRecord::default();
        case.facts.insert("keep".into(), Value::Int(1));
        case.events.push(fidryn_core::LedgerEvent {
            kind: "evidence".into(),
            valid_time: fidryn_core::Interval::always(),
            record_time: at(),
            payload: Value::Bool(true),
        });
        let before = case.clone();
        let claimed = Value::Bool(true);
        let witness = covering_witness(
            true,
            vec![bool_branch(false, true), bool_branch(true, true)],
        );
        covering_eval_case(&module, &case, &claimed, witness).expect("tautology covers");
        assert_eq!(case, before);
    }

    #[test]
    fn test_kernel_crate_has_no_fidryn_adapt_dependency() {
        let manifest = include_str!("../Cargo.toml");
        assert!(
            !manifest.contains("fidryn-adapt"),
            "fidryn-kernel must not depend on fidryn-adapt:\n{manifest}"
        );
    }

    #[test]
    fn test_isolated_replay_does_not_resume_file_or_publish() {
        let mut handler = IsolatedReplay::new(CaseRecord::default());
        for effect in ["file", "publish"] {
            let request = OpenRequest::NeedCustom {
                effect: effect.into(),
                payload: "{}".into(),
            };
            match handler.handle(&request) {
                HandlerResult::Resume { .. } => {
                    panic!("isolated replay must not resume NeedCustom {effect}")
                }
                HandlerResult::Suspend { .. } | HandlerResult::Halt { .. } => {}
            }
        }
    }

    #[test]
    fn test_structural_accept_covering_cannot_authorize_ignored_issues() {
        let case = CaseRecord::default();
        let answer = Value::Int(7);
        let witness = complete_witness(answer.clone());
        let cert = covering_shape(&case, &answer, witness).expect("accept covering");
        assert_eq!(cert.method(), CoverageMethod::Structural);
        assert!(!cert.is_covering());
        let mut ignored = BTreeSet::new();
        ignored.insert(ignored_issue());
        let err = Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), ignored)
            .expect_err("structural is not covering");
        assert!(err.contains("covering"), "{err}");
    }

    #[test]
    fn test_accept_covering_eval_can_authorize_ignored_issues() {
        let module = tautology_module();
        let claimed = Value::Bool(true);
        let witness = covering_witness(
            true,
            vec![bool_branch(false, true), bool_branch(true, true)],
        );
        let cert = covering_eval(&module, &claimed, witness).expect("tautology covers");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        let mut ignored = BTreeSet::new();
        ignored.insert(ignored_issue());
        Outcome::determinate(claimed, TraceId::of(b"t"), Some(cert), ignored)
            .expect("finite replay may ignore open issues");
    }
}
