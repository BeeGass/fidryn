//! The tests of the definition rules: their vectors, the invented fixtures
//! of title 99, the clarifications of the rules each on its own text, and
//! the cases of the words the rules read.

use super::extract;
use crate::testing::{Document, MEMBER, RELEASE, fixture, read_fixture, workspace};
use fidryn_provision::{
    Address, Basis, Block, Definition, DefinitionKind, Item, Layer, Module, Note, PartRole,
    Provision, Run, Table, Text, UnitModule,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;

use Basis::{Enacted, Restated, Unresolved};
use DefinitionKind::{Borrowed, DoesNotInclude, Includes, Means};

fn address(text: &str) -> Address {
    Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

/// A definition with no condition, no source and no note.
fn definition(
    at: &str,
    terms: &[&str],
    kind: DefinitionKind,
    words: Option<&str>,
    region: Option<&str>,
    basis: Basis,
) -> Definition {
    Definition {
        at: address(at),
        terms: terms.iter().map(|term| (*term).to_owned()).collect(),
        kind,
        words: words.map(str::to_owned),
        condition: None,
        region: region.map(address),
        basis,
        borrowed_from: None,
        borrowed_words: None,
        see_note: None,
    }
}

/// `definition` with the condition `condition`.
fn with_condition(definition: Definition, condition: &str) -> Definition {
    Definition {
        condition: Some(condition.to_owned()),
        ..definition
    }
}

/// The definitions of the units of `modules`, unit after unit.
fn definitions_of(modules: &[Module]) -> Vec<Definition> {
    modules
        .iter()
        .flat_map(|module| match module {
            Module::Unit(unit) => unit.definitions.clone(),
            _ => Vec::new(),
        })
        .collect()
}

/// The definitions of the document whose section `/us/usc/t99/s1` holds
/// `inner`.
fn defined(inner: &str) -> Vec<Definition> {
    definitions_of(
        &Document::with_section(inner)
            .read()
            .unwrap_or_else(|error| panic!("{inner}: {error}")),
    )
}

/// The definitions of the text of a document of title 99.
fn definitions_of_text(text: &str) -> Vec<Definition> {
    definitions_of(
        &crate::ingest_xml(MEMBER, text, RELEASE)
            .unwrap_or_else(|error| panic!("{error}"))
            .0,
    )
}

/// The fields of a record, in their order.
const FIELDS: [&str; 10] = [
    "at",
    "terms",
    "kind",
    "words",
    "condition",
    "region",
    "basis",
    "borrowed_from",
    "borrowed_words",
    "see_note",
];

/// The string at `key` of `value`.
fn string_at(value: &Value, key: &str) -> String {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("`{key}` of {value}"))
        .to_owned()
}

/// The array at `key` of `value`.
fn array_at<'v>(value: &'v Value, key: &str) -> &'v [Value] {
    value[key]
        .as_array()
        .unwrap_or_else(|| panic!("`{key}` of {value}"))
}

/// The text of the runs of a vector: words, links, superscripts and
/// footnote markers.
fn text_of_runs(runs: &Value) -> Text {
    let runs = runs
        .as_array()
        .unwrap_or_else(|| panic!("runs: {runs}"))
        .iter()
        .map(|run| {
            if run.get("words").is_some() {
                Run::Words(string_at(run, "words"))
            } else if run.get("link").is_some() {
                Run::Ref {
                    words: string_at(run, "link"),
                    target: string_at(run, "to"),
                }
            } else if run.get("sup").is_some() {
                Run::Sup(string_at(run, "sup"))
            } else if run.get("marker").is_some() {
                Run::Mark {
                    shown: string_at(run, "marker"),
                    footnote: 1,
                }
            } else {
                panic!("a run of no known kind: {run}")
            }
        })
        .collect();
    Text::from_runs(runs)
}

/// The provision of a vector, with its parts and children in order.
fn provision_of(value: &Value) -> Provision {
    let items = array_at(value, "items")
        .iter()
        .map(|item| {
            if let Some(child) = item.get("child") {
                return Item::Child(provision_of(child));
            }
            let role = match string_at(item, "part").as_str() {
                "chapeau" => PartRole::Chapeau,
                "content" => PartRole::Content,
                "continuation" => PartRole::Continuation,
                "proviso" => PartRole::Proviso,
                other => panic!("a part of the role {other}"),
            };
            let blocks = array_at(item, "blocks")
                .iter()
                .map(|block| match (block.get("p"), block.get("table")) {
                    (Some(runs), None) => Block::Para(text_of_runs(runs)),
                    (None, Some(_)) => Block::Table(Table { rows: Vec::new() }),
                    _ => panic!("a block of no known kind: {block}"),
                })
                .collect();
            Item::Part { role, blocks }
        })
        .collect();
    Provision {
        address: address(&string_at(value, "address")),
        kind: string_at(value, "kind"),
        num: None,
        heading: None,
        status: None,
        items,
    }
}

