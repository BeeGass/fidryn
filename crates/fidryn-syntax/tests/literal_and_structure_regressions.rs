use fidryn_syntax::{
    ast::{Expr, Item},
    format_module, parse_file,
};

fn query_expr(body: &str) -> Expr {
    let source =
        format!("module SyntaxAudit version \"0.1.0\" {{ query q() -> Int {{ return {body} }} }}");
    let parsed = parse_file(&source);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    parsed
        .module()
        .unwrap()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Query(query) => query.expr.clone(),
            _ => None,
        })
        .unwrap()
}

#[test]
fn statements_after_a_result_are_rejected() {
    for declaration in [
        "fn f() -> Int { return 1; require false }",
        "query q() -> Int { return 1; return 2 }",
        "query q() -> Int { goal Evaluate { return 1; require false } }",
        "query q() -> Int { goal Evaluate { 1 }; require false }",
    ] {
        let parsed = parse_file(&format!("module M version \"1\" {{ {declaration} }}"));
        assert!(parsed.has_errors(), "{declaration}");
    }
}

#[test]
fn signed_integer_limits_are_preserved_and_overflow_is_rejected() {
    assert_eq!(query_expr("-9223372036854775808"), Expr::Int(i64::MIN));
    assert_eq!(query_expr("9223372036854775807"), Expr::Int(i64::MAX));
    for literal in [
        "9223372036854775808",
        "-9223372036854775809",
        "9999999999999999999999999999999999",
    ] {
        let source =
            format!("module M version \"1\" {{ query q() -> Int {{ return {literal} }} }}");
        let parsed = parse_file(&source);
        assert!(parsed.has_errors(), "accepted {literal}");
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.primary_span.is_some())
        );
    }
}

#[test]
fn json_strings_decode_and_format_without_changing_the_value() {
    for (literal, expected) in [
        (r#""\u0041""#, "A"),
        (r#""a\nb""#, "a\nb"),
        (r#""\uD83D\uDE00""#, "😀"),
        (r#""a\\b\"c""#, "a\\b\"c"),
    ] {
        assert_eq!(query_expr(literal), Expr::String(expected.into()));
        let source =
            format!("module M version \"1\" {{ query q() -> String {{ return {literal} }} }}   ");
        let formatted = format_module(&source).unwrap();
        let parsed = parse_file(&formatted);
        assert!(!parsed.has_errors());
        let Item::Query(query) = &parsed.module().unwrap().items[0] else {
            panic!("query disappeared")
        };
        assert_eq!(query.expr, Some(Expr::String(expected.into())));
    }
    for literal in [
        r#""\q""#,
        r#""\uD800""#,
        "\"raw\nnewline\"",
        "\"raw\ttab\"",
        "\"unterminated",
    ] {
        let source =
            format!("module M version \"1\" {{ query q() -> String {{ return {literal} }} }}");
        assert!(parse_file(&source).has_errors(), "accepted {literal:?}");
        assert!(format_module(&source).is_err());
    }
}

#[test]
fn collections_and_named_arguments_keep_their_distinct_syntax() {
    assert!(matches!(query_expr("{1, 2}"), Expr::Set(values) if values.len() == 2));
    assert!(
        matches!(query_expr("{x: 1}"), Expr::Record(fields) if fields.get("x") == Some(&Expr::Int(1)))
    );
    assert!(
        matches!(query_expr("f(x = 10)"), Expr::Call { args, .. } if matches!(&args[..], [Expr::NamedArg { name, value }] if name == "x" && **value == Expr::Int(10)))
    );
}

#[test]
fn deep_expressions_produce_a_diagnostic_instead_of_aborting() {
    for expr in [
        format!("{}1{}", "(".repeat(2000), ")".repeat(2000)),
        format!("{}true", "not ".repeat(2000)),
    ] {
        let source = format!("module M version \"1\" {{ query q() -> Int {{ return {expr} }} }}");
        let parsed = parse_file(&source);
        assert!(parsed.has_errors());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("nesting"))
        );
    }
}

#[test]
fn flat_operator_and_field_chains_are_bounded_before_lowering() {
    for expression in [
        std::iter::repeat_n("1", 2000)
            .collect::<Vec<_>>()
            .join(" + "),
        format!("a{}", ".field".repeat(2000)),
    ] {
        let parsed = parse_file(&format!(
            "module M version \"1\" {{ query q() -> Int {{ return {expression} }} }}"
        ));
        assert!(parsed.has_errors());
    }
}

#[test]
fn generic_entity_does_not_swallow_the_next_declaration() {
    let parsed = parse_file(
        "module M version \"1\" {\nentity X: Option<Int>\nquery q() -> Int { return 7 }\n}",
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    assert!(
        parsed
            .module()
            .unwrap()
            .items
            .iter()
            .any(|item| matches!(item, Item::Query(_)))
    );
}

#[test]
fn comparison_chains_require_an_explicit_grouping() {
    let bad = parse_file("module M version \"1\" { query q() -> Bool { return 1 < 2 < 3 } }");
    assert!(bad.has_errors());
    let good =
        parse_file("module M version \"1\" { query q() -> Bool { return (1 < 2) == true } }");
    assert!(!good.has_errors(), "{:?}", good.diagnostics);
}

#[test]
fn malformed_assertion_suffix_is_not_silently_discarded() {
    let bad = parse_file("module M version \"1\" { verify Bad { assert true = false } }");
    assert!(bad.has_errors());
    let temporal = parse_file(
        "module M version \"1\" { verify Open { assert always\n occupied(Office)\n or active Procedure } }",
    );
    assert!(!temporal.has_errors(), "{:?}", temporal.diagnostics);
    let Item::Verify(verification) = &temporal.module().unwrap().items[0] else {
        panic!("verification disappeared")
    };
    assert_eq!(verification.assertions.len(), 1);
    assert!(!matches!(verification.assertions[0], Expr::Bool(_)));
}

#[test]
fn manifest_header_value_excludes_statement_separators_and_trivia() {
    let parsed = parse_file(
        "module M version \"1\" { source_manifest \"sources/meta.json\"; // path metadata\n query q() -> Bool { return true } }",
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    let value = parsed
        .module()
        .unwrap()
        .items
        .iter()
        .find_map(|item| match item {
            Item::Header(header)
                if header.kind == fidryn_syntax::ast::HeaderKind::SourceManifest =>
            {
                Some(&header.value)
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(value, "\"sources/meta.json\"");
}

#[test]
fn source_and_token_budgets_refuse_before_ast_construction() {
    for source in [" ".repeat(8 * 1024 * 1024 + 1), ";".repeat(262_145)] {
        let parsed = parse_file(&source);
        assert!(parsed.has_errors());
        assert!(parsed.module().is_none());
        assert!(parsed.tokens.is_empty());
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("budget"))
        );
    }
}
