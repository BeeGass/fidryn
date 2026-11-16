//! A comment means what the same amount of white space means, and the
//! inside of a string literal is no structure.
//!
//! The lowering hands the text of an `effect` and of a `conflict_doctrine`
//! on as it is written, and the checker reads the operation of the effect
//! and the clauses the doctrine defeats out of that text. Each test here
//! puts a comment, or a string, in the place one of those readers looks,
//! but one, which puts the word `defeat` inside a longer name.

use super::*;
use fidryn_hir::elaborate;
use fidryn_syntax::parse_file;

/// A comment that holds what a reader of raw text trips over.
const TRAP: &str = "// (x: Wrong) -> Wrong { wrong(y: Wrong) } \" , : defeat Residual";

/// The comments each template is tried with.
const COMMENTS: [&str; 2] = ["// a note", TRAP];

fn module(declarations: &str) -> String {
    format!("module Readers.Text version \"0.1.0\" {{\n{declarations}\n}}\n")
}

fn checked(declarations: &str) -> Result<CoreModule, Vec<(DiagnosticCode, String)>> {
    let parsed = parse_file(&module(declarations));
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let hir = elaborate(&parsed, &SourceManifest::default()).expect("the module is lowered");
    check(&hir, &SourceManifest::default()).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message))
            .collect()
    })
}

/// `template` with each `{c}` replaced by `comment`, and its twin with each
/// `{c}` replaced by as many spaces.
fn twins(template: &str, comment: &str) -> (String, String) {
    (
        template.replace("{c}", comment),
        template.replace("{c}", &" ".repeat(comment.len())),
    )
}

fn string() -> Type {
    Type::Primitive(PrimitiveType::String)
}

fn time() -> Type {
    Type::Primitive(PrimitiveType::Time)
}

fn params(list: &[(&str, Type)]) -> Vec<(String, Type)> {
    list.iter()
        .map(|(name, ty)| ((*name).to_owned(), ty.clone()))
        .collect()
}

fn effect_declaration(module: &CoreModule) -> &CoreEffectDecl {
    module
        .declarations
        .iter()
        .find_map(|decl| match decl {
            CoreDecl::EffectDecl(effect) => Some(effect),
            _ => None,
        })
        .expect("an effect")
}

#[test]
fn the_parameters_of_an_operation_are_not_read_from_a_comment() {
    let template = "effect Review {c}\n{\n    {c}\n    request(name: String, {c}\n        at: Time) -> Bool {c}\n}";
    for comment in COMMENTS {
        let (commented, twin) = twins(template, comment);
        let expected = params(&[("name", string()), ("at", time())]);
        assert_eq!(source_typed_params(&twin), expected, "{comment:?}");
        assert_eq!(source_typed_params(&commented), expected, "{comment:?}");
    }
}

#[test]
fn a_bracket_or_a_separator_inside_a_string_does_not_move_the_parameters_of_an_operation() {
    // The citation holds a parenthesis that nothing closes.
    assert_eq!(
        source_typed_params(
            "effect Review from \"Stat. § 12(a\" {\n    request(name: String) -> Bool\n}"
        ),
        params(&[("name", string())])
    );
    // A parenthesis, a comma and a colon inside a string, in the list.
    assert_eq!(
        source_typed_params(
            "effect Review {\n    request(doc: Tagged<\"a) , b: c\">, at: Time) -> Bool\n}"
        ),
        params(&[
            (
                "doc",
                Type::Applied {
                    ctor: "Tagged".into(),
                    args: vec![Type::Sort(Sort::Nominal("\"a) , b: c\"".into()))],
                }
            ),
            ("at", time()),
        ])
    );
}

#[test]
fn the_operation_of_an_effect_is_named_by_the_first_word_of_its_block_not_by_a_comment() {
    let template = "effect Review {c}\n{\n    {c}\n    lookup(name: String) -> Bool\n}";
    for comment in COMMENTS {
        let (commented, twin) = twins(template, comment);
        assert_eq!(effect_op_name(&twin), "lookup", "{comment:?}");
        assert_eq!(effect_op_name(&commented), "lookup", "{comment:?}");
    }
    // With no word there the name is `request`, as before.
    assert_eq!(
        effect_op_name("effect Review {\n    // nothing\n}"),
        "request"
    );
    assert_eq!(effect_op_name("effect Review { \"lookup\" }"), "request");
}

