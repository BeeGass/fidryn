//! Deterministic worklist evaluator. No partial mutation on Open or Conflict.

pub mod conflict;
pub mod duty;
pub mod law;
mod worklist;

pub use conflict::resolve_conflict;
pub use law::select_applicable_law;
pub use worklist::DerivedWorld;
use worklist::{binder_name, domain_name, parse_prop_issue, term_as_proposition};

use fidryn_core::ir::{
    ClauseSelector, CompareOp, ConflictTarget, CoreClause, CoreConflictDoctrine, CoreDecision,
    CoreDecl, CoreDuty, CoreFunction, CoreModule, NodeMeta, QueryPlan,
};
use fidryn_core::outcome::OpenRequest;
use fidryn_core::patterns::{
    LegalEffectPattern, LegalStatusPattern, LegalSubjectPattern, PropPattern, TermPattern,
};
use fidryn_core::state::{LegalState, Occupancy, StatusMode};
use fidryn_core::time::Interval;
use fidryn_core::types::{Sort, Type};
use fidryn_core::value::{BinOp, PropTerm, Term, Value};
use fidryn_core::{
    Assumption, CaseRecord, ClauseId, EvaluationReport, EvidenceItem, ExecutionMode,
    FrozenCaseView, Guard, HaltReason, Handler, HandlerResult, Instant, JurisdictionId,
    ManifestArtifact, NodeId, OriginId, Outcome, QueryName, RunContext, SourceWeight, TraceId,
};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};

pub use fidryn_core::EngineError;

/// Residual work captured when evaluation suspends.
#[derive(Clone, Debug)]
pub enum Residual {
    Term(Term),
    Plan(QueryPlan),
}

/// Snapshot of a suspended computation so [`resume`] can continue it.
///
/// Same case, program, query, arguments, and clocks: [`resume`] keeps seq
/// frames and bindings and answers the pending request. If the module
/// content fingerprint or query plan later differs, [`resume`] discards the
/// residual and recomputes as a fresh [`evaluate_session`] of the new
/// module/query. If only the case snapshot (or arguments/clocks) differs,
/// [`resume`] rebases the residual: derived facts are recomputed, remembered
/// handler answers and seq progress are discarded, and bindings restart from
/// the invocation arguments so completed requirements are not reused.
///
/// Seq frames are keyed by a stable path of child indices from the outermost
/// `seq`, so skipping a completed nested `seq` does not shift later frames.
/// Nested `seq` under Binary/Call/If skips its completed prefix on a
/// same-identity resume. Transaction rollback restores both bindings and seq
/// frames.
#[derive(Clone, Debug)]
pub struct Continuation {
    pub residual: Residual,
    pub bindings: BTreeMap<String, Value>,
    pub derived: DerivedWorld,
    pub fuel: Option<u32>,
    pub answered: BTreeMap<String, Value>,
    pub completed: Vec<Value>,
    pub seq_index: usize,
    seq_frames: Vec<SeqFrame>,
    case_identity: CaseIdentity,
}

#[derive(Clone, Debug)]
struct SeqFrame {
    /// Child indices from the outermost `seq`. Empty when this `seq` is not
    /// nested under another `seq`.
    path: Vec<usize>,
    index: usize,
    completed: Vec<Value>,
}

#[derive(Clone, Debug)]
struct TxSavepoint {
    bindings: BTreeMap<String, Value>,
    seq_frames: Vec<SeqFrame>,
    seq_path: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct EvalSession {
    pub outcome: Outcome<Value>,
    pub continuation: Option<Continuation>,
    pub bindings: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CaseIdentity {
    facts: BTreeMap<String, Value>,
    evidence: Vec<(String, Instant, Value)>,
    events: Vec<(String, Interval, Instant, Value)>,
    determinations: Vec<(String, String, bool, String, Option<Instant>)>,
    interpretations: BTreeMap<String, String>,
    decisions: BTreeMap<String, String>,
    closures: Vec<(String, bool)>,
    module_name: String,
    module_version: String,
    program_fingerprint: Option<[u8; 32]>,
    query: String,
    args: BTreeMap<String, Value>,
    valid_time: Instant,
    record_time: Instant,
}

impl CaseIdentity {
    fn of(
        module: &CoreModule,
        query: &QueryName,
        args: &BTreeMap<String, Value>,
        ctx: &RunContext,
        case: &CaseRecord,
    ) -> Self {
        Self {
            facts: case.facts.clone(),
            evidence: case
                .evidence
                .iter()
                .map(|e| (e.schema.clone(), e.observed_at, e.value.clone()))
                .collect(),
            events: case
                .events
                .iter()
                .map(|e| {
                    (
                        e.kind.clone(),
                        e.valid_time,
                        e.record_time,
                        e.payload.clone(),
                    )
                })
                .collect(),
            determinations: case
                .determinations
                .iter()
                .map(|d| {
                    (
                        d.issue.clone(),
                        d.protocol.clone(),
                        d.established,
                        d.decider.clone(),
                        d.recorded_at,
                    )
                })
                .collect(),
            interpretations: case.interpretations.clone(),
            decisions: case.decisions.clone(),
            closures: case
                .closures
                .iter()
                .map(|c| (c.domain.clone(), c.closed))
                .collect(),
            module_name: module.name.clone(),
            module_version: module.version.clone(),
            program_fingerprint: module.content_fingerprint().ok(),
            query: query.as_str().to_owned(),
            args: args.clone(),
            valid_time: ctx.valid_time,
            record_time: ctx.record_time,
        }
    }
}

pub fn evaluate<H: Handler>(
    module: &CoreModule,
    query: &QueryName,
    args: &BTreeMap<String, Value>,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
) -> Result<Outcome<Value>, EngineError> {
    Ok(evaluate_session(module, query, args, state, ctx, handler, case)?.outcome)
}

/// Evaluate with `case.assumptions` as a facts overlay. Does not mutate `case.events`.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub fn evaluate_scenario<H: Handler>(
    module: &CoreModule,
    query: &QueryName,
    args: &BTreeMap<String, Value>,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
) -> Result<Outcome<Value>, EngineError> {
    let overlay = duty::scenario_overlay_case(case);
    evaluate(module, query, args, state, ctx, handler, &overlay)
}

/// Wrap an outcome as an [`EvaluationReport`]. Coverage is unset.
pub fn report_from(outcome: Outcome<Value>) -> EvaluationReport {
    EvaluationReport::from_outcome(outcome)
}

/// Wrap an outcome as a scenario [`EvaluationReport`].
pub fn report_from_scenario(
    outcome: Outcome<Value>,
    assumptions: Vec<Assumption>,
) -> EvaluationReport {
    let mut report = EvaluationReport::from_outcome(outcome);
    report.execution_mode = ExecutionMode::Scenario;
    report.assumptions = assumptions;
    report
}

/// Evaluate `query` and, on [`Outcome::Suspended`], capture a [`Continuation`].
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub fn evaluate_session<H: Handler>(
    module: &CoreModule,
    query: &QueryName,
    args: &BTreeMap<String, Value>,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
) -> Result<EvalSession, EngineError> {
    let Some(q) = module.query(query.as_str()) else {
        return Err(EngineError::UnknownQuery(query.as_str().to_owned()));
    };
    let derived = DerivedWorld::compute(module, case, ctx, args)?;
    eval_with_state(
        &q.plan,
        module,
        query,
        args,
        state,
        ctx,
        handler,
        case,
        args.clone(),
        derived,
        None,
        BTreeMap::new(),
        Vec::new(),
    )
}

/// Continue a suspended session after a handler can [`HandlerResult::Resume`].
///
/// Same pinned identity: keep seq frames, bindings, residual, and remembered
/// answers. When the module content fingerprint or query plan differs, do not
/// evaluate the old residual; recompute like a fresh [`evaluate_session`] of
/// the new module/query. When only the case snapshot, arguments, or clocks
/// have changed, rebase the residual with empty seq progress and invocation
/// arguments so completed requirements are evaluated against the new snapshot.
#[allow(clippy::too_many_arguments, clippy::result_large_err)]
pub fn resume<H: Handler>(
    session: EvalSession,
    module: &CoreModule,
    query: &QueryName,
    args: &BTreeMap<String, Value>,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
) -> Result<EvalSession, EngineError> {
    let Some(cont) = session.continuation else {
        return evaluate_session(module, query, args, state, ctx, handler, case);
    };
    if module.query(query.as_str()).is_none() {
        return Err(EngineError::UnknownQuery(query.as_str().to_owned()));
    }
    let identity = CaseIdentity::of(module, query, args, ctx, case);
    if program_or_query_changed(&cont, module, query, &identity) {
        return evaluate_session(module, query, args, state, ctx, handler, case);
    }
    let residual = match &cont.residual {
        Residual::Term(term) => QueryPlan::Evaluate(term.clone()),
        Residual::Plan(plan) => plan.clone(),
    };
    if cont.case_identity != identity {
        let derived = DerivedWorld::compute(module, case, ctx, args)?;
        return eval_with_state(
            &residual,
            module,
            query,
            args,
            state,
            ctx,
            handler,
            case,
            args.clone(),
            derived,
            cont.fuel,
            BTreeMap::new(),
            Vec::new(),
        );
    }
    eval_with_state(
        &residual,
        module,
        query,
        args,
        state,
        ctx,
        handler,
        case,
        cont.bindings,
        cont.derived,
        cont.fuel,
        cont.answered,
        cont.seq_frames,
    )
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
fn eval_with_state<H: Handler>(
    plan: &QueryPlan,
    module: &CoreModule,
    query: &QueryName,
    args: &BTreeMap<String, Value>,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
    bindings: BTreeMap<String, Value>,
    derived: DerivedWorld,
    fuel: Option<u32>,
    answered: BTreeMap<String, Value>,
    seq_frames: Vec<SeqFrame>,
) -> Result<EvalSession, EngineError> {
    let mut handler = RememberingHandler {
        inner: handler,
        answered,
    };
    let identity = CaseIdentity::of(module, query, args, ctx, case);
    match plan {
        QueryPlan::Evaluate(term) => {
            let mut frame = EvalFrame {
                module,
                args,
                bindings,
                ctx,
                handler: &mut handler,
                case,
                derived,
                fuel,
                seq_frames,
                seq_path: Vec::new(),
            };
            let outcome = frame.eval_term(term)?;
            let EvalFrame {
                bindings,
                derived,
                fuel,
                seq_frames,
                ..
            } = frame;
            Ok(session_from(
                outcome,
                Residual::Term(term.clone()),
                bindings,
                derived,
                fuel,
                handler.answered,
                identity,
                seq_frames,
            ))
        }
        other => {
            let outcome =
                eval_specialized_plan(other, module, state, ctx, &mut handler, case, &derived)?;
            Ok(session_from(
                outcome,
                Residual::Plan(other.clone()),
                bindings,
                derived,
                fuel,
                handler.answered,
                identity,
                Vec::new(),
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn session_from(
    outcome: Outcome<Value>,
    residual: Residual,
    bindings: BTreeMap<String, Value>,
    derived: DerivedWorld,
    fuel: Option<u32>,
    answered: BTreeMap<String, Value>,
    identity: CaseIdentity,
    seq_frames: Vec<SeqFrame>,
) -> EvalSession {
    let (seq_index, completed) = seq_progress(&seq_frames);
    let continuation = match &outcome {
        Outcome::Suspended { .. } => Some(Continuation {
            residual,
            bindings: bindings.clone(),
            derived,
            fuel,
            answered,
            completed,
            seq_index,
            seq_frames,
            case_identity: identity,
        }),
        _ => None,
    };
    EvalSession {
        outcome,
        continuation,
        bindings,
    }
}

struct RememberingHandler<'a, H> {
    inner: &'a mut H,
    answered: BTreeMap<String, Value>,
}

fn request_key(request: &OpenRequest) -> String {
    format!("{request:?}")
}

impl<H: Handler> Handler for RememberingHandler<'_, H> {
    fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
        self.remembered_or(request, |this, req| this.inner.handle_observe(req))
    }
    fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
        self.remembered_or(request, |this, req| this.inner.handle_determine(req))
    }
    fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
        self.remembered_or(request, |this, req| this.inner.handle_choose(req))
    }
    fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
        self.remembered_or(request, |this, req| this.inner.handle_interpret(req))
    }
    fn handle_conflict(&mut self, request: &OpenRequest) -> HandlerResult {
        self.remembered_or(request, |this, req| this.inner.handle_conflict(req))
    }
    fn handle_law(&mut self, request: &OpenRequest) -> HandlerResult {
        self.remembered_or(request, |this, req| this.inner.handle_law(req))
    }
    fn handle_custom(&mut self, request: &OpenRequest) -> HandlerResult {
        self.remembered_or(request, |this, req| this.inner.handle_custom(req))
    }
}

impl<H: Handler> RememberingHandler<'_, H> {
    fn remembered_or(
        &mut self,
        request: &OpenRequest,
        call: impl FnOnce(&mut Self, &OpenRequest) -> HandlerResult,
    ) -> HandlerResult {
        let key = request_key(request);
        if let Some(value) = self.answered.get(&key) {
            return HandlerResult::Resume {
                value: value.clone(),
                trace_fragment: format!("resume-answered:{key}"),
            };
        }
        let result = call(self, request);
        if let HandlerResult::Resume { value, .. } = &result {
            self.answered.insert(key, value.clone());
        }
        result
    }
}

fn eval_specialized_plan<H: Handler>(
    plan: &QueryPlan,
    module: &CoreModule,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
    derived: &DerivedWorld,
) -> Result<Outcome<Value>, EngineError> {
    match plan {
        QueryPlan::UniqueOccupant { office } => Ok(eval_unique_occupant(
            module, office, state, ctx, handler, case, derived,
        )),
        QueryPlan::StatusOf {
            status,
            when_present,
            when_closed_absent,
        } => Ok(eval_status_of(
            status,
            when_present,
            when_closed_absent,
            case,
            ctx,
        )),
        QueryPlan::EvaluateClause { clause, .. } => Ok(eval_clause(module, clause, case, handler)),
        QueryPlan::RunDecision {
            decision,
            arguments,
            ..
        } => eval_run_decision(module, decision, arguments, case, handler, ctx, derived),
        QueryPlan::Evaluate(_) => Ok(Outcome::Suspended {
            requests: BTreeSet::new(),
            trace: TraceId::of(b"eval"),
        }),
    }
}

struct EvalFrame<'a, H: Handler> {
    module: &'a CoreModule,
    args: &'a BTreeMap<String, Value>,
    bindings: BTreeMap<String, Value>,
    ctx: &'a RunContext,
    handler: &'a mut H,
    case: &'a CaseRecord,
    derived: DerivedWorld,
    fuel: Option<u32>,
    seq_frames: Vec<SeqFrame>,
    seq_path: Vec<usize>,
}

impl<'a, H: Handler> EvalFrame<'a, H> {
    fn eval_term(&mut self, term: &Term) -> Result<Outcome<Value>, EngineError> {
        if term_is_named(term, "ResolveNormConflict") {
            let (doctrines, graph) = conflict_inputs(term, self.case);
            return Ok(eval_resolve_norm_conflict(doctrines, graph, self.handler));
        }
        if term_is_named(term, "SelectApplicableLaw") {
            let (issue, artifacts) = law_inputs(term, self.case);
            return Ok(eval_select_applicable_law(issue, artifacts, self.handler));
        }
        match term {
            Term::Bool(b) => Ok(determinate(Value::Bool(*b), TraceId::of(b"eval"))),
            Term::Int(i) => Ok(determinate(Value::Int(*i), TraceId::of(b"eval"))),
            Term::Decimal(d) => Ok(determinate(Value::Decimal(*d), TraceId::of(b"eval"))),
            Term::String(s) => Ok(determinate(Value::String(s.clone()), TraceId::of(b"eval"))),
            Term::Instant(i) => Ok(determinate(Value::Instant(*i), TraceId::of(b"eval"))),
            Term::Duration(d) => Ok(determinate(Value::Duration(*d), TraceId::of(b"eval"))),
            Term::Ident(name) => self.eval_ident(name),
            Term::Apply { ctor, args } => self.eval_apply(ctor, args),
            Term::Set(xs) => self.eval_set(xs),
            Term::Record(fields) => self.eval_record(fields),
            Term::Wildcard | Term::Binder(_) => Err(unsupported(format!("term `{term:?}`"))),
            Term::Binary { op, left, right } => self.eval_binary(binop_name(*op), left, right),
            Term::Call { callee, args } => self.eval_call(callee, args),
            Term::If { cond, then, else_ } => self.eval_if(cond, then, else_),
            Term::Field { base, name } => self.eval_field(base, name),
        }
    }

    fn eval_ident(&mut self, name: &str) -> Result<Outcome<Value>, EngineError> {
        if let Some(v) = self.lookup(name) {
            return Ok(determinate(v, TraceId::of(b"eval")));
        }
        if name == "true" {
            return Ok(determinate(Value::Bool(true), TraceId::of(b"eval")));
        }
        if name == "false" {
            return Ok(determinate(Value::Bool(false), TraceId::of(b"eval")));
        }
        if let Ok(i) = name.parse::<i64>() {
            return Ok(determinate(Value::Int(i), TraceId::of(b"eval")));
        }
        if let Ok(d) = name.parse::<Decimal>() {
            return Ok(determinate(Value::Decimal(d), TraceId::of(b"eval")));
        }
        if term_is_named(&Term::Ident(name.into()), "judgment")
            || name.eq_ignore_ascii_case("adjudicated")
        {
            return Ok(eval_judgment(self.handler, self.case));
        }
        if name.eq_ignore_ascii_case("EligibleForOpenTexturedCredit") {
            return Ok(eval_need_determine(
                "EligibleForOpenTexturedCredit",
                "OpenTexturedCredit",
                self.handler,
            ));
        }
        if let Some(function) = find_function(self.module, name).cloned() {
            return self.eval_function(&function, &[]);
        }
        if let Some(effect) = find_effect_name(self.module, name) {
            return Ok(self.eval_effect(&effect, name));
        }
        if is_entity(self.module, name) {
            return Ok(determinate(
                Value::Entity(name.to_owned()),
                TraceId::of(b"eval"),
            ));
        }
        Err(unsupported(format!("unbound identifier `{name}`")))
    }

    fn eval_apply(&mut self, ctor: &str, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if let Some(kind) = quantifier_kind(ctor) {
            return self.eval_quantifier(kind, args);
        }
        if let Some(op) = binop_ctor(ctor)
            && args.len() == 2
        {
            return self.eval_binary(&op, &args[0], &args[1]);
        }
        if is_not(ctor) && args.len() == 1 {
            return self.eval_not(&args[0]);
        }
        if is_if(ctor) && args.len() == 3 {
            return self.eval_if(&args[0], &args[1], &args[2]);
        }
        if (ctor == "field" || ctor == "Field") && args.len() == 2 {
            let name = match &args[1] {
                Term::Ident(n) | Term::String(n) => n.clone(),
                other => return Err(unsupported(format!("field name `{other:?}`"))),
            };
            return self.eval_field(&args[0], &name);
        }
        if is_seq(ctor) {
            return self.eval_seq(args);
        }
        if is_transaction(ctor) {
            return self.eval_transaction(args);
        }
        if is_require(ctor) {
            return self.eval_require(args);
        }
        if is_duty_step(ctor) {
            return self.eval_duty_step(args);
        }
        if is_duty_status(ctor) {
            return self.eval_duty_status(args);
        }
        if is_require_authority(ctor) {
            return self.eval_require_authority(args);
        }
        if ctor.eq_ignore_ascii_case("operative")
            || ctor.eq_ignore_ascii_case("determined")
            || ctor.eq_ignore_ascii_case("assumed")
        {
            return self.eval_modal(ctor, args);
        }
        if ctor.eq_ignore_ascii_case("EligibleForOpenTexturedCredit") {
            return Ok(eval_need_determine(
                "EligibleForOpenTexturedCredit",
                "OpenTexturedCredit",
                self.handler,
            ));
        }
        if ctor.eq_ignore_ascii_case("judgment") || ctor.eq_ignore_ascii_case("Adjudicated") {
            return Ok(eval_judgment(self.handler, self.case));
        }
        if let Some(function) = find_function(self.module, ctor).cloned() {
            return self.eval_function(&function, args);
        }
        if let Some(effect) = find_effect_name(self.module, ctor) {
            return Ok(self.eval_effect(&effect, &custom_effect_payload(&effect, args)));
        }
        self.eval_ctor(ctor, args)
    }

    fn eval_call(&mut self, callee: &str, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if quantifier_kind(callee).is_some()
            || callee.eq_ignore_ascii_case("operative")
            || callee.eq_ignore_ascii_case("determined")
            || callee.eq_ignore_ascii_case("assumed")
            || is_not(callee)
            || is_if(callee)
            || is_seq(callee)
            || is_transaction(callee)
            || is_require(callee)
            || is_duty_step(callee)
            || is_duty_status(callee)
            || is_require_authority(callee)
            || binop_ctor(callee).is_some()
        {
            return self.eval_apply(callee, args);
        }
        if let Some(function) = find_function(self.module, callee) {
            return self.eval_function(function, args);
        }
        if let Some(effect) = find_effect_name(self.module, callee) {
            return Ok(self.eval_effect(&effect, &custom_effect_payload(&effect, args)));
        }
        Err(unsupported(format!("unknown function `{callee}`")))
    }

