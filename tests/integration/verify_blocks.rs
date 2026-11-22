//! What `fidryn verify` reports for a `verify` block, field by field.
//!
//! `docs/language.md` states the rules these tests pin, under "The fields of
//! a verify block": which words begin a field, where a field ends, and what
//! becomes of a line the block does not recognize.

use fidryn_cli::{compile_module, compile_source};
use fidryn_core::{CoreModule, CoreVerify, DiagnosticCode, SourceManifest, Term};
use fidryn_verify::{PropertyVerdict, verify_property};
use std::path::PathBuf;

const PROPERTY: &str = "Checked";
const UNDECIDED_LINE: &str = "unsupported verification operation `$unsupported_assertion`";
const NO_ASSERTIONS: &str = "verification contains no assertions";
const EMPTY_BOUNDS: &str = "W620 BoundedVerification: quantifier has empty bounds";
const FOR_EVERY: &str = "for every x in {1, 2}: x == 3";

fn source(lines: &[&str]) -> String {
    let block: String = lines
        .iter()
        .map(|line| format!("        {line}\n"))
        .collect();
    format!(
        "module Verify.Lines version \"0.1.0\" {{\n    verify {PROPERTY} {{\n{block}    }}\n}}\n"
    )
}

/// A module whose only declaration is a `verify` block of these lines.
fn block(lines: &[&str]) -> CoreModule {
    compile_source(&source(lines), &SourceManifest::default()).expect("verify block compiles")
}

/// The codes and messages a `verify` block of these lines is rejected with.
fn rejected(lines: &[&str]) -> Vec<(DiagnosticCode, String)> {
    compile_source(&source(lines), &SourceManifest::default())
        .expect_err("verify block is rejected")
        .into_iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.message))
        .collect()
}

fn property(module: &CoreModule) -> &CoreVerify {
    module
        .verifications
        .iter()
        .find(|property| property.name == PROPERTY)
        .expect("the property")
}

fn assertion_count(module: &CoreModule) -> usize {
    property(module).assertions.len()
}

/// The numbers of synthetic persons, events and time points of the property.
fn bounds(module: &CoreModule) -> (u32, u32, u32) {
    let bounds = &property(module).bounds;
    (bounds.persons, bounds.events, bounds.time_points)
}

/// The text of each assertion kept from lines `verify` cannot decide.
fn undecided_text(module: &CoreModule) -> Vec<String> {
    property(module)
        .assertions
        .iter()
        .filter_map(|assertion| match assertion {
            Term::Apply { ctor, args } if ctor == "$unsupported_assertion" => {
                match args.as_slice() {
                    [Term::String(text)] => Some(text.trim().to_owned()),
                    _ => None,
                }
            }
            _ => None,
        })
        .collect()
}

fn assert_proved(module: &CoreModule) {
    let verdict = verify_property(module, PROPERTY);
    assert!(verdict.is_proved(), "expected proved, got: {verdict}");
}

/// The detail of the counterexample `verify` reports.
fn counterexample(module: &CoreModule) -> String {
    match verify_property(module, PROPERTY) {
        PropertyVerdict::Counterexample { detail, .. } => detail,
        other => panic!("expected a counterexample, got: {other}"),
    }
}

/// The reason `verify` gives for leaving the property undecided.
fn unknown(module: &CoreModule) -> String {
    match verify_property(module, PROPERTY) {
        PropertyVerdict::Unknown { reason, .. } => reason,
        other => panic!("expected unknown, got: {other}"),
    }
}

