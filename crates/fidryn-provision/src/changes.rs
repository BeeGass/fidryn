//! Change sets: what changed between two releases of one corpus.
//!
//! A [`ChangeSet`] (`fidryn.change-set/v0.1`) lists, by layer, the
//! provisions, notes and containers that were added, removed, moved or
//! amended between two generated workspaces of one prefix, and the
//! bindings of curated workspaces that are stale against the newer one.
//! `fidryn_driver` builds it from the interfaces of the two workspaces,
//! opening only the modules whose hashes differ; this module holds what it
//! is made of and the comparisons of two modules:
//!
//! - [`diff_units`] compares the two versions of a unit provision by
//!   provision. A provision present in both is listed for what is its own:
//!   the words of its own text parts and tables (`amended_text`), and its
//!   kind, number, heading, status, the order of its text parts and the
//!   addresses of its children in their order, and the shape of its tables
//!   (`amended_structure`). A provision whose tree hash changed only
//!   because something below it changed is not listed. A footnote marker
//!   is no word of the text it stands in. Of the provisions on one side
//!   only, the highest are kept, those whose parent is on both sides.
//! - [`diff_containers`] compares the credit and the footnotes of the two
//!   versions of a container, as [`diff_units`] does those of a unit.
//! - [`note_changes`] compares the notes of one layer: it pairs like-named
//!   notes of one module by their note hashes first, and then compares the
//!   notes that remain by address.
//! - [`pair_moved`] makes one `moved` entry of a removed and an added entry
//!   of equal hashes: provisions by [`unnumbered_hash`], notes by their
//!   note hash, and among several candidates in the order of their
//!   addresses.
//!
//! [`ChangeSet::to_json`] writes the change set as pretty JSON, and
//! [`ChangeSet::to_text`] as the text of a commit message.

use crate::address::Address;
use crate::binding::StaleReason;
use crate::hash::{Hash32, marks_of_container, marks_of_unit, unnumbered_tree_hash};
use crate::model::{
    Block, Cell, ContainerModule, Footnote, Item, Layer, Num, PartRole, Provision, Quoted,
    QuotedItem, QuotedLevel, Row, RowGroup, Run, Table, Text, UnitModule,
};
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The `schema` of a change set.
pub const CHANGE_SET_SCHEMA: &str = "fidryn.change-set/v0.1";

/// One side of a change set: the release of the workspace and the BLAKE3 of
/// the bytes of its lock.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Release {
    pub release: String,
    pub lock_digest: Hash32,
}

/// A removed and an added provision or note that are one: what was at
/// `from` on the old side is at `to` on the new.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Moved {
    pub from: Address,
    pub to: Address,
}

/// What changed of the provisions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct OperativeChanges {
    /// The highest provisions on the new side only.
    pub added: Vec<Address>,
    /// The highest provisions on the old side only.
    pub removed: Vec<Address>,
    /// The provisions on both sides whose own words differ.
    pub amended_text: Vec<Address>,
    /// The provisions on both sides whose own structure differs.
    pub amended_structure: Vec<Address>,
    pub moved: Vec<Moved>,
}

/// What changed of the notes of one layer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct NoteChanges {
    pub added: Vec<Address>,
    pub removed: Vec<Address>,
    /// The notes at one address on both sides, once like-named notes are
    /// paired ([`note_changes`]), whose note hashes differ.
    pub amended: Vec<Address>,
    pub moved: Vec<Moved>,
}

/// What changed of the editorial matter: the editorial notes, and the units
/// and containers whose credit or whose footnotes changed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct EditorialChanges {
    #[serde(flatten)]
    pub notes: NoteChanges,
    /// The units and containers on both sides whose credit was added,
    /// removed or changed.
    pub credits_changed: Vec<Address>,
    /// The units and containers on both sides whose footnotes or footnote
    /// markers differ in any way.
    pub footnotes_changed: Vec<Address>,
}

/// What changed of the containers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct ContainerChanges {
    pub added: Vec<Address>,
    pub removed: Vec<Address>,
    /// The containers on both sides whose kind, number, heading, status,
    /// aliases or members differ.
    pub changed: Vec<Address>,
}

/// A binding of a curated workspace that is stale against the new side.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct StaleBinding {
    /// The name of the curated workspace.
    pub workspace: String,
    /// The address of the curated module that declares the binding.
    pub module: Address,
    /// The binding's name.
    pub binding: String,
    /// The address the binding binds.
    pub address: Address,
    #[serde(serialize_with = "reason_message")]
    pub reason: StaleReason,
}

fn reason_message<S: Serializer>(reason: &StaleReason, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(reason.message())
}

/// What changed between two generated workspaces of one prefix, and the
/// bindings that are stale against the newer one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChangeSet {
    pub prefix: Address,
    pub old: Release,
    pub new: Release,
    pub operative: OperativeChanges,
    pub enacted_notes: NoteChanges,
    pub editorial: EditorialChanges,
    pub containers: ContainerChanges,
    pub stale_bindings: Vec<StaleBinding>,
}

