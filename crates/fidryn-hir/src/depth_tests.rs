//! The text readers of the lowering stop on deeply nested text.
//!
//! The parser steps over the body of a `duty`, a `decision` and an
//! `interpretation_family`, and the lowering reads expressions back out of
//! that text. The reader of those expressions goes 128 levels deep, and a
//! declaration that holds a deeper expression is rejected: the reader leaves
//! out what lies below its limit, so what it returns then is not what the
//! text says. The tests here take each kind of nesting through each place
//! the reader is used, up to its limit, just past it, and far past it.
//!
//! The reader of type names has the limit the parser has for a type it
//! reads: 128 levels of `<`.

use super::*;
use fidryn_core::types::{PrimitiveType, Type};
use fidryn_syntax::parse_file;

/// The name at the bottom of every nested expression. A declaration is read
/// whole when what the lowering made of it holds this name.
const LEAF: &str = "LEAF";

/// A depth far past every limit.
const FAR: usize = 5_000;

/// `LEAF` under `n` levels of one kind of nesting.
fn nested(kind: &str, n: usize) -> String {
    match kind {
        "parentheses" => format!("{}{LEAF}{}", "(".repeat(n), ")".repeat(n)),
        "not" => format!("{}{LEAF}", "not ".repeat(n)),
        "calls" => format!("{}{LEAF}{}", "f(".repeat(n), ")".repeat(n)),
        "sets" => format!("{}{LEAF}{}", "{".repeat(n), "}".repeat(n)),
        "records" => format!("{}{LEAF}{}", "{k: ".repeat(n), "}".repeat(n)),
        "if" => format!("{}{LEAF}{}", "if c {".repeat(n), "}".repeat(n)),
        "else" => format!("{}{LEAF}{}", "if c {a} else {".repeat(n), "}".repeat(n)),
        // An `if` with `n` links of `else if`. The last branch is the
        // deepest part of the term.
        "else if" => format!("if c{{a}}{}else{{{LEAF}}}", "else if c{a}".repeat(n)),
        other => panic!("no nesting named {other}"),
    }
}

const KINDS: [&str; 8] = [
    "parentheses",
    "not",
    "calls",
    "sets",
    "records",
    "if",
    "else",
    "else if",
];

/// A place where the lowering reads an expression out of text the parser
/// has stepped over.
struct Place {
    name: &'static str,
    /// The declarations of a module with `expression` in the place.
    declarations: fn(&str) -> String,
    /// What the lowering made of the place, as text.
    read: fn(&HirModule) -> String,
    /// The message the lowering rejects the declaration with.
    message: &'static str,
    /// True when the expression stands inside another: an element of a
    /// set, an argument of a call or a field of a record. The reader has
    /// one level less for it.
    inside: bool,
}

const DUTY: &str = "duty `Keep` contains an invalid or over-budget attachment/deadline expression";
const DECISION: &str = "decision return expression is invalid or exceeds resource limits";
const FAMILY: &str =
    "interpretation family `Reading` contains a definition that exceeds resource limits";

fn duty_read(hir: &HirModule) -> String {
    format!("{:?}", hir.duties[0])
}

fn decision_read(hir: &HirModule) -> String {
    format!("{:?}", hir.decisions[0].result)
}

fn family_read(hir: &HirModule) -> String {
    format!("{:?}", hir.interpretation_families["Reading"])
}

const PLACES: [Place; 10] = [
    Place {
        name: "the bearer of a duty",
        declarations: |expression| {
            format!("    duty Keep {{\n        bearer {expression}\n        content a\n    }}")
        },
        read: duty_read,
        message: DUTY,
        inside: false,
    },
    Place {
        name: "the claimant of a duty",
        declarations: |expression| {
            format!(
                "    duty Keep {{\n        bearer Acme\n        claimant {expression}\n        content a\n    }}"
            )
        },
        read: duty_read,
        message: DUTY,
        inside: false,
    },
    Place {
        name: "the content of a duty",
        declarations: |expression| {
            format!("    duty Keep {{\n        bearer Acme\n        content {expression}\n    }}")
        },
        read: duty_read,
        message: DUTY,
        inside: false,
    },
    Place {
        name: "the attachment of a duty",
        declarations: |expression| {
            format!(
                "    duty Keep {{\n        bearer Acme\n        attaches when {expression}\n        content a\n    }}"
            )
        },
        read: duty_read,
        message: DUTY,
        inside: false,
    },
    Place {
        name: "the deadline of a duty",
        declarations: |expression| {
            format!(
                "    duty Keep {{\n        bearer Acme\n        content a\n        due {expression}\n    }}"
            )
        },
        read: duty_read,
        message: DUTY,
        inside: false,
    },
    Place {
        name: "what a decision returns",
        declarations: |expression| {
            format!(
                "    decision Grant(who: LegalPerson) -> Int {{\n        returns {expression}\n    }}"
            )
        },
        read: decision_read,
        message: DECISION,
        inside: false,
    },
    Place {
        name: "an element of the set a decision returns",
        declarations: |expression| {
            format!(
                "    decision Grant(who: LegalPerson) -> Int {{\n        returns {{first, {expression}}}\n    }}"
            )
        },
        read: decision_read,
        message: DECISION,
        inside: true,
    },
    Place {
        name: "an argument of the call a decision returns",
        declarations: |expression| {
            format!(
                "    decision Grant(who: LegalPerson) -> Int {{\n        returns g(first, {expression})\n    }}"
            )
        },
        read: decision_read,
        message: DECISION,
        inside: true,
    },
    Place {
        name: "a field of the record a decision returns",
        declarations: |expression| {
            format!(
                "    decision Grant(who: LegalPerson) -> Int {{\n        returns {{first: 1, second: {expression}}}\n    }}"
            )
        },
        read: decision_read,
        message: DECISION,
        inside: true,
    },
    Place {
        name: "an argument of what an alternative defines",
        declarations: |expression| {
            format!(
                "    interpretation_family Reading {{\n        alternative Wide {{\n            defines Eligible({expression}) as established\n        }}\n    }}"
            )
        },
        read: family_read,
        message: FAMILY,
        inside: true,
    },
];

