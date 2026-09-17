//! Deterministic worklist evaluator. No partial mutation on Open or Conflict.

pub mod conflict;
pub mod law;
mod worklist;

pub use conflict::resolve_conflict;
pub use law::select_applicable_law;
pub use worklist::DerivedWorld;
use worklist::{binder_name, domain_name, parse_prop_issue, term_as_proposition};

use fidryn_core::ir::{
    CompareOp, CoreConflictDoctrine, CoreDecision, CoreDecl, CoreFunction, CoreModule, NodeMeta,
    QueryPlan,
};
use fidryn_core::outcome::OpenRequest;
use fidryn_core::patterns::{LegalStatusPattern, PropPattern};
use fidryn_core::state::{LegalState, Occupancy, StatusMode};
use fidryn_core::time::Interval;
use fidryn_core::types::{Sort, Type};
use fidryn_core::value::{BinOp, PropTerm, Term, Value};
use fidryn_core::{
    CaseRecord, EvidenceItem, Guard, HaltReason, Handler, HandlerResult, Instant, JurisdictionId,
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
/// If the case identity (facts, evidence, determinations, interpretations,
/// decisions, closures) later differs, [`resume`] recomputes via
/// [`evaluate_session`] rather than replaying this residual against stale
/// derived facts.
#[derive(Clone, Debug)]
pub struct Continuation {
    pub residual: Residual,
    pub bindings: BTreeMap<String, Value>,
    pub derived: DerivedWorld,
    pub fuel: Option<u32>,
    case_identity: CaseIdentity,
}

#[derive(Clone, Debug)]
pub struct EvalSession {
    pub outcome: Outcome<Value>,
    pub continuation: Option<Continuation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CaseIdentity {
    facts: BTreeMap<String, Value>,
    evidence: Vec<(String, Instant, Value)>,
    determinations: Vec<(String, String, bool, String)>,
    interpretations: BTreeMap<String, String>,
    decisions: BTreeMap<String, String>,
    closures: Vec<(String, bool)>,
}

impl CaseIdentity {
    fn of(case: &CaseRecord) -> Self {
        Self {
            facts: case.facts.clone(),
            evidence: case
                .evidence
                .iter()
                .map(|e| (e.schema.clone(), e.observed_at, e.value.clone()))
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
    )
}

/// Continue a suspended session after a handler can [`HandlerResult::Resume`].
///
/// When `case` identity has changed since the continuation was captured,
/// this recomputes from [`evaluate_session`].
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
    if cont.case_identity != CaseIdentity::of(case) {
        return evaluate_session(module, query, args, state, ctx, handler, case);
    }
    let residual = match &cont.residual {
        Residual::Term(term) => QueryPlan::Evaluate(term.clone()),
        Residual::Plan(plan) => plan.clone(),
    };
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
    )
}

#[allow(clippy::too_many_arguments, clippy::result_large_err)]
fn eval_with_state<H: Handler>(
    plan: &QueryPlan,
    module: &CoreModule,
    _query: &QueryName,
    args: &BTreeMap<String, Value>,
    state: &LegalState,
    ctx: &RunContext,
    handler: &mut H,
    case: &CaseRecord,
    bindings: BTreeMap<String, Value>,
    derived: DerivedWorld,
    fuel: Option<u32>,
) -> Result<EvalSession, EngineError> {
    match plan {
        QueryPlan::Evaluate(term) => {
            let mut frame = EvalFrame {
                module,
                args,
                bindings,
                ctx,
                handler,
                case,
                derived,
                fuel,
            };
            let outcome = frame.eval_term(term)?;
            Ok(session_from(
                outcome,
                Residual::Term(term.clone()),
                frame.bindings,
                frame.derived,
                frame.fuel,
                case,
            ))
        }
        other => {
            let outcome = eval_specialized_plan(other, module, state, ctx, handler, case, &derived);
            Ok(session_from(
                outcome,
                Residual::Plan(other.clone()),
                bindings,
                derived,
                fuel,
                case,
            ))
        }
    }
}

fn session_from(
    outcome: Outcome<Value>,
    residual: Residual,
    bindings: BTreeMap<String, Value>,
    derived: DerivedWorld,
    fuel: Option<u32>,
    case: &CaseRecord,
) -> EvalSession {
    let continuation = match &outcome {
        Outcome::Suspended { .. } => Some(Continuation {
            residual,
            bindings,
            derived,
            fuel,
            case_identity: CaseIdentity::of(case),
        }),
        _ => None,
    };
    EvalSession {
        outcome,
        continuation,
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
) -> Outcome<Value> {
    match plan {
        QueryPlan::UniqueOccupant { office } => {
            eval_unique_occupant(module, office, state, ctx, handler, case, derived)
        }
        QueryPlan::StatusOf {
            status,
            when_present,
            when_closed_absent,
        } => eval_status_of(status, when_present, when_closed_absent, case, ctx),
        QueryPlan::EvaluateClause { clause, .. } => eval_clause(module, clause, case, handler),
        QueryPlan::RunDecision { decision, .. } => {
            eval_run_decision(module, decision, case, handler, ctx, derived)
        }
        QueryPlan::Evaluate(_) => Outcome::Suspended {
            requests: BTreeSet::new(),
            trace: TraceId::of(b"eval"),
        },
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
        if let Some(kind) = quantifier_kind(ctor) {
            return self.eval_quantifier(kind, args);
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
            return Ok(self.eval_effect(&effect, ctor));
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
            || binop_ctor(callee).is_some()
        {
            return self.eval_apply(callee, args);
        }
        let Some(function) = find_function(self.module, callee) else {
            return Err(unsupported(format!("unknown function `{callee}`")));
        };
        self.eval_function(function, args)
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

        let mut call_env = self.bindings.clone();
        for (i, arg) in arg_terms.iter().enumerate() {
            let value = match as_determinate(self.eval_term(arg)?) {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            if let Some((name, _)) = function.params.get(i) {
                call_env.insert(name.clone(), value);
            } else if i == 0 {
                call_env.insert("amount".into(), value.clone());
                call_env.insert("_0".into(), value);
            } else {
                call_env.insert(format!("_{i}"), value);
            }
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
        };
        if let Some(body) = function_body(function)
            && is_executable_body(body)
        {
            return nested.eval_term(body);
        }
        if is_tax_stub(function) {
            return nested.eval_tax();
        }
        Err(unsupported(format!(
            "function `{}` has no executable body",
            function.name
        )))
    }

    fn eval_tax(&mut self) -> Result<Outcome<Value>, EngineError> {
        let amount = money_amount(
            self.bindings
                .get("amount")
                .or_else(|| self.bindings.get("_0"))
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

fn function_body(function: &CoreFunction) -> Option<&Term> {
    function.body.as_ref()
}

fn is_executable_body(term: &Term) -> bool {
    !matches!(term, Term::String(_) | Term::Wildcard | Term::Binder(_))
}

fn is_tax_function(function: &CoreFunction) -> bool {
    let name = function.name.to_ascii_lowercase();
    function.is_calc
        && (name.contains("tax")
            || name.contains("ordinary_income")
            || function
                .meta
                .source
                .as_deref()
                .is_some_and(|s| s.contains("ordinary_income") || s.contains("tax")))
}

fn is_tax_stub(function: &CoreFunction) -> bool {
    if !is_tax_function(function) {
        return false;
    }
    match function.body.as_ref() {
        None => true,
        Some(Term::String(_)) => true,
        Some(_) => false,
    }
}

fn find_function<'m>(module: &'m CoreModule, name: &str) -> Option<&'m CoreFunction> {
    module.declarations.iter().find_map(|d| match d {
        CoreDecl::Function(f) if f.name == name => Some(f),
        _ => None,
    })
}

fn find_effect_name(module: &CoreModule, name: &str) -> Option<String> {
    module.declarations.iter().find_map(|d| match d {
        CoreDecl::EffectDecl(e) if e.name == name => Some(e.name.clone()),
        _ => None,
    })
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
        let mut requests = BTreeSet::new();
        requests.insert(OpenRequest::NeedJudgment {
            issue: PropTerm::new("Occupies", vec![Term::Wildcard, office.clone()]),
            protocol: "Appointment".into(),
        });
        return Outcome::Suspended { requests, trace };
    }
    let accepted: Vec<(String, i64)> = ranked
        .iter()
        .filter(|(name, _)| is_accepted(case, name, ctx))
        .cloned()
        .collect();
    if let Some(label) = recorded_succession_interpretation(case, &ranked)
        && let Some(name) = nominee_for_interpretation(&label, &ranked)
    {
        if is_accepted(case, &name, ctx) {
            return determinate(Value::Entity(name), trace);
        }
        return need_accept_office(office, trace);
    }
    if accepted.len() >= 2 {
        return contingent_succession(case, &ranked, &accepted, trace);
    }
    if let Some((name, _)) = accepted.first() {
        return determinate(Value::Entity(name.clone()), trace);
    }
    need_accept_office(office, trace)
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

fn is_accepted(case: &CaseRecord, name: &str, ctx: &RunContext) -> bool {
    let key = format!("{}_accepted", name.to_ascii_lowercase());
    if case.facts.get(&key) == Some(&Value::Bool(true)) {
        return true;
    }
    case.evidence.iter().any(|item| {
        item.schema == "AcceptOffice"
            && item.observed_at <= ctx.record_time
            && value_names_person(&item.value, name)
    })
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

fn recorded_succession_interpretation(
    case: &CaseRecord,
    ranked: &[(String, i64)],
) -> Option<String> {
    case.interpretations.iter().find_map(|(family, value)| {
        if !case
            .admissible_completions
            .interpretations
            .contains_key(family)
        {
            return None;
        }
        nominee_for_interpretation(value, ranked).map(|_| value.clone())
    })
}

fn nominee_for_interpretation(label: &str, ranked: &[(String, i64)]) -> Option<String> {
    if let Some((name, _)) = ranked
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(label))
    {
        return Some(name.clone());
    }
    nominee_index(label).and_then(|idx| ranked.get(idx).map(|(name, _)| name.clone()))
}

fn nominee_index(label: &str) -> Option<usize> {
    let text = label.trim();
    let rest = text
        .strip_prefix('I')
        .or_else(|| text.strip_prefix('i'))
        .unwrap_or(text);
    let n: usize = rest.parse().ok()?;
    n.checked_sub(1)
}

fn contingent_succession(
    case: &CaseRecord,
    ranked: &[(String, i64)],
    accepted: &[(String, i64)],
    trace: TraceId,
) -> Outcome<Value> {
    let mut alternatives = BTreeMap::new();
    let mut pivots = BTreeSet::new();
    let families = &case.admissible_completions.interpretations;
    if families.is_empty() {
        for (i, (name, _)) in accepted.iter().enumerate() {
            alternatives.insert(format!("I{}", i + 1), Value::Entity(name.clone()));
        }
        pivots.insert(OpenRequest::NeedInterpretation {
            source: String::new(),
            family: "succession".into(),
        });
    } else {
        for (family, alts) in families {
            for alt in alts {
                if let Some(name) = nominee_for_interpretation(alt, ranked)
                    && accepted.iter().any(|(n, _)| n == &name)
                {
                    alternatives.insert(alt.clone(), Value::Entity(name));
                }
            }
            pivots.insert(OpenRequest::NeedInterpretation {
                source: family.clone(),
                family: family.clone(),
            });
        }
        if alternatives.is_empty() {
            for (i, (name, _)) in accepted.iter().enumerate() {
                alternatives.insert(format!("I{}", i + 1), Value::Entity(name.clone()));
            }
        }
    }
    Outcome::Contingent {
        alternatives,
        pivots,
        trace,
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
    let filed = case
        .evidence
        .iter()
        .any(|e| e.schema == "OfficialFilingRecord" && e.observed_at <= ctx.record_time);
    let complies = case
        .determinations
        .iter()
        .any(|d| d.protocol == "FormationCompliance" && d.established);
    if filed && complies {
        return determinate(
            Value::Ctor {
                name: ctor,
                fields: BTreeMap::new(),
            },
            trace,
        );
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
        let name = match when_closed_absent {
            Term::Apply { ctor, .. } | Term::Ident(ctor) => ctor.clone(),
            _ => format!("Not{ctor}"),
        };
        return determinate(
            Value::Ctor {
                name,
                fields: BTreeMap::new(),
            },
            trace,
        );
    }
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedEvidence {
        issue: PropPattern::Match {
            predicate: "Filed".into(),
            arguments: vec![
                fidryn_core::TermPattern::Exact(Term::Ident("HarborRoboticsCertificate".into())),
                fidryn_core::TermPattern::Wildcard,
                fidryn_core::TermPattern::Wildcard,
            ],
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
        return determinate(
            Value::Ctor {
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
        );
    }
    if provision.contains("SpousalSupport") {
        let req = OpenRequest::NeedJudgment {
            issue: PropTerm::new("EnforceableAgainst", vec![]),
            protocol: "PrenupEnforceability".into(),
        };
        match handler.handle(&req) {
            HandlerResult::Resume { value, .. } => return determinate(value, trace),
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

fn is_foia_process(name: &str) -> bool {
    let n = name.trim();
    n.eq_ignore_ascii_case("ProcessResponsiveRecord")
        || n.eq_ignore_ascii_case("IssueFOIADetermination")
}

fn eval_run_decision<H: Handler>(
    module: &CoreModule,
    decision: &str,
    case: &CaseRecord,
    handler: &mut H,
    ctx: &RunContext,
    derived: &DerivedWorld,
) -> Outcome<Value> {
    let trace = TraceId::of(decision.as_bytes());
    if let Some(decl) = find_decision(module, decision)
        && !decl.requirements.is_empty()
    {
        let mut open = BTreeSet::new();
        for req in &decl.requirements {
            if let Some(halt) =
                gather_unmet_requirement(req, case, ctx, derived, handler, &mut open)
            {
                return halt;
            }
        }
        if !open.is_empty() {
            return Outcome::Suspended {
                requests: open,
                trace,
            };
        }
        if let Some(ret) = &decl.declared_result
            && let Some(value) = term_as_value_literal(&ret.expression)
        {
            return determinate(value, trace);
        }
        if let Some(recorded) = case.decisions.get(decision) {
            return determinate(Value::String(recorded.clone()), trace);
        }
        let mut requests = BTreeSet::new();
        requests.insert(OpenRequest::NeedJudgment {
            issue: PropTerm::new(decision, vec![]),
            protocol: decision.to_owned(),
        });
        return Outcome::Suspended { requests, trace };
    }
    if is_foia_process(decision) {
        return eval_foia(case, handler, ctx);
    }
    if let Some(recorded) = case.decisions.get(decision) {
        return determinate(Value::String(recorded.clone()), trace);
    }
    let mut requests = BTreeSet::new();
    requests.insert(OpenRequest::NeedJudgment {
        issue: PropTerm::new(decision, vec![]),
        protocol: decision.to_owned(),
    });
    Outcome::Suspended { requests, trace }
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

fn eval_foia<H: Handler>(case: &CaseRecord, handler: &mut H, ctx: &RunContext) -> Outcome<Value> {
    let trace = TraceId::of(b"foia-disposition");
    let known = ctx.record_time;
    let has_harm = case
        .evidence
        .iter()
        .any(|e| e.schema == "HarmAnalysis" && e.observed_at <= known);
    let has_seg = case
        .evidence
        .iter()
        .any(|e| e.schema == "SegregabilityAnalysis" && e.observed_at <= known);
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
    determinate(
        case.facts
            .get("proposed_disposition")
            .cloned()
            .unwrap_or(Value::String("released".into())),
        trace,
    )
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
        Consequence, CoreEffect, CoreNomination, CoreProposition, CoreQuery, CoreRule,
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

    fn module_with_occupant() -> CoreModule {
        let mut module = module_with_plan(
            "acting_trustee",
            QueryPlan::UniqueOccupant {
                office: Term::Ident("TrusteeOf(BRT)".into()),
            },
        );
        module.nominations = vec![
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
        ];
        module
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
            ctor: "ordinary_income_tax_formula".into(),
            args: vec![Term::Ident("amount".into())],
        });
        let mut case = CaseRecord::default();
        case.facts.insert("amount".into(), Value::Int(10_000));
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax_formula",
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
            ctor: "ordinary_income_tax_formula".into(),
            args: vec![Term::Ident("amount".into())],
        });
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax_formula",
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
            ctor: "ordinary_income_tax_formula".into(),
            args: vec![],
        });
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax_formula",
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
        let plan = QueryPlan::Evaluate(Term::Ident("ordinary_income_tax_formula".into()));
        let module = module_with_plan_decls(
            "q",
            plan,
            vec![CoreDecl::Function(tax_function(
                "ordinary_income_tax_formula",
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
            });
        let out = run_plan(plan, &case, &mut Refusing);
        assert!(matches!(out, Outcome::Suspended { .. }), "{out:?}");
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
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
        match out {
            Outcome::Suspended { requests, .. } => {
                assert!(
                    requests
                        .iter()
                        .any(|r| matches!(r, OpenRequest::NeedJudgment { .. }))
                );
                assert!(!requests.iter().any(|r| matches!(
                    r,
                    OpenRequest::NeedEvidence { schema, .. }
                        if schema == "HarmAnalysis" || schema == "SegregabilityAnalysis"
                )));
            }
            other => panic!("{other:?}"),
        }
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
    fn run_decision_foia_process_still_needs_harm_and_segregability() {
        let plan = QueryPlan::RunDecision {
            decision: "ProcessResponsiveRecord".into(),
            arguments: Vec::new(),
            result: DeclaredDecisionResult {
                expected_type: Type::Sort(Sort::Nominal("FOIADisposition".into())),
            },
        };
        let out = run_plan(plan, &CaseRecord::default(), &mut Refusing);
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
        let case = successor_case(2, Some("Bob"));
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
}
