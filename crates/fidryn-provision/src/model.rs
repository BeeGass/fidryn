//! The model of a law module.
//!
//! A module has one of three shapes ([`Module`]). A unit holds one tree of
//! provisions, with its credit, notes, footnotes and definitions; a container
//! lists its members, the containers and units below it, with its own credit,
//! notes and footnotes; a curated module imports modules, cites provisions by
//! name and binds names to them.
//!
//! Every text is a [`Text`]: a list of runs whose words, concatenated, are the
//! text of its block (see [`crate::normalize`]). Kinds, status words, groups
//! and topics are identifiers that the model does not interpret.
//!
//! In JSON every type has the form `serde` derives: a struct is an object of
//! its fields, `None` is `null`, a variant without fields is its name in snake
//! case, and any other variant is an object with that name as its one key.

use crate::address::Address;
use crate::hash::Hash32;
use serde::{Deserialize, Serialize};

/// What a text is part of: the operative text of the provisions, the notes
/// the source marks as enacted, or the editorial matter. Layers are ordered
/// as they are declared.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Operative,
    Enacted,
    Editorial,
}

/// A text: runs of words, links, superscripts, subscripts and footnote
/// markers, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Text {
    pub runs: Vec<Run>,
}

/// One run of a [`Text`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Run {
    /// Plain words.
    Words(String),
    /// A link: the linked words and the target as the source writes it.
    Ref { words: String, target: String },
    /// Superscript words.
    Sup(String),
    /// Subscript words.
    Sub(String),
    /// A footnote marker: the marker as the source shows it and the ordinal of
    /// the footnote it points at. Its text is not part of the text it stands
    /// in.
    Mark { shown: String, footnote: u32 },
}

impl Run {
    /// The words this run adds to the text it stands in; none for a footnote
    /// marker.
    pub(crate) fn words(&self) -> &str {
        match self {
            Run::Words(words) | Run::Sup(words) | Run::Sub(words) | Run::Ref { words, .. } => words,
            Run::Mark { .. } => "",
        }
    }
}

impl Text {
    /// The text of `runs`, with adjacent `Words` runs merged into one and
    /// every run whose words are empty dropped. A link with no words is
    /// dropped together with its target, so a caller that must count such
    /// links counts them before it calls. A footnote marker has no words of
    /// its own and is always kept.
    pub fn from_runs(runs: Vec<Run>) -> Text {
        let mut merged: Vec<Run> = Vec::with_capacity(runs.len());
        for run in runs {
            if run.words().is_empty() && !matches!(run, Run::Mark { .. }) {
                continue;
            }
            match (merged.last_mut(), run) {
                (Some(Run::Words(last)), Run::Words(words)) => last.push_str(&words),
                (_, run) => merged.push(run),
            }
        }
        Text { runs: merged }
    }

    /// The words of every run but the footnote markers, concatenated.
    pub fn plain(&self) -> String {
        self.runs.iter().map(Run::words).collect()
    }
}

/// A block of text.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Block {
    /// A paragraph.
    Para(Text),
    /// A heading inside a flow of blocks, below the heading of its provision
    /// or note.
    Subheading(Text),
    Table(Table),
    /// Text quoted from another source.
    Quoted(Quoted),
    /// A picture, named by its source; the picture itself is not in the model.
    Image {
        source: String,
    },
}

/// A table: its rows in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub rows: Vec<Row>,
}

/// The part of a table a row belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowGroup {
    Head,
    Body,
    Foot,
}

/// A row of a table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub group: RowGroup,
    pub cells: Vec<Cell>,
}

/// A cell of a table. A cell may hold no block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    pub header: bool,
    pub colspan: u32,
    pub rowspan: u32,
    pub blocks: Vec<Block>,
}

/// A number: its machine form and the form printed, which may hold a footnote
/// marker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Num {
    pub value: String,
    pub shown: Text,
}