/// The first depth at which a place rejects a kind of nesting. Below it the
/// expression is read whole.
///
/// A pair of parentheses, a call, a set, a record and an `if` each take two
/// of the reader's 128 levels, so the 64th is refused. A `not` takes one,
/// and a link of `else if` adds one level to the term, which may be 128
/// deep: a chain of 126 links is read and one of 127 is not.
fn first_depth_rejected(place: &Place, kind: &str) -> usize {
    match (kind, place.inside) {
        ("not", false) | ("else if", false) => 127,
        ("not", true) => 125,
        ("else if", true) => 126,
        (_, false) => 64,
        (_, true) => 63,
    }
}

fn module(declarations: &str) -> String {
    format!("module Readers.Depth version \"0.1.0\" {{\n{declarations}\n}}\n")
}

/// What the lowering makes of a module with these declarations: the
/// lowered module, or the messages of the `E100` diagnostics it is rejected
/// with.
fn lowered(declarations: &str) -> Result<HirModule, Vec<String>> {
    let parsed = parse_file(&module(declarations));
    assert!(
        !parsed.has_errors(),
        "the parser reads the module: {:?}",
        parsed.diagnostics
    );
    elaborate(&parsed, &SourceManifest::default()).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| {
                assert_eq!(diagnostic.code, DiagnosticCode::E100, "{diagnostic:?}");
                diagnostic.message
            })
            .collect()
    })
}

/// Checks one place with one kind of nesting at one depth: read whole below
/// `rejected_from`, rejected with the message of the place from there on.
fn check_depth(place: &Place, kind: &str, depth: usize, rejected_from: usize) {
    let result = lowered(&(place.declarations)(&nested(kind, depth)));
    let at = format!("{} under {depth} levels of {kind}", place.name);
    if depth < rejected_from {
        let hir = result.unwrap_or_else(|messages| panic!("{at} is rejected: {messages:?}"));
        assert!(
            (place.read)(&hir).contains(LEAF),
            "{at} is cut short: what was read does not reach the innermost name"
        );
    } else {
        assert_eq!(
            result.err(),
            Some(vec![place.message.to_owned()]),
            "{at} is not rejected"
        );
    }
}

#[test]
fn an_expression_is_read_whole_or_its_declaration_is_rejected() {
    for place in &PLACES {
        for kind in KINDS {
            let rejected_from = first_depth_rejected(place, kind);
            for depth in (1..=rejected_from + 4).chain([FAR]) {
                check_depth(place, kind, depth, rejected_from);
            }
        }
    }
}

#[test]
fn a_chain_of_else_if_is_read_126_links_long_and_no_longer() {
    // The reader calls itself for each link, and the links are counted. A
    // chain of 30,000 ended the process with a stack overflow before.
    let content = &PLACES[2];
    assert_eq!(content.name, "the content of a duty");
    for links in [1, 126] {
        check_depth(content, "else if", links, 127);
    }
    for links in [127, 128, 129, 130, FAR, 30_000] {
        check_depth(content, "else if", links, 127);
    }
    // The condition of a link is an expression of its own: it does not use
    // up the levels of the chain. A chain whose conditions are each under
    // two `not` is two links shorter, because its term is two levels
    // deeper, and no shorter than that.
    let chain = |links: usize| {
        format!(
            "    duty Keep {{\n        bearer Acme\n        content if not not c{{a}}{}else{{{LEAF}}}\n    }}",
            "else if not not c{a}".repeat(links)
        )
    };
    assert!(duty_read(&lowered(&chain(124)).expect("124 links are read")).contains(LEAF));
    assert_eq!(lowered(&chain(125)).err(), Some(vec![DUTY.to_owned()]));
}

