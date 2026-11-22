//! A keyword is read where it stands as a word of its own.
//!
//! The lowering reads some parts of a declaration back out of its text by
//! looking for a keyword: the `for` and the `rank` of a nomination, the
//! `artifact` field of a source, and the `alternative`, `defines` and
//! `as established` of an interpretation family. A keyword is not found as
//! a part of a longer name, where a letter, a digit, a `_` or the `.` of a
//! dotted name stands directly beside it. Where the grammar puts white
//! space beside the keyword, a tab or a line end serves as well as a space.
//! Each test here puts the keyword inside a longer name before and after
//! the place where the keyword stands, or a tab or a line end beside it.

use super::*;
use fidryn_syntax::parse_file;

fn module(declarations: &str) -> String {
    format!("module Readers.Words version \"0.1.0\" {{\n{declarations}\n}}\n")
}

/// What the lowering makes of a module with these declarations.
fn lowered(declarations: &str) -> HirModule {
    let parsed = parse_file(&module(declarations));
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
    elaborate(&parsed, &SourceManifest::default()).expect("the module is lowered")
}

/// The candidate, the office and the rank of each nomination.
fn nominations(hir: &HirModule) -> Vec<(String, String, i64)> {
    hir.nominations
        .iter()
        .map(|n| (n.candidate.clone(), n.office.clone(), n.rank))
        .collect()
}

fn read(list: &[(&str, &str, i64)]) -> Vec<(String, String, i64)> {
    list.iter()
        .map(|(candidate, office, rank)| ((*candidate).to_owned(), (*office).to_owned(), *rank))
        .collect()
}

/// Each alternative of the family `Reading`, with the predicate of each of
/// its definitions and whether it is established.
fn alternatives(hir: &HirModule) -> Vec<(String, Vec<(String, bool)>)> {
    hir.interpretation_families["Reading"]
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
}

fn family(body: &str) -> HirModule {
    lowered(&format!(
        "    interpretation_family Reading {{\n{body}\n    }}"
    ))
}

/// The family `Reading` with one alternative, `Wide`, that has these
/// statements.
fn wide(statements: &str) -> HirModule {
    family(&format!(
        "        alternative Wide {{\n            {statements}\n        }}"
    ))
}

fn alternative(name: &str, definitions: &[(&str, bool)]) -> (String, Vec<(String, bool)>) {
    (
        name.to_owned(),
        definitions
            .iter()
            .map(|(predicate, established)| ((*predicate).to_owned(), *established))
            .collect(),
    )
}

// Nominations.

#[test]
fn the_rank_of_a_nomination_is_not_read_from_a_name_that_holds_the_word() {
    // Before the place of the keyword, the word is part of the name of the
    // candidate or of an argument of the office. After it, of the basis.
    // The rank was read as 0 where the name came first.
    let hir = lowered(concat!(
        "    nomination Frank for TrusteeOf(BRT) rank 2\n",
        "    nomination Franklin for TrusteeOf(BRT) rank 3\n",
        "    nomination rank_holder for TrusteeOf(BRT) rank 4\n",
        "    nomination ranked for TrusteeOf(BRT) rank 5\n",
        "    nomination Org.rank for TrusteeOf(BRT) rank 6\n",
        "    nomination Pat for TrusteeOf(p.rank) rank 7\n",
        "    nomination Lee for TrusteeOf(BRT) rank 8\n        from Frankfurt.ranked(\"4\")",
    ));
    assert_eq!(
        nominations(&hir),
        read(&[
            ("Frank", "TrusteeOf(BRT)", 2),
            ("Franklin", "TrusteeOf(BRT)", 3),
            ("rank_holder", "TrusteeOf(BRT)", 4),
            ("ranked", "TrusteeOf(BRT)", 5),
            ("Org.rank", "TrusteeOf(BRT)", 6),
            ("Pat", "TrusteeOf(p.rank)", 7),
            ("Lee", "TrusteeOf(BRT)", 8),
        ])
    );
}