/// The JSON of a change set: its `schema`, then the change set.
#[derive(Serialize)]
struct ChangeSetFile<'a> {
    schema: &'static str,
    #[serde(flatten)]
    set: &'a ChangeSet,
}

impl ChangeSet {
    /// The change set between the releases `old` and `new` of the prefix
    /// `prefix` in which nothing changed.
    pub fn empty(prefix: Address, old: Release, new: Release) -> ChangeSet {
        ChangeSet {
            prefix,
            old,
            new,
            operative: OperativeChanges::default(),
            enacted_notes: NoteChanges::default(),
            editorial: EditorialChanges::default(),
            containers: ContainerChanges::default(),
            stale_bindings: Vec::new(),
        }
    }

    /// Sorts every list: the lists of addresses by the bytes of their
    /// addresses, the moved entries by the address they moved from and then
    /// by the one they moved to, and the stale bindings by the name of the
    /// workspace, the address of the module and the name of the binding; a
    /// stale binding found twice, in two copies of one curated workspace, is
    /// kept once.
    pub fn sort(&mut self) {
        let operative = &mut self.operative;
        for list in [
            &mut operative.added,
            &mut operative.removed,
            &mut operative.amended_text,
            &mut operative.amended_structure,
        ] {
            list.sort();
        }
        operative.moved.sort();
        for notes in [&mut self.enacted_notes, &mut self.editorial.notes] {
            for list in [&mut notes.added, &mut notes.removed, &mut notes.amended] {
                list.sort();
            }
            notes.moved.sort();
        }
        self.editorial.credits_changed.sort();
        self.editorial.footnotes_changed.sort();
        let containers = &mut self.containers;
        for list in [
            &mut containers.added,
            &mut containers.removed,
            &mut containers.changed,
        ] {
            list.sort();
        }
        self.stale_bindings.sort_by(|a, b| {
            (&a.workspace, &a.module, &a.binding, &a.address, a.reason).cmp(&(
                &b.workspace,
                &b.module,
                &b.binding,
                &b.address,
                b.reason,
            ))
        });
        self.stale_bindings.dedup();
    }

    /// The change set as JSON: `schema` first, then its fields in their
    /// order, every key present and every list as it is, indented by two
    /// spaces, with a final line end.
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(&ChangeSetFile {
            schema: CHANGE_SET_SCHEMA,
            set: self,
        })
        .expect("a change set of strings, digests and addresses is written");
        text.push('\n');
        text
    }

    /// The change set as the text of a commit message: a first line
    /// `<prefix>: <old release> to <new release>`; a blank line; one line of
    /// counts for each of the four groups, operative, enacted notes,
    /// editorial and containers; then each list that is not empty after a
    /// blank line, its name as its key path with a colon, and one entry to a
    /// line: an address, a moved entry as `<from> to <to>`, and a stale
    /// binding as `<address>: <binding> of <module> in <workspace>
    /// (<reason>)`. The text ends with one line end.
    pub fn to_text(&self) -> String {
        let mut text = format!(
            "{}: {} to {}\n\n",
            self.prefix, self.old.release, self.new.release
        );
        let counts: [(&str, Vec<(&str, usize)>); 4] = [
            (
                "operative",
                vec![
                    ("added", self.operative.added.len()),
                    ("removed", self.operative.removed.len()),
                    ("amended_text", self.operative.amended_text.len()),
                    ("amended_structure", self.operative.amended_structure.len()),
                    ("moved", self.operative.moved.len()),
                ],
            ),
            ("enacted_notes", note_counts(&self.enacted_notes)),
            ("editorial", {
                let mut counts = note_counts(&self.editorial.notes);
                counts.push(("credits_changed", self.editorial.credits_changed.len()));
                counts.push(("footnotes_changed", self.editorial.footnotes_changed.len()));
                counts
            }),
            (
                "containers",
                vec![
                    ("added", self.containers.added.len()),
                    ("removed", self.containers.removed.len()),
                    ("changed", self.containers.changed.len()),
                ],
            ),
        ];
        for (group, counts) in counts {
            let counts: Vec<String> = counts
                .into_iter()
                .map(|(name, count)| format!("{count} {name}"))
                .collect();
            text.push_str(&format!("{group}: {}\n", counts.join(", ")));
        }
        for (name, lines) in self.lists() {
            if lines.is_empty() {
                continue;
            }
            text.push('\n');
            text.push_str(&name);
            text.push_str(":\n");
            for line in lines {
                text.push_str(&line);
                text.push('\n');
            }
        }
        text
    }

    /// Every list of the change set, by its key path, with one line for each
    /// of its entries, in the order of the JSON.
    fn lists(&self) -> Vec<(String, Vec<String>)> {
        let addresses =
            |list: &[Address]| -> Vec<String> { list.iter().map(ToString::to_string).collect() };
        let moved = |list: &[Moved]| -> Vec<String> {
            list.iter()
                .map(|moved| format!("{} to {}", moved.from, moved.to))
                .collect()
        };
        let notes = |group: &str, notes: &NoteChanges| {
            vec![
                (format!("{group}.added"), addresses(&notes.added)),
                (format!("{group}.removed"), addresses(&notes.removed)),
                (format!("{group}.amended"), addresses(&notes.amended)),
                (format!("{group}.moved"), moved(&notes.moved)),
            ]
        };
        let operative = &self.operative;
        let mut lists = vec![
            ("operative.added".to_owned(), addresses(&operative.added)),
            (
                "operative.removed".to_owned(),
                addresses(&operative.removed),
            ),
            (
                "operative.amended_text".to_owned(),
                addresses(&operative.amended_text),
            ),
            (
                "operative.amended_structure".to_owned(),
                addresses(&operative.amended_structure),
            ),
            ("operative.moved".to_owned(), moved(&operative.moved)),
        ];
        lists.extend(notes("enacted_notes", &self.enacted_notes));
        lists.extend(notes("editorial", &self.editorial.notes));
        lists.push((
            "editorial.credits_changed".to_owned(),
            addresses(&self.editorial.credits_changed),
        ));
        lists.push((
            "editorial.footnotes_changed".to_owned(),
            addresses(&self.editorial.footnotes_changed),
        ));
        lists.push((
            "containers.added".to_owned(),
            addresses(&self.containers.added),
        ));
        lists.push((
            "containers.removed".to_owned(),
            addresses(&self.containers.removed),
        ));
        lists.push((
            "containers.changed".to_owned(),
            addresses(&self.containers.changed),
        ));
        lists.push((
            "stale_bindings".to_owned(),
            self.stale_bindings
                .iter()
                .map(|stale| {
                    format!(
                        "{}: {} of {} in {} ({})",
                        stale.address, stale.binding, stale.module, stale.workspace, stale.reason
                    )
                })
                .collect(),
        ));
        lists
    }
}

