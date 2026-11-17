//! The interface of a module: everything that the checks across the
//! modules of a workspace, the coverage ledger and the change sets read of
//! a module, so that none of them reads its source again.
//!
//! [`interface_of`] builds the interface of a lowered module from the model
//! and the byte ranges of its declarations ([`DeclSpans`]). It holds:
//!
//! - the module's address, shape, aliases, `within`, `source` and
//!   `positive_law`; for a container its kind, status and members;
//! - each provision's address, kind, status and tree hash, and whether it
//!   holds an image; each note's address, layer, group and hash, and whether
//!   it holds an image; the number of footnotes; the three layer hashes;
//! - each reference's layer, the provision or note it stands in, and its
//!   target ([`Reference`]);
//! - each definition's provision, terms, kind and basis;
//! - for a curated module its imports, citations and bindings;
//! - the byte range of the module's address, of `within`, of each alias and
//!   member, and of each import, citation and binding.
//!
//! An interface is small: the address of a provision, a note or the
//! provision of a definition is written relative to the module's
//! ([`Interface::address_at`]), and each kind, status and group is written
//! once, in [`Interface::words`], and named by its place there.
//!
//! [`Interface::to_json`] writes it as one line of JSON, `{"schema":
//! "fidryn.interface/v0.1", "interface": {...}}`, with the fields of
//! [`Interface`] in their order. [`Interface::from_json`] reads back what it
//! writes, and nothing else.

use crate::address::{self, Address};
use crate::hash::{
    Hash32, LayerHashes, hash_note, layer_hashes_of_container, layer_hashes_of_unit, tree_hashes,
};
use crate::model::{
    Basis, Binding, Block, Citation, DefinitionKind, Footnote, Import, Item, Layer, Module, Note,
    Provision, QuotedItem, Run, Text,
};
use crate::workspace::as_written;
use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize};
use std::fmt;

/// The `schema` of an interface.
pub const INTERFACE_SCHEMA: &str = "fidryn.interface/v0.1";

/// The shape of a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Unit,
    Container,
    Curated,
}

impl Shape {
    /// The shape as a sentence names it: `a unit`, `a container` or `a
    /// curated module`.
    pub fn described(self) -> &'static str {
        match self {
            Shape::Unit => "a unit",
            Shape::Container => "a container",
            Shape::Curated => "a curated module",
        }
    }
}

/// The bytes `start..end` of a module's source; `end` is exclusive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteRange {
    pub start: u32,
    pub end: u32,
}

/// Where the declarations of a module stand in its source, each list in
/// the order of the declarations. A range of a string covers its quotation
/// marks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeclSpans {
    /// The module's address, after the word `module`.
    pub address: ByteRange,
    /// The address after `within`.
    pub within: Option<ByteRange>,
    /// The address after each `alias`.
    pub aliases: Vec<ByteRange>,
    /// The address after each `member` of a container.
    pub members: Vec<ByteRange>,
    /// Each `import` declaration, whole.
    pub imports: Vec<ByteRange>,
    /// Each `citation` declaration, whole.
    pub citations: Vec<ByteRange>,
    /// Each `binding` declaration, whole.
    pub bindings: Vec<ByteRange>,
}

/// A declaration of a module and its byte range in the module's source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Declared<T> {
    pub item: T,
    pub range: ByteRange,
}

/// A word of [`Interface::words`], named by its place there, counted from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Word(pub u32);

/// What an interface holds of a container module.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerEntry {
    pub kind: String,
    pub status: Option<String>,
    /// The address after each `member`, in order.
    pub members: Vec<Declared<Address>>,
}

/// What an interface holds of a provision of a unit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisionEntry {
    /// The provision's address relative to the module's
    /// ([`Interface::address_at`]).
    pub at: Box<str>,
    pub kind: Word,
    pub status: Option<Word>,
    /// The tree hash of the provision.
    pub hash: Hash32,
    /// Whether a text part of the provision, not of a child, holds an image.
    pub image: bool,
}

/// What an interface holds of a note.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoteEntry {
    /// The note's address relative to the module's
    /// ([`Interface::address_at`]).
    pub at: Box<str>,
    pub layer: Layer,
    pub group: Word,
    pub hash: Hash32,
    /// Whether a block of the note holds an image.
    pub image: bool,
}