#[test]
fn the_office_of_a_nomination_is_not_cut_at_a_name_that_holds_for_or_rank() {
    // The office is the text between the words `for` and `rank`. A name in
    // the office that begins with `rank` cut it short, and its rank was 0.
    let hir = lowered(concat!(
        "    nomination Platform for Forum rank 1\n",
        "    nomination Forum for Platform rank 2\n",
        "    nomination A for Office(BRT, rank_holder) rank 3\n",
        "    nomination B for Office(BRT, ranked) rank 4\n",
        "    nomination C for Office(Frank) rank 5\n",
        "    nomination Org.for for TrusteeOf(BRT) rank 6",
    ));
    assert_eq!(
        nominations(&hir),
        read(&[
            ("Platform", "Forum", 1),
            ("Forum", "Platform", 2),
            ("A", "Office(BRT, rank_holder)", 3),
            ("B", "Office(BRT, ranked)", 4),
            ("C", "Office(Frank)", 5),
            ("Org.for", "TrusteeOf(BRT)", 6),
        ])
    );
}

#[test]
fn a_tab_or_a_line_end_beside_for_and_rank_serves_as_a_space() {
    // `for` and `rank` continue the statement on the next line. The office
    // was empty where no space stood before or after `for`, and it ran on to
    // the end where no space stood before `rank`.
    for written in [
        "nomination\tPat\tfor\tTrusteeOf(BRT)\trank\t2",
        "nomination Pat\nfor TrusteeOf(BRT)\nrank 2",
        "nomination Pat\n\tfor TrusteeOf(BRT)\n\trank 2",
        "nomination Pat for TrusteeOf(BRT)\nrank 2",
        "nomination Pat\tfor TrusteeOf(BRT) rank 2",
    ] {
        assert_eq!(
            nominations(&lowered(&format!("    {written}"))),
            read(&[("Pat", "TrusteeOf(BRT)", 2)]),
            "{written:?}"
        );
    }
}

#[test]
fn a_nomination_that_was_read_whole_is_read_as_before() {
    // With no `for` the office is empty, and with no `rank` the rank is 0,
    // as before, with no diagnostic.
    let hir = lowered(concat!(
        "    nomination A for TrusteeOf(BRT) rank 1\n",
        "    nomination B for TrusteeOf(BRT) rank -2\n",
        "    nomination C for TrusteeOf(BRT) rank 3;\n",
        "    nomination D for TrusteeOf(BRT)\n",
        "    nomination E rank 5\n",
        "    nomination F\n",
        "    nomination G\n        for TrusteeOf(BRT)\n        rank 7\n",
        "    nomination H for TrusteeOf(BRT) rank 8 from Instrument.clause(\"4.4.a\")",
    ));
    assert_eq!(
        nominations(&hir),
        read(&[
            ("A", "TrusteeOf(BRT)", 1),
            ("B", "TrusteeOf(BRT)", -2),
            ("C", "TrusteeOf(BRT)", 3),
            ("D", "TrusteeOf(BRT)", 0),
            ("E", "", 5),
            ("F", "", 0),
            ("G", "TrusteeOf(BRT)", 7),
            ("H", "TrusteeOf(BRT)", 8),
        ])
    );
    assert!(hir.diagnostics.is_empty(), "{:?}", hir.diagnostics);
}

// Sources.

#[test]
fn the_artifact_of_a_source_is_not_read_from_a_name_that_holds_the_word() {
    let artifact = |name: &str, fields: &str| -> Option<String> {
        lowered(&format!("    source {name} {{\n{fields}\n    }}")).sources[0]
            .artifact
            .clone()
    };
    let field = "        artifact \"files/a.txt\"";
    let others = "        kind statute\n        citation \"Stat. 12\"";
    // The word is part of the name of the source. The artifact was cut out
    // of the text that follows the name, and a source without the field had
    // one.
    for name in ["artifact_registry", "my_artifact", "Org.artifact"] {
        assert_eq!(
            artifact(name, &format!("{others}\n{field}")).as_deref(),
            Some("files/a.txt"),
            "{name}"
        );
        assert_eq!(artifact(name, others), None, "{name}");
    }
    // The word is part of the value of another field, before the field or
    // after it. A tab after the word serves as a space.
    for fields in [
        format!("        kind artifactual\n{field}"),
        format!("        kind raw_artifact\n{field}"),
        format!("{field}\n        digest artifact_digest"),
        "        artifact\t\"files/a.txt\"".to_owned(),
    ] {
        assert_eq!(
            artifact("Statute", &fields).as_deref(),
            Some("files/a.txt"),
            "{fields}"
        );
    }
}

// Interpretation families.