fn note_counts(notes: &NoteChanges) -> Vec<(&'static str, usize)> {
    vec![
        ("added", notes.added.len()),
        ("removed", notes.removed.len()),
        ("amended", notes.amended.len()),
        ("moved", notes.moved.len()),
    ]
}

/// A highest provision on one side only, as [`pair_moved`] takes it, with
/// its kind and its status, which the hash it is paired by holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OneSided {
    pub address: Address,
    pub kind: String,
    pub status: Option<String>,
    /// Its [`unnumbered_hash`].
    pub unnumbered: Hash32,
}

/// Whether the credit and the footnotes of a unit or a container changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatterDiff {
    /// The credit was added, removed, or changed in its words, its links,
    /// its superscripts or its subscripts; its footnote markers count for
    /// `footnotes_changed`.
    pub credit_changed: bool,
    /// A footnote was added, removed or changed, or a footnote marker of
    /// the module was added, removed, or stands in another provision or
    /// note, or shows another mark, or points at another footnote.
    pub footnotes_changed: bool,
}

/// What [`diff_units`] finds between two versions of a unit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitDiff {
    /// The provisions of both versions whose own words differ, in the order
    /// of their addresses.
    pub amended_text: Vec<Address>,
    /// The provisions of both versions whose own structure differs, in the
    /// order of their addresses.
    pub amended_structure: Vec<Address>,
    /// The highest provisions of the old version that the new does not hold,
    /// in document order.
    pub removed: Vec<OneSided>,
    /// The highest provisions of the new version that the old does not hold,
    /// in document order.
    pub added: Vec<OneSided>,
    pub matter: MatterDiff,
}

/// The tree hash of `provision` computed with its own number absent: with
/// the frame of no number, `F(0x04, empty)`, in place of its number, and
/// every child's tree hash as it is. A provision renumbered and otherwise
/// unchanged keeps it.
pub fn unnumbered_hash(provision: &Provision) -> Hash32 {
    unnumbered_tree_hash(provision)
}

/// The provisions of `old` and `new`, two versions of one unit, compared,
/// and their credits and footnotes.
///
/// A provision whose address both trees hold is compared with the one at
/// its address. It is in `amended_text` when its own words differ: in
/// order, the text of each paragraph and subheading of its own text parts,
/// those in the cells of its tables included, with its links, superscripts
/// and subscripts, each quoted content whole, and the source of each image. It is in `amended_structure` when its own
/// structure differs: its kind, number, heading and status, and its items
/// in order, each text part by its role and the kind of each of its blocks,
/// a table by the group of each row and the header and the spans of each
/// cell, and each child by its address. Footnote markers are in neither,
/// and nothing of a child is. A provision that one tree holds and the other
/// does not is kept when its parent is in both, or when it is the top
/// provision, with its [`unnumbered_hash`].
pub fn diff_units(old: &UnitModule, new: &UnitModule) -> UnitDiff {
    let old_tree = by_address(&old.root);
    let new_tree = by_address(&new.root);
    let mut diff = UnitDiff::default();
    for provision in old_tree.values() {
        let Some(other) = new_tree.get(&provision.address) else {
            continue;
        };
        if own_words(provision) != own_words(other) {
            diff.amended_text.push(provision.address.clone());
        }
        if own_structure(provision) != own_structure(other) {
            diff.amended_structure.push(provision.address.clone());
        }
    }
    highest_of(&old.root, &new_tree, false, &mut diff.removed);
    highest_of(&new.root, &old_tree, false, &mut diff.added);
    diff.matter = MatterDiff {
        credit_changed: unmarked_credit(old.credit.as_ref())
            != unmarked_credit(new.credit.as_ref()),
        footnotes_changed: unmarked_footnotes(&old.footnotes) != unmarked_footnotes(&new.footnotes)
            || marks_of_unit(old) != marks_of_unit(new),
    };
    diff
}

