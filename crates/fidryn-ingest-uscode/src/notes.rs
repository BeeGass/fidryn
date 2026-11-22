//! Notes, source credits and footnotes: what a module holds beside its
//! provisions or its own fields.
//!
//! The notes of a module are the `note` children of its `notes` elements and
//! the notes that stand directly in its levels, footnotes left out. A note
//! that stands directly in a `title` or an `appendix` and whose text begins
//! `Current through ` is a release note: it is counted and not imported, and
//! nor is a footnote inside it. A `note` of a `notes` element with the role
//! `crossHeading` is no note: it opens the group named by its topic, one
//! word of ASCII letters and digits that begins with a letter, and its label
//! is counted and not imported; what it holds after its label is a note of
//! that group, with no heading and no topics. A note that follows no cross
//! heading, and one that stands directly in a level, is `ungrouped`. The
//! topics of a note are its `topic` attribute split at U+0020, the empty
//! members dropped; a topic that holds any other white-space character is
//! refused.
//!
//! A note is editorial when its group is `editorialNotes`, or when it is
//! ungrouped and one of its topics is `removalDescription` or
//! `historicalAndRevision`; every other note is enacted.
//!
//! The address of a note is that of the level that holds it or its `notes`
//! element, followed by `note`, its group, its law and its heading:
//!
//! - the law is taken from the origin of its first quoted content when that
//!   names a public law, an act or a page of the Statutes at Large, and
//!   otherwise from its first link to a public law or an act, its heading
//!   first; the first four segments of the reference are kept, and the last
//!   three joined by full stops (`pl.117.228`). A note with neither has
//!   `nolaw`;
//! - the heading is lower-cased in `A` to `Z`, each run of characters other
//!   than ASCII letters and digits is one `-`, and it is cut to 60
//!   characters. A note with no heading, or whose heading leaves nothing, has
//!   `noheading` when it has a law and `h.` and 16 digits of its hash when it
//!   has neither;
//! - when several notes of one anchor agree in all three, the second gets
//!   `/2`, the third `/3`, in document order.

use crate::IngestError;
use crate::uslm::{self, Scope};
use fidryn_provision::hash::hash_note;
use fidryn_provision::normalize::{WHITE_SPACE, normalize};
use fidryn_provision::{Address, Block, Footnote, Layer, Note, Quoted, QuotedItem, Run, Text};
use roxmltree::{Document, Node};
use std::collections::HashMap;

/// The group of a note that follows no cross heading, or that stands
/// directly in a level.
const UNGROUPED: &str = "ungrouped";

/// The beginnings of an origin of quoted content that names a law.
const QUOTED_LAWS: [&str; 3] = ["/us/pl/", "/us/act/", "/us/stat/"];

/// The beginnings of the target of a link that names a law.
const LINKED_LAWS: [&str; 2] = ["/us/pl/", "/us/act/"];

/// What a module holds beside its provisions or its own fields.
#[derive(Debug, Default)]
pub(crate) struct Matter {
    pub(crate) credit: Option<Text>,
    /// In document order.
    pub(crate) notes: Vec<Note>,
    /// In ordinal order.
    pub(crate) footnotes: Vec<Footnote>,
}

/// Reads the credit, the notes and the footnotes of the module whose
/// reading `scope` holds. `is_provision` says whether an address is that of
/// a provision of the module: no note may have it.
pub(crate) fn matter(
    document: &Document<'_>,
    scope: &mut Scope,
    is_provision: &dyn Fn(&str) -> bool,
) -> Result<Matter, IngestError> {
    let element = |id| {
        document
            .get_node(id)
            .ok_or_else(|| IngestError::new("an element set aside is not in the document"))
    };
    let aside = std::mem::take(&mut scope.aside);
    let credit = match aside.credit {
        Some(id) => uslm::heading(element(id)?, scope)?,
        None => None,
    };
    let mut read = Vec::new();
    for (id, anchor) in &aside.notes {
        notes_of(element(*id)?, anchor, scope, &mut read)?;
    }
    let notes = named(read, is_provision)?;
    let mut footnotes = Vec::with_capacity(scope.footnotes.nodes.len());
    for (id, ordinal) in scope.footnotes.nodes.clone().into_iter().zip(1..) {
        footnotes.push(uslm::footnote(element(id)?, ordinal, scope)?);
    }
    Ok(Matter {
        credit,
        notes,
        footnotes,
    })
}

/// A note as it is read, before it is named: its element, the address of
/// the level that holds it, and the note, whose address is not yet its own.
struct Unnamed<'a, 'i> {
    node: Node<'a, 'i>,
    anchor: Address,
    note: Note,
}

/// Adds the notes of `node`, a `notes` element or a note that stands
/// directly in the level at `anchor`, to `out`.
fn notes_of<'a, 'i>(
    node: Node<'a, 'i>,
    anchor: &Address,
    scope: &mut Scope,
    out: &mut Vec<Unnamed<'a, 'i>>,
) -> Result<(), IngestError> {
    if !uslm::is(node, "notes") {
        if is_release_note(node) {
            scope.counts.release_notes += 1;
        } else {
            out.push(note(node, anchor, UNGROUPED.to_owned(), scope)?);
        }
        return Ok(());
    }
    let mut group: Option<String> = None;
    for child in node.children() {
        uslm::only_elements(child)?;
        if !child.is_element() || uslm::is_footnote(child) {
            continue;
        }
        if !uslm::is(child, "note") {
            return Err(uslm::misplaced(child, node));
        }
        if child.attribute("role") != Some("crossHeading") {
            let group = group.clone().unwrap_or_else(|| UNGROUPED.to_owned());
            out.push(note(child, anchor, group, scope)?);
            continue;
        }
        let topic = group_word(child)?;
        scope.counts.note_group_headings += 1;
        let (_, blocks) = uslm::note(child, false, scope)?;
        if !blocks.is_empty() {
            out.push(Unnamed {
                node: child,
                anchor: anchor.clone(),
                note: Note {
                    address: anchor.clone(),
                    layer: layer(&topic, &[]),
                    group: topic.clone(),
                    topics: Vec::new(),
                    heading: None,
                    blocks,
                },
            });
        }
        group = Some(topic);
    }
    Ok(())
}

/// The note `node` of the group `group`, anchored at `anchor`.
fn note<'a, 'i>(
    node: Node<'a, 'i>,
    anchor: &Address,
    group: String,
    scope: &mut Scope,
) -> Result<Unnamed<'a, 'i>, IngestError> {
    let topics = topics(node)?;
    let (heading, blocks) = uslm::note(node, true, scope)?;
    Ok(Unnamed {
        node,
        anchor: anchor.clone(),
        note: Note {
            address: anchor.clone(),
            layer: layer(&group, &topics),
            group,
            topics,
            heading,
            blocks,
        },
    })
}

/// The topics of the note `node`: its `topic` attribute split at U+0020,
/// with the empty members dropped, so that an empty attribute gives none. A
/// member that holds any other white-space character is refused.
fn topics(node: Node<'_, '_>) -> Result<Vec<String>, IngestError> {
    let mut topics = Vec::new();
    for topic in node.attribute("topic").unwrap_or("").split(' ') {
        if topic.is_empty() {
            continue;
        }
        if topic.contains(WHITE_SPACE) {
            return Err(IngestError::at(
                node,
                format!(
                    "a note whose topic {topic:?} holds a white-space character other than \
                     U+0020, at which its topics are split"
                ),
            ));
        }
        topics.push(topic.to_owned());
    }
    Ok(topics)
}

/// Whether `node` is a release note: a `note` that is no footnote, stands
/// directly in a `title` or an `appendix`, and whose text begins `Current
/// through `. It is not imported, and nor is a footnote inside it.
pub(crate) fn is_release_note(node: Node<'_, '_>) -> bool {
    uslm::is(node, "note")
        && !uslm::is_footnote(node)
        && node
            .parent()
            .is_some_and(|parent| uslm::is(parent, "title") || uslm::is(parent, "appendix"))
        && normalize(&uslm::text_content(node)).starts_with("Current through ")
}

