//! The records of a workspace: the text records of a module, and the
//! coverage ledger of a generated workspace.
//!
//! The text records of a module are one line of compact JSON for each
//! provision, container, note, credit and footnote, for comparison line by
//! line with an independent reading of the source.
//!
//! A text object carries one text: `{"text": plain, "refs": [[start, end,
//! target], ...], "sup": [[start, end], ...], "sub": [[start, end], ...]}`,
//! followed by `"marks": [[offset, shown, footnote], ...]` when the text
//! holds a footnote marker. Offsets count the Unicode scalar values of the
//! plain text, and `end` is exclusive. A paragraph is a text object with the
//! key `p` in place of `text`, a subheading one with the key `subheading`.
//! The records are sorted by the bytes of their address; for one address the
//! provision or container comes first, then the credit, then the footnotes by
//! ordinal.
//!
//! The coverage ledger ([`coverage`]) counts what a generated workspace
//! holds, from the interfaces of its modules, the index of their addresses,
//! its manifest and its lock alone: its units, containers and provisions,
//! its notes and footnotes, the outcomes of its references, the definitions
//! extracted from its provisions, and what the importer counted and did not
//! import.

use crate::address::Address;
use crate::interface::{Interface, Shape};
use crate::model::{
    Basis, Block, DefinitionKind, Footnote, Item, Layer, Module, Note, Num, PartRole, Provision,
    QuotedItem, RowGroup, Run, Text, UnitModule,
};
use crate::reference::{AddressIndex, Outcome};
use crate::workspace::{AddressCounts, Lock, Manifest, NotImported};
use serde::{Serialize, Serializer};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// One text record. It serializes to one line of compact JSON, with its keys
/// in the order of the record's kind: the order in which they are inserted,
/// which `serde_json` keeps because the workspace builds it with the feature
/// `preserve_order`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRecord {
    address: Address,
    value: Value,
}

impl TextRecord {
    /// The address the record is filed under.
    pub fn address(&self) -> &Address {
        &self.address
    }

    /// The record as one line of compact JSON, without a line end.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.value).expect("a JSON value serializes")
    }
}

impl Serialize for TextRecord {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.value.serialize(serializer)
    }
}

/// The text records of `module`, sorted. A curated module has none.
pub fn text_records(module: &Module) -> Vec<TextRecord> {
    let mut records = Vec::new();
    match module {
        Module::Unit(unit) => {
            provision_records(&unit.root, Some(unit), &mut records);
            matter_records(
                &unit.address,
                unit.credit.as_ref(),
                &unit.notes,
                &unit.footnotes,
                &mut records,
            );
        }
        Module::Container(container) => {
            let mut record = operative_record(
                &container.address,
                &container.kind,
                container.status.as_deref(),
                container.num.as_ref(),
                container.heading.as_ref(),
            );
            record.insert("aliases".into(), addresses(&container.aliases));
            record.insert("within".into(), optional_address(container.within.as_ref()));
            record.insert("members".into(), addresses(&container.members));
            records.push(Sorted::new(&container.address, Rank::Main, record));
            matter_records(
                &container.address,
                container.credit.as_ref(),
                &container.notes,
                &container.footnotes,
                &mut records,
            );
        }
        Module::Curated(_) => {}
    }
    records.sort_by(|a, b| {
        a.record
            .address
            .cmp(&b.record.address)
            .then(a.rank.cmp(&b.rank))
    });
    records.into_iter().map(|sorted| sorted.record).collect()
}

/// Where a record stands among the records of one address.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    /// A provision, a container or a note.
    Main,
    Credit,
    Footnote(u32),
}

struct Sorted {
    rank: Rank,
    record: TextRecord,
}

impl Sorted {
    fn new(at: &Address, rank: Rank, record: Map<String, Value>) -> Sorted {
        Sorted {
            rank,
            record: TextRecord {
                address: at.clone(),
                value: Value::Object(record),
            },
        }
    }
}

/// The keys a provision record and a container record begin with.
fn operative_record(
    at: &Address,
    kind: &str,
    status: Option<&str>,
    number: Option<&Num>,
    text: Option<&Text>,
) -> Map<String, Value> {
    let mut record = Map::new();
    record.insert("address".into(), address(at));
    record.insert("layer".into(), Value::from("operative"));
    record.insert("kind".into(), Value::from(kind));
    record.insert("status".into(), Value::from(status));
    record.insert("num".into(), num(number));
    record.insert("heading".into(), heading(text));
    record
}

/// Adds the record of `provision` and of every provision below it. The record
/// of the top provision of `unit` also has the unit's own fields.
fn provision_records(provision: &Provision, unit: Option<&UnitModule>, out: &mut Vec<Sorted>) {
    let mut record = operative_record(
        &provision.address,
        &provision.kind,
        provision.status.as_deref(),
        provision.num.as_ref(),
        provision.heading.as_ref(),
    );
    let items = provision
        .items
        .iter()
        .map(|item| match item {
            Item::Part {
                role,
                blocks: inner,
            } => part(*role, inner),
            Item::Child(child) => json!({ "child": address(&child.address) }),
        })
        .collect();
    record.insert("items".into(), Value::Array(items));
    if let Some(unit) = unit {
        record.insert("aliases".into(), addresses(&unit.aliases));
        record.insert("within".into(), optional_address(unit.within.as_ref()));
        record.insert("source".into(), Value::from(unit.source.as_str()));
        record.insert("positive_law".into(), Value::from(unit.positive_law));
    }
    out.push(Sorted::new(&provision.address, Rank::Main, record));
    for item in &provision.items {
        if let Item::Child(child) = item {
            provision_records(child, None, out);
        }
    }
}

/// Adds the records of a module's credit, notes and footnotes.
fn matter_records(
    module: &Address,
    credit: Option<&Text>,
    notes: &[Note],
    footnotes: &[Footnote],
    out: &mut Vec<Sorted>,
) {
    if let Some(credit) = credit {
        let mut record = Map::new();
        record.insert("address".into(), address(module));
        record.insert("record".into(), Value::from("credit"));
        record.extend(text_object("p", credit));
        out.push(Sorted::new(module, Rank::Credit, record));
    }
    for note in notes {
        let layer = match note.layer {
            Layer::Operative => "operative",
            Layer::Enacted => "enacted",
            Layer::Editorial => "editorial",
        };
        let mut record = Map::new();
        record.insert("address".into(), address(&note.address));
        record.insert("layer".into(), Value::from(layer));
        record.insert("group".into(), Value::from(note.group.as_str()));
        record.insert("topics".into(), Value::from(note.topics.clone()));
        record.insert("heading".into(), heading(note.heading.as_ref()));
        record.insert("blocks".into(), blocks(&note.blocks));
        out.push(Sorted::new(&note.address, Rank::Main, record));
    }
    for footnote in footnotes {
        let mut record = Map::new();
        record.insert("address".into(), address(module));
        record.insert("record".into(), Value::from("footnote"));
        record.insert("ordinal".into(), Value::from(footnote.ordinal));
        record.insert("shown".into(), Value::from(footnote.shown.as_str()));
        record.insert("blocks".into(), blocks(&footnote.blocks));
        out.push(Sorted::new(
            module,
            Rank::Footnote(footnote.ordinal),
            record,
        ));
    }
}

fn address(at: &Address) -> Value {
    Value::from(at.as_str())
}

fn optional_address(at: Option<&Address>) -> Value {
    at.map_or(Value::Null, address)
}

fn addresses(all: &[Address]) -> Value {
    Value::Array(all.iter().map(address).collect())
}

