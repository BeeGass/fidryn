use fidryn_core::ir::{CoreDecl, QueryPlan};
use fidryn_core::value::Term;
use fidryn_core::{CoreModule, Diagnostic, DiagnosticCode, SourceManifest};

fn compile(body: &str) -> Result<CoreModule, Vec<Diagnostic>> {
    let source = format!("module FrontendAudit version \"0.1.0\" {{ {body} }}");
    let parsed = fidryn_syntax::parse_file(&source);
    let manifest = SourceManifest::default();
    if parsed.has_errors() {
        return Err(parsed.diagnostics);
    }
    let hir = fidryn_hir::elaborate(&parsed, &manifest)?;
    fidryn_check::check(&hir, &manifest)
}

fn rejects(body: &str, code: DiagnosticCode) {
    let diagnostics = compile(body).unwrap_err();
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.code == code),
        "{diagnostics:?}"
    );
}

#[test]
fn invalid_guards_cannot_establish_propositions_or_attach_duties() {
    for guard in ["0", "\"yes\"", "determined false", "P()"] {
        let code = if guard == "P()" {
            DiagnosticCode::E310
        } else {
            DiagnosticCode::E210
        };
        rejects(
            &format!("proposition P(); rule R: derive {{ when {guard} then derive P() }}"),
            code,
        );
    }
    rejects(
        "entity A: NaturalPerson; duty D { bearer A attaches when 0 content USD(1.00) }",
        DiagnosticCode::E210,
    );
    assert!(compile("proposition P(); rule R: derive { when false then derive P() }").is_ok());
}

#[test]
fn prefixes_are_preserved_for_functions_and_specialized_goals() {
    let module = compile("fn f() -> Int { require false; return 7 } decision D() -> Int { returns 7 } query q() -> Int { require false; goal RunDecision { decision D } }").unwrap();
    let body = module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            CoreDecl::Function(function) if function.name == "f" => function.body.as_ref(),
            _ => None,
        })
        .unwrap();
    assert!(
        matches!(body, Term::Apply { ctor, args } if ctor == "seq" && matches!(&args[0], Term::Apply { ctor, args } if ctor == "require" && args == &[Term::Bool(false)]))
    );
    assert!(
        matches!(&module.query("q").unwrap().plan, QueryPlan::Guarded { prefix, .. } if !prefix.is_empty())
    );
}

#[test]
fn named_labels_are_not_lowered_to_calls() {
    let module = compile("fn x(v: Int) -> Int { v + 1 } fn id(x: Int) -> Int { x } query q() -> Int { return id(x = 10) }").unwrap();
    assert!(
        matches!(&module.query("q").unwrap().plan, QueryPlan::Evaluate(Term::Call { args, .. }) if matches!(&args[0], Term::Apply { ctor, args } if ctor == "$named" && args == &[Term::String("x".into()), Term::Int(10)]))
    );
    rejects(
        "fn id(x: Int) -> Int { x } query q() -> Int { return id(y = 10) }",
        DiagnosticCode::E210,
    );
    rejects(
        "fn id(x: Int, y: Int) -> Int { x } query q() -> Int { return id(x = 1, x = 2) }",
        DiagnosticCode::E210,
    );
}

#[test]
fn signature_and_purity_checks_close_untyped_escape_paths() {
    for body in [
        "fn id(x: Int) -> Int { x } fn f() -> Int { id(unknown) }",
        "fn f() -> Int { if false { 7 } else { unknown } }",
        "calc f() -> Int { if false { 1 } }",
        "proposition P(x: Int); query q() -> Bool { return determined P(\"wrong\") }",
        "proposition P(x: Int); query q() -> Bool { return determined P() }",
        "query q() -> Bool { return true + false }",
        "query q() -> Bool { return 1 < \"x\" }",
        "query q() -> Money<USD> { return USD(1.00) + EUR(1.00) }",
        "query q() -> Money<USD> { return USD(1.00) + 1.00 }",
    ] {
        rejects(body, DiagnosticCode::E210);
    }
    rejects(
        "proposition Gate(); calc f() -> Bool { determined Gate() }",
        DiagnosticCode::E420,
    );
    rejects(
        "proposition Gate(); fn g() -> Bool { determined Gate() } calc f() -> Bool { g() }",
        DiagnosticCode::E420,
    );
}

