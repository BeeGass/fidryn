//! Known limits of the 0.2 interpreter, pinned as executable facts.
//!
//! Each test states what the implementation does today, not what it should
//! do. `docs/implementation-status.md` cites these tests by name. When a
//! limit is lifted the test is replaced in the same change, never deleted
//! on its own.

use fidryn_cli::compile_source;
use fidryn_core::ir::Consequence;
use fidryn_core::{CoreDecl, CoreModule, DiagnosticCode, SourceManifest};
use fidryn_verify::verify_property;

fn source(name: &str, body: &str) -> String {
    format!(
        "module {name} version \"0.1.0\" {{\n    jurisdiction Test\n    source_snapshot \"limits\"\n    effective_at 2026-01-01\n    recorded_at 2026-01-01T00:00:00Z\n    outside_scope {{ everything_else }}\n{body}\n}}\n"
    )
}

fn compile(name: &str, body: &str) -> CoreModule {
    compile_source(&source(name, body), &SourceManifest::default()).expect("limit program compiles")
}

fn codes(name: &str, body: &str) -> Vec<DiagnosticCode> {
    compile_source(&source(name, body), &SourceManifest::default())
        .expect_err("limit program is rejected")
        .into_iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn a_duration_cannot_be_added_to_a_time_in_an_expression() {
    assert_eq!(
        codes(
            "Limits.TimePlusDuration",
            "    query automatic due(start: Time) -> Time { return start + 30 days }"
        ),
        [DiagnosticCode::E210]
    );
}

#[test]
fn a_date_literal_is_untyped_in_an_expression() {
    assert_eq!(
        codes(
            "Limits.DateLiteral",
            "    query automatic before(d: Date) -> Bool { return d <= 2026-12-31 }"
        ),
        [DiagnosticCode::E210]
    );
}

#[test]
fn the_activate_verb_is_lowered_as_derive() {
    let module = compile(
        "Limits.RuleVerb",
        "    entity Acme : LegalPerson\n    proposition Registered(who: LegalPerson)\n    proposition Flagged(who: LegalPerson)\n    rule Flag : constitutive {\n        when operative Registered(Acme)\n        then activate Flagged(Acme)\n    }\n    query q() -> Int { return 7 }",
    );
    let rule = module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::Rule(rule) if rule.name == "Flag" => Some(rule),
            _ => None,
        })
        .expect("rule Flag");
    assert!(matches!(
        &rule.consequences[0].consequence,
        Consequence::Derive(prop) if prop.predicate == "Flagged"
    ));
}

#[test]
fn an_import_that_names_nothing_is_accepted_and_its_alias_is_ignored() {
    compile(
        "Limits.Import",
        "    import Nowhere.DoesNotExist version \"9.9.9\" {\n        alias Gone\n    }\n    query q() -> Int { return 7 }",
    );
}

#[test]
fn power_and_legal_act_bodies_are_not_parsed_so_unknown_fields_pass() {
    let module = compile(
        "Limits.PowerAct",
        "    entity Acme : LegalPerson\n    proposition Adopted(who: LegalPerson)\n    power AmendPower {\n        holder Acme\n        subject Acme\n        effect Adopted(Acme)\n        this_field_does_not_exist 42\n    }\n    legal_act Amend(who: LegalPerson) {\n        performer who\n        nonsense_field also_ignored\n    }\n    query q() -> Int { return 7 }",
    );
    assert!(
        !module
            .declarations
            .iter()
            .any(|decl| matches!(decl, CoreDecl::Power(_) | CoreDecl::LegalAct(_)))
    );
}

#[test]
fn a_source_declaration_does_not_reach_core_and_its_artifact_is_never_read() {
    let module = compile(
        "Limits.Source",
        "    source Statute {\n        kind statute\n        authority Congress\n        citation \"1 U.S.C. 1\"\n        artifact \"sources/none.txt\"\n        digest fixture\n        effective [2026-01-01, +inf)\n    }\n    query q() -> Int { return 7 }",
    );
    assert!(
        !module
            .declarations
            .iter()
            .any(|decl| matches!(decl, CoreDecl::Source(_)))
    );
}

#[test]
fn an_office_keeps_none_of_its_declared_fields() {
    let module = compile(
        "Limits.Office",
        "    office Board(of: LegalPerson) occupied_by NaturalPerson {\n        cardinality 3..9\n        competence {Manage}\n    }\n    query q() -> Int { return 7 }",
    );
    let office = module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::Office(office) => Some(office),
            _ => None,
        })
        .expect("office");
    assert_eq!(
        (
            office.cardinality_min,
            office.cardinality_max,
            office.competence.len()
        ),
        (0, Some(1), 0)
    );
}

#[test]
fn a_misspelled_type_name_becomes_a_new_sort_without_a_diagnostic() {
    compile(
        "Limits.Misspelled",
        "    proposition Holds(who: NaturalPersn)\n    query q() -> Int { return 7 }",
    );
}

#[test]
fn a_query_that_ends_in_a_require_has_an_untyped_result() {
    assert_eq!(
        codes(
            "Limits.TrailingRequire",
            "    entity Acme : LegalPerson\n    proposition Sound(who: LegalPerson)\n    query q() -> Bool ! {Determine} { goal Evaluate { require determined Sound(Acme) using Determine } }"
        ),
        [DiagnosticCode::E210]
    );
}

#[test]
fn a_rule_whose_basis_is_a_bare_name_is_not_checked_and_swallows_the_next_declaration() {
    let module = compile(
        "Limits.BareBasis",
        "    proposition P(name: String)\n    proposition Q(name: String)\n    rule R : constitutive from Basis {\n        when operative P(undeclared)\n        then derive Q(\"x\")\n    }\n    query a() -> Int { return 1 }\n    query b() -> Int { return 2 }",
    );
    let names: Vec<&str> = module
        .queries
        .iter()
        .map(|query| query.name.as_str())
        .collect();
    assert_eq!(names, ["b"]);
}

#[test]
fn an_assertion_ending_in_an_integer_literal_swallows_the_next_assertion() {
    let module = compile(
        "Limits.SwallowedAssertion",
        "    verify Swallowed {\n        assert true or 1 == 2\n        assert false\n    }",
    );
    let property = module
        .verifications
        .iter()
        .find(|property| property.name == "Swallowed")
        .expect("property Swallowed");
    assert_eq!(property.assertions.len(), 1);
    assert!(verify_property(&module, "Swallowed").is_proved());
}