/// The credit and the footnotes of `old` and `new`, two versions of one
/// container, compared, as [`diff_units`] compares those of a unit.
pub fn diff_containers(old: &ContainerModule, new: &ContainerModule) -> MatterDiff {
    MatterDiff {
        credit_changed: unmarked_credit(old.credit.as_ref())
            != unmarked_credit(new.credit.as_ref()),
        footnotes_changed: unmarked_footnotes(&old.footnotes) != unmarked_footnotes(&new.footnotes)
            || marks_of_container(old) != marks_of_container(new),
    }
}

/// Every provision of the tree below `root`, `root` included, by address.
fn by_address(root: &Provision) -> BTreeMap<&Address, &Provision> {
    fn walk<'a>(provision: &'a Provision, out: &mut BTreeMap<&'a Address, &'a Provision>) {
        out.insert(&provision.address, provision);
        for child in children(provision) {
            walk(child, out);
        }
    }
    let mut out = BTreeMap::new();
    walk(root, &mut out);
    out
}

fn children(provision: &Provision) -> impl Iterator<Item = &Provision> {
    provision.items.iter().filter_map(|item| match item {
        Item::Child(child) => Some(child),
        Item::Part { .. } => None,
    })
}

/// Adds to `out`, in document order, each provision of the tree below
/// `provision` whose address `other` does not hold and whose parent it does
/// hold, or that has no parent; `parent_gone` says whether the parent of
/// `provision` is one `other` does not hold.
fn highest_of(
    provision: &Provision,
    other: &BTreeMap<&Address, &Provision>,
    parent_gone: bool,
    out: &mut Vec<OneSided>,
) {
    let gone = !other.contains_key(&provision.address);
    if gone && !parent_gone {
        out.push(OneSided {
            address: provision.address.clone(),
            kind: provision.kind.clone(),
            status: provision.status.clone(),
            unnumbered: unnumbered_hash(provision),
        });
    }
    for child in children(provision) {
        highest_of(child, other, gone, out);
    }
}

/// `text` without its footnote markers, whose runs on either side of a
/// marker are joined: the text that its words, links, superscripts and
/// subscripts make.
fn unmarked(text: &Text) -> Text {
    Text::from_runs(
        text.runs
            .iter()
            .filter(|run| !matches!(run, Run::Mark { .. }))
            .cloned()
            .collect(),
    )
}

fn unmarked_num(num: &Num) -> Num {
    Num {
        value: num.value.clone(),
        shown: unmarked(&num.shown),
    }
}

fn unmarked_blocks(blocks: &[Block]) -> Vec<Block> {
    blocks.iter().map(unmarked_block).collect()
}

fn unmarked_block(block: &Block) -> Block {
    match block {
        Block::Para(text) => Block::Para(unmarked(text)),
        Block::Subheading(text) => Block::Subheading(unmarked(text)),
        Block::Table(table) => Block::Table(Table {
            rows: table
                .rows
                .iter()
                .map(|row| Row {
                    group: row.group,
                    cells: row
                        .cells
                        .iter()
                        .map(|cell| Cell {
                            header: cell.header,
                            colspan: cell.colspan,
                            rowspan: cell.rowspan,
                            blocks: unmarked_blocks(&cell.blocks),
                        })
                        .collect(),
                })
                .collect(),
        }),
        Block::Quoted(quoted) => Block::Quoted(unmarked_quoted(quoted)),
        Block::Image { source } => Block::Image {
            source: source.clone(),
        },
    }
}

fn unmarked_quoted(quoted: &Quoted) -> Quoted {
    Quoted {
        origin: quoted.origin.clone(),
        items: unmarked_quoted_items(&quoted.items),
    }
}

fn unmarked_quoted_items(items: &[QuotedItem]) -> Vec<QuotedItem> {
    items
        .iter()
        .map(|item| match item {
            QuotedItem::Block(block) => QuotedItem::Block(unmarked_block(block)),
            QuotedItem::Part { role, blocks } => QuotedItem::Part {
                role: *role,
                blocks: unmarked_blocks(blocks),
            },
            QuotedItem::Level(level) => QuotedItem::Level(QuotedLevel {
                kind: level.kind.clone(),
                num: level.num.as_ref().map(unmarked_num),
                heading: level.heading.as_ref().map(unmarked),
                items: unmarked_quoted_items(&level.items),
            }),
        })
        .collect()
}

fn unmarked_credit(credit: Option<&Text>) -> Option<Text> {
    credit.map(unmarked)
}

/// Each footnote's ordinal, shown mark and blocks, without the footnote
/// markers the blocks hold, which the markers of the module compare.
fn unmarked_footnotes(footnotes: &[Footnote]) -> Vec<(u32, &str, Vec<Block>)> {
    footnotes
        .iter()
        .map(|footnote| {
            (
                footnote.ordinal,
                footnote.shown.as_str(),
                unmarked_blocks(&footnote.blocks),
            )
        })
        .collect()
}

