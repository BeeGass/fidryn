//! The text the parser keeps for a declaration or a header value ends with
//! the line of the last token it read. It holds neither the lines below nor
//! a comment at the end of that line.
//!
//! The lowering crate reads the body of some declarations, and the value of
//! a header, from that text, and it finds a body by its braces. A brace
//! inside a comment or a string literal is not one of them, and neither are
//! the braces of an effect row. So a `query` whose header is not followed
//! by a body is rejected as a query with no body, whatever stands beside
//! it, below it or between the lines of its header. A `calc` or `fn` in the
//! same place is rejected by the parser, which requires its body. A brace in
//! a comment or a string inside the body of a `rule` or a `duty` does not
//! end that body. And a comment at the end of a header line or of an
//! `entity` line is not part of the value or the type before it.

use fidryn_cli::compile_source;
use fidryn_core::ir::Guard;
use fidryn_core::{
    CaseRecord, CoreDecl, CoreModule, DiagnosticCode, Instant, Outcome, QueryName, RunContext,
    SourceManifest, Term, Value,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use std::collections::BTreeMap;

const FUNCTION_WITHOUT_A_BODY: [(DiagnosticCode, &str); 1] = [(
    DiagnosticCode::E100,
    "expected `{` to begin the function body",
)];
const QUERY_WITHOUT_A_BODY: [(DiagnosticCode, &str); 1] = [(
    DiagnosticCode::E430,
    "query `Q` is missing a body or explicit goal",
)];

/// A module with the usual header lines, each followed by `after_header`,
/// and these declarations.
fn source(after_header: &str, declarations: &str) -> String {
    format!(
        "module Declarations.Text version \"0.1.0\" {{\n    jurisdiction Test{after_header}\n    source_snapshot \"text\"{after_header}\n    effective_at 2026-01-01{after_header}\n    recorded_at 2026-01-01T00:00:00Z{after_header}\n    outside_scope {{ everything_else }}{after_header}\n{declarations}\n}}\n"
    )
}

fn compile(source: &str) -> CoreModule {
    compile_source(source, &SourceManifest::default()).expect("the module compiles")
}

/// The codes and messages a module with these declarations is rejected with.
fn rejected(declarations: &str) -> Vec<(DiagnosticCode, String)> {
    compile_source(&source("", declarations), &SourceManifest::default())
        .expect_err("the module is rejected")
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.message))
        .collect()
}

fn owned(expected: &[(DiagnosticCode, &str)]) -> Vec<(DiagnosticCode, String)> {
    expected
        .iter()
        .map(|(code, message)| (*code, (*message).to_owned()))
        .collect()
}

/// The outcome of a query against a case record that holds nothing.
fn run(module: &CoreModule, query: &str) -> Outcome<Value> {
    let at = Instant::parse("2026-09-17T12:00:00Z").expect("timestamp");
    let case = CaseRecord::default();
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(at),
    };
    evaluate(
        module,
        &QueryName::from(query),
        &BTreeMap::new(),
        &state,
        &RunContext::new(at, at),
        &mut handler,
        &case,
    )
    .expect("the query runs")
}

#[test]
fn a_function_with_no_body_is_e100_whatever_stands_beside_or_below_it() {
    for keyword in ["calc", "fn"] {
        // After the header stands a query with a body, or a comment that
        // holds braces: on the line of the header, on the next line, or
        // between two blank lines. Or the header goes on with a `fuel`
        // clause, a number, the name of a parameter or an effect row, none
        // of which is a body. Or it has no type.
        for rest in [
            " -> Int\n    query q() -> Int { return 7 }\n",
            " -> Int // { return 7 }\n",
            " -> Int\n    // { return 7 }\n",
            " -> Int\n\n    // { return 7 }\n\n",
            " -> Int fuel = 100\n",
            " -> Int 42\n",
            " -> Int a\n",
            " -> Int ! {a}\n",
            "\n",
        ] {
            let declarations = format!(
                "    {keyword} f(a: Int){rest}    query automatic r() -> Int {{ return f(1) }}"
            );
            assert_eq!(
                rejected(&declarations),
                owned(&FUNCTION_WITHOUT_A_BODY),
                "{keyword} then {rest:?}"
            );
        }
    }
}

#[test]
fn a_comment_between_the_lines_of_a_header_is_not_the_body() {
    // The comment holds braces and stands inside the header, so it is part
    // of the text the parser keeps for the declaration.
    for keyword in ["calc", "fn"] {
        let declarations = format!(
            "    {keyword} f(a: Int)\n    // {{ return 7 }}\n    -> Int\n    query automatic r() -> Int {{ return f(1) }}"
        );
        assert_eq!(
            rejected(&declarations),
            owned(&FUNCTION_WITHOUT_A_BODY),
            "{keyword}"
        );
    }
    assert_eq!(
        rejected("    query Q()\n    // { return 7 }\n    -> Int\n    proposition Witness()"),
        owned(&QUERY_WITHOUT_A_BODY)
    );
}