/// The topic of the cross heading `node`: one word of ASCII letters and
/// digits that begins with a letter.
fn group_word(node: Node<'_, '_>) -> Result<String, IngestError> {
    let topic = node.attribute("topic");
    match topic {
        Some(word)
            if word.starts_with(|c: char| c.is_ascii_alphabetic())
                && word.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            Ok(word.to_owned())
        }
        _ => Err(IngestError::at(
            node,
            format!(
                "a cross heading whose topic is not one word of ASCII letters and digits that \
                 begins with a letter: {}",
                topic.map_or_else(|| "it has none".to_owned(), |word| format!("{word:?}"))
            ),
        )),
    }
}

/// The layer of a note of the group `group` with the topics `topics`.
fn layer(group: &str, topics: &[String]) -> Layer {
    let revision = topics
        .iter()
        .any(|topic| topic == "removalDescription" || topic == "historicalAndRevision");
    if group == "editorialNotes" || (group == UNGROUPED && revision) {
        Layer::Editorial
    } else {
        Layer::Enacted
    }
}

/// The notes of `read`, in document order, each at its address. A note
/// whose address `is_provision` is refused.
fn named(
    read: Vec<Unnamed<'_, '_>>,
    is_provision: &dyn Fn(&str) -> bool,
) -> Result<Vec<Note>, IngestError> {
    let mut held: HashMap<String, u32> = HashMap::new();
    let mut notes = Vec::with_capacity(read.len());
    for Unnamed {
        node,
        anchor,
        mut note,
    } in read
    {
        let law = law(&note, node)?;
        let heading = note
            .heading
            .as_ref()
            .map(heading_segment)
            .filter(|segment| !segment.is_empty());
        let last = match (heading, &law) {
            (Some(heading), _) => heading,
            (None, Some(_)) => "noheading".to_owned(),
            (None, None) => format!("h.{}", &hash_note(&note).to_string()[..16]),
        };
        let law = law.as_deref().unwrap_or("nolaw");
        let base = format!("{anchor}/note/{}/{law}/{last}", note.group);
        let holders = held.entry(base.clone()).or_default();
        *holders += 1;
        let text = match *holders {
            1 => base,
            k => format!("{base}/{k}"),
        };
        if is_provision(&text) {
            return Err(IngestError::at(
                node,
                format!("{text} is the address of two elements: a provision and a note"),
            ));
        }
        note.address = Address::parse(&text).map_err(|error| {
            IngestError::at(node, format!("the note has no address {text:?}: {error}"))
        })?;
        notes.push(note);
    }
    Ok(notes)
}

/// The segment that names the law of `note`, the element `node`, or `None`
/// when it names none. A reference of fewer than four segments, or one whose
/// second, third or fourth segment is no address segment, is refused.
fn law(note: &Note, node: Node<'_, '_>) -> Result<Option<String>, IngestError> {
    let quoted = first_quoted(&note.blocks)
        .and_then(|quoted| quoted.origin.as_deref())
        .filter(|origin| QUOTED_LAWS.iter().any(|law| origin.starts_with(law)));
    let Some(reference) = quoted.or_else(|| {
        note.heading
            .as_ref()
            .and_then(link_in_text)
            .or_else(|| link_in_blocks(&note.blocks))
    }) else {
        return Ok(None);
    };
    let named = match &note.heading {
        Some(heading) => format!("the note {:?}", heading.plain()),
        None => "a note with no heading".to_owned(),
    };
    let segments: Vec<&str> = reference[1..].split('/').collect();
    if segments.len() < 4 {
        return Err(IngestError::at(
            node,
            format!("{named} cites the law {reference:?}, a reference of fewer than four segments"),
        ));
    }
    if let Some(segment) = segments[1..4]
        .iter()
        .find(|segment| Address::parse(&format!("/{segment}")).is_err())
    {
        return Err(IngestError::at(
            node,
            format!(
                "{named} cites the law {reference:?}, whose segment {segment:?} is empty or holds \
                 a character an address segment cannot hold"
            ),
        ));
    }
    Ok(Some(segments[1..4].join(".")))
}

/// The first quoted content of `blocks`, in a table cell included.
fn first_quoted(blocks: &[Block]) -> Option<&Quoted> {
    blocks.iter().find_map(|block| match block {
        Block::Quoted(quoted) => Some(quoted),
        Block::Table(table) => table
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .find_map(|cell| first_quoted(&cell.blocks)),
        Block::Para(_) | Block::Subheading(_) | Block::Image { .. } => None,
    })
}

/// The target of the first link of `text` to a public law or an act.
fn link_in_text(text: &Text) -> Option<&str> {
    text.runs.iter().find_map(|run| match run {
        Run::Ref { target, .. } if LINKED_LAWS.iter().any(|law| target.starts_with(law)) => {
            Some(target.as_str())
        }
        _ => None,
    })
}

/// The target of the first link of `blocks` to a public law or an act, in
/// a table and in quoted content included.
fn link_in_blocks(blocks: &[Block]) -> Option<&str> {
    blocks.iter().find_map(|block| match block {
        Block::Para(text) | Block::Subheading(text) => link_in_text(text),
        Block::Table(table) => table
            .rows
            .iter()
            .flat_map(|row| &row.cells)
            .find_map(|cell| link_in_blocks(&cell.blocks)),
        Block::Quoted(quoted) => link_in_items(&quoted.items),
        Block::Image { .. } => None,
    })
}

/// The target of the first link of quoted `items` to a public law or an act.
fn link_in_items(items: &[QuotedItem]) -> Option<&str> {
    items.iter().find_map(|item| match item {
        QuotedItem::Block(block) => link_in_blocks(std::slice::from_ref(block)),
        QuotedItem::Part { blocks, .. } => link_in_blocks(blocks),
        QuotedItem::Level(level) => level
            .num
            .as_ref()
            .and_then(|num| link_in_text(&num.shown))
            .or_else(|| level.heading.as_ref().and_then(link_in_text))
            .or_else(|| link_in_items(&level.items)),
    })
}

