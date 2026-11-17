//! The hashes of provisions, notes, containers and layers.
//!
//! Every hash is BLAKE3 over a sequence of frames. A frame is one tag byte,
//! the length of its value as eight bytes little-endian, and the value. The
//! first frame of every hash has the tag `0x00` and holds a domain string.
//! `docs/law-modules.md` gives every frame under "Hashes".

use crate::address::Address;
use crate::model::{
    Block, ContainerModule, Footnote, Item, Layer, Note, Num, PartRole, Provision, Quoted,
    QuotedItem, RowGroup, Run, Table, Text, UnitModule,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// A BLAKE3 digest, written as 64 lower-case hexadecimal digits.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hash32(pub [u8; 32]);

/// Why a text is no [`Hash32`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HashError {
    /// The text has `found` characters, not 64.
    Length { found: usize },
    /// The character at `index`, counted from 0, is no lower-case
    /// hexadecimal digit.
    Digit { index: usize, character: char },
}

impl fmt::Display for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for Hash32 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash32({self})")
    }
}

impl FromStr for Hash32 {
    type Err = HashError;

    fn from_str(text: &str) -> Result<Hash32, HashError> {
        let found = text.chars().count();
        if found != 64 {
            return Err(HashError::Length { found });
        }
        let mut digits = [0_u8; 64];
        for (index, character) in text.chars().enumerate() {
            digits[index] = match character {
                '0'..='9' => character as u8 - b'0',
                'a'..='f' => character as u8 - b'a' + 10,
                _ => return Err(HashError::Digit { index, character }),
            };
        }
        let mut bytes = [0_u8; 32];
        let (pairs, _) = digits.as_chunks::<2>();
        for (byte, [high, low]) in bytes.iter_mut().zip(pairs) {
            *byte = (high << 4) | low;
        }
        Ok(Hash32(bytes))
    }
}

impl fmt::Display for HashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Length { found } => write!(
                f,
                "a hash is 64 lower-case hexadecimal digits, and this text has {found} characters"
            ),
            Self::Digit { index, character } => write!(
                f,
                "a hash is 64 lower-case hexadecimal digits, and character {index} of this \
                 text, counted from 0, is {character:?}"
            ),
        }
    }
}

impl std::error::Error for HashError {}

impl Serialize for Hash32 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Hash32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// The three layer hashes of a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerHashes {
    pub operative: Hash32,
    pub enacted: Hash32,
    pub editorial: Hash32,
}

/// The tree hash of `provision`: of its own kind, number, heading, status and
/// text parts, and of the tree hash of each child. Its address is not in it.
pub fn hash_provision(provision: &Provision) -> Hash32 {
    tree_hash(provision, &mut hash_provision)
}

/// The tree hash of every provision of the tree below `root`, `root`
/// included, in document order: each provision before its children, and the
/// children in order. Each provision is hashed once, so the cost grows with
/// the size of the tree and not with its depth; `hash_provision` of each
/// provision gives the same values.
pub fn tree_hashes(root: &Provision) -> Vec<Hash32> {
    fn walk(provision: &Provision, hashes: &mut Vec<Hash32>) -> Hash32 {
        let at = hashes.len();
        hashes.push(Hash32([0; 32]));
        let hash = tree_hash(provision, &mut |child| walk(child, hashes));
        hashes[at] = hash;
        hash
    }
    let mut hashes = Vec::new();
    walk(root, &mut hashes);
    hashes
}

/// The tree hash of `provision`, taking the tree hash of each child from
/// `child_hash`.
fn tree_hash(provision: &Provision, child_hash: &mut dyn FnMut(&Provision) -> Hash32) -> Hash32 {
    let mut out = Frames::new(PROVISION_DOMAIN);
    out.frame(tag::KIND, provision.kind.as_bytes());
    out.num(provision.num.as_ref());
    out.optional_text(provision.heading.as_ref());
    out.status(provision.status.as_deref());
    for item in &provision.items {
        match item {
            Item::Part { role, blocks } => out.part(*role, blocks),
            Item::Child(child) => out.frame(tag::CHILD, &child_hash(child).0),
        }
    }
    out.finish()
}

/// The hash of `note`: of its group, topics, heading and blocks. Its address
/// and its layer are not in it.
pub fn hash_note(note: &Note) -> Hash32 {
    let mut out = Frames::new(NOTE_DOMAIN);
    out.frame(tag::GROUP, note.group.as_bytes());
    out.frame(tag::TOPICS, note.topics.join(" ").as_bytes());
    out.optional_text(note.heading.as_ref());
    out.blocks(&note.blocks);
    out.finish()
}

/// The hash of `container`: of its kind, number, heading, status and the
/// address of each member, in order.
pub fn hash_container(container: &ContainerModule) -> Hash32 {
    let mut out = Frames::new(CONTAINER_DOMAIN);
    out.frame(tag::KIND, container.kind.as_bytes());
    out.num(container.num.as_ref());
    out.optional_text(container.heading.as_ref());
    out.status(container.status.as_deref());
    for member in &container.members {
        out.frame(tag::MEMBER, member.as_str().as_bytes());
    }
    out.finish()
}

