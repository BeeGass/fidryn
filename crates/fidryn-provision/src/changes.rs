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
    Block, Cell, ContainerModule, Footnote, Item, Num, PartRole, Provision, Quoted, QuotedItem,
    QuotedLevel, Row, RowGroup, Run, Table, Text, UnitModule,
};
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap, VecDeque};

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
    /// The notes on both sides whose note hash differs.
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
    /// workspace, the address of the module and the name of the binding.
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
    /// The credit was added, removed, or its words changed.
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