/// A text of one run of words.
fn words_text(words: &str) -> Text {
    Text::from_runs(vec![Run::Words(words.to_owned())])
}

/// The unit of a vector and its containers, the nearest first.
fn unit_of(value: &Value) -> (UnitModule, Vec<(Address, String)>) {
    let containers = array_at(value, "containers")
        .iter()
        .map(|container| {
            (
                address(&string_at(container, "address")),
                string_at(container, "kind"),
            )
        })
        .collect();
    let notes = array_at(value, "notes")
        .iter()
        .map(|note| Note {
            address: address(&string_at(note, "address")),
            layer: match string_at(note, "layer").as_str() {
                "editorial" => Layer::Editorial,
                "enacted" => Layer::Enacted,
                other => panic!("a note of the layer {other}"),
            },
            group: "ungrouped".to_owned(),
            topics: Vec::new(),
            heading: Some(words_text(&string_at(note, "heading"))),
            blocks: array_at(note, "paragraphs")
                .iter()
                .map(|paragraph| {
                    Block::Para(words_text(
                        paragraph
                            .as_str()
                            .unwrap_or_else(|| panic!("a paragraph: {paragraph}")),
                    ))
                })
                .collect(),
        })
        .collect();
    let unit = UnitModule {
        address: address(&string_at(value, "address")),
        aliases: Vec::new(),
        within: None,
        source: String::new(),
        positive_law: Some(
            value["positive_law"]
                .as_bool()
                .unwrap_or_else(|| panic!("`positive_law` of {value}")),
        ),
        root: provision_of(&value["root"]),
        credit: None,
        notes,
        footnotes: Vec::new(),
        definitions: Vec::new(),
    };
    (unit, containers)
}

/// `None` when `found` holds the records of `expected`, as many times
/// each and in any order; otherwise what differs, naming the first field in
/// which two records differ when there are as many of each.
fn records_differ(expected: &[Definition], found: &[Definition]) -> Option<String> {
    let sorted = |list: &[Definition]| {
        let mut values: Vec<Value> = list
            .iter()
            .map(|definition| serde_json::to_value(definition).expect("a definition is JSON"))
            .collect();
        values.sort_by_cached_key(Value::to_string);
        values
    };
    let (expected, found) = (sorted(expected), sorted(found));
    if expected == found {
        return None;
    }
    if expected.len() != found.len() {
        let lines = |values: &[Value]| -> String {
            values
                .iter()
                .map(|value| format!("\n    {value}"))
                .collect()
        };
        return Some(format!(
            "it expects {} records and the importer gives {}; expected:{}\n  given:{}",
            expected.len(),
            found.len(),
            lines(&expected),
            lines(&found)
        ));
    }
    expected
        .iter()
        .zip(&found)
        .find(|(expected, found)| expected != found)
        .map(|(expected, found)| {
            let field = FIELDS
                .iter()
                .find(|field| expected[**field] != found[**field])
                .unwrap_or(&"at");
            format!(
                "the record {expected} differs first in `{field}`: the importer gives {}",
                found[*field]
            )
        })
}

/// The SHA-256 of the vectors of the rules, which are committed byte for
/// byte as the rules give them.
const VECTORS_SHA256: &str = "ffc209c55aa669d3144d7ace126333e4a00d54f998a70246d4e75e74db66ab6c";