/// What a reference stands in: the module itself (a source credit, a
/// footnote, the number or heading of a container), or a provision or a
/// note, named by its place in [`Interface::provisions`] or
/// [`Interface::notes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Place {
    Module,
    Provision(u32),
    Note(u32),
}

/// A reference: a link of a text, with the target as the source writes it.
/// Its layer is that of what it stands in: operative in a provision and in
/// the number or heading of a container, the note's own layer in a note,
/// quoted content included, and editorial in a source credit and in a
/// footnote.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub layer: Layer,
    pub from: Place,
    pub target: Box<str>,
}

/// What an interface holds of a definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefinitionEntry {
    /// The address of the provision that holds the definition, relative to
    /// the module's ([`Interface::address_at`]).
    pub at: Box<str>,
    pub terms: Vec<String>,
    pub kind: DefinitionKind,
    pub basis: Basis,
}

/// The interface of a module. [`interface_of`] builds it, and
/// [`Interface::from_json`] reads it; either gives one whose words, places
/// and relative addresses name what is there, which the methods rely on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    pub address: Address,
    /// The module's address after the word `module`.
    pub address_range: ByteRange,
    pub shape: Shape,
    pub aliases: Vec<Declared<Address>>,
    pub within: Option<Declared<Address>>,
    /// `None` for a curated module.
    pub source: Option<String>,
    pub positive_law: Option<bool>,
    /// `Some` for a container module only.
    pub container: Option<ContainerEntry>,
    /// The kinds and statuses of the provisions and the groups of the notes,
    /// each once, in the order in which they first occur.
    pub words: Vec<Box<str>>,
    /// The provisions of a unit in document order, each before its children;
    /// the first is the unit's top provision. Empty for any other shape.
    pub provisions: Vec<ProvisionEntry>,
    /// In the order of the module's notes.
    pub notes: Vec<NoteEntry>,
    pub footnotes: u32,
    /// `None` for a curated module.
    pub layers: Option<LayerHashes>,
    /// In document order.
    pub references: Vec<Reference>,
    pub definitions: Vec<DefinitionEntry>,
    pub imports: Vec<Declared<Import>>,
    pub citations: Vec<Declared<Citation>>,
    pub bindings: Vec<Declared<Binding>>,
}

/// The text of the address `at` relative to `module`, as
/// [`Interface::address_at`] gives it.
pub(crate) fn address_text_at(module: &Address, at: &str) -> String {
    if at.is_empty() {
        module.as_str().to_owned()
    } else if at.starts_with('/') {
        at.to_owned()
    } else {
        format!("{module}/{at}")
    }
}

/// `address` relative to `module`: empty when they are equal, the segments
/// below `module` without a leading `/` when it lies below, and the whole
/// address otherwise, which only a module with an `E700` has.
fn relative(module: &Address, address: &Address) -> Box<str> {
    match address.as_str().strip_prefix(module.as_str()) {
        Some("") => "".into(),
        Some(rest) if rest.starts_with('/') => rest[1..].into(),
        _ => address.as_str().into(),
    }
}

impl Interface {
    /// The word `word` of [`Interface::words`].
    pub fn word(&self, word: Word) -> &str {
        &self.words[word.0 as usize]
    }

    /// The address that `at`, an address relative to the module's, names:
    /// the module's address when `at` is empty, `at` itself when it begins
    /// with `/`, and otherwise the module's address followed by `/` and
    /// `at`.
    pub fn address_at(&self, at: &str) -> Address {
        Address::from_checked(address_text_at(&self.address, at))
    }

    /// The address of what a reference stands in.
    pub fn place_address(&self, place: Place) -> Address {
        match place {
            Place::Module => self.address.clone(),
            Place::Provision(index) => self.address_at(&self.provisions[index as usize].at),
            Place::Note(index) => self.address_at(&self.notes[index as usize].at),
        }
    }

