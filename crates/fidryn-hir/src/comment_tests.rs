//! A comment means what the same amount of white space means, and the
//! inside of a string literal is no structure.
//!
//! The lowering reads parts of a declaration back out of its text. Each test
//! here puts a comment, or a string, in the place one of those readers
//! looks, and checks what the reader takes. A comment is tested against its
//! twin: the same text with as many spaces where the comment stood.

use super::*;
use fidryn_core::types::{PrimitiveType, Sort, Type};
use fidryn_syntax::parse_file;

/// A comment that holds what a reader of raw text trips over: a parameter
/// list, an arrow, brackets, a quote, the separators, and the words the
/// readers search for, each with something wrong after it.
const TRAP: &str = "// (x: Wrong) -> Wrong { when false then establish Wrong() } [ ] < > \" , ; : | assert false bounds { persons: 9 } returns 9 defeat Residual rank 9 for Nobody alternative Ghost { defines Wrong() as established } artifact \"wrong\" digest \"wrong\" version \"9\" for_all x in People: true";

/// The comments each template is tried with.
const COMMENTS: [&str; 2] = ["// a note", TRAP];

fn module(declarations: &str) -> String {
    format!("module Readers.Text version \"0.1.0\" {{\n{declarations}\n}}\n")
}

/// What the lowering makes of a module with these declarations.
fn lowered(declarations: &str) -> HirModule {
    let parsed = parse_file(&module(declarations));
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    elaborate(&parsed, &SourceManifest::default()).expect("the module is lowered")
}

/// The codes and messages the lowering rejects a module with.
fn rejected(declarations: &str) -> Vec<(DiagnosticCode, String)> {
    let parsed = parse_file(&module(declarations));
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    elaborate(&parsed, &SourceManifest::default())
        .expect_err("the module is rejected")
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.message))
        .collect()
}

/// `template` with each `{c}` replaced by `comment`, and its twin with each
/// `{c}` replaced by as many spaces.
fn twins(template: &str, comment: &str) -> (String, String) {
    (
        template.replace("{c}", comment),
        template.replace("{c}", &" ".repeat(comment.len())),
    )
}

/// What a lowered module means: all of it but the fields that keep text as
/// it is written. Those hold the comments, and the twin holds spaces there.
fn meaning(mut hir: HirModule) -> String {
    hir.jurisdiction.clear();
    hir.effective_at.clear();
    hir.recorded_at.clear();
    for text in hir.offices.values_mut().chain(hir.clauses.values_mut()) {
        text.clear();
    }
    for query in hir.queries.values_mut() {
        query.plan.clear();
    }
    for rule in &mut hir.rules {
        rule.source = None;
    }
    for effect in hir.effects.values_mut() {
        effect.source.clear();
    }
    for function in hir.functions.values_mut() {
        function.source.clear();
    }
    for doctrine in &mut hir.conflict_doctrines {
        doctrine.target.clear();
    }
    for verification in &mut hir.verifications {
        verification.formula.clear();
    }
    for retained in &mut hir.retained {
        retained.source.clear();
    }
    for judgment in &mut hir.judgments {
        judgment.unsupported_clauses.clear();
    }
    for observation in &mut hir.observations {
        observation.unsupported_clauses.clear();
    }
    format!("{hir:#?}")
}

/// Lowers `template` with each of the comments in place of `{c}` and
/// checks that it means what its twin means. Returns the module lowered
/// with the comment that holds the traps.
fn same_as_its_twin(template: &str) -> HirModule {
    for comment in COMMENTS {
        let (commented, twin) = twins(template, comment);
        let (commented, twin) = (meaning(lowered(&commented)), meaning(lowered(&twin)));
        if commented != twin {
            // The lines of each that the other does not have.
            let only = |ours: &str, theirs: &str| -> String {
                ours.lines()
                    .filter(|line| !theirs.lines().any(|other| other == *line))
                    .map(|line| format!("\n    {}", line.trim()))
                    .take(12)
                    .collect()
            };
            panic!(
                "with {comment:?} in\n{template}\nthe module has these lines:{}\nand its twin has these:{}",
                only(&commented, &twin),
                only(&twin, &commented)
            );
        }
    }
    lowered(&twins(template, TRAP).0)
}