#[test]
fn every_vector_gives_the_records_it_expects() {
    let path = workspace().join("conformance/fixtures/definitions/vectors.jsonl");
    let bytes = fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(digest, VECTORS_SHA256, "{}", path.display());
    let text = String::from_utf8(bytes).expect("UTF-8");
    let mut failures = Vec::new();
    let mut count = 0;
    for line in text.lines() {
        let vector: Value = serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}"));
        let (id, rule) = (string_at(&vector, "id"), string_at(&vector, "rule"));
        let expected: Vec<Definition> = array_at(&vector, "expect")
            .iter()
            .map(|record| {
                serde_json::from_value(record.clone())
                    .unwrap_or_else(|error| panic!("{id}: {error}: {record}"))
            })
            .collect();
        let (unit, containers) = unit_of(&vector["unit"]);
        if let Some(difference) = records_differ(&expected, &extract(&unit, &containers)) {
            failures.push(format!("{id} ({rule}): {difference}"));
        }
        count += 1;
    }
    assert_eq!(count, 238);
    assert!(
        failures.is_empty(),
        "{} of {count} vectors fail:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn the_second_fixture_gives_the_records_written_for_it() {
    let path = workspace().join("conformance/fixtures/definitions/second-expected.jsonl");
    let text =
        fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let expected: Vec<&str> = text.lines().collect();
    let mut found = definitions_of(&read_fixture("definitions-second"));
    found.sort_by(|a, b| {
        a.at.as_str()
            .as_bytes()
            .cmp(b.at.as_str().as_bytes())
            .then_with(|| a.terms.cmp(&b.terms))
    });
    let found: Vec<String> = found
        .iter()
        .map(|definition| serde_json::to_string(definition).expect("a definition is JSON"))
        .collect();
    assert_eq!(found, expected);
}

const CHAPTER: &str = "/us/usc/t99/ch1";

#[test]
fn the_sentence_form_has_a_reach_of_its_own_or_none() {
    let s1 = "/us/usc/t99/s1";
    let s2 = "/us/usc/t99/s2";
    let found = definitions_of(&read_fixture("definitions-sentence"));
    let in_s1: Vec<&Definition> = found
        .iter()
        .filter(|definition| definition.at.as_str().starts_with("/us/usc/t99/s1/"))
        .collect();
    assert_eq!(
        in_s1,
        [
            // The aside is a condition (D6.5, D9.6).
            &with_condition(
                definition(
                    "/us/usc/t99/s1/a",
                    &["widget"],
                    Means,
                    Some("As used in this section"),
                    Some(s1),
                    Enacted
                ),
                "with respect to a gadget"
            ),
            &definition(
                "/us/usc/t99/s1/b",
                &["widget", "gadget"],
                Means,
                None,
                None,
                Unresolved
            ),
            &definition(
                "/us/usc/t99/s1/c",
                &["widget"],
                Includes,
                Some("For purposes of this chapter"),
                Some(CHAPTER),
                Enacted
            ),
            &definition(
                "/us/usc/t99/s1/d",
                &["widget"],
                DoesNotInclude,
                None,
                None,
                Unresolved
            ),
            &Definition {
                borrowed_from: Some(s2.to_owned()),
                ..definition(
                    "/us/usc/t99/s1/e",
                    &["widget"],
                    Borrowed,
                    None,
                    None,
                    Unresolved,
                )
            },
            &definition(
                "/us/usc/t99/s1/f",
                &["widget"],
                Means,
                Some("For purposes of this Act"),
                None,
                Unresolved
            ),
            &definition(
                "/us/usc/t99/s1/g",
                &["widget"],
                Means,
                Some("As used in section 2 of this title"),
                Some(s2),
                Enacted
            ),
            &definition(
                "/us/usc/t99/s1/h",
                &["widget"],
                Means,
                Some("As used in Public Law 99\u{2013}1"),
                None,
                Unresolved
            ),
            &definition(
                "/us/usc/t99/s1/i",
                &["widget"],
                Means,
                Some("For the purpose of this section"),
                Some(s1),
                Enacted
            ),
            // `, with respect to` after the verb form is no condition
            // (D6.6).
            &definition(
                "/us/usc/t99/s1/j",
                &["widget"],
                Means,
                None,
                None,
                Unresolved
            ),
        ]
    );
}