/// What of a provision is its own words: in order, the text of each
/// paragraph and subheading of its own text parts, those in the cells of
/// its tables included, each quoted content whole, and the source of each
/// image; footnote markers left out. Nothing of its children is in it.
#[derive(Debug, PartialEq, Eq)]
enum Word<'a> {
    Text(Text),
    Quoted(Quoted),
    Image(&'a str),
}

fn own_words(provision: &Provision) -> Vec<Word<'_>> {
    fn of_blocks<'a>(blocks: &'a [Block], out: &mut Vec<Word<'a>>) {
        for block in blocks {
            match block {
                Block::Para(text) | Block::Subheading(text) => out.push(Word::Text(unmarked(text))),
                Block::Table(table) => {
                    for cell in table.rows.iter().flat_map(|row| &row.cells) {
                        of_blocks(&cell.blocks, out);
                    }
                }
                Block::Quoted(quoted) => out.push(Word::Quoted(unmarked_quoted(quoted))),
                Block::Image { source } => out.push(Word::Image(source)),
            }
        }
    }
    let mut out = Vec::new();
    for item in &provision.items {
        if let Item::Part { blocks, .. } = item {
            of_blocks(blocks, &mut out);
        }
    }
    out
}

/// What of a provision is its own structure: its kind, its number, its
/// heading and its status, footnote markers left out; and its items in
/// order, each text part by its role and the kind of each of its blocks, a
/// table by the group of each row and the header and spans of each cell,
/// and each child by its address.
#[derive(Debug, PartialEq, Eq)]
struct Structure<'a> {
    kind: &'a str,
    num: Option<Num>,
    heading: Option<Text>,
    status: Option<&'a str>,
    items: Vec<ItemShape<'a>>,
}

#[derive(Debug, PartialEq, Eq)]
enum ItemShape<'a> {
    Part(PartRole, Vec<BlockShape>),
    Child(&'a Address),
}

#[derive(Debug, PartialEq, Eq)]
enum BlockShape {
    Para,
    Subheading,
    Table(Vec<(RowGroup, Vec<CellShape>)>),
    Quoted,
    Image,
}

#[derive(Debug, PartialEq, Eq)]
struct CellShape {
    header: bool,
    colspan: u32,
    rowspan: u32,
    blocks: Vec<BlockShape>,
}

fn own_structure(provision: &Provision) -> Structure<'_> {
    fn of_blocks(blocks: &[Block]) -> Vec<BlockShape> {
        blocks
            .iter()
            .map(|block| match block {
                Block::Para(_) => BlockShape::Para,
                Block::Subheading(_) => BlockShape::Subheading,
                Block::Table(table) => BlockShape::Table(
                    table
                        .rows
                        .iter()
                        .map(|row| {
                            let cells = row
                                .cells
                                .iter()
                                .map(|cell| CellShape {
                                    header: cell.header,
                                    colspan: cell.colspan,
                                    rowspan: cell.rowspan,
                                    blocks: of_blocks(&cell.blocks),
                                })
                                .collect();
                            (row.group, cells)
                        })
                        .collect(),
                ),
                Block::Quoted(_) => BlockShape::Quoted,
                Block::Image { .. } => BlockShape::Image,
            })
            .collect()
    }
    Structure {
        kind: &provision.kind,
        num: provision.num.as_ref().map(unmarked_num),
        heading: provision.heading.as_ref().map(unmarked),
        status: provision.status.as_deref(),
        items: provision
            .items
            .iter()
            .map(|item| match item {
                Item::Part { role, blocks } => ItemShape::Part(*role, of_blocks(blocks)),
                Item::Child(child) => ItemShape::Child(&child.address),
            })
            .collect(),
    }
}

/// A note of one side of a change set, as [`note_changes`] takes it: the
/// address of the unit or container that holds it, its own address, its
/// layer and its note hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteAt {
    pub module: Address,
    pub address: Address,
    pub layer: Layer,
    pub hash: Hash32,
}