    /// The interface as one line of compact JSON, with a final line end.
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string(&InterfaceFile {
            schema: InterfaceSchema,
            interface: self,
        })
        .expect("an interface of strings, numbers, digests and addresses is written");
        text.push('\n');
        text
    }

    /// The interface written as `text`. Refused, with the reason, when it is
    /// no JSON of an interface, names another `schema`, has a field it does
    /// not know or lacks one, or names a word, a provision or a note that is
    /// not there, an address that is none, or parts that its shape has not.
    /// It is also refused when its JSON value is not the value that the
    /// interface read from it writes back, as when the key of an optional
    /// field is missing or a struct is written as an array of its values,
    /// which the readers `serde` derives would take; the values are compared,
    /// not the bytes.
    pub fn from_json(text: &str) -> Result<Interface, String> {
        let file: InterfaceFile<Interface> =
            serde_json::from_str(text).map_err(|error| error.to_string())?;
        as_written(
            text,
            &InterfaceFile {
                schema: InterfaceSchema,
                interface: &file.interface,
            },
        )?;
        file.interface.validated()
    }

    /// This interface, when what it names is there: every word, provision
    /// and note it names by its place, every address it writes relative to
    /// the module's, and the parts of its shape. [`Interface::from_json`]
    /// checks this of what it reads; an interface read with `serde` as part
    /// of another document is checked with this before it is used.
    pub fn validated(self) -> Result<Interface, String> {
        let words = self.words.len();
        let word = |word: Word, what: &str| {
            if (word.0 as usize) < words {
                Ok(())
            } else {
                Err(format!(
                    "{what} names word {}, and the interface has {words}",
                    word.0
                ))
            }
        };
        let at = |at: &str, what: &str| {
            address::check(&address_text_at(&self.address, at))
                .map_err(|error| format!("{what} `{at}` names no address: {error}"))
        };
        for provision in &self.provisions {
            word(provision.kind, "the kind of a provision")?;
            if let Some(status) = provision.status {
                word(status, "the status of a provision")?;
            }
            at(&provision.at, "the provision")?;
        }
        for note in &self.notes {
            word(note.group, "the group of a note")?;
            at(&note.at, "the note")?;
        }
        for definition in &self.definitions {
            at(&definition.at, "the provision of a definition")?;
        }
        for reference in &self.references {
            let (index, count, what) = match reference.from {
                Place::Module => continue,
                Place::Provision(index) => (index, self.provisions.len(), "provision"),
                Place::Note(index) => (index, self.notes.len(), "note"),
            };
            if index as usize >= count {
                return Err(format!(
                    "a reference stands in {what} {index}, and the interface has {count}"
                ));
            }
        }
        let shape = self.shape;
        let parts = [
            (
                self.container.is_some(),
                shape == Shape::Container,
                "a container",
            ),
            (
                !self.provisions.is_empty(),
                shape == Shape::Unit,
                "provisions",
            ),
            (self.source.is_some(), shape != Shape::Curated, "a source"),
            (self.layers.is_some(), shape != Shape::Curated, "layers"),
        ];
        let shape_of = shape.described();
        for (has, should, what) in parts {
            if has != should {
                let verb = if has { "has" } else { "lacks" };
                return Err(format!("the interface of {shape_of} {verb} {what}"));
            }
        }
        let curated_only =
            !self.imports.is_empty() || !self.citations.is_empty() || !self.bindings.is_empty();
        if curated_only && shape != Shape::Curated {
            return Err(format!(
                "the interface of {shape_of} has imports, citations or bindings"
            ));
        }
        if !self.definitions.is_empty() && shape != Shape::Unit {
            return Err(format!("the interface of {shape_of} has definitions"));
        }
        if shape == Shape::Unit && !self.provisions[0].at.is_empty() {
            return Err("the first provision of a unit is not at the unit's address".to_owned());
        }
        Ok(self.shrunk())
    }

    /// This interface with no spare room in its longer lists, which a list
    /// that grows while it is built or read leaves, up to as much again as it
    /// holds.
    fn shrunk(mut self) -> Interface {
        self.words.shrink_to_fit();
        self.provisions.shrink_to_fit();
        self.notes.shrink_to_fit();
        self.references.shrink_to_fit();
        self.definitions.shrink_to_fit();
        self
    }
}

/// The JSON of an interface: its `schema`, then the interface.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InterfaceFile<T> {
    schema: InterfaceSchema,
    interface: T,
}

/// The `schema` of an interface, which is written as its text and refuses
/// every other string when it is read.
struct InterfaceSchema;

impl Serialize for InterfaceSchema {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(INTERFACE_SCHEMA)
    }
}

impl<'de> Deserialize<'de> for InterfaceSchema {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(InterfaceSchemaVisitor)
    }
}

struct InterfaceSchemaVisitor;