/// The layer hashes of `unit`: the tree hash of its root, the hash of its
/// enacted notes, and the hash of its editorial matter.
pub fn layer_hashes_of_unit(unit: &UnitModule) -> LayerHashes {
    let mut marks = Vec::new();
    marks_of_provision(&unit.root, &mut marks);
    let matter = Matter {
        address: &unit.address,
        credit: unit.credit.as_ref(),
        notes: &unit.notes,
        footnotes: &unit.footnotes,
    };
    matter.marks(&mut marks);
    LayerHashes {
        operative: hash_provision(&unit.root),
        enacted: enacted_layer(&unit.notes),
        editorial: matter.editorial_layer(&marks),
    }
}

/// The layer hashes of `container`: its container hash, the hash of its
/// enacted notes, and the hash of its editorial matter.
pub fn layer_hashes_of_container(container: &ContainerModule) -> LayerHashes {
    let mut marks = Vec::new();
    if let Some(num) = &container.num {
        marks_of_text(&num.shown, &container.address, &mut marks);
    }
    if let Some(heading) = &container.heading {
        marks_of_text(heading, &container.address, &mut marks);
    }
    let matter = Matter {
        address: &container.address,
        credit: container.credit.as_ref(),
        notes: &container.notes,
        footnotes: &container.footnotes,
    };
    matter.marks(&mut marks);
    LayerHashes {
        operative: hash_container(container),
        enacted: enacted_layer(&container.notes),
        editorial: matter.editorial_layer(&marks),
    }
}

const PROVISION_DOMAIN: &str = "fidryn.provision/v1";
const NOTE_DOMAIN: &str = "fidryn.note/v1";
const CONTAINER_DOMAIN: &str = "fidryn.container/v1";
const ENACTED_DOMAIN: &str = "fidryn.layer.enacted/v1";
const EDITORIAL_DOMAIN: &str = "fidryn.layer.editorial/v1";

/// The tags of the frames.
mod tag {
    pub(super) const DOMAIN: u8 = 0x00;
    pub(super) const KIND: u8 = 0x01;
    pub(super) const NUM_VALUE: u8 = 0x02;
    pub(super) const NUM_SHOWN: u8 = 0x03;
    pub(super) const NO_NUM: u8 = 0x04;
    pub(super) const STATUS: u8 = 0x05;
    pub(super) const PLAIN: u8 = 0x10;
    pub(super) const REF: u8 = 0x11;
    pub(super) const SUP: u8 = 0x12;
    pub(super) const SUB: u8 = 0x13;
    pub(super) const NO_TEXT: u8 = 0x1F;
    pub(super) const PARA: u8 = 0x20;
    pub(super) const SUBHEADING: u8 = 0x21;
    pub(super) const TABLE: u8 = 0x22;
    pub(super) const QUOTED: u8 = 0x23;
    pub(super) const IMAGE: u8 = 0x24;
    /// Plus 0 for a row of the head, 1 of the body, 2 of the foot.
    pub(super) const ROW: u8 = 0x30;
    pub(super) const CELL: u8 = 0x34;
    pub(super) const HEADER_CELL: u8 = 0x35;
    pub(super) const ORIGIN: u8 = 0x40;
    pub(super) const LEVEL: u8 = 0x41;
    /// Plus 0 for a chapeau, 1 content, 2 continuation, 3 proviso.
    pub(super) const PART: u8 = 0x50;
    pub(super) const CHILD: u8 = 0x60;
    pub(super) const MEMBER: u8 = 0x61;
    pub(super) const GROUP: u8 = 0x70;
    pub(super) const TOPICS: u8 = 0x71;
    pub(super) const NOTE_ADDRESS: u8 = 0x72;
    pub(super) const NOTE_HASH: u8 = 0x73;
    pub(super) const FOOTNOTE: u8 = 0x74;
    pub(super) const MARK_ADDRESS: u8 = 0x75;
    pub(super) const MARK_SHOWN: u8 = 0x76;
    pub(super) const MARK_FOOTNOTE: u8 = 0x77;
}

/// The bytes a hash is computed over.
struct Frames(Vec<u8>);

impl Frames {
    /// Frames that begin with the domain frame of `domain`.
    fn new(domain: &str) -> Frames {
        let mut frames = Frames(Vec::new());
        frames.frame(tag::DOMAIN, domain.as_bytes());
        frames
    }

    fn finish(self) -> Hash32 {
        Hash32(*blake3::hash(&self.0).as_bytes())
    }

    fn frame(&mut self, tag: u8, value: &[u8]) {
        self.0.push(tag);
        self.length(value.len());
        self.0.extend_from_slice(value);
    }

    fn length(&mut self, length: usize) {
        let length = u64::try_from(length).expect("a length fits in 64 bits");
        self.0.extend_from_slice(&length.to_le_bytes());
    }

    /// A frame whose value is what `value` writes.
    fn nested(&mut self, tag: u8, value: impl FnOnce(&mut Frames)) {
        self.0.push(tag);
        let at = self.0.len();
        self.0.extend_from_slice(&[0; 8]);
        value(self);
        let length = self.0.len() - at - 8;
        let length = u64::try_from(length).expect("a length fits in 64 bits");
        self.0[at..at + 8].copy_from_slice(&length.to_le_bytes());
    }

