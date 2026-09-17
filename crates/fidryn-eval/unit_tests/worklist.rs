use super::*;
use fidryn_core::Interval;
use fidryn_core::case::CaseDetermination;
use fidryn_core::ids::{
    EffectId, JurisdictionId, ModuleId, NodeId, OriginId, SourceManifestId, SourceSnapshotId,
};
use fidryn_core::ir::{NodeMeta, RuleKind};
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
fn seed_determinations_retains_protocol_and_decider() {
    let known = instant("2033-01-01T00:00:00Z");
    let mut case = CaseRecord::default();
    case.determinations.push(CaseDetermination {
        issue: "P(A)".into(),
        protocol: "Eligibility".into(),
        established: true,
        decider: "Reviewer".into(),
        recorded_at: Some(known),
    });
    let world = compute(&case, &RunContext::new(known, known));
    let rec = world
        .seeded_determinations
        .iter()
        .find(|d| d.prop.predicate == "P")
        .expect("seeded determination");
    assert_eq!(rec.protocol, "Eligibility");
    assert_eq!(rec.decider, "Reviewer");
    assert!(rec.established);
    assert_eq!(
        world.modal_determined(&parse_prop_issue("P(A)")),
        Some(true)
    );
}

#[test]
fn denied_and_held_determination_is_not_determined_true() {
    let known = instant("2033-01-01T00:00:00Z");
    let mut case = CaseRecord::default();
    case.determinations.push(CaseDetermination {
        issue: "P(A)".into(),
        protocol: "P".into(),
        established: true,
        decider: "YesVote".into(),
        recorded_at: Some(known),
    });
    case.determinations.push(CaseDetermination {
        issue: "P(A)".into(),
        protocol: "P".into(),
        established: false,
        decider: "NoVote".into(),
        recorded_at: Some(known),
    });
    let world = compute(&case, &RunContext::new(known, known));
    assert_ne!(
        world.modal_determined(&parse_prop_issue("P(A)")),
        Some(true),
        "a qualified denial must not be treated as determined true"
    );
    assert_eq!(
        world.modal_determined(&parse_prop_issue("P(A)")),
        Some(false)
    );
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

fn world_of(module: &CoreModule) -> DerivedWorld {
    world_with_facts(module, &[])
}

fn world_with_facts(module: &CoreModule, facts: &[(&str, bool)]) -> DerivedWorld {
    let mut case = CaseRecord::default();
    for (name, value) in facts {
        case.facts.insert((*name).into(), Value::Bool(*value));
    }
    let t = instant("2033-01-01T00:00:00Z");
    DerivedWorld::compute(module, &case, &RunContext::new(t, t), &BTreeMap::new())
        .expect("worklist")
}

fn held_p(world: &DerivedWorld) -> bool {
    world.holds(&PropTerm::new("P", Vec::new()))
}

fn held_q(world: &DerivedWorld) -> bool {
    world.holds(&PropTerm::new("Q", Vec::new()))
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

fn prop(name: &str) -> PropTerm {
    PropTerm::new(name, Vec::new())
}

fn effects(rule: &str, consequences: Vec<Consequence>) -> Vec<CoreEffect> {
    let meta = test_meta(rule);
    consequences
        .into_iter()
        .enumerate()
        .map(|(i, consequence)| CoreEffect {
            id: EffectId::of(format!("{rule}:{i}").as_bytes()),
            consequence,
            meta: meta.clone(),
        })
        .collect()
}

fn rule(
    name: &str,
    kind: RuleKind,
    guard: Guard,
    consequences: Vec<Consequence>,
    fallback: Option<Vec<Consequence>>,
) -> CoreDecl {
    CoreDecl::Rule(CoreRule {
        id: NodeId::of(name.as_bytes()),
        name: name.into(),
        kind,
        binders: Vec::new(),
        selection: None,
        guard,
        consequences: effects(name, consequences),
        fallback: fallback.map(|items| effects(&format!("{name}:otherwise"), items)),
        meta: test_meta(name),
    })
}

fn module_with_rules(decls: Vec<CoreDecl>) -> CoreModule {
    let mut module = empty_module();
    module.declarations = decls;
    module
}

fn when_false() -> Guard {
    Guard::Not(Box::new(Guard::Satisfied))
}

fn operative(name: &str) -> Guard {
    Guard::Operative(prop(name), String::new())
}

#[test]
fn false_rule_guard_does_not_establish_a_proposition() {
    let module = module_with_rules(vec![rule(
        "R",
        RuleKind::Derive,
        when_false(),
        vec![Consequence::Derive(prop("P"))],
        None,
    )]);
    let world = world_of(&module);
    assert!(!held_p(&world), "when false must not derive P: {world:?}");
}

#[test]
fn true_rule_guard_still_derives_the_then_consequence() {
    let module = module_with_rules(vec![rule(
        "R",
        RuleKind::Derive,
        Guard::Satisfied,
        vec![Consequence::Derive(prop("P"))],
        None,
    )]);
    let world = world_of(&module);
    assert!(held_p(&world), "when true must derive P: {world:?}");
}

#[test]
fn otherwise_is_not_an_additional_then_consequence() {
    let module = module_with_rules(vec![rule(
        "R",
        RuleKind::Derive,
        Guard::Satisfied,
        vec![Consequence::Derive(prop("P"))],
        Some(vec![Consequence::Derive(prop("Q"))]),
    )]);
    let world = world_of(&module);
    assert!(held_p(&world), "when true must derive P: {world:?}");
    assert!(
        !held_q(&world),
        "otherwise must not fire when the guard holds: {world:?}"
    );
}

#[test]
fn otherwise_fires_only_when_the_guard_is_established_false() {
    let module = module_with_rules(vec![rule(
        "R",
        RuleKind::Derive,
        when_false(),
        vec![Consequence::Derive(prop("P"))],
        Some(vec![Consequence::Derive(prop("Q"))]),
    )]);
    let world = world_of(&module);
    assert!(
        !held_p(&world),
        "then must not fire on a false guard: {world:?}"
    );
    assert!(
        held_q(&world),
        "otherwise must fire on a false guard: {world:?}"
    );
}

#[test]
fn unknown_guard_fires_neither_then_nor_otherwise() {
    let module = module_with_rules(vec![rule(
        "R",
        RuleKind::Derive,
        operative("Unobtanium"),
        vec![Consequence::Derive(prop("P"))],
        Some(vec![Consequence::Derive(prop("Q"))]),
    )]);
    let world = world_of(&module);
    assert!(
        !held_p(&world) && !held_q(&world),
        "unknown guard must fire neither branch: {world:?}"
    );
}

#[test]
fn rule_require_false_does_not_derive() {
    let module = module_with_rules(vec![rule(
        "R",
        RuleKind::Derive,
        Guard::And(vec![Guard::Satisfied, when_false()]),
        vec![Consequence::Derive(prop("P"))],
        None,
    )]);
    let world = world_of(&module);
    assert!(
        !held_p(&world),
        "require false must not derive P: {world:?}"
    );
}

fn derive_b() -> CoreDecl {
    rule(
        "R1",
        RuleKind::Derive,
        operative("A"),
        vec![Consequence::Derive(prop("B"))],
        None,
    )
}

fn terminate_a() -> CoreDecl {
    rule(
        "R2",
        RuleKind::Constitutive,
        Guard::Satisfied,
        vec![Consequence::Terminate(prop("A"))],
        None,
    )
}

#[test]
fn derive_then_terminate_does_not_leave_unsupported_facts() {
    let module = module_with_rules(vec![derive_b(), terminate_a()]);
    let world = world_with_facts(&module, &[("A", true)]);
    assert!(
        !world.holds_named("B"),
        "B must not remain after A is terminated: {world:?}"
    );
}

#[test]
fn terminate_then_derive_does_not_leave_unsupported_facts() {
    let module = module_with_rules(vec![terminate_a(), derive_b()]);
    let world = world_with_facts(&module, &[("A", true)]);
    assert!(
        !world.holds_named("B"),
        "B must not remain after A is terminated: {world:?}"
    );
}

#[test]
fn unsupported_chain_is_dropped_after_premise_terminate() {
    let module = module_with_rules(vec![
        rule(
            "R1",
            RuleKind::Derive,
            operative("A"),
            vec![Consequence::Derive(prop("B"))],
            None,
        ),
        rule(
            "R2",
            RuleKind::Derive,
            operative("B"),
            vec![Consequence::Derive(prop("C"))],
            None,
        ),
        terminate_a(),
    ]);
    let world = world_with_facts(&module, &[("A", true)]);
    assert!(
        !world.holds_named("B") && !world.holds_named("C"),
        "C must not survive on support that only ran through terminated A: {world:?}"
    );
}

#[test]
fn independent_support_survives_terminate_of_other_premise() {
    let module = module_with_rules(vec![
        rule(
            "R1",
            RuleKind::Derive,
            operative("A"),
            vec![Consequence::Derive(prop("B"))],
            None,
        ),
        rule(
            "R2",
            RuleKind::Derive,
            operative("D"),
            vec![Consequence::Derive(prop("B"))],
            None,
        ),
        terminate_a(),
    ]);
    let world = world_with_facts(&module, &[("A", true), ("D", true)]);
    assert!(
        world.holds_named("B"),
        "B still has support from D: {world:?}"
    );
}

fn true_false_module(false_first: bool) -> CoreModule {
    let when_false_rule = rule(
        "F",
        RuleKind::Derive,
        when_false(),
        vec![Consequence::Derive(prop("P"))],
        None,
    );
    let when_true_rule = rule(
        "T",
        RuleKind::Derive,
        Guard::Satisfied,
        vec![Consequence::Derive(prop("Q"))],
        None,
    );
    if false_first {
        module_with_rules(vec![when_false_rule, when_true_rule])
    } else {
        module_with_rules(vec![when_true_rule, when_false_rule])
    }
}

#[test]
fn positive_only_when_true_when_false_is_order_independent() {
    let false_first = world_of(&true_false_module(true));
    let true_first = world_of(&true_false_module(false));
    assert!(
        !held_p(&false_first) && held_q(&false_first),
        "{false_first:?}"
    );
    assert!(
        !held_p(&true_first) && held_q(&true_first),
        "{true_first:?}"
    );
    assert_eq!(false_first.held, true_first.held);
    assert_eq!(false_first.denied, true_first.denied);
}