fn nominal(name: &str) -> Type {
    Type::Sort(Sort::Nominal(name.to_owned()))
}

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(name, ty)| ((*name).to_owned(), (*ty).to_owned()))
        .collect()
}

// The readers of `lib.rs`.

#[test]
fn the_artifact_of_a_source_is_read_from_its_field_and_not_from_a_comment_or_a_string() {
    let hir = same_as_its_twin(
        "    source Statute {\n        kind statute {c}\n        citation \"see artifact \\\"wrong.txt\\\" (repealed)\"\n        artifact \"files/a}1.txt\"\n        digest fixture\n    }",
    );
    assert_eq!(hir.sources[0].artifact.as_deref(), Some("files/a}1.txt"));
    // A comment or a string that holds the word is no artifact field.
    for body in [
        "kind statute // artifact \"wrong.txt\"",
        "citation \"artifact \\\"wrong.txt\\\"\"",
    ] {
        let hir = lowered(&format!("    source Statute {{\n        {body}\n    }}"));
        assert_eq!(hir.sources[0].artifact, None, "{body}");
    }
    // A string that holds the word does not end the value before it.
    let hir = lowered("    source Statute {\n        artifact \"files/artifact.txt\"\n    }");
    assert_eq!(
        hir.sources[0].artifact.as_deref(),
        Some("files/artifact.txt")
    );
}

#[test]
fn the_type_of_an_entity_holds_no_comment() {
    let hir = same_as_its_twin("    entity Pat : {c}\n        NaturalPerson");
    assert_eq!(
        hir.entities.get("Pat").map(String::as_str),
        Some("NaturalPerson")
    );
}

#[test]
fn parameters_are_not_read_from_a_comment_or_from_inside_a_string() {
    // The parser read no parameters for these declarations, so the lowering
    // looks for a parameter list in their text.
    let hir = same_as_its_twin(
        "    entity Payer : LegalPerson\n    duty Pay {c}\n    {\n        bearer Payer\n    }",
    );
    assert!(
        hir.duties[0].params.is_empty(),
        "{:?}",
        hir.duties[0].params
    );
    let hir = same_as_its_twin(
        "    proposition Witnessed {c}\n        from \"the record\"\n    legal_act Sign {c}\n    {\n        performer signer\n    }\n    office Bench {c}\n        occupied_by NaturalPerson",
    );
    for (name, params) in [
        ("Witnessed", &hir.propositions["Witnessed"]),
        ("Sign", &hir.callable_signatures["Sign"].0),
        ("Bench", &hir.callable_signatures["Bench"].0),
    ] {
        assert!(params.is_empty(), "{name}: {params:?}");
    }
    // A list that is written is read whatever the comment before it holds,
    // and a comma or a colon inside a string separates nothing.
    assert_eq!(
        body::extract_params(
            "duty Pay // (x: Wrong)\n(payer: LegalPerson, // , y: Wrong\n payee: LegalPerson) {\n}"
        ),
        pairs(&[("payer", "LegalPerson"), ("payee", "LegalPerson")])
    );
    assert_eq!(
        body::extract_params("legal_act Sign(doc: Tagged<\"a, b: c)\">, who: LegalPerson)"),
        pairs(&[("doc", "Tagged<\"a, b: c)\">"), ("who", "LegalPerson")])
    );
}

#[test]
fn a_query_with_an_empty_body_has_no_goal_whatever_a_comment_in_it_holds() {
    let hir = same_as_its_twin("    query q() -> Bool {\n        {c}\n    }");
    assert!(!hir.queries["q"].has_goal);
    let hir = lowered("    query q() -> Bool {\n        // return true\n    }");
    assert!(!hir.queries["q"].has_goal);
}

