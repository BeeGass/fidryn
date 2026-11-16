//! Text that the interpreter cannot read is rejected where it stands. It is
//! not dropped while the rest of the module passes.
//!
//! Three places dropped such text without a diagnostic: the last statement
//! of a block of statements, a `when`, `require` or `source` of a rule with
//! nothing after its word, and a consequence of a rule whose expression is
//! not a proposition. In each the module passed the checker and answered
//! without the text: a query returned the value of the statement above the
//! one it could not read, a rule that had lost its condition fired, and a
//! rule established less than it said.

use fidryn_cli::compile_source;
use fidryn_core::{DiagnosticCode, SourceManifest};

/// The codes and messages a module with these declarations is rejected with.
fn rejected(declarations: &str) -> Vec<(DiagnosticCode, String)> {
    let source = format!("module Unreadable.Text version \"0.1.0\" {{\n{declarations}\n}}\n");
    compile_source(&source, &SourceManifest::default())
        .expect_err("the module is rejected")
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.message))
        .collect()
}

fn one(code: DiagnosticCode, message: &str) -> Vec<(DiagnosticCode, String)> {
    vec![(code, message.to_owned())]
}

#[test]
fn a_statement_that_cannot_be_read_is_e100_when_it_is_the_last_of_its_block() {
    // `return` with no expression, and `not` with no operand, as the last
    // line of each of the four blocks of statements.
    for last in ["return", "not"] {
        for (declarations, message) in [
            (
                format!("    query q() -> Bool {{\n        true\n        {last}\n    }}"),
                "expected query statement",
            ),
            (
                format!(
                    "    query q() -> Bool {{\n        goal Evaluate {{\n            true\n            {last}\n        }}\n    }}"
                ),
                "expected evaluation statement",
            ),
            (
                format!(
                    "    calc f() -> Bool {{\n        true\n        {last}\n    }}\n    query automatic q() -> Bool {{ return f() }}"
                ),
                "expected function statement",
            ),
            (
                format!(
                    "    query q() -> Bool {{\n        transaction {{\n            true\n            {last}\n        }}\n    }}"
                ),
                "expected transaction step",
            ),
        ] {
            assert_eq!(
                rejected(&declarations),
                one(DiagnosticCode::E100, message),
                "{declarations}"
            );
        }
    }
}

#[test]
fn a_rule_statement_with_nothing_after_its_word_is_e100() {
    // The condition of the `when` is in a comment, so the rule has no
    // guard. It derived `B()` whatever the case held.
    for (statement, message) in [
        ("when // operative A()", "expected a condition after `when`"),
        (
            "require // operative A()",
            "expected a condition after `require`",
        ),
        ("source // \"cite\"", "expected a source after `source`"),
    ] {
        // The statement stands above the consequence, with a `require`
        // between them, or is the last line of the rule.
        for statements in [
            format!("{statement}\n        require true\n        then derive B()"),
            format!("then derive B()\n        {statement}"),
        ] {
            let declarations = format!(
                "    proposition A()\n    proposition B()\n    rule R : derive {{\n        {statements}\n    }}\n    query b() -> Bool {{ goal Evaluate {{ operative B() }} }}"
            );
            assert_eq!(
                rejected(&declarations),
                one(DiagnosticCode::E100, message),
                "{statements}"
            );
        }
    }
}

#[test]
fn a_consequence_that_is_not_a_proposition_is_e210_and_names_the_rule_and_the_consequence() {
    let rule = |consequences: &str| {
        format!(
            "    entity Acme : LegalPerson\n    proposition Exempt(who: LegalPerson)\n    rule Exemption : constitutive {{\n        when true\n        {consequences}\n    }}\n    query q() -> Bool {{ goal Evaluate {{ operative Exempt(Acme) }} }}"
        )
    };
    // A number where the proposition should stand, as a `then` consequence
    // and as an `otherwise` consequence. The lowering reports it.
    assert_eq!(
        rejected(&rule("then establish 5")),
        one(
            DiagnosticCode::E210,
            "`then establish 5` in rule `Exemption` requires a proposition"
        )
    );
    assert_eq!(
        rejected(&rule(
            "then establish Exempt(Acme)\n        otherwise establish \"none\""
        )),
        one(
            DiagnosticCode::E210,
            "`otherwise establish \"none\"` in rule `Exemption` requires a proposition"
        )
    );
    // The name of an entity where the proposition should stand. A name with
    // no arguments has the shape of a proposition, so the checker reports
    // it, by its type.
    for written in ["(Acme)", "Acme"] {
        assert_eq!(
            rejected(&rule(&format!("then establish {written}"))),
            one(
                DiagnosticCode::E210,
                "`establish Acme` in rule `Exemption` requires a proposition, not LegalPerson"
            ),
            "{written}"
        );
    }
    // The same rule with a proposition compiles.
    let source = format!(
        "module Unreadable.Text version \"0.1.0\" {{\n{}\n}}\n",
        rule("then establish Exempt(Acme)")
    );
    compile_source(&source, &SourceManifest::default()).expect("the module compiles");
}
