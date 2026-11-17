//! The text records of a module: one line of compact JSON for each
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

use crate::address::Address;
use crate::model::{
    Block, Footnote, Item, Layer, Module, Note, Num, PartRole, Provision, QuotedItem, RowGroup,
    Run, Text, UnitModule,
};
use serde::{Serialize, Serializer};
use serde_json::{Map, Value, json};

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
