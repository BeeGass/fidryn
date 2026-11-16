//! Text that the interpreter cannot read is rejected where it stands. It is
//! not dropped while the rest of the module passes.
//!
//! Four places dropped such text without a diagnostic: the last statement
//! of a block of statements, a `when`, `require` or `source` of a rule with
//! nothing after its word, a consequence of a rule whose expression is not
//! a proposition, and the header of a declaration that is not finished at
//! the end of its line. In each the module passed the checker and answered
//! without the text: a query returned the value of the statement above the
//! one it could not read, a rule that had lost its condition fired, a rule
//! established less than it said, and a header took the declaration below
//! it as its own text, so that the declaration was gone.

use fidryn_cli::{CaseInput, RunRequest, compile_source, run_report_text};
use fidryn_core::{DiagnosticCode, SourceManifest};
use fidryn_syntax::ast::Item;
use fidryn_syntax::parse_file;
use std::sync::atomic::{AtomicU64, Ordering};

const VERBS: [&str; 9] = [
    "establish",
    "constitute",
    "terminate",
    "suspend",
    "create",
    "derive",
    "activate",
    "defeat",
    "make",
];

fn module(declarations: &str) -> String {
    format!("module Unreadable.Text version \"0.1.0\" {{\n{declarations}\n}}\n")
}

/// What `fidryn run` writes to stderr when it does not answer `query` of a
/// module with these declarations.
fn run_is_refused(declarations: &str, query: &str) -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "fidryn-unreadable-text-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(&directory).expect("a temporary directory");
    let path = directory.join("module.fr");
    std::fs::write(&path, module(declarations)).expect("the module file");
    let at = "2026-09-17T12:00:00Z";
    let answer = run_report_text(&RunRequest {
        path: &path,
        query,
        case: CaseInput::Json(
            r#"{"schema":"fidryn.case-record/v0.1","facts":{},"admissibleCompletions":{}}"#,
        ),
        valid_at: at,
        known_at: at,
        args: &[],
        scenario: false,
    });
    let _ = std::fs::remove_dir_all(&directory);
    answer.expect_err("the query is not answered")
}

/// The codes and messages a module with these declarations is rejected with.
fn rejected(declarations: &str) -> Vec<(DiagnosticCode, String)> {
    compile_source(&module(declarations), &SourceManifest::default())
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
    assert_eq!(
        rejected(&rule("then establish (Acme)")),
        one(
            DiagnosticCode::E210,
            "`establish Acme` in rule `Exemption` requires a proposition, not LegalPerson"
        )
    );
    // That holds after each of the nine verbs, `create` among them, in the
    // `then` list and in the `otherwise` list.
    for verb in VERBS {
        for consequences in [
            format!("then {verb} Acme"),
            format!("then establish Exempt(Acme)\n        otherwise {verb} Acme"),
        ] {
            assert_eq!(
                rejected(&rule(&consequences)),
                one(
                    DiagnosticCode::E210,
                    &format!(
                        "`{verb} Acme` in rule `Exemption` requires a proposition, not LegalPerson"
                    )
                ),
                "{consequences}"
            );
        }
    }
    // The same rule with a proposition compiles.
    compile_source(
        &module(&rule("then establish Exempt(Acme)")),
        &SourceManifest::default(),
    )
    .expect("the module compiles");
}