impl Visitor<'_> for InterfaceSchemaVisitor {
    type Value = InterfaceSchema;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{INTERFACE_SCHEMA:?} in `schema`")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<InterfaceSchema, E> {
        if text == INTERFACE_SCHEMA {
            Ok(InterfaceSchema)
        } else {
            Err(E::custom(format_args!(
                "`schema` is {text:?}, not {INTERFACE_SCHEMA:?}"
            )))
        }
    }
}

/// The interface of `module`, whose declarations stand at `spans` in its
/// source. A declaration that `spans` gives no range for, as for a model
/// that was built rather than read, has the empty range at 0.
pub fn interface_of(module: &Module, spans: &DeclSpans) -> Interface {
    let mut out = Builder::default();
    let interface = match module {
        Module::Unit(unit) => {
            let hashes = tree_hashes(&unit.root);
            out.provision(&unit.address, &unit.root, &hashes);
            out.matter(
                &unit.address,
                unit.credit.as_ref(),
                &unit.notes,
                &unit.footnotes,
            );
            let definitions = unit
                .definitions
                .iter()
                .map(|definition| DefinitionEntry {
                    at: relative(&unit.address, &definition.at),
                    terms: definition.terms.clone(),
                    kind: definition.kind,
                    basis: definition.basis,
                })
                .collect();
            Interface {
                address: unit.address.clone(),
                address_range: spans.address,
                shape: Shape::Unit,
                aliases: declared(&unit.aliases, &spans.aliases),
                within: within(unit.within.as_ref(), spans.within),
                source: Some(unit.source.clone()),
                positive_law: unit.positive_law,
                container: None,
                words: out.words,
                provisions: out.provisions,
                notes: out.notes,
                footnotes: count(unit.footnotes.len()),
                layers: Some(layer_hashes_of_unit(unit)),
                references: out.references,
                definitions,
                imports: Vec::new(),
                citations: Vec::new(),
                bindings: Vec::new(),
            }
        }
        Module::Container(container) => {
            if let Some(num) = &container.num {
                out.text(&num.shown, Layer::Operative, Place::Module);
            }
            if let Some(heading) = &container.heading {
                out.text(heading, Layer::Operative, Place::Module);
            }
            out.matter(
                &container.address,
                container.credit.as_ref(),
                &container.notes,
                &container.footnotes,
            );
            Interface {
                address: container.address.clone(),
                address_range: spans.address,
                shape: Shape::Container,
                aliases: declared(&container.aliases, &spans.aliases),
                within: within(container.within.as_ref(), spans.within),
                source: Some(container.source.clone()),
                positive_law: container.positive_law,
                container: Some(ContainerEntry {
                    kind: container.kind.clone(),
                    status: container.status.clone(),
                    members: declared(&container.members, &spans.members),
                }),
                words: out.words,
                provisions: Vec::new(),
                notes: out.notes,
                footnotes: count(container.footnotes.len()),
                layers: Some(layer_hashes_of_container(container)),
                references: out.references,
                definitions: Vec::new(),
                imports: Vec::new(),
                citations: Vec::new(),
                bindings: Vec::new(),
            }
        }
        Module::Curated(curated) => Interface {
            address: curated.address.clone(),
            address_range: spans.address,
            shape: Shape::Curated,
            aliases: Vec::new(),
            within: None,
            source: None,
            positive_law: None,
            container: None,
            words: Vec::new(),
            provisions: Vec::new(),
            notes: Vec::new(),
            footnotes: 0,
            layers: None,
            references: Vec::new(),
            definitions: Vec::new(),
            imports: declared(&curated.imports, &spans.imports),
            citations: declared(&curated.citations, &spans.citations),
            bindings: declared(&curated.bindings, &spans.bindings),
        },
    };
    interface.shrunk()
}

/// Each of `items` with the range at its place in `ranges`, or the empty
/// range at 0 when `ranges` has none there.
fn declared<T: Clone>(items: &[T], ranges: &[ByteRange]) -> Vec<Declared<T>> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| Declared {
            item: item.clone(),
            range: ranges.get(index).copied().unwrap_or_default(),
        })
        .collect()
}

fn within(within: Option<&Address>, range: Option<ByteRange>) -> Option<Declared<Address>> {
    within.map(|item| Declared {
        item: item.clone(),
        range: range.unwrap_or_default(),
    })
}