/// The segment a heading gives a note's address: its text with `A` to `Z`
/// in lower case, each run of characters other than ASCII letters and digits
/// one `-`, no `-` at either end, cut to 60 characters and again without `-`
/// at its end. Empty when the heading leaves nothing.
fn heading_segment(heading: &Text) -> String {
    let mut segment = String::new();
    for character in heading.plain().chars() {
        let character = character.to_ascii_lowercase();
        if character.is_ascii_alphanumeric() {
            segment.push(character);
        } else if !segment.is_empty() && !segment.ends_with('-') {
            segment.push('-');
        }
    }
    // Every character is ASCII, so 60 characters are 60 bytes.
    segment.truncate(60);
    segment.trim_end_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use crate::testing::{Document, MEMBER, RELEASE, assert_fixture, read_fixture, refusal};
    use fidryn_provision::hash::{
        hash_note, hash_provision, layer_hashes_of_container, layer_hashes_of_unit,
    };
    use fidryn_provision::{
        Address, Block, ContainerModule, Item, Layer, Module, Note, QuotedItem, Run, Text,
        UnitModule,
    };

    fn unit_at<'m>(modules: &'m [Module], address: &str) -> &'m UnitModule {
        modules
            .iter()
            .find_map(|module| match module {
                Module::Unit(unit) if unit.address.as_str() == address => Some(unit),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no unit {address}"))
    }

    fn container_at<'m>(modules: &'m [Module], address: &str) -> &'m ContainerModule {
        modules
            .iter()
            .find_map(|module| match module {
                Module::Container(container) if container.address.as_str() == address => {
                    Some(container)
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no container {address}"))
    }

    /// The modules of the document whose section `/us/usc/t99/s1` holds
    /// `inner`.
    fn section(inner: &str) -> Vec<Module> {
        Document::with_section(inner)
            .read()
            .unwrap_or_else(|error| panic!("{error}"))
    }

    /// The notes of the section of the document whose section holds
    /// `<content>Text.</content>` and the notes `notes` in one `notes`
    /// element.
    fn notes_of(notes: &str) -> Vec<Note> {
        let modules = section(&format!(
            "<content>Text.</content><notes type=\"uscNote\">{notes}</notes>"
        ));
        unit_at(&modules, "/us/usc/t99/s1").notes.clone()
    }

    fn addresses(notes: &[Note]) -> Vec<String> {
        notes.iter().map(|note| note.address.to_string()).collect()
    }

    fn words(text: &str) -> Run {
        Run::Words(text.to_owned())
    }

    fn para(runs: Vec<Run>) -> Block {
        Block::Para(Text { runs })
    }

    fn plain(text: &str) -> Text {
        Text {
            runs: vec![words(text)],
        }
    }

    fn mark(shown: &str, footnote: u32) -> Run {
        Run::Mark {
            shown: shown.to_owned(),
            footnote,
        }
    }

    #[test]
    fn the_group_of_a_note_is_the_heading_it_follows() {
        assert_fixture("notes-groups");
        let modules = read_fixture("notes-groups");
        let unit = unit_at(&modules, "/us/usc/t99/s1");
        // In document order; the four cross headings are no notes.
        let read: Vec<(&str, Layer, Vec<&str>)> = unit
            .notes
            .iter()
            .map(|note| {
                (
                    note.group.as_str(),
                    note.layer,
                    note.topics.iter().map(String::as_str).collect(),
                )
            })
            .collect();
        assert_eq!(
            read,
            [
                ("ungrouped", Layer::Editorial, vec!["removalDescription"]),
                ("ungrouped", Layer::Editorial, vec!["historicalAndRevision"]),
                ("ungrouped", Layer::Enacted, vec!["miscellaneous"]),
                ("editorialNotes", Layer::Editorial, vec!["amendments"]),
                ("statutoryNotes", Layer::Enacted, vec!["effectiveDate"]),
                ("execDoc", Layer::Enacted, vec!["executiveOrder"]),
                ("courtRules", Layer::Enacted, vec!["miscellaneous"]),
            ]
        );
        // The label of each cross heading is counted and not imported.
        let (_, counted) =
            crate::ingest_xml(MEMBER, &crate::testing::fixture("notes-groups"), RELEASE)
                .expect("read");
        assert_eq!(counted.note_group_headings, 4);
        let labels = [
            "Editorial Notes",
            "Statutory Notes and Related Subsidiaries",
            "Executive Documents",
            "Court Rules",
        ];
        assert!(unit.notes.iter().all(|note| {
            note.heading
                .as_ref()
                .is_none_or(|heading| !labels.contains(&heading.plain().as_str()))
        }));
        // A word the rules do not know is a group like any other, and its
        // notes are enacted.
        assert_fixture("notes-unknown-group");
        let notes = notes_of(
            "<note role=\"crossHeading\" topic=\"demoNotes2\"><heading>Demo</heading></note>\
             <note topic=\"amendments\"><heading>Words</heading><p>Words.</p></note>",
        );
        assert_eq!(
            (notes[0].group.as_str(), notes[0].layer),
            ("demoNotes2", Layer::Enacted)
        );
        // The text a cross heading holds after its label is a note of its
        // group, with no heading and no topics.
        assert_fixture("notes-group-headings");
        let notes = notes_of(
            "<note role=\"crossHeading\" topic=\"editorialNotes\"><heading>Editorial \
             Notes</heading><p>Held words.</p></note>",
        );
        assert_eq!(notes.len(), 1);
        assert_eq!(
            (
                notes[0].group.as_str(),
                notes[0].layer,
                notes[0].topics.len(),
                &notes[0].heading
            ),
            ("editorialNotes", Layer::Editorial, 0, &None)
        );
        assert_eq!(notes[0].blocks, [para(vec![words("Held words.")])]);
    }

    #[test]
    fn an_ungrouped_removal_description_or_revision_note_is_editorial_and_any_other_is_enacted() {
        let notes = notes_of(
            "<note topic=\"removalDescription\"><p>A.</p></note>\
             <note topic=\"amendments historicalAndRevision\"><p>B.</p></note>\
             <note topic=\"amendments\"><p>C.</p></note>\
             <note><p>D.</p></note>\
             <note topic=\"removalDescription x\"><p>E.</p></note>\
             <note role=\"crossHeading\" topic=\"statutoryNotes\"><heading>S</heading></note>\
             <note topic=\"removalDescription\"><p>F.</p></note>\
             <note role=\"crossHeading\" topic=\"editorialNotes\"><heading>E</heading></note>\
             <note topic=\"miscellaneous\"><p>G.</p></note>",
        );
        let layers: Vec<Layer> = notes.iter().map(|note| note.layer).collect();
        assert_eq!(
            layers,
            [
                Layer::Editorial,
                Layer::Editorial,
                Layer::Enacted,
                Layer::Enacted,
                Layer::Editorial,
                Layer::Enacted,
                Layer::Editorial,
            ]
        );
        assert_eq!(notes[1].topics, ["amendments", "historicalAndRevision"]);
        assert_eq!(notes[3].topics, Vec::<String>::new());
        assert_eq!(notes[4].topics, ["removalDescription", "x"]);
    }

    #[test]
    fn topics_are_split_at_spaces_and_an_empty_one_is_none() {
        assert_fixture("notes-empty-topic");
        assert_fixture("notes-spaced-topics");
        // The empty members of the split are dropped: an empty topic attribute
        // and one of spaces give no topic, and spaces before, between and
        // after topics give the topics alone.
        let notes = notes_of(
            "<note topic=\"\"><p>A.</p></note>\
             <note topic=\"   \"><p>B.</p></note>\
             <note topic=\" amendments   historicalAndRevision  \"><p>C.</p></note>",
        );
        let topics: Vec<Vec<&str>> = notes
            .iter()
            .map(|note| note.topics.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            topics,
            [vec![], vec![], vec!["amendments", "historicalAndRevision"]]
        );
        assert_eq!(notes[2].layer, Layer::Editorial);
        // A line break written in the attribute is a space once the attribute
        // is read; one written as a character reference is not.
        let notes = notes_of("<note topic=\"amendments\nshortTitle\"><p>A.</p></note>");
        assert_eq!(notes[0].topics, ["amendments", "shortTitle"]);
        // A topic that holds any other white-space character is refused, and
        // the refusal names the note.
        for topic in [
            "amendments\u{A0}x",
            "amendments&#x9;x",
            "amendments&#xA;x",
            "&#xD;",
            "amendments \u{2003}",
            "\u{3000}",
        ] {
            let error = refusal(&Document::with_section(&format!(
                "<content>Text.</content><notes type=\"uscNote\"><note topic=\"{topic}\">\
                 <heading>Odd</heading><p>A.</p></note></notes>"
            )));
            assert!(
                error
                    .message()
                    .contains("holds a white-space character other than U+0020"),
                "{topic}: {error}"
            );
            assert_eq!(error.element(), Some("note"), "{topic}: {error}");
            assert_eq!(error.identifier(), Some("/us/usc/t99/s1"), "{topic}");
        }
    }

    #[test]
    fn a_note_directly_under_a_level_is_anchored_there() {
        assert_fixture("notes-anchors");
        let modules = read_fixture("notes-anchors");
        let unit = unit_at(&modules, "/us/usc/t99/s1");
        assert_eq!(
            addresses(&unit.notes),
            [
                "/us/usc/t99/s1/note/ungrouped/pl.999.3/noheading",
                "/us/usc/t99/s1/2/note/ungrouped/pl.999.4/noheading"
            ]
        );
        let paragraph = Address::parse("/us/usc/t99/s1/2").expect("an address");
        assert!(unit.notes[1].address.is_within(&paragraph));
        assert!(unit.notes.iter().all(|note| note.group == "ungrouped"));
        // A note that stands in a paragraph gives the paragraph no part.
        let Item::Child(second) = &unit.root.items[1] else {
            panic!("{:?}", unit.root.items);
        };
        assert!(second.items.is_empty());
        // A `notes` element in a paragraph is anchored at the paragraph.
        let modules = section(
            "<paragraph identifier=\"/us/usc/t99/s1/1\"><num value=\"1\">(1)</num><content>Kept.\
             </content><notes type=\"uscNote\"><note topic=\"miscellaneous\"><heading>Here\
             </heading><p>Words.</p></note></notes></paragraph>",
        );
        assert_eq!(
            addresses(&unit_at(&modules, "/us/usc/t99/s1").notes),
            ["/us/usc/t99/s1/1/note/ungrouped/nolaw/here"]
        );
    }

    #[test]
    fn a_note_under_a_container_belongs_to_the_container() {
        let modules = read_fixture("notes-anchors");
        let chapter = container_at(&modules, "/us/usc/t99/ch1");
        assert_eq!(
            addresses(&chapter.notes),
            ["/us/usc/t99/ch1/note/ungrouped/nolaw/chapter-note"]
        );
        assert_eq!(chapter.notes[0].layer, Layer::Enacted);
        assert!(container_at(&modules, "/us/usc/t99").notes.is_empty());
        // A container has a credit, notes that stand directly in it, and
        // footnotes of its own; its units number their footnotes apart.
        let modules = Document::with_body(
            "<chapter identifier=\"/us/usc/t99/ch1\"><num value=\"1\">CHAPTER 1<ref \
             class=\"footnoteRef\" idref=\"c\">1</ref><note type=\"footnote\" id=\"c\"><num>1\
             </num> So in original.</note></num><heading>Chapter</heading>\
             <note topic=\"miscellaneous\"><p>[Invented.]</p></note>\
             <sourceCredit>(Invented act.)</sourceCredit>\
             <section identifier=\"/us/usc/t99/s1\"><content>Text<ref class=\"footnoteRef\" \
             idref=\"s\">2</ref><note type=\"footnote\" id=\"s\"><num>2</num> A footnote.</note>.\
             </content></section></chapter>"
                .to_owned(),
        )
        .read()
        .expect("read");
        let chapter = container_at(&modules, "/us/usc/t99/ch1");
        assert_eq!(chapter.notes.len(), 1);
        assert!(
            chapter.notes[0]
                .address
                .as_str()
                .starts_with("/us/usc/t99/ch1/note/ungrouped/nolaw/h.")
        );
        assert_eq!(chapter.credit, Some(plain("(Invented act.)")));
        assert_eq!(
            chapter.num.as_ref().map(|num| num.shown.clone()),
            Some(Text {
                runs: vec![words("CHAPTER 1"), mark("1", 1)]
            })
        );
        assert_eq!(chapter.footnotes.len(), 1);
        let unit = unit_at(&modules, "/us/usc/t99/s1");
        assert_eq!((unit.footnotes.len(), unit.footnotes[0].ordinal), (1, 1));
        assert!(unit.notes.is_empty() && unit.credit.is_none());
    }

    #[test]
    fn a_note_is_named_by_its_group_law_and_heading() {
        assert_fixture("notes-addresses");
        let modules = read_fixture("notes-addresses");
        let notes = &unit_at(&modules, "/us/usc/t99/s1").notes;
        let named: Vec<(&str, String)> = notes
            .iter()
            .map(|note| {
                (
                    note.address.as_str(),
                    note.heading.as_ref().map(Text::plain).unwrap_or_default(),
                )
            })
            .collect();
        assert!(named.contains(&(
            "/us/usc/t99/s1/note/statutoryNotes/pl.999.7/effective-date",
            "Effective Date".to_owned()
        )));
        // The three forms of the law: a public law, an act and a page of
        // the Statutes at Large.
        for address in [
            "/us/usc/t99/s1/note/statutoryNotes/pl.999.7/short-title",
            "/us/usc/t99/s1/note/statutoryNotes/act.2099-01-01.ch9/noheading",
            "/us/usc/t99/s1/note/statutoryNotes/stat.99.100/quoted-origin",
        ] {
            assert!(named.iter().any(|(at, _)| *at == address), "{address}");
        }
        // A link to the Code, a link with no words and a link inside a
        // footnote name no law; a link in the heading comes first.
        let notes = notes_of(
            "<note topic=\"miscellaneous\"><heading>Links</heading><p>See <ref \
             href=\"/us/usc/t99/s2\">section 2</ref>, <ref href=\"/us/pl/999/20\"> </ref>, \
             a<ref class=\"footnoteRef\" idref=\"f\">1</ref><note type=\"footnote\" id=\"f\">\
             <num>1</num> See <ref href=\"/us/pl/999/21\">Pub. L. 999–21</ref>.</note> and <ref \
             href=\"/us/stat/99/3\">99 Stat. 3</ref> and <ref href=\"/us/act/2099-02-02/ch3/s1\">\
             an act</ref>.</p></note>\
             <note topic=\"miscellaneous\"><heading>Under <ref href=\"/us/pl/999/30\">Pub. L. \
             999–30</ref></heading><p><ref href=\"/us/pl/999/31\">Pub. L. 999–31</ref>.</p></note>",
        );
        assert_eq!(
            addresses(&notes),
            [
                "/us/usc/t99/s1/note/ungrouped/act.2099-02-02.ch3/links",
                "/us/usc/t99/s1/note/ungrouped/pl.999.30/under-pub-l-999-30"
            ]
        );
    }

    #[test]
    fn the_law_of_a_note_is_its_quoted_origin_before_its_first_link() {
        let notes = notes_of(
            "<note topic=\"amendments\"><heading>Amendments</heading><p><ref \
             href=\"/us/pl/999/40/s1\">Pub. L. 999–40</ref> amended it to read: <quotedContent \
             origin=\"/us/pl/999/41/s2\">“Text.”</quotedContent></p></note>\
             <note topic=\"amendments\"><heading>Elsewhere</heading><p><ref \
             href=\"/us/pl/999/40/s1\">Pub. L. 999–40</ref> quoted: <quotedContent \
             origin=\"/us/usc/t99/s2\">“Text.”</quotedContent><quotedContent \
             origin=\"/us/pl/999/42\">“More.”</quotedContent></p></note>\
             <note topic=\"amendments\"><heading>Unnamed</heading><p>It read: <quotedContent>“Text \
             of <ref href=\"/us/act/2099-03-03/ch4\">an act</ref>.”</quotedContent></p></note>",
        );
        assert_eq!(
            addresses(&notes),
            [
                "/us/usc/t99/s1/note/ungrouped/pl.999.41/amendments",
                "/us/usc/t99/s1/note/ungrouped/pl.999.40/elsewhere",
                "/us/usc/t99/s1/note/ungrouped/act.2099-03-03.ch4/unnamed"
            ]
        );
        let modules = read_fixture("notes-content");
        assert!(
            unit_at(&modules, "/us/usc/t99/s1")
                .notes
                .iter()
                .any(|note| note.address.as_str()
                    == "/us/usc/t99/s1/note/statutoryNotes/pl.999.5/grant-program")
        );
    }

    #[test]
    fn two_notes_with_one_law_and_one_heading_get_ordinals_in_document_order() {
        let modules = read_fixture("notes-addresses");
        let reports: Vec<(String, String)> = unit_at(&modules, "/us/usc/t99/s1")
            .notes
            .iter()
            .filter(|note| note.address.as_str().contains("/report"))
            .map(|note| {
                let Block::Para(text) = &note.blocks[0] else {
                    panic!("{note:?}");
                };
                (note.address.to_string(), text.plain())
            })
            .collect();
        assert_eq!(
            reports,
            [
                (
                    "/us/usc/t99/s1/note/statutoryNotes/pl.999.8/report".to_owned(),
                    "Pub. L. 999–8, § 3, required a report.".to_owned()
                ),
                (
                    "/us/usc/t99/s1/note/statutoryNotes/pl.999.8/report/2".to_owned(),
                    "Pub. L. 999–8, § 4, required another report.".to_owned()
                ),
            ]
        );
        // The count runs over every note of one anchor, in two `notes`
        // elements and directly under the level alike, and not over another
        // anchor.
        let note = "<note topic=\"miscellaneous\"><heading>Same</heading><p>Words.</p></note>";
        let modules = section(&format!(
            "{note}<content>Text.</content><notes type=\"uscNote\">{note}</notes>\
             <paragraph identifier=\"/us/usc/t99/s1/1\"><content>Kept.</content>{note}\
             </paragraph><notes type=\"uscNote\">{note}</notes>"
        ));
        assert_eq!(
            addresses(&unit_at(&modules, "/us/usc/t99/s1").notes),
            [
                "/us/usc/t99/s1/note/ungrouped/nolaw/same",
                "/us/usc/t99/s1/note/ungrouped/nolaw/same/2",
                "/us/usc/t99/s1/1/note/ungrouped/nolaw/same",
                "/us/usc/t99/s1/note/ungrouped/nolaw/same/3",
            ]
        );
        // The note a cross heading holds comes first, where it stands.
        let modules = read_fixture("notes-group-headings");
        assert_eq!(
            addresses(&unit_at(&modules, "/us/usc/t99/s1").notes)[..2],
            [
                "/us/usc/t99/s1/note/editorialNotes/pl.999.11/noheading",
                "/us/usc/t99/s1/note/editorialNotes/pl.999.11/noheading/2"
            ]
        );
    }

    #[test]
    fn a_note_with_no_law_is_named_nolaw_and_its_heading() {
        let notes = notes_of(
            "<note topic=\"miscellaneous\"><heading>Heading Without Law</heading><p>No law.</p>\
             </note>\
             <note topic=\"miscellaneous\"><heading>  Section 5–A (“Sic”) &amp; Ünïcode, 2099 \
             </heading><p>Words.</p></note>\
             <note topic=\"miscellaneous\"><heading>See <ref href=\"/us/usc/t99/s2\">Section 2\
             </ref></heading><p>Words.</p></note>",
        );
        assert_eq!(
            addresses(&notes),
            [
                "/us/usc/t99/s1/note/ungrouped/nolaw/heading-without-law",
                "/us/usc/t99/s1/note/ungrouped/nolaw/section-5-a-sic-n-code-2099",
                "/us/usc/t99/s1/note/ungrouped/nolaw/see-section-2",
            ]
        );
    }

    #[test]
    fn a_note_with_a_law_and_no_heading_is_named_noheading() {
        let notes = notes_of(
            "<note topic=\"miscellaneous\"><p><ref href=\"/us/act/2099-01-01/ch9\">Act Jan. 1, \
             2099, ch. 9</ref>.</p></note>\
             <note topic=\"miscellaneous\"><heading>“—”</heading><p><ref \
             href=\"/us/pl/999/50\">Pub. L. 999–50</ref>.</p></note>\
             <note topic=\"miscellaneous\"><heading> </heading><p><ref href=\"/us/pl/999/50\">Pub. \
             L. 999–50</ref>.</p></note>",
        );
        assert_eq!(
            addresses(&notes),
            [
                "/us/usc/t99/s1/note/ungrouped/act.2099-01-01.ch9/noheading",
                "/us/usc/t99/s1/note/ungrouped/pl.999.50/noheading",
                "/us/usc/t99/s1/note/ungrouped/pl.999.50/noheading/2",
            ]
        );
        // A heading that leaves nothing is kept as the note's heading.
        assert_eq!(notes[1].heading, Some(plain("“—”")));
        assert_eq!(notes[2].heading, None);
    }

    #[test]
    fn a_note_with_neither_is_named_by_its_hash() {
        let named_by_hash = |note: &Note| {
            let hash = hash_note(note).to_string();
            let expected = format!("h.{}", &hash[..16]);
            assert_eq!(
                note.address.as_str().rsplit('/').next(),
                Some(expected.as_str()),
                "{note:?}"
            );
        };
        // The two addresses the fixtures pin were computed from frames
        // written out by hand.
        let modules = read_fixture("notes-addresses");
        let note = unit_at(&modules, "/us/usc/t99/s1")
            .notes
            .iter()
            .find(|note| {
                note.address.as_str()
                    == "/us/usc/t99/s1/note/statutoryNotes/nolaw/h.45c6a464dec81d35"
            })
            .expect("the note named by its hash");
        named_by_hash(note);
        let modules = read_fixture("notes-group-headings");
        let note = unit_at(&modules, "/us/usc/t99/s1")
            .notes
            .iter()
            .find(|note| {
                note.address.as_str()
                    == "/us/usc/t99/s1/note/statutoryNotes/nolaw/h.487c6549f4263bf1"
            })
            .expect("the cross heading's note");
        named_by_hash(note);
        // A heading that leaves nothing, with no law; its address follows its
        // text, and two such notes with one text are counted.
        let one = "<note topic=\"miscellaneous\"><heading>§</heading><p>One text.</p></note>";
        let notes = notes_of(&format!(
            "{one}<note topic=\"miscellaneous\"><p>Other text.</p></note>{one}"
        ));
        notes.iter().take(2).for_each(named_by_hash);
        assert_ne!(notes[0].address, notes[1].address);
        assert_eq!(notes[2].address.as_str(), format!("{}/2", notes[0].address));
    }

    #[test]
    fn a_long_heading_is_cut_to_sixty_characters() {
        let modules = read_fixture("notes-addresses");
        assert!(
            unit_at(&modules, "/us/usc/t99/s1")
                .notes
                .iter()
                .any(|note| {
                    note.address.as_str()
                        == "/us/usc/t99/s1/note/statutoryNotes/nolaw/\
                    a-heading-that-is-much-longer-than-sixty-characters-and-the"
                })
        );
        // Cut where a run of other characters begins, it keeps no `-` at its
        // end.
        let heading = format!("{} B", "A".repeat(59));
        let notes = notes_of(&format!(
            "<note topic=\"miscellaneous\"><heading>{heading}</heading><p>Words.</p></note>\
             <note topic=\"miscellaneous\"><heading>{}</heading><p>Words.</p></note>",
            "Z".repeat(61)
        ));
        assert_eq!(
            addresses(&notes),
            [
                format!("/us/usc/t99/s1/note/ungrouped/nolaw/{}", "a".repeat(59)),
                format!("/us/usc/t99/s1/note/ungrouped/nolaw/{}", "z".repeat(60)),
            ]
        );
    }

    #[test]
    fn inserting_a_note_changes_no_other_notes_address_unless_they_agree_in_group_law_and_heading()
    {
        let report = |section: u32| {
            format!(
                "<note topic=\"miscellaneous\"><heading>Report</heading><p><ref \
                 href=\"/us/pl/999/8/s{section}\">Pub. L. 999–8, § {section}</ref>.</p></note>"
            )
        };
        let others = "<note topic=\"miscellaneous\"><heading>Other</heading><p>Words.</p></note>\
                      <note topic=\"miscellaneous\"><p>Neither.</p></note>\
                      <note role=\"crossHeading\" topic=\"statutoryNotes\"><heading>S</heading>\
                      </note><note topic=\"miscellaneous\"><heading>Report</heading><p><ref \
                      href=\"/us/pl/999/8\">Pub. L. 999–8</ref>.</p></note>";
        let before = notes_of(&format!("{}{}{others}", report(3), report(4)));
        // A note that differs from every other in group, law or heading.
        for inserted in [
            "<note topic=\"miscellaneous\"><heading>Inserted</heading><p>Words.</p></note>"
                .to_owned(),
            "<note topic=\"miscellaneous\"><heading>Report</heading><p>No law.</p></note>"
                .to_owned(),
            "<note topic=\"miscellaneous\"><heading>Report</heading><p><ref \
             href=\"/us/pl/999/9\">Pub. L. 999–9</ref>.</p></note>"
                .to_owned(),
        ] {
            let after = notes_of(&format!("{inserted}{}{}{others}", report(3), report(4)));
            assert_eq!(addresses(&after[1..]), addresses(&before), "{inserted}");
        }
        // A note that agrees with two others in all three parts renumbers
        // them, and no other.
        let after = notes_of(&format!("{}{}{}{others}", report(5), report(3), report(4)));
        let mut expected = addresses(&before);
        expected[0] = "/us/usc/t99/s1/note/ungrouped/pl.999.8/report/2".to_owned();
        expected[1] = "/us/usc/t99/s1/note/ungrouped/pl.999.8/report/3".to_owned();
        assert_eq!(addresses(&after[1..]), expected);
        assert_eq!(
            after[0].address.as_str(),
            "/us/usc/t99/s1/note/ungrouped/pl.999.8/report"
        );
    }

    #[test]
    fn the_release_note_and_the_tables_of_contents_are_counted_and_not_imported() {
        assert_fixture("not-imported");
        let (modules, counted) =
            crate::ingest_xml(MEMBER, &crate::testing::fixture("not-imported"), RELEASE)
                .expect("read");
        assert_eq!(
            crate::testing::not_imported_counts(&counted),
            [1, 1, 1, 1, 0, 0]
        );
        // Neither the release note nor the footnote in the table of contents
        // is imported.
        let title = container_at(&modules, "/us/usc/t99");
        assert!(title.notes.is_empty() && title.footnotes.is_empty());
        // A note under a title that does not begin `Current through `, and
        // one under a section that does, are notes.
        let modules = Document::with_body(
            "<note topic=\"enacting\"><p>This title was enacted by an invented act.</p></note>\
             <section identifier=\"/us/usc/t99/s1\"><content>Text.</content><note \
             topic=\"miscellaneous\"><p>Current through 999-1</p></note></section>"
                .to_owned(),
        )
        .read()
        .expect("read");
        assert_eq!(container_at(&modules, "/us/usc/t99").notes.len(), 1);
        assert_eq!(unit_at(&modules, "/us/usc/t99/s1").notes.len(), 1);
        // A marker that points at a footnote of a table of contents is
        // refused: that footnote is not one of the module's.
        let error = refusal(&Document::with_section(
            "<toc><layout><tocItem><column>A<note type=\"footnote\" id=\"t\"><num>1</num> X.\
             </note></column></tocItem></layout></toc><content>Text<ref class=\"footnoteRef\" \
             idref=\"t\">1</ref>.</content>",
        ));
        assert!(error.message().contains("names no footnote"), "{error}");
    }

    #[test]
    fn a_table_of_contents_inside_another_counts_each_toc_once() {
        assert_fixture("toc-nested");
        let (_, counted) =
            crate::ingest_xml(MEMBER, &crate::testing::fixture("toc-nested"), RELEASE)
                .expect("read");
        assert_eq!(
            crate::testing::not_imported_counts(&counted),
            [2, 1, 1, 0, 0, 0]
        );
    }

    #[test]
    fn a_footnote_inside_the_release_note_is_left_out_with_the_note() {
        assert_fixture("release-note-footnote");
        let (modules, counted) = crate::ingest_xml(
            MEMBER,
            &crate::testing::fixture("release-note-footnote"),
            RELEASE,
        )
        .expect("read");
        assert_eq!(counted.release_notes, 1);
        // The footnote of the credit is the title's first, and its marker
        // names it.
        let title = container_at(&modules, "/us/usc/t99");
        let footnotes: Vec<(u32, &str)> = title
            .footnotes
            .iter()
            .map(|footnote| (footnote.ordinal, footnote.shown.as_str()))
            .collect();
        assert_eq!(footnotes, [(1, "2")]);
        assert_eq!(
            title.credit,
            Some(Text {
                runs: vec![words("(Invented."), mark("2", 1), words(")")]
            })
        );
        // A marker outside the release note that points at its footnote names
        // no footnote of the module.
        let error = refusal(&Document::with_body(
            "<note topic=\"miscellaneous\"><p>Current through 999-1<note type=\"footnote\" \
             id=\"r\"><num>1</num> X.</note></p></note><sourceCredit>(A<ref \
             class=\"footnoteRef\" idref=\"r\">1</ref>.)</sourceCredit>"
                .to_owned(),
        ));
        assert!(error.message().contains("names no footnote"), "{error}");
    }

    #[test]
    fn a_content_directly_in_a_note_gives_the_blocks_of_its_parts() {
        assert_fixture("notes-split-content");
        let modules = read_fixture("notes-split-content");
        let note = unit_at(&modules, "/us/usc/t99/s1")
            .notes
            .iter()
            .find(|note| note.address.as_str().ends_with("/split-content"))
            .expect("the note");
        // The continuation and the proviso inside the content give their
        // blocks in their place, and no role word is kept.
        assert_eq!(
            note.blocks,
            [
                "“(1) a widget;",
                "and a gadget;",
                "both small",
                "Provided, That the widget is red.”",
                "Closing words.",
            ]
            .map(|text| para(vec![words(text)]))
        );
    }

    #[test]
    fn a_source_credit_keeps_its_links() {
        assert_fixture("source-credit");
        let modules = read_fixture("source-credit");
        let credit = unit_at(&modules, "/us/usc/t99/s1")
            .credit
            .clone()
            .expect("a credit");
        assert_eq!(
            credit.runs,
            [
                words("("),
                Run::Ref {
                    words: "Pub. L. 999–10, § 1".to_owned(),
                    target: "/us/pl/999/10/s1".to_owned()
                },
                words(", Feb. 3, 2099, "),
                Run::Ref {
                    words: "99 Stat. 2".to_owned(),
                    target: "/us/stat/99/2".to_owned()
                },
                words(".)"),
            ]
        );
        assert!(container_at(&modules, "/us/usc/t99").credit.is_some());
        // A credit with no text is absent; one that holds only a marker is
        // kept, with an empty text.
        let modules = read_fixture("empty-labels");
        assert_eq!(unit_at(&modules, "/us/usc/t99/s1").credit, None);
        let modules = read_fixture("marker-labels");
        assert_eq!(
            unit_at(&modules, "/us/usc/t99/s1").credit,
            Some(Text {
                runs: vec![mark("3", 3)]
            })
        );
    }

    #[test]
    fn a_footnote_in_operative_words_is_editorial_matter() {
        assert_fixture("footnotes");
        let modules = read_fixture("footnotes");
        let unit = unit_at(&modules, "/us/usc/t99/s1");
        let Item::Part { blocks, .. } = &unit.root.items[0] else {
            panic!("{:?}", unit.root.items);
        };
        assert_eq!(
            blocks,
            &[para(vec![
                words("The keeper shall keep"),
                mark("3", 3),
                words(" the gate and"),
                mark("3", 3),
                words(" the key."),
            ])]
        );
        let footnotes: Vec<(u32, &str)> = unit
            .footnotes
            .iter()
            .map(|footnote| (footnote.ordinal, footnote.shown.as_str()))
            .collect();
        assert_eq!(footnotes, [(1, "1"), (2, "2"), (3, "3")]);
        assert_eq!(
            unit.footnotes[2].blocks,
            [para(vec![words(
                "So in original. Probably should be “keep and guard”."
            )])]
        );
        // The provision's hash is that of the same text without the marker
        // and the footnote; the editorial layer holds them.
        let with = section(
            "<content>The keeper shall keep<ref class=\"footnoteRef\" idref=\"f\">1</ref><note \
             type=\"footnote\" id=\"f\"><num>1</num> So in original.</note> the gate.</content>",
        );
        let without = section("<content>The keeper shall keep the gate.</content>");
        let (with, without) = (
            unit_at(&with, "/us/usc/t99/s1"),
            unit_at(&without, "/us/usc/t99/s1"),
        );
        assert_eq!(hash_provision(&with.root), hash_provision(&without.root));
        assert_ne!(with.root, without.root);
        let (a, b) = (layer_hashes_of_unit(with), layer_hashes_of_unit(without));
        assert_eq!((a.operative, a.enacted), (b.operative, b.enacted));
        assert_ne!(a.editorial, b.editorial);
    }

    #[test]
    fn a_quoted_section_keeps_its_structure_and_has_no_address() {
        assert_fixture("notes-content");
        let modules = read_fixture("notes-content");
        let unit = unit_at(&modules, "/us/usc/t99/s1");
        let note = unit
            .notes
            .iter()
            .find(|note| note.address.as_str().ends_with("/grant-program"))
            .expect("the note");
        let Block::Quoted(quoted) = &note.blocks[1] else {
            panic!("{:?}", note.blocks);
        };
        assert_eq!(quoted.origin.as_deref(), Some("/us/pl/999/5/s2"));
        let [
            QuotedItem::Level(quoted_section),
            QuotedItem::Block(closing),
        ] = quoted.items.as_slice()
        else {
            panic!("{:?}", quoted.items);
        };
        assert_eq!(closing, &para(vec![words("”")]));
        assert_eq!(quoted_section.kind, "section");
        let shape: Vec<String> = quoted_section
            .items
            .iter()
            .map(|item| match item {
                QuotedItem::Level(level) => format!(
                    "{} {}",
                    level.kind,
                    level
                        .num
                        .as_ref()
                        .map(|num| num.value.as_str())
                        .unwrap_or("")
                ),
                QuotedItem::Block(Block::Para(text)) => text.plain(),
                other => format!("{other:?}"),
            })
            .collect();
        // The bracketed note inside the quoted section is replaced by its
        // blocks.
        assert_eq!(
            shape,
            [
                "subsection a",
                "subsection b",
                "[Subsection (c) was struck out.]"
            ]
        );
        // The quoted levels add no provision and no address.
        assert_eq!(crate::testing::Counts::of(&modules).provisions, 1);
        assert!(
            crate::testing::records(&modules)
                .iter()
                .all(|line| !line.contains("\"address\":\"/us/usc/t99/s1/s2"))
        );
        assert_fixture("empty-labels");
        assert_fixture("subheadings");
    }

    #[test]
    fn a_note_with_a_table() {
        let modules = read_fixture("notes-content");
        let note = unit_at(&modules, "/us/usc/t99/s1")
            .notes
            .iter()
            .find(|note| note.address.as_str().ends_with("/table-of-amounts"))
            .expect("the note");
        let [Block::Table(table)] = note.blocks.as_slice() else {
            panic!("{:?}", note.blocks);
        };
        let cells: Vec<Vec<Block>> = table.rows[0]
            .cells
            .iter()
            .map(|cell| cell.blocks.clone())
            .collect();
        assert_eq!(
            cells,
            [
                vec![para(vec![words("2099")])],
                vec![para(vec![words("Ten coins")])]
            ]
        );
        // A table cell in a note is a flow of the note.
        let notes = notes_of(
            "<note topic=\"miscellaneous\"><heading>Cells</heading><table \
             xmlns=\"http://www.w3.org/1999/xhtml\"><tr><td><p xmlns=\"http://xml.house.gov/schemas/uslm/1.0\">A \
             <quotedContent origin=\"/us/pl/999/60\">“B.”</quotedContent></p></td></tr></table>\
             </note>",
        );
        assert_eq!(
            addresses(&notes),
            ["/us/usc/t99/s1/note/ungrouped/pl.999.60/cells"]
        );
    }

    #[test]
    fn a_note_with_a_signature_and_a_second_heading() {
        let modules = read_fixture("notes-content");
        let notes = &unit_at(&modules, "/us/usc/t99/s1").notes;
        let blocks = |suffix: &str| {
            notes
                .iter()
                .find(|note| note.address.as_str().ends_with(suffix))
                .unwrap_or_else(|| panic!("{suffix}"))
                .blocks
                .clone()
        };
        assert_eq!(
            blocks("/invented-order"),
            [
                para(vec![words("The order is made.")]),
                para(vec![words("A. Person.")]),
                para(vec![words("Invented Officer.")]),
            ]
        );
        assert_eq!(
            blocks("/two-parts"),
            [
                para(vec![words("The first part.")]),
                Block::Subheading(plain("Second Part")),
                para(vec![words("The second part.")]),
            ]
        );
        // A proviso that stands directly in a note is replaced by its blocks.
        assert_eq!(
            blocks("/loose-provisions"),
            [
                para(vec![words("“(1) a widget;")]),
                para(vec![words("Provided, That the widget is small.”")]),
            ]
        );
        // A signature in quoted content inside a note gives its paragraphs.
        let notes = notes_of(
            "<note topic=\"executiveOrder\"><heading>Order</heading><p>It reads: <quotedContent>\
             <p>“Ordered.</p><signature><name>B. Person.”</name></signature></quotedContent></p>\
             </note>",
        );
        let Block::Quoted(quoted) = &notes[0].blocks[1] else {
            panic!("{:?}", notes[0].blocks);
        };
        assert_eq!(
            quoted.items,
            [
                QuotedItem::Block(para(vec![words("“Ordered.")])),
                QuotedItem::Block(para(vec![words("B. Person.”")])),
            ]
        );
    }

    #[test]
    #[allow(non_snake_case)]
    fn the_three_layer_hashes_follow_S5() {
        let unit = |content: &str, credit: &str, enacted: &str, editorial: &str, footnote: &str| {
            let modules = section(&format!(
                "<content>{content}<note type=\"footnote\" id=\"f\"><num>1</num> {footnote}</note>\
                 </content><sourceCredit>({credit})</sourceCredit><notes type=\"uscNote\">\
                 <note role=\"crossHeading\" topic=\"editorialNotes\"><heading>E</heading></note>\
                 <note topic=\"amendments\"><heading>Amendments</heading><p>{editorial}</p></note>\
                 <note role=\"crossHeading\" topic=\"statutoryNotes\"><heading>S</heading></note>\
                 <note topic=\"effectiveDate\"><heading>Effective Date</heading><p>{enacted}</p>\
                 </note></notes>"
            ));
            layer_hashes_of_unit(unit_at(&modules, "/us/usc/t99/s1"))
        };
        let marked = "Kept<ref class=\"footnoteRef\" idref=\"f\">1</ref>.";
        let base = unit(marked, "Credit.", "Enacted.", "Editorial.", "Footnote.");
        let changed = |layers: fidryn_provision::LayerHashes| {
            [
                layers.operative != base.operative,
                layers.enacted != base.enacted,
                layers.editorial != base.editorial,
            ]
        };
        assert_eq!(
            changed(unit(
                marked,
                "Credit.",
                "Changed.",
                "Editorial.",
                "Footnote."
            )),
            [false, true, false]
        );
        for (label, layers) in [
            (
                "a credit",
                unit(marked, "Changed.", "Enacted.", "Editorial.", "Footnote."),
            ),
            (
                "an editorial note",
                unit(marked, "Credit.", "Enacted.", "Changed.", "Footnote."),
            ),
            (
                "a footnote",
                unit(marked, "Credit.", "Enacted.", "Editorial.", "Changed."),
            ),
            (
                "a marker",
                unit("Kept.", "Credit.", "Enacted.", "Editorial.", "Footnote."),
            ),
        ] {
            assert_eq!(changed(layers), [false, false, true], "{label}");
        }
        assert_eq!(
            changed(unit(
                "Changed<ref class=\"footnoteRef\" idref=\"f\">1</ref>.",
                "Credit.",
                "Enacted.",
                "Editorial.",
                "Footnote."
            )),
            [true, false, false]
        );
        // The order of the notes in the file is in no layer hash.
        let swapped = |first: &str, second: &str| {
            let modules = section(&format!(
                "<content>Text.</content><notes type=\"uscNote\">{first}{second}</notes>"
            ));
            layer_hashes_of_unit(unit_at(&modules, "/us/usc/t99/s1"))
        };
        let a = "<note topic=\"x\"><heading>A</heading><p>A.</p></note>";
        let b = "<note topic=\"x\"><heading>B</heading><p>B.</p></note>";
        assert_eq!(swapped(a, b), swapped(b, a));
        // A container's notes and credit are in its own layers.
        let chapter = |note: &str, credit: &str| {
            let modules = Document::with_body(format!(
                "<chapter identifier=\"/us/usc/t99/ch1\"><num value=\"1\">CHAPTER 1</num>\
                 <sourceCredit>({credit})</sourceCredit><notes type=\"uscNote\"><note \
                 topic=\"miscellaneous\"><heading>Note</heading><p>{note}</p></note></notes>\
                 </chapter>"
            ))
            .read()
            .expect("read");
            layer_hashes_of_container(container_at(&modules, "/us/usc/t99/ch1"))
        };
        let base = chapter("Words.", "Credit.");
        let other = chapter("Changed.", "Credit.");
        assert_eq!(
            [
                other.operative == base.operative,
                other.enacted == base.enacted,
                other.editorial == base.editorial
            ],
            [true, false, true]
        );
        let other = chapter("Words.", "Changed.");
        assert_eq!(
            [
                other.operative == base.operative,
                other.enacted == base.enacted,
                other.editorial == base.editorial
            ],
            [true, true, false]
        );
    }

    #[test]
    fn what_the_rules_refuse_in_notes_credits_and_footnotes_is_refused() {
        let in_notes = |notes: &str| {
            format!("<content>Text.</content><notes type=\"uscNote\">{notes}</notes>")
        };
        let cases: Vec<(String, &str)> = vec![
            (
                "<content>Text<ref class=\"footnoteRef\" idref=\"none\">1</ref>.</content>"
                    .to_owned(),
                "names no footnote",
            ),
            (
                "<content>Text<ref class=\"footnoteRef\">1</ref>.</content>".to_owned(),
                "names no footnote",
            ),
            (
                in_notes(
                    "<note topic=\"x\"><p>Text<ref class=\"footnoteRef\" idref=\"f\"><b>1</b>\
                     </ref><note type=\"footnote\" id=\"f\"><num>1</num> X.</note></p></note>",
                ),
                "a footnote marker that holds an element",
            ),
            (
                "<content>Text.</content><sourceCredit>(One.)</sourceCredit>\
                 <sourceCredit>(Two.)</sourceCredit>"
                    .to_owned(),
                "a second source credit",
            ),
            (
                in_notes(
                    "<note topic=\"x\"><p>Text.</p><sourceCredit>(One.)</sourceCredit></note>",
                ),
                "<sourceCredit> does not stand in <note>",
            ),
            (
                in_notes("<note role=\"crossHeading\"><heading>Notes</heading></note>"),
                "not one word of ASCII letters and digits",
            ),
            (
                in_notes(
                    "<note role=\"crossHeading\" topic=\"editorial-notes\"><heading>Notes\
                     </heading></note>",
                ),
                "not one word of ASCII letters and digits",
            ),
            (
                in_notes(
                    "<note role=\"crossHeading\" topic=\"9notes\"><heading>Notes</heading></note>",
                ),
                "not one word of ASCII letters and digits",
            ),
            (
                in_notes(
                    "<note role=\"crossHeading\" topic=\"editorialNotes statutoryNotes\">\
                     <heading>Notes</heading></note>",
                ),
                "not one word of ASCII letters and digits",
            ),
            (
                in_notes(
                    "<note topic=\"amendments\u{A0}effectiveDate\"><heading>Odd</heading>\
                     <p>Text.</p></note>",
                ),
                "holds a white-space character other than U+0020",
            ),
            (
                in_notes(
                    "<note topic=\"x\"><heading>Odd</heading><p><ref href=\"/us/pl/999\">Pub. L. \
                     999</ref></p></note>",
                ),
                "fewer than four segments",
            ),
            (
                in_notes(
                    "<note topic=\"x\"><heading>Odd</heading><p><ref href=\"/us/pl/999/1(a)\">Pub. \
                     L. 999–1</ref></p></note>",
                ),
                "a character an address segment cannot hold",
            ),
            (
                in_notes(
                    "<note topic=\"x\"><heading>Odd</heading><p><ref href=\"/us/act//ch1\">An \
                     act</ref></p></note>",
                ),
                "a character an address segment cannot hold",
            ),
            (
                in_notes(
                    "<note topic=\"x\"><heading>Odd</heading><p>Read: <quotedContent \
                     origin=\"/us/stat/99\">“A.”</quotedContent></p></note>",
                ),
                "fewer than four segments",
            ),
            (
                in_notes(
                    "<note topic=\"x\"><heading>Signed</heading><signature><name>A. Person.</name>\
                     <note type=\"footnote\" id=\"f\"><num>1</num> X.</note></signature></note>",
                ),
                "a footnote that stands directly in a signature",
            ),
            (
                in_notes(
                    "<note topic=\"x\"><heading>Signed</heading><signature><name>A. Person.</name>\
                     <date>2099</date></signature></note>",
                ),
                "a signature holds only name and role",
            ),
            (
                "<content>A<note type=\"footnote\" id=\"f\"><num>1</num> X.</note> b<note \
                 type=\"footnote\" id=\"f\"><num>2</num> Y.</note>.</content>"
                    .to_owned(),
                "two footnotes of the module have the id",
            ),
        ];
        for (inner, phrase) in cases {
            let error = refusal(&Document::with_section(&inner));
            assert!(error.message().contains(phrase), "{inner}: {error}");
            assert_eq!(
                error.identifier(),
                Some("/us/usc/t99/s1"),
                "{inner}: {error}"
            );
        }
        // A note and a provision never share an address.
        let error = refusal(&Document::with_section(
            "<subsection identifier=\"/us/usc/t99/s1/note\"><paragraph \
             identifier=\"/us/usc/t99/s1/note/ungrouped\"><clause \
             identifier=\"/us/usc/t99/s1/note/ungrouped/nolaw\"><item \
             identifier=\"/us/usc/t99/s1/note/ungrouped/nolaw/x\"><content>A.</content></item>\
             </clause></paragraph></subsection><note topic=\"x\"><heading>X</heading><p>B.</p>\
             </note>",
        ));
        assert!(
            error.message().contains(
                "/us/usc/t99/s1/note/ungrouped/nolaw/x is the address of two elements: a \
                 provision and a note"
            ),
            "{error}"
        );
        // A container has one credit, and a credit stands nowhere else in it.
        for (body, phrase) in [
            (
                "<sourceCredit>(One.)</sourceCredit><sourceCredit>(Two.)</sourceCredit>",
                "a second source credit",
            ),
            (
                "<notes type=\"uscNote\"><note topic=\"x\"><p>A.</p><sourceCredit>(One.)\
                 </sourceCredit></note></notes>",
                "<sourceCredit> does not stand in <note>",
            ),
        ] {
            let error = refusal(&Document::with_body(format!(
                "<chapter identifier=\"/us/usc/t99/ch1\"><num value=\"1\">CHAPTER 1</num>{body}\
                 </chapter>"
            )));
            assert!(error.message().contains(phrase), "{body}: {error}");
            assert_eq!(error.identifier(), Some("/us/usc/t99/ch1"), "{error}");
        }
        // A marker in one unit that points at a footnote of another.
        let error = refusal(&Document::with_body(
            "<section identifier=\"/us/usc/t99/s1\"><content>A<note type=\"footnote\" id=\"f\">\
             <num>1</num> X.</note>.</content></section><section identifier=\"/us/usc/t99/s2\">\
             <content>B<ref class=\"footnoteRef\" idref=\"f\">1</ref>.</content></section>"
                .to_owned(),
        ));
        assert!(error.message().contains("names no footnote"), "{error}");
        assert_eq!(error.identifier(), Some("/us/usc/t99/s2"));
        // A part of a law reference past the fourth segment is not read.
        let notes = notes_of(
            "<note topic=\"x\"><heading>Deep</heading><p><ref href=\"/us/pl/999/1/s2(a)\">Pub. \
             L. 999–1</ref></p></note>",
        );
        assert_eq!(
            addresses(&notes),
            ["/us/usc/t99/s1/note/ungrouped/pl.999.1/deep"]
        );
    }
}
