//! What `fidryn verify` reports for a `verify` block, line by line.
//!
//! Each `assert` line is an assertion of its own, and so is a top-level
//! `for_all` or `exists` line. A `for every` line, or a line the block does
//! not recognize, leaves the property undecided.

use fidryn_cli::compile_source;
use fidryn_core::{CoreModule, SourceManifest};
use fidryn_verify::{PropertyVerdict, verify_property};

const PROPERTY: &str = "Checked";
const UNDECIDED_LINE: &str = "unsupported verification operation `$unsupported_assertion`";

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

fn assertion_count(module: &CoreModule) -> usize {
    module
        .verifications
        .iter()
        .find(|property| property.name == PROPERTY)
        .expect("the property")
        .assertions
        .len()
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
    let module = block(&["for every x in {1, 2}: x == 3", "assert true"]);
    assert_eq!(unknown(&module), UNDECIDED_LINE);
}

#[test]
fn a_for_every_line_leaves_the_property_unknown_below_any_other_line() {
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
        "asert true",
    ] {
        let module = block(&[above, "for every x in {1, 2}: x == 3", "assert true"]);
        let verdict = verify_property(&module, PROPERTY);
        assert!(
            matches!(verdict, PropertyVerdict::Unknown { .. }),
            "below `{above}`: {verdict}"
        );
    }
}

#[test]
fn a_false_quantifier_line_is_a_counterexample() {
    let module = block(&["for_all x in {1, 2}: x == 3", "assert true"]);
    assert_eq!(assertion_count(&module), 2);
    assert_eq!(
        counterexample(&module),
        "x = Int(1) makes the quantified assertion false"
    );
}

#[test]
fn a_true_quantifier_line_and_a_true_assertion_are_proved() {
    let module = block(&["for_all x in {1, 2}: x < 3", "assert true"]);
    assert_eq!(assertion_count(&module), 2);
    assert_proved(&module);
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
fn a_field_that_is_not_read_ends_where_the_next_field_begins() {
    for unread in [
        "assuming fair +",
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
        "assert",
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