#[test]
fn a_false_assertion_after_one_that_ends_in_an_integer_literal_is_a_counterexample() {
    let module = block(&["assert true or 1 == 2", "assert false"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(counterexample(&module), "assertion 2 evaluates to false");
}

#[test]
fn a_false_assertion_is_a_counterexample_whatever_the_next_line_holds() {
    let module = block(&["assert 1 + 1 == 3", "assert true or true"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(counterexample(&module), "assertion 1 evaluates to false");
}

#[test]
fn two_comparisons_on_consecutive_lines_are_two_assertions() {
    let module = block(&["assert 1 + 1 == 2", "assert 2 == 2"]);
    assert_eq!(assertion_count(&module), 2);
    assert_proved(&module);
}

#[test]
fn two_assertions_on_one_line_are_two_assertions() {
    let module = block(&["assert true or 1 == 2 assert false"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(counterexample(&module), "assertion 2 evaluates to false");
}

#[test]
fn an_assertion_cut_off_by_the_next_assertion_is_e100() {
    for lines in [
        ["assert true or 1 +", "assert false"],
        ["assert true or 1 ==", "assert false"],
        ["assert true or not", "assert false"],
        ["for_all x in {1, 2}: true or 1 +", "assert false"],
    ] {
        assert_eq!(
            rejected(&lines),
            [(
                DiagnosticCode::E100,
                "expected expression after operator".to_owned()
            )],
            "{lines:?}"
        );
    }
}

#[test]
fn an_else_with_no_branch_is_e100_whatever_the_line_below_it_holds() {
    for below in [
        "for_all x in {1, 2}: x == 3",
        "exists x in {1, 2}: x == 3",
        "assert true",
        "assert false",
        "asert false",
        FOR_EVERY,
        "",
    ] {
        let lines = ["assert if true { true } else", below];
        assert_eq!(
            rejected(&lines),
            [(DiagnosticCode::E100, "expected `{` after `else`".to_owned())],
            "{lines:?}"
        );
    }
}

#[test]
fn tokens_that_are_not_a_word_below_an_assertion_are_e100() {
    assert_eq!(
        rejected(&["assert true", "1 == 2"]),
        [(
            DiagnosticCode::E100,
            "unexpected tokens after verification assertion".to_owned()
        )]
    );
}

#[test]
fn a_unit_word_below_an_assertion_is_text_like_any_other_word() {
    // `dayz` is no unit; it shows what any other word gives.
    for lines in [
        ["assert true", "days"],
        ["assert 1 == 1", "working_days x"],
        ["assert true", "dayz"],
    ] {
        let module = block(&lines);
        assert_eq!(
            undecided_text(&module),
            [format!("{}\n        {}", lines[0], lines[1])],
            "{lines:?}"
        );
        assert_eq!(unknown(&module), UNDECIDED_LINE, "{lines:?}");
    }
}

#[test]
fn a_misspelled_assert_line_before_an_assertion_leaves_the_property_unknown() {
    let module = block(&["asert false", "assert true"]);
    assert_eq!(unknown(&module), UNDECIDED_LINE);
}

#[test]
fn a_bare_expression_line_before_an_assertion_leaves_the_property_unknown() {
    let module = block(&["false", "assert true"]);
    assert_eq!(unknown(&module), UNDECIDED_LINE);
}

#[test]
fn a_for_every_line_leaves_the_property_unknown() {
    let module = block(&[FOR_EVERY, "assert true"]);
    assert_eq!(unknown(&module), UNDECIDED_LINE);
}

#[test]
fn a_for_every_line_leaves_the_property_unknown_below_any_other_line() {
    // Below a field, the line is an assertion of its own.
    for above in [
        "interpretations all_admissible",
        "judgments all_lawful",
        "choices all_lawful",
        "assuming false",
        "bounds { persons: 2 }",
        "bounds persons 2 events 1 time_points 1",
        "bounds",
        "assert true",
        "for_all x in {1, 2}: x < 3",
        "exists x in {1, 2}: x == 2",
        "assuming fair_response(Court))",
        "interpretations all_admissible ]",
        "bounds persons 2 )",
    ] {
        let module = block(&[above, FOR_EVERY, "assert true"]);
        assert_eq!(undecided_text(&module), [FOR_EVERY], "below `{above}`");
        assert_eq!(unknown(&module), UNDECIDED_LINE, "below `{above}`");
    }
    // Below a line the block does not recognize, it continues that line,
    // because `for` continues a statement. The two lines are one assertion
    // that cannot be decided.
    let module = block(&["asert true", FOR_EVERY, "assert true"]);
    assert_eq!(
        undecided_text(&module),
        [format!("asert true\n        {FOR_EVERY}")]
    );
    assert_eq!(unknown(&module), UNDECIDED_LINE);
}

#[test]
fn a_false_assertion_above_a_for_every_line_is_a_counterexample() {
    let module = block(&["assert false", FOR_EVERY]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(counterexample(&module), "assertion 1 evaluates to false");
    let module = block(&["for_all x in {1, 2}: x == 3", "for every y in {1}: y == 2"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(
        counterexample(&module),
        "x = Int(1) makes the quantified assertion false"
    );
}

#[test]
fn a_line_that_begins_with_for_is_kept_below_an_assertion() {
    let module = block(&["assert true or x", "for y"]);
    assert_eq!(undecided_text(&module), ["for y"]);
    assert_eq!(unknown(&module), UNDECIDED_LINE);
    let module = block(&["assert true", FOR_EVERY]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(property(&module).assertions[0], Term::Bool(true));
    assert_eq!(unknown(&module), UNDECIDED_LINE);
}

#[test]
fn a_false_quantifier_line_is_a_counterexample() {
    for lines in [
        ["for_all x in {1, 2}: x == 3", "assert true"],
        ["assert true", "for_all x in {1, 2}: x == 3"],
    ] {
        let module = block(&lines);
        assert_eq!(assertion_count(&module), 2, "{lines:?}");
        assert_eq!(
            counterexample(&module),
            "x = Int(1) makes the quantified assertion false",
            "{lines:?}"
        );
    }
}

#[test]
fn a_true_quantifier_line_and_a_true_assertion_are_proved() {
    let module = block(&["for_all x in {1, 2}: x < 3", "assert true"]);
    assert_eq!(assertion_count(&module), 2);
    assert_proved(&module);
}

#[test]
fn a_top_level_exists_line_is_checked_as_an_assertion() {
    let module = block(&["exists x in {1, 2}: x == 2"]);
    assert_eq!(assertion_count(&module), 1);
    assert_proved(&module);
    let module = block(&["exists x in {1, 2}: x == 3", "assert true"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(counterexample(&module), "assertion 1 evaluates to false");
}

#[test]
fn a_quantifier_line_goes_on_into_a_quantifier_line_and_ends_at_an_assert_line() {
    let module = block(&["for_all x in {1, 2}:", "    exists y in {1, 2}: x == y"]);
    assert_eq!(assertion_count(&module), 1);
    assert_proved(&module);
    // The quantifier has no body, and the `assert` is an assertion of its own.
    let module = block(&["for_all x in {1, 2}:", "    assert true"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(property(&module).assertions[1], Term::Bool(true));
    assert_eq!(unknown(&module), "unsupported verification expression");
}

#[test]
fn a_misspelled_assert_line_after_an_assertion_leaves_the_property_unknown() {
    let module = block(&["assert true", "asert false"]);
    assert_eq!(unknown(&module), UNDECIDED_LINE);
}

#[test]
fn an_assuming_line_is_recognized_and_not_read() {
    let module = block(&["assuming false", "assert true"]);
    assert_eq!(assertion_count(&module), 1);
    assert_proved(&module);
}

#[test]
fn an_assertion_on_the_line_of_a_field_that_is_not_read_is_an_assertion() {
    let module = block(&["assuming fair assert true"]);
    assert_eq!(assertion_count(&module), 1);
    assert_proved(&module);
    let module = block(&["assuming fair assert false"]);
    assert_eq!(assertion_count(&module), 1);
    assert_eq!(counterexample(&module), "assertion 1 evaluates to false");
}

#[test]
fn a_field_word_after_an_expression_is_an_operand_unless_a_colon_follows() {
    // The word is taken into the expression, which `verify` cannot decide.
    let module = block(&["assert true assuming fair"]);
    assert_eq!(assertion_count(&module), 1);
    assert_eq!(
        unknown(&module),
        "unsupported verification operation `apply`"
    );
    // Before a `:` the word is not an operand. It begins a field, and that
    // field is not read.
    let module = block(&["assert true assuming: x"]);
    assert_eq!(assertion_count(&module), 1);
    assert_proved(&module);
}

#[test]
fn a_field_that_is_not_read_ends_where_the_next_field_begins() {
    for unread in [
        "assuming fair +",
        "assuming fair ) +",
        "interpretations all_admissible,",
        "bounds persons 2,",
    ] {
        let module = block(&[unread, "assert false", "assert true"]);
        assert_eq!(assertion_count(&module), 2, "below `{unread}`");
        assert_eq!(
            counterexample(&module),
            "assertion 1 evaluates to false",
            "below `{unread}`"
        );
        // A quantifier line begins a field below a field that is not read.
        let module = block(&[unread, "for_all x in {1, 2}: x == 3", "assert true"]);
        assert_eq!(assertion_count(&module), 2, "below `{unread}`");
        assert_eq!(
            counterexample(&module),
            "x = Int(1) makes the quantified assertion false",
            "below `{unread}`"
        );
    }
}

#[test]
fn a_line_a_field_that_is_not_read_goes_on_into_is_not_read() {
    // `(` continues a statement, and so does a line below a `+` or a `,`,
    // and a line that begins with `and` below a blank line. None of these
    // lines begins with a field word, so each belongs to the field above it.
    // A `bounds` field is read for its three numbers only, so the same holds
    // for it.
    let blocks: [&[&str]; 4] = [
        &["assuming fair", "(1 == 2)", "assert true"],
        &["assuming fair +", "asert false", "assert true"],
        &["assuming fair", "", "and asert false", "assert true"],
        &["bounds persons 2,", "asert false", "assert true"],
    ];
    for lines in blocks {
        let module = block(lines);
        assert_eq!(assertion_count(&module), 1, "{lines:?}");
        assert_proved(&module);
    }
}

#[test]
fn a_field_with_no_value_does_not_take_the_assertion_below_it() {
    for field in [
        "interpretations",
        "judgments",
        "choices",
        "assuming",
        "bounds",
    ] {
        let module = block(&[field, "assert false", "assert true"]);
        assert_eq!(assertion_count(&module), 2, "below `{field}`");
        assert_eq!(
            counterexample(&module),
            "assertion 1 evaluates to false",
            "below `{field}`"
        );
    }
}

#[test]
fn an_assert_with_no_expression_leaves_the_property_unknown() {
    let module = block(&["assert", "assert true"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(undecided_text(&module), ["assert"]);
    assert_eq!(unknown(&module), UNDECIDED_LINE);
    // It does not take the assertion below it either.
    let module = block(&["assert", "assert false", "assert true"]);
    assert_eq!(assertion_count(&module), 3);
    assert_eq!(counterexample(&module), "assertion 2 evaluates to false");
    // `not` with nothing after it gives no expression either.
    let module = block(&["assert not", "assert true"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(undecided_text(&module), ["assert not"]);
    assert_eq!(unknown(&module), UNDECIDED_LINE);
}

#[test]
fn a_block_with_no_assertion_is_unknown_whatever_its_comments_say() {
    let blocks: [&[&str]; 7] = [
        &[],
        &["// assert true"],
        &["// assert 1 == 2"],
        &["assuming fair", "// assert true"],
        &["interpretations { assert true }"],
        &["assuming fair", "(1 == 2)", "// assert true"],
        &["bounds { persons: 1 }", "// assert true"],
    ];
    for lines in blocks {
        let module = block(lines);
        assert_eq!(assertion_count(&module), 0, "{lines:?}");
        assert_eq!(property(&module).formula, "", "{lines:?}");
        assert_eq!(unknown(&module), NO_ASSERTIONS, "{lines:?}");
    }
}

#[test]
fn the_bounds_come_only_from_a_bounds_field() {
    const FOR_ALL: &str = "assert for_all p in People: true";
    const EXISTS: &str = "assert exists p in People: true";
    // The lines of a block, and its numbers of persons, events and time
    // points. Each block quantifies over the persons, so it is proved when
    // it has one and `unknown` when it has none.
    let blocks = [
        // With no `bounds` field there is no synthetic person.
        (vec![FOR_ALL], (0, 0, 0)),
        // A comment does not set the bounds, and neither does another field.
        (vec!["// bounds { persons: 2 }", FOR_ALL], (0, 0, 0)),
        (vec!["assuming bounds persons 3", EXISTS], (0, 0, 0)),
        // A `bounds` field sets them, in each of its forms.
        (vec!["bounds { persons: 2 }", FOR_ALL], (2, 0, 0)),
        (
            vec!["bounds persons 2 events 1 time_points 1", FOR_ALL],
            (2, 1, 1),
        ),
        (vec!["bounds", "{ persons: 3 }", FOR_ALL], (3, 0, 0)),
        // The last of several `bounds` fields counts.
        (
            vec!["bounds { persons: 2 }", "bounds { events: 1 }", EXISTS],
            (0, 1, 0),
        ),
        // A comment below a `bounds` field sets nothing,
        (
            vec!["bounds { events: 1 }", "// bounds { persons: 2 }", FOR_ALL],
            (0, 1, 0),
        ),
        (
            vec![
                EXISTS,
                "bounds { events: 1 }",
                "// note: persons 2 is enough",
            ],
            (0, 1, 0),
        ),
        (vec!["bounds events 1", "// persons: 2", EXISTS], (0, 1, 0)),
        (
            vec!["bounds { events: 1 }", "", "// persons: 2", "", EXISTS],
            (0, 1, 0),
        ),
        (
            vec![
                "bounds { events: 1 }",
                "// persons: 2",
                "assuming fair",
                EXISTS,
            ],
            (0, 1, 0),
        ),
        // nor does one on its line or inside it.
        (vec!["bounds events 1 // persons: 2", EXISTS], (0, 1, 0)),
        (
            vec!["bounds {", "// persons: 9", "persons: 2", "}", EXISTS],
            (2, 0, 0),
        ),
        (
            vec!["bounds { events: 1, // persons: 5", "persons: 2 }", EXISTS],
            (2, 1, 0),
        ),
        // The contents of a string are not read, and a number is an integer
        // token.
        (vec!["bounds { label: \"persons: 2\" }", EXISTS], (0, 0, 0)),
        (vec!["bounds { persons: 2.5 }", EXISTS], (0, 0, 0)),
        // The first place where a word stands decides, and an integer that
        // does not fit in 32 bits gives zero.
        (vec!["bounds persons x persons 2", EXISTS], (0, 0, 0)),
        (vec!["bounds { persons: 99999999999 }", EXISTS], (0, 0, 0)),
        // The integer follows its word directly or after one `:`.
        (vec!["bounds persons : 2", EXISTS], (2, 0, 0)),
        (vec!["bounds persons :: 2", EXISTS], (0, 0, 0)),
    ];
    let mut failures = Vec::new();
    for (lines, expected) in blocks {
        let module = block(&lines);
        let verdict = verify_property(&module, PROPERTY);
        let decided = match &verdict {
            PropertyVerdict::Proved { .. } => expected.0 > 0,
            PropertyVerdict::Unknown { reason, .. } => expected.0 == 0 && reason == EMPTY_BOUNDS,
            _ => false,
        };
        if bounds(&module) != expected || !decided {
            failures.push(format!("{lines:?}: expected {expected:?}, read {verdict}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_bounds_field_of_the_trust_example_gives_its_three_numbers() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/trust/bryan-revocable-trust.fr");
    let (module, _) = compile_module(&path).expect("the trust example compiles");
    let printed = verify_property(&module, "TrusteeContinuity").to_string();
    assert!(
        printed.ends_with("(persons=8, events=40, time_points=80)"),
        "{printed}"
    );
}