#[test]
fn section_3_holds_paragraphs_the_rules_read_and_paragraphs_they_refuse() {
    let modules = read_fixture("definitions-sentence");
    let Some(Module::Unit(s3)) = modules
        .iter()
        .find(|module| crate::testing::address_of(module) == "/us/usc/t99/s3")
    else {
        panic!("section 3");
    };
    assert_eq!(s3.root.items.len(), 11);
    assert_eq!(s3.notes.len(), 1);
    let s3_widget = |at: &str| definition(at, &["widget"], Means, None, None, Unresolved);
    // Nothing is read from (b), whose reach is none (D4.3) and states no
    // condition, from (d), which defines a second term (D11.1), from (i),
    // where a reach follows the verb (D11.4), from (j), which has no reach
    // and speaks of one (D11.6), from the quoted content of (k), or from the
    // note.
    assert_eq!(
        s3.definitions,
        [
            Definition {
                borrowed_words: Some("the Widget Act".to_owned()),
                ..definition(
                    "/us/usc/t99/s3/a",
                    &["widget"],
                    Borrowed,
                    None,
                    None,
                    Unresolved,
                )
            },
            with_condition(s3_widget("/us/usc/t99/s3/c"), "In the case of a gadget"),
            definition(
                "/us/usc/t99/s3/e",
                &["gadget"],
                Means,
                None,
                None,
                Unresolved
            ),
            s3_widget("/us/usc/t99/s3/e"),
            s3_widget("/us/usc/t99/s3/f"),
            definition(
                "/us/usc/t99/s3/g",
                &["widget"],
                Means,
                Some("as used in this section"),
                Some("/us/usc/t99/s3"),
                Enacted
            ),
            with_condition(s3_widget("/us/usc/t99/s3/h"), "in the case of a gadget"),
        ]
    );
}

#[test]
fn a_chapeau_that_states_a_reach_gives_it_to_the_paragraphs_below() {
    let s4 = "/us/usc/t99/s4";
    let s6 = "/us/usc/t99/s6";
    assert_eq!(
        definitions_of(&read_fixture("definitions-chapeau")),
        [
            definition(
                "/us/usc/t99/s1/a/1",
                &["widget"],
                Means,
                Some("In this chapter"),
                Some(CHAPTER),
                Enacted
            ),
            definition(
                "/us/usc/t99/s1/a/2",
                &["gadget"],
                Includes,
                Some("In this chapter"),
                Some(CHAPTER),
                Enacted
            ),
            definition(
                "/us/usc/t99/s1/c/1",
                &["widget"],
                Means,
                Some("As used in this chapter"),
                Some(CHAPTER),
                Enacted
            ),
            definition(
                "/us/usc/t99/s2/1",
                &["widget"],
                Means,
                Some("In this section"),
                Some("/us/usc/t99/s2"),
                Enacted
            ),
            definition(
                "/us/usc/t99/s3/a/1/A",
                &["widget"],
                Means,
                Some("For the purpose of this chapter"),
                Some(CHAPTER),
                Enacted
            ),
            definition(
                "/us/usc/t99/s4/a/1",
                &["widget"],
                Means,
                Some("In this section"),
                Some(s4),
                Enacted
            ),
            definition(
                "/us/usc/t99/s4/b/1",
                &["widget"],
                Means,
                Some("In this section"),
                Some(s4),
                Enacted
            ),
            definition(
                "/us/usc/t99/s4/c/1",
                &["widget"],
                Means,
                Some("In this chapter, unless the context otherwise requires"),
                Some(CHAPTER),
                Enacted
            ),
            definition(
                "/us/usc/t99/s4/d/1",
                &["widget"],
                Means,
                Some("In this chapter and chapter 5"),
                None,
                Unresolved
            ),
            definition(
                "/us/usc/t99/s4/e/1",
                &["widget"],
                Means,
                Some("In this subsection"),
                Some("/us/usc/t99/s4/e"),
                Enacted
            ),
            // An exception, then a lowercase reach (D8.1.2).
            definition(
                "/us/usc/t99/s5/a/1",
                &["widget"],
                Means,
                Some("in this chapter"),
                Some(CHAPTER),
                Enacted
            ),
            // A reach, then a later lead-in (D8.1.2).
            definition(
                "/us/usc/t99/s5/b/1",
                &["widget"],
                Means,
                Some("In this chapter"),
                Some(CHAPTER),
                Enacted
            ),
            // One provision: by the first term, then in the order of the
            // parts.
            definition(
                "/us/usc/t99/s6/a",
                &["apple"],
                Means,
                Some("For purposes of this section"),
                Some(s6),
                Enacted
            ),
            definition(
                "/us/usc/t99/s6/a",
                &["zebra"],
                Means,
                Some("For purposes of this section"),
                Some(s6),
                Enacted
            ),
            definition(
                "/us/usc/t99/s6/b",
                &["widget"],
                Means,
                None,
                None,
                Unresolved
            ),
            definition(
                "/us/usc/t99/s6/b",
                &["widget"],
                Includes,
                None,
                None,
                Unresolved
            ),
        ]
    );
}

