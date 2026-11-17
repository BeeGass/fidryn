//! Deeply nested text that only the lowering reads is rejected with a
//! diagnostic.
//!
//! The parser steps over the body of a `duty`, a `decision`, an
//! `interpretation_family` and an `effect`. The lowering and the checker
//! read expressions and types back out of that text, and their readers go
//! 128 levels deep. `fidryn check` on a module with deeper text there ended
//! with a stack overflow in two cases, and in two others it passed with the
//! deep part left out. Each module here is made in the test.

use fidryn_cli::compile_source;
use fidryn_core::{DiagnosticCode, SourceManifest};

/// The codes and messages `fidryn check` gives for a module with these
/// declarations, or nothing when the module passes.
fn diagnostics(declarations: &str) -> Vec<(DiagnosticCode, String)> {
    let source = format!(
        "module Probe.Deep version \"0.1.0\" {{\n    entity Acme : LegalPerson\n    proposition Formed(who: LegalPerson)\n{declarations}\n    proposition Witness()\n}}\n"
    );
    match compile_source(&source, &SourceManifest::default()) {
        Ok(_) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message))
            .collect(),
    }
}

/// What a module that passes gives.
const NONE: Vec<(DiagnosticCode, String)> = Vec::new();

fn e100(message: &str) -> Vec<(DiagnosticCode, String)> {
    vec![(DiagnosticCode::E100, message.to_owned())]
}

/// A duty whose content is an `if` with this many links of `else if`.
fn duty_with_a_chain(links: usize) -> String {
    format!(
        "    duty Keep {{\n        bearer Acme\n        attaches when operative Formed(Acme)\n        content if c{{a}}{}else{{b}}\n    }}",
        "else if c{a}".repeat(links)
    )
}

/// An effect whose operation has a result type nested this deep.
fn effect_with_a_result(depth: usize) -> String {
    format!(
        "    effect Ask {{ request(n: String) -> {}Int{} }}",
        "Option<".repeat(depth),
        ">".repeat(depth)
    )
}

#[test]
fn a_duty_whose_content_is_a_chain_of_30_000_else_if_links_is_rejected() {
    let rejected =
        e100("duty `Keep` contains an invalid or over-budget attachment/deadline expression");
    // The longest chain that is read has 126 links.
    assert_eq!(diagnostics(&duty_with_a_chain(126)), NONE);
    assert_eq!(diagnostics(&duty_with_a_chain(127)), rejected);
    assert_eq!(diagnostics(&duty_with_a_chain(1_000)), rejected);
    assert_eq!(diagnostics(&duty_with_a_chain(30_000)), rejected);
}

#[test]
fn an_effect_whose_result_type_nests_5_000_deep_is_rejected() {
    let rejected = e100("type nesting exceeds the limit of 128");
    // 128 levels are what the parser lets a type it reads have.
    assert_eq!(diagnostics(&effect_with_a_result(128)), NONE);
    assert_eq!(diagnostics(&effect_with_a_result(129)), rejected);
    assert_eq!(diagnostics(&effect_with_a_result(200)), rejected);
    assert_eq!(diagnostics(&effect_with_a_result(5_000)), rejected);
}

/// `Int` under `levels` levels of `Box<...>`.
fn boxes(levels: usize) -> String {
    format!("{}Int{}", "Box<".repeat(levels), ">".repeat(levels))
}

#[test]
fn an_effect_type_that_its_reader_cuts_short_is_counted_whole() {
    let rejected = e100("type nesting exceeds the limit of 128");
    // A parameter whose type has two arguments: the parameter list is
    // split at the comma inside `Map<...>`, and the deep part was left out.
    let parameter = |levels: usize| {
        format!(
            "    effect Ask {{\n        request(n: String, m: Map<String, {}>) -> Bool\n    }}",
            boxes(levels - 1)
        )
    };
    // A result over two lines: the result is cut at the end of its first
    // line, and the second was left out.
    let result = |levels: usize| {
        format!(
            "    effect Ask {{\n        request(n: String) -> Box<\n            {}>\n    }}",
            boxes(levels - 1)
        )
    };
    let forms: [fn(usize) -> String; 2] = [parameter, result];
    for declarations in forms {
        assert_eq!(diagnostics(&declarations(128)), NONE);
        assert_eq!(diagnostics(&declarations(129)), rejected);
        assert_eq!(diagnostics(&declarations(5_000)), rejected);
    }
}

#[test]
fn a_duty_whose_content_cannot_be_read_is_rejected() {
    let rejected =
        e100("duty `Keep` contains an invalid or over-budget attachment/deadline expression");
    let duty = |content: &str| {
        format!("    duty Keep {{\n        bearer Acme\n        content {content}\n    }}")
    };
    // The content was dropped, and the duty passed with none.
    assert_eq!(diagnostics(&duty("[a, b)")), rejected);
    // Inside a call or a set the interval was stepped over, and the duty
    // passed with the content `Pay(a, b)`, or a set of `a` and `b`.
    assert_eq!(diagnostics(&duty("Pay([a, b))")), rejected);
    assert_eq!(diagnostics(&duty("{ [a, b) }")), rejected);
    assert_eq!(
        diagnostics(&duty(&format!(
            "{}a{}",
            "[".repeat(5_000),
            "]".repeat(5_000)
        ))),
        rejected
    );
    assert_eq!(diagnostics(&duty("Pay(Acme)")), NONE);
}

#[test]
fn a_decision_that_returns_a_set_with_an_element_nested_too_deep_is_rejected() {
    let returns = |depth: usize| {
        format!(
            "    decision Grant(who: LegalPerson) -> Int {{\n        returns {{first, {}second{}}}\n    }}",
            "(".repeat(depth),
            ")".repeat(depth)
        )
    };
    assert_eq!(diagnostics(&returns(62)), NONE);
    // The module passed before, and the decision returned the set without
    // its second element.
    let rejected = e100("decision return expression is invalid or exceeds resource limits");
    assert_eq!(diagnostics(&returns(63)), rejected);
    assert_eq!(diagnostics(&returns(5_000)), rejected);
}

#[test]
fn a_family_with_a_definition_nested_too_deep_is_rejected() {
    let family = |depth: usize| {
        format!(
            "    proposition Eligible(who: LegalPerson)\n    interpretation_family Reading {{\n        alternative Wide {{\n            defines Eligible({}Acme{}) as established\n        }}\n    }}",
            "(".repeat(depth),
            ")".repeat(depth)
        )
    };
    assert_eq!(diagnostics(&family(62)), NONE);
    // The module passed before, and the alternative had lost its
    // definition.
    let rejected =
        e100("interpretation family `Reading` contains a definition that exceeds resource limits");
    assert_eq!(diagnostics(&family(63)), rejected);
    assert_eq!(diagnostics(&family(5_000)), rejected);
}
