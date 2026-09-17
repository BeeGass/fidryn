//! Bounded constitutive/derive worklist. The case record is never mutated.

use fidryn_core::ir::{CompareOp, Consequence, CoreDecl, CoreModule, CoreRule, Guard};
use fidryn_core::value::{PropTerm, Term, Value};
use fidryn_core::{CaseRecord, EngineError, FrozenCaseView, RunContext};
use std::collections::{BTreeMap, BTreeSet};

const WORKLIST_FUEL: u32 = 64;
const MAX_SUBSTITUTIONS: usize = 256;

/// Staged derived propositions, denials, and observed schemas.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DerivedWorld {
    held: BTreeSet<GroundProp>,
    denied: BTreeSet<GroundProp>,
    observed: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct GroundProp {
    predicate: String,
    arguments: Vec<String>,
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
        let mut world = Self::seed(module, case, ctx, args);
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
            let before_held = world.held.clone();
            let before_denied = world.denied.clone();
            for rule in &rules {
                if !rule_in_force(rule, ctx) {
                    continue;
                }
                let binders = rule_binders(rule, module);
                for subst in substitutions(&binders, &candidates)? {
                    if world.guard_holds(&rule.guard, &subst, case, ctx) != Hold::Yes {
                        continue;
                    }
                    world.apply_consequences(rule, &subst);
                }
            }
            if world.held == before_held && world.denied == before_denied {
                return Ok(world);
            }
        }
        Err(EngineError::FuelExhausted { remaining: 0 })
    }

    pub fn holds(&self, prop: &PropTerm) -> bool {
        self.held.iter().any(|held| held.satisfies(prop, false))
    }

    pub fn denied(&self, prop: &PropTerm) -> bool {
        let want = GroundProp::from_prop(prop);
        self.denied.iter().any(|denied| {
            names_eq(&denied.predicate, &want.predicate)
                && (denied.arguments.is_empty() || denied.arguments == want.arguments)
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

    fn seed(
        module: &CoreModule,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
    ) -> Self {
        let mut world = Self::default();
        let propositions = declared_propositions(module);
        let view = FrozenCaseView::from_context(case, ctx);
        seed_facts(&mut world, &case.facts, &propositions);
        seed_facts(&mut world, args, &propositions);
        seed_determinations(&mut world, &view);
        seed_evidence(&mut world, &view);
        seed_events(&mut world, &view, &propositions);
        seed_core_facts(&mut world, module);
        seed_observations(&mut world, module, &view);
        world
    }

    fn insert_held(&mut self, prop: PropTerm) {
        self.held.insert(GroundProp::from_prop(&prop));
    }

    fn insert_denied(&mut self, prop: PropTerm) {
        self.denied.insert(GroundProp::from_prop(&prop));
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
            Guard::Operative(prop, _) | Guard::Derived(prop) => {
                let grounded = subst_prop(prop, subst);
                if self.held.iter().any(|held| held.satisfies(&grounded, true)) {
                    Hold::Yes
                } else if self.denied(&grounded) {
                    Hold::No
                } else {
                    Hold::Unknown
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
            Guard::CompletedAct(name) | Guard::EffectiveAct(name) => {
                if case.facts.contains_key(name)
                    || FrozenCaseView::from_context(case, ctx)
                        .determinations()
                        .any(|d| names_eq(&d.issue, name) && d.established)
                {
                    Hold::Yes
                } else {
                    Hold::Unknown
                }
            }
            Guard::Request(_) => Hold::Unknown,
        }
    }

    fn apply_consequences(&mut self, rule: &CoreRule, subst: &BTreeMap<String, Term>) {
        for effect in &rule.consequences {
            match &effect.consequence {
                Consequence::Derive(prop) | Consequence::Establish(prop) => {
                    self.insert_held(subst_prop(prop, subst));
                }
                Consequence::Terminate(prop) | Consequence::Suspend(prop) => {
                    let grounded = GroundProp::from_prop(&subst_prop(prop, subst));
                    self.held.retain(|held| held != &grounded);
                }
                _ => {}
            }
        }
    }
}

impl GroundProp {
    fn from_prop(prop: &PropTerm) -> Self {
        Self {
            predicate: prop.predicate.clone(),
            arguments: prop.arguments.iter().map(arg_label).collect(),
        }
    }

    fn satisfies(&self, want: &PropTerm, allow_zero_ary: bool) -> bool {
        if !names_eq(&self.predicate, &want.predicate) {
            return false;
        }
        let want_args: Vec<String> = want.arguments.iter().map(arg_label).collect();
        if self.arguments == want_args {
            return true;
        }
        if want_args.is_empty() {
            return true;
        }
        allow_zero_ary && self.arguments.is_empty()
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
        world.insert_held(PropTerm::new(name.clone(), Vec::new()));
        for prop in propositions {
            if fact_names_proposition(name, prop) {
                world.insert_held(PropTerm::new(prop.clone(), Vec::new()));
            }
        }
    }
}

fn seed_determinations(world: &mut DerivedWorld, view: &FrozenCaseView<'_>) {
    for det in view.determinations() {
        let prop = parse_prop_issue(&det.issue);
        if det.established {
            world.insert_held(prop);
        } else {
            world.insert_denied(prop);
        }
    }
}

fn seed_evidence(world: &mut DerivedWorld, view: &FrozenCaseView<'_>) {
    for item in view.evidence() {
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

fn seed_observations(world: &mut DerivedWorld, module: &CoreModule, view: &FrozenCaseView<'_>) {
    for decl in &module.declarations {
        let CoreDecl::Observation(obs) = decl else {
            continue;
        };
        let seen =
            world.observed(&obs.name) || view.evidence().any(|e| names_eq(&e.schema, &obs.name));
        if !seen {
            continue;
        }
        if obs.establishes.arguments.iter().all(term_is_ground) {
            world.insert_held(obs.establishes.clone());
        } else {
            world.insert_held(PropTerm::new(obs.establishes.predicate.clone(), Vec::new()));
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
    names_eq(fact, prop) || names_eq(fact, camel_stem(prop))
}

fn camel_stem(name: &str) -> &str {
    let mut end = name.len();
    for (i, c) in name.char_indices().skip(1) {
        if c.is_uppercase() {
            end = i;
            break;
        }
    }
    &name[..end]
}

fn names_eq(a: &str, b: &str) -> bool {
    a == b || a.eq_ignore_ascii_case(b)
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
    for effect in &rule.consequences {
        match &effect.consequence {
            Consequence::Derive(p)
            | Consequence::Establish(p)
            | Consequence::Terminate(p)
            | Consequence::Suspend(p) => collect_prop_idents(p, &mut names),
            _ => {}
        }
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

fn collect_guard_idents(guard: &Guard, out: &mut BTreeSet<String>) {
    match guard {
        Guard::Operative(prop, _) | Guard::Derived(prop) => collect_prop_idents(prop, out),
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
    for (index, binder) in binders.iter().enumerate() {
        let last_binder = index + 1 == binders.len();
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
        if truncated && !last_binder {
            return Err(EngineError::Unsupported(format!(
                "substitution cap {MAX_SUBSTITUTIONS} reached with binders remaining"
            )));
        }
        if next.is_empty() {
            return Ok(Vec::new());
        }
        acc = next;
        if truncated {
            break;
        }
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
    left == right || arg_label_value(left) == arg_label_value(right)
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

fn arg_label(term: &Term) -> String {
    match term {
        Term::Ident(s) | Term::String(s) | Term::Binder(s) => s.clone(),
        Term::Bool(b) => b.to_string(),
        Term::Int(i) => i.to_string(),
        Term::Decimal(d) => d.to_string(),
        Term::Apply { ctor, args } if args.is_empty() => ctor.clone(),
        Term::Call { callee, args } if args.is_empty() => callee.clone(),
        other => format!("{other:?}"),
    }
}

fn arg_label_value(value: &Value) -> String {
    match value {
        Value::Entity(s) | Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Decimal(d) => d.to_string(),
        Value::Ctor { name, fields } if fields.is_empty() => name.clone(),
        Value::Prop(p) => p.predicate.clone(),
        other => other.display_label(),
    }
}

fn term_is_ground(term: &Term) -> bool {
    match term {
        Term::Ident(name) | Term::Binder(name) => name.starts_with(|c: char| c.is_uppercase()),
        Term::Bool(_)
        | Term::Int(_)
        | Term::Decimal(_)
        | Term::String(_)
        | Term::Instant(_)
        | Term::Duration(_) => true,
        Term::Apply { args, .. } | Term::Call { args, .. } | Term::Set(args) => {
            args.iter().all(term_is_ground)
        }
        _ => false,
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
    if s.eq_ignore_ascii_case("true") {
        return Term::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return Term::Bool(false);
    }
    if let Ok(i) = s.parse::<i64>() {
        return Term::Int(i);
    }
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        return Term::String(s[1..s.len() - 1].to_owned());
    }
    Term::Ident(s.to_owned())
}

fn split_commas(src: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0i32;
    for (i, c) in src.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
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
mod tests {
    use super::*;
    use fidryn_core::Interval;
    use fidryn_core::case::CaseDetermination;
    use fidryn_core::ids::{JurisdictionId, ModuleId, SourceManifestId, SourceSnapshotId};
    use fidryn_core::{Instant, LedgerEvent, Value};
    use std::collections::BTreeMap;

    fn instant(text: &str) -> Instant {
        Instant::parse(text).unwrap()
    }

    fn empty_module() -> CoreModule {
        CoreModule {
            id: ModuleId::of(b"Review"),
            name: "Review".into(),
            version: "0.1.0".into(),
            snapshot: SourceSnapshotId::of(b"s"),
            manifest: SourceManifestId::of(b"m"),
            jurisdiction: JurisdictionId::of(b"j"),
            outside_scope: Vec::new(),
            declarations: Vec::new(),
            nominations: Vec::new(),
            queries: Vec::new(),
            verifications: Vec::new(),
            assertions: Vec::new(),
        }
    }

    fn compute(case: &CaseRecord, ctx: &RunContext) -> DerivedWorld {
        DerivedWorld::compute(&empty_module(), case, ctx, &BTreeMap::new()).unwrap()
    }

    #[test]
    fn future_determinations_are_not_visible_at_an_earlier_known_time() {
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: true,
            decider: "Reviewer".into(),
            recorded_at: Some(instant("2034-01-01T00:00:00Z")),
        });
        let ctx = RunContext::new(
            instant("2033-01-01T00:00:00Z"),
            instant("2033-01-01T00:00:00Z"),
        );
        let world = compute(&case, &ctx);
        assert!(
            !world.holds(&parse_prop_issue("P(A)")),
            "future knowledge must not establish a past answer"
        );
        assert!(!world.holds_named("P"));
    }

    #[test]
    fn determination_at_known_time_is_visible() {
        let known = instant("2033-01-01T00:00:00Z");
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "P(A)".into(),
            protocol: "P".into(),
            established: true,
            decider: "Reviewer".into(),
            recorded_at: Some(known),
        });
        let world = compute(&case, &RunContext::new(known, known));
        assert!(world.holds(&parse_prop_issue("P(A)")));
    }

    #[test]
    fn determination_without_recorded_at_remains_visible() {
        let known = instant("2033-01-01T00:00:00Z");
        let mut case = CaseRecord::default();
        case.determinations.push(CaseDetermination {
            issue: "InvoiceIssued".into(),
            protocol: "Invoice".into(),
            established: false,
            decider: "Tribunal".into(),
            recorded_at: None,
        });
        let world = compute(&case, &RunContext::new(known, known));
        assert!(world.denied(&parse_prop_issue("InvoiceIssued")));
    }

    #[test]
    fn correction_performed_payload_does_not_seed_performed() {
        let t = instant("2033-01-01T00:00:00Z");
        let mut case = CaseRecord::default();
        case.events.push(LedgerEvent {
            kind: "correction".into(),
            valid_time: Interval::always(),
            record_time: t,
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        });
        let world = compute(&case, &RunContext::new(t, t));
        assert!(
            !world.holds_named("performed"),
            "an ungated event label must not admit a performed payload"
        );
    }
}