#[test]
fn lexical_scope_and_trivia_do_not_change_effects_or_recursion() {
    assert!(
        compile(
            "fn identity(identity: Int) -> Int { identity } query q() -> Int { return identity(7) }"
        )
        .is_ok()
    );
    assert!(compile("fn g() -> Bool ! {Observe} { true } fn f(g: Bool) -> Bool { g } query automatic q() -> Bool { return f(false) }").is_ok());
    let module = compile(
        "fn f() -> Int {\n// fuel = 0 !{Observe}\n7 } query automatic q() -> Int { return f() }",
    )
    .unwrap();
    assert!(module.query("q").unwrap().effects.is_empty());
    assert!(module.declarations.iter().any(|declaration| matches!(declaration, CoreDecl::Function(function) if function.name == "f" && function.fuel.is_none())));
    assert!(compile("fn f() -> String { \"fuel = 0 !{Observe}\" } query automatic q() -> String { return f() }").is_ok());
}

#[test]
fn duplicates_are_rejected_with_related_locations() {
    for body in [
        "query q() -> Int { return 1 } query q() -> Int { return 2 }",
        "entity A: NaturalPerson; entity A: NaturalPerson",
        "proposition P(); proposition P()",
    ] {
        let diagnostics = compile(body).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::E200
                    && diagnostic.primary_span.is_some()
                    && !diagnostic.related_spans.is_empty())
        );
    }
}

#[test]
fn nominations_refer_to_existing_entities_without_redeclaring_them() {
    let module = compile("entity Alice: NaturalPerson; entity Bob: NaturalPerson; entity T: Trust; office TrusteeOf(t: Trust) -> LegalPerson; nomination Alice for TrusteeOf(T) rank 1; nomination Bob for TrusteeOf(T) rank 2; query q() -> Bool { return true }").unwrap();
    assert_eq!(module.nominations.len(), 2);
    assert!(
        module
            .nominations
            .iter()
            .any(|nomination| nomination.candidate == "Alice")
    );
}

#[test]
fn records_and_sets_are_typed_and_lowered_as_values() {
    let module = compile("record_type R { x: Int } query q() -> R { return {x: 1} } query all() -> Bool { return for_all x in {1, 2}: x > 0 }").unwrap();
    assert!(module.declarations.iter().any(
        |declaration| matches!(declaration, CoreDecl::RecordType(record) if record.name == "R")
    ));
    assert!(
        matches!(&module.query("q").unwrap().plan, QueryPlan::Evaluate(Term::Record(fields)) if fields.get("x") == Some(&Term::Int(1)))
    );
    rejects(
        "record_type R { x: Int } query q() -> R { return {x: \"wrong\"} }",
        DiagnosticCode::E210,
    );
    rejects(
        "record_type R { x: Int } query q() -> R { return {unknown: 1} }",
        DiagnosticCode::E210,
    );
}

#[test]
fn every_verification_assertion_is_preserved() {
    let module = compile("verify Both { assert true; assert false }").unwrap();
    assert_eq!(
        module.verifications[0].assertions,
        vec![Term::Bool(true), Term::Bool(false)]
    );
}

#[test]
fn explicit_import_digest_cannot_fall_back_to_a_path_match() {
    let source = "module M version \"1\" { import Other.Law version \"1\" { digest \"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" } }";
    let manifest = fidryn_core::SourceManifest {
        artifacts: vec![fidryn_core::ManifestArtifact {
            path: "Other.Law".into(),
            digest: "fixture".into(),
            kind: "text".into(),
            effective: "1".into(),
            weight: fidryn_core::SourceWeight::Explanatory,
        }],
        ..Default::default()
    };
    let parsed = fidryn_syntax::parse_file(source);
    let hir = fidryn_hir::elaborate(&parsed, &manifest).unwrap();
    assert!(fidryn_check::check(&hir, &manifest).is_err());
}