#[test]
fn the_links_of_chains_that_stand_inside_one_another_are_counted_together() {
    // Sixty chains of 120 links, each in the last branch of the one around
    // it. No chain is longer than the reader allows, and together they are
    // 7,200 links deep.
    let mut expression = LEAF.to_owned();
    for _ in 0..60 {
        expression = format!(
            "if c{{a}}{}else{{{expression}}}",
            "else if c{a}".repeat(120)
        );
    }
    for place in &PLACES {
        assert_eq!(
            lowered(&(place.declarations)(&expression)).err(),
            Some(vec![place.message.to_owned()]),
            "{}",
            place.name
        );
    }
}

#[test]
fn an_alternative_whose_definitions_are_read_whole_keeps_them() {
    // The diagnostic of the limit is for a definition the reader gives up
    // on, and for nothing else. A family whose definitions are read whole
    // keeps them all. One with a definition that is no proposition is
    // rejected for that definition, with `E210`, and not for the limit.
    let family = |second: &str| {
        format!(
            "    interpretation_family Reading {{\n        alternative Wide {{\n            defines Eligible(Alice) as established\n{second}            defines Eligible(Bob) as not_established\n        }}\n    }}"
        )
    };
    let parsed = parse_file(&module(&family("            defines 5 as established\n")));
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let rejected: Vec<(DiagnosticCode, String)> = elaborate(&parsed, &SourceManifest::default())
        .expect_err("the family is rejected")
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.message))
        .collect();
    assert_eq!(
        rejected,
        [(
            DiagnosticCode::E210,
            "`defines 5 as established` in interpretation family `Reading` requires a proposition"
                .to_owned()
        )]
    );
    let hir = lowered(&family("")).expect("the family is lowered");
    let alternatives = &hir.interpretation_families["Reading"];
    assert_eq!(alternatives.len(), 1);
    assert_eq!(alternatives[0].0, "Wide");
    assert_eq!(
        alternatives[0]
            .1
            .iter()
            .map(|(proposition, established)| (proposition.predicate.as_str(), *established))
            .collect::<Vec<_>>(),
        [("Eligible", true), ("Eligible", false)]
    );
}

// The reader of type names.

/// `Int` under `n` levels of `ctor<...>`.
fn nested_type(ctor: &str, n: usize) -> String {
    format!("{}Int{}", format!("{ctor}<").repeat(n), ">".repeat(n))
}

/// How many levels of arguments a type holds.
fn levels(ty: &Type) -> usize {
    let mut levels = 0;
    let mut ty = ty;
    loop {
        ty = match ty {
            Type::Primitive(PrimitiveType::Option { inner }) => inner,
            Type::Applied { args, .. } if args.len() == 1 => &args[0],
            _ => return levels,
        };
        levels += 1;
    }
}

#[test]
fn a_type_name_is_read_128_levels_deep_and_no_deeper() {
    for ctor in ["Option", "Box"] {
        // 128 levels are the most the parser lets a type have.
        for depth in [1, 127, 128] {
            let text = nested_type(ctor, depth);
            assert!(!type_nests_too_deep(&text), "{ctor} {depth}");
            assert_eq!(levels(&parse_type_name(&text)), depth, "{ctor} {depth}");
        }
        // One of 5,000 ended the process with a stack overflow before.
        for depth in [129, 130, FAR] {
            let text = nested_type(ctor, depth);
            assert!(type_nests_too_deep(&text), "{ctor} {depth}");
            // The reader stops. What it returns is not the type the text
            // names, which is why a caller asks first.
            assert_eq!(levels(&parse_type_name(&text)), 0, "{ctor} {depth}");
        }
    }
    let texts: [fn(usize) -> String; 4] = [
        // The levels are those of the deepest argument, wherever it
        // stands.
        |depth| {
            format!(
                "Map<String, {}> | Pair<Int, Int>",
                nested_type("Box", depth - 1)
            )
        },
        |depth| format!("Pair<Int, {}>", nested_type("Option", depth - 1)),
        // They are counted in the text, as the parser counts them: also
        // below `Money`, whose argument the reader takes for a name, and
        // in a text whose `<` nothing closes.
        |depth| format!("Money<{}>", nested_type("Box", depth - 1)),
        |depth| format!("{}Int", "Box<".repeat(depth)),
    ];
    for text in texts {
        assert!(!type_nests_too_deep(&text(128)), "{}", text(3));
        assert!(type_nests_too_deep(&text(129)), "{}", text(3));
        assert!(type_nests_too_deep(&text(FAR)), "{}", text(3));
    }
    // A text the reader would go down into 200 times, though no two of
    // its `<` are open at once.
    let chained = format!("{}{}", "A<b>".repeat(200), ">".repeat(199));
    assert!(type_nests_too_deep(&chained));
    let chained = format!("{}{}", "A<b>".repeat(100), ">".repeat(99));
    assert!(!type_nests_too_deep(&chained));
}
