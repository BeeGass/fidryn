//! Finite coverage verification by replay.
//!
//! Proof generation lives in `fidryn-verify` / `fidryn-solve`; this crate
//! only accepts or rejects a covering claim. Closed value-fragment queries
//! (literals, ident overlay, Boolean/`if`/integer ops, `seq`/`require`/
//! `transaction`, field access) are checked by [`eval_fragment`], which
//! does **not** call [`fidryn_eval::evaluate`]. `duty_status` and
//! `require_authority` are the institutional fragment: [`eval_institutional`]
//! uses [`fidryn_core::ir::CoreDuty`] plus [`fidryn_core::FrozenCaseView`] /
//! `surface_duty_state` on the restricted overlay case, with no CaseFile
//! handlers. Observe, UniqueOccupant, `duty_step`, handlers, and other
//! plans still replay each claimed world with the trusted evaluator under
//! [`IsolatedReplay`].
//! Expected completion size is the checked invocation's
//! [`CaseRecord::admissible_completions`] product. This is not a Lean
//! kernel; a general independent checker, Salsa, SMT, and packages remain
//! Remaining.
//!
//! Replay is isolated: [`IsolatedReplay`] wraps a cloned [`CaseFile`] and a
//! fresh [`LegalState::new`]. It never files or publishes. This crate does
//! not import `fidryn-adapt`.

mod completion;
mod duty;
mod pure;

pub use completion::ValidatedCompletionModel;
pub use duty::{eval_institutional, is_institutional_fragment};
pub use pure::{eval_fragment, is_boolean_fragment, is_pure_fragment};

use crate::completion::{CHOICE_NS, EVIDENCE_NS, INTERPRETATION_NS};
use fidryn_core::{
    BranchClaim, CaseRecord, CheckedCertificate, CompletionProofId, CoreModule, CoverageWitness,
    ExecutionMode, Handler, HandlerResult, LegalState, ModuleId, OpenRequest, Outcome, QueryName,
    QueryPlan, ReplayIssuance, RunContext, SourceSnapshotId, SuspensionReason, Term, Value,
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

/// Accept a covering witness after checking each claimed branch.
///
/// Shape completeness is not enough: [`check_branches`] must re-check the
/// query under every admitted assignment. Closed value-fragment terms use
/// [`eval_fragment`] (no handlers). `duty_status` / `require_authority` use
/// [`eval_institutional`] on the overlay case (no CaseFile). Other plans
/// replay with [`evaluate`] under [`IsolatedReplay`]. Then kernel issuance
/// stamps a FiniteReplay certificate (`is_covering()`). The bound program
/// identity must be the module that was replayed.
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
/// Finite coverage verification by replay: each admitted assignment is
/// checked with [`eval_fragment`] when the query is in the closed value
/// fragment, [`eval_institutional`] for `duty_status` / `require_authority`,
/// otherwise with [`evaluate`] under [`IsolatedReplay`]. The caller's
/// `case` is not mutated. Bindings are a restricted overlay of declared
/// completion slots.
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
    if program != module.id || snapshot != module.snapshot {
        return Err("the replayed program and bound program must agree".into());
    }
    let fingerprint = module.content_fingerprint()?;
    check_branches_with_args(module, query, case, ctx, &witness, answer, args)?;
    let sealed_id = CheckedCertificate::covering_claims_id_with_identity(
        program,
        snapshot,
        fingerprint,
        case,
        query,
        ctx.valid_time,
        ctx.record_time,
        constraints,
        answer,
        &witness,
        args,
        ExecutionMode::Operative,
    )?;
    let legacy_id = CheckedCertificate::covering_claims_id_with_args(
        program,
        snapshot,
        case,
        query,
        ctx.valid_time,
        ctx.record_time,
        constraints,
        answer,
        &witness,
        args,
    )?;
    if id != sealed_id && id != legacy_id {
        return Err(format!(
            "completion proof id {} does not match covering claims {}",
            id.hex(),
            sealed_id.hex()
        ));
    }
    CheckedCertificate::issue_finite_replay(
        sealed_id,
        ReplayIssuance {
            program,
            snapshot,
            fingerprint,
            case,
            query,
            valid: ctx.valid_time,
            known: ctx.record_time,
            constraints,
            answer,
            witness: &witness,
            args,
            execution_mode: ExecutionMode::Operative,
        },
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

/// Re-check each claimed world. Shape completeness is not covering.
///
/// Closed value-fragment queries use [`eval_fragment`]. Institutional
/// `duty_status` / `require_authority` use [`eval_institutional`]. Other
/// plans call [`evaluate`]. Rejects duplicate bindings, a determinate
/// value other than the branch answer (fabricated evaluation), and any
/// suspend or engine error.
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
    let model = ValidatedCompletionModel::from_case(base)?;
    let _admitted = model.admit_witness(witness)?;
    for branch in &witness.branches {
        if branch.answer != *claimed || branch.answer != witness.answer {
            return Err("coverage witness branch answer does not match claimed answer".into());
        }
        check_branch_evaluation(module, query, base, ctx, branch, args, &model)?;
    }
    Ok(())
}

/// Replay one claimed world.
///
/// Closed value fragment: [`eval_fragment`] over admitted slots and
/// original facts (no handlers, duty, Observe, or adapt). Institutional
/// fragment: [`eval_institutional`] on the restricted overlay case (no
/// CaseFile). Otherwise a cloned [`CaseRecord`], [`IsolatedReplay`], and
/// [`LegalState::new()`] (never `into_state` on the caller's record) with
/// [`evaluate`]. Replay does not import `fidryn-adapt` and does not publish
/// institutional state.
fn check_branch_evaluation(
    module: &CoreModule,
    query: &QueryName,
    base: &CaseRecord,
    ctx: &RunContext,
    branch: &BranchClaim,
    args: &BTreeMap<String, Value>,
    model: &ValidatedCompletionModel,
) -> Result<(), String> {
    let cloned = model.overlay(base, &branch.bindings, ctx)?;
    if let Some(term) = pure_fragment_term(module, query) {
        let assignment = fragment_assignment(&cloned.facts, &branch.bindings);
        return match eval_fragment(term, &assignment) {
            Ok(value) if value == branch.answer => Ok(()),
            Ok(value) => Err(format!(
                "fabricated evaluation: branch answer {:?} but evaluation produced {value:?}",
                branch.answer
            )),
            Err(err) => Err(format!("not a covering evaluation: {err}")),
        };
    }
    if let Some(term) = institutional_fragment_term(module, query) {
        return match eval_institutional(module, term, &cloned, ctx, args) {
            Ok(value) if value == branch.answer => Ok(()),
            Ok(value) => Err(format!(
                "fabricated evaluation: branch answer {:?} but evaluation produced {value:?}",
                branch.answer
            )),
            Err(err) => Err(format!("not a covering evaluation: {err}")),
        };
    }
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

fn pure_fragment_term<'a>(module: &'a CoreModule, query: &QueryName) -> Option<&'a Term> {
    match &module.query(query.as_str())?.plan {
        QueryPlan::Evaluate(term) if is_pure_fragment(term) => Some(term),
        _ => None,
    }
}

