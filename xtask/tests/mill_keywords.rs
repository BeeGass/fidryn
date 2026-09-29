//! The mill highlights `.fr` keywords from a list inside `web/mill.js`. That
//! list must stay equal to the grammar's keywords: every quoted terminal in
//! `grammar.ebnf` made only of `[a-z_]` and longer than one character.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn grammar_keywords(grammar: &str) -> BTreeSet<String> {
    let mut words = BTreeSet::new();
    let mut rest = grammar;
    while let Some(open) = rest.find('"') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('"') else { break };
        let term = &after[..close];
        if term.len() > 1 && term.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
            words.insert(term.to_owned());
        }
        rest = &after[close + 1..];
    }
    words
}

/// The entries between `// KEYWORDS-BEGIN` and `// KEYWORDS-END`, in file
/// order. Each line must be one JSON string followed by a comma.
fn mill_keywords(js: &str) -> Vec<String> {
    let begin = js
        .find("// KEYWORDS-BEGIN")
        .expect("web/mill.js has // KEYWORDS-BEGIN");
    let end = js
        .find("// KEYWORDS-END")
        .expect("web/mill.js has // KEYWORDS-END");
    assert!(begin < end, "KEYWORDS-BEGIN must come before KEYWORDS-END");
    js[begin..end]
        .lines()
        .skip(1)
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let entry = line
                .strip_suffix(',')
                .unwrap_or_else(|| panic!("keyword line {line:?} must end with a comma"));
            entry
                .strip_prefix('"')
                .and_then(|word| word.strip_suffix('"'))
                .filter(|word| !word.contains('"') && !word.contains('\\'))
                .unwrap_or_else(|| panic!("keyword line {line:?} must be one JSON string"))
                .to_owned()
        })
        .collect()
}

#[test]
fn grammar_rule_finds_the_keywords_and_skips_other_terminals() {
    let words = grammar_keywords(
        r#"A ::= "module" QName "{" "outside_scope" "UniqueOccupant" "+inf" "as" "_" "#,
    );
    let expected: BTreeSet<String> = ["as", "module", "outside_scope"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(words, expected);
}

#[test]
fn mill_keyword_list_equals_the_grammar_keywords() {
    let grammar = grammar_keywords(&read("grammar.ebnf"));
    let mill: BTreeSet<String> = mill_keywords(&read("web/mill.js")).into_iter().collect();
    let missing: Vec<&String> = grammar.difference(&mill).collect();
    let extra: Vec<&String> = mill.difference(&grammar).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "web/mill.js KEYWORDS differs from grammar.ebnf\n  missing: {missing:?}\n  not in the grammar: {extra:?}"
    );
    for word in ["module", "query", "require", "return", "outside_scope"] {
        assert!(mill.contains(word), "{word}");
    }
}

#[test]
fn mill_keyword_list_is_sorted_without_duplicates() {
    let list = mill_keywords(&read("web/mill.js"));
    let mut sorted = list.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(list, sorted, "keep the KEYWORDS lines sorted and unique");
}