#[test]
fn the_words_of_the_rules_are_read_as_they_say() {
    let s1 = "/us/usc/t99/s1";
    let s2 = "/us/usc/t99/s2";
    let widget = |at: &str, words: Option<&str>, region: Option<&str>, basis| {
        vec![definition(at, &["widget"], Means, words, region, basis)]
    };
    let in_content = |content: &str| format!("<content>{content}</content>");
    let under = |chapeau: &str, child: &str| {
        format!(
            "<chapeau>{chapeau}</chapeau><paragraph identifier=\"/us/usc/t99/s1/1\">\
             <num value=\"1\">(1)</num><content>{child}</content></paragraph>"
        )
    };
    let sentence = "The term “widget” means a small device.";
    let none = Vec::new();
    for (inner, expected) in [
        // A verb form stands before the end or a character that is not a
        // letter, and a term has at least one character.
        (
            in_content("The term “widget” meanest a small device."),
            none.clone(),
        ),
        (
            in_content("The term “” means a small device."),
            none.clone(),
        ),
        // Where the rules say "in any case", in any case, and words are
        // whole words.
        (
            in_content("The term “widget” means a device AS USED IN commerce."),
            none.clone(),
        ),
        // An aside that begins `In the case of` speaks of no reach (D11.5),
        // and it is no condition, which begins with a lowercase letter
        // (D6.5).
        (
            in_content("The term “widget”, In the case of a gadget, means a small device."),
            widget(s1, None, None, Unresolved),
        ),
        (
            in_content("The term “widget” means, For Purposes Of this section, a device."),
            none.clone(),
        ),
        (
            in_content("For purposes of the Terms below, the term “widget” means a device."),
            none.clone(),
        ),
        (
            in_content("For purposes of this termination, the term “widget” means a device."),
            widget(
                s1,
                Some("For purposes of this termination"),
                None,
                Unresolved,
            ),
        ),
        // Elsewhere, exactly.
        (
            in_content("for purposes of this section, the term “widget” means a device."),
            none.clone(),
        ),
        (
            in_content("The Term “widget” means a small device."),
            none.clone(),
        ),
        (
            in_content("The term “widget” Means a small device."),
            none.clone(),
        ),
        // After In, the object begins with `this` and a space, a kind word
        // and a space, or is the words of one link.
        (
            in_content(
                "In <ref href=\"/us/usc/t99/s2\">section 2</ref>, the term “widget” means a \
                 device.",
            ),
            widget(s1, Some("In section 2"), Some(s2), Enacted),
        ),
        (
            in_content(
                "In <ref href=\"/us/usc/t99/s2\">section 2</ref> and section 3, the term \
                 “widget” means a device.",
            ),
            widget(s1, Some("In section 2 and section 3"), None, Unresolved),
        ),
        (
            in_content(
                "In <ref href=\"/us/pl/99/1\">Public Law 99\u{2013}1</ref> and section 3, the \
                 term “widget” means a device.",
            ),
            none.clone(),
        ),
        (
            in_content(
                "As used in <ref href=\"/us/usc/t99/s2\">section 2</ref> and section 3, the \
                 term “widget” means a device.",
            ),
            widget(
                s1,
                Some("As used in section 2 and section 3"),
                None,
                Unresolved,
            ),
        ),
        (
            in_content(
                "In <ref href=\"/us/pl/99/1\">Public Law 99\u{2013}1</ref>, the term \
                 “widget” means a device.",
            ),
            widget(s1, Some("In Public Law 99\u{2013}1"), None, Unresolved),
        ),
        // The word of a region is followed by nothing more, or by a comma,
        // a space and `unless the context`.
        (
            in_content(
                "In this section, unless the context requires otherwise, the term “widget” \
                 means a device.",
            ),
            widget(
                s1,
                Some("In this section, unless the context requires otherwise"),
                Some(s1),
                Enacted,
            ),
        ),
        (
            in_content(
                "For purposes of this section, except as provided, the term “widget” means a \
                 device.",
            ),
            widget(
                s1,
                Some("For purposes of this section, except as provided"),
                None,
                Unresolved,
            ),
        ),
        // A close is a dash, a hyphen or a colon, with at most one space
        // before it; an en dash is none.
        (
            under("In this section :", sentence),
            widget(
                "/us/usc/t99/s1/1",
                Some("In this section"),
                Some(s1),
                Enacted,
            ),
        ),
        (
            under("In this section-", sentence),
            widget(
                "/us/usc/t99/s1/1",
                Some("In this section"),
                Some(s1),
                Enacted,
            ),
        ),
        (under("In this section\u{2013}", sentence), none.clone()),
        (
            under(
                "In this section, the terms\u{2014}",
                "“widget” means a small device.",
            ),
            widget(
                "/us/usc/t99/s1/1",
                Some("In this section"),
                Some(s1),
                Enacted,
            ),
        ),
        // A paragraph of the list form under no chapeau is not extracted.
        (in_content("“widget” means a small device."), none.clone()),
    ] {
        assert_eq!(defined(&inner), expected, "{inner}");
    }
}