    fn offset(&mut self, offset: usize) {
        let offset = u64::try_from(offset).expect("an offset fits in 64 bits");
        self.0.extend_from_slice(&offset.to_le_bytes());
    }

    /// The frames of `text`. Offsets are bytes of its plain text.
    fn text(&mut self, text: &Text) {
        self.frame(tag::PLAIN, text.plain().as_bytes());
        let mut offset = 0;
        for run in &text.runs {
            let start = offset;
            offset += run.words().len();
            let tag = match run {
                Run::Ref { .. } => tag::REF,
                Run::Sup(_) => tag::SUP,
                Run::Sub(_) => tag::SUB,
                Run::Words(_) | Run::Mark { .. } => continue,
            };
            self.nested(tag, |out| {
                out.offset(start);
                out.offset(offset);
                if let Run::Ref { target, .. } = run {
                    out.0.extend_from_slice(target.as_bytes());
                }
            });
        }
    }

    fn optional_text(&mut self, text: Option<&Text>) {
        match text {
            Some(text) => self.text(text),
            None => self.frame(tag::NO_TEXT, &[]),
        }
    }

    fn num(&mut self, num: Option<&Num>) {
        match num {
            Some(num) => {
                self.frame(tag::NUM_VALUE, num.value.as_bytes());
                self.nested(tag::NUM_SHOWN, |out| out.text(&num.shown));
            }
            None => self.frame(tag::NO_NUM, &[]),
        }
    }

    fn status(&mut self, status: Option<&str>) {
        self.frame(tag::STATUS, status.unwrap_or("").as_bytes());
    }

    fn part(&mut self, role: PartRole, blocks: &[Block]) {
        let role = match role {
            PartRole::Chapeau => 0,
            PartRole::Content => 1,
            PartRole::Continuation => 2,
            PartRole::Proviso => 3,
        };
        self.nested(tag::PART + role, |out| out.blocks(blocks));
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.block(block);
        }
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Para(text) => self.nested(tag::PARA, |out| out.text(text)),
            Block::Subheading(text) => self.nested(tag::SUBHEADING, |out| out.text(text)),
            Block::Table(table) => self.nested(tag::TABLE, |out| out.table(table)),
            Block::Quoted(quoted) => self.nested(tag::QUOTED, |out| out.quoted(quoted)),
            Block::Image { source } => self.frame(tag::IMAGE, source.as_bytes()),
        }
    }

    fn table(&mut self, table: &Table) {
        for row in &table.rows {
            let group = match row.group {
                RowGroup::Head => 0,
                RowGroup::Body => 1,
                RowGroup::Foot => 2,
            };
            self.nested(tag::ROW + group, |out| {
                for cell in &row.cells {
                    let tag = if cell.header {
                        tag::HEADER_CELL
                    } else {
                        tag::CELL
                    };
                    out.nested(tag, |out| {
                        out.0.extend_from_slice(&cell.colspan.to_le_bytes());
                        out.0.extend_from_slice(&cell.rowspan.to_le_bytes());
                        out.blocks(&cell.blocks);
                    });
                }
            });
        }
    }

    fn quoted(&mut self, quoted: &Quoted) {
        self.frame(
            tag::ORIGIN,
            quoted.origin.as_deref().unwrap_or("").as_bytes(),
        );
        self.quoted_items(&quoted.items);
    }

    fn quoted_items(&mut self, items: &[QuotedItem]) {
        for item in items {
            match item {
                QuotedItem::Block(block) => self.block(block),
                QuotedItem::Part { role, blocks } => self.part(*role, blocks),
                QuotedItem::Level(level) => self.nested(tag::LEVEL, |out| {
                    out.frame(tag::KIND, level.kind.as_bytes());
                    out.num(level.num.as_ref());
                    out.optional_text(level.heading.as_ref());
                    out.quoted_items(&level.items);
                }),
            }
        }
    }

    /// The frames of the notes of `layer`, in the byte order of their
    /// addresses.
    fn notes(&mut self, notes: &[Note], layer: Layer) {
        for note in in_address_order(notes) {
            if note.layer == layer {
                self.frame(tag::NOTE_ADDRESS, note.address.as_str().as_bytes());
                self.frame(tag::NOTE_HASH, &hash_note(note).0);
            }
        }
    }
}

/// `notes` in the byte order of their addresses.
fn in_address_order(notes: &[Note]) -> Vec<&Note> {
    let mut sorted: Vec<&Note> = notes.iter().collect();
    sorted.sort_by(|a, b| a.address.cmp(&b.address));
    sorted
}

fn enacted_layer(notes: &[Note]) -> Hash32 {
    let mut out = Frames::new(ENACTED_DOMAIN);
    out.notes(notes, Layer::Enacted);
    out.finish()
}

/// A footnote marker of a module, with the address it is recorded under.
struct MarkAt<'a> {
    address: &'a Address,
    shown: &'a str,
    footnote: u32,
}

/// What a module holds besides its provisions or its own fields.
struct Matter<'a> {
    address: &'a Address,
    credit: Option<&'a Text>,
    notes: &'a [Note],
    footnotes: &'a [Footnote],
}

