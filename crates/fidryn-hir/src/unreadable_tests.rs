//! The text reader reads a term whole, or gives up on it.
//!
//! The lowering reads the fields of a `duty` and the `returns` of a
//! `decision` out of text the parser steps over. Inside an argument list,
//! the braces of a set, a record or a block, and a `transaction` block, the
//! reader stepped over a token that begins no item it can read and went on.
//! So a term was read that the text does not say: `content Pay([a, b))`
//! was read as `Pay(a, b)`. The reader now gives up there, and the
//! declaration is rejected with the diagnostic it has for a term the
//! reader cannot read.
//!
//! The reader of a duty also steps over the text of a field it does not
//! know. A word that begins a field of a duty begins one there only
//! outside the parentheses, brackets and braces of that text.

use super::*;
use fidryn_syntax::parse_file;

const DUTY: &str = "duty `Keep` contains an invalid or over-budget attachment/deadline expression";
const DECISION: &str = "decision return expression is invalid or exceeds resource limits";

fn module(declarations: &str) -> String {
    format!(
        "module Readers.Terms version \"0.1.0\" {{\n    entity Acme : LegalPerson\n{declarations}\n}}\n"
    )
}

/// What the lowering makes of a module with these declarations: the
/// lowered module, or the code and the message of each diagnostic.
fn lowered(declarations: &str) -> Result<HirModule, Vec<(DiagnosticCode, String)>> {
    let parsed = parse_file(&module(declarations));
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    elaborate(&parsed, &SourceManifest::default()).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message))
            .collect()
    })
}

fn rejected_with(message: &str) -> Option<Vec<(DiagnosticCode, String)>> {
    Some(vec![(DiagnosticCode::E100, message.to_owned())])
}

/// Terms with an item inside that the reader cannot read: in a call, a
/// set, a record, a block and a `transaction`; first, last, alone and two
/// deep. `[a, b)` is an interval, which the reader does not read.
const UNREADABLE: [&str; 14] = [
    "Pay([a, b))",
    "Pay([a, b), z)",
    "Pay(z, [a, b))",
    "Pay(f([a, b)))",
    "Pay(days)",
    "Pay(z +)",
    "{ [a, b) }",
    "{ z, [a, b) }",
    "{ { [a, b) } }",
    "{ k: [a, b) }",
    "{ k: 1, [a, b) }",
    "if c { [a, b) } else { b }",
    "if c { a } else { z [a, b) }",
    "transaction { [a, b) }",
];

/// A place a term is read from text, with the declarations of a module with
/// the term in the place, and the message the declaration is rejected with
/// when the term cannot be read.
type Place = (&'static str, fn(&str) -> String, &'static str);

/// The places a term is read from text.
const PLACES: [Place; 6] = [
    (
        "bearer",
        |term| format!("    duty Keep {{\n        bearer {term}\n        content a\n    }}"),
        DUTY,
    ),
    (
        "claimant",
        |term| {
            format!(
                "    duty Keep {{\n        bearer Acme\n        claimant {term}\n        content a\n    }}"
            )
        },
        DUTY,
    ),
    (
        "attaches",
        |term| {
            format!(
                "    duty Keep {{\n        bearer Acme\n        attaches when {term}\n        content a\n    }}"
            )
        },
        DUTY,
    ),
    (
        "content",
        |term| format!("    duty Keep {{\n        bearer Acme\n        content {term}\n    }}"),
        DUTY,
    ),
    (
        "due",
        |term| {
            format!(
                "    duty Keep {{\n        bearer Acme\n        content a\n        due {term}\n    }}"
            )
        },
        DUTY,
    ),
    (
        "returns",
        |term| {
            format!(
                "    decision Grant(who: LegalPerson) -> Int {{\n        returns {term}\n    }}"
            )
        },
        DECISION,
    ),
];

#[test]
fn a_term_with_an_item_the_reader_cannot_read_rejects_its_declaration() {
    for (place, declarations, message) in PLACES {
        for term in UNREADABLE {
            assert_eq!(
                lowered(&declarations(term)).err(),
                rejected_with(message),
                "{place} {term}"
            );
        }
    }
}