#[test]
fn a_single_link_below_the_prefix_is_the_region_unless_the_object_is_this_and_a_kind() {
    let s1 = "/us/usc/t99/s1";
    let s3 = "/us/usc/t99/s3";
    // The reach `As used in OBJECT`, its object the words of a link to
    // `target`, or no link when there is none.
    let reach = |object: &str, target: Option<&str>| {
        let object = match target {
            Some(target) => format!("<ref href=\"{target}\">{object}</ref>"),
            None => object.to_owned(),
        };
        format!("<content>As used in {object}, the term “widget” means a device.</content>")
    };
    let widget = |words: &str, region: Option<&str>, basis| {
        vec![definition(
            s1,
            &["widget"],
            Means,
            Some(words),
            region,
            basis,
        )]
    };
    for (inner, expected) in [
        // An object that begins with `this ` and is not `this` and a kind
        // of region: the link's target.
        (
            reach("this Act", Some(s3)),
            widget("As used in this Act", Some(s3), Enacted),
        ),
        (
            reach("this section and section 3", Some(s3)),
            widget("As used in this section and section 3", Some(s3), Enacted),
        ),
        // Not a single link, or not one below the prefix: no region.
        (
            reach("this Act", None),
            widget("As used in this Act", None, Unresolved),
        ),
        (
            reach("this Act", Some("/us/pl/99/1")),
            widget("As used in this Act", None, Unresolved),
        ),
        (
            reach("this Code", Some("/us/usc")),
            widget("As used in this Code", None, Unresolved),
        ),
        // `this` and a kind, a link or not: the nearest of that kind, and
        // no region when none is of it, whatever the link names.
        (
            reach("this section", Some(s3)),
            widget("As used in this section", Some(s1), Enacted),
        ),
        (
            reach("this title", Some(s3)),
            widget("As used in this title", Some("/us/usc/t99"), Enacted),
        ),
        (
            reach(
                "this section, unless the context requires otherwise",
                Some(s3),
            ),
            widget(
                "As used in this section, unless the context requires otherwise",
                Some(s1),
                Enacted,
            ),
        ),
        (
            reach("this chapter", Some(s3)),
            widget("As used in this chapter", None, Unresolved),
        ),
    ] {
        assert_eq!(defined(&inner), expected, "{inner}");
    }
}

/// The definitions of `definitions-restated`, with each basis and note
/// that `bases` and `notes` give.
fn restated(bases: [Basis; 5], notes: [Option<&str>; 5]) -> Vec<Definition> {
    let mut definitions = vec![
        definition(
            "/us/usc/t99/s1",
            &["widget"],
            Means,
            Some("In this chapter"),
            Some(CHAPTER),
            bases[0],
        ),
        definition(
            "/us/usc/t99/s2",
            &["gadget"],
            Means,
            Some("In this section"),
            Some("/us/usc/t99/s2"),
            bases[1],
        ),
        definition("/us/usc/t99/s3", &["sprocket"], Means, None, None, bases[2]),
        definition(
            "/us/usc/t99/s4",
            &["cog"],
            Means,
            Some("As used in section 2 of this title"),
            Some("/us/usc/t99/s2"),
            bases[3],
        ),
        definition(
            "/us/usc/t99/s5",
            &["gear"],
            Means,
            Some("In this chapter"),
            Some(CHAPTER),
            bases[4],
        ),
    ];
    for (definition, note) in definitions.iter_mut().zip(notes) {
        definition.see_note = note.map(address);
    }
    definitions
}

const FLAG_NO: &str = "<property role=\"is-positive-law\">no</property>";

const NOTE_OF_S1: &str = "/us/usc/t99/s1/note/editorialNotes/nolaw/references-in-text";
const NOTE_OF_S4: &str = "/us/usc/t99/s4/note/editorialNotes/nolaw/references-in-text";