#[test]
fn generic_instantiation_uses_exact_identity_and_substitutes_nominal_units() {
    use fidryn_core::types::{PrimitiveType, Type};
    let source = "module Coin<C> version \"1\" { import Coin<USD>; fn identity(x: Money<C>) -> Money<C> { x } query q(x: Money<C>) -> Money<C> { return identity(x) } }";
    let manifest = SourceManifest::default();
    let parsed = fidryn_syntax::parse_file(source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let hir = fidryn_hir::elaborate(&parsed, &manifest).unwrap();
    let module = fidryn_check::check(&hir, &manifest).unwrap();
    assert_eq!(module.name, "Coin<USD>");
    assert_eq!(
        module.query("q").unwrap().result_type,
        Type::Primitive(PrimitiveType::Money {
            currency: "USD".into()
        })
    );
    let source =
        "module Foo<T> version \"1\" { import Other.Foo<Int>; query q() -> Int { return 7 } }";
    let parsed = fidryn_syntax::parse_file(source);
    let hir = fidryn_hir::elaborate(&parsed, &manifest).unwrap();
    assert_eq!(fidryn_check::check(&hir, &manifest).unwrap().name, "Foo<T>");
}

#[test]
fn malformed_sets_and_modal_arities_are_rejected() {
    rejects(
        "query q() -> FiniteSet<Int> { return {\"wrong\", 1, 2} }",
        DiagnosticCode::E210,
    );
    rejects(
        "proposition P(); query q() -> Bool { return determined(P(), 1, 2) }",
        DiagnosticCode::E100,
    );
    rejects(
        "query q() -> Money<USD> { return USD(\"wrong\") }",
        DiagnosticCode::E210,
    );
    rejects(
        "query q() -> NonEmptySet<Int> { return {} }",
        DiagnosticCode::E210,
    );
}

#[test]
fn decisions_preserve_typed_signatures_and_literal_results() {
    let module = compile("decision D(x: Int) -> Int { returns x + 1 } query q() -> Int { goal RunDecision { decision D arguments {2} } }").unwrap();
    let decision = module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            CoreDecl::Decision(decision) => Some(decision),
            _ => None,
        })
        .unwrap();
    assert_eq!(decision.binders.len(), 1);
    assert_eq!(
        &decision.declared_result.as_ref().unwrap().expression,
        &Term::Apply {
            ctor: "+".into(),
            args: vec![Term::Ident("x".into()), Term::Int(1)],
        }
    );
}

#[test]
fn aliases_record_members_and_nominal_sum_constructors_are_resolved() {
    let module = compile("type Count = Int; type Box<T> = T; type Event = Started | Counted(Count); record_type R { amount: Box<Count> } fn amount(r: R) -> Count { return r.amount } query q() -> Event { return Counted(amount({amount: 3})) }").unwrap();
    assert!(
        module
            .declarations
            .iter()
            .any(|declaration| matches!(declaration, CoreDecl::Type(ty) if ty.name == "Event"))
    );
    for body in [
        "type Event = Started | Counted(Int); query q() -> Event { return Counted(\"bad\") }",
        "type Event = Started | Counted(Int); query q() -> Event { return Unrelated(1) }",
        "type Event = Started | Counted(Int); query q() -> Event { return Started(1) }",
        "record_type R { amount: Int } query q(r: R) -> Int { return r.missing }",
        "type A = B; type B = A; query q() -> Int { return 1 }",
    ] {
        rejects(body, DiagnosticCode::E210);
    }
}

#[test]
fn generic_case_input_requires_an_explicit_nested_type_and_string_key() {
    let module = compile("query q() -> FiniteSet<Money<USD>> { return case_input<FiniteSet<Money<USD>>>(\"amounts\") }").unwrap();
    assert!(module.declarations.iter().any(|declaration| matches!(declaration, CoreDecl::RecordType(record) if record.name == "JudgmentEntry")));
    for body in [
        "query q() -> Int { return case_input(\"x\") }",
        "query q() -> Int { return case_input<Int>(1) }",
        "query q() -> Int { return case_input<Int, Bool>(\"x\") }",
        "query q() -> Bool { return case_input<Int>(\"x\") }",
    ] {
        rejects(body, DiagnosticCode::E210);
    }
}