#[test]
fn a_result_type_is_not_read_from_a_comment_or_from_inside_a_string() {
    // The header names no result type, so the lowering looks for one in the
    // text.
    let hir = same_as_its_twin(
        "    query q() {c}\n    {\n        return true\n    }\n    calc f() {c}\n    {\n        return 1\n    }",
    );
    assert_eq!(hir.queries["q"].result_type, "");
    assert_eq!(hir.functions["f"].result_type, "");
    let hir = lowered("    query q() {\n        return note(\"a -> Wrong\")\n    }");
    assert_eq!(hir.queries["q"].result_type, "");
}

#[test]
fn a_quantifier_in_a_comment_or_a_string_is_no_quantifier() {
    let hir = same_as_its_twin(
        "    query q() -> Bool {\n        {c}\n        return true\n    }\n    verify V {\n        {c}\n        assert true\n    }",
    );
    assert!(hir.quantifiers.is_empty(), "{:?}", hir.quantifiers);
    let hir = lowered(
        "    query q() -> String {\n        return \"for_all x in People: true\"\n    }\n    verify V {\n        assert \"exists y in People { true }\" == \"\"\n    }",
    );
    assert!(hir.quantifiers.is_empty(), "{:?}", hir.quantifiers);
}

#[test]
fn a_quantifier_is_read_as_it_is_written_beside_a_comment_or_a_string() {
    let quantifier = |kind, binder: &str, domain: &str, formula: &str| HirQuantifier {
        kind,
        binder: binder.to_owned(),
        domain: domain.to_owned(),
        formula: formula.to_owned(),
    };
    // A comment after the formula, after the domain and inside the block.
    let hir = same_as_its_twin(
        "    query q() -> Bool {\n        return for_all x in People: Eligible(x) {c}\n    }\n    verify V {\n        exists y in People {c}\n        {\n            {c}\n            Trustee(y)\n        }\n    }",
    );
    assert_eq!(
        hir.quantifiers,
        [
            quantifier(QuantifierKind::ForAll, "x", "People", "Eligible(x)"),
            quantifier(QuantifierKind::Exists, "y", "People", "Trustee(y)"),
        ]
    );
    // A colon, a brace or a bracket inside a string ends neither the domain
    // nor the block.
    let hir = lowered(
        "    query q() -> Bool {\n        return for_all x in Tagged(\"a: { b\"): Named(x, \"c\")\n    }\n    verify V {\n        exists y in People { Named(y, \"}\") }\n    }",
    );
    assert_eq!(
        hir.quantifiers,
        [
            quantifier(
                QuantifierKind::ForAll,
                "x",
                "Tagged(\"a: { b\")",
                "Named(x, \"c\")"
            ),
            quantifier(QuantifierKind::Exists, "y", "People", "Named(y, \"}\")"),
        ]
    );
}

#[test]
fn the_office_and_the_rank_of_a_nomination_are_not_read_from_a_comment_or_a_string() {
    let hir = same_as_its_twin(
        "    entity Pat : NaturalPerson\n    nomination Pat {c}\n        for TrusteeOf(BRT)\n        rank 2",
    );
    assert_eq!(hir.nominations[0].office, "TrusteeOf(BRT)");
    assert_eq!(hir.nominations[0].rank, 2);
    let hir = lowered("    nomination Pat for TrusteeOf(\"a for b rank 9\") rank 2");
    assert_eq!(hir.nominations[0].office, "TrusteeOf(\"a for b rank 9\")");
    assert_eq!(hir.nominations[0].rank, 2);
}

