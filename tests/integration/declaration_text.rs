//! The text the parser keeps for a declaration or a header value ends with
//! the line of the last token it read. It holds neither the lines below nor
//! a comment at the end of that line.
//!
//! The lowering crate reads the body of some declarations, and the value of
//! a header, from that text. So a `calc`, `fn` or `query` whose header is
//! not followed by a body has none: it takes neither the body of the
//! declaration after it nor one from a comment beside or below it. And a
//! comment at the end of a header line or of an `entity` line is not part of
//! the value or the type before it.

use fidryn_cli::compile_source;
use fidryn_core::{CoreModule, DiagnosticCode, SourceManifest};

const UNTYPED_FUNCTION: &str = "function `f` has an untyped result but is declared to return Int";
const QUERY_WITHOUT_A_BODY: [(DiagnosticCode, &str); 2] = [
    (
        DiagnosticCode::E430,
        "query `Q` is missing a body or explicit goal",
    ),
    (
        DiagnosticCode::E210,
        "query `Q` has an untyped result but is declared to return Int",
    ),
];

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

#[test]
fn a_function_with_no_body_does_not_use_the_body_of_the_declaration_after_it() {
    for keyword in ["calc", "fn"] {
        let declarations = format!(
            "    {keyword} f(a: Int) -> Int\n    query q() -> Int {{ return 7 }}\n    query automatic r() -> Int {{ return f(1) }}"
        );
        assert_eq!(
            rejected(&declarations),
            owned(&[(DiagnosticCode::E210, UNTYPED_FUNCTION)]),
            "{keyword}"
        );
    }
}

#[test]
fn a_comment_beside_or_below_a_function_with_no_body_is_not_its_body() {
    for keyword in ["calc", "fn"] {
        // The comment stands on the line of the header, on the next line, or
        // between two blank lines.
        for comment in [
            " // { return 7 }\n",
            "\n    // { return 7 }\n",
            "\n\n    // { return 7 }\n\n",
        ] {
            let declarations = format!(
                "    {keyword} f(a: Int) -> Int{comment}    query automatic r() -> Int {{ return f(1) }}"
            );
            assert_eq!(
                rejected(&declarations),
                owned(&[(DiagnosticCode::E210, UNTYPED_FUNCTION)]),
                "{keyword} then {comment:?}"
            );
        }
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