#[test]
fn observations_preserve_and_typecheck_each_validator_and_record_binding() {
    let module = compile("entity Alice: NaturalPerson; record_type R { person: NaturalPerson, n: Int } proposition P(person: NaturalPerson); observation Read(r: R) establishes P(r.person) { authenticate r.person == Alice; require r.n > 0 } query q() -> Bool { return true }").unwrap();
    let observation = module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            CoreDecl::Observation(observation) => Some(observation),
            _ => None,
        })
        .unwrap();
    assert_eq!(observation.record_binder, "r");
    assert_eq!(observation.validator_terms.len(), 2);
    assert!(observation.record_type.is_some());
    assert_eq!(
        observation.establishes.arguments,
        vec![Term::Field {
            base: Box::new(Term::Ident("r".into())),
            name: "person".into()
        }]
    );
    rejects(
        "record_type R {n: Int} proposition P(); observation Read(r: R) establishes P() { require r.missing > 0 } query q() -> Bool { return true }",
        DiagnosticCode::E210,
    );
}

#[test]
fn judgment_policies_and_every_condition_survive_source_compilation() {
    let module = compile(r#"
        record_type R { issuer: LicensedPhysician, subject: NaturalPerson, purpose: CapacityPurpose, conclusion: Supports | Opposes, signed_at: Time }
        proposition Incapacitated(person: NaturalPerson, purpose: CapacityPurpose)
        judgment J(person: NaturalPerson, trust: Trust) decides Incapacitated(person, Administer(trust)) {
            authority concurrence(2 distinct LicensedPhysician)
            record requires first: R
            record requires second: R
            where same_subject_and_purpose(first, second) and first.issuer != second.issuer
            establish when first.conclusion == Supports and second.conclusion == Supports
            reject when first.conclusion == Opposes and second.conclusion == Opposes
            otherwise suspend NeedJudgment { issue: Incapacitated(person, Administer(trust)), protocol: CourtReview }
            on established make issue operative in Administration(trust)
            review by Court under DeNovo
        }
        query q() -> Bool { return true }
    "#).unwrap();
    let judgment = module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            CoreDecl::Judgment(judgment) => Some(judgment),
            _ => None,
        })
        .unwrap();
    assert_eq!(judgment.binders.len(), 2);
    assert_eq!(judgment.record_requires.len(), 2);
    assert_eq!(judgment.constraints.len(), 1);
    assert!(judgment.establish_when.is_some() && judgment.reject_when.is_some());
    assert!(judgment.otherwise.is_some() && judgment.review.is_some());
    assert_eq!(judgment.on_established.len(), 1);
    assert!(judgment.unsupported_clauses.is_empty());
    assert!(matches!(
        judgment.authority_policy,
        Some(fidryn_core::ir::JudgmentAuthority::Concurrence {
            minimum: 2,
            distinct: true,
            ..
        })
    ));
}

#[test]
fn source_selection_preserves_scope_predicate_and_ordering() {
    let module = compile(
        r#"
        entity T: Trust
        office TrusteeOf(t: Trust) occupied_by LegalPerson
        proposition Eligible(person: LegalPerson, office: Office)
        proposition Occupies(person: LegalPerson, office: Office)
        legal_act AcceptOffice(person: LegalPerson, office: Office) {}
        rule Appoint : constitutive {
            require closed nominations_for(TrusteeOf(T))
            require unique nomination_rank
            select unique candidate from nominations_for(TrusteeOf(T))
                where operative Eligible(candidate, TrusteeOf(T))
                    and effective AcceptOffice(candidate, TrusteeOf(T))
                order_by nomination_rank ascending
            when vacancy_or_suspension(TrusteeOf(T))
            then establish Occupies(candidate, TrusteeOf(T))
        }
        query q() -> Bool { return true }
    "#,
    )
    .unwrap();
    let selection = module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            CoreDecl::Rule(rule) if rule.name == "Appoint" => rule.selection.as_ref(),
            _ => None,
        })
        .unwrap();
    assert_eq!(selection.binder, "candidate");
    assert_eq!(selection.ordering, ["nomination_rank"]);
    assert_eq!(selection.ascending, [true]);
    assert!(matches!(
        selection.predicate,
        fidryn_core::ir::Guard::And(_)
    ));
}