#[test]
fn the_alternatives_of_an_interpretation_family_are_not_read_from_a_comment_or_a_string() {
    let alternatives = |hir: &HirModule| -> Vec<(String, Vec<(String, bool)>)> {
        hir.interpretation_families["Successor"]
            .iter()
            .map(|(name, definitions)| {
                (
                    name.clone(),
                    definitions
                        .iter()
                        .map(|(prop, established)| (prop.predicate.clone(), *established))
                        .collect(),
                )
            })
            .collect()
    };
    let hir = same_as_its_twin(
        "    interpretation_family Successor {\n        {c}\n        alternative Both {\n            defines Eligible(Pat) as established {c}\n            defines Barred(Pat) as not_established\n        }\n    }",
    );
    let both = vec![(
        "Both".to_owned(),
        vec![("Eligible".to_owned(), true), ("Barred".to_owned(), false)],
    )];
    assert_eq!(alternatives(&hir), both);
    let hir = lowered(
        "    interpretation_family Successor {\n        alternative Both {\n            defines Eligible(Pat, \"alternative Ghost defines Wrong() as established\") as established\n            defines Barred(Pat) as not_established\n        }\n    }",
    );
    assert_eq!(alternatives(&hir), both);
    assert_eq!(
        hir.interpretation_families["Successor"][0].1[0].0.arguments[1],
        Term::String("alternative Ghost defines Wrong() as established".to_owned())
    );
}

#[test]
fn the_result_type_of_an_effect_is_not_read_from_a_comment_or_a_string() {
    let hir = same_as_its_twin(
        "    effect Review {\n        {c}\n        request(name: String) -> Bool {c}\n    }",
    );
    assert_eq!(hir.effects["Review"].result_type, "Bool");
    let hir = lowered(
        "    effect Review {\n        request(name: String) -> Bool\n        note \"a -> Wrong\"\n    }",
    );
    assert_eq!(hir.effects["Review"].result_type, "Bool");
}

#[test]
fn the_formula_of_a_verify_block_is_not_cut_out_of_a_comment_or_ended_inside_a_string() {
    // The block has no `assert` field, so its formula is the text of the
    // block. That text is kept as it is written. It is not searched for the
    // word `assert`: found in the comment, the word began the formula there.
    let hir = lowered(
        "    verify V {\n        // assert false\n        for_all p in {1, 2} { p == p }\n    }",
    );
    assert_eq!(
        hir.verifications[0].formula,
        "// assert false\n        for_all p in {1, 2} { p == p }"
    );
    // The same for the word in a string.
    let hir = lowered(
        "    verify V {\n        interpretations { note \"assert nothing\" }\n        for_all p in {1, 2} { p == p }\n    }",
    );
    assert_eq!(
        hir.verifications[0].formula,
        "interpretations { note \"assert nothing\" }\n        for_all p in {1, 2} { p == p }"
    );
    // An `assert` between the braces of a `bounds` field is no field of its
    // own, and the formula is cut from it to the brace that closes the
    // field. A brace or a `;` inside a string does not end it earlier.
    let hir = lowered(
        "    verify V {\n        bounds { assert \"a } ; b\" == \"a } ; b\" }\n        for_all p in {1, 2} { p == p }\n    }",
    );
    assert_eq!(
        hir.verifications[0].formula,
        "assert \"a } ; b\" == \"a } ; b\""
    );
}

#[test]
fn type_arguments_are_split_outside_comments_and_strings() {
    let owned = |list: &[&str]| -> Vec<String> { list.iter().map(|s| (*s).to_owned()).collect() };
    assert_eq!(
        split_qname_type_args("Box<A, Map<B, C>>"),
        ("Box".to_owned(), owned(&["A", "Map<B, C>"]))
    );
    assert_eq!(
        split_qname_type_args("Wrap<A, // first > x, y\n    B>"),
        ("Wrap".to_owned(), owned(&["A", "B"]))
    );
    assert_eq!(
        split_qname_type_args("Tag<\"a, b > c\", D>"),
        ("Tag".to_owned(), owned(&["\"a, b > c\"", "D"]))
    );
    assert_eq!(
        parse_angle_args("of <A, // , Wrong>\n B> more"),
        owned(&["A", "B"])
    );
    // The type parameters of a module header and the type arguments of an
    // import, where the lowering reads them from the text.
    assert_eq!(
        type_params_from_source("module Box<T, // , Wrong>\n    U> version \"0.1.0\" { }"),
        owned(&["T", "U"])
    );
    assert_eq!(
        type_args_from_import_text("import Id<A, // , Wrong>\n    B> version \"1\""),
        owned(&["A", "B"])
    );
}