/// The role of a text part: the words that lead into the children
/// (`Chapeau`), the text itself (`Content`), words that follow children
/// (`Continuation`), or a proviso.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartRole {
    Chapeau,
    Content,
    Continuation,
    Proviso,
}

/// An item of a provision: a text part, which has at least one block, or a
/// child provision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Item {
    Part { role: PartRole, blocks: Vec<Block> },
    Child(Provision),
}

/// A provision and the tree below it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provision {
    pub address: Address,
    /// An identifier, such as `section`.
    pub kind: String,
    pub num: Option<Num>,
    pub heading: Option<Text>,
    /// An identifier; `None` is the default of the source, in force.
    pub status: Option<String>,
    /// Text parts and children in document order.
    pub items: Vec<Item>,
}

/// Quoted content: where it comes from, as the source names it, and its
/// items.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quoted {
    pub origin: Option<String>,
    pub items: Vec<QuotedItem>,
}

/// An item of quoted content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum QuotedItem {
    Block(Block),
    Part { role: PartRole, blocks: Vec<Block> },
    Level(QuotedLevel),
}

/// A level of quoted content, such as a quoted section. It has no address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotedLevel {
    pub kind: String,
    pub num: Option<Num>,
    pub heading: Option<Text>,
    pub items: Vec<QuotedItem>,
}

/// A note of a unit or a container.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Note {
    pub address: Address,
    /// `Enacted` or `Editorial`.
    pub layer: Layer,
    /// The group word of the source, or `ungrouped`.
    pub group: String,
    pub topics: Vec<String>,
    pub heading: Option<Text>,
    pub blocks: Vec<Block>,
}

/// A footnote of a module, numbered from 1 in document order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Footnote {
    pub ordinal: u32,
    /// The marker as the footnote shows it.
    pub shown: String,
    pub blocks: Vec<Block>,
}

/// How a definition gives its term a meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DefinitionKind {
    Means,
    Includes,
    DoesNotInclude,
    Borrowed,
}

/// Where the reach of a definition comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Enacted,
    Restated,
    Unresolved,
}

/// A definition found in a provision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    /// The provision that holds the definition.
    pub at: Address,
    pub terms: Vec<String>,
    pub kind: DefinitionKind,
    /// The phrase of the text that states the reach; `None` when the text
    /// states none.
    pub words: Option<String>,
    /// The condition; `None` when the definition has none.
    pub condition: Option<String>,
    pub region: Option<Address>,
    pub basis: Basis,
    /// The target of the first link of the source, for a definition of kind
    /// `borrowed` with a linked source.
    pub borrowed_from: Option<String>,
    /// The source words, for a definition of kind `borrowed` with an
    /// unlinked source. A definition of kind `borrowed` has exactly one of
    /// `borrowed_from` and `borrowed_words`, and one of any other kind has
    /// neither.
    pub borrowed_words: Option<String>,
    pub see_note: Option<Address>,
}

/// A module that holds one tree of provisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnitModule {
    pub address: Address,
    /// Other addresses the source gives this unit.
    pub aliases: Vec<Address>,
    pub within: Option<Address>,
    /// The member of the source archive the unit was read from.
    pub source: String,
    pub positive_law: Option<bool>,
    /// The top provision; its address is the module's.
    pub root: Provision,
    pub credit: Option<Text>,
    /// In document order.
    pub notes: Vec<Note>,
    /// In ordinal order, from 1.
    pub footnotes: Vec<Footnote>,
    pub definitions: Vec<Definition>,
}

/// A module that lists the containers and units below it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerModule {
    pub address: Address,
    pub aliases: Vec<Address>,
    pub within: Option<Address>,
    pub source: String,
    pub positive_law: Option<bool>,
    pub kind: String,
    pub num: Option<Num>,
    pub heading: Option<Text>,
    pub status: Option<String>,
    /// Sub-containers and units in document order.
    pub members: Vec<Address>,
    pub credit: Option<Text>,
    pub notes: Vec<Note>,
    pub footnotes: Vec<Footnote>,
}