/// The text object of `text`, with the plain text under `key`.
fn text_object(key: &str, text: &Text) -> Map<String, Value> {
    let mut refs = Vec::new();
    let mut sup = Vec::new();
    let mut sub = Vec::new();
    let mut marks = Vec::new();
    let mut offset = 0;
    for run in &text.runs {
        let start = offset;
        offset += run.words().chars().count();
        match run {
            Run::Words(_) => {}
            Run::Ref { target, .. } => refs.push(json!([start, offset, target])),
            Run::Sup(_) => sup.push(json!([start, offset])),
            Run::Sub(_) => sub.push(json!([start, offset])),
            Run::Mark { shown, footnote } => marks.push(json!([start, shown, footnote])),
        }
    }
    let mut object = Map::new();
    object.insert(key.into(), Value::from(text.plain()));
    object.insert("refs".into(), Value::Array(refs));
    object.insert("sup".into(), Value::Array(sup));
    object.insert("sub".into(), Value::Array(sub));
    if !marks.is_empty() {
        object.insert("marks".into(), Value::Array(marks));
    }
    object
}

fn heading(text: Option<&Text>) -> Value {
    text.map_or(Value::Null, |text| Value::Object(text_object("text", text)))
}

/// `null`, or `[value, shown]` with `shown` the plain string when the shown
/// text is words only.
fn num(num: Option<&Num>) -> Value {
    let Some(num) = num else {
        return Value::Null;
    };
    let words_only = num
        .shown
        .runs
        .iter()
        .all(|run| matches!(run, Run::Words(_)));
    let shown = if words_only {
        Value::from(num.shown.plain())
    } else {
        Value::Object(text_object("text", &num.shown))
    };
    json!([num.value, shown])
}

fn role(role: PartRole) -> &'static str {
    match role {
        PartRole::Chapeau => "chapeau",
        PartRole::Content => "content",
        PartRole::Continuation => "continuation",
        PartRole::Proviso => "proviso",
    }
}

fn part(part_role: PartRole, inner: &[Block]) -> Value {
    json!({ "part": role(part_role), "blocks": blocks(inner) })
}

fn blocks(all: &[Block]) -> Value {
    Value::Array(all.iter().map(block).collect())
}

fn block(block: &Block) -> Value {
    match block {
        Block::Para(text) => Value::Object(text_object("p", text)),
        Block::Subheading(text) => Value::Object(text_object("subheading", text)),
        Block::Table(table) => {
            let rows: Vec<Value> = table
                .rows
                .iter()
                .map(|row| {
                    let group = match row.group {
                        RowGroup::Head => "head",
                        RowGroup::Body => "body",
                        RowGroup::Foot => "foot",
                    };
                    let cells: Vec<Value> = row
                        .cells
                        .iter()
                        .map(|cell| {
                            json!([
                                cell.header,
                                cell.colspan,
                                cell.rowspan,
                                blocks(&cell.blocks)
                            ])
                        })
                        .collect();
                    json!([group, cells])
                })
                .collect();
            json!({ "table": rows })
        }
        Block::Quoted(quoted) => json!({
            "quoted": { "origin": quoted.origin, "items": quoted_items(&quoted.items) }
        }),
        Block::Image { source } => json!({ "image": source }),
    }
}

fn quoted_items(items: &[QuotedItem]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|item| match item {
                QuotedItem::Block(inner) => block(inner),
                QuotedItem::Part {
                    role,
                    blocks: inner,
                } => part(*role, inner),
                QuotedItem::Level(level) => json!({
                    "level": level.kind,
                    "num": num(level.num.as_ref()),
                    "heading": heading(level.heading.as_ref()),
                    "items": quoted_items(&level.items),
                }),
            })
            .collect(),
    )
}

/// The `schema` of a coverage ledger.
pub const COVERAGE_SCHEMA: &str = "fidryn.coverage/v0.1";

/// The status under which a unit whose top provision has none is counted.
pub const IN_FORCE: &str = "in-force";

/// The layers of the model, in the order the ledger lists them.
const LAYERS: [Layer; 3] = [Layer::Operative, Layer::Enacted, Layer::Editorial];

/// The coverage ledger of a generated workspace, `fidryn.coverage/v0.1`,
/// which [`coverage`] gives and [`Coverage::to_json`] writes. Every map is
/// sorted by the bytes of its keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    /// The prefix of the workspace, its manifest's.
    pub prefix: Address,
    /// The release of the workspace, its lock's.
    pub release: String,
    pub totals: Totals,
    /// One row for each source file of a unit or a container, sorted by the
    /// bytes of `source`.
    pub by_source: Vec<SourceCounts>,
}

/// What a ledger counts of the whole workspace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    /// The unit modules.
    pub units: u64,
    /// The container modules.
    pub containers: u64,
    /// Every provision of every unit, the unit's own top provision included.
    pub provisions: u64,
    /// The units by the kind of their top provision.
    pub units_by_kind: BTreeMap<String, u64>,
    /// The units by the status of their top provision, [`IN_FORCE`] for
    /// none.
    pub units_by_status: BTreeMap<String, u64>,
    pub containers_by_kind: BTreeMap<String, u64>,
    /// The lock's count of the addresses the importer gave.
    pub addresses: AddressCounts,
    /// The notes of the units and the containers of the enacted layer.
    pub enacted_notes: u64,
    /// The notes of the units and the containers of the editorial layer.
    pub editorial_notes: u64,
    /// The notes by their group.
    pub notes_by_group: BTreeMap<String, u64>,
    pub footnotes: u64,
    /// The provisions and the notes that hold an image.
    pub with_images: u64,
    /// The references of each layer by the name of their outcome; every
    /// layer and every outcome is present.
    pub references: BTreeMap<Layer, BTreeMap<String, u64>>,
    /// The references of outcome `outside`, of every layer, by the body of
    /// law they name.
    pub outside_by_body: BTreeMap<String, u64>,
    /// The definitions whose basis is `enacted`.
    pub enacted_definitions: u64,
    /// The definitions whose basis is `restated`.
    pub restated_definitions: u64,
    /// The definitions whose basis is `unresolved`.
    pub unresolved_definitions: u64,
    /// The definitions by the name of their kind.
    pub definitions_by_kind: BTreeMap<String, u64>,
    /// The lock's count of what the importer did not import.
    pub not_imported: NotImported,
}

/// What a ledger counts of the modules of one source file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCounts {
    /// The source file, as the modules declare it.
    pub source: String,
    /// The file's positive-law flag, as the module of the file whose address
    /// comes first in the byte order of addresses declares it, whatever the
    /// order of the interfaces; `None` when it declares none. An importer
    /// gives every module of a file the file's flag, so they agree; when
    /// they do not, that module's flag is the row's.
    pub positive_law: Option<bool>,
    /// The unit modules of the file.
    pub units: u64,
    /// Every provision of those units.
    pub provisions: u64,
    /// The definitions of those units.
    pub definitions: u64,
}

/// The name of a definition's kind, as a module writes it.
fn kind_name(kind: DefinitionKind) -> &'static str {
    match kind {
        DefinitionKind::Means => "means",
        DefinitionKind::Includes => "includes",
        DefinitionKind::DoesNotInclude => "does_not_include",
        DefinitionKind::Borrowed => "borrowed",
    }
}

/// The name of a layer, as the ledger writes it.
fn layer_name(layer: Layer) -> &'static str {
    match layer {
        Layer::Operative => "operative",
        Layer::Enacted => "enacted",
        Layer::Editorial => "editorial",
    }
}

