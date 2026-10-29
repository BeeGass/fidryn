//! Bounded constitutive/derive worklist. The case record is never mutated.

use crate::trace::{DerivationStep, record_step};
use fidryn_core::ir::{CompareOp, Consequence, CoreDecl, CoreEffect, CoreModule, CoreRule, Guard};
use fidryn_core::value::{PropTerm, Term, Value};
use fidryn_core::{CaseRecord, EngineError, FrozenCaseView, RunContext};
use std::collections::{BTreeMap, BTreeSet};

const WORKLIST_FUEL: u32 = 64;
const MAX_SUBSTITUTIONS: usize = 256;

/// One conjunctive support set. Empty means axiomatic (seed or same-rule replacement).
type SupportSet = BTreeSet<GroundProp>;
/// Alternative justifications for a held proposition.
type Justifications = BTreeSet<SupportSet>;

/// Staged derived propositions, denials, and observed schemas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DerivedWorld {
    held: BTreeSet<GroundProp>,
    denied: BTreeSet<GroundProp>,
    observed: BTreeSet<String>,
    support: BTreeMap<GroundProp, Justifications>,
    base: BTreeSet<GroundProp>,
    withdrawn: BTreeSet<GroundProp>,
    seeded_determinations: Vec<SeededDetermination>,
    conflicts: BTreeSet<GroundProp>,
    assumed: BTreeSet<GroundProp>,
    assumed_denied: BTreeSet<GroundProp>,
    scenario: bool,
    trace: Vec<DerivationStep>,
    provenance: BTreeMap<GroundProp, BTreeSet<String>>,
    work_units: u64,
    operation_limit: u64,
    judgment_requests: BTreeMap<GroundProp, BTreeSet<fidryn_core::OpenRequest>>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct GroundProp {
    predicate: String,
    arguments: Vec<Term>,
}

/// A case determination as seeded, including protocol and decider.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SeededDetermination {
    prop: GroundProp,
    protocol: String,
    decider: String,
    established: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hold {
    Yes,
    No,
    Unknown,
}

impl DerivedWorld {
    /// Seed from the case and fire `CoreDecl::Rule` until quiescence or fuel exhaustion.
    pub fn compute(
        module: &CoreModule,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
    ) -> Result<Self, EngineError> {
        Self::compute_in_mode(
            module,
            case,
            ctx,
            args,
            false,
            crate::ExecutionBudget::default(),
        )
    }

    pub(crate) fn compute_with_budget(
        module: &CoreModule,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
        scenario: bool,
        budget: crate::ExecutionBudget,
    ) -> Result<Self, EngineError> {
        Self::compute_in_mode(module, case, ctx, args, scenario, budget)
    }

