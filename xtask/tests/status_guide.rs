//! The implementation status guide cites the tests that pin known limits, and
//! every test it cites exists.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const GUIDE: &str = "docs/implementation-status.md";
const TABLE_HEADING: &str = "## Known limits pinned by tests";
const KNOWN_LIMITS: &str = "tests/integration/known_limits.rs";
const REPAIRED_EXAMPLES: &str = "tests/integration/repaired_examples.rs";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_owned()
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|error| panic!("read {rel}: {error}"))
}

/// Names of the `#[test]` functions in a Rust source file.
fn test_functions(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut after_test_attribute = false;
    for line in source.lines() {
        let line = line.trim();
        if line == "#[test]" {
            after_test_attribute = true;
        } else if after_test_attribute && let Some(rest) = line.strip_prefix("fn ") {
            let name = rest.split('(').next().unwrap_or("").trim();
            names.insert(name.to_owned());
            after_test_attribute = false;
        }
    }
    names
}

/// The first column of the table under `TABLE_HEADING`, without backticks.
fn cited_tests(guide: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = guide.lines().collect();
    let heading = lines
        .iter()
        .position(|line| line.trim_end() == TABLE_HEADING)
        .unwrap_or_else(|| panic!("{GUIDE} has no `{TABLE_HEADING}` section"));
    let rows: Vec<&str> = lines[heading + 1..]
        .iter()
        .skip_while(|line| line.trim().is_empty())
        .take_while(|line| line.starts_with('|'))
        .copied()
        .collect();
    assert!(
        rows.len() > 2,
        "{GUIDE}: the table under `{TABLE_HEADING}` has no rows"
    );
    rows[2..]
        .iter()
        .map(|row| {
            let cell = row.split('|').nth(1).unwrap_or("").trim();
            cell.trim_matches('`').to_owned()
        })
        .collect()
}

#[test]
fn every_known_limits_test_is_cited_and_every_cited_test_exists() {
    let known_limits = test_functions(&read(KNOWN_LIMITS));
    let repaired_examples = test_functions(&read(REPAIRED_EXAMPLES));
    assert!(!known_limits.is_empty(), "{KNOWN_LIMITS} has no tests");
    let cited = cited_tests(&read(GUIDE));

    let uncited: Vec<&String> = known_limits.difference(&cited).collect();
    assert!(
        uncited.is_empty(),
        "tests in {KNOWN_LIMITS} that {GUIDE} does not cite under `{TABLE_HEADING}`: {uncited:?}"
    );
    let missing: Vec<&String> = cited
        .iter()
        .filter(|name| !known_limits.contains(*name) && !repaired_examples.contains(*name))
        .collect();
    assert!(
        missing.is_empty(),
        "tests cited by {GUIDE} that are not in {KNOWN_LIMITS} or {REPAIRED_EXAMPLES}: {missing:?}"
    );
}

#[test]
fn test_functions_reads_names_after_the_test_attribute_only() {
    let source =
        "fn helper() {}\n\n#[test]\nfn first() {}\n\n#[test]\n#[ignore]\nfn second_one() {\n}\n";
    assert_eq!(
        test_functions(source),
        BTreeSet::from(["first".to_owned(), "second_one".to_owned()])
    );
}
