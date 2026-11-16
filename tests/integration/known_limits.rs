//! Known limits of the 0.2 interpreter, pinned as executable facts.
//!
//! Each test states what the implementation does today, not what it should
//! do. `docs/implementation-status.md` cites these tests by name. When a
//! limit is lifted the test is replaced in the same change, never deleted
//! on its own.

use fidryn_cli::compile_source;
use fidryn_core::ir::Consequence;
use fidryn_core::{
    CaseRecord, CoreDecl, CoreModule, DiagnosticCode, Instant, Outcome, QueryName, RunContext,
    SourceManifest, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use fidryn_verify::verify_property;
use std::collections::BTreeMap;

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
fn a_rule_basis_that_names_nothing_is_accepted() {
    let module = compile(
        "Limits.RuleBasis",
        "    proposition P()\n    proposition Q()\n    rule R : derive from NoSuchSource {\n        when operative P()\n        then derive Q()\n    }\n    query q() -> Int { return 7 }",
    );
    let rule = module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::Rule(rule) if rule.name == "R" => Some(rule),
            _ => None,
        })
        .expect("rule R");
    assert!(matches!(
        &rule.consequences[0].consequence,
        Consequence::Derive(prop) if prop.predicate == "Q"
    ));
    // Nothing of the basis reaches the intermediate representation.
    assert!(!format!("{module:?}").contains("NoSuchSource"));
}

#[test]
fn the_valid_time_and_record_time_lines_of_a_rule_are_not_applied() {
    let module = compile(
        "Limits.RuleTimes",
        "    proposition Exempt()\n    rule Exemption : constitutive {\n        when true\n        then establish Exempt()\n            valid_time [2026-08-14, +inf)\n            record_time [2026-08-14T00:00:00-04:00, +inf)\n    }\n    query exempt() -> Bool { goal Evaluate { operative Exempt() } }",
    );
    // The first time lies inside both intervals. The other two lie before
    // them, and the consequence holds there all the same.
    for at in [
        "2026-09-01T00:00:00Z",
        "2026-08-01T00:00:00Z",
        "2020-01-01T00:00:00Z",
    ] {
        let at = Instant::parse(at).expect("timestamp");
        let case = CaseRecord::default();
        let state = case.into_state();
        let mut handler = CaseFile {
            record: case.clone(),
            known_at: Some(at),
        };
        let outcome = evaluate(
            &module,
            &QueryName::from("exempt"),
            &BTreeMap::new(),
            &state,
            &RunContext::new(at, at),
            &mut handler,
            &case,
        )
        .expect("the query runs");
        assert!(
            matches!(
                outcome,
                Outcome::Determinate {
                    value: Value::Bool(true),
                    ..
                }
            ),
            "{at:?}: {outcome:?}"
        );
    }
}

#[test]
fn a_comment_at_the_end_of_a_line_inside_an_effect_is_read_into_its_result_type() {
    let reviewed = |operation: &str| {
        format!(
            "    effect Review {{\n{operation}\n    }}\n    query q() -> Bool ! {{Review}} {{\n        require Review(\"x\")\n        return true\n    }}"
        )
    };
    // On a line of its own the comment changes nothing.
    compile(
        "Limits.EffectComment",
        &reviewed("        // asks a clerk\n        request(name: String) -> Bool"),
    );
    let diagnostics: Vec<(DiagnosticCode, String)> = compile_source(
        &source(
            "Limits.EffectComment",
            &reviewed("        request(name: String) -> Bool // asks a clerk"),
        ),
        &SourceManifest::default(),
    )
    .expect_err("limit program is rejected")
    .into_iter()
    .map(|diagnostic| (diagnostic.code, diagnostic.message))
    .collect();
    assert_eq!(
        diagnostics,
        [(
            DiagnosticCode::E210,
            "require in `q` has type Nominal(\"Bool // asks a clerk\") but must be Bool".to_owned()
        )]
    );
}

#[test]
fn a_claim_inside_the_brackets_of_a_field_that_is_not_read_is_not_read() {
    let module = compile(
        "Limits.VerifyBrackets",
        "    verify Braces {\n        interpretations {\n            assert false\n        }\n        assert true\n    }\n    verify Parentheses {\n        assuming (fair\n            for every x in {1, 2}: x == 3\n        )\n        assert true\n    }\n    verify BoundsBraces {\n        bounds { assert false }\n        assert true\n    }\n    verify BoundsParentheses {\n        bounds persons 2 (assert false)\n        assert true\n    }",
    );
    for name in ["Braces", "Parentheses", "BoundsBraces", "BoundsParentheses"] {
        let verdict = verify_property(&module, name);
        assert!(verdict.is_proved(), "{name}: {verdict}");
    }
}

#[test]
fn an_operand_that_is_not_needed_is_not_examined() {
    let module = compile(
        "Limits.VerifyOperand",
        "    verify UndeclaredName {\n        assert true or no_such_name\n        assert no_such_name or true\n        assert not (false and no_such_name)\n        assert if true { true } else { no_such_name }\n    }\n    verify LineBelow {\n        assert true or 1 +\n        for_all x in {1, 2}: x == 3\n    }\n    verify MisspelledLineBelow {\n        assert true or 1 +\n        asert false\n    }\n    verify Parentheses {\n        assert true or f(\n            assert false\n        )\n    }\n    verify BelowABlankLine {\n        assert true\n\n        or asert false\n    }\n    verify OperandOfNot {\n        assuming fair ) +\n        assert true or not\n        for_all x in {1, 2}: x == 3\n    }",
    );
    for name in [
        "UndeclaredName",
        "LineBelow",
        "MisspelledLineBelow",
        "Parentheses",
        "BelowABlankLine",
        "OperandOfNot",
    ] {
        let verdict = verify_property(&module, name);
        assert!(verdict.is_proved(), "{name}: {verdict}");
    }
}
