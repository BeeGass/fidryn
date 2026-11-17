//! The corpus-neutral code names no corpus.
//!
//! Only `crates/fidryn-ingest-uscode` may name the Code it imports. The paths
//! listed here hold none of the words `usc`, `uscode` and `uslm`, and nowhere
//! the string `/us/`, their tests included.

use std::fs;
use std::path::{Path, PathBuf};

/// Directories and files, relative to the workspace root, whose `.rs` files
/// must name no corpus.
const CORPUS_NEUTRAL: &[&str] = &["crates/fidryn-provision/src"];

/// Words compared without regard to case. A word is a maximal run of ASCII
/// letters and digits.
const FORBIDDEN_WORDS: &[&str] = &["usc", "uscode", "uslm"];

const FORBIDDEN_STRING: &str = "/us/";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_owned()
}

/// The `.rs` files at or below `rel`, relative to `root`, sorted.
fn rust_files(root: &Path, rel: &str, files: &mut Vec<String>) {
    let full = root.join(rel);
    let metadata = fs::metadata(&full).unwrap_or_else(|error| panic!("listed path {rel}: {error}"));
    if metadata.is_file() {
        if rel.ends_with(".rs") {
            files.push(rel.to_owned());
        }
        return;
    }
    let mut entries: Vec<String> = fs::read_dir(&full)
        .unwrap_or_else(|error| panic!("read {rel}: {error}"))
        .map(|entry| {
            let entry = entry.unwrap_or_else(|error| panic!("read {rel}: {error}"));
            entry
                .file_name()
                .into_string()
                .unwrap_or_else(|name| panic!("{rel}: file name {name:?} is not UTF-8"))
        })
        .collect();
    entries.sort();
    for name in entries {
        rust_files(root, &format!("{rel}/{name}"), files);
    }
}

/// What a line names that it must not.
fn findings(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    if line.contains(FORBIDDEN_STRING) {
        found.push(format!("the string `{FORBIDDEN_STRING}`"));
    }
    for word in line
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
    {
        if FORBIDDEN_WORDS
            .iter()
            .any(|forbidden| word.eq_ignore_ascii_case(forbidden))
        {
            found.push(format!("the word `{word}`"));
        }
    }
    found
}

#[test]
fn corpus_neutral_code_names_no_corpus() {
    let root = root();
    let mut files = Vec::new();
    for rel in CORPUS_NEUTRAL {
        rust_files(&root, rel, &mut files);
    }
    let mut violations = Vec::new();
    for rel in &files {
        let text = fs::read_to_string(root.join(rel))
            .unwrap_or_else(|error| panic!("read {rel}: {error}"));
        for (index, line) in text.lines().enumerate() {
            for finding in findings(line) {
                violations.push(format!("{rel}:{}: {finding}", index + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "corpus-neutral code names a corpus:\n{}",
        violations.join("\n")
    );
}