    fn eval_function(
        &mut self,
        function: &CoreFunction,
        arg_terms: &[Term],
    ) -> Result<Outcome<Value>, EngineError> {
        let budget = match (function.fuel, self.fuel) {
            (Some(0), _) | (_, Some(0)) => {
                return Err(EngineError::FuelExhausted { remaining: 0 });
            }
            (Some(declared), Some(remaining)) => Some(declared.min(remaining)),
            (Some(declared), None) => Some(declared),
            (None, remaining) => remaining,
        };
        if budget == Some(0) {
            return Err(EngineError::FuelExhausted { remaining: 0 });
        }
        if function.params.len() != arg_terms.len() {
            let names: Vec<&str> = function.params.iter().map(|(n, _)| n.as_str()).collect();
            let named = if names.is_empty() {
                String::new()
            } else {
                format!(" ({})", names.join(", "))
            };
            return Err(EngineError::InvalidInput(format!(
                "function `{}` expects {} argument(s){named}, got {}",
                function.name,
                function.params.len(),
                arg_terms.len()
            )));
        }

        let mut call_env = BTreeMap::new();
        for ((name, _), arg) in function.params.iter().zip(arg_terms) {
            let value = match as_determinate(self.eval_term(arg)?) {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            call_env.insert(name.clone(), value);
        }

        let mut nested = EvalFrame {
            module: self.module,
            args: self.args,
            bindings: call_env,
            ctx: self.ctx,
            handler: self.handler,
            case: self.case,
            derived: self.derived.clone(),
            fuel: budget.map(|n| n.saturating_sub(1)),
            seq_frames: std::mem::take(&mut self.seq_frames),
            seq_path: self.seq_path.clone(),
        };
        let result = if let Some(body) = &function.body {
            nested.eval_term(body)
        } else if function.name == "ordinary_income_tax" {
            nested.eval_tax()
        } else {
            Err(unsupported(format!(
                "function `{}` has no executable body",
                function.name
            )))
        };
        self.seq_frames = nested.seq_frames;
        self.seq_path = nested.seq_path;
        result
    }

    fn eval_tax(&mut self) -> Result<Outcome<Value>, EngineError> {
        let amount = money_amount(
            self.bindings
                .get("amount")
                .or_else(|| self.args.get("amount"))
                .or_else(|| self.case.facts.get("amount")),
        )?;
        let tax = ordinary_income_tax(amount);
        Ok(determinate(Value::Decimal(tax), TraceId::of(b"tax_on")))
    }

    fn eval_binary(
        &mut self,
        op: &str,
        left: &Term,
        right: &Term,
    ) -> Result<Outcome<Value>, EngineError> {
        let left = match as_determinate(self.eval_term(left)?) {
            Ok(v) => v,
            Err(outcome) => return Ok(outcome),
        };
        let right = match as_determinate(self.eval_term(right)?) {
            Ok(v) => v,
            Err(outcome) => return Ok(outcome),
        };
        eval_binop(op, &left, &right).map(|v| determinate(v, TraceId::of(b"eval")))
    }

    fn eval_not(&mut self, inner: &Term) -> Result<Outcome<Value>, EngineError> {
        match as_determinate(self.eval_term(inner)?) {
            Ok(Value::Bool(b)) => Ok(determinate(Value::Bool(!b), TraceId::of(b"eval"))),
            Ok(other) => Err(unsupported(format!("not of `{other:?}`"))),
            Err(outcome) => Ok(outcome),
        }
    }

    fn eval_if(
        &mut self,
        cond: &Term,
        then: &Term,
        else_: &Term,
    ) -> Result<Outcome<Value>, EngineError> {
        match as_determinate(self.eval_term(cond)?) {
            Ok(Value::Bool(true)) => self.eval_term(then),
            Ok(Value::Bool(false)) => self.eval_term(else_),
            Ok(other) => Err(unsupported(format!("if condition `{other:?}`"))),
            Err(outcome) => Ok(outcome),
        }
    }

    fn eval_seq(&mut self, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if args.is_empty() {
            return Err(unsupported("empty seq"));
        }
        let path = self.seq_path.clone();
        let frame_i = self.seq_frame_index(&path);
        let start = {
            let frame = &self.seq_frames[frame_i];
            frame.index.max(frame.completed.len())
        };
        if start > args.len() {
            return Err(unsupported("seq resume index past arguments"));
        }
        let mut last = if start > 0 {
            self.seq_frames[frame_i].completed.get(start - 1).cloned()
        } else {
            None
        };
        for (index, arg) in args.iter().enumerate().skip(start) {
            self.seq_path.push(index);
            let step = self.eval_term(arg);
            self.seq_path.pop();
            match as_determinate(step?) {
                Ok(v) => {
                    last = Some(v.clone());
                    let frame = &mut self.seq_frames[frame_i];
                    if frame.completed.len() > index {
                        frame.completed[index] = v;
                    } else {
                        frame.completed.push(v);
                    }
                    frame.index = index + 1;
                }
                Err(outcome) => {
                    self.seq_frames[frame_i].index = index;
                    return Ok(outcome);
                }
            }
        }
        match last {
            Some(value) => Ok(determinate(value, TraceId::of(b"seq"))),
            None => Err(unsupported("empty seq")),
        }
    }

    fn seq_frame_index(&mut self, path: &[usize]) -> usize {
        if let Some(index) = self.seq_frames.iter().position(|frame| frame.path == path) {
            index
        } else {
            self.seq_frames.push(SeqFrame {
                path: path.to_vec(),
                index: 0,
                completed: Vec::new(),
            });
            self.seq_frames.len() - 1
        }
    }

    fn eval_transaction(&mut self, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if args.is_empty() {
            return Err(unsupported("empty transaction"));
        }
        let saved = self.capture_savepoint();
        let mut last = None;
        for step in args {
            match self.eval_term(step) {
                Ok(Outcome::Determinate { value, .. }) => last = Some(value),
                Ok(outcome) => {
                    self.restore_savepoint(&saved);
                    return Ok(outcome);
                }
                Err(err) => {
                    self.restore_savepoint(&saved);
                    return Err(err);
                }
            }
        }
        match last {
            Some(value) => Ok(determinate(value, TraceId::of(b"transaction"))),
            None => {
                self.restore_savepoint(&saved);
                Err(unsupported("empty transaction"))
            }
        }
    }

    fn capture_savepoint(&self) -> TxSavepoint {
        TxSavepoint {
            bindings: self.bindings.clone(),
            seq_frames: self.seq_frames.clone(),
            seq_path: self.seq_path.clone(),
        }
    }

    fn restore_savepoint(&mut self, saved: &TxSavepoint) {
        self.bindings = saved.bindings.clone();
        self.seq_frames = saved.seq_frames.clone();
        self.seq_path = saved.seq_path.clone();
    }

    fn eval_duty_status(&mut self, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if args.is_empty() {
            return Err(unsupported("duty_status expects a duty name"));
        }
        let name = term_literal_name(&args[0])?;
        let instance = if args.len() >= 2 {
            term_literal_name(&args[1])?
        } else {
            duty::DEFAULT_DUTY_INSTANCE.to_owned()
        };
        let Some(duty) = find_duty(self.module, &name).cloned() else {
            return Err(unsupported(format!("unknown duty `{name}`")));
        };
        let attaches_held = self
            .derived
            .is_guard_held(&duty.attaches, self.case, self.ctx);
        let attaches_denied = self
            .derived
            .is_guard_denied(&duty.attaches, self.case, self.ctx);
        let performed_held = self.derived.holds_named("performed")
            || self.derived.holds_named(&format!("{name}_performed"));
        let state = duty::surface_duty_state(
            &duty,
            self.case,
            self.ctx,
            attaches_held,
            attaches_denied,
            performed_held,
            &instance,
        );
        self.bindings.insert(
            duty::duty_instance_key(&name, &instance),
            duty::duty_state_value(&state),
        );
        Ok(determinate(
            duty::duty_state_value(&state),
            TraceId::of(b"duty_status"),
        ))
    }

    fn eval_duty_step(&mut self, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if args.len() < 2 {
            return Err(EngineError::InvalidInput(
                "duty_step expects [name, action]".into(),
            ));
        }
        let name = term_literal_name(&args[0])?;
        let action = term_literal_name(&args[1])?;
        let person = if args.len() >= 3 {
            Some(term_literal_name(&args[2])?)
        } else {
            None
        };
        let instance = if args.len() >= 4 {
            term_literal_name(&args[3])?
        } else {
            duty::DEFAULT_DUTY_INSTANCE.to_owned()
        };
        if duty::has_authority_constraint(self.case)
            && !duty::action_is_granted(self.case, &action, self.ctx.record_time)
        {
            return Ok(authority_missing(&action));
        }
        let key = if args.len() >= 4 {
            duty::duty_instance_key(&name, &instance)
        } else {
            duty::duty_fact_key(&name)
        };
        let current = self
            .lookup(&key)
            .and_then(|value| duty::parse_duty_state(&value, &name));
        let mut next = duty::apply_duty_action(current.as_ref(), &name, &action, person)?;
        next.instance = instance;
        self.bindings.insert(key, duty::duty_state_value(&next));
        Ok(determinate(
            duty::status_value(next.status),
            TraceId::of(b"duty"),
        ))
    }

    fn eval_require_authority(&mut self, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if args.is_empty() {
            return Err(unsupported("require_authority expects an action"));
        }
        let action = term_literal_name(&args[0])?;
        if duty::action_is_granted(self.case, &action, self.ctx.record_time) {
            Ok(determinate(Value::Unit, TraceId::of(b"authority")))
        } else {
            Ok(authority_missing(&action))
        }
    }

    fn eval_require(&mut self, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if args.is_empty() {
            return Err(unsupported("require expects a condition"));
        }
        match as_determinate(self.eval_term(&args[0])?) {
            Ok(Value::Bool(true)) => Ok(determinate(Value::Unit, TraceId::of(b"require"))),
            Ok(Value::Bool(false)) => Ok(require_failed()),
            Ok(other) => Err(unsupported(format!("require condition `{other:?}`"))),
            Err(outcome) => Ok(outcome),
        }
    }

    fn eval_field(&mut self, base: &Term, name: &str) -> Result<Outcome<Value>, EngineError> {
        match as_determinate(self.eval_term(base)?) {
            Ok(Value::Map(fields) | Value::Ctor { fields, .. }) => {
                fields.get(name).cloned().map_or_else(
                    || Err(unsupported(format!("missing field `{name}`"))),
                    |v| Ok(determinate(v, TraceId::of(b"eval"))),
                )
            }
            Ok(other) => Err(unsupported(format!("field access on `{other:?}`"))),
            Err(outcome) => Ok(outcome),
        }
    }

    fn eval_set(&mut self, xs: &[Term]) -> Result<Outcome<Value>, EngineError> {
        let mut values = Vec::with_capacity(xs.len());
        for x in xs {
            match as_determinate(self.eval_term(x)?) {
                Ok(v) => values.push(v),
                Err(outcome) => return Ok(outcome),
            }
        }
        Ok(determinate(Value::Set(values), TraceId::of(b"eval")))
    }

    fn eval_record(
        &mut self,
        fields: &BTreeMap<String, Term>,
    ) -> Result<Outcome<Value>, EngineError> {
        let mut map = BTreeMap::new();
        for (k, t) in fields {
            match as_determinate(self.eval_term(t)?) {
                Ok(v) => {
                    map.insert(k.clone(), v);
                }
                Err(outcome) => return Ok(outcome),
            }
        }
        Ok(determinate(Value::Map(map), TraceId::of(b"eval")))
    }

    fn eval_ctor(&mut self, ctor: &str, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if args.is_empty() {
            return Ok(determinate(
                Value::Ctor {
                    name: ctor.to_owned(),
                    fields: BTreeMap::new(),
                },
                TraceId::of(b"eval"),
            ));
        }
        let mut fields = BTreeMap::new();
        for (i, arg) in args.iter().enumerate() {
            match as_determinate(self.eval_term(arg)?) {
                Ok(v) => {
                    fields.insert(format!("_{i}"), v);
                }
                Err(outcome) => return Ok(outcome),
            }
        }
        Ok(determinate(
            Value::Ctor {
                name: ctor.to_owned(),
                fields,
            },
            TraceId::of(b"eval"),
        ))
    }

    fn eval_effect(&mut self, effect: &str, payload: &str) -> Outcome<Value> {
        let req = OpenRequest::NeedCustom {
            effect: effect.to_owned(),
            payload: payload.to_owned(),
        };
        match self.handler.handle(&req) {
            HandlerResult::Resume { value, .. } => determinate(value, TraceId::of(b"custom")),
            HandlerResult::Suspend { requests, .. } => Outcome::Suspended {
                requests,
                trace: TraceId::of(b"custom"),
            },
            HandlerResult::Halt { reason, .. } => {
                halt_to_outcome(reason, &[], TraceId::of(b"custom"))
            }
        }
    }

    fn eval_modal(&mut self, ctor: &str, args: &[Term]) -> Result<Outcome<Value>, EngineError> {
        if args.is_empty() {
            return Err(unsupported(format!("`{ctor}` expects one argument")));
        }
        let Some(prop) = self.ground_proposition(&args[0]) else {
            return self.eval_term(&args[0]);
        };
        if ctor.eq_ignore_ascii_case("determined") {
            match self.derived.modal_determined(&prop) {
                Some(value) => {
                    return Ok(determinate(Value::Bool(value), TraceId::of(b"operative")));
                }
                None => return Ok(eval_need_determine_prop(&prop, ctor, self.handler)),
            }
        }
        if self.derived.holds(&prop) {
            return Ok(determinate(Value::Bool(true), TraceId::of(b"operative")));
        }
        if self.derived.denied(&prop) {
            return Ok(determinate(Value::Bool(false), TraceId::of(b"operative")));
        }
        Ok(eval_need_determine_prop(&prop, ctor, self.handler))
    }

    fn eval_quantifier(
        &mut self,
        kind: QuantifierKind,
        args: &[Term],
    ) -> Result<Outcome<Value>, EngineError> {
        if args.len() != 3 {
            return Err(unsupported(format!(
                "`{}` expects [binder, domain, body]",
                kind.name()
            )));
        }
        let binder = binder_name(&args[0])
            .ok_or_else(|| unsupported(format!("quantifier binder `{:?}`", args[0])))?;
        match self.resolve_quantifier_domain(&args[1])? {
            DomainEval::Closed(elements) => self.eval_quantified(kind, &binder, elements, &args[2]),
            DomainEval::Open(outcome) => Ok(outcome),
        }
    }

    fn eval_quantified(
        &mut self,
        kind: QuantifierKind,
        binder: &str,
        elements: Vec<Value>,
        body: &Term,
    ) -> Result<Outcome<Value>, EngineError> {
        let previous = self.bindings.get(binder).cloned();
        let mut pending: Option<Outcome<Value>> = None;
        for element in elements {
            // Nested `for_all`/`exists` Apply/Call in `body` read this binder via lookup.
            self.bindings.insert(binder.to_owned(), element);
            match as_determinate(self.eval_term(body)?) {
                Ok(Value::Bool(true)) => {
                    if kind == QuantifierKind::Exists {
                        self.restore_binding(binder, previous.clone());
                        return Ok(determinate(Value::Bool(true), TraceId::of(b"exists")));
                    }
                }
                Ok(Value::Bool(false)) => {
                    if kind == QuantifierKind::ForAll {
                        self.restore_binding(binder, previous.clone());
                        return Ok(determinate(Value::Bool(false), TraceId::of(b"for_all")));
                    }
                }
                Ok(other) => {
                    self.restore_binding(binder, previous.clone());
                    return Err(unsupported(format!("quantifier body `{other:?}`")));
                }
                Err(outcome) => {
                    if pending.is_none() {
                        pending = Some(outcome);
                    }
                }
            }
        }
        self.restore_binding(binder, previous);
        if let Some(outcome) = pending {
            return Ok(outcome);
        }
        let value = match kind {
            QuantifierKind::ForAll => true,
            QuantifierKind::Exists => false,
        };
        Ok(determinate(
            Value::Bool(value),
            TraceId::of(kind.name().as_bytes()),
        ))
    }

    fn restore_binding(&mut self, binder: &str, previous: Option<Value>) {
        if let Some(value) = previous {
            self.bindings.insert(binder.to_owned(), value);
        } else {
            self.bindings.remove(binder);
        }
    }

    fn resolve_quantifier_domain(&mut self, domain: &Term) -> Result<DomainEval, EngineError> {
        if let Term::Set(_) = domain {
            return Ok(match as_determinate(self.eval_term(domain)?) {
                Ok(Value::Set(elements)) => DomainEval::Closed(elements),
                Ok(_) => DomainEval::Open(need_closure_record(domain_name(domain))),
                Err(outcome) => DomainEval::Open(outcome),
            });
        }
        if let Some(name) = domain_name(domain) {
            if let Some(Value::Set(elements)) = self.lookup(&name) {
                return Ok(DomainEval::Closed(elements));
            }
            if let Some(closure) = self
                .case
                .closures
                .iter()
                .find(|c| c.domain == name || c.domain.eq_ignore_ascii_case(&name))
            {
                if closure.closed {
                    let members = match self.lookup(&name) {
                        Some(Value::Set(elements)) => elements,
                        _ => Vec::new(),
                    };
                    return Ok(DomainEval::Closed(members));
                }
                return Ok(DomainEval::Open(need_closure_record(Some(name))));
            }
            return Ok(DomainEval::Open(need_closure_record(Some(name))));
        }
        Ok(match as_determinate(self.eval_term(domain)?) {
            Ok(Value::Set(elements)) => DomainEval::Closed(elements),
            Ok(_) => DomainEval::Open(need_closure_record(None)),
            Err(outcome) => DomainEval::Open(outcome),
        })
    }

    fn ground_proposition(&self, term: &Term) -> Option<PropTerm> {
        let mut prop = term_as_proposition(term)?;
        prop.arguments = prop
            .arguments
            .into_iter()
            .map(|arg| self.ground_term(arg))
            .collect();
        Some(prop)
    }

    fn ground_term(&self, term: Term) -> Term {
        match term {
            Term::Ident(name) | Term::Binder(name) => match self.lookup(&name) {
                Some(Value::Entity(s) | Value::String(s)) => Term::Ident(s),
                Some(Value::Int(i)) => Term::Int(i),
                Some(Value::Decimal(d)) => Term::Decimal(d),
                Some(Value::Bool(b)) => Term::Bool(b),
                _ => Term::Ident(name),
            },
            Term::Apply { ctor, args } => Term::Apply {
                ctor,
                args: args.into_iter().map(|a| self.ground_term(a)).collect(),
            },
            Term::Call { callee, args } => Term::Call {
                callee,
                args: args.into_iter().map(|a| self.ground_term(a)).collect(),
            },
            other => other,
        }
    }

    fn lookup(&self, name: &str) -> Option<Value> {
        self.bindings
            .get(name)
            .or_else(|| self.args.get(name))
            .or_else(|| self.case.facts.get(name))
            .cloned()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QuantifierKind {
    ForAll,
    Exists,
}

impl QuantifierKind {
    fn name(self) -> &'static str {
        match self {
            Self::ForAll => "for_all",
            Self::Exists => "exists",
        }
    }
}

enum DomainEval {
    Closed(Vec<Value>),
    Open(Outcome<Value>),
}

fn quantifier_kind(name: &str) -> Option<QuantifierKind> {
    if name.eq_ignore_ascii_case("for_all") || name.eq_ignore_ascii_case("forall") {
        Some(QuantifierKind::ForAll)
    } else if name.eq_ignore_ascii_case("exists") {
        Some(QuantifierKind::Exists)
    } else {
        None
    }
}

fn need_closure_record(domain: Option<String>) -> Outcome<Value> {
    let issue = PropTerm::new(
        domain.clone().unwrap_or_else(|| "domain".into()),
        Vec::new(),
    );
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedEvidence {
        issue: PropPattern::Ground(issue),
        schema: "ClosureRecord".into(),
    });
    Outcome::Suspended {
        requests,
        trace: TraceId::of(b"quantifier"),
    }
}

fn eval_need_determine_prop<H: Handler>(
    prop: &PropTerm,
    protocol: &str,
    handler: &mut H,
) -> Outcome<Value> {
    let req = OpenRequest::NeedJudgment {
        issue: prop.clone(),
        protocol: protocol.to_owned(),
    };
    let trace = TraceId::of(protocol.as_bytes());
    match handler.handle(&req) {
        HandlerResult::Resume { value, .. } => determinate(value, trace),
        HandlerResult::Suspend { requests, .. } => Outcome::Suspended { requests, trace },
        HandlerResult::Halt { reason, .. } => halt_to_outcome(reason, &[], trace),
    }
}

fn unsupported(detail: impl Into<String>) -> EngineError {
    EngineError::Unsupported(detail.into())
}

#[allow(clippy::result_large_err)]
fn as_determinate(out: Outcome<Value>) -> Result<Value, Outcome<Value>> {
    match out {
        Outcome::Determinate { value, .. } => Ok(value),
        other => Err(other),
    }
}

fn find_function<'m>(module: &'m CoreModule, name: &str) -> Option<&'m CoreFunction> {
    module.declarations.iter().find_map(|d| match d {
        CoreDecl::Function(f) if f.name == name => Some(f),
        _ => None,
    })
}

fn find_duty<'m>(module: &'m CoreModule, name: &str) -> Option<&'m CoreDuty> {
    module.declarations.iter().find_map(|d| match d {
        CoreDecl::Duty(duty) if duty.name == name || duty.name.eq_ignore_ascii_case(name) => {
            Some(duty)
        }
        _ => None,
    })
}

fn seq_progress(frames: &[SeqFrame]) -> (usize, Vec<Value>) {
    frames
        .iter()
        .find(|frame| frame.path.is_empty())
        .or_else(|| frames.first())
        .map(|frame| (frame.index, frame.completed.clone()))
        .unwrap_or((0, Vec::new()))
}

fn program_or_query_changed(
    cont: &Continuation,
    module: &CoreModule,
    query: &QueryName,
    identity: &CaseIdentity,
) -> bool {
    if cont.case_identity.program_fingerprint != identity.program_fingerprint
        || cont.case_identity.query != identity.query
    {
        return true;
    }
    let Some(q) = module.query(query.as_str()) else {
        return true;
    };
    match (&cont.residual, &q.plan) {
        (Residual::Term(term), QueryPlan::Evaluate(plan_term)) => term != plan_term,
        (Residual::Plan(plan), other) => plan != other,
        _ => true,
    }
}

fn find_effect_name(module: &CoreModule, name: &str) -> Option<String> {
    module.declarations.iter().find_map(|d| match d {
        CoreDecl::EffectDecl(e) if e.name == name => Some(e.name.clone()),
        _ => None,
    })
}

fn custom_effect_payload(effect: &str, args: &[Term]) -> String {
    if args.is_empty() {
        return effect.to_owned();
    }
    format!(
        "{effect}({})",
        args.iter()
            .map(term_effect_arg)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn term_effect_arg(term: &Term) -> String {
    match term {
        Term::String(s) | Term::Ident(s) | Term::Binder(s) => s.clone(),
        Term::Int(n) => n.to_string(),
        Term::Bool(b) => b.to_string(),
        Term::Decimal(d) => d.to_string(),
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => {
            custom_effect_payload(ctor, args)
        }
        other => format!("{other:?}"),
    }
}

fn find_decision<'m>(module: &'m CoreModule, name: &str) -> Option<&'m CoreDecision> {
    module.declarations.iter().find_map(|d| match d {
        CoreDecl::Decision(dec) if dec.name == name => Some(dec),
        _ => None,
    })
}

fn is_entity(module: &CoreModule, name: &str) -> bool {
    module.declarations.iter().any(|d| match d {
        CoreDecl::Entity(e) => e.name == name,
        _ => false,
    })
}

fn is_if(ctor: &str) -> bool {
    ctor == "if" || ctor == "If" || ctor.eq_ignore_ascii_case("cond")
}

fn is_not(ctor: &str) -> bool {
    ctor == "not" || ctor == "!" || ctor.eq_ignore_ascii_case("Not")
}

fn is_seq(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("seq")
}

fn is_transaction(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("transaction")
}

fn is_require(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("require")
}

fn is_duty_step(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("duty_step")
}

fn is_duty_status(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("duty_status")
}

fn is_require_authority(ctor: &str) -> bool {
    ctor.eq_ignore_ascii_case("require_authority")
}

fn term_literal_name(term: &Term) -> Result<String, EngineError> {
    match term {
        Term::Ident(name) | Term::String(name) | Term::Binder(name) => Ok(name.clone()),
        Term::Apply { ctor, args } if args.is_empty() => Ok(ctor.clone()),
        Term::Call { callee, args } if args.is_empty() => Ok(callee.clone()),
        other => Err(EngineError::InvalidInput(format!(
            "expected name, got `{other:?}`"
        ))),
    }
}

fn require_failed() -> Outcome<Value> {
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedCustom {
        effect: "require".into(),
        payload: "requirement failed".into(),
    });
    Outcome::Suspended {
        requests,
        trace: TraceId::of(b"require"),
    }
}

fn authority_missing(action: &str) -> Outcome<Value> {
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedCustom {
        effect: "authority".into(),
        payload: action.to_owned(),
    });
    Outcome::Suspended {
        requests,
        trace: TraceId::of(b"authority"),
    }
}

fn binop_ctor(ctor: &str) -> Option<String> {
    let op = match ctor {
        "+" | "add" | "plus" => "+",
        "-" | "sub" | "minus" => "-",
        "*" | "mul" | "times" => "*",
        "/" | "div" => "/",
        "&&" | "and" => "&&",
        "||" | "or" => "||",
        "==" | "eq" | "=" => "==",
        "!=" | "ne" => "!=",
        "<" | "lt" => "<",
        "<=" | "le" => "<=",
        ">" | "gt" => ">",
        ">=" | "ge" => ">=",
        _ => return None,
    };
    Some(op.to_owned())
}

fn binop_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-",
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn eval_binop(op: &str, left: &Value, right: &Value) -> Result<Value, EngineError> {
    match op {
        "+" | "-" | "*" | "/" => eval_arith(op, left, right),
        "&&" => bool_bin(left, right, |a, b| a && b),
        "||" => bool_bin(left, right, |a, b| a || b),
        "==" => Ok(Value::Bool(left == right)),
        "!=" => Ok(Value::Bool(left != right)),
        "<" | "<=" | ">" | ">=" => eval_cmp(op, left, right),
        _ => Err(unsupported(format!("operator `{op}`"))),
    }
}

fn bool_bin(
    left: &Value,
    right: &Value,
    f: impl Fn(bool, bool) -> bool,
) -> Result<Value, EngineError> {
    match (left, right) {
        (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(f(*a, *b))),
        _ => Err(unsupported("boolean operator on non-bool")),
    }
}