#[test]
fn the_effect_row_of_a_query_with_no_body_is_not_its_body() {
    // `Acme` is a name the query could return. It stands between the
    // braces of the effect row, which are the only braces of the query.
    for rest in [
        "\n    proposition Witness()",
        " // { return Acme }\n    proposition Witness()",
    ] {
        let declarations =
            format!("    entity Acme : LegalPerson\n    query Q() -> LegalPerson ! {{Acme}}{rest}");
        assert_eq!(
            rejected(&declarations),
            owned(&QUERY_WITHOUT_A_BODY),
            "{rest:?}"
        );
    }
}

#[test]
fn a_query_with_no_body_is_e430_whatever_stands_beside_or_below_it() {
    for rest in [
        "\n    query q() -> Int { return 7 }",
        " // { return 7 }\n    proposition Witness()",
        "\n    // { return 7 }\n    proposition Witness()",
        "\n\n    // { return 7 }\n\n    proposition Witness()",
    ] {
        let declarations = format!("    query Q() -> Int{rest}");
        assert_eq!(
            rejected(&declarations),
            owned(&QUERY_WITHOUT_A_BODY),
            "{rest:?}"
        );
    }
}

#[test]
fn a_brace_in_a_comment_or_a_string_does_not_end_the_body_of_a_rule() {
    // Counted as braces, the comment and the string would each end the body
    // and begin a second one that holds `when false`, and the rule would
    // never fire.
    for consequences in [
        "        then derive Own()\n        // } { when false",
        "        then derive Tagged(\"} { when false\")\n        then derive Own()",
    ] {
        let module = compile(&source(
            "",
            &format!(
                "    proposition Own()\n    proposition Tagged(note: String)\n    rule R : derive {{\n{consequences}\n    }}\n    query own() -> Bool {{ goal Evaluate {{ operative Own() }} }}"
            ),
        ));
        let rule = module
            .declarations
            .iter()
            .find_map(|decl| match decl {
                CoreDecl::Rule(rule) => Some(rule),
                _ => None,
            })
            .expect("rule R");
        assert_eq!(rule.guard, Guard::Satisfied, "{consequences:?}");
        let own = run(&module, "own");
        assert!(
            matches!(
                own,
                Outcome::Determinate {
                    value: Value::Bool(true),
                    ..
                }
            ),
            "{consequences:?}: {own:?}"
        );
    }
}

#[test]
fn a_brace_in_a_comment_or_a_string_does_not_end_the_body_of_a_duty() {
    let duty = |bearer_line: &str, content: &str| {
        let module = compile(&source(
            "",
            &format!(
                "    entity Payer : LegalPerson\n    entity Payee : LegalPerson\n    proposition Issued(who: LegalPerson)\n    duty Pay {{\n        bearer Payer{bearer_line}\n        content {content}\n        claimant Payee\n        attaches when operative Issued(Payer)\n    }}\n    query q() -> Int {{ return 7 }}"
            ),
        ));
        module
            .declarations
            .iter()
            .find_map(|decl| match decl {
                CoreDecl::Duty(duty) => Some((
                    duty.bearer.clone(),
                    duty.claimant.clone(),
                    duty.attaches.clone(),
                    duty.content.clone(),
                )),
                _ => None,
            })
            .expect("duty Pay")
    };
    let plain = duty("", "USD(100.00)");
    assert!(plain.1.is_some(), "{plain:?}");
    assert!(
        matches!(&plain.2, Guard::Operative(prop, _) if prop.predicate == "Issued"),
        "{plain:?}"
    );
    // A `}` in a comment: every field below it is still read.
    assert_eq!(duty(" // } not the end", "USD(100.00)"), plain);
    // A `}` in a string: the fields below the string are still read.
    let (bearer, claimant, attaches, content) = duty("", "\"pay } now\"");
    assert_eq!((bearer, claimant, attaches), (plain.0, plain.1, plain.2));
    assert_eq!(content, [Term::String("pay } now".to_owned())]);
}

#[test]
fn a_comment_at_the_end_of_a_header_line_or_an_entity_line_changes_nothing() {
    let declarations = |comment: &str| {
        format!(
            "    entity Pat : NaturalPerson{comment}\n    proposition Lives(person: NaturalPerson){comment}\n    rule Stays : derive from \"cite\" {{\n        when operative Lives(Pat)\n        then derive Lives(Pat)\n    }}\n    calc twice(a: Int) -> Int{comment}\n    {{\n        return a + a\n    }}\n    query automatic four() -> Int {{ return twice(2) }}"
        )
    };
    let plain = compile(&source("", &declarations("")));
    // The second comment holds what a body, a guard or a string would begin
    // with.
    for comment in [" // a note", " // { when then } ( [ \" return 7"] {
        let commented = compile(&source(comment, &declarations(comment)));
        assert_eq!(commented, plain, "{comment:?}");
    }
}