#[test]
fn the_same_terms_with_a_readable_item_are_read_as_before() {
    // `z` stands where the item that cannot be read stood.
    let content = |term: &str| -> String {
        let (_, declarations, _) = PLACES[3];
        let hir = lowered(&declarations(term)).expect("the duty is lowered");
        format!("{:?}", hir.duties[0].content)
    };
    for (term, read) in [
        ("Pay(z)", r#"[Apply { ctor: "Pay", args: [Ident("z")] }]"#),
        (
            "Pay(z, y)",
            r#"[Apply { ctor: "Pay", args: [Ident("z"), Ident("y")] }]"#,
        ),
        (
            "Pay(f(z))",
            r#"[Apply { ctor: "Pay", args: [Apply { ctor: "f", args: [Ident("z")] }] }]"#,
        ),
        ("{ z }", r#"[Set([Ident("z")])]"#),
        ("{ { z } }", r#"[Set([Set([Ident("z")])])]"#),
        ("{ k: z }", r#"[Record({"k": Ident("z")})]"#),
        (
            "{ k: 1, j: z }",
            r#"[Record({"j": Ident("z"), "k": Int(1)})]"#,
        ),
        (
            "if c { z } else { b }",
            r#"[Apply { ctor: "if", args: [Ident("c"), Ident("z"), Ident("b")] }]"#,
        ),
        (
            "transaction { z }",
            r#"[Apply { ctor: "transaction", args: [Ident("z")] }]"#,
        ),
    ] {
        assert_eq!(content(term), read, "{term}");
    }
    // In the other places too.
    for (place, declarations, _) in PLACES {
        assert!(lowered(&declarations("Pay(z)")).is_ok(), "{place}");
    }
}

// The words that begin the fields of a duty.

/// The content and the deadline of the duty `Keep` with these lines below
/// its `bearer`, as text.
fn duty_read(lines: &str) -> (String, String) {
    let hir = lowered(&format!(
        "    duty Keep {{\n        bearer Acme\n        {lines}\n    }}"
    ))
    .unwrap_or_else(|diagnostics| panic!("{lines}: {diagnostics:?}"));
    (
        format!("{:?}", hir.duties[0].content),
        format!("{:?}", hir.duties[0].due),
    )
}

const PAY: &str = r#"[Apply { ctor: "Pay", args: [Ident("Acme")] }]"#;

#[test]
fn a_field_word_inside_the_brackets_of_an_unknown_field_begins_no_field() {
    // The word is text of the field the reader does not know. Before, the
    // first two gave the duty a second content and a deadline, and the
    // others were rejected.
    for unknown in [
        "discharge_by Delivered(content Pay(Acme))",
        "discharge_by Delivered(due 30 days)",
        "discharge_by Delivered(content)",
        "discharge_by Delivered(due)",
        "discharge_by Delivered(bearer)",
        "discharge_by [content Pay(Acme)]",
        "discharge_by [content]",
        "discharge_by { content Pay(Acme) }",
        "discharge_by { due }",
        "discharge_by Notice(x, f(claimant), attaches)",
    ] {
        assert_eq!(
            duty_read(&format!("content Pay(Acme)\n        {unknown}")),
            (PAY.to_owned(), "None".to_owned()),
            "{unknown}"
        );
    }
}

#[test]
fn a_field_word_outside_brackets_begins_its_field_as_before() {
    // On the line of an unknown field, after its brackets close.
    assert_eq!(
        duty_read("discharge_by Delivered(x) content Pay(Acme)"),
        (PAY.to_owned(), "None".to_owned())
    );
    assert_eq!(
        duty_read("content Pay(Acme)\n        discharge_by Delivered(x) due 30 days"),
        (
            PAY.to_owned(),
            r#"Some(Apply { ctor: "days", args: [Int(30)] })"#.to_owned()
        )
    );
    // Inside the expression of one of the fields, the word is a name.
    assert_eq!(
        duty_read("content Pay(due)"),
        (
            r#"[Apply { ctor: "Pay", args: [Ident("due")] }]"#.to_owned(),
            "None".to_owned()
        )
    );
    let hir = lowered("    duty Keep {\n        bearer Acme\n        claimant Acme(content)\n        content Pay(Acme)\n    }")
        .expect("the duty is lowered");
    assert_eq!(
        format!("{:?}", hir.duties[0].claimant_term),
        r#"Some(Apply { ctor: "Acme", args: [Ident("content")] })"#
    );
    assert_eq!(format!("{:?}", hir.duties[0].content), PAY);
}