fn eval_arith(op: &str, left: &Value, right: &Value) -> Result<Value, EngineError> {
    match (promote_num(left)?, promote_num(right)?) {
        (Num::Int(a), Num::Int(b)) => match op {
            "+" => checked_int(a.checked_add(b), "+"),
            "-" => checked_int(a.checked_sub(b), "-"),
            "*" => checked_int(a.checked_mul(b), "*"),
            "/" => {
                if b == 0 {
                    return Err(EngineError::InvalidInput("division by zero".into()));
                }
                checked_int(a.checked_div(b), "/")
            }
            _ => Err(unsupported(format!("arithmetic `{op}`"))),
        },
        (a, b) => {
            let a = num_to_dec(a);
            let b = num_to_dec(b);
            match op {
                "+" => checked_dec(a.checked_add(b), "+"),
                "-" => checked_dec(a.checked_sub(b), "-"),
                "*" => checked_dec(a.checked_mul(b), "*"),
                "/" => {
                    if b.is_zero() {
                        return Err(EngineError::InvalidInput("division by zero".into()));
                    }
                    checked_dec(a.checked_div(b), "/")
                }
                _ => Err(unsupported(format!("arithmetic `{op}`"))),
            }
        }
    }
}

fn num_to_dec(n: Num) -> Decimal {
    match n {
        Num::Int(i) => Decimal::from(i),
        Num::Dec(d) => d,
    }
}

enum Num {
    Int(i64),
    Dec(Decimal),
}

fn promote_num(value: &Value) -> Result<Num, EngineError> {
    match value {
        Value::Int(i) => Ok(Num::Int(*i)),
        Value::Decimal(d) => Ok(Num::Dec(*d)),
        Value::String(s) => {
            if let Ok(i) = s.parse::<i64>() {
                return Ok(Num::Int(i));
            }
            if let Ok(d) = s.parse::<Decimal>() {
                return Ok(Num::Dec(d));
            }
            money_amount(Some(value)).map(Num::Dec)
        }
        _ => Err(unsupported(format!("not a number `{value:?}`"))),
    }
}

fn checked_int(value: Option<i64>, op: &str) -> Result<Value, EngineError> {
    value
        .map(Value::Int)
        .ok_or_else(|| EngineError::InvalidInput(format!("integer overflow in `{op}`")))
}

fn checked_dec(value: Option<Decimal>, op: &str) -> Result<Value, EngineError> {
    value
        .map(Value::Decimal)
        .ok_or_else(|| EngineError::InvalidInput(format!("decimal overflow in `{op}`")))
}

fn eval_cmp(op: &str, left: &Value, right: &Value) -> Result<Value, EngineError> {
    let ord = match (promote_num(left)?, promote_num(right)?) {
        (Num::Int(a), Num::Int(b)) => a.cmp(&b),
        (Num::Dec(a), Num::Dec(b)) => a.cmp(&b),
        (Num::Int(a), Num::Dec(b)) => Decimal::from(a).cmp(&b),
        (Num::Dec(a), Num::Int(b)) => a.cmp(&Decimal::from(b)),
    };
    let result = match op {
        "<" => ord.is_lt(),
        "<=" => ord.is_le(),
        ">" => ord.is_gt(),
        ">=" => ord.is_ge(),
        _ => return Err(unsupported(format!("comparator `{op}`"))),
    };
    Ok(Value::Bool(result))
}

fn money_amount(value: Option<&Value>) -> Result<Decimal, EngineError> {
    let Some(value) = value else {
        return Err(EngineError::InvalidInput("missing money amount".into()));
    };
    match value {
        Value::Decimal(d) => Ok(*d),
        Value::Int(i) => Ok(Decimal::from(*i)),
        Value::String(s) => {
            let t = s
                .trim()
                .trim_start_matches("USD(")
                .trim_end_matches(')')
                .trim();
            t.parse()
                .map_err(|_| EngineError::InvalidInput(format!("invalid money `{s}`")))
        }
        other => Err(EngineError::InvalidInput(format!(
            "invalid money `{other:?}`"
        ))),
    }
}

fn ordinary_income_tax(amount: Decimal) -> Decimal {
    let b1 = Decimal::from(11_925);
    let b2 = Decimal::from(48_475);
    let b3 = Decimal::from(103_350);
    let p10 = Decimal::new(10, 2);
    let p12 = Decimal::new(12, 2);
    let p22 = Decimal::new(22, 2);
    let p24 = Decimal::new(24, 2);
    if amount <= b1 {
        return amount * p10;
    }
    let mut tax = b1 * p10;
    if amount <= b2 {
        return tax + (amount - b1) * p12;
    }
    tax += (b2 - b1) * p12;
    if amount <= b3 {
        return tax + (amount - b2) * p22;
    }
    tax += (b3 - b2) * p22;
    tax + (amount - b3) * p24
}

fn eval_judgment<H: Handler>(handler: &mut H, case: &CaseRecord) -> Outcome<Value> {
    if let Some(d) = case
        .determinations
        .iter()
        .find(|d| d.protocol.contains("Judgment") && d.established)
    {
        return determinate(
            case.facts
                .get("judgment_entry")
                .cloned()
                .unwrap_or_else(|| Value::String(d.protocol.clone())),
            TraceId::of(b"judgment"),
        );
    }
    eval_need_determine("Adjudicated", "JudgmentOnTheMerits", handler)
}

fn eval_need_determine<H: Handler>(issue: &str, protocol: &str, handler: &mut H) -> Outcome<Value> {
    let req = OpenRequest::NeedJudgment {
        issue: PropTerm::new(issue, vec![]),
        protocol: protocol.into(),
    };
    let trace = TraceId::of(protocol.as_bytes());
    match handler.handle(&req) {
        HandlerResult::Resume { value, .. } => determinate(value, trace),
        HandlerResult::Suspend { requests, .. } => Outcome::Suspended { requests, trace },
        HandlerResult::Halt { reason, .. } => halt_to_outcome(reason, &[], trace),
    }
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
        Term::Call { callee, .. } => callee.as_str(),
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

fn office_key(office: &Term) -> String {
    match office {
        Term::Ident(s) | Term::String(s) => s.clone(),
        Term::Apply { ctor, args } => format_office(ctor, args),
        Term::Call { callee, args } => format_office(callee, args),
        other => format!("{other:?}"),
    }
}

fn format_office(head: &str, args: &[Term]) -> String {
    if args.is_empty() {
        head.to_owned()
    } else {
        format!(
            "{head}({})",
            term_strings(&Term::Set(args.to_vec())).join(",")
        )
    }
}

fn office_matches(nomination_office: &str, key: &str) -> bool {
    if nomination_office == key {
        return true;
    }
    let nom_head = nomination_office
        .split('(')
        .next()
        .unwrap_or(nomination_office);
    let key_head = key.split('(').next().unwrap_or(key);
    nom_head == key_head || key.starts_with(nomination_office) || nomination_office.starts_with(key)
}

fn eval_unique_occupant<H: Handler>(
    module: &CoreModule,
    office: &Term,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
    derived: &DerivedWorld,
) -> Outcome<Value> {
    let trace = TraceId::of(b"unique-occupant");
    let key = office_key(office);
    let recorded = occupancy_from_records(case, &key, ctx);
    let occupants = occupants_for_office(state, recorded, &key, ctx);
    if occupants.is_empty() {
        let mut requests = BTreeSet::new();
        requests.insert(OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(PropTerm::new(
                "Occupies",
                vec![Term::Wildcard, office.clone()],
            )),
            schema: "OccupancyRecord".into(),
        });
        return Outcome::Suspended { requests, trace };
    }
    if occupants.len() > 1 {
        return Outcome::Inconsistent {
            core: vec![format!("office `{key}` has multiple occupants")],
            trace,
        };
    }
    let current = occupants[0].person.clone();
    let required = required_concurring(case);
    let schemas = concurring_schemas(module, case);
    let matching = count_concurring(case, &schemas, Some(current.as_str()), ctx.record_time);
    match occupant_incapacitated(derived, case, &current, office) {
        Some(false) => determinate(Value::Entity(current), trace),
        Some(true) => eval_succession(module, office, &key, case, ctx, trace),
        None if required > 0 && matching >= required => {
            eval_succession(module, office, &key, case, ctx, trace)
        }
        None if required > 0 && matching < required => {
            incomplete_concurring(module, office, &current, case, &schemas, handler, trace)
        }
        None => determinate(Value::Entity(current), trace),
    }
}

fn occupants_for_office(
    state: &LegalState,
    recorded: Vec<Occupancy>,
    office: &str,
    ctx: &RunContext,
) -> Vec<Occupancy> {
    let from_state: Vec<Occupancy> = state
        .authority
        .occupancy
        .iter()
        .filter(|o| {
            office_matches(&o.office, office)
                && o.mode == StatusMode::Established
                && o.valid_time.contains(ctx.valid_time)
                && o.record_time <= ctx.record_time
        })
        .cloned()
        .collect();
    if from_state.is_empty() {
        recorded
    } else {
        from_state
    }
}

fn required_concurring(case: &CaseRecord) -> usize {
    match case.facts.get("required_concurring") {
        Some(Value::Int(i)) if *i >= 0 => *i as usize,
        Some(Value::String(s)) => s.parse().ok().unwrap_or(2),
        _ => 2,
    }
}

fn is_structural_schema(schema: &str) -> bool {
    matches!(
        schema,
        "OccupancyRecord"
            | "AcceptOffice"
            | "OfficialFilingRecord"
            | "ClosureRecord"
            | "FilingTransportReceipt"
    )
}

fn concurring_schemas(module: &CoreModule, case: &CaseRecord) -> Vec<String> {
    let mut out = BTreeSet::new();
    for item in &case.evidence {
        if !is_structural_schema(&item.schema) {
            out.insert(item.schema.clone());
        }
    }
    for schema in case.admissible_completions.evidence.keys() {
        if !is_structural_schema(schema) {
            out.insert(schema.clone());
        }
    }
    for decl in &module.declarations {
        match decl {
            CoreDecl::RecordType(rt) if rt.is_evidence && !is_structural_schema(&rt.name) => {
                out.insert(rt.name.clone());
            }
            CoreDecl::Observation(obs) if !is_structural_schema(&obs.name) => {
                out.insert(obs.name.clone());
            }
            CoreDecl::Judgment(judgment)
                if judgment
                    .decides
                    .predicate
                    .eq_ignore_ascii_case("Incapacitated") =>
            {
                for (_, ty) in &judgment.record_requires {
                    if let Some(name) = type_schema_name(ty)
                        && !is_structural_schema(&name)
                    {
                        out.insert(name);
                    }
                }
            }
            _ => {}
        }
    }
    out.into_iter().collect()
}

fn type_schema_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Sort(Sort::Nominal(name)) => Some(name.clone()),
        Type::Applied { ctor, .. } => Some(ctor.clone()),
        _ => None,
    }
}

fn count_concurring(
    case: &CaseRecord,
    schemas: &[String],
    subject: Option<&str>,
    known_at: Instant,
) -> usize {
    case.evidence
        .iter()
        .filter(|item| concurring_matches(item, schemas, subject, known_at))
        .count()
}

fn concurring_matches(
    evidence: &EvidenceItem,
    schemas: &[String],
    subject: Option<&str>,
    known_at: Instant,
) -> bool {
    if evidence.observed_at > known_at {
        return false;
    }
    if !schemas.iter().any(|schema| schema == &evidence.schema) {
        return false;
    }
    let got_subject = evidence_subject(&evidence.value);
    if let (Some(want), Some(got)) = (subject, got_subject.as_deref())
        && !want.eq_ignore_ascii_case(got)
    {
        return false;
    }
    true
}

fn evidence_subject(value: &Value) -> Option<String> {
    match value {
        Value::Map(fields) | Value::Ctor { fields, .. } => fields
            .get("subject")
            .or_else(|| fields.get("person"))
            .and_then(|v| strings_from_value(v).into_iter().next()),
        _ => None,
    }
}

fn occupant_incapacitated(
    derived: &DerivedWorld,
    case: &CaseRecord,
    occupant: &str,
    office: &Term,
) -> Option<bool> {
    let grounded = incapacitated_prop(occupant, office);
    if derived.denied(&grounded) {
        return Some(false);
    }
    if derived.holds(&grounded) || derived.holds(&PropTerm::new("Incapacitated", Vec::new())) {
        return Some(true);
    }
    for det in &case.determinations {
        let prop = parse_prop_issue(&det.issue);
        if !prop.predicate.eq_ignore_ascii_case("Incapacitated") {
            continue;
        }
        let args: Vec<String> = prop.arguments.iter().map(arg_label_term).collect();
        if !args.is_empty()
            && !args
                .iter()
                .any(|arg| arg.eq_ignore_ascii_case(occupant) || office_matches(arg, occupant))
        {
            continue;
        }
        return Some(det.established);
    }
    None
}

fn arg_label_term(term: &Term) -> String {
    match term {
        Term::Ident(s) | Term::String(s) | Term::Binder(s) => s.clone(),
        Term::Apply { ctor, args } => format_office(ctor, args),
        Term::Call { callee, args } => format_office(callee, args),
        other => format!("{other:?}"),
    }
}

fn incapacitated_prop(occupant: &str, office: &Term) -> PropTerm {
    PropTerm::new(
        "Incapacitated",
        vec![Term::Ident(occupant.to_owned()), office.clone()],
    )
}

fn incomplete_concurring<H: Handler>(
    module: &CoreModule,
    office: &Term,
    occupant: &str,
    case: &CaseRecord,
    schemas: &[String],
    handler: &mut H,
    trace: TraceId,
) -> Outcome<Value> {
    if let Some(schema) = missing_concurring_schema(case, schemas) {
        let req = OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(incapacitated_prop(occupant, office)),
            schema,
        };
        return handle_incomplete(handler, req, trace);
    }
    let req = OpenRequest::NeedJudgment {
        issue: incapacitated_prop(occupant, office),
        protocol: incapacity_protocol(module),
    };
    handle_incomplete(handler, req, trace)
}

fn handle_incomplete<H: Handler>(
    handler: &mut H,
    req: OpenRequest,
    trace: TraceId,
) -> Outcome<Value> {
    match handler.handle(&req) {
        HandlerResult::Resume { .. } => {
            // Resume of already-present evidence must not close an incomplete
            // concurring set. Ignored open issues need a real CheckedCertificate.
            let mut requests = BTreeSet::new();
            requests.insert(req);
            Outcome::Suspended { requests, trace }
        }
        HandlerResult::Suspend { mut requests, .. } => {
            if requests.is_empty() {
                requests.insert(req);
            }
            Outcome::Suspended { requests, trace }
        }
        HandlerResult::Halt { reason, .. } => halt_to_outcome(reason, &[], trace),
    }
}

fn missing_concurring_schema(case: &CaseRecord, schemas: &[String]) -> Option<String> {
    for schema in case.admissible_completions.evidence.keys() {
        if is_structural_schema(schema) {
            continue;
        }
        if !case.evidence.iter().any(|item| item.schema == *schema) {
            return Some(schema.clone());
        }
    }
    schemas
        .iter()
        .find(|schema| case.evidence.iter().any(|item| item.schema == **schema))
        .cloned()
        .or_else(|| schemas.first().cloned())
}

fn incapacity_protocol(module: &CoreModule) -> String {
    module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::Judgment(judgment)
                if judgment
                    .decides
                    .predicate
                    .eq_ignore_ascii_case("Incapacitated") =>
            {
                Some(judgment.name.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| "Incapacity".into())
}

fn eval_succession(
    module: &CoreModule,
    office: &Term,
    key: &str,
    case: &CaseRecord,
    ctx: &RunContext,
    trace: TraceId,
) -> Outcome<Value> {
    let mut nominations: Vec<_> = module
        .nominations
        .iter()
        .filter(|n| office_matches(&n.office, key))
        .cloned()
        .collect();
    nominations.sort_by_key(|n| n.rank);
    let ranked: Vec<(String, i64)> = nominations
        .iter()
        .map(|n| (n.candidate.clone(), n.rank))
        .collect();
    if ranked.is_empty() {
        return need_appointment(office, trace);
    }
    let Some((family_name, alts)) = succession_family(module, case, key) else {
        return need_appointment(office, trace);
    };
    if let Some(label) = case.interpretations.get(&family_name) {
        let defs = defs_for_alternative(alts, label);
        return select_from_eligibility(&ranked, defs, office, case, ctx, trace);
    }
    contingent_from_alternatives(&family_name, alts, &ranked, office, case, ctx, trace)
}

type EligibilityDef = (PropTerm, bool);
type InterpretationAlts = [(String, Vec<EligibilityDef>)];

fn succession_family<'m>(
    module: &'m CoreModule,
    case: &CaseRecord,
    office: &str,
) -> Option<(String, &'m InterpretationAlts)> {
    let matching: Vec<_> = module
        .declarations
        .iter()
        .filter_map(|d| match d {
            CoreDecl::InterpretationFamily(family)
                if family_defines_office(&family.alternatives, office) =>
            {
                Some(family)
            }
            _ => None,
        })
        .collect();
    if matching.is_empty() {
        return None;
    }
    if let Some(name) = case
        .interpretations
        .keys()
        .find(|name| matching.iter().any(|family| family.name == **name))
        && let Some(family) = matching.iter().find(|family| family.name == *name)
    {
        return Some((family.name.clone(), family.alternatives.as_slice()));
    }
    matching
        .first()
        .map(|family| (family.name.clone(), family.alternatives.as_slice()))
}

fn family_defines_office(alts: &InterpretationAlts, office: &str) -> bool {
    alts.iter().any(|(_, defs)| {
        defs.iter().any(|(prop, _)| {
            prop.predicate == "Eligible" && argument_names_office(prop.arguments.get(1), office)
        })
    })
}

fn defs_for_alternative<'a>(alts: &'a InterpretationAlts, label: &str) -> &'a [EligibilityDef] {
    alts.iter()
        .find(|(name, _)| name == label)
        .map(|(_, defs)| defs.as_slice())
        .unwrap_or(&[])
}

fn select_from_eligibility(
    ranked: &[(String, i64)],
    defs: &[EligibilityDef],
    office: &Term,
    case: &CaseRecord,
    ctx: &RunContext,
    trace: TraceId,
) -> Outcome<Value> {
    let office_key = office_key(office);
    let eligible: Vec<(String, i64)> = ranked
        .iter()
        .filter(|(name, _)| is_defined_eligible(defs, name, &office_key))
        .cloned()
        .collect();
    let accepted: Vec<(String, i64)> = eligible
        .iter()
        .filter(|(name, _)| is_accepted(case, name, &office_key, ctx))
        .cloned()
        .collect();
    if accepted.is_empty() {
        if eligible.is_empty() {
            let mut requests = BTreeSet::new();
            requests.insert(OpenRequest::NeedJudgment {
                issue: PropTerm::new("Eligible", vec![Term::Wildcard, office.clone()]),
                protocol: "Eligibility".into(),
            });
            return Outcome::Suspended { requests, trace };
        }
        return need_accept_office(office, trace);
    }
    let winner = accepted.iter().min_by_key(|(_, rank)| *rank).unwrap();
    determinate(Value::Entity(winner.0.clone()), trace)
}

fn is_defined_eligible(defs: &[EligibilityDef], person: &str, office: &str) -> bool {
    defs.iter().any(|(prop, established)| {
        *established
            && prop.predicate == "Eligible"
            && argument_names_person(prop.arguments.first(), person)
            && argument_names_office(prop.arguments.get(1), office)
    })
}

fn argument_names_person(term: Option<&Term>, person: &str) -> bool {
    match term {
        Some(Term::Ident(name) | Term::String(name)) => name.eq_ignore_ascii_case(person),
        _ => false,
    }
}

fn argument_names_office(term: Option<&Term>, office: &str) -> bool {
    match term {
        Some(t) => office_matches(&office_key(t), office),
        None => false,
    }
}

fn contingent_from_alternatives(
    family: &str,
    alts: &InterpretationAlts,
    ranked: &[(String, i64)],
    office: &Term,
    case: &CaseRecord,
    ctx: &RunContext,
    trace: TraceId,
) -> Outcome<Value> {
    if alts.is_empty() {
        return need_accept_office(office, trace);
    }
    let branches: Vec<(String, Outcome<Value>)> = alts
        .iter()
        .map(|(label, defs)| {
            (
                label.clone(),
                select_from_eligibility(ranked, defs, office, case, ctx, trace),
            )
        })
        .collect();
    combine_succession_alternatives(family, branches, office, trace)
}

fn combine_succession_alternatives(
    family: &str,
    branches: Vec<(String, Outcome<Value>)>,
    office: &Term,
    trace: TraceId,
) -> Outcome<Value> {
    let mut alternatives = BTreeMap::new();
    let mut requests = BTreeSet::new();
    let mut inconsistent = Vec::new();
    let mut competence = Vec::new();
    let mut conflicts = Vec::new();
    let mut all_determinate = true;
    let mut has_suspended = false;

    for (label, outcome) in branches {
        match outcome {
            Outcome::Determinate { value, .. } => {
                alternatives.insert(label, value);
            }
            Outcome::Suspended { requests: open, .. } => {
                all_determinate = false;
                has_suspended = true;
                requests.extend(open);
            }
            Outcome::Contingent {
                alternatives: nested,
                pivots,
                ..
            } => {
                all_determinate = false;
                alternatives.extend(nested);
                requests.extend(pivots);
            }
            Outcome::Inconsistent { .. } => {
                all_determinate = false;
                inconsistent.push(outcome);
            }
            Outcome::OutsideCompetence { ref request, .. } => {
                all_determinate = false;
                requests.insert(request.clone());
                competence.push(outcome);
            }
            Outcome::NormConflict { .. } => {
                all_determinate = false;
                conflicts.push(outcome);
            }
        }
    }

    if all_determinate {
        if alternatives.is_empty() {
            return need_accept_office(office, trace);
        }
        if alternative_values_agree(&alternatives) {
            let value = alternatives
                .into_values()
                .next()
                .expect("non-empty alternatives");
            return determinate(value, trace);
        }
        return succession_contingent(family, alternatives, BTreeSet::new(), trace);
    }

    if let Some(outcome) = inconsistent.into_iter().next() {
        return outcome;
    }
    if let Some(outcome) = conflicts.into_iter().next() {
        return outcome;
    }
    if has_suspended {
        if !alternative_values_agree(&alternatives) && alternatives.len() > 1 {
            return succession_contingent(family, alternatives, requests, trace);
        }
        return Outcome::Suspended { requests, trace };
    }
    if competence.len() == 1 {
        return competence.remove(0);
    }
    if !competence.is_empty() {
        return Outcome::Suspended { requests, trace };
    }
    if alternatives.is_empty() {
        return need_accept_office(office, trace);
    }
    succession_contingent(family, alternatives, requests, trace)
}

fn alternative_values_agree(alternatives: &BTreeMap<String, Value>) -> bool {
    let mut values = alternatives.values();
    let Some(first) = values.next() else {
        return false;
    };
    values.all(|value| value == first)
}

fn succession_contingent(
    family: &str,
    alternatives: BTreeMap<String, Value>,
    mut pivots: BTreeSet<OpenRequest>,
    trace: TraceId,
) -> Outcome<Value> {
    pivots.insert(OpenRequest::NeedInterpretation {
        source: family.to_owned(),
        family: family.to_owned(),
    });
    Outcome::Contingent {
        alternatives,
        pivots,
        trace,
    }
}

fn need_appointment(office: &Term, trace: TraceId) -> Outcome<Value> {
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedJudgment {
        issue: PropTerm::new("Occupies", vec![Term::Wildcard, office.clone()]),
        protocol: "Appointment".into(),
    });
    Outcome::Suspended { requests, trace }
}

fn need_accept_office(office: &Term, trace: TraceId) -> Outcome<Value> {
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedEvidence {
        issue: PropPattern::Ground(PropTerm::new(
            "Accepted",
            vec![Term::Wildcard, office.clone()],
        )),
        schema: "AcceptOffice".into(),
    });
    Outcome::Suspended { requests, trace }
}

fn is_accepted(case: &CaseRecord, name: &str, office: &str, ctx: &RunContext) -> bool {
    let key = format!("{}_accepted", name.to_ascii_lowercase());
    if case.facts.get(&key) == Some(&Value::Bool(true)) {
        return true;
    }
    case.evidence.iter().any(|item| {
        item.schema == "AcceptOffice"
            && item.observed_at <= ctx.record_time
            && accept_office_matches(&item.value, name, office)
    })
}

fn accept_office_matches(value: &Value, name: &str, office: &str) -> bool {
    match value {
        Value::Entity(got) | Value::String(got) => got.eq_ignore_ascii_case(name),
        Value::Map(fields) | Value::Ctor { fields, .. } => {
            let person_ok = person_field(fields)
                .map(|got| got.eq_ignore_ascii_case(name))
                .unwrap_or_else(|| value_names_person(value, name));
            if !person_ok {
                return false;
            }
            match fields.get("office") {
                Some(rec) => strings_from_value(rec)
                    .iter()
                    .any(|got| office_matches(got, office)),
                None => true,
            }
        }
        _ => false,
    }
}