/// Adds one to the count of `key` in `counts`.
fn add(counts: &mut BTreeMap<String, u64>, key: &str) {
    match counts.get_mut(key) {
        Some(count) => *count += 1,
        None => {
            counts.insert(key.to_owned(), 1);
        }
    }
}

/// `count` as a `u64`.
fn wide(count: usize) -> u64 {
    u64::try_from(count).expect("a count fits in 64 bits")
}

/// The coverage ledger of the generated workspace whose manifest is
/// `manifest` and whose lock is `lock`, from the interfaces of its modules,
/// `interfaces`, and `index`, the index of their addresses below the
/// manifest's prefix, which gives each reference its outcome. It reads no
/// module's source, and nothing from outside the workspace.
///
/// A unit, a container and their notes, footnotes, references and
/// definitions are counted; a curated module is not. `addresses` and
/// `not_imported` are the lock's, copied. The flag of a source file is that
/// of its module whose address comes first ([`SourceCounts::positive_law`]).
/// The ledger is the same for any order of `interfaces` in which no two
/// declare one address or one alias, which no workspace that checks has: a
/// reference to such an address reaches the interface that comes first in
/// `index`, and of two modules of one file at one address the first gives
/// the file's flag.
pub fn coverage(
    interfaces: &[Interface],
    index: &AddressIndex,
    manifest: &Manifest,
    lock: &Lock,
) -> Coverage {
    let mut totals = Totals {
        addresses: lock.addresses,
        not_imported: lock.not_imported,
        references: LAYERS
            .into_iter()
            .map(|layer| {
                let outcomes = Outcome::NAMES
                    .into_iter()
                    .map(|name| (name.to_owned(), 0))
                    .collect();
                (layer, outcomes)
            })
            .collect(),
        ..Totals::default()
    };
    let mut sources: BTreeMap<&str, SourceCounts> = BTreeMap::new();
    // The address of the module whose flag each source's row holds.
    let mut flagged_by: BTreeMap<&str, &Address> = BTreeMap::new();
    for interface in interfaces {
        let (Some(source), Shape::Unit | Shape::Container) =
            (interface.source.as_deref(), interface.shape)
        else {
            continue;
        };
        let row = sources.entry(source).or_insert_with(|| SourceCounts {
            source: source.to_owned(),
            positive_law: interface.positive_law,
            units: 0,
            provisions: 0,
            definitions: 0,
        });
        match flagged_by.get(source) {
            Some(first) if **first <= interface.address => {}
            _ => {
                flagged_by.insert(source, &interface.address);
                row.positive_law = interface.positive_law;
            }
        }
        if interface.shape == Shape::Unit {
            let provisions = wide(interface.provisions.len());
            totals.units += 1;
            totals.provisions += provisions;
            // The first provision of the interface of a unit is its top one.
            if let Some(top) = interface.provisions.first() {
                add(&mut totals.units_by_kind, interface.word(top.kind));
                let status = top.status.map_or(IN_FORCE, |status| interface.word(status));
                add(&mut totals.units_by_status, status);
            }
            totals.with_images += wide(
                interface
                    .provisions
                    .iter()
                    .filter(|provision| provision.image)
                    .count(),
            );
            row.units += 1;
            row.provisions += provisions;
            row.definitions += wide(interface.definitions.len());
        }
        if let Some(container) = &interface.container {
            totals.containers += 1;
            add(&mut totals.containers_by_kind, &container.kind);
        }
        for note in &interface.notes {
            match note.layer {
                Layer::Enacted => totals.enacted_notes += 1,
                Layer::Editorial => totals.editorial_notes += 1,
                // A note of a module that lowers is of one of the other two
                // layers.
                Layer::Operative => continue,
            }
            add(&mut totals.notes_by_group, interface.word(note.group));
            if note.image {
                totals.with_images += 1;
            }
        }
        totals.footnotes += u64::from(interface.footnotes);
        for reference in &interface.references {
            let outcome = index.outcome(&reference.target);
            let counts = totals
                .references
                .get_mut(&reference.layer)
                .expect("every layer is counted");
            *counts
                .get_mut(outcome.name())
                .expect("every outcome is counted") += 1;
            if let Outcome::Outside(body) = &outcome {
                add(&mut totals.outside_by_body, body);
            }
        }
        for definition in &interface.definitions {
            match definition.basis {
                Basis::Enacted => totals.enacted_definitions += 1,
                Basis::Restated => totals.restated_definitions += 1,
                Basis::Unresolved => totals.unresolved_definitions += 1,
            }
            add(&mut totals.definitions_by_kind, kind_name(definition.kind));
        }
    }
    Coverage {
        prefix: manifest.prefix.clone(),
        release: lock.release.clone(),
        totals,
        by_source: sources.into_values().collect(),
    }
}

/// `counts` as a JSON object, its keys in their order.
fn counted(counts: &BTreeMap<String, u64>) -> Value {
    Value::Object(
        counts
            .iter()
            .map(|(key, count)| (key.clone(), Value::from(*count)))
            .collect(),
    )
}

/// An object of `entries`, in their order.
fn object<const N: usize>(entries: [(&str, Value); N]) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

impl Coverage {
    /// The ledger as `coverage.json` holds it and `fidryn coverage` prints
    /// it: JSON with two spaces of indentation and a final line end, the
    /// keys of the ledger in their order, the outcomes in the order of
    /// [`Outcome::NAMES`], and the keys of every other map, the kinds,
    /// statuses, groups and bodies of law, sorted by their bytes.
    pub fn to_json(&self) -> String {
        let totals = &self.totals;
        let references = Value::Object(
            LAYERS
                .into_iter()
                .map(|layer| {
                    let counts = totals.references.get(&layer);
                    let outcomes = Outcome::NAMES
                        .into_iter()
                        .map(|name| {
                            let count = counts.and_then(|counts| counts.get(name)).copied();
                            (name.to_owned(), Value::from(count.unwrap_or(0)))
                        })
                        .collect();
                    (layer_name(layer).to_owned(), Value::Object(outcomes))
                })
                .collect(),
        );
        let by_source = self
            .by_source
            .iter()
            .map(|row| {
                object([
                    ("source", Value::from(row.source.as_str())),
                    ("positive_law", Value::from(row.positive_law)),
                    ("units", Value::from(row.units)),
                    ("provisions", Value::from(row.provisions)),
                    ("definitions", Value::from(row.definitions)),
                ])
            })
            .collect();
        let ledger = object([
            ("schema", Value::from(COVERAGE_SCHEMA)),
            ("prefix", Value::from(self.prefix.as_str())),
            ("release", Value::from(self.release.as_str())),
            (
                "totals",
                object([
                    ("units", Value::from(totals.units)),
                    ("containers", Value::from(totals.containers)),
                    ("provisions", Value::from(totals.provisions)),
                    ("units_by_kind", counted(&totals.units_by_kind)),
                    ("units_by_status", counted(&totals.units_by_status)),
                    ("containers_by_kind", counted(&totals.containers_by_kind)),
                    (
                        "addresses",
                        serde_json::to_value(totals.addresses).expect("three counts"),
                    ),
                    (
                        "notes",
                        object([
                            ("enacted", Value::from(totals.enacted_notes)),
                            ("editorial", Value::from(totals.editorial_notes)),
                        ]),
                    ),
                    ("notes_by_group", counted(&totals.notes_by_group)),
                    ("footnotes", Value::from(totals.footnotes)),
                    ("with_images", Value::from(totals.with_images)),
                    ("references", references),
                    ("outside_by_body", counted(&totals.outside_by_body)),
                    (
                        "definitions",
                        object([
                            ("enacted", Value::from(totals.enacted_definitions)),
                            ("restated", Value::from(totals.restated_definitions)),
                            ("unresolved", Value::from(totals.unresolved_definitions)),
                        ]),
                    ),
                    ("definitions_by_kind", counted(&totals.definitions_by_kind)),
                    (
                        "not_imported",
                        serde_json::to_value(totals.not_imported).expect("six counts"),
                    ),
                ]),
            ),
            ("by_source", Value::Array(by_source)),
        ]);
        let mut text =
            serde_json::to_string_pretty(&ledger).expect("a ledger of strings and numbers");
        text.push('\n');
        text
    }
}