#[test]
fn a_consequence_is_checked_alike_after_each_of_the_nine_verbs() {
    let consequence = |consequences: &str| {
        format!(
            "    type Color = Red | Green\n    type Truth = Prop\n    entity Acme : LegalPerson\n    entity Pr : Prop\n    entity Al : Truth\n    office Chair\n    office Seat(n: Int)\n    record_type Paper {{\n        a: Int\n    }}\n    effect reads {{ request(a: Int) -> Bool }}\n    calc seven() -> Int {{ return 7 }}\n    fn eight() -> Int {{ return 8 }}\n    query nine() -> Int {{ return 9 }}\n    proposition Exempt(who: LegalPerson)\n    proposition Flag()\n    rule Exemption(who: LegalPerson) : constitutive {{\n        when true\n        {consequences}\n    }}\n    query q() -> Bool {{ goal Evaluate {{ operative Exempt(Acme) }} }}"
        )
    };
    // The consequence stands in the `then` list or in the `otherwise` list.
    let both_lists = |verb: &str, form: &str| {
        [
            consequence(&format!("then {verb} {form}")),
            consequence(&format!(
                "then establish Exempt(Acme)\n        otherwise {verb} {form}"
            )),
        ]
    };
    // What a consequence can be: a declared proposition with its arguments,
    // a name of type `Prop`, which an entity can have, directly or through
    // an alias, and a capitalized name with no arguments that the module
    // does not declare.
    let accepted = [
        "Exempt(Acme)",
        "Flag",
        "Flag()",
        "Pr",
        "Pr()",
        "Al",
        "Al()",
        "Unknown",
        "Unknown()",
    ];
    // Anything else is `E210` after each of the nine verbs: an expression
    // that is no name, written with or without parentheses, and a name that
    // is no proposition, alone, with `()` and with arguments.
    let expressions = [
        "5",
        "2.5",
        "\"none\"",
        "true",
        "1 + 2",
        "1 < 2",
        "{1, 2}",
        "{ a: 1 }",
        "2026-01-01",
        "2026-01-01T00:00:00Z",
        "2026-01-01T09:30:00-05:00",
        "5 days",
        "USD(500.00)",
        "[1, 2]",
        "[1, 2)",
        "[2026-08-14, +inf)",
        "for_all x in {1, 2} : x > 0",
        "exists x in {1, 2} : x > 0",
        "_",
        "+inf",
        "-inf",
        "if true { Flag() } else { Flag() }",
        "who.name",
        "\"a\"(1)",
        "operative Exempt(Acme)",
    ];
    let names = [
        "Exempt",
        "Exempt()",
        "Exempt(5)",
        "Exempt(Acme, Acme)",
        "Flag(1)",
        "Acme",
        "Acme()",
        "Acme(1)",
        "Pr(1)",
        "Al(1)",
        "Chair",
        "Chair()",
        "Chair(1)",
        "Seat",
        "Seat()",
        "Seat(1)",
        "reads",
        "reads()",
        "reads(1)",
        "Red",
        "Red()",
        "Red(1)",
        "who",
        "who()",
        "who(1)",
        "Paper",
        "Paper()",
        "Paper(1)",
        "seven",
        "seven()",
        "seven(1)",
        "eight",
        "eight()",
        "eight(1)",
        "nine",
        "nine()",
        "nine(1)",
        "Unknown(Acme)",
        "unknown",
        "unknown()",
        "unknown(1)",
    ];
    let refused: Vec<String> = expressions
        .iter()
        .flat_map(|expression| [(*expression).to_owned(), format!("({expression})")])
        .chain(names.iter().map(|name| (*name).to_owned()))
        .collect();
    for verb in VERBS {
        for form in accepted {
            for declarations in both_lists(verb, form) {
                if let Err(diagnostics) =
                    compile_source(&module(&declarations), &SourceManifest::default())
                {
                    panic!("{verb} {form}: {diagnostics:?}");
                }
            }
        }
        for form in &refused {
            for declarations in both_lists(verb, form) {
                let diagnostics = rejected(&declarations);
                assert!(
                    diagnostics
                        .iter()
                        .all(|(code, _)| *code == DiagnosticCode::E210),
                    "{verb} {form}: {diagnostics:?}"
                );
            }
        }
    }
    // `Duty` is the one exception, and only after `create`: with its three
    // fields it is a consequence, and without them it is told what it
    // lacks. After the other eight verbs `Duty` is a name like any other
    // that the module does not declare.
    let duty = "Duty(bearer = Acme, claimant = Acme, content = Exempt(Acme))";
    let lacking: Vec<(DiagnosticCode, String)> = [
        "Duty creation requires `bearer`",
        "Duty creation requires `claimant`",
        "Duty creation requires content",
    ]
    .map(|message| (DiagnosticCode::E210, message.to_owned()))
    .to_vec();
    for verb in VERBS {
        for (form, passes) in [
            (duty, verb == "create"),
            ("Duty", verb != "create"),
            ("Duty()", verb != "create"),
            ("Duty(1)", false),
        ] {
            for declarations in both_lists(verb, form) {
                let compiled = compile_source(&module(&declarations), &SourceManifest::default());
                assert_eq!(compiled.is_ok(), passes, "{verb} {form}");
                if verb == "create" && !passes {
                    assert_eq!(rejected(&declarations), lacking, "{verb} {form}");
                }
            }
        }
    }
}