fn person_field(fields: &BTreeMap<String, Value>) -> Option<String> {
    fields
        .get("person")
        .or_else(|| fields.get("candidate"))
        .or_else(|| fields.get("occupant"))
        .or_else(|| fields.get("holder"))
        .or_else(|| fields.get("name"))
        .and_then(|v| strings_from_value(v).into_iter().next())
}

fn value_names_person(value: &Value, name: &str) -> bool {
    match value {
        Value::Entity(got) | Value::String(got) => got.eq_ignore_ascii_case(name),
        Value::Map(fields) | Value::Ctor { fields, .. } => fields.values().any(|v| {
            strings_from_value(v)
                .iter()
                .any(|s| s.eq_ignore_ascii_case(name))
        }),
        _ => false,
    }
}

fn eval_status_of(
    status: &LegalStatusPattern,
    when_present: &Term,
    when_closed_absent: &Term,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Outcome<Value> {
    let trace = TraceId::of(b"status-of");
    let ctor = status_constructor(status, when_present);
    let subjects = status_subjects(status, when_present);
    let fields = status_subject_fields(status, when_present);
    let view = FrozenCaseView::from_context(case, ctx);
    let filed = view.evidence().any(|item| {
        item.schema == "OfficialFilingRecord" && record_matches_subjects(&item.value, &subjects)
    });
    let complies = view
        .determinations()
        .any(|det| determination_complies(det, &subjects));
    if filed && complies {
        return determinate(Value::Ctor { name: ctor, fields }, trace);
    }
    if filed && !complies {
        let mut requests = BTreeSet::new();
        requests.insert(OpenRequest::NeedJudgment {
            issue: PropTerm::new("SubstantiallyComplies", vec![]),
            protocol: "FormationCompliance".into(),
        });
        return Outcome::Suspended { requests, trace };
    }
    if closed_world_absent(case, &ctor) {
        return determinate(
            closed_absent_value(when_closed_absent, &ctor, fields),
            trace,
        );
    }
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedEvidence {
        issue: PropPattern::Match {
            predicate: "Filed".into(),
            arguments: filed_need_args(&subjects),
        },
        schema: "OfficialFilingRecord".into(),
    });
    Outcome::Suspended { requests, trace }
}

fn status_constructor(status: &LegalStatusPattern, when_present: &Term) -> String {
    let named = match status {
        LegalStatusPattern::InstitutionalStatus { constructor, .. } => constructor.clone(),
        LegalStatusPattern::PropositionStatus { proposition, .. } => match proposition {
            PropPattern::Ground(p) => p.predicate.clone(),
            PropPattern::Match { predicate, .. } => predicate.clone(),
        },
    };
    if named.is_empty() {
        match when_present {
            Term::Apply { ctor, .. } | Term::Ident(ctor) => ctor.clone(),
            _ => "FormedLLC".into(),
        }
    } else {
        named
    }
}

fn closed_world_absent(case: &CaseRecord, ctor: &str) -> bool {
    case.closures
        .iter()
        .any(|c| c.closed && closure_matches_status(&c.domain, ctor))
}

fn closure_matches_status(domain: &str, ctor: &str) -> bool {
    let domain = domain.trim();
    let ctor = ctor.trim();
    if domain.eq_ignore_ascii_case(ctor) {
        return true;
    }
    let stripped = ctor.strip_prefix("Not").unwrap_or(ctor);
    if domain.eq_ignore_ascii_case(stripped) {
        return true;
    }
    let dn = domain.to_ascii_lowercase();
    let cn = ctor.to_ascii_lowercase();
    if cn.contains("formed") || cn.contains("filing") || cn.contains("llc") {
        return matches!(dn.as_str(), "filings" | "filing" | "formation");
    }
    false
}

fn status_subjects(status: &LegalStatusPattern, when_present: &Term) -> Vec<String> {
    let mut names = status_pattern_arg_names(status);
    if names.is_empty() {
        names = term_arg_names(when_present);
    }
    names
}

fn status_pattern_arg_names(status: &LegalStatusPattern) -> Vec<String> {
    status_pattern_arg_terms(status)
        .iter()
        .flat_map(term_arg_names)
        .collect()
}

fn status_pattern_arg_terms(status: &LegalStatusPattern) -> Vec<Term> {
    match status {
        LegalStatusPattern::InstitutionalStatus { arguments, .. } => arguments
            .iter()
            .filter_map(|arg| match arg {
                TermPattern::Exact(term) => Some(term.clone()),
                _ => None,
            })
            .collect(),
        LegalStatusPattern::PropositionStatus { proposition, .. } => match proposition {
            PropPattern::Ground(prop) => prop.arguments.clone(),
            PropPattern::Match { arguments, .. } => arguments
                .iter()
                .filter_map(|arg| match arg {
                    TermPattern::Exact(term) => Some(term.clone()),
                    _ => None,
                })
                .collect(),
        },
    }
}

fn status_subject_fields(
    status: &LegalStatusPattern,
    when_present: &Term,
) -> BTreeMap<String, Value> {
    let mut args = status_pattern_arg_terms(status);
    if args.is_empty() {
        match when_present {
            Term::Apply { args: xs, .. } | Term::Call { args: xs, .. } => args = xs.clone(),
            _ => {}
        }
    }
    fields_from_terms(&args)
}

fn fields_from_terms(args: &[Term]) -> BTreeMap<String, Value> {
    let mut fields = BTreeMap::new();
    for (i, arg) in args.iter().enumerate() {
        let Some(value) = term_as_status_arg_value(arg) else {
            continue;
        };
        if i == 0 {
            fields.insert("subject".into(), value.clone());
        }
        fields.insert(format!("_{i}"), value);
    }
    fields
}

fn term_as_status_arg_value(term: &Term) -> Option<Value> {
    match term {
        Term::Ident(s) => Some(Value::Entity(s.clone())),
        Term::String(s) => Some(Value::String(s.clone())),
        Term::Bool(b) => Some(Value::Bool(*b)),
        Term::Int(i) => Some(Value::Int(*i)),
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } if args.is_empty() => {
            Some(Value::Ctor {
                name: ctor.clone(),
                fields: BTreeMap::new(),
            })
        }
        _ => None,
    }
}

fn term_arg_names(term: &Term) -> Vec<String> {
    match term {
        Term::Ident(s) | Term::String(s) | Term::Binder(s) => vec![s.clone()],
        Term::Apply { args, .. } | Term::Call { args, .. } | Term::Set(args) => {
            args.iter().flat_map(term_arg_names).collect()
        }
        Term::Record(fields) => fields.values().flat_map(term_arg_names).collect(),
        _ => Vec::new(),
    }
}

fn closed_absent_value(
    when_closed_absent: &Term,
    ctor: &str,
    present_fields: BTreeMap<String, Value>,
) -> Value {
    let name = match when_closed_absent {
        Term::Apply { ctor, .. } | Term::Ident(ctor) | Term::Call { callee: ctor, .. } => {
            ctor.clone()
        }
        _ => format!("Not{ctor}"),
    };
    let fields = match when_closed_absent {
        Term::Apply { args, .. } | Term::Call { args, .. } if !args.is_empty() => {
            fields_from_terms(args)
        }
        _ => present_fields,
    };
    Value::Ctor { name, fields }
}

fn filed_need_args(subjects: &[String]) -> Vec<TermPattern> {
    if subjects.is_empty() {
        return vec![
            TermPattern::Wildcard,
            TermPattern::Wildcard,
            TermPattern::Wildcard,
        ];
    }
    let mut args: Vec<TermPattern> = subjects
        .iter()
        .map(|name| TermPattern::Exact(Term::Ident(name.clone())))
        .collect();
    while args.len() < 3 {
        args.push(TermPattern::Wildcard);
    }
    args
}

fn record_matches_subjects(value: &Value, requested: &[String]) -> bool {
    if requested.is_empty() {
        return true;
    }
    subjects_compatible(&named_subjects_in_value(value), requested)
}

fn named_subjects_in_value(value: &Value) -> Vec<String> {
    match value {
        Value::Entity(s) => vec![s.clone()],
        Value::Ctor { name, fields } if fields.is_empty() => vec![name.clone()],
        Value::Ctor { fields, .. } | Value::Map(fields) => subjects_from_fields(fields),
        Value::Set(items) => items.iter().flat_map(named_subjects_in_value).collect(),
        _ => Vec::new(),
    }
}

fn subjects_from_fields(fields: &BTreeMap<String, Value>) -> Vec<String> {
    const KEYS: &[&str] = &[
        "subject", "entity", "party", "person", "llc", "occupant", "holder",
    ];
    let mut names = Vec::new();
    for key in KEYS {
        if let Some(value) = fields.get(*key) {
            names.extend(entity_like_labels(value));
        }
    }
    if let Some(value) = fields.get("_0") {
        names.extend(entity_like_labels(value));
    }
    names
}

fn entity_like_labels(value: &Value) -> Vec<String> {
    match value {
        Value::Entity(s) | Value::String(s) => vec![s.clone()],
        Value::Ctor { name, fields } if fields.is_empty() => vec![name.clone()],
        Value::Ctor { fields, .. } | Value::Map(fields) => {
            fields.values().flat_map(entity_like_labels).collect()
        }
        _ => Vec::new(),
    }
}

fn determination_complies(det: &fidryn_core::case::CaseDetermination, subjects: &[String]) -> bool {
    if !det.established {
        return false;
    }
    let issue = parse_prop_issue(&det.issue);
    let protocol_ok = ident_eq(&det.protocol, "FormationCompliance")
        || ident_eq(&issue.predicate, "SubstantiallyComplies");
    if !protocol_ok {
        return false;
    }
    let named: Vec<String> = issue.arguments.iter().flat_map(term_arg_names).collect();
    subjects_compatible(&named, subjects)
}

fn subjects_compatible(named: &[String], requested: &[String]) -> bool {
    if requested.is_empty() || named.is_empty() {
        return true;
    }
    requested
        .iter()
        .any(|want| named.iter().any(|got| ident_eq(got, want)))
}

fn ident_eq(a: &str, b: &str) -> bool {
    a == b || a.eq_ignore_ascii_case(b)
}

fn eval_clause<H: Handler>(
    module: &CoreModule,
    clause: &ClauseSelector,
    case: &CaseRecord,
    handler: &mut H,
) -> Outcome<Value> {
    let trace = TraceId::of(b"provision_result");
    let selected = resolve_clause_name(module, clause, case);
    if let Some(value) = prevented_as_to(module, &selected) {
        return determinate(value, trace);
    }
    if ident_eq(&selected, "SpousalSupportWaiver") {
        return clause_enforceability(handler, trace);
    }
    Outcome::Suspended {
        requests: BTreeSet::from([OpenRequest::NeedJudgment {
            issue: PropTerm::new("EnforceableAgainst", vec![]),
            protocol: "PrenupEnforceability".into(),
        }]),
        trace,
    }
}

fn clause_enforceability<H: Handler>(handler: &mut H, trace: TraceId) -> Outcome<Value> {
    let req = OpenRequest::NeedJudgment {
        issue: PropTerm::new("EnforceableAgainst", vec![]),
        protocol: "PrenupEnforceability".into(),
    };
    match handler.handle(&req) {
        HandlerResult::Resume { value, .. } => determinate(value, trace),
        _ => {
            let mut requests = BTreeSet::new();
            requests.insert(req);
            Outcome::Suspended { requests, trace }
        }
    }
}

fn resolve_clause_name(
    module: &CoreModule,
    selector: &ClauseSelector,
    case: &CaseRecord,
) -> String {
    match selector {
        ClauseSelector::Instantiated { clause, arguments } => {
            if let Some(name) = declared_clause_name_for_id(module, clause) {
                return name;
            }
            if let Some(name) = binder_name_for_id(clause, case) {
                return name;
            }
            resolve_instantiated_from_args(module, arguments, case)
        }
        ClauseSelector::Bound { binder, .. } => bound_clause_name(case, binder),
    }
}

fn bound_clause_name(case: &CaseRecord, binder: &str) -> String {
    if let Some(raw) = case.facts.get(binder).and_then(value_as_clause_raw) {
        clause_ident(&raw).to_owned()
    } else {
        clause_ident(binder).to_owned()
    }
}

fn binder_name_for_id(id: &ClauseId, case: &CaseRecord) -> Option<String> {
    for key in case.facts.keys() {
        if ClauseId::of(key.as_bytes()) == *id {
            return Some(bound_clause_name(case, key));
        }
    }
    for binder in ["provision", "clause"] {
        if ClauseId::of(binder.as_bytes()) == *id {
            return Some(bound_clause_name(case, binder));
        }
    }
    None
}

fn declared_clause_name_for_id(module: &CoreModule, id: &ClauseId) -> Option<String> {
    declared_clauses(module).into_iter().find_map(|clause| {
        if clause.id == *id || ClauseId::of(clause.name.as_bytes()) == *id {
            Some(clause.name.clone())
        } else {
            None
        }
    })
}

fn resolve_instantiated_from_args(
    module: &CoreModule,
    arguments: &[Term],
    case: &CaseRecord,
) -> String {
    let mut names = Vec::new();
    for arg in arguments {
        collect_term_names(arg, &mut names);
    }
    let declared: Vec<String> = declared_clauses(module)
        .into_iter()
        .map(|clause| clause.name.clone())
        .collect();
    for name in &names {
        if is_known_clause_name(&declared, name) {
            return name.clone();
        }
    }
    for name in &names {
        if let Some(raw) = case.facts.get(name).and_then(value_as_clause_raw) {
            let ident = clause_ident(&raw);
            if is_known_clause_name(&declared, ident) {
                return ident.to_owned();
            }
        }
    }
    if names.iter().any(|name| ident_eq(name, "provision")) {
        return bound_clause_name(case, "provision");
    }
    String::new()
}

fn is_known_clause_name(declared: &[String], name: &str) -> bool {
    declared.iter().any(|declared| ident_eq(declared, name))
        || ident_eq(name, "ChildSupportWaiver")
        || ident_eq(name, "SpousalSupportWaiver")
}

fn collect_term_names(term: &Term, out: &mut Vec<String>) {
    match term {
        Term::Ident(s) | Term::String(s) | Term::Binder(s) => out.push(s.clone()),
        Term::Apply { ctor, args } => {
            out.push(ctor.clone());
            for arg in args {
                collect_term_names(arg, out);
            }
        }
        Term::Call { callee, args } => {
            out.push(callee.clone());
            for arg in args {
                collect_term_names(arg, out);
            }
        }
        Term::Set(args) => {
            for arg in args {
                collect_term_names(arg, out);
            }
        }
        Term::Record(fields) => {
            for value in fields.values() {
                collect_term_names(value, out);
            }
        }
        Term::Binary { left, right, .. } => {
            collect_term_names(left, out);
            collect_term_names(right, out);
        }
        Term::If { cond, then, else_ } => {
            collect_term_names(cond, out);
            collect_term_names(then, out);
            collect_term_names(else_, out);
        }
        Term::Field { base, .. } => collect_term_names(base, out),
        _ => {}
    }
}

fn declared_clauses(module: &CoreModule) -> Vec<&CoreClause> {
    module
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            CoreDecl::Clause(clause) => Some(clause),
            _ => None,
        })
        .collect()
}

fn declared_doctrines(module: &CoreModule) -> Vec<&CoreConflictDoctrine> {
    module
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            CoreDecl::ConflictDoctrine(doctrine) => Some(doctrine),
            _ => None,
        })
        .collect()
}

fn prevented_as_to(module: &CoreModule, clause_name: &str) -> Option<Value> {
    if clause_name.is_empty() {
        return None;
    }
    let clause_id = declared_clauses(module)
        .into_iter()
        .find(|clause| ident_eq(&clause.name, clause_name))
        .map(|clause| clause.id)
        .unwrap_or_else(|| ClauseId::of(clause_name.as_bytes()));
    for doctrine in declared_doctrines(module) {
        let defeats = doctrine
            .defeats
            .iter()
            .any(|target| matches!(target, ConflictTarget::Clause(id) if *id == clause_id));
        if defeats {
            return Some(prevented_value(&doctrine.name, &doctrine_right(doctrine)));
        }
    }
    if ident_eq(clause_name, "ChildSupportWaiver") {
        return Some(prevented_value(
            "ChildSupportCannotBeAdverselyAffected",
            "ChildSupportRight",
        ));
    }
    None
}

fn doctrine_right(doctrine: &CoreConflictDoctrine) -> String {
    match &doctrine.as_to {
        Some(LegalEffectPattern::Affect(LegalSubjectPattern::Exact(name))) => name.clone(),
        Some(LegalEffectPattern::Establish(LegalStatusPattern::InstitutionalStatus {
            constructor,
            ..
        })) => constructor.clone(),
        Some(LegalEffectPattern::Establish(LegalStatusPattern::PropositionStatus {
            proposition,
            ..
        })) => match proposition {
            PropPattern::Ground(prop) => prop.predicate.clone(),
            PropPattern::Match { predicate, .. } => predicate.clone(),
        },
        _ => String::new(),
    }
}

fn prevented_value(doctrine: &str, right: &str) -> Value {
    Value::Ctor {
        name: "PreventedAsTo".into(),
        fields: BTreeMap::from([
            ("right".into(), Value::String(right.into())),
            ("doctrine".into(), Value::String(doctrine.into())),
        ]),
    }
}

fn clause_ident(raw: &str) -> &str {
    let s = raw.trim();
    let s = s.rsplit("::").next().unwrap_or(s).trim();
    match s.find('(') {
        Some(i) => s[..i].trim(),
        None => s,
    }
}

fn value_as_clause_raw(value: &Value) -> Option<String> {
    match value {
        Value::String(s) | Value::Entity(s) => Some(s.clone()),
        Value::Ctor { name, .. } => Some(name.clone()),
        _ => None,
    }
}

fn eval_run_decision<H: Handler>(
    module: &CoreModule,
    decision: &str,
    arguments: &[Term],
    case: &CaseRecord,
    handler: &mut H,
    ctx: &RunContext,
    derived: &DerivedWorld,
) -> Result<Outcome<Value>, EngineError> {
    let Some(decl) = find_decision(module, decision) else {
        return Err(unsupported(format!("unknown decision `{decision}`")));
    };
    if !decl.binders.is_empty() && decl.binders.len() != arguments.len() {
        return Err(EngineError::InvalidInput(format!(
            "decision `{decision}` expects {} argument(s), got {}",
            decl.binders.len(),
            arguments.len()
        )));
    }
    let trace = TraceId::of(decision.as_bytes());
    let mut open = BTreeSet::new();
    for req in &decl.requirements {
        if let Some(halt) = gather_unmet_requirement(req, case, ctx, derived, handler, &mut open) {
            return Ok(halt);
        }
    }
    if !open.is_empty() {
        return Ok(Outcome::Suspended {
            requests: open,
            trace,
        });
    }
    if let Some(ret) = &decl.declared_result
        && let Some(value) = decision_result_value(&ret.expression, case)
    {
        return Ok(determinate(value, trace));
    }
    if let Some(recorded) = case.decisions.get(decision) {
        return Ok(determinate(Value::String(recorded.clone()), trace));
    }
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedJudgment {
        issue: PropTerm::new(decision, vec![]),
        protocol: decision.to_owned(),
    });
    Ok(Outcome::Suspended { requests, trace })
}

fn decision_result_value(term: &Term, case: &CaseRecord) -> Option<Value> {
    if let Some(value) = term_as_value_literal(term) {
        match &value {
            Value::String(name) => case
                .facts
                .get(name)
                .cloned()
                .or_else(|| case.facts.get("proposed_disposition").cloned())
                .or(Some(value)),
            other => Some(other.clone()),
        }
    } else {
        None
    }
}

fn gather_unmet_requirement<H: Handler>(
    guard: &Guard,
    case: &CaseRecord,
    ctx: &RunContext,
    derived: &DerivedWorld,
    handler: &mut H,
    open: &mut BTreeSet<OpenRequest>,
) -> Option<Outcome<Value>> {
    match guard {
        Guard::Satisfied => None,
        Guard::Request(req) => push_handler_request(handler, req, open),
        Guard::Observed { schema, .. } => {
            let seen = case
                .evidence
                .iter()
                .any(|e| e.schema == *schema && e.observed_at <= ctx.record_time);
            if seen {
                return None;
            }
            let req = OpenRequest::NeedEvidence {
                issue: PropPattern::Ground(PropTerm::new(schema.clone(), vec![])),
                schema: schema.clone(),
            };
            push_handler_request(handler, &req, open)
        }
        Guard::And(gs) => {
            for g in gs {
                if let Some(halt) = gather_unmet_requirement(g, case, ctx, derived, handler, open) {
                    return Some(halt);
                }
            }
            None
        }
        Guard::Or(gs) => {
            let mut nested = BTreeSet::new();
            for g in gs {
                let mut part = BTreeSet::new();
                if let Some(halt) =
                    gather_unmet_requirement(g, case, ctx, derived, handler, &mut part)
                {
                    return Some(halt);
                }
                if part.is_empty() {
                    return None;
                }
                nested.extend(part);
            }
            open.extend(nested);
            None
        }
        Guard::Operative(prop, _) | Guard::Derived(prop) => {
            if derived.holds(prop) {
                None
            } else {
                open.insert(OpenRequest::NeedJudgment {
                    issue: prop.clone(),
                    protocol: prop.predicate.clone(),
                });
                None
            }
        }
        Guard::Compare { op, left, right } => {
            match (term_as_value_literal(left), term_as_value_literal(right)) {
                (Some(a), Some(b)) if compare_values(*op, &a, &b) => None,
                _ => {
                    open.insert(OpenRequest::NeedJudgment {
                        issue: PropTerm::new("requirement", vec![]),
                        protocol: "decision".into(),
                    });
                    None
                }
            }
        }
        Guard::Not(inner) => {
            let mut part = BTreeSet::new();
            if let Some(halt) =
                gather_unmet_requirement(inner, case, ctx, derived, handler, &mut part)
            {
                return Some(halt);
            }
            if part.is_empty() {
                open.insert(OpenRequest::NeedJudgment {
                    issue: PropTerm::new("requirement", vec![]),
                    protocol: "decision".into(),
                });
            }
            None
        }
        Guard::CompletedAct(name) | Guard::EffectiveAct(name) => {
            let done = case.facts.contains_key(name)
                || case
                    .determinations
                    .iter()
                    .any(|d| d.issue == *name && d.established);
            if done {
                None
            } else {
                open.insert(OpenRequest::NeedJudgment {
                    issue: PropTerm::new(name.clone(), vec![]),
                    protocol: "decision".into(),
                });
                None
            }
        }
    }
}

fn push_handler_request<H: Handler>(
    handler: &mut H,
    req: &OpenRequest,
    open: &mut BTreeSet<OpenRequest>,
) -> Option<Outcome<Value>> {
    match handler.handle(req) {
        HandlerResult::Resume { .. } => None,
        HandlerResult::Suspend { requests, .. } => {
            if requests.is_empty() {
                open.insert(req.clone());
            } else {
                open.extend(requests);
            }
            None
        }
        HandlerResult::Halt { reason, .. } => {
            Some(halt_to_outcome(reason, &[], TraceId::of(b"decision")))
        }
    }
}

fn compare_values(op: CompareOp, left: &Value, right: &Value) -> bool {
    match op {
        CompareOp::Eq => left == right,
        CompareOp::Ne => left != right,
        CompareOp::Lt | CompareOp::Le | CompareOp::Gt | CompareOp::Ge => false,
    }
}

fn term_as_value_literal(term: &Term) -> Option<Value> {
    match term {
        Term::Bool(b) => Some(Value::Bool(*b)),
        Term::Int(i) => Some(Value::Int(*i)),
        Term::Decimal(d) => Some(Value::Decimal(*d)),
        Term::String(s) => Some(Value::String(s.clone())),
        Term::Ident(s) => Some(Value::String(s.clone())),
        Term::Apply { ctor, args } if args.is_empty() => Some(Value::Ctor {
            name: ctor.clone(),
            fields: BTreeMap::new(),
        }),
        _ => None,
    }
}