    fn compute_in_mode(
        module: &CoreModule,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
        scenario: bool,
        budget: crate::ExecutionBudget,
    ) -> Result<Self, EngineError> {
        if budget.operations == 0 {
            return Err(EngineError::FuelExhausted { remaining: 0 });
        }
        let input_units = input_cost(module, case, args, u64::from(budget.operations))?;
        if fidryn_core::ValidatedCompletionModel::admit(case).is_err() {
            return Ok(Self {
                scenario,
                work_units: input_units,
                operation_limit: u64::from(budget.operations),
                ..Self::default()
            });
        }
        let mut world = Self::seed(module, case, ctx, args);
        world.work_units = input_units;
        world.operation_limit = u64::from(budget.operations);
        world.scenario = scenario;
        if scenario {
            for assumption in &case.assumptions {
                record_step(
                    &mut world.trace,
                    "assumption",
                    format!("{assumption:?}"),
                    Vec::new(),
                );
                if let Value::Map(fields) = &assumption.payload {
                    for (issue, value) in fields {
                        let prop = GroundProp::from_prop(&parse_prop_issue(issue));
                        match value {
                            Value::Bool(true) => {
                                world.assumed.insert(prop);
                            }
                            Value::Bool(false) => {
                                world.assumed_denied.insert(prop);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if world.trace.len() >= 100_000 || crate::trace::exhausted(&world.trace) {
            return Err(EngineError::FuelExhausted { remaining: 0 });
        }
        let observation_world = world.clone();
        for proposition in crate::judgment::observations(
            module,
            case,
            ctx,
            &observation_world,
            &mut world.work_units,
            &mut world.trace,
        )? {
            world.insert_held(proposition);
        }
        let context_world = world.clone();
        let findings = crate::judgment::evaluate(
            module,
            case,
            ctx,
            args,
            &context_world,
            &mut world.work_units,
            &mut world.trace,
        )?;
        for finding in findings {
            let ground = GroundProp::from_prop(&finding.proposition);
            world
                .provenance
                .entry(ground.clone())
                .or_default()
                .insert(finding.trace);
            if let Some(established) = finding.result {
                world.seeded_determinations.push(SeededDetermination {
                    prop: ground.clone(),
                    protocol: finding.protocol,
                    decider: "DeclaredEvidencePolicy".into(),
                    established,
                });
                if established && finding.operative {
                    world.insert_held(finding.proposition);
                } else if !established {
                    world.insert_denied(finding.proposition);
                }
            } else {
                world
                    .judgment_requests
                    .entry(ground)
                    .or_default()
                    .extend(finding.requests);
            }
        }
        let rules: Vec<&CoreRule> = module
            .declarations
            .iter()
            .filter_map(|decl| match decl {
                CoreDecl::Rule(rule) => Some(rule),
                _ => None,
            })
            .collect();
        if rules.is_empty() {
            return Ok(world);
        }
        let candidates = ground_candidates(module, case, args);
        for _ in 0..WORKLIST_FUEL {
            let before = world.clone();
            let mut pending = Vec::new();
            let mut additions = BTreeSet::new();
            let mut withdrawals = BTreeSet::new();
            for rule in &rules {
                spend_work(&mut world.work_units, 1, world.operation_limit)?;
                if !rule_in_force(rule, ctx) {
                    continue;
                }
                if rule.selection.is_some() {
                    record_step(
                        &mut world.trace,
                        "open_selection",
                        format!(
                            "rule={} requires declared selection {:?}",
                            rule.name, rule.selection
                        ),
                        Vec::new(),
                    );
                    continue;
                }
                let binders = rule_binders(rule, module);
                for subst in substitutions(&binders, &candidates)? {
                    spend_work(
                        &mut world.work_units,
                        guard_cost(&rule.guard),
                        world.operation_limit,
                    )?;
                    let (effects, justifications) =
                        match before.guard_holds(&rule.guard, &subst, case, ctx) {
                            Hold::Yes => (
                                &rule.consequences,
                                before.guard_justifications(
                                    &rule.guard,
                                    &subst,
                                    case,
                                    ctx,
                                    &mut world.work_units,
                                )?,
                            ),
                            Hold::No => match &rule.fallback {
                                Some(effects) => (effects, axiomatic_justification()),
                                None => continue,
                            },
                            Hold::Unknown => continue,
                        };
                    for effect in effects {
                        if !matches!(
                            effect.consequence,
                            Consequence::Derive(_)
                                | Consequence::Establish(_)
                                | Consequence::Terminate(_)
                                | Consequence::Suspend(_)
                        ) {
                            return Err(EngineError::Unsupported(format!(
                                "rule `{}` requires unsupported institutional effect {:?}",
                                rule.name, effect.consequence
                            )));
                        }
                        match &effect.consequence {
                            Consequence::Derive(prop) | Consequence::Establish(prop) => {
                                additions.insert(GroundProp::from_prop(&subst_prop(prop, &subst)));
                            }
                            Consequence::Terminate(prop) | Consequence::Suspend(prop) => {
                                withdrawals
                                    .insert(GroundProp::from_prop(&subst_prop(prop, &subst)));
                            }
                            _ => {}
                        }
                    }
                    pending.push((effects, subst, justifications));
                }
            }
            world
                .conflicts
                .extend(additions.intersection(&withdrawals).cloned());
            if !world.conflicts.is_empty() {
                record_step(
                    &mut world.trace,
                    "norm_conflict",
                    format!(
                        "conflicting staged effects: {:?}; candidate effects={pending:?}",
                        world.conflicts
                    ),
                    Vec::new(),
                );
                return Ok(world);
            }
            for (effects, subst, justifications) in pending {
                if world.trace.len().saturating_add(effects.len()) >= 100_000 {
                    return Err(EngineError::FuelExhausted { remaining: 0 });
                }
                let support_cost = justifications.iter().fold(0_u64, |cost, support| {
                    cost.saturating_add(1 + support.len() as u64)
                });
                spend_work(
                    &mut world.work_units,
                    (effects.len() as u64).saturating_mul(support_cost),
                    world.operation_limit,
                )?;
                world.apply_effects(effects, &subst, &justifications);
                if crate::trace::exhausted(&world.trace) {
                    return Err(EngineError::FuelExhausted { remaining: 0 });
                }
            }
            world.retract_unsupported();
            world.reseed_base();
            if world.held == before.held
                && world.denied == before.denied
                && world.support == before.support
                && world.withdrawn == before.withdrawn
            {
                return Ok(world);
            }
        }
        Err(EngineError::FuelExhausted { remaining: 0 })
    }

    /// Operations consumed while admitting evidence and reaching the fixed point.
    pub fn work_units(&self) -> u64 {
        self.work_units
    }
    pub(crate) fn operation_limit(&self) -> u64 {
        self.operation_limit
    }

    pub fn holds(&self, prop: &PropTerm) -> bool {
        self.held.iter().any(|held| held.satisfies(prop, false))
    }

    pub fn denied(&self, prop: &PropTerm) -> bool {
        let want = GroundProp::from_prop(prop);
        self.denied.iter().any(|denied| {
            names_eq(&denied.predicate, &want.predicate) && denied.arguments == want.arguments
        })
    }

    pub fn observed(&self, schema: &str) -> bool {
        self.observed.iter().any(|s| names_eq(s, schema))
    }

    /// Whether `guard` holds under the empty substitution, as in worklist firing.
    pub fn is_guard_held(&self, guard: &Guard, case: &CaseRecord, ctx: &RunContext) -> bool {
        self.guard_holds(guard, &BTreeMap::new(), case, ctx) == Hold::Yes
    }

    /// Whether `guard` is established false (`Hold::No`), not merely unknown.
    pub fn is_guard_denied(&self, guard: &Guard, case: &CaseRecord, ctx: &RunContext) -> bool {
        self.guard_holds(guard, &BTreeMap::new(), case, ctx) == Hold::No
    }

    pub fn holds_named(&self, predicate: &str) -> bool {
        self.holds(&PropTerm::new(predicate, Vec::new()))
    }

    /// Modal `determined`: a qualified denial is never true, even if also held.
    pub(crate) fn modal_determined(&self, prop: &PropTerm) -> Option<bool> {
        let mut established = false;
        let mut denied = false;
        for rec in &self.seeded_determinations {
            if !rec.applies_to(prop) {
                continue;
            }
            if rec.established {
                established = true;
            } else {
                denied = true;
            }
        }
        match (established, denied) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }

    pub(crate) fn is_scenario(&self) -> bool {
        self.scenario
    }

    pub fn derivation(&self) -> &[DerivationStep] {
        &self.trace
    }

    pub(crate) fn judgment_requests(
        &self,
        proposition: &PropTerm,
    ) -> Option<&BTreeSet<fidryn_core::OpenRequest>> {
        self.judgment_requests
            .get(&GroundProp::from_prop(proposition))
    }

    pub fn has_conflict(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub(crate) fn proposition_conflicted(&self, prop: &PropTerm) -> bool {
        self.holds(prop) && self.denied(prop)
    }

    pub(crate) fn assumed_value(&self, prop: &PropTerm) -> Option<bool> {
        let grounded = GroundProp::from_prop(prop);
        match (
            self.assumed.contains(&grounded),
            self.assumed_denied.contains(&grounded),
        ) {
            (true, false) => Some(true),
            (false, true) => Some(false),
            _ => None,
        }
    }

    fn seed(
        module: &CoreModule,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
    ) -> Self {
        let mut world = Self::default();
        let propositions = declared_propositions(module);
        let view = FrozenCaseView::from_context(case, ctx);
        for item in &case.evidence {
            if !FrozenCaseView::evidence_is_known(item.observed_at, Some(ctx.record_time)) {
                record_step(
                    &mut world.trace,
                    "temporal_rejection",
                    format!(
                        "evidence observed at {} after known-at {}: {item:?}",
                        item.observed_at, ctx.record_time
                    ),
                    Vec::new(),
                );
            }
        }
        for determination in &case.determinations {
            if !FrozenCaseView::determination_is_known(
                determination.recorded_at,
                Some(ctx.record_time),
            ) {
                record_step(
                    &mut world.trace,
                    "temporal_rejection",
                    format!("future determination: {determination:?}"),
                    Vec::new(),
                );
            }
        }
        seed_facts(&mut world, &case.facts, &propositions);
        seed_facts(&mut world, args, &propositions);
        seed_determinations(&mut world, &view);
        seed_evidence(&mut world, &view);
        seed_events(&mut world, &view, &propositions);
        seed_core_facts(&mut world, module);
        world
    }

    fn insert_held(&mut self, prop: PropTerm) {
        let ground = GroundProp::from_prop(&prop);
        self.held.insert(ground.clone());
        self.base.insert(ground.clone());
        self.withdrawn.remove(&ground);
        self.support
            .entry(ground.clone())
            .or_default()
            .insert(SupportSet::new());
        let id = record_step(
            &mut self.trace,
            "foundation",
            format!("admitted {ground:?}"),
            Vec::new(),
        );
        self.provenance.entry(ground).or_default().insert(id);
    }

    fn insert_derived(&mut self, prop: PropTerm, justifications: &Justifications) {
        let ground = GroundProp::from_prop(&prop);
        self.held.insert(ground.clone());
        self.withdrawn.remove(&ground);
        let entry = self.support.entry(ground).or_default();
        if justifications.is_empty() {
            entry.insert(SupportSet::new());
        } else {
            entry.extend(justifications.iter().cloned());
        }
    }

    fn insert_denied(&mut self, prop: PropTerm) {
        self.denied.insert(GroundProp::from_prop(&prop));
    }

    fn withdraw(&mut self, ground: GroundProp) {
        self.held.remove(&ground);
        self.support.remove(&ground);
        self.withdrawn.insert(ground);
    }

    fn guard_holds(
        &self,
        guard: &Guard,
        subst: &BTreeMap<String, Term>,
        case: &CaseRecord,
        ctx: &RunContext,
    ) -> Hold {
        match guard {
            Guard::Satisfied => Hold::Yes,
            Guard::Operative(prop, _) => {
                let grounded = subst_prop(prop, subst);
                if self.held.iter().any(|held| held.satisfies(&grounded, true)) {
                    Hold::Yes
                } else if self.denied(&grounded) {
                    Hold::No
                } else {
                    Hold::Unknown
                }
            }
            Guard::Derived(prop) => {
                let grounded = subst_prop(prop, subst);
                if self.proposition_conflicted(&grounded) {
                    Hold::Unknown
                } else if self.holds(&grounded) {
                    Hold::Yes
                } else if self.denied(&grounded) {
                    Hold::No
                } else {
                    Hold::Unknown
                }
            }
            Guard::Assumed(prop) => match self.assumed_value(&subst_prop(prop, subst)) {
                Some(value) => Hold::from_bool(value),
                None => Hold::Unknown,
            },
            Guard::Determined(prop) => {
                let grounded = subst_prop(prop, subst);
                match self.modal_determined(&grounded) {
                    Some(true) => Hold::Yes,
                    Some(false) => Hold::No,
                    None => Hold::Unknown,
                }
            }
            Guard::Observed { schema, .. } => {
                if self.observed(schema)
                    || FrozenCaseView::from_context(case, ctx)
                        .evidence()
                        .any(|e| names_eq(&e.schema, schema))
                {
                    Hold::Yes
                } else {
                    Hold::Unknown
                }
            }
            Guard::And(parts) => parts.iter().fold(Hold::Yes, |acc, part| {
                acc.and(self.guard_holds(part, subst, case, ctx))
            }),
            Guard::Or(parts) => parts.iter().fold(Hold::No, |acc, part| {
                acc.or(self.guard_holds(part, subst, case, ctx))
            }),
            Guard::Not(inner) => self.guard_holds(inner, subst, case, ctx).not(),
            Guard::Compare { op, left, right } => {
                let left = subst_term(left, subst);
                let right = subst_term(right, subst);
                match (
                    term_value(&left, case, subst),
                    term_value(&right, case, subst),
                ) {
                    (Some(a), Some(b)) => Hold::from_bool(compare_values(*op, &a, &b)),
                    _ => Hold::Unknown,
                }
            }
            Guard::EffectiveActTerm(term) => {
                let term = subst_term(term, subst);
                match &term {
                    Term::Apply { ctor, args } | Term::Call { callee: ctor, args }
                        if ctor == "AcceptOffice" && args.len() == 2 =>
                    {
                        let person = crate::judgment::term_label(&args[0]);
                        let office = crate::judgment::term_label(&args[1]);
                        if crate::is_accepted(case, &person, &office, ctx) {
                            Hold::Yes
                        } else {
                            Hold::Unknown
                        }
                    }
                    _ => {
                        let key = format!("Effective({})", crate::judgment::term_label(&term));
                        match case.facts.get(&key) {
                            Some(Value::Bool(value)) => Hold::from_bool(*value),
                            _ => Hold::Unknown,
                        }
                    }
                }
            }
            Guard::CompletedAct(name) | Guard::EffectiveAct(name) => {
                if case.facts.get(name) == Some(&Value::Bool(true))
                    || FrozenCaseView::from_context(case, ctx)
                        .determinations()
                        .any(|d| {
                            names_eq(&d.issue, name)
                                && d.established
                                && !d.protocol.is_empty()
                                && !d.decider.is_empty()
                        })
                {
                    Hold::Yes
                } else if case.facts.get(name) == Some(&Value::Bool(false)) {
                    Hold::No
                } else {
                    Hold::Unknown
                }
            }
            Guard::Request(_) => Hold::Unknown,
        }
    }

    fn guard_justifications(
        &self,
        guard: &Guard,
        subst: &BTreeMap<String, Term>,
        case: &CaseRecord,
        ctx: &RunContext,
        work_units: &mut u64,
    ) -> Result<Justifications, EngineError> {
        spend_work(work_units, 1, self.operation_limit)?;
        Ok(match guard {
            Guard::Satisfied => axiomatic_justification(),
            Guard::Operative(prop, _)
            | Guard::Derived(prop)
            | Guard::Determined(prop)
            | Guard::Assumed(prop) => {
                let grounded = subst_prop(prop, subst);
                let premise = self
                    .matching_held(&grounded)
                    .unwrap_or_else(|| GroundProp::from_prop(&grounded));
                Justifications::from([SupportSet::from([premise])])
            }
            Guard::And(parts) => {
                let mut out = axiomatic_justification();
                for part in parts {
                    out = cartesian_support(
                        &out,
                        &self.guard_justifications(part, subst, case, ctx, work_units)?,
                        work_units,
                        self.operation_limit,
                    )?;
                }
                out
            }
            Guard::Or(parts) => {
                let mut out = Justifications::new();
                for part in parts {
                    if self.guard_holds(part, subst, case, ctx) == Hold::Yes {
                        out.extend(self.guard_justifications(part, subst, case, ctx, work_units)?);
                    }
                }
                out
            }
            Guard::Not(_)
            | Guard::Compare { .. }
            | Guard::Observed { .. }
            | Guard::CompletedAct(_)
            | Guard::EffectiveAct(_)
            | Guard::EffectiveActTerm(_)
            | Guard::Request(_) => axiomatic_justification(),
        })
    }

    fn matching_held(&self, want: &PropTerm) -> Option<GroundProp> {
        self.held
            .iter()
            .find(|held| held.satisfies(want, true))
            .cloned()
    }

    fn apply_effects(
        &mut self,
        effects: &[CoreEffect],
        subst: &BTreeMap<String, Term>,
        justifications: &Justifications,
    ) {
        let cut: BTreeSet<GroundProp> = effects
            .iter()
            .filter_map(|effect| match &effect.consequence {
                Consequence::Terminate(prop) | Consequence::Suspend(prop) => {
                    Some(GroundProp::from_prop(&subst_prop(prop, subst)))
                }
                _ => None,
            })
            .collect();
        let justifications = strip_cut(justifications, &cut);
        for effect in effects {
            let parents: BTreeSet<_> = justifications
                .iter()
                .flatten()
                .flat_map(|premise| self.provenance.get(premise).into_iter().flatten().cloned())
                .collect();
            let id = record_step(
                &mut self.trace,
                "rule_application",
                format!(
                    "effect={}; consequence={:?}; substitution={subst:?}",
                    effect.id, effect.consequence
                ),
                parents,
            );
            match &effect.consequence {
                Consequence::Derive(prop) | Consequence::Establish(prop) => {
                    let prop = subst_prop(prop, subst);
                    self.provenance
                        .entry(GroundProp::from_prop(&prop))
                        .or_default()
                        .insert(id.clone());
                    self.insert_derived(prop, &justifications);
                }
                Consequence::Terminate(prop) | Consequence::Suspend(prop) => {
                    self.withdraw(GroundProp::from_prop(&subst_prop(prop, subst)));
                }
                _ => {}
            }
        }
    }

    fn retract_unsupported(&mut self) {
        // Compute the least set with a live foundation. A mutually supporting
        // component without a surviving premise is not its own foundation.
        let mut grounded: BTreeSet<_> = self.base.difference(&self.withdrawn).cloned().collect();
        loop {
            let before = grounded.len();
            for (prop, justifications) in &self.support {
                if self.withdrawn.contains(prop) || !self.held.contains(prop) {
                    continue;
                }
                if justifications
                    .iter()
                    .any(|support| support.iter().all(|premise| grounded.contains(premise)))
                {
                    grounded.insert(prop.clone());
                }
            }
            if grounded.len() == before {
                break;
            }
        }
        self.held.retain(|prop| grounded.contains(prop));
        self.support.retain(|prop, _| grounded.contains(prop));
    }

    fn reseed_base(&mut self) {
        let base: Vec<GroundProp> = self.base.iter().cloned().collect();
        for ground in base {
            if self.withdrawn.contains(&ground) {
                continue;
            }
            self.held.insert(ground.clone());
            self.support
                .entry(ground)
                .or_default()
                .insert(SupportSet::new());
        }
    }
}

fn axiomatic_justification() -> Justifications {
    let mut justifications = Justifications::new();
    justifications.insert(SupportSet::new());
    justifications
}

fn cartesian_support(
    left: &Justifications,
    right: &Justifications,
    work_units: &mut u64,
    limit: u64,
) -> Result<Justifications, EngineError> {
    let mut out = Justifications::new();
    for a in left {
        for b in right {
            spend_work(work_units, (a.len() + b.len() + 1) as u64, limit)?;
            let mut united = a.clone();
            united.extend(b.iter().cloned());
            out.insert(united);
        }
    }
    Ok(out)
}

fn input_cost(
    module: &CoreModule,
    case: &CaseRecord,
    args: &BTreeMap<String, Value>,
    limit: u64,
) -> Result<u64, EngineError> {
    fn value(value: &Value, units: &mut u64, limit: u64, depth: usize) -> Result<(), EngineError> {
        if depth >= 128 {
            return Err(EngineError::FuelExhausted { remaining: 0 });
        }
        spend_work(units, 1, limit)?;
        match value {
            Value::String(text) | Value::Entity(text) => {
                spend_work(units, (text.len() / 128) as u64, limit)?
            }
            Value::Map(fields) | Value::Ctor { fields, .. } => {
                for child in fields.values() {
                    value_cost(child, units, limit, depth + 1)?;
                }
            }
            Value::Set(values)
            | Value::ClauseRef {
                arguments: values, ..
            } => {
                for child in values {
                    value_cost(child, units, limit, depth + 1)?;
                }
            }
            Value::Option(Some(child)) => value_cost(child, units, limit, depth + 1)?,
            _ => {}
        }
        Ok(())
    }
    fn value_cost(
        item: &Value,
        units: &mut u64,
        limit: u64,
        depth: usize,
    ) -> Result<(), EngineError> {
        value(item, units, limit, depth)
    }
    let mut units = 0;
    spend_work(
        &mut units,
        module
            .declarations
            .len()
            .saturating_add(module.nominations.len())
            .saturating_add(case.facts.len())
            .saturating_add(case.evidence.len())
            .saturating_add(case.events.len())
            .saturating_add(case.determinations.len())
            .saturating_add(case.assumptions.len()) as u64,
        limit,
    )?;
    for domain in case
        .admissible_completions
        .interpretations
        .values()
        .chain(case.admissible_completions.choices.values())
        .chain(
            case.admissible_completions
                .evidence
                .values()
                .map(|domain| &domain.responses),
        )
    {
        spend_work(&mut units, 1, limit)?;
        for value in domain {
            spend_work(&mut units, 1 + (value.len() / 128) as u64, limit)?;
        }
    }
    for item in case
        .facts
        .values()
        .chain(args.values())
        .chain(case.evidence.iter().map(|record| &record.value))
        .chain(case.events.iter().map(|event| &event.payload))
        .chain(
            case.assumptions
                .iter()
                .map(|assumption| &assumption.payload),
        )
    {
        value_cost(item, &mut units, limit, 0)?;
    }
    Ok(units)
}

fn spend_work(consumed: &mut u64, amount: u64, limit: u64) -> Result<(), EngineError> {
    *consumed = consumed.saturating_add(amount);
    if *consumed >= limit {
        Err(EngineError::FuelExhausted { remaining: 0 })
    } else {
        Ok(())
    }
}

fn guard_cost(guard: &Guard) -> u64 {
    let mut pending = vec![guard];
    let mut count = 0_u64;
    while let Some(guard) = pending.pop() {
        count += 1;
        if count >= 100_000 {
            return count;
        }
        match guard {
            Guard::And(parts) | Guard::Or(parts) => pending.extend(parts),
            Guard::Not(inner) => pending.push(inner),
            _ => {}
        }
    }
    count
}

fn strip_cut(justifications: &Justifications, cut: &BTreeSet<GroundProp>) -> Justifications {
    justifications
        .iter()
        .map(|justification| justification.difference(cut).cloned().collect())
        .collect()
}

impl SeededDetermination {
    fn applies_to(&self, prop: &PropTerm) -> bool {
        !self.protocol.is_empty() && !self.decider.is_empty() && self.prop.satisfies(prop, false)
    }
}

impl GroundProp {
    fn from_prop(prop: &PropTerm) -> Self {
        Self {
            predicate: prop.predicate.clone(),
            arguments: prop.arguments.iter().map(canonical_ground_term).collect(),
        }
    }

    fn satisfies(&self, want: &PropTerm, allow_zero_ary: bool) -> bool {
        if !names_eq(&self.predicate, &want.predicate) {
            return false;
        }
        let want_args: Vec<Term> = want.arguments.iter().map(canonical_ground_term).collect();
        if self.arguments == want_args {
            return true;
        }
        let _ = allow_zero_ary;
        false
    }
}

impl Hold {
    fn from_bool(value: bool) -> Self {
        if value { Self::Yes } else { Self::No }
    }

    fn and(self, other: Self) -> Self {
        match (self, other) {
            (Self::No, _) | (_, Self::No) => Self::No,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Yes, Self::Yes) => Self::Yes,
        }
    }

    fn or(self, other: Self) -> Self {
        match (self, other) {
            (Self::Yes, _) | (_, Self::Yes) => Self::Yes,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::No, Self::No) => Self::No,
        }
    }

    fn not(self) -> Self {
        match self {
            Self::Yes => Self::No,
            Self::No => Self::Yes,
            Self::Unknown => Self::Unknown,
        }
    }
}

fn seed_facts(
    world: &mut DerivedWorld,
    facts: &BTreeMap<String, Value>,
    propositions: &BTreeSet<String>,
) {
    for (name, value) in facts {
        if value != &Value::Bool(true) {
            continue;
        }
        world.insert_held(parse_prop_issue(name));
        for prop in propositions {
            if fact_names_proposition(name, prop) {
                world.insert_held(PropTerm::new(prop.clone(), Vec::new()));
            }
        }
    }
}

fn seed_determinations(world: &mut DerivedWorld, view: &FrozenCaseView<'_>) {
    for det in view.determinations() {
        if det.protocol.trim().is_empty() || det.decider.trim().is_empty() {
            record_step(
                &mut world.trace,
                "admission_rejection",
                format!("determination lacks protocol or decider: {det:?}"),
                Vec::new(),
            );
            continue;
        }
        let prop = parse_prop_issue(&det.issue);
        let id = record_step(
            &mut world.trace,
            "determination",
            format!("{det:?}"),
            Vec::new(),
        );
        world
            .provenance
            .entry(GroundProp::from_prop(&prop))
            .or_default()
            .insert(id);
        world.seeded_determinations.push(SeededDetermination {
            prop: GroundProp::from_prop(&prop),
            protocol: det.protocol.clone(),
            decider: det.decider.clone(),
            established: det.established,
        });
        if det.established {
            world.insert_held(prop);
        } else {
            world.insert_denied(prop);
        }
    }
}

fn seed_evidence(world: &mut DerivedWorld, view: &FrozenCaseView<'_>) {
    for item in view.evidence() {
        record_step(
            &mut world.trace,
            "evidence",
            format!("{item:?}"),
            Vec::new(),
        );
        world.observed.insert(item.schema.clone());
    }
}

fn seed_events(
    world: &mut DerivedWorld,
    view: &FrozenCaseView<'_>,
    propositions: &BTreeSet<String>,
) {
    let case = view.case();
    let known_at = view.known_at();
    for event in view.events() {
        if !crate::duty::event_is_admitted(case, event, known_at) {
            continue;
        }
        match &event.payload {
            Value::Map(fields) => seed_facts(world, fields, propositions),
            Value::Ctor { name, fields } => {
                if !fields.is_empty() {
                    seed_facts(world, fields, propositions);
                }
                if name.eq_ignore_ascii_case("performed")
                    && crate::duty::payload_applies_to_default_instance(&event.payload)
                {
                    world.insert_held(PropTerm::new("performed", Vec::new()));
                }
            }
            Value::String(name) | Value::Entity(name) if name.eq_ignore_ascii_case("performed") => {
                world.insert_held(PropTerm::new("performed", Vec::new()));
            }
            Value::Bool(true) => {
                world.insert_held(PropTerm::new(event.kind.clone(), Vec::new()));
            }
            _ => {}
        }
    }
}

fn seed_core_facts(world: &mut DerivedWorld, module: &CoreModule) {
    for decl in &module.declarations {
        if let CoreDecl::Fact(fact) = decl {
            world.insert_held(PropTerm {
                predicate: fact.relation.clone(),
                arguments: fact.arguments.clone(),
            });
        }
    }
}

fn declared_propositions(module: &CoreModule) -> BTreeSet<String> {
    module
        .declarations
        .iter()
        .filter_map(|decl| match decl {
            CoreDecl::Proposition(p) => Some(p.name.clone()),
            _ => None,
        })
        .collect()
}

fn fact_names_proposition(fact: &str, prop: &str) -> bool {
    fact == prop
}

fn names_eq(a: &str, b: &str) -> bool {
    a == b
}

fn rule_in_force(rule: &CoreRule, ctx: &RunContext) -> bool {
    rule.meta.valid_time.contains(ctx.valid_time) && rule.meta.record_time.contains(ctx.record_time)
}

fn rule_binders(rule: &CoreRule, module: &CoreModule) -> Vec<String> {
    if !rule.binders.is_empty() {
        return rule.binders.clone();
    }
    let mut names = BTreeSet::new();
    collect_guard_idents(&rule.guard, &mut names);
    collect_effect_idents(&rule.consequences, &mut names);
    if let Some(fallback) = &rule.fallback {
        collect_effect_idents(fallback, &mut names);
    }
    names
        .into_iter()
        .filter(|name| is_binder_name(name, module))
        .collect()
}

fn is_binder_name(name: &str, module: &CoreModule) -> bool {
    if name.is_empty() || name == "true" || name == "false" {
        return false;
    }
    if module.declarations.iter().any(|decl| match decl {
        CoreDecl::Entity(e) => names_eq(&e.name, name),
        CoreDecl::Proposition(p) => names_eq(&p.name, name),
        _ => false,
    }) {
        return false;
    }
    name.starts_with(|c: char| c.is_lowercase() || c == '_')
}

fn collect_effect_idents(effects: &[CoreEffect], out: &mut BTreeSet<String>) {
    for effect in effects {
        match &effect.consequence {
            Consequence::Derive(p)
            | Consequence::Establish(p)
            | Consequence::Terminate(p)
            | Consequence::Suspend(p) => collect_prop_idents(p, out),
            _ => {}
        }
    }
}

fn collect_guard_idents(guard: &Guard, out: &mut BTreeSet<String>) {
    match guard {
        Guard::Operative(prop, _)
        | Guard::Derived(prop)
        | Guard::Determined(prop)
        | Guard::Assumed(prop) => collect_prop_idents(prop, out),
        Guard::And(parts) | Guard::Or(parts) => {
            for part in parts {
                collect_guard_idents(part, out);
            }
        }
        Guard::Not(inner) => collect_guard_idents(inner, out),
        Guard::Compare { left, right, .. } => {
            collect_term_idents(left, out);
            collect_term_idents(right, out);
        }
        _ => {}
    }
}

fn collect_prop_idents(prop: &PropTerm, out: &mut BTreeSet<String>) {
    for arg in &prop.arguments {
        collect_term_idents(arg, out);
    }
}

fn collect_term_idents(term: &Term, out: &mut BTreeSet<String>) {
    match term {
        Term::Ident(name) | Term::Binder(name) => {
            out.insert(name.clone());
        }
        Term::Apply { args, .. } | Term::Call { args, .. } | Term::Set(args) => {
            for arg in args {
                collect_term_idents(arg, out);
            }
        }
        Term::Binary { left, right, .. } => {
            collect_term_idents(left, out);
            collect_term_idents(right, out);
        }
        Term::If { cond, then, else_ } => {
            collect_term_idents(cond, out);
            collect_term_idents(then, out);
            collect_term_idents(else_, out);
        }
        Term::Field { base, .. } => collect_term_idents(base, out),
        Term::Record(fields) => {
            for value in fields.values() {
                collect_term_idents(value, out);
            }
        }
        _ => {}
    }
}

fn ground_candidates(
    module: &CoreModule,
    case: &CaseRecord,
    args: &BTreeMap<String, Value>,
) -> Vec<Term> {
    let mut terms = BTreeSet::new();
    for decl in &module.declarations {
        if let CoreDecl::Entity(entity) = decl {
            terms.insert(Term::Ident(entity.name.clone()));
        }
    }
    for values in [&case.facts, args] {
        for value in values.values() {
            if let Some(term) = value_as_term(value) {
                terms.insert(term);
            }
        }
    }
    terms.into_iter().collect()
}

fn substitutions(
    binders: &[String],
    candidates: &[Term],
) -> Result<Vec<BTreeMap<String, Term>>, EngineError> {
    if binders.is_empty() {
        return Ok(vec![BTreeMap::new()]);
    }
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let mut acc = vec![BTreeMap::new()];
    for binder in binders {
        let mut next = Vec::new();
        let mut truncated = false;
        'expand: for env in &acc {
            for candidate in candidates {
                if next.len() >= MAX_SUBSTITUTIONS {
                    truncated = true;
                    break 'expand;
                }
                let mut env = env.clone();
                env.insert(binder.clone(), candidate.clone());
                next.push(env);
            }
        }
        if truncated {
            return Err(EngineError::Unsupported(format!(
                "substitution cap {MAX_SUBSTITUTIONS} reached before complete enumeration"
            )));
        }
        if next.is_empty() {
            return Ok(Vec::new());
        }
        acc = next;
    }
    let total = binders.len();
    acc.retain(|subst| subst.len() == total && binders.iter().all(|b| subst.contains_key(b)));
    if acc.is_empty() && !binders.is_empty() {
        return Err(EngineError::Unsupported(
            "substitution cap dropped binders before they were bound".into(),
        ));
    }
    Ok(acc)
}

fn subst_prop(prop: &PropTerm, subst: &BTreeMap<String, Term>) -> PropTerm {
    PropTerm {
        predicate: prop.predicate.clone(),
        arguments: prop
            .arguments
            .iter()
            .map(|t| subst_term(t, subst))
            .collect(),
    }
}

fn subst_term(term: &Term, subst: &BTreeMap<String, Term>) -> Term {
    match term {
        Term::Ident(name) | Term::Binder(name) => {
            subst.get(name).cloned().unwrap_or_else(|| term.clone())
        }
        Term::Apply { ctor, args } => Term::Apply {
            ctor: ctor.clone(),
            args: args.iter().map(|t| subst_term(t, subst)).collect(),
        },
        Term::Call { callee, args } => Term::Call {
            callee: callee.clone(),
            args: args.iter().map(|t| subst_term(t, subst)).collect(),
        },
        Term::Set(xs) => Term::Set(xs.iter().map(|t| subst_term(t, subst)).collect()),
        Term::Binary { op, left, right } => Term::Binary {
            op: *op,
            left: Box::new(subst_term(left, subst)),
            right: Box::new(subst_term(right, subst)),
        },
        Term::If { cond, then, else_ } => Term::If {
            cond: Box::new(subst_term(cond, subst)),
            then: Box::new(subst_term(then, subst)),
            else_: Box::new(subst_term(else_, subst)),
        },
        Term::Field { base, name } => Term::Field {
            base: Box::new(subst_term(base, subst)),
            name: name.clone(),
        },
        Term::Record(fields) => Term::Record(
            fields
                .iter()
                .map(|(k, v)| (k.clone(), subst_term(v, subst)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn term_value(term: &Term, case: &CaseRecord, subst: &BTreeMap<String, Term>) -> Option<Value> {
    let term = subst_term(term, subst);
    match &term {
        Term::Bool(b) => Some(Value::Bool(*b)),
        Term::Int(i) => Some(Value::Int(*i)),
        Term::Decimal(d) => Some(Value::Decimal(*d)),
        Term::String(s) => Some(Value::String(s.clone())),
        Term::Ident(name) | Term::Binder(name) => case
            .facts
            .get(name)
            .cloned()
            .or_else(|| subst.get(name).and_then(|t| term_value(t, case, subst)))
            .or_else(|| Some(Value::Entity(name.clone()))),
        Term::Apply { ctor, args } if args.is_empty() => Some(Value::Ctor {
            name: ctor.clone(),
            fields: BTreeMap::new(),
        }),
        _ => None,
    }
}

fn compare_values(op: CompareOp, left: &Value, right: &Value) -> bool {
    match op {
        CompareOp::Eq => values_eq(left, right),
        CompareOp::Ne => !values_eq(left, right),
        CompareOp::Lt => cmp_ord(left, right).is_some_and(std::cmp::Ordering::is_lt),
        CompareOp::Le => cmp_ord(left, right).is_some_and(std::cmp::Ordering::is_le),
        CompareOp::Gt => cmp_ord(left, right).is_some_and(std::cmp::Ordering::is_gt),
        CompareOp::Ge => cmp_ord(left, right).is_some_and(std::cmp::Ordering::is_ge),
    }
}

fn values_eq(left: &Value, right: &Value) -> bool {
    left == right
        || matches!((left, right), (Value::Int(a), Value::Decimal(b)) if rust_decimal::Decimal::from(*a) == *b)
        || matches!((left, right), (Value::Decimal(a), Value::Int(b)) if *a == rust_decimal::Decimal::from(*b))
}

fn cmp_ord(left: &Value, right: &Value) -> Option<std::cmp::Ordering> {
    match (left, right) {
        (Value::Int(a), Value::Int(b)) => Some(a.cmp(b)),
        (Value::Decimal(a), Value::Decimal(b)) => Some(a.cmp(b)),
        (Value::Int(a), Value::Decimal(b)) => Some(rust_decimal::Decimal::from(*a).cmp(b)),
        (Value::Decimal(a), Value::Int(b)) => Some(a.cmp(&rust_decimal::Decimal::from(*b))),
        _ => None,
    }
}

fn value_as_term(value: &Value) -> Option<Term> {
    match value {
        Value::Entity(s) | Value::String(s) => Some(Term::Ident(s.clone())),
        Value::Int(i) => Some(Term::Int(*i)),
        Value::Decimal(d) => Some(Term::Decimal(*d)),
        Value::Bool(b) => Some(Term::Bool(*b)),
        _ => None,
    }
}

pub(crate) fn canonical_ground_term(term: &Term) -> Term {
    match term {
        Term::Ident(name) | Term::Binder(name) if name.contains('(') => {
            parse_arg_term_bounded(name, 0)
        }
        Term::Binder(name) => Term::Ident(name.clone()),
        Term::Apply { ctor, args } | Term::Call { callee: ctor, args } => Term::Apply {
            ctor: ctor.clone(),
            args: args.iter().map(canonical_ground_term).collect(),
        },
        Term::Set(items) => Term::Set(items.iter().map(canonical_ground_term).collect()),
        Term::Record(fields) => Term::Record(
            fields
                .iter()
                .map(|(name, term)| (name.clone(), canonical_ground_term(term)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Parse a determination issue (`P`, `P()`, `ExemptFromBOI(AcmeLLC)`).
pub fn parse_prop_issue(issue: &str) -> PropTerm {
    let s = issue.trim();
    if s.is_empty() {
        return PropTerm::new(String::new(), Vec::new());
    }
    let Some(open) = s.find('(') else {
        return PropTerm::new(s, Vec::new());
    };
    if !s.ends_with(')') {
        return PropTerm::new(s, Vec::new());
    }
    let pred = s[..open].trim();
    if pred.is_empty() {
        return PropTerm::new(s, Vec::new());
    }
    let inner = s[open + 1..s.len() - 1].trim();
    if inner.is_empty() {
        return PropTerm::new(pred, Vec::new());
    }
    let args = split_commas(inner)
        .into_iter()
        .map(|part| parse_arg_term(part.trim()))
        .collect();
    PropTerm::new(pred, args)
}

fn parse_arg_term(s: &str) -> Term {
    parse_arg_term_bounded(s, 0)
}

fn parse_arg_term_bounded(s: &str, depth: usize) -> Term {
    let s = s.trim();
    if depth >= 64 || s.len() > 65_536 {
        return Term::Apply {
            ctor: "$invalid_issue".into(),
            args: vec![Term::String(s.to_owned())],
        };
    }
    if s == "true" {
        return Term::Bool(true);
    }
    if s == "false" {
        return Term::Bool(false);
    }
    if let Ok(i) = s.parse::<i64>() {
        return Term::Int(i);
    }
    if let Ok(value) = s.parse::<rust_decimal::Decimal>() {
        return Term::Decimal(value);
    }
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        return Term::String(s[1..s.len() - 1].to_owned());
    }
    if let Some(open) = s.find('(') {
        let head = s[..open].trim();
        if !head.is_empty()
            && head
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
            && s.ends_with(')')
        {
            let inside = &s[open + 1..s.len() - 1];
            let args = if inside.trim().is_empty() {
                Vec::new()
            } else {
                split_commas(inside)
                    .into_iter()
                    .map(|part| parse_arg_term_bounded(part, depth + 1))
                    .collect()
            };
            return Term::Apply {
                ctor: head.into(),
                args,
            };
        }
    }
    Term::Ident(s.to_owned())
}

fn split_commas(src: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0_i32;
    let mut quoted = None;
    let mut escaped = false;
    for (i, c) in src.char_indices() {
        if let Some(quote) = quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == quote {
                quoted = None;
            }
            continue;
        }
        match c {
            '"' | '\'' => quoted = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(&src[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&src[start..]);
    parts
}

/// Proposition constructor/call/ident, excluding evaluator keywords.
pub fn term_as_proposition(term: &Term) -> Option<PropTerm> {
    match term {
        Term::Ident(name) | Term::Binder(name) if !is_eval_keyword(name) => {
            Some(PropTerm::new(name.clone(), Vec::new()))
        }
        Term::Apply { ctor, args } if !is_eval_keyword(ctor) => Some(PropTerm {
            predicate: ctor.clone(),
            arguments: args.clone(),
        }),
        Term::Call { callee, args } if !is_eval_keyword(callee) => Some(PropTerm {
            predicate: callee.clone(),
            arguments: args.clone(),
        }),
        _ => None,
    }
}

pub fn is_eval_keyword(name: &str) -> bool {
    matches!(
        name,
        "operative"
            | "determined"
            | "assumed"
            | "observed"
            | "necessarily"
            | "for_all"
            | "forall"
            | "exists"
            | "not"
            | "if"
            | "If"
            | "field"
            | "Field"
            | "and"
            | "or"
            | "call"
            | "seq"
            | "transaction"
            | "require"
            | "duty_step"
            | "duty_status"
            | "require_authority"
    ) || name.eq_ignore_ascii_case("for_all")
        || name.eq_ignore_ascii_case("exists")
}

pub fn binder_name(term: &Term) -> Option<String> {
    match term {
        Term::Ident(s) | Term::String(s) | Term::Binder(s) => Some(s.clone()),
        _ => None,
    }
}

pub fn domain_name(term: &Term) -> Option<String> {
    binder_name(term)
}

#[cfg(test)]
#[path = "../unit_tests/worklist.rs"]
mod unit_tests;