impl<'a> Matter<'a> {
    fn footnotes_in_order(&self) -> Vec<&'a Footnote> {
        let mut footnotes: Vec<&Footnote> = self.footnotes.iter().collect();
        footnotes.sort_by_key(|footnote| footnote.ordinal);
        footnotes
    }

    /// Adds the markers of the credit, of each note in the byte order of its
    /// address, as the layers take the notes, and of each footnote in ordinal
    /// order. So the order of the module's list of notes is in no hash.
    fn marks(&self, out: &mut Vec<MarkAt<'a>>) {
        if let Some(credit) = self.credit {
            marks_of_text(credit, self.address, out);
        }
        for note in in_address_order(self.notes) {
            if let Some(heading) = &note.heading {
                marks_of_text(heading, &note.address, out);
            }
            marks_of_blocks(&note.blocks, &note.address, out);
        }
        for footnote in self.footnotes_in_order() {
            marks_of_blocks(&footnote.blocks, self.address, out);
        }
    }

    fn editorial_layer(&self, marks: &[MarkAt<'_>]) -> Hash32 {
        let mut out = Frames::new(EDITORIAL_DOMAIN);
        out.optional_text(self.credit);
        out.notes(self.notes, Layer::Editorial);
        for footnote in self.footnotes_in_order() {
            out.frame(tag::FOOTNOTE, footnote.shown.as_bytes());
            out.blocks(&footnote.blocks);
        }
        for mark in marks {
            out.frame(tag::MARK_ADDRESS, mark.address.as_str().as_bytes());
            out.frame(tag::MARK_SHOWN, mark.shown.as_bytes());
            out.frame(tag::MARK_FOOTNOTE, &mark.footnote.to_le_bytes());
        }
        out.finish()
    }
}

/// Adds the markers of the tree below `provision`, depth first: for each
/// provision its number, its heading and then its items in order.
fn marks_of_provision<'a>(provision: &'a Provision, out: &mut Vec<MarkAt<'a>>) {
    let address = &provision.address;
    if let Some(num) = &provision.num {
        marks_of_text(&num.shown, address, out);
    }
    if let Some(heading) = &provision.heading {
        marks_of_text(heading, address, out);
    }
    for item in &provision.items {
        match item {
            Item::Part { blocks, .. } => marks_of_blocks(blocks, address, out),
            Item::Child(child) => marks_of_provision(child, out),
        }
    }
}

fn marks_of_text<'a>(text: &'a Text, address: &'a Address, out: &mut Vec<MarkAt<'a>>) {
    for run in &text.runs {
        if let Run::Mark { shown, footnote } = run {
            out.push(MarkAt {
                address,
                shown,
                footnote: *footnote,
            });
        }
    }
}

fn marks_of_blocks<'a>(blocks: &'a [Block], address: &'a Address, out: &mut Vec<MarkAt<'a>>) {
    for block in blocks {
        match block {
            Block::Para(text) | Block::Subheading(text) => marks_of_text(text, address, out),
            Block::Table(table) => {
                for cell in table.rows.iter().flat_map(|row| &row.cells) {
                    marks_of_blocks(&cell.blocks, address, out);
                }
            }
            Block::Quoted(quoted) => marks_of_quoted_items(&quoted.items, address, out),
            Block::Image { .. } => {}
        }
    }
}