#[test]
fn a_header_that_is_not_finished_does_not_take_the_declaration_below_it() {
    // The basis of the rule ends in `+`, so its line goes on into the line
    // below it. That line begins a declaration of its own. The parser read
    // `clause C` as the operand and the block of the clause as the body of
    // the rule: the module passed, the rule established `Witness()`, and
    // `q` answered true from a rule the text gives no body.
    let clause = "clause C {\n        then establish Witness()\n    }";
    let rule_above_a_clause = format!(
        "    proposition Witness()\n    rule R : derive from Basis +\n    {clause}\n    query q() -> Bool {{ goal Evaluate {{ operative Witness() }} }}"
    );
    assert_eq!(
        rejected(&rule_above_a_clause),
        [
            (
                DiagnosticCode::E100,
                "expected expression after operator".to_owned()
            ),
            (
                DiagnosticCode::E100,
                "expected `{` to begin the rule body".to_owned()
            ),
        ]
    );
    // The clause is a declaration of the module, with its own block.
    let parsed = parse_file(&module(&rule_above_a_clause));
    let declarations: Vec<(&str, Option<&str>, &str)> = parsed
        .module()
        .expect("a module")
        .items
        .iter()
        .filter_map(Item::decl)
        .map(|decl| {
            (
                decl.keyword.as_str(),
                decl.name.as_deref(),
                decl.source.as_str(),
            )
        })
        .collect();
    assert_eq!(declarations.len(), 4, "{declarations:?}");
    assert_eq!(
        declarations[1],
        ("rule", Some("R"), "rule R : derive from Basis +\n")
    );
    assert_eq!(declarations[2], ("clause", Some("C"), clause));
    // And the query is not answered.
    let refused = run_is_refused(&rule_above_a_clause, "q");
    assert!(refused.contains("E100"), "{refused}");

    // The source of an observation ends in `+` above a `verify` block. The
    // block became the body of the observation, the module passed, and the
    // property `V`, which has a counterexample, no longer existed.
    let observation_above_a_verify_block = "    proposition P()\n    record_type Rec {\n        a: Int\n    }\n    observation O(r: Rec) establishes P() from Src +\n    verify V {\n        assert false\n    }\n    query q() -> Bool { goal Evaluate { operative P() } }";
    assert_eq!(
        rejected(observation_above_a_verify_block),
        [
            (
                DiagnosticCode::E100,
                "expected expression after operator".to_owned()
            ),
            (DiagnosticCode::E100, "expected `{`".to_owned()),
        ]
    );
    let parsed = parse_file(&module(observation_above_a_verify_block));
    let property = parsed
        .module()
        .expect("a module")
        .items
        .iter()
        .filter_map(Item::decl)
        .find(|decl| decl.keyword == "verify")
        .expect("the verify block is a declaration of the module");
    assert_eq!(property.name.as_deref(), Some("V"));
    assert_eq!(property.assertions.len(), 1);
    let refused = run_is_refused(observation_above_a_verify_block, "q");
    assert!(refused.contains("E100"), "{refused}");
}

#[test]
fn a_bracket_among_type_arguments_is_e100_and_nothing_is_answered() {
    // Type arguments were read as text, a parenthesis among them too. The
    // header of the rule is not finished at the end of its line, and the
    // `(` in the type of its parameter counted as a bracket the header had
    // left open. So the rule took the clause below it for its own text, the
    // module passed the checker, and the clause was gone.
    let rule_above_a_clause = "    proposition Witness()\n    rule R(x: Int<(>) : derive from Basis +\n    clause C {\n        then establish Witness()\n    }\n    query q() -> Bool { goal Evaluate { operative Witness() } }";
    assert_eq!(
        rejected(rule_above_a_clause),
        one(DiagnosticCode::E100, "unexpected bracket in type arguments")
    );
    let refused = run_is_refused(rule_above_a_clause, "q");
    assert!(refused.contains("E100"), "{refused}");

    // The same in the type of the record of an observation above a `verify`
    // block: the module passed, and the property `V`, which has a
    // counterexample, no longer existed.
    let observation_above_a_verify_block = "    proposition P()\n    record_type Rec {\n        a: Int\n    }\n    observation O(r: Rec<(>) establishes P() from Src +\n    verify V {\n        assert false\n    }\n    query q() -> Bool { goal Evaluate { operative P() } }";
    assert_eq!(
        rejected(observation_above_a_verify_block),
        one(DiagnosticCode::E100, "unexpected bracket in type arguments")
    );
    let refused = run_is_refused(observation_above_a_verify_block, "q");
    assert!(refused.contains("E100"), "{refused}");
}