/// The notes of the layer `layer` from `old` to `new`, each side given
/// whole, every layer included.
///
/// Like-named notes are paired first. Two notes of one module and one side
/// are of one family when their addresses differ only in the ordinal that
/// marks like-named notes: a last segment that is a whole number of 2 or
/// more, written in ASCII digits with no leading zero, after an address
/// that is itself the address of a note of the module on that side. Within
/// one module, the layer `layer` and one family, an old and a new note of
/// equal hashes are paired: each old note, in the order of the addresses,
/// with the first new one, in the order of the addresses, that has an equal
/// hash and is not paired yet. A pair at one address is unchanged and is
/// listed nowhere; a pair at two addresses is one `moved` entry. So a note
/// inserted before two like-named notes is `added` at the address of the
/// first, and the two have moved, each to the ordinal after its own.
///
/// The notes that remain are compared by address: one at an address that a
/// remaining note on the other side has is `amended`, and one at an address
/// that none has is `removed` or `added`. A removed and an added note of
/// equal hashes are then one `moved` entry, as [`pair_moved`] pairs them,
/// across the whole workspace. No note is paired by its hash across modules
/// or families before the notes are compared by address, since many notes
/// are the same words in different units. Every list is sorted.
pub fn note_changes(old: &[NoteAt], new: &[NoteAt], layer: Layer) -> NoteChanges {
    // The notes of `layer` of one side, each with its family, by module and
    // family, and within each in the order of their addresses.
    fn families(side: &[NoteAt], layer: Layer) -> BTreeMap<(&Address, &str), Vec<&NoteAt>> {
        let held: BTreeSet<(&Address, &str)> = side
            .iter()
            .map(|note| (&note.module, note.address.as_str()))
            .collect();
        let mut families: BTreeMap<(&Address, &str), Vec<&NoteAt>> = BTreeMap::new();
        for note in side.iter().filter(|note| note.layer == layer) {
            let text = note.address.as_str();
            let family = match text.rsplit_once('/') {
                Some((base, last)) if is_ordinal(last) && held.contains(&(&note.module, base)) => {
                    base
                }
                _ => text,
            };
            families
                .entry((&note.module, family))
                .or_default()
                .push(note);
        }
        for notes in families.values_mut() {
            notes.sort_by(|a, b| a.address.cmp(&b.address));
        }
        families
    }
    let (old_families, new_families) = (families(old, layer), families(new, layer));
    let mut changes = NoteChanges::default();
    let mut paired_old: BTreeSet<&Address> = BTreeSet::new();
    let mut paired_new: BTreeSet<&Address> = BTreeSet::new();
    for (key, old_notes) in &old_families {
        let Some(new_notes) = new_families.get(key) else {
            continue;
        };
        for before in old_notes {
            let Some(after) = new_notes
                .iter()
                .find(|after| after.hash == before.hash && !paired_new.contains(&after.address))
            else {
                continue;
            };
            paired_old.insert(&before.address);
            paired_new.insert(&after.address);
            if before.address != after.address {
                changes.moved.push(Moved {
                    from: before.address.clone(),
                    to: after.address.clone(),
                });
            }
        }
    }
    let remaining = |families: &BTreeMap<(&Address, &str), Vec<&NoteAt>>,
                     paired: &BTreeSet<&Address>| {
        families
            .values()
            .flatten()
            .filter(|note| !paired.contains(&note.address))
            .map(|note| (note.address.clone(), note.hash))
            .collect::<BTreeMap<Address, Hash32>>()
    };
    let (old_left, new_left) = (
        remaining(&old_families, &paired_old),
        remaining(&new_families, &paired_new),
    );
    let mut removed = Vec::new();
    for (address, hash) in &old_left {
        match new_left.get(address) {
            Some(other) if other != hash => changes.amended.push(address.clone()),
            Some(_) => {}
            None => removed.push((address.clone(), Some(*hash))),
        }
    }
    let added = new_left
        .iter()
        .filter(|(address, _)| !old_left.contains_key(*address))
        .map(|(address, hash)| (address.clone(), Some(*hash)))
        .collect();
    let paired = pair_moved(removed, added);
    changes.removed = paired.removed;
    changes.added = paired.added;
    changes.moved.extend(paired.moved);
    changes.amended.sort();
    changes.moved.sort();
    changes
}

/// Whether `segment` is an ordinal that marks a like-named note: a whole
/// number of 2 or more, written in ASCII digits with no leading zero.
fn is_ordinal(segment: &str) -> bool {
    !segment.is_empty()
        && !segment.starts_with('0')
        && segment.bytes().all(|byte| byte.is_ascii_digit())
        && segment != "1"
}

/// What [`pair_moved`] gives: the entries that are removed and added, and
/// those that moved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Paired {
    pub removed: Vec<Address>,
    pub added: Vec<Address>,
    pub moved: Vec<Moved>,
}

