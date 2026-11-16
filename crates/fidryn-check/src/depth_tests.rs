//! A type nests its arguments 128 levels deep and no deeper, wherever it
//! stands.
//!
//! The parser has that limit for a type it reads. The body of an `effect`,
//! of a `duty` and of the other declarations the parser steps over is text
//! to it, and a type in that text is read later, from the text. Such a type
//! has the same limit, and a module that holds a deeper one is rejected in
//! the parser's words. Before, a type nested a few thousand deep there
//! ended the process with a stack overflow.

use super::*;
use fidryn_hir::elaborate;
use fidryn_syntax::parse_file;

const TOO_DEEP: &str = "type nesting exceeds the limit of 128";

/// `Int` under `n` levels of `Box<...>`.
fn nested_type(n: usize) -> String {
    format!("{}Int{}", "Box<".repeat(n), ">".repeat(n))
}

/// What the parser, the lowering and the checker make of a module with
/// these declarations: the compiled module, or the code and the message of
/// each diagnostic.
fn compiled(declarations: &str) -> Result<CoreModule, Vec<(DiagnosticCode, String)>> {
    let messages = |diagnostics: Vec<Diagnostic>| -> Vec<(DiagnosticCode, String)> {
        diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message))
            .collect()
    };
    let source = format!("module Readers.Depth version \"0.1.0\" {{\n{declarations}\n}}\n");
    let parsed = parse_file(&source);
    if parsed.has_errors() {
        return Err(messages(parsed.diagnostics));
    }
    let hir = elaborate(&parsed, &SourceManifest::default()).map_err(messages)?;
    check(&hir, &SourceManifest::default()).map_err(messages)
}

/// How many levels of arguments a type holds.
fn levels(ty: &Type) -> usize {
    let mut levels = 0;
    let mut ty = ty;
    while let Type::Applied { args, .. } = ty
        && let [inner] = args.as_slice()
    {
        ty = inner;
        levels += 1;
    }
    levels
}

/// A place where a type stands: its name, and the declarations of a module
/// with a type in the place.
type Place = (&'static str, fn(&str) -> String);

/// Places where a type stands in text the parser steps over.
const READ_FROM_TEXT: [Place; 6] = [
    ("the result of an operation of an effect", |ty| {
        format!("    effect Ask {{\n        request(n: String) -> {ty}\n    }}")
    }),
    ("a parameter of an operation of an effect", |ty| {
        format!("    effect Ask {{\n        request(n: String, m: {ty}) -> Bool\n    }}")
    }),
    // A declaration with no parameter list of its own takes the first
    // parenthesized list in its text for one.
    ("the first list in the body of a duty", |ty| {
        format!(
            "    entity Acme : LegalPerson\n    duty Keep {{\n        bearer Acme\n        content pay(amount: {ty})\n    }}"
        )
    }),
    ("the first list below a proposition", |ty| {
        format!("    proposition Seen\n        from basis(what: {ty})")
    }),
    ("the first list in the body of a legal act", |ty| {
        format!("    legal_act Sign {{\n        performer signer(who: {ty})\n    }}")
    }),
    ("the first list in the body of an office", |ty| {
        format!("    office Bench {{\n        competence hear(what: {ty})\n    }}")
    }),
];

#[test]
fn a_type_read_from_text_nests_128_deep_and_no_deeper() {
    for (place, declarations) in READ_FROM_TEXT {
        for depth in [1, 127, 128] {
            let result = compiled(&declarations(&nested_type(depth)));
            assert!(result.is_ok(), "{place}, {depth} deep: {:?}", result.err());
        }
        for depth in [129, 130, 5_000] {
            assert_eq!(
                compiled(&declarations(&nested_type(depth))).err(),
                Some(vec![(DiagnosticCode::E100, TOO_DEEP.to_owned())]),
                "{place}, {depth} deep"
            );
        }
    }
}

#[test]
fn the_levels_of_a_type_are_counted_in_its_text() {
    // The parser counts every `<` of a type it reads. A type read from
    // text is counted in the same way: below `Money`, whose argument is a
    // name and no type, the levels count as they do anywhere else.
    let (_, result) = READ_FROM_TEXT[0];
    let money = |depth: usize| format!("Money<{}>", nested_type(depth - 1));
    assert!(compiled(&result(&money(128))).is_ok());
    assert_eq!(
        compiled(&result(&money(129))).err(),
        Some(vec![(DiagnosticCode::E100, TOO_DEEP.to_owned())])
    );
    let by_the_parser =
        compiled(&format!("    entity Deep : {}", money(129))).expect_err("the module is rejected");
    assert!(
        by_the_parser.contains(&(DiagnosticCode::E100, TOO_DEEP.to_owned())),
        "{by_the_parser:?}"
    );
    assert!(compiled(&format!("    entity Deep : {}", money(128))).is_ok());
}

#[test]
fn a_type_of_128_levels_in_an_effect_is_read_whole() {
    let (_, result) = READ_FROM_TEXT[0];
    let module = compiled(&result(&nested_type(128))).expect("the module compiles");
    let operation = module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            CoreDecl::EffectDecl(effect) => Some(&effect.operations[0]),
            _ => None,
        })
        .expect("an effect");
    assert_eq!(levels(&operation.result), 128);
    let (_, parameter) = READ_FROM_TEXT[1];
    let module = compiled(&parameter(&nested_type(128))).expect("the module compiles");
    let operation = module
        .declarations
        .iter()
        .find_map(|declaration| match declaration {
            CoreDecl::EffectDecl(effect) => Some(&effect.operations[0]),
            _ => None,
        })
        .expect("an effect");
    assert_eq!(operation.params.len(), 2);
    assert_eq!(levels(&operation.params[1].1), 128);
}

#[test]
fn a_type_the_parser_reads_has_the_same_limit_and_the_same_words() {
    let read_by_the_parser: [Place; 3] = [
        ("the type of an entity", |ty| {
            format!("    entity Deep : {ty}")
        }),
        ("the type of a parameter of a calc", |ty| {
            format!("    calc f(x: {ty}) -> Int {{ return 1 }}")
        }),
        ("the type of a field of a record", |ty| {
            format!("    record_type Deep {{\n        field: {ty}\n    }}")
        }),
    ];
    for (place, declarations) in read_by_the_parser {
        let result = compiled(&declarations(&nested_type(128)));
        assert!(result.is_ok(), "{place}, 128 deep: {:?}", result.err());
        for depth in [129, 5_000] {
            let diagnostics =
                compiled(&declarations(&nested_type(depth))).expect_err("the module is rejected");
            assert!(
                diagnostics.contains(&(DiagnosticCode::E100, TOO_DEEP.to_owned())),
                "{place}, {depth} deep: {diagnostics:?}"
            );
        }
    }
}