/// `count` as a `u32`; a module of edition 2 has fewer than 2^32 of
/// anything, as its source has at most 2^24 tokens.
fn count(count: usize) -> u32 {
    u32::try_from(count).expect("fewer than 2^32 parts of one module")
}

/// What [`interface_of`] gathers while it walks a module.
#[derive(Default)]
struct Builder {
    words: Vec<Box<str>>,
    provisions: Vec<ProvisionEntry>,
    notes: Vec<NoteEntry>,
    references: Vec<Reference>,
}

impl Builder {
    /// `text` as a word of the interface, added when it is not there yet.
    fn word(&mut self, text: &str) -> Word {
        let at = match self.words.iter().position(|word| **word == *text) {
            Some(at) => at,
            None => {
                self.words.push(text.into());
                self.words.len() - 1
            }
        };
        Word(count(at))
    }

    /// Adds `provision` and the provisions below it, in document order;
    /// `hashes` holds the tree hash of each in that order.
    fn provision(&mut self, module: &Address, provision: &Provision, hashes: &[Hash32]) {
        let index = self.provisions.len();
        let place = Place::Provision(count(index));
        let kind = self.word(&provision.kind);
        let status = provision.status.as_deref().map(|status| self.word(status));
        self.provisions.push(ProvisionEntry {
            at: relative(module, &provision.address),
            kind,
            status,
            hash: hashes[index],
            image: false,
        });
        if let Some(num) = &provision.num {
            self.text(&num.shown, Layer::Operative, place);
        }
        if let Some(heading) = &provision.heading {
            self.text(heading, Layer::Operative, place);
        }
        let mut image = false;
        for item in &provision.items {
            match item {
                Item::Part { blocks, .. } => image |= self.blocks(blocks, Layer::Operative, place),
                Item::Child(child) => self.provision(module, child, hashes),
            }
        }
        self.provisions[index].image = image;
    }

    /// Adds the references of a module's credit, notes and footnotes, and
    /// its notes.
    fn matter(
        &mut self,
        module: &Address,
        credit: Option<&Text>,
        notes: &[Note],
        footnotes: &[Footnote],
    ) {
        if let Some(credit) = credit {
            self.text(credit, Layer::Editorial, Place::Module);
        }
        for note in notes {
            let index = self.notes.len();
            let place = Place::Note(count(index));
            let group = self.word(&note.group);
            self.notes.push(NoteEntry {
                at: relative(module, &note.address),
                layer: note.layer,
                group,
                hash: hash_note(note),
                image: false,
            });
            if let Some(heading) = &note.heading {
                self.text(heading, note.layer, place);
            }
            self.notes[index].image = self.blocks(&note.blocks, note.layer, place);
        }
        for footnote in footnotes {
            self.blocks(&footnote.blocks, Layer::Editorial, Place::Module);
        }
    }

    fn text(&mut self, text: &Text, layer: Layer, from: Place) {
        for run in &text.runs {
            if let Run::Ref { target, .. } = run {
                self.references.push(Reference {
                    layer,
                    from,
                    target: target.as_str().into(),
                });
            }
        }
    }

    /// Adds the references of `blocks`, at any depth; whether they hold an
    /// image.
    fn blocks(&mut self, blocks: &[Block], layer: Layer, from: Place) -> bool {
        let mut image = false;
        for block in blocks {
            match block {
                Block::Para(text) | Block::Subheading(text) => self.text(text, layer, from),
                Block::Table(table) => {
                    for cell in table.rows.iter().flat_map(|row| &row.cells) {
                        image |= self.blocks(&cell.blocks, layer, from);
                    }
                }
                Block::Quoted(quoted) => image |= self.quoted(&quoted.items, layer, from),
                Block::Image { .. } => image = true,
            }
        }
        image
    }