#[test]
fn an_alternative_is_not_found_in_a_name_that_ends_in_the_word() {
    // In the header before the alternatives, and in the name of an
    // alternative. `strict_alternative` was the alternative `strict_`, with
    // no definitions.
    let hir = lowered(concat!(
        "    interpretation_family Reading for reading_alternative {\n",
        "        alternative strict_alternative {\n",
        "            defines Eligible(Alice) as established\n",
        "        }\n",
        "        alternative Other {\n",
        "            defines Flag as not_established\n",
        "        }\n",
        "    }",
    ));
    assert_eq!(
        alternatives(&hir),
        [
            alternative("strict_alternative", &[("Eligible", true)]),
            alternative("Other", &[("Flag", false)]),
        ]
    );
}

#[test]
fn a_definition_is_not_found_in_a_name_that_ends_in_the_word() {
    // In the name of the proposition after the keyword, and in the name of
    // the alternative before it. `law_defines` was lost.
    assert_eq!(
        alternatives(&wide("defines law_defines as established")),
        [alternative("Wide", &[("law_defines", true)])]
    );
    let hir = family(
        "        alternative redefines {\n            defines Flag as established\n        }",
    );
    assert_eq!(
        alternatives(&hir),
        [alternative("redefines", &[("Flag", true)])]
    );
    assert!(hir.diagnostics.is_empty(), "{:?}", hir.diagnostics);
}

#[test]
fn a_tab_or_a_line_end_beside_alternative_defines_and_as_serves_as_a_space() {
    // Each of these was lost, with the alternative or with the definition.
    for status in ["established", "not_established"] {
        let established = status == "established";
        for body in [
            format!(
                "        alternative\tWide\t{{\n            defines Flag as {status}\n        }}"
            ),
            format!(
                "        alternative\nWide {{\n            defines Flag as {status}\n        }}"
            ),
            format!(
                "        alternative Wide {{\n            defines\tFlag\tas\t{status}\n        }}"
            ),
            format!(
                "        alternative Wide {{\n            defines\nFlag as {status}\n        }}"
            ),
            format!(
                "        alternative Wide {{\n            defines Flag\nas {status}\n        }}"
            ),
            format!(
                "        alternative Wide {{\n            defines Flag\n\tas {status}\n        }}"
            ),
            format!(
                "        alternative Wide {{\n            defines Flag as\n{status}\n        }}"
            ),
            format!(
                "        alternative Wide {{\n            defines Flag as  {status}\n        }}"
            ),
        ] {
            assert_eq!(
                alternatives(&family(&body)),
                [alternative("Wide", &[("Flag", established)])],
                "{body:?}"
            );
        }
    }
}

#[test]
fn a_family_that_was_read_whole_is_read_as_before() {
    let hir = lowered(concat!(
        "    interpretation_family Reading\n        for Instrument.clause(\"4.4\")\n    {\n",
        "        alternative I1 {\n",
        "            defines Eligible(Alice, TrusteeOf(BRT)) as established\n",
        "            defines StrictDeadline() as not_established\n",
        "            defines Flag\n                as established\n",
        "            defines law_defines() as not_established\n",
        "        }\n\n",
        "        alternative I2 { defines Flag as established; defines Barred(Bob) as not_established; }\n",
        "        selector explicit_instrument_definition\n          or request Interpret\n",
        "    }",
    ));
    assert_eq!(
        alternatives(&hir),
        [
            alternative(
                "I1",
                &[
                    ("Eligible", true),
                    ("StrictDeadline", false),
                    ("Flag", true),
                    ("law_defines", false),
                ]
            ),
            alternative("I2", &[("Flag", true), ("Barred", false)]),
        ]
    );
    assert!(hir.diagnostics.is_empty(), "{:?}", hir.diagnostics);
}

// The other readers that look for a word.

#[test]
fn a_word_joined_to_a_name_by_a_dot_is_part_of_that_name() {
    // The readers that found a word only where no letter, digit or `_`
    // stood beside it do not find it after or before the `.` of a dotted
    // name either.
    assert_eq!(find_ident("x.assert y assert z", "assert"), Some(11));
    assert_eq!(find_ident("assert.x assert", "assert"), Some(9));
    assert_eq!(find_ident("Org.fuel", "fuel"), None);
    assert!(!contains_ident("{ Org.goal }", "goal"));
    assert_eq!(quantifier_at("Set.exists x in S: true", 4), None);
}