/// An import of a curated module: the module, and either an alias for it or
/// the names it brings into scope.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub module: Address,
    pub alias: Option<String>,
    pub names: Vec<String>,
}

/// A name a curated module gives to an address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub name: String,
    pub target: Address,
}

/// What a binding binds: an address, or a name, unqualified or qualified by
/// the alias of an import.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum BindTarget {
    Address(Address),
    Name {
        module: Option<String>,
        name: String,
    },
}

/// A binding of a curated module, with the release and the two hashes it was
/// made against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub name: String,
    pub binds: BindTarget,
    pub release: String,
    pub provision_hash: Hash32,
    pub notes_hash: Hash32,
}

/// A module written by hand that imports, cites and binds.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CuratedModule {
    pub address: Address,
    pub imports: Vec<Import>,
    pub citations: Vec<Citation>,
    pub bindings: Vec<Binding>,
}

/// A module of one of the three shapes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Module {
    Unit(UnitModule),
    Container(ContainerModule),
    Curated(CuratedModule),
}

/// Short constructors for the tests of this crate.
#[cfg(test)]
pub(crate) mod build {
    use super::*;

    pub(crate) fn address(text: &str) -> Address {
        Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    pub(crate) fn words(text: &str) -> Run {
        Run::Words(text.to_owned())
    }

    pub(crate) fn link(words: &str, target: &str) -> Run {
        Run::Ref {
            words: words.to_owned(),
            target: target.to_owned(),
        }
    }

    pub(crate) fn sup(text: &str) -> Run {
        Run::Sup(text.to_owned())
    }

    pub(crate) fn sub(text: &str) -> Run {
        Run::Sub(text.to_owned())
    }

    pub(crate) fn mark(shown: &str, footnote: u32) -> Run {
        Run::Mark {
            shown: shown.to_owned(),
            footnote,
        }
    }

    /// A text of the runs as given, merged or not.
    pub(crate) fn text(runs: Vec<Run>) -> Text {
        Text { runs }
    }

    /// A text of one `Words` run, or of none when `words` is empty.
    pub(crate) fn plain_text(words: &str) -> Text {
        if words.is_empty() {
            Text::default()
        } else {
            text(vec![self::words(words)])
        }
    }

    pub(crate) fn para(runs: Vec<Run>) -> Block {
        Block::Para(text(runs))
    }

    pub(crate) fn num(value: &str, shown: &str) -> Option<Num> {
        Some(Num {
            value: value.to_owned(),
            shown: plain_text(shown),
        })
    }

    pub(crate) fn part(role: PartRole, blocks: Vec<Block>) -> Item {
        Item::Part { role, blocks }
    }

    pub(crate) fn content(blocks: Vec<Block>) -> Item {
        part(PartRole::Content, blocks)
    }

    pub(crate) fn provision(
        at: &str,
        kind: &str,
        num: Option<Num>,
        heading: Option<Text>,
        items: Vec<Item>,
    ) -> Provision {
        Provision {
            address: address(at),
            kind: kind.to_owned(),
            num,
            heading,
            status: None,
            items,
        }
    }

    /// A note of the group `demoNotes` with no topic, a heading and one
    /// paragraph.
    pub(crate) fn note(at: &str, layer: Layer, heading: &str, words: &str) -> Note {
        Note {
            address: address(at),
            layer,
            group: "demoNotes".to_owned(),
            topics: Vec::new(),
            heading: Some(plain_text(heading)),
            blocks: vec![para(vec![self::words(words)])],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::*;

    #[test]
    fn adjacent_words_merge() {
        assert_eq!(
            Text::from_runs(vec![words("a"), words("b")]),
            Text::from_runs(vec![words("ab")])
        );
        assert_eq!(Text::from_runs(vec![words("ab")]), text(vec![words("ab")]));
        assert_eq!(
            Text::from_runs(vec![
                words(""),
                words("a "),
                sup(""),
                link("", "/zz/demo/t1/s2"),
                words("b"),
                mark("1", 1),
                words("c"),
                sub("2"),
                words(" d"),
                words(" e"),
            ]),
            text(vec![
                words("a b"),
                mark("1", 1),
                words("c"),
                sub("2"),
                words(" d e"),
            ])
        );
        assert_eq!(Text::from_runs(vec![words(""), sub("")]), Text::default());
    }

    #[test]
    fn plain_is_the_words_of_every_run_but_the_markers() {
        let text = text(vec![
            words("See "),
            link("section 2", "/zz/demo/t1/s2"),
            words(" of 10"),
            sup("3"),
            words(" and H"),
            sub("2"),
            words("O"),
            mark("1", 1),
            words("."),
        ]);
        assert_eq!(text.plain(), "See section 2 of 103 and H2O.");
        assert_eq!(Text::default().plain(), "");
    }

    #[test]
    fn the_json_form_is_the_one_the_known_answers_use() {
        let item = content(vec![
            para(vec![words("a "), link("b", "/zz/demo/b"), mark("1", 1)]),
            Block::Image {
                source: "demo.png".to_owned(),
            },
        ]);
        let json = serde_json::to_string(&item).expect("serialize");
        assert_eq!(
            json,
            r#"{"part":{"role":"content","blocks":[{"para":{"runs":[{"words":"a "},{"ref":{"words":"b","target":"/zz/demo/b"}},{"mark":{"shown":"1","footnote":1}}]}},{"image":{"source":"demo.png"}}]}}"#
        );
        assert_eq!(serde_json::from_str::<Item>(&json).expect("read"), item);
        let error = serde_json::from_str::<Text>(r#"{"runs":[],"words":"a"}"#)
            .expect_err("an unknown field");
        assert!(error.to_string().contains("unknown field"), "{error}");
        assert_eq!(
            serde_json::to_string(&DefinitionKind::DoesNotInclude).expect("serialize"),
            r#""does_not_include""#
        );
    }

    #[test]
    fn a_definition_writes_every_field_and_reads_without_its_condition_or_source_words() {
        let definition = Definition {
            at: address("/zz/demo/t1/s1/a"),
            terms: vec!["widget".to_owned()],
            kind: DefinitionKind::Borrowed,
            words: Some("In this section".to_owned()),
            condition: Some("in the case of a gadget".to_owned()),
            region: Some(address("/zz/demo/t1/s1")),
            basis: Basis::Enacted,
            borrowed_from: None,
            borrowed_words: Some("the Widget Act".to_owned()),
            see_note: None,
        };
        let json = serde_json::to_string(&definition).expect("serialize");
        assert_eq!(
            json,
            r#"{"at":"/zz/demo/t1/s1/a","terms":["widget"],"kind":"borrowed","words":"In this section","condition":"in the case of a gadget","region":"/zz/demo/t1/s1","basis":"enacted","borrowed_from":null,"borrowed_words":"the Widget Act","see_note":null}"#
        );
        assert_eq!(
            serde_json::from_str::<Definition>(&json).expect("read"),
            definition
        );
        // An absent value is written as `null`, and a definition written
        // without the two keys reads as one that has neither.
        let without = Definition {
            kind: DefinitionKind::Means,
            condition: None,
            borrowed_words: None,
            ..definition
        };
        let json = serde_json::to_string(&without).expect("serialize");
        assert!(
            json.contains(r#""words":"In this section","condition":null,"region""#),
            "{json}"
        );
        assert!(
            json.contains(r#""borrowed_from":null,"borrowed_words":null,"see_note":null}"#),
            "{json}"
        );
        let older = r#"{"at":"/zz/demo/t1/s1/a","terms":["widget"],"kind":"means","words":"In this section","region":"/zz/demo/t1/s1","basis":"enacted","borrowed_from":null,"see_note":null}"#;
        assert_eq!(
            serde_json::from_str::<Definition>(older).expect("read"),
            without
        );
    }
}