fn occupancy_from_records(case: &CaseRecord, office: &str, ctx: &RunContext) -> Vec<Occupancy> {
    case.evidence
        .iter()
        .filter(|e| e.schema == "OccupancyRecord" && e.observed_at <= ctx.record_time)
        .filter_map(|e| {
            let (person, rec_office) = occupancy_fields(&e.value)?;
            let rec_office = rec_office.unwrap_or_else(|| office.to_owned());
            if !office_matches(&rec_office, office) {
                return None;
            }
            Some(Occupancy {
                person,
                office: rec_office,
                mode: StatusMode::Established,
                valid_time: Interval {
                    start: fidryn_core::time::Bound::Inclusive(e.observed_at),
                    end: fidryn_core::time::Bound::PosInf,
                },
                record_time: e.observed_at,
            })
        })
        .collect()
}

fn occupancy_fields(value: &Value) -> Option<(String, Option<String>)> {
    match value {
        Value::Entity(name) | Value::String(name) => Some((name.clone(), None)),
        Value::Map(fields) | Value::Ctor { fields, .. } => {
            let person = fields
                .get("person")
                .or_else(|| fields.get("occupant"))
                .or_else(|| fields.get("holder"))
                .and_then(|v| strings_from_value(v).into_iter().next())?;
            let rec_office = fields
                .get("office")
                .and_then(|v| strings_from_value(v).into_iter().next());
            Some((person, rec_office))
        }
        _ => None,
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
    use fidryn_core::case::CaseDetermination;
    use fidryn_core::effects::HandlerResult;
    use fidryn_core::ir::{
        Consequence, CoreClause, CoreDecision, CoreDuty, CoreEffect, CoreEffectDecl, CoreEffectOp,
        CoreEntity, CoreInterpretationFamily, CoreNomination, CoreProposition, CoreQuery, CoreRule,
        DecisionReturn, DeclaredDecisionResult, RuleKind,
    };
    use fidryn_core::{
        EffectId, ModuleId, PrimitiveType, Sort, SourceManifestId, SourceSnapshotId, Type,
    };

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

    struct Scripted {
        resume: BTreeSet<String>,
        halt: BTreeSet<String>,
        counts: BTreeMap<String, usize>,
    }

    impl Scripted {
        fn resume_only(schemas: &[&str]) -> Self {
            Self {
                resume: schemas.iter().map(|s| (*s).to_owned()).collect(),
                halt: BTreeSet::new(),
                counts: BTreeMap::new(),
            }
        }

        fn halt_on(schema: &str) -> Self {
            Self {
                resume: BTreeSet::new(),
                halt: BTreeSet::from([schema.to_owned()]),
                counts: BTreeMap::new(),
            }
        }

        fn count(&self, schema: &str) -> usize {
            self.counts.get(schema).copied().unwrap_or(0)
        }

        fn bump(&mut self, schema: &str) {
            *self.counts.entry(schema.to_owned()).or_insert(0) += 1;
        }
    }

    impl Handler for Scripted {
        fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
            let schema = match request {
                OpenRequest::NeedEvidence { schema, .. } => schema.clone(),
                _ => "other".into(),
            };
            self.bump(&schema);
            if self.halt.contains(&schema) {
                return HandlerResult::Halt {
                    reason: HaltReason::OutsideCompetence {
                        request: request.clone(),
                        reason: "invalid response".into(),
                    },
                    trace_fragment: "halt".into(),
                };
            }
            if self.resume.contains(&schema) {
                return HandlerResult::Resume {
                    value: Value::Bool(true),
                    trace_fragment: format!("resume:{schema}"),
                };
            }
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
            self.handle_observe(request)
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

    fn module_with_occupant() -> CoreModule {
        let office = "TrusteeOf(BRT)";
        let family = CoreDecl::InterpretationFamily(CoreInterpretationFamily {
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
                (
                    "BobOnly".into(),
                    vec![
                        eligible_def("Alice", office, false),
                        eligible_def("Bob", office, true),
                    ],
                ),
            ],
            meta: test_meta("SuccessorEligibility"),
        });
        let mut module = module_with_plan_decls(
            "acting_trustee",
            QueryPlan::UniqueOccupant {
                office: Term::Ident(office.into()),
            },
            vec![family],
        );
        module.nominations = vec![
            CoreNomination {
                candidate: "Alice".into(),
                office: office.into(),
                rank: 1,
            },
            CoreNomination {
                candidate: "Bob".into(),
                office: office.into(),
                rank: 2,
            },
        ];
        module
    }

    fn family_mut(module: &mut CoreModule) -> &mut CoreInterpretationFamily {
        module
            .declarations
            .iter_mut()
            .find_map(|decl| match decl {
                CoreDecl::InterpretationFamily(family) => Some(family),
                _ => None,
            })
            .expect("family")
    }

    fn rename_family_labels(module: &mut CoreModule, pairs: &[(&str, &str)]) {
        let family = family_mut(module);
        for (from, to) in pairs {
            for (label, _) in &mut family.alternatives {
                if label == from {
                    *label = (*to).to_owned();
                }
            }
        }
    }

    fn set_family_defs(module: &mut CoreModule, label: &str, defs: Vec<(PropTerm, bool)>) {
        let family = family_mut(module);
        if let Some((_, slot)) = family
            .alternatives
            .iter_mut()
            .find(|(name, _)| name == label)
        {
            *slot = defs;
        }
    }

    fn nomination(candidate: &str, office: &str, rank: i64) -> CoreNomination {
        CoreNomination {
            candidate: candidate.into(),
            office: office.into(),
            rank,
        }
    }

    fn module_with_two_offices() -> CoreModule {
        let trustee = "TrusteeOf(BRT)";
        let executor = "ExecutorOf(Est)";
        let trustee_family = CoreDecl::InterpretationFamily(CoreInterpretationFamily {
            id: NodeId::of(b"TrusteeEligibility"),
            name: "TrusteeEligibility".into(),
            source: Term::Ident("TrusteeEligibility".into()),
            alternatives: vec![(
                "HighRank".into(),
                vec![
                    eligible_def("Alice", trustee, true),
                    eligible_def("Bob", trustee, false),
                ],
            )],
            meta: test_meta("TrusteeEligibility"),
        });
        let executor_family = CoreDecl::InterpretationFamily(CoreInterpretationFamily {
            id: NodeId::of(b"ExecutorEligibility"),
            name: "ExecutorEligibility".into(),
            source: Term::Ident("ExecutorEligibility".into()),
            alternatives: vec![(
                "NextOfKin".into(),
                vec![
                    eligible_def("Dana", executor, true),
                    eligible_def("Eve", executor, false),
                ],
            )],
            meta: test_meta("ExecutorEligibility"),
        });
        let mut module = module_with_plan_decls(
            "acting_trustee",
            QueryPlan::UniqueOccupant {
                office: Term::Ident(trustee.into()),
            },
            vec![trustee_family, executor_family],
        );
        module.queries.push(CoreQuery {
            id: NodeId::of(b"acting_executor"),
            name: "acting_executor".into(),
            binders: Vec::new(),
            result_type: Type::Sort(Sort::LegalPerson),
            effects: BTreeSet::new(),
            automatic: false,
            plan: QueryPlan::UniqueOccupant {
                office: Term::Ident(executor.into()),
            },
            meta: test_meta("acting_executor"),
        });
        module.nominations = vec![
            nomination("Alice", trustee, 1),
            nomination("Bob", trustee, 2),
            nomination("Dana", executor, 1),
            nomination("Eve", executor, 2),
        ];
        module
    }

    fn two_office_case() -> CaseRecord {
        let mut case = successor_case(2, None);
        case.facts.insert("dana_accepted".into(), Value::Bool(true));
        case.facts.insert("eve_accepted".into(), Value::Bool(true));
        case.admissible_completions
            .interpretations
            .insert("TrusteeEligibility".into(), vec!["HighRank".into()]);
        case.admissible_completions
            .interpretations
            .insert("ExecutorEligibility".into(), vec!["NextOfKin".into()]);
        case.interpretations
            .insert("TrusteeEligibility".into(), "HighRank".into());
        case.interpretations
            .insert("ExecutorEligibility".into(), "NextOfKin".into());
        case
    }

    fn successor_case(certs: usize, interpretation: Option<&str>) -> CaseRecord {
        let t = fidryn_core::Instant::parse("2026-08-23T12:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts
            .insert("alice_accepted".into(), Value::Bool(true));
        case.facts.insert("bob_accepted".into(), Value::Bool(true));
        case.evidence.push(EvidenceItem {
            schema: "OccupancyRecord".into(),
            value: Value::String("Pat".into()),
            observed_at: t,
        });
        for i in 0..certs {
            case.evidence.push(EvidenceItem {
                schema: "PhysicianCertificate".into(),
                value: Value::String(format!("c{i}")),
                observed_at: t,
            });
        }
        case.admissible_completions.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["I1".into(), "I2".into()],
        );
        if let Some(label) = interpretation {
            case.interpretations
                .insert("SuccessorEligibility".into(), label.to_owned());
        }
        case
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

    fn module_with_plan(name: &str, plan: QueryPlan) -> CoreModule {
        module_with_plan_decls(name, plan, Vec::new())
    }

    fn module_with_plan_decls(
        name: &str,
        plan: QueryPlan,
        declarations: Vec<CoreDecl>,
    ) -> CoreModule {
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
                id: NodeId::of(name.as_bytes()),
                name: name.into(),
                binders: Vec::new(),
                result_type: Type::Sort(Sort::LegalPerson),
                effects: BTreeSet::new(),
                automatic: false,
                plan,
                meta: test_meta(name),
            }],
            verifications: Vec::new(),
            assertions: Vec::new(),
        }
    }

    fn tax_function(name: &str, fuel: Option<u32>) -> CoreFunction {
        CoreFunction {
            id: NodeId::of(name.as_bytes()),
            name: name.into(),
            params: vec![(
                "amount".into(),
                Type::Primitive(PrimitiveType::Money {
                    currency: "USD".into(),
                }),
            )],
            result: Type::Primitive(PrimitiveType::Money {
                currency: "USD".into(),
            }),
            effects: BTreeSet::new(),
            is_calc: true,
            fuel,
            body: None,
            meta: test_meta(name),
        }
    }

    fn run_plan(plan: QueryPlan, case: &CaseRecord, handler: &mut impl Handler) -> Outcome<Value> {
        run_module(&module_with_plan("q", plan), "q", case, handler).expect("evaluate")
    }

    fn closed_int_set(xs: &[i64]) -> Term {
        Term::Set(xs.iter().copied().map(Term::Int).collect())
    }

    fn quantifier_apply(kind: &str, binder: &str, domain: Term, body: Term) -> Term {
        Term::Apply {
            ctor: kind.into(),
            args: vec![Term::Binder(binder.into()), domain, body],
        }
    }

    fn eq_idents(left: &str, right: &str) -> Term {
        Term::Apply {
            ctor: "==".into(),
            args: vec![Term::Ident(left.into()), Term::Ident(right.into())],
        }
    }

    fn assert_determinate_bool(out: Outcome<Value>, expected: bool) {
        match out {
            Outcome::Determinate {
                value: Value::Bool(value),
                ..
            } if value == expected => {}
            other => panic!("{other:?}"),
        }
    }

    fn assert_needs_closure_record(out: Outcome<Value>) {
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedEvidence { schema, .. } if schema == "ClosureRecord"
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    fn run_module(
        module: &CoreModule,
        query: &str,
        case: &CaseRecord,
        handler: &mut impl Handler,
    ) -> Result<Outcome<Value>, EngineError> {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        evaluate(
            module,
            &QueryName::from(query),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            handler,
            case,
        )
    }

    fn run_succession(module: &CoreModule, query: &str, case: &CaseRecord) -> Outcome<Value> {
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        evaluate(
            module,
            &QueryName::from(query),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            &mut Refusing,
            case,
        )
        .expect("evaluate")
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
    fn one_certificate_without_checked_cert_is_not_determinate() {
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
        )
        .expect("evaluate");
        assert!(
            !matches!(out, Outcome::Determinate { .. }),
            "ignored issues without a checked certificate must not be determinate: {out:?}"
        );
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    #[test]
    fn missing_occupancy_suspends_without_defaulting_bryan() {
        let module = module_with_occupant();
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mut h = Refusing;
        let out = evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            &mut h,
            &CaseRecord::default(),
        )
        .expect("evaluate");
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedEvidence { schema, .. } if schema == "OccupancyRecord"
                )));
            }
            other => panic!("{other:?}"),
        }
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
    fn evaluate_tax_on_is_closed_form() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "ordinary_income_tax".into(),
            args: vec![Term::Ident("amount".into())],
        });
        let mut case = CaseRecord::default();
        case.facts.insert("amount".into(), Value::Int(10_000));
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax",
                None,
            ))],
        );
        let out = run_module(&module, "q", &case, &mut Refusing).expect("evaluate");
        match out {
            Outcome::Determinate { value, .. } => match value {
                Value::Decimal(d) => assert!(d > Decimal::ZERO),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn ordinary_income_tax_is_monotonic_across_the_fourth_bracket() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "ordinary_income_tax".into(),
            args: vec![Term::Ident("amount".into())],
        });
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax",
                None,
            ))],
        );
        let tax = |amount: Decimal| {
            let mut case = CaseRecord::default();
            case.facts.insert("amount".into(), Value::Decimal(amount));
            match run_module(&module, "q", &case, &mut Refusing).expect("evaluate") {
                Outcome::Determinate {
                    value: Value::Decimal(d),
                    ..
                } => d,
                other => panic!("{other:?}"),
            }
        };
        let at_top = tax(Decimal::from(103_350));
        let just_over = tax(Decimal::new(10_335_001, 2));
        assert!(just_over >= at_top, "{just_over} < {at_top}");
        assert_eq!(at_top, Decimal::new(176_510, 1));
    }

    #[test]
    fn missing_money_is_invalid_input() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "ordinary_income_tax".into(),
            args: vec![],
        });
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax",
                None,
            ))],
        );
        let err = run_module(&module, "q", &CaseRecord::default(), &mut Refusing).unwrap_err();
        assert!(
            matches!(err, EngineError::InvalidInput(ref msg) if msg.contains("amount") || msg.contains("money"))
        );
    }

    #[test]
    fn fuel_exhaustion_is_engine_error() {
        let plan = QueryPlan::Evaluate(Term::Ident("ordinary_income_tax".into()));
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax",
                Some(0),
            ))],
        );
        let mut case = CaseRecord::default();
        case.facts.insert("amount".into(), Value::Int(10_000));
        let err = run_module(&module, "q", &case, &mut Refusing).unwrap_err();
        assert!(matches!(err, EngineError::FuelExhausted { .. }));
    }

    #[test]
    fn unknown_query_is_engine_error() {
        let module = module_with_plan("q", QueryPlan::Evaluate(Term::Bool(true)));
        let err =
            run_module(&module, "missing", &CaseRecord::default(), &mut Refusing).unwrap_err();
        assert!(matches!(err, EngineError::UnknownQuery(ref query) if query == "missing"));
    }

    #[test]
    fn evaluate_judgment_without_record_suspends() {
        let plan = QueryPlan::Evaluate(Term::Ident("judgment".into()));
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    #[test]
    fn evaluate_other_term_is_unsupported() {
        let plan = QueryPlan::Evaluate(Term::Ident("plain".into()));
        let err = run_module(
            &module_with_plan("q", plan),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .unwrap_err();
        assert!(matches!(err, EngineError::Unsupported(_)));
    }

    #[test]
    fn evaluate_binary_and_call_executes_function_body() {
        let out = run_plan(
            QueryPlan::Evaluate(Term::Binary {
                op: BinOp::Add,
                left: Box::new(Term::Int(2)),
                right: Box::new(Term::Int(3)),
            }),
            &CaseRecord::default(),
            &mut Refusing,
        );
        match out {
            Outcome::Determinate {
                value: Value::Int(5),
                ..
            } => {}
            other => panic!("{other:?}"),
        }

        let mut function = tax_function("inc", None);
        function.body = Some(Term::Binary {
            op: BinOp::Add,
            left: Box::new(Term::Ident("amount".into())),
            right: Box::new(Term::Int(1)),
        });
        let plan = QueryPlan::Evaluate(Term::Call {
            callee: "inc".into(),
            args: vec![Term::Int(10)],
        });
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Function(function)]);
        match run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::Int(11),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_bool_true_is_determinate() {
        let out = run_plan(
            QueryPlan::Evaluate(Term::Bool(true)),
            &CaseRecord::default(),
            &mut Refusing,
        );
        match out {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn checked_add_does_not_wrap_to_zero() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "+".into(),
            args: vec![Term::Int(i64::MAX), Term::Int(1)],
        });
        let err = run_module(
            &module_with_plan("q", plan),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .unwrap_err();
        assert!(matches!(err, EngineError::InvalidInput(_)));
    }

    #[test]
    fn filing_transport_receipt_is_not_official_filing() {
        let plan = QueryPlan::StatusOf {
            status: LegalStatusPattern::InstitutionalStatus {
                constructor: "FormedLLC".into(),
                arguments: Vec::new(),
            },
            when_present: Term::unit_ctor("FormedLLC"),
            when_closed_absent: Term::unit_ctor("NotFormedLLC"),
        };
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "FilingTransportReceipt".into(),
            value: Value::String("tx".into()),
            observed_at: fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap(),
        });
        case.closures.push(fidryn_core::ClosureRecord {
            domain: "filings".into(),
            closed: false,
        });
        let out = run_plan(plan, &case, &mut Refusing);
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    #[test]
    fn status_absent_without_closed_world_suspends() {
        let plan = QueryPlan::StatusOf {
            status: LegalStatusPattern::InstitutionalStatus {
                constructor: "FormedLLC".into(),
                arguments: Vec::new(),
            },
            when_present: Term::unit_ctor("FormedLLC"),
            when_closed_absent: Term::unit_ctor("NotFormedLLC"),
        };
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    #[test]
    fn evidence_after_known_at_does_not_count() {
        let plan = QueryPlan::StatusOf {
            status: LegalStatusPattern::InstitutionalStatus {
                constructor: "FormedLLC".into(),
                arguments: Vec::new(),
            },
            when_present: Term::unit_ctor("FormedLLC"),
            when_closed_absent: Term::unit_ctor("NotFormedLLC"),
        };
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "OfficialFilingRecord".into(),
            value: Value::String("filing-1".into()),
            observed_at: fidryn_core::Instant::parse("2026-06-01T00:00:00Z").unwrap(),
        });
        case.determinations
            .push(fidryn_core::case::CaseDetermination {
                issue: "SubstantiallyComplies".into(),
                protocol: "FormationCompliance".into(),
                established: true,
                decider: "CompetentFormationAuthority".into(),
                recorded_at: None,
            });
        let out = run_plan(plan, &case, &mut Refusing);
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    fn formed_llc_status(subject: &str) -> QueryPlan {
        QueryPlan::StatusOf {
            status: LegalStatusPattern::InstitutionalStatus {
                constructor: "FormedLLC".into(),
                arguments: vec![TermPattern::Exact(Term::Ident(subject.into()))],
            },
            when_present: Term::Apply {
                ctor: "FormedLLC".into(),
                args: vec![Term::Ident(subject.into())],
            },
            when_closed_absent: Term::Apply {
                ctor: "NotFormedLLC".into(),
                args: vec![Term::Ident(subject.into())],
            },
        }
    }

    #[test]
    fn status_of_record_about_another_entity_does_not_form_requested_subject() {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "OfficialFilingRecord".into(),
            value: Value::Map(BTreeMap::from([(
                "entity".into(),
                Value::Entity("HarborRobotics".into()),
            )])),
            observed_at: t,
        });
        case.determinations.push(CaseDetermination {
            issue: "SubstantiallyComplies(HarborRobotics)".into(),
            protocol: "FormationCompliance".into(),
            established: true,
            decider: "CompetentFormationAuthority".into(),
            recorded_at: Some(t),
        });
        let out = run_plan(formed_llc_status("Acme"), &case, &mut Refusing);
        assert!(
            matches!(out, Outcome::Suspended { .. }),
            "records about HarborRobotics must not establish FormedLLC(Acme): {out:?}"
        );
    }

    #[test]
    fn status_of_keeps_requested_subject_fields_when_present() {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "OfficialFilingRecord".into(),
            value: Value::String("filing-1".into()),
            observed_at: t,
        });
        case.determinations.push(CaseDetermination {
            issue: "SubstantiallyComplies".into(),
            protocol: "FormationCompliance".into(),
            established: true,
            decider: "CompetentFormationAuthority".into(),
            recorded_at: Some(t),
        });
        let out = run_plan(formed_llc_status("HarborRobotics"), &case, &mut Refusing);
        match out {
            Outcome::Determinate {
                value: Value::Ctor { name, fields },
                ..
            } => {
                assert_eq!(name, "FormedLLC");
                assert_eq!(
                    fields.get("subject"),
                    Some(&Value::Entity("HarborRobotics".into()))
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn status_of_future_determination_does_not_count() {
        let known = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let later = fidryn_core::Instant::parse("2026-06-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "OfficialFilingRecord".into(),
            value: Value::String("filing-1".into()),
            observed_at: known,
        });
        case.determinations.push(CaseDetermination {
            issue: "SubstantiallyComplies".into(),
            protocol: "FormationCompliance".into(),
            established: true,
            decider: "CompetentFormationAuthority".into(),
            recorded_at: Some(later),
        });
        let out = run_plan(formed_llc_status("HarborRobotics"), &case, &mut Refusing);
        assert!(
            matches!(out, Outcome::Suspended { .. }),
            "a determination recorded after known_at must not form the entity: {out:?}"
        );
    }

    fn bound_clause_plan() -> QueryPlan {
        QueryPlan::EvaluateClause {
            clause: ClauseSelector::Bound {
                binder: "provision".into(),
                module: ModuleId::of(b"test"),
            },
            context: String::new(),
            result: Term::Wildcard,
        }
    }

    #[test]
    fn evaluate_clause_matches_declared_child_support_waiver_exactly() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "provision".into(),
            Value::String("Examples.AvaNoahPrenup@0.1.0::ChildSupportWaiver()".into()),
        );
        match run_plan(bound_clause_plan(), &case, &mut Refusing) {
            Outcome::Determinate {
                value: Value::Ctor { name, fields },
                ..
            } => {
                assert_eq!(name, "PreventedAsTo");
                assert_eq!(
                    fields.get("doctrine"),
                    Some(&Value::String(
                        "ChildSupportCannotBeAdverselyAffected".into()
                    ))
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_clause_ignores_incidental_child_support_substring() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "provision".into(),
            Value::String("MemoAboutChildSupportAndTaxes".into()),
        );
        let out = run_plan(bound_clause_plan(), &case, &mut Refusing);
        match &out {
            Outcome::Determinate {
                value: Value::Ctor { name, .. },
                ..
            } if name == "PreventedAsTo" => {
                panic!("substring ChildSupport must not prevent: {out:?}");
            }
            Outcome::Suspended { .. } => {}
            other => panic!("{other:?}"),
        }
    }

    fn instantiated_clause_plan(name: &str) -> QueryPlan {
        QueryPlan::EvaluateClause {
            clause: ClauseSelector::Instantiated {
                clause: ClauseId::of(name.as_bytes()),
                arguments: Vec::new(),
            },
            context: String::new(),
            result: Term::Wildcard,
        }
    }

    #[test]
    fn instantiated_provision_binder_uses_exact_declared_clause_fact() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "provision".into(),
            Value::String("Examples.AvaNoahPrenup@0.1.0::ChildSupportWaiver()".into()),
        );
        match run_plan(instantiated_clause_plan("provision"), &case, &mut Refusing) {
            Outcome::Determinate {
                value: Value::Ctor { name, fields },
                ..
            } => {
                assert_eq!(name, "PreventedAsTo");
                assert_eq!(
                    fields.get("doctrine"),
                    Some(&Value::String(
                        "ChildSupportCannotBeAdverselyAffected".into()
                    ))
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn instantiated_apply_tree_recovers_bound_provision_clause() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "provision".into(),
            Value::String("Examples.AvaNoahPrenup@0.1.0::ChildSupportWaiver()".into()),
        );
        let plan = QueryPlan::EvaluateClause {
            clause: ClauseSelector::Instantiated {
                clause: ClauseId::of(b"apply"),
                arguments: vec![Term::Apply {
                    ctor: "provision".into(),
                    args: vec![Term::Ident("context".into())],
                }],
            },
            context: String::new(),
            result: Term::Wildcard,
        };
        match run_plan(plan, &case, &mut Refusing) {
            Outcome::Determinate {
                value: Value::Ctor { name, fields },
                ..
            } => {
                assert_eq!(name, "PreventedAsTo");
                assert_eq!(
                    fields.get("doctrine"),
                    Some(&Value::String(
                        "ChildSupportCannotBeAdverselyAffected".into()
                    ))
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn instantiated_unrelated_clause_does_not_use_provision_substring() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "provision".into(),
            Value::String("Examples.AvaNoahPrenup@0.1.0::ChildSupportWaiver()".into()),
        );
        let out = run_plan(
            instantiated_clause_plan("SeparateProperty"),
            &case,
            &mut Refusing,
        );
        match &out {
            Outcome::Determinate {
                value: Value::Ctor { name, .. },
                ..
            } if name == "PreventedAsTo" => {
                panic!("instantiated SeparateProperty must not become ChildSupport: {out:?}");
            }
            Outcome::Suspended { .. } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn instantiated_clause_selector_uses_declared_clause_not_literal_clause() {
        let clause_id = ClauseId::of(b"ChildSupportWaiver");
        let plan = QueryPlan::EvaluateClause {
            clause: ClauseSelector::Instantiated {
                clause: clause_id,
                arguments: Vec::new(),
            },
            context: String::new(),
            result: Term::Wildcard,
        };
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![
                CoreDecl::Clause(CoreClause {
                    id: clause_id,
                    name: "ChildSupportWaiver".into(),
                    rules: Vec::new(),
                    meta: test_meta("ChildSupportWaiver"),
                }),
                CoreDecl::ConflictDoctrine(CoreConflictDoctrine {
                    id: NodeId::of(b"ChildSupportCannotBeAdverselyAffected"),
                    name: "ChildSupportCannotBeAdverselyAffected".into(),
                    guard: Guard::Satisfied,
                    defeats: vec![ConflictTarget::Clause(clause_id)],
                    as_to: Some(LegalEffectPattern::Affect(LegalSubjectPattern::Exact(
                        "ChildSupportRight".into(),
                    ))),
                    reason: "MandatoryStatutoryLimit".into(),
                    meta: test_meta("ChildSupportCannotBeAdverselyAffected"),
                }),
            ],
        );
        match run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::Ctor { name, fields },
                ..
            } => {
                assert_eq!(name, "PreventedAsTo");
                assert_eq!(
                    fields.get("doctrine"),
                    Some(&Value::String(
                        "ChildSupportCannotBeAdverselyAffected".into()
                    ))
                );
                assert_ne!(name, "clause");
            }
            other => panic!("instantiated selector must not become the string clause: {other:?}"),
        }
    }

    #[test]
    fn determined_is_not_true_when_the_same_issue_is_denied() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "determined".into(),
            args: vec![Term::Apply {
                ctor: "P".into(),
                args: vec![Term::Ident("A".into())],
            }],
        });
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: true,
            decider: "YesVote".into(),
            recorded_at: Some(t),
        });
        case.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: false,
            decider: "NoVote".into(),
            recorded_at: Some(t),
        });
        let out = run_plan(plan, &case, &mut Refusing);
        assert!(
            !matches!(
                out,
                Outcome::Determinate {
                    value: Value::Bool(true),
                    ..
                }
            ),
            "denied+held must not be determined true: {out:?}"
        );
    }

    #[test]
    fn run_decision_does_not_silently_run_foia() {
        let plan = QueryPlan::RunDecision {
            decision: "TrusteeDistributionDecision".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::Sort(Sort::Nominal("Distribution".into())),
            },
        };
        let err = run_module(
            &module_with_plan("q", plan),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .expect_err("unknown decision is an implementation error");
        assert!(
            matches!(err, EngineError::Unsupported(ref msg) if msg.contains("TrusteeDistributionDecision")),
            "{err:?}"
        );
    }

    fn proposition(name: &str) -> CoreDecl {
        CoreDecl::Proposition(CoreProposition {
            id: NodeId::of(name.as_bytes()),
            name: name.into(),
            params: Vec::new(),
            meta: test_meta(name),
        })
    }

    fn derive_rule(name: &str, from: &str, to: &str) -> CoreDecl {
        CoreDecl::Rule(CoreRule {
            id: NodeId::of(name.as_bytes()),
            name: name.into(),
            kind: RuleKind::Derive,
            binders: Vec::new(),
            selection: None,
            guard: Guard::Operative(PropTerm::new(from, Vec::new()), String::new()),
            consequences: vec![CoreEffect {
                id: EffectId::of(format!("{name}:derive").as_bytes()),
                consequence: Consequence::Derive(PropTerm::new(to, Vec::new())),
                meta: test_meta(name),
            }],
            fallback: None,
            meta: test_meta(name),
        })
    }

    fn terminate_establish_rule(name: &str, from: &str, to: &str) -> CoreDecl {
        CoreDecl::Rule(CoreRule {
            id: NodeId::of(name.as_bytes()),
            name: name.into(),
            kind: RuleKind::Constitutive,
            binders: Vec::new(),
            selection: None,
            guard: Guard::Operative(PropTerm::new(from, Vec::new()), String::new()),
            consequences: vec![
                CoreEffect {
                    id: EffectId::of(format!("{name}:terminate").as_bytes()),
                    consequence: Consequence::Terminate(PropTerm::new(from, Vec::new())),
                    meta: test_meta(name),
                },
                CoreEffect {
                    id: EffectId::of(format!("{name}:establish").as_bytes()),
                    consequence: Consequence::Establish(PropTerm::new(to, Vec::new())),
                    meta: test_meta(name),
                },
            ],
            fallback: None,
            meta: test_meta(name),
        })
    }

    fn entity_decl(name: &str) -> CoreDecl {
        CoreDecl::Entity(CoreEntity {
            id: NodeId::of(name.as_bytes()),
            name: name.into(),
            ty: Type::Sort(Sort::LegalPerson),
            meta: test_meta(name),
        })
    }

    fn ordinary_function(name: &str, params: Vec<&str>, body: Option<Term>) -> CoreFunction {
        CoreFunction {
            id: NodeId::of(name.as_bytes()),
            name: name.into(),
            params: params
                .into_iter()
                .map(|p| (p.to_owned(), Type::bool()))
                .collect(),
            result: Type::bool(),
            effects: BTreeSet::new(),
            is_calc: false,
            fuel: None,
            body,
            meta: test_meta(name),
        }
    }

    fn operative_of(pred: &str) -> Term {
        Term::Apply {
            ctor: "operative".into(),
            args: vec![Term::Apply {
                ctor: pred.into(),
                args: Vec::new(),
            }],
        }
    }

    #[test]
    fn derived_rule_fires_when_antecedent_is_established() {
        let plan = QueryPlan::Evaluate(operative_of("Q"));
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![
                proposition("P"),
                proposition("Q"),
                derive_rule("R", "P", "Q"),
            ],
        );
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "P".into(),
            protocol: "P".into(),
            established: true,
            decider: "test".into(),
            recorded_at: None,
        });
        match run_module(&module, "q", &case, &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn derived_rule_does_not_make_consequent_determinate_without_antecedent() {
        let plan = QueryPlan::Evaluate(operative_of("Q"));
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![
                proposition("P"),
                proposition("Q"),
                derive_rule("R", "P", "Q"),
            ],
        );
        let out =
            run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate");
        assert!(
            !matches!(
                out,
                Outcome::Determinate {
                    value: Value::Bool(true),
                    ..
                }
            ),
            "Q must not be determinate true without P: {out:?}"
        );
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedJudgment { issue, .. } if issue.predicate == "Q"
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn for_all_over_closed_positive_set_is_true() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "for_all".into(),
            args: vec![
                Term::Binder("x".into()),
                Term::Set(vec![Term::Int(1), Term::Int(2)]),
                Term::Apply {
                    ctor: ">".into(),
                    args: vec![Term::Ident("x".into()), Term::Int(0)],
                },
            ],
        });
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        match out {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn for_all_open_ident_domain_without_closure_suspends() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "for_all".into(),
            args: vec![
                Term::Ident("x".into()),
                Term::Ident("People".into()),
                Term::Apply {
                    ctor: ">".into(),
                    args: vec![Term::Ident("x".into()), Term::Int(0)],
                },
            ],
        });
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedEvidence { schema, .. } if schema == "ClosureRecord"
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nested_for_all_over_closed_sets_with_true_is_true() {
        let plan = QueryPlan::Evaluate(quantifier_apply(
            "for_all",
            "x",
            closed_int_set(&[1, 2]),
            quantifier_apply("for_all", "y", closed_int_set(&[1, 2]), Term::Bool(true)),
        ));
        assert_determinate_bool(run_plan(plan, &CaseRecord::default(), &mut Refusing), true);
    }

    #[test]
    fn nested_for_all_exists_equality_witness_is_true() {
        let plan = QueryPlan::Evaluate(quantifier_apply(
            "for_all",
            "x",
            closed_int_set(&[1]),
            quantifier_apply("exists", "y", closed_int_set(&[1, 2]), eq_idents("x", "y")),
        ));
        assert_determinate_bool(run_plan(plan, &CaseRecord::default(), &mut Refusing), true);
    }

    #[test]
    fn nested_for_all_exists_equality_without_witness_is_false() {
        let plan = QueryPlan::Evaluate(quantifier_apply(
            "for_all",
            "x",
            closed_int_set(&[1]),
            quantifier_apply("exists", "y", closed_int_set(&[2]), eq_idents("x", "y")),
        ));
        assert_determinate_bool(run_plan(plan, &CaseRecord::default(), &mut Refusing), false);
    }

    #[test]
    fn nested_for_all_inner_open_ident_domain_without_closure_suspends() {
        let plan = QueryPlan::Evaluate(quantifier_apply(
            "for_all",
            "x",
            closed_int_set(&[1]),
            quantifier_apply(
                "for_all",
                "y",
                Term::Ident("People".into()),
                Term::Bool(true),
            ),
        ));
        assert_needs_closure_record(run_plan(plan, &CaseRecord::default(), &mut Refusing));
    }

    #[test]
    fn run_decision_foia_process_still_needs_harm_and_segregability() {
        let plan = QueryPlan::RunDecision {
            decision: "ProcessResponsiveRecord".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::Sort(Sort::Nominal("FOIADisposition".into())),
            },
        };
        let decision = CoreDecision {
            id: NodeId::of(b"ProcessResponsiveRecord"),
            name: "ProcessResponsiveRecord".into(),
            binders: Vec::new(),
            requirements: vec![
                Guard::Observed {
                    schema: "HarmAnalysis".into(),
                    binder: "h".into(),
                },
                Guard::Observed {
                    schema: "SegregabilityAnalysis".into(),
                    binder: "s".into(),
                },
            ],
            option_space: Term::Wildcard,
            declared_result: Some(DecisionReturn {
                result_type: Type::Sort(Sort::Nominal("FOIADisposition".into())),
                expression: Term::Ident("proposed".into()),
            }),
            meta: test_meta("ProcessResponsiveRecord"),
        };
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Decision(decision)]);
        let out =
            run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate");
        match out {
            Outcome::Suspended { requests, .. } => {
                let schemas: Vec<_> = requests
                    .iter()
                    .filter_map(|r| match r {
                        OpenRequest::NeedEvidence { schema, .. } => Some(schema.as_str()),
                        _ => None,
                    })
                    .collect();
                assert!(schemas.contains(&"HarmAnalysis"), "{schemas:?}");
                assert!(schemas.contains(&"SegregabilityAnalysis"), "{schemas:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn i1_with_both_accepted_selects_highest_rank_not_the_label() {
        let module = module_with_occupant();
        let case = successor_case(2, Some("I1"));
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let out = evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            &mut Refusing,
            &case,
        )
        .expect("evaluate");
        match out {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Alice"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn i1_with_only_bob_accepted_selects_bob() {
        let module = module_with_occupant();
        let mut case = successor_case(2, Some("I1"));
        case.facts
            .insert("alice_accepted".into(), Value::Bool(false));
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let out = evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            &mut Refusing,
            &case,
        )
        .expect("evaluate");
        match out {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn renamed_alternatives_keep_eligibility_results() {
        let mut module = module_with_occupant();
        rename_family_labels(&mut module, &[("I1", "Alpha"), ("I2", "Beta")]);
        let mut case = successor_case(2, Some("Alpha"));
        case.admissible_completions.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["Alpha".into(), "Beta".into()],
        );
        match run_succession(&module, "acting_trustee", &case) {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Alice"),
            other => panic!("{other:?}"),
        }
        case.interpretations
            .insert("SuccessorEligibility".into(), "Beta".into());
        match run_succession(&module, "acting_trustee", &case) {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn changing_i1_definitions_without_renaming_changes_the_result() {
        let mut module = module_with_occupant();
        set_family_defs(
            &mut module,
            "I1",
            vec![
                eligible_def("Alice", "TrusteeOf(BRT)", false),
                eligible_def("Bob", "TrusteeOf(BRT)", true),
            ],
        );
        let case = successor_case(2, Some("I1"));
        match run_succession(&module, "acting_trustee", &case) {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn third_nominee_can_win_from_declared_eligibility() {
        let mut module = module_with_occupant();
        module.nominations.push(CoreNomination {
            candidate: "Carol".into(),
            office: "TrusteeOf(BRT)".into(),
            rank: 3,
        });
        set_family_defs(
            &mut module,
            "I1",
            vec![
                eligible_def("Alice", "TrusteeOf(BRT)", false),
                eligible_def("Bob", "TrusteeOf(BRT)", false),
                eligible_def("Carol", "TrusteeOf(BRT)", true),
            ],
        );
        let mut case = successor_case(2, Some("I1"));
        case.facts
            .insert("carol_accepted".into(), Value::Bool(true));
        match run_succession(&module, "acting_trustee", &case) {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Carol"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn two_offices_follow_distinct_succession_families() {
        let module = module_with_two_offices();
        let case = two_office_case();
        match run_succession(&module, "acting_trustee", &case) {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Alice"),
            other => panic!("trustee: {other:?}"),
        }
        match run_succession(&module, "acting_executor", &case) {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Dana"),
            other => panic!("executor: {other:?}"),
        }
    }

    #[test]
    fn office_without_succession_family_does_not_rank_accepted_nominees() {
        let office = "ClerkOf(X)";
        let unrelated = CoreDecl::InterpretationFamily(CoreInterpretationFamily {
            id: NodeId::of(b"ExecutorEligibility"),
            name: "ExecutorEligibility".into(),
            source: Term::Ident("ExecutorEligibility".into()),
            alternatives: vec![(
                "NextOfKin".into(),
                vec![
                    eligible_def("Dana", "ExecutorOf(Est)", true),
                    eligible_def("Eve", "ExecutorOf(Est)", false),
                ],
            )],
            meta: test_meta("ExecutorEligibility"),
        });
        let mut module = module_with_plan_decls(
            "acting_clerk",
            QueryPlan::UniqueOccupant {
                office: Term::Ident(office.into()),
            },
            vec![unrelated],
        );
        module.nominations = vec![nomination("Alice", office, 1), nomination("Bob", office, 2)];
        let case = successor_case(2, None);
        let out = run_succession(&module, "acting_clerk", &case);
        assert!(
            !matches!(out, Outcome::Determinate { .. }),
            "accepted nominees must not become a successor without a family for this office: {out:?}"
        );
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(
                    requests.iter().any(|r| matches!(
                        r,
                        OpenRequest::NeedJudgment { protocol, issue, .. }
                            if protocol == "Appointment" && issue.predicate == "Occupies"
                    ) || matches!(
                        r,
                        OpenRequest::NeedInterpretation { .. }
                    )),
                    "{requests:?}"
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn process_responsive_record_executes_its_declared_body() {
        let plan = QueryPlan::RunDecision {
            decision: "ProcessResponsiveRecord".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::bool(),
            },
        };
        let decision = CoreDecision {
            id: NodeId::of(b"ProcessResponsiveRecord"),
            name: "ProcessResponsiveRecord".into(),
            binders: Vec::new(),
            requirements: Vec::new(),
            option_space: Term::Wildcard,
            declared_result: Some(DecisionReturn {
                result_type: Type::bool(),
                expression: Term::Bool(true),
            }),
            meta: test_meta("ProcessResponsiveRecord"),
        };
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Decision(decision)]);
        match run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("name must not activate FOIA: {other:?}"),
        }
    }

    #[test]
    fn two_certificates_and_i2_selects_second_ranked_nominee() {
        let module = module_with_occupant();
        let case = successor_case(2, Some("I2"));
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let out = evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            &mut Refusing,
            &case,
        )
        .expect("evaluate");
        match out {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn recorded_nominee_name_selects_that_successor() {
        let module = module_with_occupant();
        let case = successor_case(2, Some("BobOnly"));
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let out = evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            &mut Refusing,
            &case,
        )
        .expect("evaluate");
        match out {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn concurring_schema_need_not_be_physician_certificate() {
        let module = module_with_occupant();
        let t = fidryn_core::Instant::parse("2026-08-23T12:00:00Z").unwrap();
        let mut case = successor_case(0, Some("I2"));
        case.evidence.retain(|e| e.schema == "OccupancyRecord");
        for i in 0..2 {
            case.evidence.push(EvidenceItem {
                schema: "CapacityAffidavit".into(),
                value: Value::String(format!("a{i}")),
                observed_at: t,
            });
        }
        let known = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let out = evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(known, known),
            &mut Refusing,
            &case,
        )
        .expect("evaluate");
        match out {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn required_concurring_fact_overrides_default_two() {
        let module = module_with_occupant();
        let mut case = successor_case(1, Some("I2"));
        case.facts
            .insert("required_concurring".into(), Value::Int(1));
        let t = fidryn_core::Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let out = evaluate(
            &module,
            &QueryName::from("acting_trustee"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            &mut Refusing,
            &case,
        )
        .expect("evaluate");
        match out {
            Outcome::Determinate { value, .. } => assert_eq!(value.display_label(), "Bob"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn resume_after_case_determination_is_determinate() {
        let module = module_with_plan("q", QueryPlan::Evaluate(Term::Ident("judgment".into())));
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let mut case = CaseRecord::default();
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(first.outcome, Outcome::Suspended { .. }),
            "{:?}",
            first.outcome
        );
        assert!(first.continuation.is_some());
        case.determinations.push(CaseDetermination {
            issue: "Adjudicated".into(),
            protocol: "JudgmentOnTheMerits".into(),
            established: true,
            decider: "Court".into(),
            recorded_at: None,
        });
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("resume");
        assert!(
            matches!(second.outcome, Outcome::Determinate { .. }),
            "{:?}",
            second.outcome
        );
        assert!(second.continuation.is_none());
    }

    #[test]
    fn run_decision_uses_core_requirements_before_foia_name() {
        let plan = QueryPlan::RunDecision {
            decision: "IssuePermit".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::bool(),
            },
        };
        let decision = CoreDecision {
            id: NodeId::of(b"IssuePermit"),
            name: "IssuePermit".into(),
            binders: Vec::new(),
            requirements: vec![Guard::Observed {
                schema: "SiteInspection".into(),
                binder: "r".into(),
            }],
            option_space: Term::Wildcard,
            declared_result: Some(DecisionReturn {
                result_type: Type::bool(),
                expression: Term::Bool(true),
            }),
            meta: test_meta("IssuePermit"),
        };
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Decision(decision)]);
        let out =
            run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate");
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedEvidence { schema, .. } if schema == "SiteInspection"
                )));
                assert!(!requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedEvidence { schema, .. }
                        if schema == "HarmAnalysis" || schema == "SegregabilityAnalysis"
                )));
            }
            other => panic!("{other:?}"),
        }

        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "SiteInspection".into(),
            value: Value::String("ok".into()),
            observed_at: fidryn_core::Instant::parse("2025-01-01T00:00:00Z").unwrap(),
        });
        match run_module(&module, "q", &case, &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("{other:?}"),
        }

        let mut future = CaseRecord::default();
        future.evidence.push(EvidenceItem {
            schema: "SiteInspection".into(),
            value: Value::String("late".into()),
            observed_at: fidryn_core::Instant::parse("2027-01-01T00:00:00Z").unwrap(),
        });
        let late = run_module(&module, "q", &future, &mut Refusing).expect("evaluate");
        assert!(matches!(late, Outcome::Suspended { .. }), "{late:?}");
    }

    #[test]
    fn run_decision_rejects_argument_arity_mismatch() {
        let plan = QueryPlan::RunDecision {
            decision: "IssuePermit".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::bool(),
            },
        };
        let decision = CoreDecision {
            id: NodeId::of(b"IssuePermit"),
            name: "IssuePermit".into(),
            binders: vec![("site".into(), Type::bool())],
            requirements: Vec::new(),
            option_space: Term::Wildcard,
            declared_result: Some(DecisionReturn {
                result_type: Type::bool(),
                expression: Term::Bool(true),
            }),
            meta: test_meta("IssuePermit"),
        };
        let err = run_module(
            &module_with_plan_decls("q", plan, vec![CoreDecl::Decision(decision)]),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .expect_err("arity");
        assert!(
            matches!(err, EngineError::InvalidInput(ref msg) if msg.contains("IssuePermit")),
            "{err:?}"
        );
    }

    #[test]
    fn resume_does_not_replay_answered_observe() {
        let plan = QueryPlan::RunDecision {
            decision: "TwoStep".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::bool(),
            },
        };
        let decision = CoreDecision {
            id: NodeId::of(b"TwoStep"),
            name: "TwoStep".into(),
            binders: Vec::new(),
            requirements: vec![
                Guard::Observed {
                    schema: "FirstRecord".into(),
                    binder: "a".into(),
                },
                Guard::Observed {
                    schema: "SecondRecord".into(),
                    binder: "b".into(),
                },
            ],
            option_space: Term::Wildcard,
            declared_result: Some(DecisionReturn {
                result_type: Type::bool(),
                expression: Term::Bool(true),
            }),
            meta: test_meta("TwoStep"),
        };
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Decision(decision)]);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let mut first_handler = Scripted::resume_only(&["FirstRecord"]);
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut first_handler,
            &case,
        )
        .expect("evaluate_session");
        assert_eq!(first_handler.count("FirstRecord"), 1);
        assert_eq!(first_handler.count("SecondRecord"), 1);
        assert!(
            matches!(first.outcome, Outcome::Suspended { .. }),
            "{:?}",
            first.outcome
        );

        let mut second_handler = Scripted::resume_only(&["FirstRecord", "SecondRecord"]);
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut second_handler,
            &case,
        )
        .expect("resume");
        assert_eq!(
            second_handler.count("FirstRecord"),
            0,
            "answered FirstRecord must not run again"
        );
        assert_eq!(second_handler.count("SecondRecord"), 1);
        match second.outcome {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn resume_invalid_response_stays_suspended_or_halts() {
        let plan = QueryPlan::RunDecision {
            decision: "TwoStep".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::bool(),
            },
        };
        let decision = CoreDecision {
            id: NodeId::of(b"TwoStep"),
            name: "TwoStep".into(),
            binders: Vec::new(),
            requirements: vec![Guard::Observed {
                schema: "FirstRecord".into(),
                binder: "a".into(),
            }],
            option_space: Term::Wildcard,
            declared_result: Some(DecisionReturn {
                result_type: Type::bool(),
                expression: Term::Bool(true),
            }),
            meta: test_meta("TwoStep"),
        };
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Decision(decision)]);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(matches!(first.outcome, Outcome::Suspended { .. }));
        let mut halting = Scripted::halt_on("FirstRecord");
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut halting,
            &case,
        )
        .expect("resume");
        assert!(
            matches!(
                second.outcome,
                Outcome::OutsideCompetence { .. } | Outcome::Suspended { .. }
            ),
            "{:?}",
            second.outcome
        );
    }

    fn judgment_issue(protocol: &str, predicate: &str, arg: &str) -> OpenRequest {
        OpenRequest::NeedJudgment {
            protocol: protocol.into(),
            issue: PropTerm::new(predicate, vec![Term::Ident(arg.into())]),
        }
    }

    struct ResumePerson {
        person: String,
        counts: BTreeMap<String, usize>,
    }

    impl ResumePerson {
        fn new(person: &str) -> Self {
            Self {
                person: person.into(),
                counts: BTreeMap::new(),
            }
        }

        fn count(&self, person: &str) -> usize {
            self.counts.get(person).copied().unwrap_or(0)
        }

        fn refuse(request: &OpenRequest) -> HandlerResult {
            let mut requests = BTreeSet::new();
            requests.insert(request.clone());
            HandlerResult::Suspend {
                requests,
                reason: fidryn_core::SuspensionReason::MissingDetermination,
                trace_fragment: "missing".into(),
            }
        }
    }

    impl Handler for ResumePerson {
        fn handle_observe(&mut self, request: &OpenRequest) -> HandlerResult {
            Self::refuse(request)
        }
        fn handle_determine(&mut self, request: &OpenRequest) -> HandlerResult {
            let person = match request {
                OpenRequest::NeedJudgment { issue, .. } => issue
                    .arguments
                    .iter()
                    .find_map(|term| match term {
                        Term::Ident(s) | Term::String(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("")
                    .to_owned(),
                _ => String::new(),
            };
            *self.counts.entry(person.clone()).or_insert(0) += 1;
            if person.eq_ignore_ascii_case(&self.person) {
                return HandlerResult::Resume {
                    value: Value::Bool(true),
                    trace_fragment: format!("resume:{person}"),
                };
            }
            Self::refuse(request)
        }
        fn handle_choose(&mut self, request: &OpenRequest) -> HandlerResult {
            Self::refuse(request)
        }
        fn handle_interpret(&mut self, request: &OpenRequest) -> HandlerResult {
            Self::refuse(request)
        }
    }

    fn determined_of(pred: &str, arg: &str) -> Term {
        Term::Apply {
            ctor: "determined".into(),
            args: vec![Term::Apply {
                ctor: pred.into(),
                args: vec![Term::Ident(arg.into())],
            }],
        }
    }

    #[test]
    fn request_key_includes_judgment_arguments() {
        let alice = judgment_issue("determined", "Eligible", "Alice");
        let bob = judgment_issue("determined", "Eligible", "Bob");
        assert_ne!(request_key(&alice), request_key(&bob));
        let same_schema_a = OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(PropTerm::new("P", vec![Term::Ident("Alice".into())])),
            schema: "Record".into(),
        };
        let same_schema_b = OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(PropTerm::new("P", vec![Term::Ident("Bob".into())])),
            schema: "Record".into(),
        };
        assert_ne!(request_key(&same_schema_a), request_key(&same_schema_b));
        let choice_a = OpenRequest::NeedChoice {
            protocol: "Pick".into(),
            options: vec!["A".into()],
        };
        let choice_b = OpenRequest::NeedChoice {
            protocol: "Pick".into(),
            options: vec!["B".into()],
        };
        assert_ne!(request_key(&choice_a), request_key(&choice_b));
        let conflict_a = OpenRequest::NeedConflict {
            graph: vec!["Follow:A".into()],
            doctrines: vec!["A".into()],
        };
        let conflict_b = OpenRequest::NeedConflict {
            graph: vec!["Follow:B".into()],
            doctrines: vec!["B".into()],
        };
        assert_ne!(request_key(&conflict_a), request_key(&conflict_b));
        let custom_a = OpenRequest::NeedCustom {
            effect: "require".into(),
            payload: "one".into(),
        };
        let custom_b = OpenRequest::NeedCustom {
            effect: "require".into(),
            payload: "two".into(),
        };
        assert_ne!(request_key(&custom_a), request_key(&custom_b));
    }

    #[test]
    fn custom_effect_argument_values_distinguish_requests() {
        let effect = CoreDecl::EffectDecl(CoreEffectDecl {
            id: NodeId::of(b"DocketLookup"),
            name: "DocketLookup".into(),
            operations: vec![CoreEffectOp {
                name: "request".into(),
                params: vec![("docket_id".into(), Type::Primitive(PrimitiveType::String))],
                result: Type::bool(),
            }],
            meta: test_meta("DocketLookup"),
        });
        let apply = |docket: &str| {
            QueryPlan::Evaluate(Term::Apply {
                ctor: "DocketLookup".into(),
                args: vec![Term::String(docket.into())],
            })
        };
        let out_a = run_module(
            &module_with_plan_decls("q", apply("A"), vec![effect.clone()]),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .expect("A");
        let out_b = run_module(
            &module_with_plan_decls("q", apply("B"), vec![effect]),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .expect("B");
        let req_a = match out_a {
            Outcome::Suspended { requests, .. } => requests,
            other => panic!("{other:?}"),
        };
        let req_b = match out_b {
            Outcome::Suspended { requests, .. } => requests,
            other => panic!("{other:?}"),
        };
        assert_ne!(
            req_a, req_b,
            "DocketLookup(\"A\") must not equal DocketLookup(\"B\")"
        );
        assert!(req_a.iter().any(|r| matches!(
            r,
            OpenRequest::NeedCustom { effect, payload }
                if effect == "DocketLookup" && payload.contains('A')
        )));
        assert!(req_b.iter().any(|r| matches!(
            r,
            OpenRequest::NeedCustom { effect, payload }
                if effect == "DocketLookup" && payload.contains('B')
        )));
    }

    #[test]
    fn remembering_handler_does_not_replay_alice_answer_for_bob() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "&&".into(),
            args: vec![
                determined_of("Eligible", "Alice"),
                determined_of("Eligible", "Bob"),
            ],
        });
        let mut handler = ResumePerson::new("Alice");
        let out = run_plan(plan, &CaseRecord::default(), &mut handler);
        assert_eq!(handler.count("Alice"), 1);
        assert_eq!(handler.count("Bob"), 1);
        assert!(
            !matches!(
                out,
                Outcome::Determinate {
                    value: Value::Bool(true),
                    ..
                }
            ),
            "Bob must not inherit Alice's Resume: {out:?}"
        );
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    #[test]
    fn resume_after_case_change_does_not_reuse_answered() {
        let plan = QueryPlan::RunDecision {
            decision: "TwoStep".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::bool(),
            },
        };
        let decision = CoreDecision {
            id: NodeId::of(b"TwoStep"),
            name: "TwoStep".into(),
            binders: Vec::new(),
            requirements: vec![
                Guard::Observed {
                    schema: "FirstRecord".into(),
                    binder: "a".into(),
                },
                Guard::Observed {
                    schema: "SecondRecord".into(),
                    binder: "b".into(),
                },
            ],
            option_space: Term::Wildcard,
            declared_result: Some(DecisionReturn {
                result_type: Type::bool(),
                expression: Term::Bool(true),
            }),
            meta: test_meta("TwoStep"),
        };
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Decision(decision)]);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let mut case = CaseRecord::default();
        let mut first_handler = Scripted::resume_only(&["FirstRecord"]);
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut first_handler,
            &case,
        )
        .expect("evaluate_session");
        assert_eq!(first_handler.count("FirstRecord"), 1);
        assert!(matches!(first.outcome, Outcome::Suspended { .. }));

        case.facts.insert("retracted".into(), Value::Bool(true));
        let mut second_handler = Scripted::resume_only(&["SecondRecord"]);
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut second_handler,
            &case,
        )
        .expect("resume");
        assert_eq!(
            second_handler.count("FirstRecord"),
            1,
            "changed case must not replay the previous FirstRecord answer"
        );
        assert!(
            !matches!(second.outcome, Outcome::Determinate { .. }),
            "{:?}",
            second.outcome
        );
    }

    #[test]
    fn contingent_does_not_drop_suspended_eligible_branch() {
        let mut module = module_with_occupant();
        let office = "TrusteeOf(BRT)";
        set_family_defs(
            &mut module,
            "I1",
            vec![
                eligible_def("Alice", office, false),
                eligible_def("Bob", office, true),
            ],
        );
        set_family_defs(
            &mut module,
            "I2",
            vec![
                eligible_def("Alice", office, false),
                eligible_def("Bob", office, false),
            ],
        );
        family_mut(&mut module)
            .alternatives
            .retain(|(label, _)| label == "I1" || label == "I2");
        let case = successor_case(2, None);
        let out = run_succession(&module, "acting_trustee", &case);
        assert!(
            !matches!(out, Outcome::Determinate { .. }),
            "Suspended Eligible branch must not collapse to Determinate: {out:?}"
        );
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedJudgment { issue, .. } if issue.predicate == "Eligible"
                )));
            }
            Outcome::Contingent { pivots, .. } => {
                assert!(pivots.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedJudgment { issue, .. } if issue.predicate == "Eligible"
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn worklist_terminate_then_establish_still_derives() {
        let plan = QueryPlan::Evaluate(operative_of("C"));
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![
                proposition("A"),
                proposition("B"),
                proposition("C"),
                derive_rule("R1", "B", "C"),
                terminate_establish_rule("R2", "A", "B"),
            ],
        );
        let mut case = CaseRecord::default();
        case.facts.insert("A".into(), Value::Bool(true));
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let world = DerivedWorld::compute(&module, &case, &RunContext::new(t, t), &BTreeMap::new())
            .expect("compute");
        assert!(
            world.holds(&PropTerm::new("C", Vec::new())),
            "B → C must fire after A is replaced by B"
        );
        match run_module(&module, "q", &case, &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn substitutions_cap_with_remaining_binders_is_not_silent() {
        let mut decls: Vec<CoreDecl> = (0..300).map(|i| entity_decl(&format!("E{i}"))).collect();
        decls.push(CoreDecl::Rule(CoreRule {
            id: NodeId::of(b"cap"),
            name: "cap".into(),
            kind: RuleKind::Derive,
            binders: vec!["x".into(), "y".into()],
            selection: None,
            guard: Guard::Satisfied,
            consequences: vec![CoreEffect {
                id: EffectId::of(b"cap:derive"),
                consequence: Consequence::Derive(PropTerm::new("C", Vec::new())),
                meta: test_meta("cap"),
            }],
            fallback: None,
            meta: test_meta("cap"),
        }));
        let module = module_with_plan_decls("q", QueryPlan::Evaluate(Term::Bool(true)), decls);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let err = DerivedWorld::compute(
            &module,
            &CaseRecord::default(),
            &RunContext::new(t, t),
            &BTreeMap::new(),
        )
        .expect_err("truncated binders");
        assert!(
            matches!(
                err,
                EngineError::Unsupported(_) | EngineError::FuelExhausted { .. }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn string_literal_body_is_executable() {
        let function = ordinary_function("text", Vec::new(), Some(Term::String("hello".into())));
        let plan = QueryPlan::Evaluate(Term::Call {
            callee: "text".into(),
            args: Vec::new(),
        });
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Function(function)]);
        match run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::String(s),
                ..
            } => assert_eq!(s, "hello"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn tax_named_function_with_string_body_returns_the_string() {
        let mut function = tax_function("tax_on", None);
        function.body = Some(Term::String("hello".into()));
        let plan = QueryPlan::Evaluate(Term::Call {
            callee: "tax_on".into(),
            args: vec![Term::Int(10_000)],
        });
        let module = module_with_plan_decls("q", plan, vec![CoreDecl::Function(function)]);
        match run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::String(s),
                ..
            } => assert_eq!(s, "hello"),
            other => panic!("string body must not run the tax helper: {other:?}"),
        }
    }

    #[test]
    fn function_call_does_not_inherit_caller_bindings() {
        let inner = ordinary_function("inner", Vec::new(), Some(Term::Ident("y".into())));
        let outer = ordinary_function(
            "outer",
            vec!["y"],
            Some(Term::Call {
                callee: "inner".into(),
                args: Vec::new(),
            }),
        );
        let plan = QueryPlan::Evaluate(Term::Call {
            callee: "outer".into(),
            args: vec![Term::Bool(true)],
        });
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(inner), CoreDecl::Function(outer)],
        );
        let err = run_module(&module, "q", &CaseRecord::default(), &mut Refusing)
            .expect_err("caller binding must not fill callee ident");
        assert!(matches!(err, EngineError::Unsupported(_)), "{err:?}");
    }

    #[test]
    fn function_call_rejects_arity_mismatch() {
        let function = ordinary_function("pair", vec!["a", "b"], Some(Term::Ident("a".into())));
        let plan = QueryPlan::Evaluate(Term::Call {
            callee: "pair".into(),
            args: vec![Term::Bool(true)],
        });
        let err = run_module(
            &module_with_plan_decls("q", plan, vec![CoreDecl::Function(function)]),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .expect_err("arity");
        assert!(
            matches!(err, EngineError::InvalidInput(ref msg) if msg.contains("pair")),
            "{err:?}"
        );
    }

    #[test]
    fn accept_office_map_requires_matching_office() {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "AcceptOffice".into(),
            value: Value::Map(BTreeMap::from([
                ("person".into(), Value::String("Alice".into())),
                ("office".into(), Value::String("TrusteeOf(BRT)".into())),
            ])),
            observed_at: t,
        });
        let ctx = RunContext::new(t, t);
        assert!(is_accepted(&case, "Alice", "TrusteeOf(BRT)", &ctx));
        assert!(!is_accepted(&case, "Alice", "ExecutorOf(Est)", &ctx));
        case.facts
            .insert("alice_accepted".into(), Value::Bool(true));
        assert!(is_accepted(&case, "Alice", "ExecutorOf(Est)", &ctx));
    }

    #[test]
    fn accept_office_person_string_still_matches() {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.evidence.push(EvidenceItem {
            schema: "AcceptOffice".into(),
            value: Value::String("Alice".into()),
            observed_at: t,
        });
        let ctx = RunContext::new(t, t);
        assert!(is_accepted(&case, "Alice", "TrusteeOf(BRT)", &ctx));
    }

    #[test]
    fn seq_returns_last_determinate_value() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "seq".into(),
            args: vec![Term::Int(1), Term::Int(2), Term::Int(3)],
        });
        match run_plan(plan, &CaseRecord::default(), &mut Refusing) {
            Outcome::Determinate {
                value: Value::Int(3),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn empty_seq_is_unsupported() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "seq".into(),
            args: Vec::new(),
        });
        let err = run_module(
            &module_with_plan("q", plan),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .unwrap_err();
        assert!(matches!(err, EngineError::Unsupported(_)), "{err:?}");
    }

    #[test]
    fn seq_require_true_continues_to_later_value() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "seq".into(),
            args: vec![
                Term::Apply {
                    ctor: "require".into(),
                    args: vec![Term::Bool(true)],
                },
                Term::Int(7),
            ],
        });
        match run_plan(plan, &CaseRecord::default(), &mut Refusing) {
            Outcome::Determinate {
                value: Value::Int(7),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn seq_require_false_does_not_return_later_value() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "seq".into(),
            args: vec![
                Term::Apply {
                    ctor: "require".into(),
                    args: vec![Term::Bool(false)],
                },
                Term::Int(7),
            ],
        });
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        assert!(
            !matches!(
                out,
                Outcome::Determinate {
                    value: Value::Int(7),
                    ..
                }
            ),
            "{out:?}"
        );
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedCustom { effect, .. } if effect == "require"
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    fn seq_terms(args: Vec<Term>) -> QueryPlan {
        QueryPlan::Evaluate(Term::Apply {
            ctor: "seq".into(),
            args,
        })
    }

    fn require_term(cond: Term) -> Term {
        Term::Apply {
            ctor: "require".into(),
            args: vec![cond],
        }
    }

    fn determined_gate() -> Term {
        Term::Apply {
            ctor: "determined".into(),
            args: vec![Term::Apply {
                ctor: "Gate".into(),
                args: vec![],
            }],
        }
    }

    fn duty_step_term(name: &str, action: &str) -> Term {
        Term::Apply {
            ctor: "duty_step".into(),
            args: vec![Term::Ident(name.into()), Term::Ident(action.into())],
        }
    }

    fn duty_step_instance_term(name: &str, action: &str, person: &str, instance: &str) -> Term {
        Term::Apply {
            ctor: "duty_step".into(),
            args: vec![
                Term::Ident(name.into()),
                Term::Ident(action.into()),
                Term::Ident(person.into()),
                Term::Ident(instance.into()),
            ],
        }
    }

    fn attached_duty_fact() -> Value {
        Value::Map(BTreeMap::from([
            ("status".into(), Value::String("Attached".into())),
            ("breached".into(), Value::Bool(false)),
            ("bearer".into(), Value::Unit),
        ]))
    }

    #[test]
    fn seq_require_unbound_ident_does_not_return_later_value() {
        let plan = seq_terms(vec![
            require_term(Term::Ident("missing".into())),
            Term::Int(7),
        ]);
        let err = run_module(
            &module_with_plan("q", plan),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .expect_err("unbound require condition");
        assert!(
            matches!(err, EngineError::Unsupported(ref msg) if msg.contains("missing")),
            "{err:?}"
        );
    }

    #[test]
    fn seq_require_suspending_ident_does_not_return_later_value() {
        let plan = seq_terms(vec![
            require_term(Term::Ident("EligibleForOpenTexturedCredit".into())),
            Term::Int(7),
        ]);
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        assert!(
            !matches!(
                out,
                Outcome::Determinate {
                    value: Value::Int(7),
                    ..
                }
            ),
            "{out:?}"
        );
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
    }

    #[test]
    fn tax_on_without_body_is_unsupported() {
        let plan = QueryPlan::Evaluate(Term::Call {
            callee: "tax_on".into(),
            args: vec![Term::Int(10_000)],
        });
        let err = run_module(
            &module_with_plan_decls(
                "q",
                plan,
                vec![CoreDecl::Function(tax_function("tax_on", None))],
            ),
            "q",
            &CaseRecord::default(),
            &mut Refusing,
        )
        .expect_err("tax_on is not a builtin");
        assert!(
            matches!(
                err,
                EngineError::Unsupported(ref msg)
                    if msg == "function `tax_on` has no executable body"
            ),
            "{err:?}"
        );
    }

    #[test]
    fn ordinary_income_tax_without_body_still_computes() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "ordinary_income_tax".into(),
            args: vec![Term::Ident("amount".into())],
        });
        let mut case = CaseRecord::default();
        case.facts.insert("amount".into(), Value::Int(10_000));
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax",
                None,
            ))],
        );
        match run_module(&module, "q", &case, &mut Refusing).expect("evaluate") {
            Outcome::Determinate {
                value: Value::Decimal(d),
                ..
            } => assert!(d > Decimal::ZERO),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn duty_late_perform_retains_breached() {
        let plan = seq_terms(vec![
            duty_step_term("pay", "attach"),
            duty_step_term("pay", "breach"),
            duty_step_term("pay", "perform"),
            Term::Field {
                base: Box::new(Term::Ident("duty:pay:default".into())),
                name: "breached".into(),
            },
        ]);
        match run_plan(plan, &CaseRecord::default(), &mut Refusing) {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn duty_illegal_discharge_from_attached_does_not_change_state() {
        let mut case = CaseRecord::default();
        case.facts
            .insert("duty:pay:default".into(), attached_duty_fact());
        let err = run_module(
            &module_with_plan("q", QueryPlan::Evaluate(duty_step_term("pay", "discharge"))),
            "q",
            &case,
            &mut Refusing,
        )
        .expect_err("illegal discharge");
        assert!(
            matches!(err, EngineError::InvalidInput(ref msg) if msg.contains("discharge")),
            "{err:?}"
        );
        match run_plan(
            QueryPlan::Evaluate(Term::Ident("duty:pay:default".into())),
            &case,
            &mut Refusing,
        ) {
            Outcome::Determinate { value, .. } => {
                let state = duty::parse_duty_state(&value, "pay").expect("duty fact");
                assert_eq!(state.status, fidryn_core::DutyStatus::Attached);
                assert!(!state.breached);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn require_authority_with_grant_is_unit() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![Value::String("attach".into())]),
        );
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "require_authority".into(),
            args: vec![Term::Ident("attach".into())],
        });
        match run_plan(plan, &case, &mut Refusing) {
            Outcome::Determinate {
                value: Value::Unit, ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn require_authority_without_grant_suspends() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "require_authority".into(),
            args: vec![Term::Ident("attach".into())],
        });
        match run_plan(plan, &CaseRecord::default(), &mut Refusing) {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedCustom { effect, payload }
                        if effect == "authority" && payload == "attach"
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unauthorized_duty_step_does_not_change_duty_facts() {
        let mut case = CaseRecord::default();
        case.facts
            .insert("authority_grants".into(), Value::Set(Vec::new()));
        let out = run_plan(
            QueryPlan::Evaluate(duty_step_term("pay", "attach")),
            &case,
            &mut Refusing,
        );
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedCustom { effect, payload }
                        if effect == "authority" && payload == "attach"
                )));
            }
            other => panic!("{other:?}"),
        }
        let err = run_module(
            &module_with_plan(
                "q",
                QueryPlan::Evaluate(Term::Ident("duty:pay:default".into())),
            ),
            "q",
            &case,
            &mut Refusing,
        )
        .expect_err("duty fact must stay absent");
        assert!(matches!(err, EngineError::Unsupported(_)), "{err:?}");
    }

    fn authority_grant_map(action: &str, revoked: bool, delegate_of: Option<&str>) -> Value {
        let mut fields = BTreeMap::from([
            ("action".into(), Value::String(action.into())),
            ("revoked".into(), Value::Bool(revoked)),
        ]);
        if let Some(grantor) = delegate_of {
            fields.insert("delegate_of".into(), Value::String(grantor.into()));
            fields.insert("principal".into(), Value::String("delegate".into()));
        }
        Value::Map(fields)
    }

    #[test]
    fn revoked_grant_does_not_authorize_require_authority() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![authority_grant_map("attach", true, None)]),
        );
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "require_authority".into(),
            args: vec![Term::Ident("attach".into())],
        });
        match run_plan(plan, &case, &mut Refusing) {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedCustom { effect, payload }
                        if effect == "authority" && payload == "attach"
                )));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn revoked_grant_does_not_authorize_duty_step() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![authority_grant_map("attach", true, None)]),
        );
        let out = run_plan(
            QueryPlan::Evaluate(duty_step_term("pay", "attach")),
            &case,
            &mut Refusing,
        );
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedCustom { effect, payload }
                        if effect == "authority" && payload == "attach"
                )));
            }
            other => panic!("{other:?}"),
        }
        let err = run_module(
            &module_with_plan(
                "q",
                QueryPlan::Evaluate(Term::Ident("duty:pay:default".into())),
            ),
            "q",
            &case,
            &mut Refusing,
        )
        .expect_err("duty fact must stay absent");
        assert!(matches!(err, EngineError::Unsupported(_)), "{err:?}");
    }

    #[test]
    fn unrevoked_delegated_grant_authorizes_require_authority() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![authority_grant_map("attach", false, Some("grantor"))]),
        );
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "require_authority".into(),
            args: vec![Term::Ident("attach".into())],
        });
        match run_plan(plan, &case, &mut Refusing) {
            Outcome::Determinate {
                value: Value::Unit, ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unrevoked_delegated_grant_authorizes_duty_step() {
        let mut case = CaseRecord::default();
        case.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![authority_grant_map("attach", false, Some("grantor"))]),
        );
        match run_plan(
            QueryPlan::Evaluate(duty_step_term("pay", "attach")),
            &case,
            &mut Refusing,
        ) {
            Outcome::Determinate {
                value: Value::Ctor { name, .. },
                ..
            } => assert_eq!(name, "Attached"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn seq_resume_skips_completed_duty_attach() {
        let plan = seq_terms(vec![
            duty_step_term("pay", "attach"),
            Term::Ident("judgment".into()),
        ]);
        let module = module_with_plan("q", plan);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(first.outcome, Outcome::Suspended { .. }),
            "{:?}",
            first.outcome
        );
        let cont = first.continuation.as_ref().expect("continuation");
        assert_eq!(cont.seq_index, 1);
        assert_eq!(cont.completed.len(), 1);
        assert_eq!(cont.completed[0].display_label(), "Attached");
        let mut handler = Scripted::resume_only(&["other"]);
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut handler,
            &case,
        )
        .expect("resume");
        assert!(
            matches!(second.outcome, Outcome::Determinate { .. }),
            "{:?}",
            second.outcome
        );
    }

    #[test]
    fn report_from_wraps_suspended_require() {
        let out = run_plan(
            QueryPlan::Evaluate(require_term(Term::Bool(false))),
            &CaseRecord::default(),
            &mut Refusing,
        );
        let report = report_from(out);
        assert!(matches!(report.outcome, Outcome::Suspended { .. }));
        assert!(report.unresolved.iter().any(|r| matches!(
            r,
            OpenRequest::NeedCustom { effect, .. } if effect == "require"
        )));
        assert!(report.coverage.is_none());
        assert_eq!(report.execution_mode, ExecutionMode::Operative);
        assert!(report.assumptions.is_empty());
    }

    fn seq_term(args: Vec<Term>) -> Term {
        Term::Apply {
            ctor: "seq".into(),
            args,
        }
    }

    fn duty_status_term(name: &str) -> Term {
        Term::Apply {
            ctor: "duty_status".into(),
            args: vec![Term::Ident(name.into())],
        }
    }

    fn due_apply(days: i64) -> Term {
        Term::Apply {
            ctor: "due".into(),
            args: vec![Term::Int(days), Term::Ident("counted_days".into())],
        }
    }

    fn pay_duty(attaches: Guard, due_days: i64) -> CoreDuty {
        CoreDuty {
            id: NodeId::of(b"pay"),
            name: "pay".into(),
            bearer: Term::Ident("Payer".into()),
            claimant: Some(Term::Ident("Payee".into())),
            attaches,
            content: vec![due_apply(due_days)],
            meta: test_meta("pay"),
        }
    }

    fn duty_status_module(duty: CoreDuty) -> CoreModule {
        module_with_plan_decls(
            "q",
            QueryPlan::Evaluate(duty_status_term("pay")),
            vec![CoreDecl::Duty(duty)],
        )
    }

    fn run_status_at(
        module: &CoreModule,
        case: &CaseRecord,
        at: fidryn_core::Instant,
    ) -> Outcome<Value> {
        evaluate(
            module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(at, at),
            &mut Refusing,
            case,
        )
        .expect("evaluate")
    }

    fn status_state(out: Outcome<Value>) -> fidryn_core::DutyState {
        match out {
            Outcome::Determinate { value, .. } => {
                duty::parse_duty_state(&value, "pay").expect("duty state")
            }
            other => panic!("{other:?}"),
        }
    }

    fn invoice_issued_guard() -> Guard {
        Guard::Operative(PropTerm::new("InvoiceIssued", Vec::new()), String::new())
    }

    #[test]
    fn duty_status_unresolved_when_attach_guard_does_not_hold() {
        let module = duty_status_module(pay_duty(invoice_issued_guard(), 30));
        let state = status_state(
            run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate"),
        );
        assert_eq!(state.status, fidryn_core::DutyStatus::Unresolved);
        assert!(!state.breached);
    }

    #[test]
    fn duty_status_not_attached_when_attach_guard_is_denied() {
        let module = duty_status_module(pay_duty(invoice_issued_guard(), 30));
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "InvoiceIssued".into(),
            protocol: "Invoice".into(),
            established: false,
            decider: "Tribunal".into(),
            recorded_at: None,
        });
        let state = status_state(run_module(&module, "q", &case, &mut Refusing).expect("evaluate"));
        assert_eq!(state.status, fidryn_core::DutyStatus::NotAttached);
        assert_ne!(state.status, fidryn_core::DutyStatus::Unresolved);
        assert_ne!(state.status, fidryn_core::DutyStatus::Attached);
        assert!(!state.breached);
    }

    #[test]
    fn duty_status_attached_when_guard_holds_and_deadline_open() {
        let module = duty_status_module(pay_duty(Guard::Satisfied, 30));
        let invoice = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        let at = fidryn_core::Instant::parse("2026-01-15T00:00:00Z").unwrap();
        let state = status_state(run_status_at(&module, &case, at));
        assert_eq!(state.status, fidryn_core::DutyStatus::Attached);
        assert!(!state.breached);
    }

    #[test]
    fn changing_due_apply_changes_duty_status_without_rust_edits() {
        let invoice = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let at = fidryn_core::Instant::parse("2026-01-15T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        let early = duty_status_module(pay_duty(Guard::Satisfied, 0));
        let later = duty_status_module(pay_duty(Guard::Satisfied, 30));
        let early_state = status_state(run_status_at(&early, &case, at));
        let later_state = status_state(run_status_at(&later, &case, at));
        assert_eq!(early_state.status, fidryn_core::DutyStatus::Breached);
        assert!(early_state.breached);
        assert_eq!(later_state.status, fidryn_core::DutyStatus::Attached);
        assert!(!later_state.breached);
    }

    #[test]
    fn duty_status_performed_from_payment_record_keeps_late_breach() {
        let invoice = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let paid = fidryn_core::Instant::parse("2026-01-20T00:00:00Z").unwrap();
        let at = fidryn_core::Instant::parse("2026-01-20T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        case.evidence.push(EvidenceItem {
            schema: "PaymentRecord".into(),
            value: Value::Entity("Payer".into()),
            observed_at: paid,
        });
        let module = duty_status_module(pay_duty(Guard::Satisfied, 0));
        let state = status_state(run_status_at(&module, &case, at));
        assert_eq!(state.status, fidryn_core::DutyStatus::Performed);
        assert!(state.breached);
    }

    #[test]
    fn duty_status_performed_on_time_is_not_breached() {
        let invoice = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let at = fidryn_core::Instant::parse("2026-01-05T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        case.facts.insert("pay_performed".into(), Value::Bool(true));
        let module = duty_status_module(pay_duty(Guard::Satisfied, 30));
        let state = status_state(run_status_at(&module, &case, at));
        assert_eq!(state.status, fidryn_core::DutyStatus::Performed);
        assert!(!state.breached);
    }

    #[test]
    fn duty_status_honors_named_late_fact_without_invoice_date() {
        let mut case = CaseRecord::default();
        case.facts.insert("pay_late".into(), Value::Bool(true));
        let module = duty_status_module(pay_duty(Guard::Satisfied, 10));
        let state = status_state(run_module(&module, "q", &case, &mut Refusing).expect("evaluate"));
        assert_eq!(state.status, fidryn_core::DutyStatus::Breached);
        assert!(state.breached);
    }

    #[test]
    fn duty_status_call_form_matches_apply() {
        let module = module_with_plan_decls(
            "q",
            QueryPlan::Evaluate(Term::Call {
                callee: "duty_status".into(),
                args: vec![Term::Ident("pay".into())],
            }),
            vec![CoreDecl::Duty(pay_duty(Guard::Satisfied, 30))],
        );
        let state = status_state(
            run_module(&module, "q", &CaseRecord::default(), &mut Refusing).expect("evaluate"),
        );
        assert_eq!(state.status, fidryn_core::DutyStatus::Attached);
    }

    #[test]
    fn operative_assumptions_vec_does_not_make_duty_status_performed() {
        let mut case = CaseRecord::default();
        case.assumptions.push(Assumption {
            id: "hyp-performed".into(),
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        });
        let events_before = case.events.clone();
        let module = duty_status_module(pay_duty(Guard::Satisfied, 30));
        let operative =
            status_state(run_module(&module, "q", &case, &mut Refusing).expect("evaluate"));
        assert_ne!(operative.status, fidryn_core::DutyStatus::Performed);
        assert_eq!(operative.status, fidryn_core::DutyStatus::Attached);
        assert_eq!(case.events, events_before);
    }

    #[test]
    fn duty_event_performed_without_grant_does_not_make_duty_status_performed() {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.events.push(fidryn_core::LedgerEvent {
            kind: "duty".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        });
        let module = duty_status_module(pay_duty(Guard::Satisfied, 30));
        let state = status_state(run_module(&module, "q", &case, &mut Refusing).expect("evaluate"));
        assert_eq!(state.status, fidryn_core::DutyStatus::Attached);
        assert_ne!(state.status, fidryn_core::DutyStatus::Performed);
    }

    #[test]
    fn assumption_event_performed_makes_duty_status_performed() {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let payload = Value::Ctor {
            name: "Performed".into(),
            fields: BTreeMap::new(),
        };
        let mut case = CaseRecord::default();
        case.events.push(fidryn_core::LedgerEvent {
            kind: "assumption".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload: payload.clone(),
        });
        let events_before = case.events.clone();
        let module = duty_status_module(pay_duty(Guard::Satisfied, 30));
        let operative =
            status_state(run_module(&module, "q", &case, &mut Refusing).expect("evaluate"));
        assert_ne!(operative.status, fidryn_core::DutyStatus::Performed);
        assert_eq!(operative.status, fidryn_core::DutyStatus::Attached);

        let assumptions = vec![Assumption {
            id: "hyp-performed".into(),
            payload,
        }];
        case.assumptions = assumptions.clone();
        let scenario_out = evaluate_scenario(
            &module,
            &QueryName::from("q"),
            &BTreeMap::new(),
            &LegalState::new(),
            &RunContext::new(t, t),
            &mut Refusing,
            &case,
        )
        .expect("evaluate_scenario");
        let scenario = status_state(scenario_out.clone());
        assert_eq!(scenario.status, fidryn_core::DutyStatus::Performed);
        assert!(!scenario.breached);
        assert_eq!(case.events, events_before);
        let report = report_from_scenario(scenario_out, assumptions.clone());
        assert_eq!(report.execution_mode, ExecutionMode::Scenario);
        assert_eq!(report.assumptions, assumptions);
    }

    #[test]
    fn duty_step_fourth_arg_selects_instance() {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let case = CaseRecord::default();
        let session = evaluate_session(
            &module_with_plan(
                "q",
                QueryPlan::Evaluate(duty_step_instance_term(
                    "pay",
                    "attach",
                    "Alice",
                    "invoice_a",
                )),
            ),
            &QueryName::from("q"),
            &BTreeMap::new(),
            &LegalState::new(),
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        match session.outcome {
            Outcome::Determinate { ref value, .. } => match value {
                Value::Ctor { name, .. } => assert_eq!(name, "Attached"),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
        let named = session
            .bindings
            .get("duty:pay:invoice_a")
            .expect("named instance binding");
        let state = duty::parse_duty_state(named, "pay").expect("duty state");
        assert_eq!(state.status, fidryn_core::DutyStatus::Attached);
        assert_eq!(state.bearer, "Alice");
        assert_eq!(state.instance, "invoice_a");
        assert!(
            !session.bindings.contains_key("duty:pay:default"),
            "{:?}",
            session.bindings
        );
        assert!(!case.facts.contains_key("duty:pay:invoice_a"));
        assert!(!case.facts.contains_key("duty:pay:default"));
    }

    #[test]
    fn duty_step_three_arg_is_person_on_default_instance() {
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let case = CaseRecord::default();
        let session = evaluate_session(
            &module_with_plan(
                "q",
                QueryPlan::Evaluate(Term::Apply {
                    ctor: "duty_step".into(),
                    args: vec![
                        Term::Ident("pay".into()),
                        Term::Ident("attach".into()),
                        Term::Ident("Alice".into()),
                    ],
                }),
            ),
            &QueryName::from("q"),
            &BTreeMap::new(),
            &LegalState::new(),
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        let stored = session
            .bindings
            .get("duty:pay:default")
            .expect("default instance binding");
        let state = duty::parse_duty_state(stored, "pay").expect("duty state");
        assert_eq!(state.status, fidryn_core::DutyStatus::Attached);
        assert_eq!(state.bearer, "Alice");
        assert_eq!(state.instance, duty::DEFAULT_DUTY_INSTANCE);
        assert!(!session.bindings.contains_key("duty:pay:Alice"));
    }

    #[test]
    fn payment_for_one_duty_instance_does_not_perform_another() {
        let invoice = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let paid = fidryn_core::Instant::parse("2026-01-05T00:00:00Z").unwrap();
        let at = fidryn_core::Instant::parse("2026-01-05T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts
            .insert("invoice_date".into(), Value::Instant(invoice));
        case.evidence.push(EvidenceItem {
            schema: "PaymentRecord".into(),
            value: Value::Map(BTreeMap::from([(
                "instance".into(),
                Value::String("A".into()),
            )])),
            observed_at: paid,
        });
        let duty = pay_duty(Guard::Satisfied, 30);
        let module_a = module_with_plan_decls(
            "q",
            QueryPlan::Evaluate(Term::Apply {
                ctor: "duty_status".into(),
                args: vec![Term::Ident("pay".into()), Term::Ident("A".into())],
            }),
            vec![CoreDecl::Duty(duty.clone())],
        );
        let module_b = module_with_plan_decls(
            "q",
            QueryPlan::Evaluate(Term::Apply {
                ctor: "duty_status".into(),
                args: vec![Term::Ident("pay".into()), Term::Ident("B".into())],
            }),
            vec![CoreDecl::Duty(duty)],
        );
        let state_a = status_state(run_status_at(&module_a, &case, at));
        let state_b = status_state(run_status_at(&module_b, &case, at));
        assert_eq!(state_a.status, fidryn_core::DutyStatus::Performed);
        assert_ne!(state_b.status, fidryn_core::DutyStatus::Performed);
        assert_eq!(state_b.status, fidryn_core::DutyStatus::Attached);
    }

    fn compile_program(rel: &str) -> CoreModule {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel);
        let src = std::fs::read_to_string(&path).expect("program source");
        let parsed = fidryn_syntax::parse_file(&src);
        assert!(!parsed.has_errors(), "{rel}: {:?}", parsed.diagnostics);
        let hir = fidryn_hir::elaborate(&parsed, &fidryn_core::SourceManifest::default())
            .unwrap_or_else(|d| panic!("{rel}: {d:?}"));
        fidryn_check::check(&hir, &fidryn_core::SourceManifest::default())
            .unwrap_or_else(|d| panic!("{rel}: {d:?}"))
    }

    #[test]
    fn independent_transaction_atomic_does_not_keep_attach() {
        let module = compile_program("tests/programs/transaction-atomic.fr");
        let case = CaseRecord::default();
        let before = case.clone();
        let err = run_module(&module, "q", &case, &mut Refusing)
            .expect_err("illegal discharge must not commit");
        assert!(
            matches!(err, EngineError::InvalidInput(ref msg) if msg.contains("discharge")),
            "{err:?}"
        );
        assert_eq!(case, before, "failed transaction must not mutate case");
        assert!(!case.facts.contains_key("duty:pay:default"));
        let absent = run_module(
            &module_with_plan(
                "q",
                QueryPlan::Evaluate(Term::Ident("duty:pay:default".into())),
            ),
            "q",
            &case,
            &mut Refusing,
        )
        .expect_err("duty still absent");
        assert!(matches!(absent, EngineError::Unsupported(_)), "{absent:?}");
    }

    #[test]
    fn transaction_illegal_discharge_does_not_keep_attach() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "transaction".into(),
            args: vec![
                duty_step_term("pay", "attach"),
                duty_step_term("pay", "discharge"),
            ],
        });
        let case = CaseRecord::default();
        let err = run_module(&module_with_plan("q", plan), "q", &case, &mut Refusing)
            .expect_err("illegal discharge");
        assert!(
            matches!(err, EngineError::InvalidInput(ref msg) if msg.contains("discharge")),
            "{err:?}"
        );
        assert!(!case.facts.contains_key("duty:pay:default"));
        let absent = run_module(
            &module_with_plan(
                "q",
                QueryPlan::Evaluate(Term::Ident("duty:pay:default".into())),
            ),
            "q",
            &case,
            &mut Refusing,
        )
        .expect_err("duty still absent");
        assert!(matches!(absent, EngineError::Unsupported(_)), "{absent:?}");

        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let session = evaluate_session(
            &module_with_plan(
                "q",
                QueryPlan::Evaluate(Term::Apply {
                    ctor: "transaction".into(),
                    args: vec![
                        duty_step_term("pay", "attach"),
                        require_term(Term::Bool(false)),
                    ],
                }),
            ),
            &QueryName::from("q"),
            &BTreeMap::new(),
            &LegalState::new(),
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("transaction suspends");
        assert!(
            matches!(session.outcome, Outcome::Suspended { .. }),
            "{:?}",
            session.outcome
        );
        let cont = session.continuation.expect("continuation");
        assert!(
            !cont.bindings.contains_key("duty:pay:default"),
            "{:?}",
            cont.bindings
        );
    }

    #[test]
    fn transaction_suspend_then_resume_commits_once() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "transaction".into(),
            args: vec![
                duty_step_term("pay", "attach"),
                Term::Apply {
                    ctor: "require_authority".into(),
                    args: vec![Term::Ident("missing_action".into())],
                },
                duty_step_term("pay", "perform"),
            ],
        });
        let module = module_with_plan("q", plan);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let before = case.clone();
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(first.outcome, Outcome::Suspended { .. }),
            "{:?}",
            first.outcome
        );
        let cont = first.continuation.as_ref().expect("continuation");
        assert!(
            !cont.bindings.contains_key("duty:pay:default"),
            "{:?}",
            cont.bindings
        );
        assert!(!first.bindings.contains_key("duty:pay:default"));
        assert_eq!(case, before);

        let mut granted = case.clone();
        granted.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![
                Value::String("missing_action".into()),
                Value::String("attach".into()),
                Value::String("perform".into()),
            ]),
        );
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &granted,
        )
        .expect("resume");
        match &second.outcome {
            Outcome::Determinate { value, .. } => match value {
                Value::Ctor { name, .. } => assert_eq!(name, "Performed"),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
        assert!(second.continuation.is_none());
        let committed = second
            .bindings
            .get("duty:pay:default")
            .expect("committed duty binding");
        let duty_state = duty::parse_duty_state(committed, "pay").expect("duty state");
        assert_eq!(duty_state.status, fidryn_core::DutyStatus::Performed);
        assert_eq!(duty_state.instance, duty::DEFAULT_DUTY_INSTANCE);
        assert_eq!(case, before);
        assert!(!case.facts.contains_key("duty:pay:default"));
        assert!(!granted.facts.contains_key("duty:pay:default"));
    }

    #[test]
    fn nested_seq_under_add_skips_completed_attach_on_resume() {
        let plan = QueryPlan::Evaluate(Term::Binary {
            op: BinOp::Add,
            left: Box::new(Term::Int(10)),
            right: Box::new(seq_term(vec![
                duty_step_term("pay", "attach"),
                Term::Apply {
                    ctor: "require_authority".into(),
                    args: vec![Term::Ident("missing_action".into())],
                },
                Term::Int(2),
            ])),
        });
        let module = module_with_plan("q", plan);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(first.outcome, Outcome::Suspended { .. }),
            "{:?}",
            first.outcome
        );
        let cont = first.continuation.as_ref().expect("continuation");
        assert_eq!(cont.seq_index, 1);
        assert_eq!(cont.completed.len(), 1);
        assert_eq!(cont.completed[0].display_label(), "Attached");
        let mut granted = case.clone();
        granted.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![
                Value::String("attach".into()),
                Value::String("missing_action".into()),
            ]),
        );
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &granted,
        )
        .expect("resume");
        match second.outcome {
            Outcome::Determinate {
                value: Value::Int(12),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
        assert!(second.continuation.is_none());
    }

    #[test]
    fn nested_seq_under_add_does_not_double_attach_on_same_case_resume() {
        let plan = QueryPlan::Evaluate(Term::Binary {
            op: BinOp::Add,
            left: Box::new(Term::Int(10)),
            right: Box::new(seq_term(vec![
                duty_step_term("pay", "attach"),
                Term::Ident("judgment".into()),
                Term::Int(2),
            ])),
        });
        let module = module_with_plan("q", plan);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(first.outcome, Outcome::Suspended { .. }),
            "{:?}",
            first.outcome
        );
        let mut handler = Scripted::resume_only(&["other"]);
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut handler,
            &case,
        )
        .expect("resume");
        match second.outcome {
            Outcome::Determinate {
                value: Value::Int(12),
                ..
            } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rolled_back_nested_sequence_is_reexecuted_before_transaction_commit() {
        let plan = QueryPlan::Evaluate(Term::Apply {
            ctor: "transaction".into(),
            args: vec![
                seq_term(vec![duty_step_term("pay", "attach"), Term::Bool(true)]),
                Term::Apply {
                    ctor: "require_authority".into(),
                    args: vec![Term::Ident("release".into())],
                },
                Term::Bool(true),
            ],
        });
        let module = module_with_plan("q", plan);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let session = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(session.outcome, Outcome::Suspended { .. }),
            "{:?}",
            session.outcome
        );
        assert!(!session.bindings.contains_key("duty:pay:default"));
        let cont = session.continuation.as_ref().expect("continuation");
        assert!(!cont.bindings.contains_key("duty:pay:default"));
        assert_eq!(cont.seq_index, 0);
        assert!(cont.completed.is_empty());

        let mut granted = case.clone();
        granted.facts.insert(
            "authority_grants".into(),
            Value::Set(vec![
                Value::String("attach".into()),
                Value::String("release".into()),
            ]),
        );
        let resumed = resume(
            session,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &granted,
        )
        .expect("resume with grant");
        assert!(
            matches!(resumed.outcome, Outcome::Determinate { .. }),
            "{:?}",
            resumed.outcome
        );
        assert!(
            resumed.bindings.contains_key("duty:pay:default"),
            "a rolled-back prefix cannot be marked completed and skipped: {:?}",
            resumed.bindings
        );
        let committed = resumed
            .bindings
            .get("duty:pay:default")
            .expect("committed duty binding");
        let duty_state = duty::parse_duty_state(committed, "pay").expect("duty state");
        assert_eq!(duty_state.status, fidryn_core::DutyStatus::Attached);
        assert!(resumed.continuation.is_none());
    }

    #[test]
    fn rebase_cannot_reuse_a_require_that_is_now_false() {
        let plan = QueryPlan::Evaluate(seq_term(vec![
            require_term(Term::Ident("gate".into())),
            Term::Apply {
                ctor: "determined".into(),
                args: vec![Term::Apply {
                    ctor: "P".into(),
                    args: vec![Term::Ident("A".into())],
                }],
            },
        ]));
        let module = module_with_plan("q", plan);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let mut case = CaseRecord::default();
        case.facts.insert("gate".into(), Value::Bool(true));
        let session = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(session.outcome, Outcome::Suspended { .. }),
            "{:?}",
            session.outcome
        );
        let cont = session.continuation.as_ref().expect("continuation");
        assert_eq!(cont.seq_index, 1);
        assert_eq!(cont.completed.len(), 1);

        case.facts.insert("gate".into(), Value::Bool(false));
        case.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: true,
            decider: "Reviewer".into(),
            recorded_at: Some(t),
        });
        let fresh = evaluate(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("fresh evaluation");
        assert!(
            !matches!(
                fresh,
                Outcome::Determinate {
                    value: Value::Bool(true),
                    ..
                }
            ),
            "{fresh:?}"
        );
        let resumed = resume(
            session,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("resume");
        assert_eq!(
            resumed.outcome, fresh,
            "either reject snapshot changes or invalidate dependent completed work"
        );
    }

    #[test]
    fn resuming_a_nested_sequence_without_new_evidence_stays_suspended() {
        let plan = seq_terms(vec![
            seq_term(vec![Term::Int(7)]),
            seq_term(vec![require_term(determined_gate()), Term::Int(9)]),
        ]);
        let module = module_with_plan("q", plan);
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let first = evaluate_session(
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(first.outcome, Outcome::Suspended { .. }),
            "{:?}",
            first.outcome
        );
        let second = resume(
            first,
            &module,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("resume");
        assert!(
            matches!(second.outcome, Outcome::Suspended { .. }),
            "skipped nested seq must not steal the inner require frame: {:?}",
            second.outcome
        );
        assert!(second.continuation.is_some());
        assert!(
            !matches!(
                second.outcome,
                Outcome::Determinate {
                    value: Value::Int(9),
                    ..
                }
            ),
            "{:?}",
            second.outcome
        );
    }

    #[test]
    fn a_rebase_runs_the_new_query_not_the_old_residual() {
        let old = module_with_plan(
            "q",
            seq_terms(vec![require_term(determined_gate()), Term::Int(1)]),
        );
        let t = fidryn_core::Instant::parse("2026-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let args = BTreeMap::new();
        let state = LegalState::new();
        let case = CaseRecord::default();
        let first = evaluate_session(
            &old,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("evaluate_session");
        assert!(
            matches!(first.outcome, Outcome::Suspended { .. }),
            "{:?}",
            first.outcome
        );
        let new = module_with_plan("q", QueryPlan::Evaluate(Term::Int(2)));
        let second = resume(
            first,
            &new,
            &QueryName::from("q"),
            &args,
            &state,
            &ctx,
            &mut Refusing,
            &case,
        )
        .expect("resume");
        match second.outcome {
            Outcome::Determinate {
                value: Value::Int(2),
                ..
            } => {}
            other => panic!("rebase must run the new query, got {other:?}"),
        }
        assert!(second.continuation.is_none());
    }
}