/// The `removed` and `added` entries, each an address and the hash it is
/// paired by, paired: each removed entry, in the order of the addresses,
/// is paired with the first added entry in the order of the addresses that
/// has an equal hash and is not paired yet, and the two are one `moved`
/// entry. An entry without a hash is never paired. Every list is sorted.
pub fn pair_moved(
    mut removed: Vec<(Address, Option<Hash32>)>,
    mut added: Vec<(Address, Option<Hash32>)>,
) -> Paired {
    removed.sort_by(|a, b| a.0.cmp(&b.0));
    added.sort_by(|a, b| a.0.cmp(&b.0));
    // The places of the added entries of each hash, in the order of their
    // addresses.
    let mut waiting: BTreeMap<Hash32, VecDeque<usize>> = BTreeMap::new();
    for (place, (_, hash)) in added.iter().enumerate() {
        if let Some(hash) = hash {
            waiting.entry(*hash).or_default().push_back(place);
        }
    }
    let mut taken = vec![false; added.len()];
    let mut paired = Paired::default();
    for (address, hash) in removed {
        let partner = hash
            .and_then(|hash| waiting.get_mut(&hash))
            .and_then(VecDeque::pop_front);
        match partner {
            Some(place) => {
                taken[place] = true;
                paired.moved.push(Moved {
                    from: address,
                    to: added[place].0.clone(),
                });
            }
            None => paired.removed.push(address),
        }
    }
    paired.added = added
        .into_iter()
        .zip(taken)
        .filter(|(_, taken)| !taken)
        .map(|((address, _), _)| address)
        .collect();
    paired
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_provision;
    use crate::model::build::*;

    /// A section `/zz/demo/t1/s<value>` with one subsection.
    fn section(value: &str, subsection_value: &str) -> Provision {
        let at = format!("/zz/demo/t1/s{value}");
        let child = provision(
            &format!("{at}/{subsection_value}"),
            "subsection",
            num(subsection_value, &format!("({subsection_value})")),
            None,
            vec![content(vec![para(vec![words("An invented subsection.")])])],
        );
        provision(
            &at,
            "section",
            num(value, &format!("§ {value}.")),
            Some(plain_text("Invented Rules")),
            vec![
                content(vec![para(vec![words("The invented rules are these.")])]),
                Item::Child(child),
            ],
        )
    }

    #[test]
    fn the_unnumbered_hash_leaves_out_the_number_of_the_provision_alone() {
        let five = section("5", "a");
        let six = section("6", "a");
        assert_ne!(hash_provision(&five), hash_provision(&six));
        assert_eq!(unnumbered_hash(&five), unnumbered_hash(&six));
        // It is the tree hash of the provision without a number.
        let mut unnumbered = five.clone();
        unnumbered.num = None;
        assert_eq!(unnumbered_hash(&five), hash_provision(&unnumbered));
        assert_ne!(unnumbered_hash(&five), hash_provision(&five));
        // The numbers of its children are in it.
        assert_ne!(unnumbered_hash(&five), unnumbered_hash(&section("5", "b")));
        // And so is everything else of its own.
        let mut headed = six.clone();
        headed.heading = Some(plain_text("Other Rules"));
        assert_ne!(unnumbered_hash(&five), unnumbered_hash(&headed));
    }

    fn address_list(texts: &[&str]) -> Vec<Address> {
        texts.iter().map(|text| address(text)).collect()
    }

    /// The note at `at` of the unit `/zz/demo/t1/s<module>`, in `layer`,
    /// whose hash stands for the words `words`.
    fn note_in(module: &str, at: &str, layer: Layer, words: &str) -> NoteAt {
        let module = format!("/zz/demo/t1/s{module}");
        NoteAt {
            address: address(&format!("{module}/note/demoNotes/nolaw/{at}")),
            module: address(&module),
            layer,
            hash: Hash32(*blake3::hash(words.as_bytes()).as_bytes()),
        }
    }

    fn enacted(at: &str, words: &str) -> NoteAt {
        note_in("1", at, Layer::Enacted, words)
    }

    fn of_s1(at: &str) -> String {
        format!("/zz/demo/t1/s1/note/demoNotes/nolaw/{at}")
    }

    fn move_of(from: &str, to: &str) -> Moved {
        Moved {
            from: address(&of_s1(from)),
            to: address(&of_s1(to)),
        }
    }

    fn of_s1_list(texts: &[&str]) -> Vec<Address> {
        texts.iter().map(|text| address(&of_s1(text))).collect()
    }

    #[test]
    fn a_note_inserted_before_two_like_named_notes_is_added_and_they_have_moved() {
        let old = [enacted("effective", "A"), enacted("effective/2", "B")];
        let new = [
            enacted("effective", "N"),
            enacted("effective/2", "A"),
            enacted("effective/3", "B"),
        ];
        assert_eq!(
            note_changes(&old, &new, Layer::Enacted),
            NoteChanges {
                added: of_s1_list(&["effective"]),
                removed: Vec::new(),
                amended: Vec::new(),
                moved: vec![
                    move_of("effective", "effective/2"),
                    move_of("effective/2", "effective/3"),
                ],
            }
        );
        // Nothing of the other layer is listed.
        assert_eq!(
            note_changes(&old, &new, Layer::Editorial),
            NoteChanges::default()
        );
    }

    #[test]
    fn a_note_amended_while_another_is_inserted_before_it_is_compared_by_address() {
        // The first note is amended, and a note is inserted before it: no new
        // note has its hash, so the notes that remain are compared by address.
        let old = [enacted("effective", "A"), enacted("effective/2", "B")];
        let new = [
            enacted("effective", "N"),
            enacted("effective/2", "A amended"),
            enacted("effective/3", "B"),
        ];
        assert_eq!(
            note_changes(&old, &new, Layer::Enacted),
            NoteChanges {
                added: of_s1_list(&["effective/2"]),
                removed: Vec::new(),
                amended: of_s1_list(&["effective"]),
                moved: vec![move_of("effective/2", "effective/3")],
            }
        );
    }

    #[test]
    fn a_pair_at_one_address_is_unchanged_and_equal_notes_pair_in_address_order() {
        let old = [enacted("effective", "A"), enacted("effective/2", "A")];
        let new = [
            enacted("effective", "A"),
            enacted("effective/2", "A"),
            enacted("effective/3", "A"),
        ];
        assert_eq!(
            note_changes(&old, &new, Layer::Enacted),
            NoteChanges {
                added: of_s1_list(&["effective/3"]),
                ..NoteChanges::default()
            }
        );
        // Two like-named notes that trade places have both moved.
        let new = [enacted("effective", "B"), enacted("effective/2", "A")];
        let old = [enacted("effective", "A"), enacted("effective/2", "B")];
        assert_eq!(
            note_changes(&old, &new, Layer::Enacted).moved,
            [
                move_of("effective", "effective/2"),
                move_of("effective/2", "effective")
            ]
        );
    }

    #[test]
    fn notes_are_paired_by_their_hash_only_within_one_module_and_one_family() {
        // The new words of one note are the old words of a note of another
        // unit, and of a note of another family of the same unit: each is
        // amended at its address, and none has moved.
        let old = [
            note_in("1", "effective", Layer::Enacted, "A"),
            note_in("1", "short-title", Layer::Enacted, "B"),
            note_in("2", "effective", Layer::Enacted, "C"),
        ];
        let new = [
            note_in("1", "effective", Layer::Enacted, "B"),
            note_in("1", "short-title", Layer::Enacted, "C"),
            note_in("2", "effective", Layer::Enacted, "A"),
        ];
        let found = note_changes(&old, &new, Layer::Enacted);
        assert_eq!(found.moved, []);
        assert_eq!(
            found.amended,
            [
                address(&of_s1("effective")),
                address(&of_s1("short-title")),
                address("/zz/demo/t1/s2/note/demoNotes/nolaw/effective"),
            ]
        );
        // A note on one side only still pairs with one of equal hash across
        // the workspace, as a note of a renumbered section does.
        let old = [note_in("1", "effective", Layer::Enacted, "A")];
        let new = [note_in("2", "effective", Layer::Enacted, "A")];
        assert_eq!(
            note_changes(&old, &new, Layer::Enacted).moved,
            [Moved {
                from: address(&of_s1("effective")),
                to: address("/zz/demo/t1/s2/note/demoNotes/nolaw/effective"),
            }]
        );
    }

    #[test]
    fn the_family_of_a_note_is_named_by_a_note_whatever_its_layer() {
        // The first of three like-named notes is editorial and the others
        // enacted; a note inserted before them shifts both layers.
        let old = [
            note_in("1", "effective", Layer::Editorial, "E"),
            note_in("1", "effective/2", Layer::Enacted, "A"),
        ];
        let new = [
            note_in("1", "effective", Layer::Editorial, "N"),
            note_in("1", "effective/2", Layer::Editorial, "E"),
            note_in("1", "effective/3", Layer::Enacted, "A"),
        ];
        assert_eq!(
            note_changes(&old, &new, Layer::Enacted).moved,
            [move_of("effective/2", "effective/3")]
        );
        assert_eq!(
            note_changes(&old, &new, Layer::Editorial),
            NoteChanges {
                added: of_s1_list(&["effective"]),
                moved: vec![move_of("effective", "effective/2")],
                ..NoteChanges::default()
            }
        );
        // A last segment of digits is an ordinal only after the address of a
        // note: a note whose heading gives the segment `2` begins a family of
        // its own, and its like-named notes follow it.
        let old = [enacted("2", "A"), enacted("2/2", "B")];
        let new = [enacted("2", "N"), enacted("2/2", "A"), enacted("2/3", "B")];
        assert_eq!(
            note_changes(&old, &new, Layer::Enacted),
            NoteChanges {
                added: of_s1_list(&["2"]),
                moved: vec![move_of("2", "2/2"), move_of("2/2", "2/3")],
                ..NoteChanges::default()
            }
        );
        for (segment, ordinal) in [
            ("2", true),
            ("10", true),
            ("1", false),
            ("0", false),
            ("02", false),
            ("", false),
            ("2a", false),
        ] {
            assert_eq!(is_ordinal(segment), ordinal, "{segment:?}");
        }
    }

    #[test]
    fn candidates_are_paired_in_address_order() {
        let [one, two, three] =
            ["1", "2", "3"].map(|text| Some(Hash32(*blake3::hash(text.as_bytes()).as_bytes())));
        let entries = |list: &[(&str, Option<Hash32>)]| -> Vec<(Address, Option<Hash32>)> {
            list.iter().map(|(at, hash)| (address(at), *hash)).collect()
        };
        let paired = pair_moved(
            entries(&[
                ("/zz/demo/b", one),
                ("/zz/demo/a", one),
                ("/zz/demo/c", None),
                ("/zz/demo/d", two),
            ]),
            entries(&[
                ("/zz/demo/y", one),
                ("/zz/demo/x", one),
                ("/zz/demo/w", None),
                ("/zz/demo/z", three),
            ]),
        );
        assert_eq!(
            paired,
            Paired {
                removed: address_list(&["/zz/demo/c", "/zz/demo/d"]),
                added: address_list(&["/zz/demo/w", "/zz/demo/z"]),
                moved: vec![
                    Moved {
                        from: address("/zz/demo/a"),
                        to: address("/zz/demo/x"),
                    },
                    Moved {
                        from: address("/zz/demo/b"),
                        to: address("/zz/demo/y"),
                    },
                ],
            }
        );
    }
}