#[test]
fn the_parameters_of_a_type_declaration_hold_no_comment() {
    same_as_its_twin("    type Pair<Left. {c}\n        Part> = Int");
}

#[test]
fn a_consequence_is_quoted_in_a_diagnostic_without_the_comment_inside_it() {
    let template = "    rule Exemption : constitutive {\n        when true\n        then establish 1 + {c}\n            2\n    }";
    for comment in COMMENTS {
        let (commented, twin) = twins(template, comment);
        let expected = [(
            DiagnosticCode::E210,
            "`then establish 1 + 2` in rule `Exemption` requires a proposition".to_owned(),
        )];
        assert_eq!(rejected(&twin), expected, "{comment:?}");
        assert_eq!(rejected(&commented), expected, "{comment:?}");
    }
}

// The readers of `body.rs`.

#[test]
fn a_type_name_is_read_without_the_comment_beside_it_or_inside_it() {
    assert_eq!(parse_type_name("Bool // asks a clerk"), Type::bool());
    assert_eq!(
        parse_type_name("Money<USD> // the fee | Int<Wrong>"),
        Type::Primitive(PrimitiveType::Money {
            currency: "USD".into()
        })
    );
    let map = Type::Primitive(PrimitiveType::Map {
        key: Box::new(Type::Primitive(PrimitiveType::String)),
        value: Box::new(Type::Primitive(PrimitiveType::Int)),
    });
    assert_eq!(parse_type_name("Map<String, Int>"), map);
    assert_eq!(
        parse_type_name("Map<String, // the key, Wrong>\n    Int>"),
        map
    );
    assert_eq!(
        parse_type_name("// either\nNaturalPerson | // or\nLegalPerson"),
        Type::Applied {
            ctor: "Union".into(),
            args: vec![
                Type::Sort(Sort::NaturalPerson),
                Type::Sort(Sort::LegalPerson)
            ],
        }
    );
    assert_eq!(
        parse_type_name("Option<Pair<A, B>> // > , |"),
        Type::Primitive(PrimitiveType::Option {
            inner: Box::new(Type::Applied {
                ctor: "Pair".into(),
                args: vec![nominal("A"), nominal("B")],
            }),
        })
    );
}

#[test]
fn a_separator_or_a_bracket_inside_a_string_does_not_split_a_type_name() {
    assert_eq!(
        parse_type_name("Tagged<\"a, b | c > d\", Int>"),
        Type::Applied {
            ctor: "Tagged".into(),
            args: vec![
                nominal("\"a, b | c > d\""),
                Type::Primitive(PrimitiveType::Int)
            ],
        }
    );
    assert_eq!(parse_type_name("\"A | B<C>\""), nominal("\"A | B<C>\""));
}