#[test]
fn a_clause_is_defeated_by_the_word_in_the_text_not_by_one_in_a_comment_or_a_string() {
    let clauses = BTreeMap::from([("Residual".to_owned(), String::new())]);
    for (doctrine, expected) in [
        (
            "conflict_doctrine D {\n    then defeat Residual as_to Bar\n}",
            vec!["Residual"],
        ),
        (
            "conflict_doctrine D {\n    // defeat Residual\n    when true\n}",
            vec![],
        ),
        (
            "conflict_doctrine D {\n    note \"defeat Residual\"\n}",
            vec![],
        ),
        (
            "conflict_doctrine D {\n    note \"defeat Nothing\" // defeat Nothing\n    then defeat Residual\n}",
            vec!["Residual"],
        ),
    ] {
        assert_eq!(
            expand_conflict_clause_ids(doctrine, &clauses),
            expected,
            "{doctrine}"
        );
    }
}

#[test]
fn a_clause_is_defeated_by_the_word_defeat_and_not_by_a_name_that_holds_it() {
    // The word is found only where no letter, digit, `_` or `.` stands
    // beside it. After the `.` of a dotted name it is part of that name,
    // and it named the clause after it.
    let clauses = BTreeMap::from([("Residual".to_owned(), String::new())]);
    for (doctrine, expected) in [
        (
            "conflict_doctrine D {\n    when Org.defeat Residual\n}",
            vec![],
        ),
        (
            "conflict_doctrine D {\n    when defeat.x Residual\n}",
            vec![],
        ),
        (
            "conflict_doctrine D {\n    when undefeated Residual or defeat_all Residual\n}",
            vec![],
        ),
        (
            "conflict_doctrine D {\n    then\tdefeat\tResidual\n}",
            vec!["Residual"],
        ),
        (
            "conflict_doctrine D {\n    when Org.defeat Other\n    then defeat Residual\n}",
            vec!["Residual"],
        ),
    ] {
        assert_eq!(
            expand_conflict_clause_ids(doctrine, &clauses),
            expected,
            "{doctrine}"
        );
    }
}

#[test]
fn a_doctrine_that_names_its_clause_only_in_a_comment_or_a_string_is_e511() {
    let doctrine = |body: &str| {
        format!(
            "    clause Residual {{ text \"the rest\" }}\n    conflict_doctrine Limit {{\n{body}\n    }}\n    query q() -> Int {{ return 7 }}"
        )
    };
    let e511 = vec![(
        DiagnosticCode::E511,
        "conflict doctrine `Limit` has an empty ClauseId expansion".to_owned(),
    )];
    for body in [
        "        // defeat Residual",
        "        note \"defeat Residual\"",
        "        when true",
    ] {
        assert_eq!(checked(&doctrine(body)), Err(e511.clone()), "{body}");
    }
    for body in [
        "        then defeat Residual",
        "        // defeat Nothing\n        then defeat Residual // defeat Nothing",
    ] {
        assert!(checked(&doctrine(body)).is_ok(), "{body}");
    }
}

#[test]
fn an_effect_with_comments_compiles_to_what_its_twin_compiles_to() {
    let template = "    effect Review {c}\n    {\n        {c}\n        request(name: String) -> Bool {c}\n    }\n    query q() -> Bool ! {Review} {\n        require Review(\"x\")\n        return true\n    }";
    for comment in COMMENTS {
        let (commented, twin) = twins(template, comment);
        let twin = checked(&twin).expect("the twin compiles");
        let operation = &effect_declaration(&twin).operations[0];
        assert_eq!(operation.name, "request");
        assert_eq!(operation.params, params(&[("name", string())]));
        assert_eq!(operation.result, Type::bool());
        assert_eq!(checked(&commented), Ok(twin), "{comment:?}");
    }
}

#[test]
fn a_comment_inside_the_value_of_the_jurisdiction_header_does_not_change_the_jurisdiction() {
    let template = "    jurisdiction Test. {c}\n        Region\n    query q() -> Int { return 7 }";
    for comment in COMMENTS {
        let (commented, twin) = twins(template, comment);
        let twin = checked(&twin).expect("the twin compiles");
        assert_eq!(checked(&commented), Ok(twin), "{comment:?}");
    }
}