#[cfg(test)]
mod tests {
    use super::{TextRecord, text_records};
    use crate::model::build::*;
    use crate::model::{
        Basis, Block, Cell, ContainerModule, CuratedModule, Definition, DefinitionKind, Footnote,
        Item, Layer, Module, Num, PartRole, Quoted, QuotedItem, QuotedLevel, Row, RowGroup, Table,
        UnitModule,
    };
    use serde_json::Value;

    /// The unit of the example of the edition-2 syntax.
    fn example_unit() -> UnitModule {
        let subsection = provision(
            "/zz/demo/t1/s8/a",
            "subsection",
            num("a", "(a)"),
            None,
            vec![content(vec![para(vec![
                words("As used in "),
                link("section 2 of this title", "/zz/demo/t1/s2"),
                words(", the term \u{201C}widget\u{201D} means a small device."),
            ])])],
        );
        UnitModule {
            address: address("/zz/demo/t1/s8"),
            aliases: Vec::new(),
            within: Some(address("/zz/demo/t1/ch1")),
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            root: provision(
                "/zz/demo/t1/s8",
                "section",
                num("8", "§ 8."),
                Some(plain_text("Words that include a widget")),
                vec![Item::Child(subsection)],
            ),
            credit: Some(plain_text("(Added by an invented act.)")),
            notes: Vec::new(),
            footnotes: Vec::new(),
            definitions: vec![Definition {
                at: address("/zz/demo/t1/s8/a"),
                terms: vec!["widget".to_owned()],
                kind: DefinitionKind::Means,
                words: Some("As used in section 2 of this title".to_owned()),
                region: Some(address("/zz/demo/t1/s2")),
                basis: Basis::Enacted,
                borrowed_from: None,
                see_note: None,
            }],
        }
    }