fn marks_of_quoted_items<'a>(
    items: &'a [QuotedItem],
    address: &'a Address,
    out: &mut Vec<MarkAt<'a>>,
) {
    for item in items {
        match item {
            QuotedItem::Block(block) => marks_of_blocks(std::slice::from_ref(block), address, out),
            QuotedItem::Part { blocks, .. } => marks_of_blocks(blocks, address, out),
            QuotedItem::Level(level) => {
                if let Some(num) = &level.num {
                    marks_of_text(&num.shown, address, out);
                }
                if let Some(heading) = &level.heading {
                    marks_of_text(heading, address, out);
                }
                marks_of_quoted_items(&level.items, address, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Address;
    use crate::model::build::*;
    use crate::model::{
        Block, Cell, Footnote, Item, Layer, PartRole, Row, RowGroup, Run, Table, Text,
    };
    use serde::de::DeserializeOwned;
    use serde_json::Value;
    use std::collections::BTreeMap;
    use std::path::Path;

    fn hash(text: &str) -> Hash32 {
        text.parse()
            .unwrap_or_else(|error| panic!("{text}: {error}"))
    }

    #[test]
    fn a_hash_is_written_as_64_lower_case_hexadecimal_digits() {
        let text = "00ff102030405060708090a0b0c0d0e0f00112233445566778899aabbccddeef";
        let parsed = hash(text);
        assert_eq!(parsed.0[0], 0x00);
        assert_eq!(parsed.0[1], 0xff);
        assert_eq!(parsed.0[31], 0xef);
        assert_eq!(parsed.to_string(), text);
        assert_eq!(format!("{parsed:?}"), format!("Hash32({text})"));

        assert_eq!("abc".parse::<Hash32>(), Err(HashError::Length { found: 3 }));
        assert_eq!(
            format!("{text}0").parse::<Hash32>(),
            Err(HashError::Length { found: 65 })
        );
        let upper = text.replacen("ff", "FF", 1);
        assert_eq!(
            upper.parse::<Hash32>(),
            Err(HashError::Digit {
                index: 2,
                character: 'F',
            })
        );
        let wide = format!("{}é", &text[..63]);
        assert_eq!(
            wide.parse::<Hash32>(),
            Err(HashError::Digit {
                index: 63,
                character: 'é',
            })
        );

        let json = serde_json::to_string(&parsed).expect("serialize");
        assert_eq!(json, format!("\"{text}\""));
        assert_eq!(serde_json::from_str::<Hash32>(&json).expect("read"), parsed);
        let error = serde_json::from_str::<Hash32>("\"abc\"").expect_err("too short");
        assert!(error.to_string().contains("3 characters"), "{error}");
    }

    /// The file of known answers that `conformance/hash_vectors.py` fills in.
    fn vectors() -> Value {
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../conformance/fixtures/hash/vectors.json");
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|error| panic!("{}: {error}", file.display()));
        serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", file.display()))
    }

    fn model<T: DeserializeOwned>(entry: &Value, key: &str) -> T {
        serde_json::from_value(entry[key].clone())
            .unwrap_or_else(|error| panic!("{}: {key}: {error}", entry["name"]))
    }

    fn digest(entry: &Value, pointer: &str) -> Hash32 {
        let text = entry
            .pointer(pointer)
            .and_then(Value::as_str)
            .unwrap_or_else(|| panic!("{}: no digest at {pointer}", entry["name"]));
        hash(text)
    }

    fn entries<'a>(vectors: &'a Value, section: &str) -> &'a [Value] {
        vectors[section]
            .as_array()
            .unwrap_or_else(|| panic!("no section {section}"))
    }

    fn layers(entry: &Value) -> LayerHashes {
        LayerHashes {
            operative: digest(entry, "/layers/operative"),
            enacted: digest(entry, "/layers/enacted"),
            editorial: digest(entry, "/layers/editorial"),
        }
    }

    #[test]
    fn known_answers_match_the_independent_implementation() {
        let vectors = vectors();
        let mut compared = 0;
        for entry in entries(&vectors, "provisions") {
            let provision: Provision = model(entry, "provision");
            assert_eq!(
                hash_provision(&provision),
                digest(entry, "/hash"),
                "{}",
                entry["name"]
            );
            compared += 1;
        }
        for entry in entries(&vectors, "notes") {
            let note: Note = model(entry, "note");
            assert_eq!(
                hash_note(&note),
                digest(entry, "/hash"),
                "{}",
                entry["name"]
            );
            compared += 1;
        }
        for entry in entries(&vectors, "units") {
            let unit: UnitModule = model(entry, "unit");
            assert_eq!(
                layer_hashes_of_unit(&unit),
                layers(entry),
                "{}",
                entry["name"]
            );
            compared += 1;
        }
        for entry in entries(&vectors, "containers") {
            let container: ContainerModule = model(entry, "container");
            assert_eq!(
                hash_container(&container),
                digest(entry, "/hash"),
                "{}",
                entry["name"]
            );
            assert_eq!(
                layer_hashes_of_container(&container),
                layers(entry),
                "{}",
                entry["name"]
            );
            compared += 1;
        }
        assert_eq!(compared, 11, "the file holds eleven models");
    }

    /// A root with two children, and two grandchildren under the first child.
    fn tree() -> Provision {
        let g1 = provision(
            "/zz/demo/t1/s9/a/1",
            "paragraph",
            num("1", "(1)"),
            None,
            vec![content(vec![
                para(vec![
                    words("See "),
                    link("section 2 of this title", "/zz/demo/t1/s2"),
                    words(" for more."),
                ]),
                Block::Table(Table {
                    rows: vec![
                        Row {
                            group: RowGroup::Head,
                            cells: vec![cell(true, "Kind"), cell(true, "Weight")],
                        },
                        Row {
                            group: RowGroup::Body,
                            cells: vec![cell(false, "widget"), cell(false, "1 gram")],
                        },
                    ],
                }),
            ])],
        );
        let g2 = provision(
            "/zz/demo/t1/s9/a/2",
            "paragraph",
            num("2", "(2)"),
            None,
            vec![content(vec![para(vec![words("x2 is the second.")])])],
        );
        let c1 = provision(
            "/zz/demo/t1/s9/a",
            "subsection",
            num("a", "(a)"),
            Some(plain_text("First")),
            vec![
                part(PartRole::Chapeau, vec![para(vec![words("In general:")])]),
                Item::Child(g1),
                Item::Child(g2),
            ],
        );
        let c2 = provision(
            "/zz/demo/t1/s9/b",
            "subsection",
            num("b", "(b)"),
            None,
            vec![content(vec![para(vec![words("The second subsection.")])])],
        );
        provision(
            "/zz/demo/t1/s9",
            "section",
            num("9", "§ 9."),
            Some(plain_text("Invented rules")),
            vec![
                part(
                    PartRole::Chapeau,
                    vec![para(vec![words("The invented rules are these:")])],
                ),
                Item::Child(c1),
                Item::Child(c2),
            ],
        )
    }

    fn cell(header: bool, text: &str) -> Cell {
        Cell {
            header,
            colspan: 1,
            rowspan: 1,
            blocks: vec![para(vec![words(text)])],
        }
    }

    /// The tree hash of every provision of the tree below `root`, by address.
    fn tree_hashes(root: &Provision) -> BTreeMap<Address, Hash32> {
        fn walk(provision: &Provision, hashes: &mut BTreeMap<Address, Hash32>) {
            hashes.insert(provision.address.clone(), hash_provision(provision));
            for item in &provision.items {
                if let Item::Child(child) = item {
                    walk(child, hashes);
                }
            }
        }
        let mut hashes = BTreeMap::new();
        walk(root, &mut hashes);
        hashes
    }

    fn node<'a>(provision: &'a mut Provision, target: &str) -> &'a mut Provision {
        if provision.address.as_str() == target {
            return provision;
        }
        provision
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Child(child) if target.starts_with(child.address.as_str()) => {
                    Some(node(child, target))
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("no provision {target}"))
    }

    fn blocks_mut(provision: &mut Provision) -> &mut Vec<Block> {
        provision
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Part { blocks, .. } => Some(blocks),
                Item::Child(_) => None,
            })
            .expect("a text part")
    }

    fn first_text(provision: &mut Provision) -> &mut Text {
        match blocks_mut(provision).first_mut() {
            Some(Block::Para(text)) => text,
            other => panic!("not a paragraph: {other:?}"),
        }
    }

    fn table(provision: &mut Provision) -> &mut Table {
        blocks_mut(provision)
            .iter_mut()
            .find_map(|block| match block {
                Block::Table(table) => Some(table),
                _ => None,
            })
            .expect("a table")
    }

    type Change = fn(&mut Provision);

    #[test]
    fn a_change_anywhere_changes_every_ancestor_and_nothing_else() {
        let g1 = "/zz/demo/t1/s9/a/1";
        let g2 = "/zz/demo/t1/s9/a/2";
        let c1 = "/zz/demo/t1/s9/a";
        let c2 = "/zz/demo/t1/s9/b";
        let root = "/zz/demo/t1/s9";
        let changes: Vec<(&str, &str, Change)> = vec![
            ("a word", g1, |p| {
                first_text(p).runs[2] = words(" for less.");
            }),
            ("num.value", g2, |p| {
                p.num.as_mut().expect("a number").value = "2a".to_owned();
            }),
            ("num.shown", c2, |p| {
                p.num.as_mut().expect("a number").shown = plain_text("(B)");
            }),
            ("the heading", c1, |p| {
                p.heading = Some(plain_text("Firsts"))
            }),
            ("the heading of the root", root, |p| p.heading = None),
            ("the status", g1, |p| p.status = Some("repealed".to_owned())),
            ("a link's target", g1, |p| {
                first_text(p).runs[1] = link("section 2 of this title", "/zz/demo/t1/s3");
            }),
            ("a link's extent over the same words", g1, |p| {
                let text = first_text(p);
                text.runs[0] = words("See section ");
                text.runs[1] = link("2 of this title", "/zz/demo/t1/s2");
            }),
            ("a superscript made of the same letters", g2, |p| {
                first_text(p).runs = vec![words("x"), sup("2"), words(" is the second.")];
            }),
            ("a table cell's text", g1, |p| {
                table(p).rows[1].cells[0].blocks = vec![para(vec![words("gadget")])];
            }),
            ("a colspan", g1, |p| table(p).rows[1].cells[0].colspan = 2),
            ("a rowspan", g1, |p| table(p).rows[1].cells[1].rowspan = 2),
            ("a header cell", g1, |p| {
                table(p).rows[1].cells[0].header = true
            }),
            ("a row's group", g1, |p| {
                table(p).rows[1].group = RowGroup::Foot
            }),
            ("a part's role", c2, |p| {
                if let Some(Item::Part { role, .. }) = p.items.first_mut() {
                    *role = PartRole::Chapeau;
                }
            }),
            ("the kind", c2, |p| p.kind = "paragraph".to_owned()),
            ("the order of two children", c1, |p| p.items.swap(1, 2)),
        ];

        let base = tree();
        let before = tree_hashes(&base);
        assert_eq!(before.len(), 5);
        for (what, at, change) in changes {
            let mut changed = base.clone();
            change(node(&mut changed, at));
            assert_ne!(changed, base, "{what}: the change changes nothing");
            let after = tree_hashes(&changed);
            assert_eq!(
                after.keys().collect::<Vec<_>>(),
                before.keys().collect::<Vec<_>>(),
                "{what}"
            );
            if what.ends_with("the same words") || what.ends_with("the same letters") {
                assert_eq!(
                    first_text(node(&mut changed.clone(), at)).plain(),
                    first_text(node(&mut base.clone(), at)).plain(),
                    "{what}: the words of the text change"
                );
            }
            let at = address(at);
            for (provision, hash) in &before {
                let expected_change = at.is_within(provision);
                assert_eq!(
                    after[provision] != *hash,
                    expected_change,
                    "{what} at {at}: the tree hash of {provision}"
                );
            }
        }
    }

    /// Every provision of the tree below `provision`, in document order.
    fn in_document_order<'a>(provision: &'a Provision, out: &mut Vec<&'a Provision>) {
        out.push(provision);
        for item in &provision.items {
            if let Item::Child(child) = item {
                in_document_order(child, out);
            }
        }
    }

    #[test]
    fn tree_hashes_are_the_tree_hash_of_every_provision_in_document_order() {
        for top in [deep_tree(), tree(), leaf()] {
            let mut provisions = Vec::new();
            in_document_order(&top, &mut provisions);
            let expected: Vec<Hash32> = provisions.iter().map(|p| hash_provision(p)).collect();
            assert_eq!(super::tree_hashes(&top), expected);
        }
        assert_eq!(super::tree_hashes(&tree()).len(), 5);
        assert_eq!(super::tree_hashes(&deep_tree()).len(), 9);
    }

    /// The tree of [`tree`] with a chain of four levels below its first
    /// grandchild.
    fn deep_tree() -> Provision {
        let mut deep = provision(
            "/zz/demo/t1/s9/a/1/A/i/I/aa",
            "item",
            None,
            None,
            vec![content(vec![para(vec![words("The deepest.")])])],
        );
        for at in [
            "/zz/demo/t1/s9/a/1/A/i/I",
            "/zz/demo/t1/s9/a/1/A/i",
            "/zz/demo/t1/s9/a/1/A",
        ] {
            deep = provision(at, "level", None, None, vec![Item::Child(deep)]);
        }
        let mut root = tree();
        node(&mut root, "/zz/demo/t1/s9/a/1")
            .items
            .push(Item::Child(deep));
        root
    }

    /// A provision with no child.
    fn leaf() -> Provision {
        provision("/zz/demo/t1/s9", "section", None, None, Vec::new())
    }

    fn unit(root: Provision, notes: Vec<Note>, footnote: &str) -> UnitModule {
        UnitModule {
            address: root.address.clone(),
            aliases: Vec::new(),
            within: Some(address("/zz/demo/t1/ch1")),
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            root,
            credit: Some(plain_text("(Added by an invented act.)")),
            notes,
            footnotes: vec![Footnote {
                ordinal: 1,
                shown: "1".to_owned(),
                blocks: vec![para(vec![words(footnote)])],
            }],
            definitions: Vec::new(),
        }
    }

    fn notes() -> Vec<Note> {
        vec![
            note(
                "/zz/demo/t1/s9/note/demoNotes/nolaw/short-title",
                Layer::Enacted,
                "Short Title",
                "This invented section may be cited as the Rule Act.",
            ),
            note(
                "/zz/demo/t1/s9/note/ungrouped/nolaw/codification",
                Layer::Editorial,
                "Codification",
                "This invented section was enacted elsewhere.",
            ),
            note(
                "/zz/demo/t1/s9/note/demoNotes/nolaw/effective-date",
                Layer::Enacted,
                "Effective Date",
                "This invented section takes effect on enactment.",
            ),
        ]
    }

    #[test]
    fn a_footnote_marker_is_outside_the_operative_hash() {
        let g1 = "/zz/demo/t1/s9/a/1";
        let c2 = "/zz/demo/t1/s9/b";
        let base = unit(tree(), notes(), "An invented footnote.");

        let with_mark = |at: &str, index: usize| {
            let mut changed = base.clone();
            first_text(node(&mut changed.root, at))
                .runs
                .insert(index, mark("1", 1));
            changed
        };
        let in_g1 = with_mark(g1, 1);
        let in_g1_elsewhere = with_mark(g1, 3);
        let in_c2 = with_mark(c2, 1);
        let mut footnote_changed = base.clone();
        footnote_changed.footnotes[0].blocks =
            vec![para(vec![words("Another invented footnote.")])];

        let layers = layer_hashes_of_unit(&base);
        let provisions = tree_hashes(&base.root);
        let mut editorials = vec![layers.editorial];
        for (what, changed) in [
            ("adding a marker", &in_g1),
            ("moving it to another provision", &in_c2),
            ("changing a footnote's text", &footnote_changed),
        ] {
            let changed_layers = layer_hashes_of_unit(changed);
            assert_eq!(tree_hashes(&changed.root), provisions, "{what}");
            assert_eq!(changed_layers.operative, layers.operative, "{what}");
            assert_eq!(changed_layers.enacted, layers.enacted, "{what}");
            assert!(
                !editorials.contains(&changed_layers.editorial),
                "{what}: the editorial layer does not change"
            );
            editorials.push(changed_layers.editorial);
        }
        // Where exactly in its provision a marker stands is in no hash.
        assert_eq!(
            layer_hashes_of_unit(&in_g1_elsewhere),
            layer_hashes_of_unit(&in_g1)
        );
    }

    #[test]
    fn a_superscript_changes_the_hash() {
        let with = |runs: Vec<Run>| {
            provision(
                "/zz/demo/t1/s9",
                "section",
                None,
                None,
                vec![content(vec![para(runs)])],
            )
        };
        let flat = with(vec![words("x2")]);
        let raised = with(vec![words("x"), sup("2")]);
        let lowered = with(vec![words("x"), sub("2")]);
        assert_ne!(hash_provision(&flat), hash_provision(&raised));
        assert_ne!(hash_provision(&flat), hash_provision(&lowered));
        assert_ne!(hash_provision(&raised), hash_provision(&lowered));
    }

    #[test]
    fn fields_cannot_be_confused() {
        let with = |value: &str, shown: &str, heading: Option<Text>| {
            provision(
                "/zz/demo/t1/s9",
                "section",
                num(value, shown),
                heading,
                vec![content(vec![para(vec![words("Invented text.")])])],
            )
        };
        assert_ne!(
            hash_provision(&with("ab", "c", None)),
            hash_provision(&with("a", "bc", None))
        );
        assert_ne!(
            hash_provision(&with("a", "b", None)),
            hash_provision(&with("a", "b", Some(Text::default())))
        );

        let mut absent = with("a", "b", None);
        absent.num = None;
        let mut empty = absent.clone();
        empty.num = num("", "");
        assert_ne!(hash_provision(&absent), hash_provision(&empty));

        let mut first = note(
            "/zz/demo/t1/s9/note/demoNotes/nolaw/a",
            Layer::Enacted,
            "A",
            "Invented.",
        );
        first.group = "ab".to_owned();
        first.topics = vec!["c".to_owned()];
        let mut second = first.clone();
        second.group = "a".to_owned();
        second.topics = vec!["bc".to_owned()];
        assert_ne!(hash_note(&first), hash_note(&second));
        let mut headless = first.clone();
        headless.heading = None;
        let mut empty_heading = first.clone();
        empty_heading.heading = Some(Text::default());
        assert_ne!(hash_note(&headless), hash_note(&empty_heading));
    }

    fn container(notes: Vec<Note>) -> ContainerModule {
        ContainerModule {
            address: address("/zz/demo/t1/ch1"),
            aliases: Vec::new(),
            within: None,
            source: "demo01.xml".to_owned(),
            positive_law: None,
            kind: "chapter".to_owned(),
            num: num("1", "CHAPTER 1"),
            heading: Some(plain_text("Invented Widgets")),
            status: None,
            members: vec![address("/zz/demo/t1/s9")],
            credit: Some(plain_text("(Invented act.)")),
            notes,
            footnotes: vec![Footnote {
                ordinal: 1,
                shown: "1".to_owned(),
                blocks: vec![para(vec![words("An invented footnote.")])],
            }],
        }
    }

    /// The notes of [`notes`], each with a footnote marker after its words,
    /// and the first also with one in its heading.
    fn notes_with_marks() -> Vec<Note> {
        let mut notes = notes();
        for note in &mut notes {
            if let Some(Block::Para(text)) = note.blocks.first_mut() {
                text.runs.push(mark("1", 1));
            }
        }
        if let Some(heading) = notes[0].heading.as_mut() {
            heading.runs.push(mark("1", 1));
        }
        notes
    }

    fn relocate(notes: &[Note]) -> Vec<Note> {
        notes
            .iter()
            .map(|note| {
                let mut note = note.clone();
                let tail = note.address.as_str()["/zz/demo/t1/s9".len()..].to_owned();
                note.address = address(&format!("/zz/demo/t1/ch1{tail}"));
                note
            })
            .collect()
    }

    #[test]
    fn note_order_does_not_change_a_layer_hash() {
        let plain = notes();
        let marked = notes_with_marks();
        // The markers are in the editorial layer: the two sets of notes differ
        // there and nowhere else.
        let plain_layers = layer_hashes_of_unit(&unit(tree(), plain.clone(), "A footnote."));
        let marked_layers = layer_hashes_of_unit(&unit(tree(), marked.clone(), "A footnote."));
        assert_ne!(plain_layers.editorial, marked_layers.editorial);
        assert_eq!(plain_layers.enacted, marked_layers.enacted);

        for (what, notes) in [("without markers", plain), ("each with markers", marked)] {
            let base = unit(tree(), notes.clone(), "An invented footnote.");
            let layers = layer_hashes_of_unit(&base);
            let container_layers = layer_hashes_of_container(&container(relocate(&notes)));
            for order in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                let reordered: Vec<Note> =
                    order.iter().map(|&index| notes[index].clone()).collect();
                let changed = unit(tree(), reordered.clone(), "An invented footnote.");
                assert_ne!(changed.notes, base.notes);
                assert_eq!(layer_hashes_of_unit(&changed), layers, "{what}, {order:?}");
                assert_eq!(
                    layer_hashes_of_container(&container(relocate(&reordered))),
                    container_layers,
                    "{what}, {order:?}"
                );
            }
        }
    }

    #[test]
    fn moving_a_note_changes_its_layer_and_not_its_hash() {
        let base = unit(tree(), notes(), "An invented footnote.");
        let layers = layer_hashes_of_unit(&base);
        for (index, layer) in [(0, Layer::Enacted), (1, Layer::Editorial)] {
            let mut moved = base.clone();
            assert_eq!(moved.notes[index].layer, layer);
            moved.notes[index].address = address("/zz/demo/t1/s9/a/note/demoNotes/nolaw/moved");
            assert_eq!(
                hash_note(&moved.notes[index]),
                hash_note(&base.notes[index])
            );
            let moved_layers = layer_hashes_of_unit(&moved);
            assert_eq!(moved_layers.operative, layers.operative);
            assert_eq!(
                moved_layers.enacted != layers.enacted,
                layer == Layer::Enacted,
                "{layer:?}"
            );
            assert_eq!(
                moved_layers.editorial != layers.editorial,
                layer == Layer::Editorial,
                "{layer:?}"
            );
        }
    }
}