#[test]
fn basis_follows_the_files_flag() {
    let text = fixture("definitions-restated");
    assert!(text.contains(FLAG_NO));
    let restated_bases = [Restated, Restated, Unresolved, Restated, Restated];
    let notes = [Some(NOTE_OF_S1), None, None, Some(NOTE_OF_S4), None];
    assert_eq!(definitions_of_text(&text), restated(restated_bases, notes));
    // A file without the flag declares no positive law either.
    assert_eq!(
        definitions_of_text(&text.replace(FLAG_NO, "")),
        restated(restated_bases, notes)
    );
    // With the flag on, a definition with a region is enacted, and it
    // points at no note.
    assert_eq!(
        definitions_of_text(
            &text.replace(FLAG_NO, "<property role=\"is-positive-law\">yes</property>")
        ),
        restated([Enacted, Enacted, Unresolved, Enacted, Enacted], [None; 5])
    );
}

#[test]
fn the_references_in_text_note_is_linked() {
    let text = fixture("definitions-restated");
    let see_notes = |text: &str| -> Vec<Option<String>> {
        definitions_of_text(text)
            .into_iter()
            .map(|definition| definition.see_note.map(|note| note.as_str().to_owned()))
            .collect()
    };
    let some = |note: &str| Some(note.to_owned());
    // Section 2's note names the chapter, not the section; section 5's
    // words stand in quoted content and in a note of the statutory
    // group.
    assert_eq!(
        see_notes(&text),
        [some(NOTE_OF_S1), None, None, some(NOTE_OF_S4), None]
    );
    let first = "<p>This chapter, referred to in text, was";
    assert!(text.contains(first));
    // The words are compared in any case.
    assert_eq!(
        see_notes(&text.replace(first, "<p>THIS CHAPTER, REFERRED TO IN text, was"))[0],
        some(NOTE_OF_S1)
    );
    // The first note that holds them: here the second one.
    assert_eq!(
        see_notes(&text.replace(first, "<p>This chapter was"))[0],
        some(&format!("{NOTE_OF_S1}/2"))
    );
    // A note with another heading is not read.
    let heading = "<heading>References in Text</heading><p>Section 2 of this title";
    assert!(text.contains(heading));
    let changed = text.replace(
        heading,
        "<heading>Codification</heading><p>Section 2 of this title",
    );
    assert_eq!(see_notes(&changed)[3], None);
}

/// Runs of words, superscripts and links, for the clarifications.
fn words(text: &str) -> Run {
    Run::Words(text.to_owned())
}

fn sup(text: &str) -> Run {
    Run::Sup(text.to_owned())
}

fn link(words: &str, target: &str) -> Run {
    Run::Ref {
        words: words.to_owned(),
        target: target.to_owned(),
    }
}

fn part(role: PartRole, runs: Vec<Run>) -> Item {
    Item::Part {
        role,
        blocks: vec![Block::Para(Text::from_runs(runs))],
    }
}

/// The section `/us/usc/t99/s1` of an invented title that is positive law,
/// in chapter 1, with `items` and `notes`.
fn section(items: Vec<Item>, positive_law: bool, notes: Vec<Note>) -> Vec<Definition> {
    let unit = UnitModule {
        address: address("/us/usc/t99/s1"),
        aliases: Vec::new(),
        within: Some(address(CHAPTER)),
        source: "usc99.xml".to_owned(),
        positive_law: Some(positive_law),
        root: Provision {
            address: address("/us/usc/t99/s1"),
            kind: "section".to_owned(),
            num: None,
            heading: None,
            status: None,
            items,
        },
        credit: None,
        notes,
        footnotes: Vec::new(),
        definitions: Vec::new(),
    };
    let containers = [
        (address(CHAPTER), "chapter".to_owned()),
        (address("/us/usc/t99"), "title".to_owned()),
    ];
    extract(&unit, &containers)
}

/// The definitions of a section of positive law whose content is `runs`.
fn of_content(runs: Vec<Run>) -> Vec<Definition> {
    section(vec![part(PartRole::Content, runs)], true, Vec::new())
}