fn institutional_fragment_term<'a>(module: &'a CoreModule, query: &QueryName) -> Option<&'a Term> {
    match &module.query(query.as_str())?.plan {
        QueryPlan::Evaluate(term) if is_institutional_fragment(term) => Some(term),
        _ => None,
    }
}

/// Admitted-slot overlay onto original facts. Bindings never overwrite a
/// fixed fact; undeclared keys are rejected before this map is built.
fn fragment_assignment(
    facts: &BTreeMap<String, Value>,
    bindings: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    let mut env = facts.clone();
    for (key, value) in bindings {
        env.entry(key.clone()).or_insert(value.clone());
        if let Some(name) = strip_slot_name(key)
            && !name.is_empty()
        {
            env.entry(name.to_owned()).or_insert(value.clone());
        }
    }
    env
}

fn strip_slot_name(key: &str) -> Option<&str> {
    key.strip_prefix(INTERPRETATION_NS)
        .or_else(|| key.strip_prefix(CHOICE_NS))
        .or_else(|| key.strip_prefix(EVIDENCE_NS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::ir::{CoreDuty, CoreQuery, QueryPlan};
    use fidryn_core::{
        BinOp, CoreDecl, CoverageMethod, DutyState, DutyStatus, Guard, Instant, Interval,
        JurisdictionId, NodeId, NodeMeta, OriginId, PrimitiveType, SourceManifestId, Term, TraceId,
        Type,
    };
    use fidryn_eval::duty::duty_state_value;
    use fidryn_eval::evaluate;

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
        module_with_plan_decls(plan, Vec::new())
    }

    fn module_with_plan_decls(plan: QueryPlan, declarations: Vec<CoreDecl>) -> CoreModule {
        CoreModule {
            id: ModuleId::of(b"test"),
            name: "Test".into(),
            version: "0.1.0".into(),
            snapshot: SourceSnapshotId::of(b"s"),
            manifest: SourceManifestId::of(b"m"),
            jurisdiction: JurisdictionId::of(b"j"),
            outside_scope: Vec::new(),
            declarations,
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

    fn constant_true_module() -> CoreModule {
        module_with_plan(QueryPlan::Evaluate(Term::Bool(true)))
    }

    fn constant_false_module() -> CoreModule {
        module_with_plan(QueryPlan::Evaluate(Term::Bool(false)))
    }

    fn tautology_module() -> CoreModule {
        constant_true_module()
    }

    fn empty_branch(answer: bool) -> BranchClaim {
        BranchClaim {
            bindings: BTreeMap::new(),
            answer: Value::Bool(answer),
        }
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
        covering_eval_case_ignored(module, case, answer, witness, &BTreeSet::new())
    }

    fn covering_eval_case_ignored(
        module: &CoreModule,
        case: &CaseRecord,
        answer: &Value,
        witness: CoverageWitness,
        constraints: &BTreeSet<OpenRequest>,
    ) -> Result<CheckedCertificate, String> {
        let query = QueryName::from("q");
        let ctx = run_ctx();
        let fingerprint = module.content_fingerprint().expect("fingerprint");
        let id = CheckedCertificate::covering_claims_id_with_identity(
            module.id,
            module.snapshot,
            fingerprint,
            case,
            &query,
            ctx.valid_time,
            ctx.record_time,
            constraints,
            answer,
            &witness,
            &BTreeMap::new(),
            ExecutionMode::Operative,
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
            constraints,
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
        let module = constant_false_module();
        let claimed = Value::Bool(true);
        let witness = covering_witness(true, vec![empty_branch(true)]);
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
        let module = constant_false_module();
        let claimed = Value::Bool(false);
        let witness = covering_witness(false, vec![empty_branch(false), empty_branch(false)]);
        let err = check_branches(
            &module,
            &QueryName::from("q"),
            &CaseRecord::default(),
            &run_ctx(),
            &witness,
            &claimed,
        )
        .expect_err("duplicate worlds");
        assert!(
            err.contains("duplicate") || err.contains("product") || err.contains("undeclared"),
            "{err}"
        );
        let err = covering_eval(&module, &claimed, witness).expect_err("not covering");
        assert!(
            err.contains("duplicate") || err.contains("product") || err.contains("undeclared"),
            "{err}"
        );
    }

    #[test]
    fn test_accept_covering_eval_with_tautology_worlds_returns_covering_certificate() {
        let module = tautology_module();
        let claimed = Value::Bool(true);
        let witness = covering_witness(true, vec![empty_branch(true)]);
        let mut ignored = BTreeSet::new();
        ignored.insert(ignored_issue());
        let cert = covering_eval_case_ignored(
            &module,
            &CaseRecord::default(),
            &claimed,
            witness,
            &ignored,
        )
        .expect("tautology covers");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        assert!(!reject_digest_as_covering(&cert));
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
        let witness = covering_witness(true, vec![empty_branch(true)]);
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
        let witness = covering_witness(true, vec![empty_branch(true)]);
        let mut ignored = BTreeSet::new();
        ignored.insert(ignored_issue());
        let cert = covering_eval_case_ignored(
            &module,
            &CaseRecord::default(),
            &claimed,
            witness,
            &ignored,
        )
        .expect("tautology covers");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        Outcome::determinate(claimed, TraceId::of(b"t"), Some(cert), ignored)
            .expect("finite replay may ignore open issues");
    }

    #[test]
    fn test_caller_supplied_answers_cannot_mint_finite_replay_without_replay() {
        let module = tautology_module();
        let case = CaseRecord::default();
        let ctx = run_ctx();
        let ignored = BTreeSet::new();
        let fabricated = covering_witness(true, vec![empty_branch(true)]);
        let id = CheckedCertificate::covering_claims_id(
            module.id,
            module.snapshot,
            &case,
            &QueryName::from("q"),
            ctx.valid_time,
            ctx.record_time,
            &ignored,
            &Value::Bool(true),
            &fabricated,
        )
        .expect("claim digest");
        let result = CheckedCertificate::verified_covering(
            id,
            module.id,
            module.snapshot,
            &case,
            &QueryName::from("q"),
            ctx.valid_time,
            ctx.record_time,
            &ignored,
            &Value::Bool(true),
            fabricated,
        );
        assert!(
            !result.is_ok_and(|certificate| certificate.is_covering()),
            "shape-only public construction must not produce replay authority"
        );
    }

    #[test]
    fn test_replay_cannot_overwrite_a_fixed_case_fact() {
        let module = module_with_plan(QueryPlan::Evaluate(Term::Ident("fixed".into())));
        let mut case = CaseRecord::default();
        case.facts.insert("fixed".into(), Value::Bool(false));
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "B".into()]);
        let branches = ["A", "B"]
            .into_iter()
            .map(|alternative| {
                let mut bindings = BTreeMap::new();
                bindings.insert("i:I".into(), Value::String(alternative.into()));
                bindings.insert("fixed".into(), Value::Bool(true));
                BranchClaim {
                    bindings,
                    answer: Value::Bool(true),
                }
            })
            .collect();
        let fabricated = covering_witness(true, branches);
        let err = covering_eval_case(&module, &case, &Value::Bool(true), fabricated)
            .expect_err("completion bindings may resolve declared slots, not rewrite fixed facts");
        assert!(
            err.contains("undeclared") || err.contains("fixed") || err.contains("fabricated"),
            "{err}"
        );
    }

    #[test]
    fn test_matching_branch_count_does_not_replace_domain_membership() {
        let module = tautology_module();
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "B".into()]);
        let fabricated = covering_witness(
            true,
            vec![
                BranchClaim {
                    bindings: BTreeMap::from([("noise".into(), Value::Int(0))]),
                    answer: Value::Bool(true),
                },
                BranchClaim {
                    bindings: BTreeMap::from([("noise".into(), Value::Int(1))]),
                    answer: Value::Bool(true),
                },
            ],
        );
        let err = covering_eval_case(&module, &case, &Value::Bool(true), fabricated)
            .expect_err("exact admitted assignment coverage is required");
        assert!(
            err.contains("undeclared") || err.contains("missing") || err.contains("membership"),
            "{err}"
        );
    }

    #[test]
    fn test_replay_rejects_a_different_program_identity() {
        let module = tautology_module();
        let case = CaseRecord::default();
        let ignored = BTreeSet::new();
        let actual = covering_witness(true, vec![empty_branch(true)]);
        let wrong_program = ModuleId::of(b"different-program");
        let ctx = run_ctx();
        let id = CheckedCertificate::covering_claims_id(
            wrong_program,
            module.snapshot,
            &case,
            &QueryName::from("q"),
            ctx.valid_time,
            ctx.record_time,
            &ignored,
            &Value::Bool(true),
            &actual,
        )
        .expect("claim digest");
        let err = accept_covering_eval(
            id,
            wrong_program,
            module.snapshot,
            &case,
            &QueryName::from("q"),
            &module,
            &ctx,
            &ignored,
            &Value::Bool(true),
            actual,
        )
        .expect_err("the replayed program and bound program must agree");
        assert!(err.contains("program") || err.contains("agree"), "{err}");
    }

    #[test]
    fn test_a_real_certificate_for_true_cannot_certify_false() {
        let module = tautology_module();
        let case = CaseRecord::default();
        let ignored = BTreeSet::from([OpenRequest::NeedInterpretation {
            source: "Review".into(),
            family: "Unneeded".into(),
        }]);
        let actual = covering_witness(true, vec![empty_branch(true)]);
        let certificate =
            covering_eval_case_ignored(&module, &case, &Value::Bool(true), actual, &ignored)
                .expect("a valid certificate for a constant true query");
        let err = Outcome::determinate(
            Value::Bool(false),
            TraceId::of(b"review"),
            Some(certificate),
            ignored,
        )
        .expect_err("certificate consumption must check the certified answer");
        assert!(err.contains("answer"), "{err}");
    }

    #[test]
    fn test_replay_rejects_same_module_id_with_different_fingerprint() {
        let module = tautology_module();
        let mut other = tautology_module();
        other.nominations.push(fidryn_core::ir::CoreNomination {
            candidate: "Alice".into(),
            office: "Trustee".into(),
            rank: 1,
        });
        assert_eq!(module.id, other.id);
        assert_ne!(
            module.content_fingerprint().unwrap(),
            other.content_fingerprint().unwrap()
        );
        let case = CaseRecord::default();
        let claimed = Value::Bool(true);
        let witness = covering_witness(true, vec![empty_branch(true)]);
        let ctx = run_ctx();
        let query = QueryName::from("q");
        let constraints = BTreeSet::new();
        let id = CheckedCertificate::covering_claims_id_with_identity(
            module.id,
            module.snapshot,
            module.content_fingerprint().unwrap(),
            &case,
            &query,
            ctx.valid_time,
            ctx.record_time,
            &constraints,
            &claimed,
            &witness,
            &BTreeMap::new(),
            ExecutionMode::Operative,
        )
        .expect("id for original fingerprint");
        let err = accept_covering_eval(
            id,
            other.id,
            other.snapshot,
            &case,
            &query,
            &other,
            &ctx,
            &constraints,
            &claimed,
            witness,
        )
        .expect_err("fingerprint must match the replayed module");
        assert!(
            err.contains("does not match")
                || err.contains("fingerprint")
                || err.contains("program"),
            "{err}"
        );
    }

    #[test]
    fn test_undeclared_fact_bindings_are_not_completion_worlds() {
        let module = tautology_module();
        let claimed = Value::Bool(true);
        let witness = covering_witness(
            true,
            vec![bool_branch(false, true), bool_branch(true, true)],
        );
        let err = covering_eval(&module, &claimed, witness)
            .expect_err("undeclared fact keys are not admitted assignments");
        assert!(
            err.contains("undeclared") || err.contains("product"),
            "{err}"
        );
    }

    #[test]
    fn test_admitted_slots_without_fact_overwrite_cannot_claim_true_for_fixed_false() {
        let module = module_with_plan(QueryPlan::Evaluate(Term::Ident("fixed".into())));
        let mut case = CaseRecord::default();
        case.facts.insert("fixed".into(), Value::Bool(false));
        case.admissible_completions
            .interpretations
            .insert("I".into(), vec!["A".into(), "B".into()]);
        let branches = ["A", "B"]
            .into_iter()
            .map(|alternative| BranchClaim {
                bindings: BTreeMap::from([("i:I".into(), Value::String(alternative.into()))]),
                answer: Value::Bool(true),
            })
            .collect();
        let witness = covering_witness(true, branches);
        let err = covering_eval_case(&module, &case, &Value::Bool(true), witness)
            .expect_err("fixed fact stays false");
        assert!(
            err.contains("fabricated") || err.contains("covering"),
            "{err}"
        );
    }

    fn or_not_b() -> Term {
        Term::Binary {
            op: BinOp::Or,
            left: Box::new(Term::Ident("b".into())),
            right: Box::new(Term::Apply {
                ctor: "not".into(),
                args: vec![Term::Ident("b".into())],
            }),
        }
    }

    fn declared_b_case() -> CaseRecord {
        let mut case = CaseRecord::default();
        case.admissible_completions
            .interpretations
            .insert("b".into(), vec!["true".into(), "false".into()]);
        case
    }

    fn b_worlds(answer: bool) -> Vec<BranchClaim> {
        ["true", "false"]
            .into_iter()
            .map(|label| BranchClaim {
                bindings: BTreeMap::from([("i:b".into(), Value::String(label.into()))]),
                answer: Value::Bool(answer),
            })
            .collect()
    }

    #[test]
    fn test_accept_covering_eval_with_or_not_tautology_uses_fragment() {
        let module = module_with_plan(QueryPlan::Evaluate(or_not_b()));
        match &module.queries[0].plan {
            QueryPlan::Evaluate(term) => assert!(is_boolean_fragment(term)),
            other => panic!("expected Evaluate plan, got {other:?}"),
        }
        let case = declared_b_case();
        let claimed = Value::Bool(true);
        let witness = covering_witness(true, b_worlds(true));
        let cert = covering_eval_case(&module, &case, &claimed, witness)
            .expect("b || !b covers both declared interpretations");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
    }

    #[test]
    fn test_fragment_rejects_fabricated_false_world_claiming_true() {
        let module = constant_false_module();
        match &module.queries[0].plan {
            QueryPlan::Evaluate(term) => assert!(is_boolean_fragment(term)),
            other => panic!("expected Evaluate plan, got {other:?}"),
        }
        let claimed = Value::Bool(true);
        let witness = covering_witness(true, vec![empty_branch(true)]);
        let err = covering_eval(&module, &claimed, witness)
            .expect_err("fragment checker must reject false → true");
        assert!(err.contains("fabricated"), "{err}");
    }

    #[test]
    fn test_fragment_rejects_fabricated_false_for_or_not_tautology() {
        let module = module_with_plan(QueryPlan::Evaluate(or_not_b()));
        let case = declared_b_case();
        let claimed = Value::Bool(false);
        let witness = covering_witness(false, b_worlds(false));
        let err = covering_eval_case(&module, &case, &claimed, witness)
            .expect_err("b || !b is not false");
        assert!(err.contains("fabricated"), "{err}");
    }

    fn covering_value(answer: Value) -> CoverageWitness {
        CoverageWitness {
            examined: 1,
            total: 1,
            incomplete: false,
            answer: answer.clone(),
            branches: vec![BranchClaim {
                bindings: BTreeMap::new(),
                answer,
            }],
        }
    }

    fn require_term(cond: Term) -> Term {
        Term::Apply {
            ctor: "require".into(),
            args: vec![cond],
        }
    }

    fn seq_term(args: Vec<Term>) -> Term {
        Term::Apply {
            ctor: "seq".into(),
            args,
        }
    }

    fn duty_step_term() -> Term {
        Term::Apply {
            ctor: "duty_step".into(),
            args: vec![Term::Ident("pay".into()), Term::Ident("attach".into())],
        }
    }

    fn observe_term() -> Term {
        Term::Apply {
            ctor: "observed".into(),
            args: vec![Term::String("Filing".into())],
        }
    }

    #[test]
    fn test_eval_fragment_is_used_when_evaluate_would_suspend_on_handlers() {
        let module = module_with_plan(QueryPlan::Evaluate(Term::Ident("judgment".into())));
        match &module.queries[0].plan {
            QueryPlan::Evaluate(term) => {
                assert!(is_boolean_fragment(term));
                assert!(is_pure_fragment(term));
            }
            other => panic!("expected Evaluate plan, got {other:?}"),
        }
        let mut case = CaseRecord::default();
        case.admissible_completions
            .choices
            .insert("judgment".into(), vec!["true".into()]);
        let bindings = BTreeMap::from([("c:judgment".into(), Value::String("true".into()))]);
        let claimed = Value::Bool(true);
        let witness = covering_witness(
            true,
            vec![BranchClaim {
                bindings: bindings.clone(),
                answer: claimed.clone(),
            }],
        );

        let model = ValidatedCompletionModel::from_case(&case).expect("completion model");
        let overlaid = model
            .overlay(&case, &bindings, &run_ctx())
            .expect("admitted overlay");
        let mut handler = IsolatedReplay::new(overlaid.clone());
        let eval_result = evaluate(
            &module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &LegalState::new(),
            &run_ctx(),
            &mut handler,
            &overlaid,
        )
        .expect("evaluate runs");
        assert!(
            matches!(eval_result, Outcome::Suspended { .. }),
            "evaluate must suspend on NeedJudgment without a determination, got {eval_result:?}"
        );

        let cert = covering_eval_case(&module, &case, &claimed, witness)
            .expect("fragment checker decides Ident from admitted bindings");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
    }

    #[test]
    fn test_covering_int_add_uses_fragment() {
        let term = Term::Binary {
            op: BinOp::Add,
            left: Box::new(Term::Int(1)),
            right: Box::new(Term::Int(1)),
        };
        assert!(is_pure_fragment(&term));
        assert!(!is_boolean_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        assert!(pure_fragment_term(&module, &QueryName::from("q")).is_some());
        let claimed = Value::Int(2);
        let cert = covering_eval(&module, &claimed, covering_value(claimed.clone()))
            .expect("1 + 1 covers 2 via fragment");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        let err = covering_eval(&module, &Value::Int(3), covering_value(Value::Int(3)))
            .expect_err("1 + 1 is not 3");
        assert!(err.contains("fabricated"), "{err}");
    }

    #[test]
    fn test_covering_if_true_then_three_uses_fragment() {
        let term = Term::If {
            cond: Box::new(Term::Bool(true)),
            then: Box::new(Term::Int(3)),
            else_: Box::new(Term::Int(4)),
        };
        assert!(is_pure_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        let claimed = Value::Int(3);
        let cert = covering_eval(&module, &claimed, covering_value(claimed.clone()))
            .expect("if true then 3 else 4 covers 3 via fragment");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        let err = covering_eval(&module, &Value::Int(4), covering_value(Value::Int(4)))
            .expect_err("taken branch is 3");
        assert!(err.contains("fabricated"), "{err}");
    }

    #[test]
    fn test_covering_seq_require_true_seven_uses_fragment() {
        let term = seq_term(vec![require_term(Term::Bool(true)), Term::Int(7)]);
        assert!(is_pure_fragment(&term));
        assert!(!is_boolean_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        assert!(pure_fragment_term(&module, &QueryName::from("q")).is_some());
        let claimed = Value::Int(7);
        let cert = covering_eval(&module, &claimed, covering_value(claimed.clone()))
            .expect("seq(require true, 7) covers 7 via fragment");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
    }

    #[test]
    fn test_covering_seq_require_false_cannot_claim_seven() {
        let term = seq_term(vec![require_term(Term::Bool(false)), Term::Int(7)]);
        assert!(is_pure_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        let err = covering_eval(&module, &Value::Int(7), covering_value(Value::Int(7)))
            .expect_err("require false is not determinate 7");
        assert!(
            err.contains("requirement failed") || err.contains("covering"),
            "{err}"
        );
    }

    #[test]
    fn test_covering_field_on_assignment_map_uses_fragment() {
        let term = Term::Field {
            base: Box::new(Term::Ident("rec".into())),
            name: "n".into(),
        };
        assert!(is_pure_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        let mut case = CaseRecord::default();
        case.facts.insert(
            "rec".into(),
            Value::Map(BTreeMap::from([("n".into(), Value::Int(9))])),
        );
        let claimed = Value::Int(9);
        let cert = covering_eval_case(&module, &case, &claimed, covering_value(claimed.clone()))
            .expect("field on assignment map covers via fragment");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
    }

    #[test]
    fn test_covering_transaction_of_fragment_steps_uses_fragment() {
        let term = Term::Apply {
            ctor: "transaction".into(),
            args: vec![require_term(Term::Bool(true)), Term::Int(5)],
        };
        assert!(is_pure_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        let claimed = Value::Int(5);
        let cert = covering_eval(&module, &claimed, covering_value(claimed.clone()))
            .expect("transaction of fragment steps covers via fragment");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
    }

    #[test]
    fn test_seq_with_observe_still_replays_with_evaluate() {
        let term = seq_term(vec![observe_term(), Term::Bool(true)]);
        assert!(!is_pure_fragment(&term));
        assert!(!is_boolean_fragment(&term));
        assert!(!is_institutional_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        assert!(pure_fragment_term(&module, &QueryName::from("q")).is_none());
        assert!(institutional_fragment_term(&module, &QueryName::from("q")).is_none());
        let claimed = Value::Bool(true);
        let witness = covering_witness(true, vec![empty_branch(true)]);
        let cert = covering_eval(&module, &claimed, witness)
            .expect("seq with Observe still covers via evaluate");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
    }

    #[test]
    fn test_seq_with_duty_still_replays_with_evaluate() {
        let term = seq_term(vec![duty_step_term(), Term::Bool(true)]);
        assert!(!is_pure_fragment(&term));
        assert!(!is_institutional_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        assert!(pure_fragment_term(&module, &QueryName::from("q")).is_none());
        assert!(institutional_fragment_term(&module, &QueryName::from("q")).is_none());
        let claimed = Value::Bool(true);
        let witness = covering_witness(true, vec![empty_branch(true)]);
        let cert = covering_eval(&module, &claimed, witness)
            .expect("seq with duty_step still covers via evaluate");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
    }

    #[test]
    fn test_unique_occupant_plan_is_not_pure_fragment() {
        let module = module_with_plan(QueryPlan::UniqueOccupant {
            office: Term::Ident("Trustee".into()),
        });
        assert!(pure_fragment_term(&module, &QueryName::from("q")).is_none());
        assert!(institutional_fragment_term(&module, &QueryName::from("q")).is_none());
    }

    fn duty_status_term(name: &str) -> Term {
        Term::Apply {
            ctor: "duty_status".into(),
            args: vec![Term::Ident(name.into())],
        }
    }

    fn pay_invoice_duty() -> CoreDuty {
        CoreDuty {
            id: NodeId::of(b"PayInvoice"),
            name: "PayInvoice".into(),
            bearer: Term::Ident("Payer".into()),
            claimant: Some(Term::Ident("Payee".into())),
            attaches: Guard::Satisfied,
            content: vec![Term::Apply {
                ctor: "due".into(),
                args: vec![Term::Apply {
                    ctor: "after".into(),
                    args: vec![
                        Term::Apply {
                            ctor: "counted_days".into(),
                            args: vec![Term::Int(30)],
                        },
                        Term::Ident("invoice_date".into()),
                    ],
                }],
            }],
            meta: test_meta("PayInvoice"),
        }
    }

    fn pay_invoice_module(term: Term) -> CoreModule {
        module_with_plan_decls(
            QueryPlan::Evaluate(term),
            vec![CoreDecl::Duty(pay_invoice_duty())],
        )
    }

    fn attached_pay_invoice_case() -> CaseRecord {
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(at()));
        case
    }

    fn attached_pay_invoice_state() -> Value {
        duty_state_value(&DutyState {
            name: "PayInvoice".into(),
            status: DutyStatus::Attached,
            breached: false,
            bearer: "Payer".into(),
            claimant: Some("Payee".into()),
            instance: "default".into(),
        })
    }

    fn performed_pay_invoice_state() -> Value {
        duty_state_value(&DutyState {
            name: "PayInvoice".into(),
            status: DutyStatus::Performed,
            breached: false,
            bearer: "Payer".into(),
            claimant: Some("Payee".into()),
            instance: "default".into(),
        })
    }

    fn ungranted_performed_event() -> fidryn_core::LedgerEvent {
        fidryn_core::LedgerEvent {
            kind: "duty".into(),
            valid_time: Interval::always(),
            record_time: at(),
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        }
    }

    #[test]
    fn test_covering_duty_status_pay_invoice_uses_institutional_fragment() {
        let term = duty_status_term("PayInvoice");
        assert!(!is_pure_fragment(&term));
        assert!(!is_boolean_fragment(&term));
        assert!(is_institutional_fragment(&term));
        let module = pay_invoice_module(term);
        assert!(pure_fragment_term(&module, &QueryName::from("q")).is_none());
        assert!(institutional_fragment_term(&module, &QueryName::from("q")).is_some());
        let case = attached_pay_invoice_case();
        let claimed = attached_pay_invoice_state();
        let before = case.clone();
        let cert = covering_eval_case(&module, &case, &claimed, covering_value(claimed.clone()))
            .expect("duty_status(PayInvoice) covers Attached via institutional fragment");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        assert_eq!(case, before);
        let err = covering_eval_case(
            &module,
            &case,
            &performed_pay_invoice_state(),
            covering_value(performed_pay_invoice_state()),
        )
        .expect_err("attached duty is not Performed");
        assert!(
            err.contains("fabricated") || err.contains("covering"),
            "{err}"
        );
    }

    #[test]
    fn test_covering_duty_status_ungranted_performed_is_not_performed() {
        let term = duty_status_term("PayInvoice");
        assert!(is_institutional_fragment(&term));
        let module = pay_invoice_module(term);
        let mut case = attached_pay_invoice_case();
        case.events.push(ungranted_performed_event());
        let performed = performed_pay_invoice_state();
        let err = covering_eval_case(
            &module,
            &case,
            &performed,
            covering_value(performed.clone()),
        )
        .expect_err("ungranted Performed event must not cover as Performed");
        assert!(
            err.contains("fabricated") || err.contains("covering"),
            "{err}"
        );
        let attached = attached_pay_invoice_state();
        let cert = covering_eval_case(&module, &case, &attached, covering_value(attached.clone()))
            .expect("ungranted Performed stays Attached via institutional fragment");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        let overlay = ValidatedCompletionModel::from_case(&case)
            .expect("model")
            .overlay(&case, &BTreeMap::new(), &run_ctx())
            .expect("overlay");
        let value = eval_institutional(
            &module,
            &duty_status_term("PayInvoice"),
            &overlay,
            &run_ctx(),
            &BTreeMap::new(),
        )
        .expect("institutional duty_status");
        assert_eq!(value, attached);
        assert_ne!(value, performed_pay_invoice_state());
    }

    #[test]
    fn test_covering_require_authority_granted_is_unit() {
        let term = Term::Apply {
            ctor: "require_authority".into(),
            args: vec![Term::Ident("release".into())],
        };
        assert!(!is_pure_fragment(&term));
        assert!(is_institutional_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        assert!(institutional_fragment_term(&module, &QueryName::from("q")).is_some());
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![Value::String("release".into())]),
        );
        let claimed = Value::Unit;
        let cert = covering_eval_case(&module, &case, &claimed, covering_value(claimed.clone()))
            .expect("granted require_authority covers Unit");
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
    }

    #[test]
    fn test_covering_require_authority_without_grant_cannot_claim_determinate() {
        let term = Term::Apply {
            ctor: "require_authority".into(),
            args: vec![Term::Ident("release".into())],
        };
        assert!(is_institutional_fragment(&term));
        let module = module_with_plan(QueryPlan::Evaluate(term));
        let err = covering_eval(&module, &Value::Unit, covering_value(Value::Unit))
            .expect_err("missing grant is not determinate covering");
        assert!(
            err.contains("requirement failed")
                || err.contains("covering")
                || err.contains("authority"),
            "{err}"
        );
        let err = covering_eval(
            &module,
            &Value::Bool(true),
            covering_value(Value::Bool(true)),
        )
        .expect_err("missing grant cannot claim true");
        assert!(
            err.contains("requirement failed")
                || err.contains("covering")
                || err.contains("fabricated"),
            "{err}"
        );
    }
}