#[test]
fn the_text_readers_that_walk_tokens_read_no_comment_and_no_string() {
    // These readers walk the tokens of the text, and a comment is no token
    // to them. They read as they did before.
    let hir = same_as_its_twin(
        "    proposition P()\n    proposition Q()\n    entity Payer : LegalPerson\n    import Other.Law version \"1\" {c}\n    {\n        {c}\n    }\n    rule R {c}\n        : constitutive {\n        {c}\n        when operative P()\n        then derive Q()\n    }\n    duty Pay(payer: LegalPerson) {\n        {c}\n        bearer Payer\n        content USD(100.00) {c}\n        due 3 counted_days after invoice_date\n    }\n    decision Grant(who: LegalPerson) -> Int {\n        {c}\n        record requires Filing\n        returns 7 {c}\n    }",
    );
    assert_eq!(hir.imports[0].version.as_deref(), Some("1"));
    assert_eq!(hir.imports[0].digest, None);
    assert!(!hir.imports[0].digest_required);
    assert_eq!(hir.rules[0].kind, "constitutive");
    assert_eq!(hir.rules[0].consequences.len(), 1);
    assert!(hir.rules[0].fallback.is_empty());
    assert_eq!(hir.duties[0].bearer, "Payer");
    assert_eq!(hir.duties[0].content.len(), 1);
    assert!(hir.duties[0].due.is_some());
    assert_eq!(hir.decisions[0].requirements, ["Filing"]);
    assert_eq!(hir.decisions[0].result, Some(Term::Int(7)));
    // The same readers, called with a text that holds the word they look
    // for in a comment and in a string.
    let text = "decision D() {\n    // returns 9 record requires Ghost\n    note \"returns 9 record requires Ghost\"\n    record requires Filing\n    returns 7\n}";
    assert_eq!(body::parse_decl_result(text), Ok(Some(Term::Int(7))));
    assert_eq!(body::parse_record_requirements(text), ["Filing"]);
    assert_eq!(
        body::parse_rule_kind("rule R // : derive\n \"x : derive\" : constitutive {\n}"),
        "constitutive"
    );
    let text = "import Other.Law version \"1\" {\n    // digest \"wrong\"\n    alias \"digest wrong version 9\"\n}";
    assert!(!body::import_requires_digest(text));
    assert_eq!(body::import_digest(text), None);
    assert_eq!(body::import_version(text).as_deref(), Some("1"));
    let text = "query q() -> Int {\n    // return 9\n    return 7 // goal Evaluate { 9 }\n}";
    assert_eq!(
        body::parse_query_body(text),
        HirQueryBody::Return(Term::Int(7))
    );
    assert_eq!(
        body::parse_function_parts("calc f(a: Int) -> Int {\n    // return 9\n    return 7\n}").1,
        Some(Term::Int(7))
    );
    let rule = body::parse_rule_parts(
        "rule R : derive {\n    // when false\n    then derive Q(\"otherwise derive P()\")\n}",
    );
    assert_eq!(rule.guard, None);
    assert_eq!(rule.consequences.len(), 1);
    assert!(rule.fallback.is_empty());
    let duty = body::parse_duty_parts(
        "duty Pay {\n    // bearer Nobody\n    bearer Payer // claimant Nobody\n    content \"due 9 days\"\n}",
    );
    assert_eq!(duty.bearer, "Payer");
    assert_eq!(duty.claimant, None);
    assert_eq!(duty.due, None);
    assert!(!duty.invalid);
    let functions = std::collections::BTreeSet::from(["f", "g"]);
    assert_eq!(
        collect_source_callees(
            "calc h() -> Int {\n    // g()\n    return f() + size(\"g()\")\n}",
            &functions
        ),
        std::collections::BTreeSet::from(["f".to_owned()])
    );
    let propositions = std::collections::BTreeSet::from(["Eligible".to_owned()]);
    assert!(!source_has_bare_prop_if(
        "query q() -> Int {\n    // if Eligible(x) { 1 }\n    return size(\"if Eligible(x) { 1 }\")\n}",
        &propositions
    ));
    assert_eq!(
        parse_expr_src("1 + // 2 +\n 3"),
        parse_expr_src("1 +       \n 3")
    );
}

#[test]
fn the_type_arguments_of_a_call_hold_no_comment() {
    let hir = same_as_its_twin(
        "    query q() -> Int {\n        return case_input<Wrap<Int, {c}\n            Int>>(\"fee\")\n    }",
    );
    let HirQueryBody::Return(Term::Call { callee, .. }) = &hir.queries["q"].body else {
        panic!("{:?}", hir.queries["q"].body);
    };
    assert!(callee.starts_with("case_input<Wrap<Int,"), "{callee:?}");
    assert!(!callee.contains("//"), "{callee:?}");
}