/// The clarifications of the rules, each on the text it gives and with the
/// record it gives.
#[test]
fn each_clarification_gives_the_record_it_states() {
    let s1 = "/us/usc/t99/s1";
    let x = |words: Option<&str>| definition(s1, &["x"], Means, words, None, Unresolved);
    // 1. D1.1.3: the space just before a number is the last character kept
    // so far; nothing kept so far, the space just after it goes.
    assert_eq!(
        of_content(vec![
            words("The term “a "),
            sup("1"),
            sup("2"),
            words(" b” means y."),
        ]),
        [definition(s1, &["a b"], Means, None, None, Unresolved)]
    );
    assert_eq!(
        of_content(vec![sup("1"), sup("2"), words(" The term “b” means y.")]),
        [definition(s1, &["b"], Means, None, None, Unresolved)]
    );
    // 2. A note is read through D1.1 too.
    let note = Note {
        address: address("/us/usc/t99/s1/note/editorialNotes/nolaw/references-in-text"),
        layer: Layer::Editorial,
        group: "editorialNotes".to_owned(),
        topics: vec!["referencesInText".to_owned()],
        heading: Some(Text::from_runs(vec![words("References in Text")])),
        blocks: vec![Block::Para(Text::from_runs(vec![
            words("This section"),
            sup("1"),
            words(", referred to in subsec. (a), is \u{2026}"),
        ]))],
    };
    assert_eq!(
        section(
            vec![part(
                PartRole::Content,
                vec![words(
                    "In this section, the term “widget” means a small device."
                )]
            )],
            false,
            vec![note.clone()],
        ),
        [Definition {
            see_note: Some(note.address.clone()),
            ..definition(
                s1,
                &["widget"],
                Means,
                Some("In this section"),
                Some(s1),
                Restated,
            )
        }]
    );
    // 3. `shall` is looked for as a standing word.
    assert_eq!(
        of_content(vec![words(
            "In the case of Marshall County, the term “x” means y."
        )]),
        [with_condition(x(None), "In the case of Marshall County")]
    );
    // 4. A link counts when its place is the Object's place.
    assert_eq!(
        of_content(vec![
            words("For purposes of this Act, the term “x” means a y of "),
            link("this Act", "/us/usc/t99/s9"),
            words("."),
        ]),
        [x(Some("For purposes of this Act"))]
    );
    // 5. A provision with two chapeau parts decides, and nothing below it is
    // extracted through it.
    assert_eq!(
        section(
            vec![
                part(PartRole::Chapeau, vec![words("In this section:")]),
                part(PartRole::Chapeau, vec![words("Also:")]),
                Item::Child(Provision {
                    address: address("/us/usc/t99/s1/1"),
                    kind: "paragraph".to_owned(),
                    num: None,
                    heading: None,
                    status: None,
                    items: vec![part(PartRole::Content, vec![words("“x” means y.")])],
                }),
            ],
            true,
            Vec::new(),
        ),
        []
    );
    // 6. The second term is compared as it would be recorded.
    assert_eq!(
        of_content(vec![words("The term “x” means y, and “x,” means z.")]),
        [x(None)]
    );
    // 7. The second term and its verb form lie within the sentence.
    assert_eq!(
        of_content(vec![words(
            "The term “x” means y or “z”, as defined. A, means w."
        )]),
        [x(None)]
    );
    // 8. An aside may be empty.
    assert_eq!(
        of_content(vec![words("The term “x”, , means y.")]),
        [x(None)]
    );
    // 9. A lowercase letter is one of `a` to `z`.
    assert_eq!(
        of_content(vec![words(
            "In the case of a gadget, élite or common, the term “x” means y."
        )]),
        [with_condition(
            x(None),
            "In the case of a gadget, élite or common"
        )]
    );
    // 10. The comma that ends a term goes once, from a term longer than one
    // character.
    assert_eq!(
        of_content(vec![words("The term “,” “x” means y.")]),
        [definition(s1, &[",", "x"], Means, None, None, Unresolved)]
    );
    assert_eq!(
        of_content(vec![words("The term “a,,” “b” means c.")]),
        [definition(s1, &["a,", "b"], Means, None, None, Unresolved)]
    );
    // 11. Head words stand where no letter is just before them.
    assert_eq!(
        of_content(vec![words(
            "Except for bathe terms elsewhere, in this section, “x” means y."
        )]),
        [definition(
            s1,
            &["x"],
            Means,
            Some("in this section"),
            Some(s1),
            Enacted
        )]
    );
    // 12. An end of a sentence is judged on the Object's own characters.
    assert_eq!(
        of_content(vec![words("In this section. The term “x” means y.")]),
        [x(Some("In this section."))]
    );
}