    fn quoted(&mut self, items: &[QuotedItem], layer: Layer, from: Place) -> bool {
        let mut image = false;
        for item in items {
            match item {
                QuotedItem::Block(block) => {
                    image |= self.blocks(std::slice::from_ref(block), layer, from);
                }
                QuotedItem::Part { blocks, .. } => image |= self.blocks(blocks, layer, from),
                QuotedItem::Level(level) => {
                    if let Some(num) = &level.num {
                        self.text(&num.shown, layer, from);
                    }
                    if let Some(heading) = &level.heading {
                        self.text(heading, layer, from);
                    }
                    image |= self.quoted(&level.items, layer, from);
                }
            }
        }
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::build::*;
    use crate::model::{
        BindTarget, Cell, ContainerModule, CuratedModule, Num, Quoted, QuotedLevel, Row, RowGroup,
        Table, UnitModule,
    };

    const PROVISION_HASH: &str = "ec085e67853dd9ced4dafa9836e747dc158ae35ed76f628d9b1eacbd6937be77";
    const NOTES_HASH: &str = "a396bac23ba81b30cb34dbf808cf06deeaeb4b87e010b66646faadc6accfc6ff";

    fn linked(target: &str) -> Text {
        text(vec![words("See "), link("this", target), words(".")])
    }

    fn linked_para(target: &str) -> Block {
        Block::Para(linked(target))
    }

    /// A unit with a link in every place a unit holds text: the number and
    /// heading of its top provision, a paragraph, a table cell and quoted
    /// content of its child, its credit, an enacted note with quoted
    /// content, an editorial note and a footnote. The child also holds an
    /// image.
    fn linked_unit() -> UnitModule {
        let quoted = Quoted {
            origin: None,
            items: vec![QuotedItem::Level(QuotedLevel {
                kind: "section".to_owned(),
                num: None,
                heading: Some(linked("/zz/demo/r/quoted-heading")),
                items: vec![QuotedItem::Block(linked_para("/zz/demo/r/quoted"))],
            })],
        };
        let table = Table {
            rows: vec![Row {
                group: RowGroup::Body,
                cells: vec![Cell {
                    header: false,
                    colspan: 1,
                    rowspan: 1,
                    blocks: vec![linked_para("/zz/demo/r/cell")],
                }],
            }],
        };
        let child = provision(
            "/zz/demo/t1/s8/a",
            "subsection",
            num("a", "(a)"),
            None,
            vec![content(vec![
                linked_para("/zz/demo/r/para"),
                Block::Table(table),
                Block::Quoted(quoted),
                Block::Image {
                    source: "demo.eps".to_owned(),
                },
            ])],
        );
        let root = Provision {
            num: Some(Num {
                value: "8".to_owned(),
                shown: linked("/zz/demo/r/num"),
            }),
            heading: Some(linked("/zz/demo/r/heading")),
            ..provision(
                "/zz/demo/t1/s8",
                "section",
                None,
                None,
                vec![Item::Child(child)],
            )
        };
        let mut enacted = note(
            "/zz/demo/t1/s8/note/statutoryNotes/nolaw/amendment",
            Layer::Enacted,
            "Amendment",
            "An invented amendment.",
        );
        enacted.blocks.push(Block::Quoted(Quoted {
            origin: Some("An invented act".to_owned()),
            items: vec![QuotedItem::Block(linked_para("/zz/demo/r/enacted"))],
        }));
        let mut editorial = note(
            "/zz/demo/t1/s8/note/editorialNotes/nolaw/codification",
            Layer::Editorial,
            "Codification",
            "An invented codification.",
        );
        editorial.blocks.push(linked_para("/zz/demo/r/editorial"));
        UnitModule {
            address: address("/zz/demo/t1/s8"),
            aliases: Vec::new(),
            within: Some(address("/zz/demo/t1/ch1")),
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            root,
            credit: Some(linked("/zz/demo/r/credit")),
            notes: vec![enacted, editorial],
            footnotes: vec![Footnote {
                ordinal: 1,
                shown: "1".to_owned(),
                blocks: vec![linked_para("/zz/demo/r/footnote")],
            }],
            definitions: Vec::new(),
        }
    }

    /// A container with a link in its number, its heading, its credit, a note
    /// and a footnote.
    fn linked_container() -> ContainerModule {
        let mut statutory = note(
            "/zz/demo/t1/ch1/note/statutoryNotes/nolaw/short-title",
            Layer::Enacted,
            "Short Title",
            "An invented short title.",
        );
        statutory
            .blocks
            .push(linked_para("/zz/demo/r/container-note"));
        ContainerModule {
            address: address("/zz/demo/t1/ch1"),
            aliases: vec![address("/zz/demo/t1/chI")],
            within: Some(address("/zz/demo/t1")),
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            kind: "chapter".to_owned(),
            num: Some(Num {
                value: "1".to_owned(),
                shown: linked("/zz/demo/r/container-num"),
            }),
            heading: Some(linked("/zz/demo/r/container-heading")),
            status: Some("repealed".to_owned()),
            members: vec![address("/zz/demo/t1/s8")],
            credit: Some(linked("/zz/demo/r/container-credit")),
            notes: vec![statutory],
            footnotes: vec![Footnote {
                ordinal: 1,
                shown: "1".to_owned(),
                blocks: vec![linked_para("/zz/demo/r/container-footnote")],
            }],
        }
    }

    /// Each reference of `interface` as its layer, the address it stands in
    /// and its target.
    fn references(interface: &Interface) -> Vec<(Layer, String, String)> {
        interface
            .references
            .iter()
            .map(|reference| {
                (
                    reference.layer,
                    interface.place_address(reference.from).to_string(),
                    reference.target.to_string(),
                )
            })
            .collect()
    }

    #[test]
    fn each_reference_stands_in_the_layer_of_what_holds_it() {
        let unit = interface_of(&Module::Unit(linked_unit()), &DeclSpans::default());
        let (section, subsection) = ("/zz/demo/t1/s8", "/zz/demo/t1/s8/a");
        let amendment = "/zz/demo/t1/s8/note/statutoryNotes/nolaw/amendment";
        let codification = "/zz/demo/t1/s8/note/editorialNotes/nolaw/codification";
        let expected = [
            (Layer::Operative, section, "num"),
            (Layer::Operative, section, "heading"),
            (Layer::Operative, subsection, "para"),
            (Layer::Operative, subsection, "cell"),
            (Layer::Operative, subsection, "quoted-heading"),
            (Layer::Operative, subsection, "quoted"),
            (Layer::Editorial, section, "credit"),
            (Layer::Enacted, amendment, "enacted"),
            (Layer::Editorial, codification, "editorial"),
            (Layer::Editorial, section, "footnote"),
        ]
        .map(|(layer, from, target)| (layer, from.to_owned(), format!("/zz/demo/r/{target}")));
        assert_eq!(references(&unit), expected);
        assert_eq!(
            unit.provisions
                .iter()
                .map(|provision| (unit.address_at(&provision.at).to_string(), provision.image))
                .collect::<Vec<_>>(),
            [(section.to_owned(), false), (subsection.to_owned(), true)]
        );
        assert_eq!(unit.provisions[1].at.as_ref(), "a");
        assert_eq!(unit.footnotes, 1);

        let container = interface_of(
            &Module::Container(linked_container()),
            &DeclSpans::default(),
        );
        let chapter = "/zz/demo/t1/ch1";
        let short_title = "/zz/demo/t1/ch1/note/statutoryNotes/nolaw/short-title";
        let expected = [
            (Layer::Operative, chapter, "container-num"),
            (Layer::Operative, chapter, "container-heading"),
            (Layer::Editorial, chapter, "container-credit"),
            (Layer::Enacted, short_title, "container-note"),
            (Layer::Editorial, chapter, "container-footnote"),
        ]
        .map(|(layer, from, target)| (layer, from.to_owned(), format!("/zz/demo/r/{target}")));
        assert_eq!(references(&container), expected);
        let entry = container.container.as_ref().expect("a container entry");
        assert_eq!(entry.status.as_deref(), Some("repealed"));
        assert_eq!(entry.members[0].item, address("/zz/demo/t1/s8"));
        assert_eq!(container.notes[0].layer, Layer::Enacted);
        assert_eq!(container.word(container.notes[0].group), "demoNotes");
    }

    /// A curated module with one import, one citation and one binding.
    fn curated() -> Module {
        Module::Curated(CuratedModule {
            address: address("/zz/demo/c1"),
            imports: vec![Import {
                module: address("/zz/demo/c2"),
                alias: Some("c2".to_owned()),
                names: Vec::new(),
            }],
            citations: vec![Citation {
                name: "rule".to_owned(),
                target: address("/zz/demo/t1/s1"),
            }],
            bindings: vec![Binding {
                name: "binding".to_owned(),
                binds: BindTarget::Name {
                    module: None,
                    name: "rule".to_owned(),
                },
                release: "42-7".to_owned(),
                provision_hash: PROVISION_HASH.parse().expect("a hash"),
                notes_hash: NOTES_HASH.parse().expect("a hash"),
            }],
        })
    }

    fn curated_spans() -> DeclSpans {
        let range = |start, end| ByteRange { start, end };
        DeclSpans {
            address: range(17, 30),
            imports: vec![range(35, 63)],
            citations: vec![range(66, 98)],
            bindings: vec![range(101, 290)],
            ..DeclSpans::default()
        }
    }

    #[test]
    fn an_interface_is_written_in_its_layout() {
        let interface = interface_of(&curated(), &curated_spans());
        let expected = format!(
            "{{\"schema\":\"fidryn.interface/v0.1\",\"interface\":{{\
             \"address\":\"/zz/demo/c1\",\"address_range\":{{\"start\":17,\"end\":30}},\
             \"shape\":\"curated\",\"aliases\":[],\"within\":null,\"source\":null,\
             \"positive_law\":null,\"container\":null,\"words\":[],\"provisions\":[],\
             \"notes\":[],\"footnotes\":0,\"layers\":null,\"references\":[],\"definitions\":[],\
             \"imports\":[{{\"item\":{{\"module\":\"/zz/demo/c2\",\"alias\":\"c2\",\"names\":[]}},\
             \"range\":{{\"start\":35,\"end\":63}}}}],\
             \"citations\":[{{\"item\":{{\"name\":\"rule\",\"target\":\"/zz/demo/t1/s1\"}},\
             \"range\":{{\"start\":66,\"end\":98}}}}],\
             \"bindings\":[{{\"item\":{{\"name\":\"binding\",\"binds\":{{\"name\":{{\"module\":null,\
             \"name\":\"rule\"}}}},\"release\":\"42-7\",\"provision_hash\":\"{PROVISION_HASH}\",\
             \"notes_hash\":\"{NOTES_HASH}\"}},\"range\":{{\"start\":101,\"end\":290}}}}]}}}}\n"
        );
        assert_eq!(interface.to_json(), expected);
        assert_eq!(Interface::from_json(&expected), Ok(interface));
    }

    #[test]
    fn what_is_not_an_interface_is_refused() {
        let unit = interface_of(&Module::Unit(linked_unit()), &DeclSpans::default());
        let text = unit.to_json();
        assert_eq!(Interface::from_json(&text), Ok(unit.clone()));
        let changed = |change: &dyn Fn(&mut Interface)| {
            let mut interface = unit.clone();
            change(&mut interface);
            interface.to_json()
        };
        assert!(text.contains(",\"status\":null,"), "{text}");
        assert!(
            text.contains("\"address_range\":{\"start\":0,\"end\":0}"),
            "{text}"
        );
        let cases: [(String, &str); 10] = [
            (
                text.replacen("fidryn.interface/v0.1", "fidryn.interface/v0.2", 1),
                "`schema` is \"fidryn.interface/v0.2\"",
            ),
            (
                text.replacen("\"footnotes\":1", "\"footnotes\":1,\"extra\":1", 1),
                "unknown field `extra`",
            ),
            (
                text.replacen(",\"footnotes\":1", "", 1),
                "missing field `footnotes`",
            ),
            // The key of an optional field, which the reader would take as
            // `null` when it is missing.
            (
                text.replacen(",\"status\":null", "", 1),
                "`/interface/provisions/0/status` is missing where the file is written with it",
            ),
            // A struct written as an array of its values, which the reader
            // would take.
            (
                text.replacen(
                    "\"address_range\":{\"start\":0,\"end\":0}",
                    "\"address_range\":[0,0]",
                    1,
                ),
                "`/interface/address_range` is an array where the file is written with an object",
            ),
            (
                changed(&|interface| interface.provisions[1].kind = Word(9)),
                "the kind of a provision names word 9",
            ),
            (
                changed(&|interface| interface.references[0].from = Place::Note(5)),
                "a reference stands in note 5, and the interface has 2",
            ),
            (
                changed(&|interface| interface.notes[0].at = "a//b".into()),
                "the note `a//b` names no address",
            ),
            (
                changed(&|interface| interface.shape = Shape::Container),
                "the interface of a container lacks a container",
            ),
            (
                changed(&|interface| interface.provisions.swap(0, 1)),
                "the first provision of a unit is not at the unit's address",
            ),
        ];
        for (text, words) in cases {
            match Interface::from_json(&text) {
                Ok(_) => panic!("accepted: {text}"),
                Err(error) => assert!(error.contains(words), "{words:?} is not in {error:?}"),
            }
        }
    }
}