    /// The record of the subsection of that unit, as the reference shows it.
    const SHOWN: &str = r#"{"address": "/zz/demo/t1/s8/a", "layer": "operative", "kind": "subsection", "status": null,
 "num": ["a", "(a)"], "heading": null,
 "items": [
   {"part": "content", "blocks": [
     {"p": "As used in section 2 of this title, the term “widget” means a small device.",
      "refs": [[11, 34, "/zz/demo/t1/s2"]], "sup": [], "sub": []}
   ]}
 ]}"#;

    fn lines(module: &Module) -> Vec<String> {
        text_records(module)
            .iter()
            .map(TextRecord::to_json)
            .collect()
    }

    fn cell(header: bool, colspan: u32, rowspan: u32, blocks: Vec<Block>) -> Cell {
        Cell {
            header,
            colspan,
            rowspan,
            blocks,
        }
    }

    fn rich_unit() -> UnitModule {
        let a = provision(
            "/zz/demo/t1/s4/a",
            "paragraph",
            num("a", "(a)"),
            None,
            vec![content(vec![para(vec![
                words("the term \u{201C}record\u{201D} in "),
                link("section 2", "/zz/demo/t1/s2"),
                words(" means a note"),
                mark("1", 1),
                words("."),
            ])])],
        );
        let mut b = provision(
            "/zz/demo/t1/s4/b",
            "paragraph",
            num("b", "(b)"),
            Some(plain_text("Weights")),
            vec![content(vec![
                Block::Table(Table {
                    rows: vec![
                        Row {
                            group: RowGroup::Head,
                            cells: vec![
                                cell(true, 1, 1, vec![para(vec![words("Kind")])]),
                                cell(true, 1, 1, vec![para(vec![words("Weight")])]),
                            ],
                        },
                        Row {
                            group: RowGroup::Body,
                            cells: vec![
                                cell(true, 1, 2, vec![para(vec![words("Small")])]),
                                cell(false, 1, 1, vec![para(vec![words("1 gram")])]),
                            ],
                        },
                        Row {
                            group: RowGroup::Body,
                            cells: vec![cell(false, 1, 1, Vec::new())],
                        },
                        Row {
                            group: RowGroup::Foot,
                            cells: vec![cell(
                                false,
                                2,
                                1,
                                vec![para(vec![
                                    words("10"),
                                    sup("3"),
                                    words(" grams of H"),
                                    sub("2"),
                                    words("O"),
                                ])],
                            )],
                        },
                    ],
                }),
                Block::Image {
                    source: "demo-figure.png".to_owned(),
                },
            ])],
        );
        b.status = Some("repealed".to_owned());
        let mut root = provision(
            "/zz/demo/t1/s4",
            "section",
            None,
            Some(plain_text("Records")),
            vec![
                part(
                    PartRole::Chapeau,
                    vec![para(vec![words("An invented maker shall keep\u{2014}")])],
                ),
                Item::Child(a),
                Item::Child(b),
            ],
        );
        root.num = Some(Num {
            value: "4".to_owned(),
            shown: text(vec![words("§ 4."), mark("2", 2)]),
        });
        let mut codification = note(
            "/zz/demo/t1/s4/note/ungrouped/nolaw/codification",
            Layer::Editorial,
            "Codification",
            "An invented act amended this section to read:",
        );
        codification.group = "ungrouped".to_owned();
        codification.topics = vec!["codification".to_owned()];
        codification.blocks.push(Block::Quoted(Quoted {
            origin: Some("/zz/demo/act/1".to_owned()),
            items: vec![
                QuotedItem::Level(QuotedLevel {
                    kind: "section".to_owned(),
                    num: num("4", "§ 4."),
                    heading: None,
                    items: vec![
                        QuotedItem::Part {
                            role: PartRole::Content,
                            blocks: vec![para(vec![words("A record is kept.")])],
                        },
                        QuotedItem::Block(Block::Subheading(plain_text("Inside"))),
                    ],
                }),
                QuotedItem::Block(para(vec![words("After the quotation.")])),
            ],
        }));
        codification.blocks.push(Block::Quoted(Quoted {
            origin: None,
            items: Vec::new(),
        }));
        UnitModule {
            address: address("/zz/demo/t1/s4"),
            aliases: vec![address("/zz/demo/t1/s4a")],
            within: None,
            source: "demo01.xml".to_owned(),
            positive_law: None,
            root,
            credit: Some(text(vec![
                words("(Added by an invented act, "),
                link("§ 4", "/zz/demo/act/1/s4"),
                words(".)"),
            ])),
            notes: vec![
                note(
                    "/zz/demo/t1/s4/note/demoNotes/nolaw/short-title",
                    Layer::Enacted,
                    "Short Title",
                    "This invented section may be cited as the Record Act.",
                ),
                codification,
                note(
                    "/zz/demo/t1/s4/note/demoNotes/nolaw/effective-date",
                    Layer::Enacted,
                    "Effective Date",
                    "This invented section takes effect on enactment.",
                ),
            ],
            footnotes: vec![
                Footnote {
                    ordinal: 2,
                    shown: "2".to_owned(),
                    blocks: vec![para(vec![words("An invented second footnote.")])],
                },
                Footnote {
                    ordinal: 1,
                    shown: "1".to_owned(),
                    blocks: vec![para(vec![words("So in the invented original.")])],
                },
            ],
            definitions: Vec::new(),
        }
    }

    fn container() -> ContainerModule {
        let mut prior = note(
            "/zz/demo/t1/ch1/note/ungrouped/nolaw/prior-provisions",
            Layer::Editorial,
            "Prior Provisions",
            "An invented chapter stood here.",
        );
        prior.group = "ungrouped".to_owned();
        ContainerModule {
            address: address("/zz/demo/t1/ch1"),
            aliases: Vec::new(),
            within: Some(address("/zz/demo/t1")),
            source: "demo01.xml".to_owned(),
            positive_law: Some(true),
            kind: "chapter".to_owned(),
            num: Some(Num {
                value: "1".to_owned(),
                shown: text(vec![words("CHAPTER 1"), mark("*", 1)]),
            }),
            heading: Some(plain_text("Invented Widgets")),
            status: None,
            members: vec![address("/zz/demo/t1/s4"), address("/zz/demo/t1/s8")],
            credit: Some(plain_text("(Invented act.)")),
            notes: vec![prior],
            footnotes: vec![Footnote {
                ordinal: 1,
                shown: "*".to_owned(),
                blocks: vec![para(vec![words("So in the invented original.")])],
            }],
        }
    }

    #[test]
    #[allow(non_snake_case)]
    fn text_records_follow_S15() {
        // The unit of the example: the record of its subsection is the one
        // shown, and every record is in address order.
        let example = lines(&Module::Unit(example_unit()));
        assert_eq!(
            example,
            [
                r#"{"address":"/zz/demo/t1/s8","layer":"operative","kind":"section","status":null,"num":["8","§ 8."],"heading":{"text":"Words that include a widget","refs":[],"sup":[],"sub":[]},"items":[{"child":"/zz/demo/t1/s8/a"}],"aliases":[],"within":"/zz/demo/t1/ch1","source":"demo01.xml","positive_law":true}"#,
                r#"{"address":"/zz/demo/t1/s8","record":"credit","p":"(Added by an invented act.)","refs":[],"sup":[],"sub":[]}"#,
                r#"{"address":"/zz/demo/t1/s8/a","layer":"operative","kind":"subsection","status":null,"num":["a","(a)"],"heading":null,"items":[{"part":"content","blocks":[{"p":"As used in section 2 of this title, the term “widget” means a small device.","refs":[[11,34,"/zz/demo/t1/s2"]],"sup":[],"sub":[]}]}]}"#,
            ]
        );
        let shown: Value = serde_json::from_str(SHOWN).expect("the record shown is JSON");
        assert_eq!(serde_json::to_string(&shown).expect("compact"), example[2]);

        // Every kind of record, the forms of each kind of text, and the order
        // of records that share an address.
        let rich = lines(&Module::Unit(rich_unit()));
        assert_eq!(
            rich,
            [
                r#"{"address":"/zz/demo/t1/s4","layer":"operative","kind":"section","status":null,"num":["4",{"text":"§ 4.","refs":[],"sup":[],"sub":[],"marks":[[4,"2",2]]}],"heading":{"text":"Records","refs":[],"sup":[],"sub":[]},"items":[{"part":"chapeau","blocks":[{"p":"An invented maker shall keep—","refs":[],"sup":[],"sub":[]}]},{"child":"/zz/demo/t1/s4/a"},{"child":"/zz/demo/t1/s4/b"}],"aliases":["/zz/demo/t1/s4a"],"within":null,"source":"demo01.xml","positive_law":null}"#,
                r#"{"address":"/zz/demo/t1/s4","record":"credit","p":"(Added by an invented act, § 4.)","refs":[[27,30,"/zz/demo/act/1/s4"]],"sup":[],"sub":[]}"#,
                r#"{"address":"/zz/demo/t1/s4","record":"footnote","ordinal":1,"shown":"1","blocks":[{"p":"So in the invented original.","refs":[],"sup":[],"sub":[]}]}"#,
                r#"{"address":"/zz/demo/t1/s4","record":"footnote","ordinal":2,"shown":"2","blocks":[{"p":"An invented second footnote.","refs":[],"sup":[],"sub":[]}]}"#,
                r#"{"address":"/zz/demo/t1/s4/a","layer":"operative","kind":"paragraph","status":null,"num":["a","(a)"],"heading":null,"items":[{"part":"content","blocks":[{"p":"the term “record” in section 2 means a note.","refs":[[21,30,"/zz/demo/t1/s2"]],"sup":[],"sub":[],"marks":[[43,"1",1]]}]}]}"#,
                r#"{"address":"/zz/demo/t1/s4/b","layer":"operative","kind":"paragraph","status":"repealed","num":["b","(b)"],"heading":{"text":"Weights","refs":[],"sup":[],"sub":[]},"items":[{"part":"content","blocks":[{"table":[["head",[[true,1,1,[{"p":"Kind","refs":[],"sup":[],"sub":[]}]],[true,1,1,[{"p":"Weight","refs":[],"sup":[],"sub":[]}]]]],["body",[[true,1,2,[{"p":"Small","refs":[],"sup":[],"sub":[]}]],[false,1,1,[{"p":"1 gram","refs":[],"sup":[],"sub":[]}]]]],["body",[[false,1,1,[]]]],["foot",[[false,2,1,[{"p":"103 grams of H2O","refs":[],"sup":[[2,3]],"sub":[[14,15]]}]]]]]},{"image":"demo-figure.png"}]}]}"#,
                r#"{"address":"/zz/demo/t1/s4/note/demoNotes/nolaw/effective-date","layer":"enacted","group":"demoNotes","topics":[],"heading":{"text":"Effective Date","refs":[],"sup":[],"sub":[]},"blocks":[{"p":"This invented section takes effect on enactment.","refs":[],"sup":[],"sub":[]}]}"#,
                r#"{"address":"/zz/demo/t1/s4/note/demoNotes/nolaw/short-title","layer":"enacted","group":"demoNotes","topics":[],"heading":{"text":"Short Title","refs":[],"sup":[],"sub":[]},"blocks":[{"p":"This invented section may be cited as the Record Act.","refs":[],"sup":[],"sub":[]}]}"#,
                r#"{"address":"/zz/demo/t1/s4/note/ungrouped/nolaw/codification","layer":"editorial","group":"ungrouped","topics":["codification"],"heading":{"text":"Codification","refs":[],"sup":[],"sub":[]},"blocks":[{"p":"An invented act amended this section to read:","refs":[],"sup":[],"sub":[]},{"quoted":{"origin":"/zz/demo/act/1","items":[{"level":"section","num":["4","§ 4."],"heading":null,"items":[{"part":"content","blocks":[{"p":"A record is kept.","refs":[],"sup":[],"sub":[]}]},{"subheading":"Inside","refs":[],"sup":[],"sub":[]}]},{"p":"After the quotation.","refs":[],"sup":[],"sub":[]}]}},{"quoted":{"origin":null,"items":[]}}]}"#,
            ]
        );

        // Offsets count Unicode scalar values: the link follows two curly
        // quotation marks of three bytes each, so it starts at the 22nd
        // scalar value and at byte 25.
        let before_link = "the term \u{201C}record\u{201D} in ";
        assert_eq!(before_link.chars().count(), 21);
        assert_eq!(before_link.len(), 25);
        // The marker in the number of the section: after four scalar values
        // and five bytes.
        assert_eq!("§ 4.".chars().count(), 4);
        assert_eq!("§ 4.".len(), 5);

        assert_eq!(
            lines(&Module::Container(container())),
            [
                r#"{"address":"/zz/demo/t1/ch1","layer":"operative","kind":"chapter","status":null,"num":["1",{"text":"CHAPTER 1","refs":[],"sup":[],"sub":[],"marks":[[9,"*",1]]}],"heading":{"text":"Invented Widgets","refs":[],"sup":[],"sub":[]},"aliases":[],"within":"/zz/demo/t1","members":["/zz/demo/t1/s4","/zz/demo/t1/s8"]}"#,
                r#"{"address":"/zz/demo/t1/ch1","record":"credit","p":"(Invented act.)","refs":[],"sup":[],"sub":[]}"#,
                r#"{"address":"/zz/demo/t1/ch1","record":"footnote","ordinal":1,"shown":"*","blocks":[{"p":"So in the invented original.","refs":[],"sup":[],"sub":[]}]}"#,
                r#"{"address":"/zz/demo/t1/ch1/note/ungrouped/nolaw/prior-provisions","layer":"editorial","group":"ungrouped","topics":[],"heading":{"text":"Prior Provisions","refs":[],"sup":[],"sub":[]},"blocks":[{"p":"An invented chapter stood here.","refs":[],"sup":[],"sub":[]}]}"#,
            ]
        );

        let records = text_records(&Module::Unit(rich_unit()));
        let addresses: Vec<&str> = records.iter().map(|r| r.address().as_str()).collect();
        assert_eq!(
            addresses,
            [
                "/zz/demo/t1/s4",
                "/zz/demo/t1/s4",
                "/zz/demo/t1/s4",
                "/zz/demo/t1/s4",
                "/zz/demo/t1/s4/a",
                "/zz/demo/t1/s4/b",
                "/zz/demo/t1/s4/note/demoNotes/nolaw/effective-date",
                "/zz/demo/t1/s4/note/demoNotes/nolaw/short-title",
                "/zz/demo/t1/s4/note/ungrouped/nolaw/codification",
            ]
        );
        assert_eq!(
            serde_json::to_string(&records[2]).expect("serialize"),
            records[2].to_json()
        );

        let curated = Module::Curated(CuratedModule {
            address: address("/zz/demo/curated"),
            imports: Vec::new(),
            citations: Vec::new(),
            bindings: Vec::new(),
        });
        assert!(text_records(&curated).is_empty());
    }

    #[test]
    fn a_text_object_is_written_whole_only_when_it_holds_more_than_words() {
        let mut unit = example_unit();
        unit.root.items.clear();
        unit.credit = None;
        let shown = |runs| {
            let mut unit = unit.clone();
            unit.root.num = Some(Num {
                value: "8".to_owned(),
                shown: text(runs),
            });
            let line = lines(&Module::Unit(unit)).remove(0);
            let start = line.find("\"num\":").expect("a number") + 6;
            let end = line.find(",\"heading\"").expect("a heading");
            line[start..end].to_owned()
        };
        assert_eq!(shown(vec![words("§ 8.")]), r#"["8","§ 8."]"#);
        assert_eq!(shown(Vec::new()), r#"["8",""]"#);
        assert_eq!(
            shown(vec![words("§ 8"), sup("a")]),
            r#"["8",{"text":"§ 8a","refs":[],"sup":[[3,4]],"sub":[]}]"#
        );
        assert_eq!(
            shown(vec![words("§ 8"), sub("a")]),
            r#"["8",{"text":"§ 8a","refs":[],"sup":[],"sub":[[3,4]]}]"#
        );
        assert_eq!(
            shown(vec![link("§ 8", "/zz/demo/t1/s8")]),
            r#"["8",{"text":"§ 8","refs":[[0,3,"/zz/demo/t1/s8"]],"sup":[],"sub":[]}]"#
        );
        unit.root.num = None;
        let line = lines(&Module::Unit(unit)).remove(0);
        assert!(line.contains(r#""num":null,"heading""#), "{line}");
    }
}

#[cfg(test)]
mod ledger_tests {
    //! The coverage ledger. Every model here is invented, in the invented
    //! corpus `/zz/demo`.

    use super::{Coverage, coverage};
    use crate::interface::{DeclSpans, Interface, interface_of};
    use crate::model::build::*;
    use crate::model::{
        Basis, Block, Citation, ContainerModule, CuratedModule, Definition, DefinitionKind,
        Footnote, Item, Layer, Module, Note, UnitModule,
    };
    use crate::reference::AddressIndex;
    use crate::workspace::{AddressCounts, Generator, Lock, Manifest, NotImported, WorkspaceKind};
    use serde_json::{Map, Value};
    use std::collections::BTreeMap;

    fn linked(targets: &[&str]) -> Block {
        let mut runs = vec![words("An invented text that names")];
        for target in targets {
            runs.push(words(" "));
            runs.push(link("this", target));
        }
        runs.push(words("."));
        para(runs)
    }

    fn container(
        at: &str,
        kind: &str,
        source: &str,
        positive_law: Option<bool>,
    ) -> ContainerModule {
        ContainerModule {
            address: address(at),
            aliases: Vec::new(),
            within: None,
            source: source.to_owned(),
            positive_law,
            kind: kind.to_owned(),
            num: num("1", "1"),
            heading: Some(plain_text("Invented Things")),
            status: None,
            members: Vec::new(),
            credit: None,
            notes: Vec::new(),
            footnotes: Vec::new(),
        }
    }

    fn unit(at: &str, kind: &str, source: &str, positive_law: Option<bool>) -> UnitModule {
        UnitModule {
            address: address(at),
            aliases: Vec::new(),
            within: None,
            source: source.to_owned(),
            positive_law,
            root: provision(
                at,
                kind,
                num("1", "§ 1."),
                None,
                vec![content(vec![para(vec![words("An invented section.")])])],
            ),
            credit: None,
            notes: Vec::new(),
            footnotes: Vec::new(),
            definitions: Vec::new(),
        }
    }

    fn child(at: &str, blocks: Vec<Block>) -> Item {
        Item::Child(provision(
            at,
            "subsection",
            num("a", "(a)"),
            None,
            vec![content(blocks)],
        ))
    }

    fn defined(at: &str, kind: DefinitionKind, basis: Basis) -> Definition {
        Definition {
            at: address(at),
            terms: vec!["widget".to_owned()],
            kind,
            words: None,
            region: None,
            basis,
            borrowed_from: None,
            see_note: None,
        }
    }

    fn grouped(at: &str, layer: Layer, group: &str, block: Block) -> Note {
        let mut made = note(at, layer, "An Invented Note", "An invented note.");
        made.group = group.to_owned();
        made.blocks.push(block);
        made
    }

    fn footnote(ordinal: u32, blocks: Vec<Block>) -> Footnote {
        Footnote {
            ordinal,
            shown: ordinal.to_string(),
            blocks,
        }
    }

    /// The modules of an invented workspace whose counts are known: a
    /// title and its chapter, three sections, the last repealed, an
    /// appendix of another source with no positive-law flag and a rule in
    /// it, and a curated module, which the ledger does not count.
    fn modules() -> Vec<Module> {
        let mut title = container("/zz/demo/t1", "title", "demo01.xml", Some(true));
        title.credit = Some(text(vec![
            words("Invented "),
            link("credit", "/zz/demo/t1/s1"),
        ]));
        title.notes.push(grouped(
            "/zz/demo/t1/note/editorialNotes/nolaw/codification",
            Layer::Editorial,
            "editorialNotes",
            linked(&["/zz/other/act/1"]),
        ));
        title
            .footnotes
            .push(footnote(1, vec![linked(&["/zz/demo/t1/s9"])]));

        let mut chapter = container("/zz/demo/t1/ch1", "chapter", "demo01.xml", Some(true));
        chapter.heading = Some(text(vec![link("Widgets", "/zz/demo/t1/s2")]));
        chapter.notes.push(grouped(
            "/zz/demo/t1/ch1/note/statutoryNotes/nolaw/figure",
            Layer::Enacted,
            "statutoryNotes",
            Block::Image {
                source: "demo-figure.png".to_owned(),
            },
        ));

        let mut s1 = unit("/zz/demo/t1/s1", "section", "demo01.xml", Some(true));
        s1.root.items = vec![
            content(vec![linked(&[
                "/zz/demo/t1/s2",
                "/zz/demo/t1/s1/z",
                "/zz/demo/t1/s9",
                "/yy/stat/116/926",
                "not an address",
            ])]),
            child("/zz/demo/t1/s1/a", vec![para(vec![words("A widget.")])]),
            child(
                "/zz/demo/t1/s1/b",
                vec![Block::Image {
                    source: "demo-widget.png".to_owned(),
                }],
            ),
        ];
        s1.notes.push(grouped(
            "/zz/demo/t1/s1/note/statutoryNotes/nolaw/amendments",
            Layer::Enacted,
            "statutoryNotes",
            linked(&["/zz/demo/t1/s3"]),
        ));
        s1.footnotes = vec![
            footnote(1, vec![para(vec![words("An invented footnote.")])]),
            footnote(2, vec![para(vec![words("Another invented footnote.")])]),
        ];
        s1.definitions.push(defined(
            "/zz/demo/t1/s1/a",
            DefinitionKind::Means,
            Basis::Enacted,
        ));

        // One provision that holds two definitions.
        let mut s2 = unit("/zz/demo/t1/s2", "section", "demo01.xml", Some(true));
        s2.definitions = vec![
            defined(
                "/zz/demo/t1/s2",
                DefinitionKind::Includes,
                Basis::Unresolved,
            ),
            defined("/zz/demo/t1/s2", DefinitionKind::Borrowed, Basis::Enacted),
        ];

        let mut s3 = unit("/zz/demo/t1/s3", "section", "demo01.xml", Some(true));
        s3.root.status = Some("repealed".to_owned());

        let appendix = container("/zz/demo/t1a", "appendix", "demo01a.xml", None);
        let mut rule = unit("/zz/demo/t1a/rule1", "rule", "demo01a.xml", None);
        rule.root.items.push(child(
            "/zz/demo/t1a/rule1/a",
            vec![linked(&["/zz/demo/t1/s1\u{2013}3", "/zz/demo/t1/s3"])],
        ));
        rule.definitions.push(defined(
            "/zz/demo/t1a/rule1/a",
            DefinitionKind::DoesNotInclude,
            Basis::Restated,
        ));

        let curated = CuratedModule {
            address: address("/zz/demo/c1"),
            imports: Vec::new(),
            citations: vec![Citation {
                name: "widgets".to_owned(),
                target: address("/zz/demo/t1/s9"),
            }],
            bindings: Vec::new(),
        };
        vec![
            Module::Container(title),
            Module::Container(chapter),
            Module::Unit(s1),
            Module::Unit(s2),
            Module::Unit(s3),
            Module::Container(appendix),
            Module::Unit(rule),
            Module::Curated(curated),
        ]
    }

    fn manifest() -> Manifest {
        Manifest {
            name: "demo".to_owned(),
            kind: WorkspaceKind::Generated,
            prefix: address("/zz/demo"),
            release: Some("42-7".to_owned()),
            toolchain: "0.0.0".to_owned(),
            dependencies: Vec::new(),
        }
    }

    fn lock() -> Lock {
        Lock {
            prefix: address("/zz/demo"),
            release: "42-7".to_owned(),
            generator: Generator {
                name: "demo-writer".to_owned(),
                version: "0.0.0".to_owned(),
            },
            artifacts: Vec::new(),
            addresses: AddressCounts {
                aliases: 2,
                disambiguated: 1,
                assigned: 3,
            },
            not_imported: NotImported {
                tables_of_contents: 1,
                footnotes_in_tables_of_contents: 2,
                links_in_tables_of_contents: 3,
                release_notes: 4,
                links_without_words: 5,
                note_group_headings: 6,
            },
            modules: BTreeMap::new(),
        }
    }

    fn interfaces(modules: &[Module]) -> Vec<Interface> {
        modules
            .iter()
            .map(|module| interface_of(module, &DeclSpans::default()))
            .collect()
    }

    fn ledger_of(interfaces: &[Interface]) -> Coverage {
        let manifest = manifest();
        let index = AddressIndex::build(interfaces, std::slice::from_ref(&manifest.prefix));
        coverage(interfaces, &index, &manifest, &lock())
    }

    /// The ledger of the workspace of [`modules`], as `coverage.json`
    /// holds it.
    const LEDGER: &str = r#"{
  "schema": "fidryn.coverage/v0.1",
  "prefix": "/zz/demo",
  "release": "42-7",
  "totals": {
    "units": 4,
    "containers": 3,
    "provisions": 7,
    "units_by_kind": {
      "rule": 1,
      "section": 3
    },
    "units_by_status": {
      "in-force": 3,
      "repealed": 1
    },
    "containers_by_kind": {
      "appendix": 1,
      "chapter": 1,
      "title": 1
    },
    "addresses": {
      "aliases": 2,
      "disambiguated": 1,
      "assigned": 3
    },
    "notes": {
      "enacted": 2,
      "editorial": 1
    },
    "notes_by_group": {
      "editorialNotes": 1,
      "statutoryNotes": 2
    },
    "footnotes": 3,
    "with_images": 2,
    "references": {
      "operative": {
        "exact": 2,
        "target-status": 1,
        "ambiguous": 0,
        "unit-only": 1,
        "range-form": 1,
        "no-unit": 1,
        "outside": 1,
        "malformed": 1
      },
      "enacted": {
        "exact": 0,
        "target-status": 1,
        "ambiguous": 0,
        "unit-only": 0,
        "range-form": 0,
        "no-unit": 0,
        "outside": 0,
        "malformed": 0
      },
      "editorial": {
        "exact": 1,
        "target-status": 0,
        "ambiguous": 0,
        "unit-only": 0,
        "range-form": 0,
        "no-unit": 1,
        "outside": 1,
        "malformed": 0
      }
    },
    "outside_by_body": {
      "/yy/stat": 1,
      "/zz/other": 1
    },
    "definitions": {
      "enacted": 2,
      "restated": 1,
      "unresolved": 1
    },
    "definitions_by_kind": {
      "borrowed": 1,
      "does_not_include": 1,
      "includes": 1,
      "means": 1
    },
    "not_imported": {
      "tables_of_contents": 1,
      "footnotes_in_tables_of_contents": 2,
      "links_in_tables_of_contents": 3,
      "release_notes": 4,
      "links_without_words": 5,
      "note_group_headings": 6
    }
  },
  "by_source": [
    {
      "source": "demo01.xml",
      "positive_law": true,
      "units": 3,
      "provisions": 5,
      "definitions": 3
    },
    {
      "source": "demo01a.xml",
      "positive_law": null,
      "units": 1,
      "provisions": 2,
      "definitions": 1
    }
  ]
}
"#;

    #[test]
    fn the_ledger_counts_a_demo_workspace() {
        let ledger = ledger_of(&interfaces(&modules()));
        // Units of two kinds and two statuses, provisions, notes by layer
        // and group, footnotes, images, references by layer and outcome,
        // definitions by basis and kind, each source with its flag, and the
        // lock's counts, copied.
        assert_eq!(ledger.totals.units, 4);
        assert_eq!(ledger.totals.provisions, 7);
        assert_eq!(ledger.totals.with_images, 2);
        assert_eq!(
            (
                ledger.totals.enacted_definitions,
                ledger.totals.restated_definitions,
                ledger.totals.unresolved_definitions
            ),
            (2, 1, 1)
        );
        assert_eq!(ledger.totals.addresses, lock().addresses);
        assert_eq!(ledger.totals.not_imported, lock().not_imported);
        assert_eq!(ledger.to_json(), LEDGER);

        // A workspace with nothing counted writes every closed key and
        // every open map empty.
        let empty = ledger_of(&[]);
        let value: Value = serde_json::from_str(&empty.to_json()).expect("JSON");
        assert_eq!(value["totals"]["units_by_kind"], Value::Object(Map::new()));
        assert_eq!(value["totals"]["references"]["enacted"]["malformed"], 0);
        assert_eq!(value["by_source"], Value::Array(Vec::new()));
        assert!(
            empty
                .to_json()
                .contains("\"units_by_kind\": {},\n    \"units_by_status\": {},")
        );
    }

    /// `value` against `schema`, a JSON Schema of the subset the schemas of
    /// this repository use: `$ref` into `$defs`, `type`, `const`, `enum`,
    /// `required`, `properties`, `additionalProperties`, `items` and
    /// `minimum`; `propertyNames` and `pattern` are left to the conformance
    /// check, which runs a full validator. An object's keys are also held to
    /// the order of `properties`.
    fn conforms(schema: &Value, value: &Value, root: &Value, at: &str) -> Result<(), String> {
        if let Some(target) = schema.get("$ref").and_then(Value::as_str) {
            let name = target
                .strip_prefix("#/$defs/")
                .unwrap_or_else(|| panic!("a reference into $defs: {target}"));
            return conforms(&root["$defs"][name], value, root, at);
        }
        if let Some(expected) = schema.get("const")
            && expected != value
        {
            return Err(format!("{at} is {value}, not {expected}"));
        }
        if let Some(Value::Array(allowed)) = schema.get("enum")
            && !allowed.contains(value)
        {
            return Err(format!("{at} is {value}, none of {allowed:?}"));
        }
        if let Some(types) = schema.get("type") {
            let types: Vec<&str> = match types {
                Value::String(one) => vec![one.as_str()],
                Value::Array(many) => many.iter().filter_map(Value::as_str).collect(),
                other => panic!("a type: {other}"),
            };
            let kind = match value {
                Value::Null => "null",
                Value::Bool(_) => "boolean",
                Value::Number(number) if number.is_u64() || number.is_i64() => "integer",
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Array(_) => "array",
                Value::Object(_) => "object",
            };
            if !types.contains(&kind) {
                return Err(format!("{at} is {kind}, not {types:?}"));
            }
        }
        if let (Some(minimum), Some(number)) = (
            schema.get("minimum").and_then(Value::as_i64),
            value.as_i64(),
        ) && number < minimum
        {
            return Err(format!("{at} is {number}, below {minimum}"));
        }
        if let Value::Object(object) = value {
            let properties = schema.get("properties").and_then(Value::as_object);
            if let Some(Value::Array(required)) = schema.get("required") {
                for key in required.iter().filter_map(Value::as_str) {
                    if !object.contains_key(key) {
                        return Err(format!("{at} lacks `{key}`"));
                    }
                }
            }
            if let Some(properties) = properties {
                let order: Vec<&String> = properties
                    .keys()
                    .filter(|key| object.contains_key(*key))
                    .collect();
                let keys: Vec<&String> = object
                    .keys()
                    .filter(|key| properties.contains_key(*key))
                    .collect();
                if order != keys {
                    return Err(format!("the keys of {at} are {keys:?}, not {order:?}"));
                }
            }
            for (key, item) in object {
                let here = format!("{at}/{key}");
                match properties.and_then(|properties| properties.get(key)) {
                    Some(property) => conforms(property, item, root, &here)?,
                    None => match schema.get("additionalProperties") {
                        Some(Value::Bool(false)) => return Err(format!("{here} is not allowed")),
                        Some(additional @ Value::Object(_)) => {
                            conforms(additional, item, root, &here)?;
                        }
                        _ => {}
                    },
                }
            }
        }
        if let (Value::Array(items), Some(each)) = (value, schema.get("items")) {
            for (place, item) in items.iter().enumerate() {
                conforms(each, item, root, &format!("{at}/{place}"))?;
            }
        }
        Ok(())
    }

    fn schema() -> Value {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../schemas/coverage-v0.1.json");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        serde_json::from_str(&text).expect("the schema is JSON")
    }

    #[test]
    fn the_ledger_is_deterministic_and_matches_its_schema() {
        let modules = modules();
        let written = ledger_of(&interfaces(&modules)).to_json();
        // Any order of the modules, and the interfaces read back from their
        // JSON, give the same bytes.
        let mut reversed = interfaces(&modules);
        reversed.reverse();
        assert_eq!(ledger_of(&reversed).to_json(), written);
        let mut rotated = interfaces(&modules);
        rotated.rotate_left(3);
        assert_eq!(ledger_of(&rotated).to_json(), written);
        let read: Vec<Interface> = interfaces(&modules)
            .iter()
            .map(|interface| Interface::from_json(&interface.to_json()).expect("read back"))
            .collect();
        assert_eq!(ledger_of(&read).to_json(), written);

        let schema = schema();
        for text in [written, ledger_of(&[]).to_json()] {
            let value: Value = serde_json::from_str(&text).expect("JSON");
            conforms(&schema, &value, &schema, "").unwrap_or_else(|error| panic!("{error}"));
        }
        // What the schema refuses, the check above refuses too.
        let mut value: Value = serde_json::from_str(LEDGER).expect("JSON");
        value["totals"]["unknown"] = Value::from(1);
        assert!(conforms(&schema, &value, &schema, "").is_err());
        let mut value: Value = serde_json::from_str(LEDGER).expect("JSON");
        value["by_source"][0]["units"] = Value::from(-1);
        assert!(conforms(&schema, &value, &schema, "").is_err());

        // When the modules of one file disagree on its flag, the row takes
        // the flag of the module whose address comes first, in any order of
        // the interfaces: the appendix `/zz/demo/t1a` before its rule.
        let flag_of = |modules: &[Module]| {
            let mut reversed = interfaces(modules);
            reversed.reverse();
            let flags = [interfaces(modules), reversed]
                .map(|order| ledger_of(&order).by_source[1].positive_law);
            assert_eq!(flags[0], flags[1], "the flag depends on the order");
            flags[0]
        };
        let mut flagged = modules;
        if let Module::Unit(rule) = &mut flagged[6] {
            rule.positive_law = Some(false);
        }
        assert_eq!(flag_of(&flagged), None);
        if let Module::Container(appendix) = &mut flagged[5] {
            appendix.positive_law = Some(true);
        }
        assert_eq!(flag_of(&flagged), Some(true));
    }
}
