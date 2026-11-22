//! `fidryn text DIR [ADDRESS] [--json]` prints the text of the provisions and
//! notes of a workspace. The workspaces here are invented, in the corpus
//! `/zz/demo`: the demo workspace under `conformance/fixtures/workspace/demo`,
//! and one that the tests write with the writer of a generated workspace,
//! whose unit has children, a footnote, a source credit and notes anchored at
//! two of its provisions.

use fidryn_driver::workspace::GeneratedWriter;
use fidryn_provision::records::text_records;
use fidryn_provision::workspace::{AddressCounts, Generator, Manifest, NotImported, WorkspaceKind};
use fidryn_provision::{
    Address, Block, Cell, ContainerModule, Footnote, Item, Layer, Module, Note, Num, PartRole,
    Provision, Quoted, QuotedItem, QuotedLevel, Row, RowGroup, Run, Table, Text, UnitModule,
};
use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn demo_workspace() -> PathBuf {
    repository().join("conformance/fixtures/workspace/demo")
}

/// What `fidryn text --json` prints of the demo workspace, which the schema
/// of a text record is checked on.
fn demo_records() -> PathBuf {
    repository().join("conformance/fixtures/text/demo.records.jsonl")
}

/// `fidryn text DIR ARGS...`.
fn text(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fidryn"))
        .args([OsStr::new("text"), dir.as_os_str()])
        .args(args)
        .output()
        .expect("run fidryn")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("the output is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The stdout of `output`, which must be that of a command that succeeded
/// and printed nothing to stderr.
fn printed(output: &Output) -> String {
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        stderr(output)
    );
    assert_eq!(stderr(output), "");
    stdout(output)
}

static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A directory of its own under the temporary directory, removed when
/// dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fidryn-cli-text-{}-{n}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch directory");
        Scratch(dir)
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn address(text: &str) -> Address {
    Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn words(text: &str) -> Run {
    Run::Words(text.to_owned())
}

fn plain(text: &str) -> Text {
    Text {
        runs: vec![words(text)],
    }
}

fn para(text: &str) -> Block {
    Block::Para(plain(text))
}

fn num(value: &str, shown: &str) -> Option<Num> {
    Some(Num {
        value: value.to_owned(),
        shown: plain(shown),
    })
}

fn provision(at: &str, kind: &str, num: Option<Num>, items: Vec<Item>) -> Provision {
    Provision {
        address: address(at),
        kind: kind.to_owned(),
        num,
        heading: None,
        status: None,
        items,
    }
}

fn part(role: PartRole, text: &str) -> Item {
    Item::Part {
        role,
        blocks: vec![para(text)],
    }
}

fn note(at: &str, layer: Layer, group: &str, heading: &str, blocks: Vec<Block>) -> Note {
    Note {
        address: address(at),
        layer,
        group: group.to_owned(),
        topics: Vec::new(),
        heading: Some(plain(heading)),
        blocks,
    }
}

fn cell(header: bool, text: &str) -> Cell {
    Cell {
        header,
        colspan: 1,
        rowspan: 1,
        blocks: vec![para(text)],
    }
}

/// The invented title, which holds a note.
fn title() -> ContainerModule {
    ContainerModule {
        address: address("/zz/demo/t1"),
        aliases: Vec::new(),
        within: None,
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        kind: "title".to_owned(),
        num: num("1", "Title 1—"),
        heading: Some(plain("Invented Things")),
        status: None,
        members: vec![address("/zz/demo/t1/s4"), address("/zz/demo/t1/s8")],
        credit: None,
        notes: vec![note(
            "/zz/demo/t1/note/editorialNotes/nolaw/prior-provisions",
            Layer::Editorial,
            "editorialNotes",
            "Prior Provisions",
            vec![para("An invented title stood here.")],
        )],
        footnotes: Vec::new(),
    }
}

/// An invented section with children, a footnote marked in its number, a
/// source credit, and a note anchored at it and another at its first child.
fn section_4() -> UnitModule {
    let mut root = provision(
        "/zz/demo/t1/s4",
        "section",
        Some(Num {
            value: "4".to_owned(),
            shown: Text {
                runs: vec![
                    words("§ 4."),
                    Run::Mark {
                        shown: "1".to_owned(),
                        footnote: 1,
                    },
                ],
            },
        }),
        vec![
            part(PartRole::Chapeau, "An invented maker shall keep—"),
            Item::Child(provision(
                "/zz/demo/t1/s4/a",
                "paragraph",
                num("a", "(a)"),
                vec![part(PartRole::Content, "a record of each widget; and")],
            )),
        ],
    );
    root.heading = Some(plain("Records"));
    let mut b = provision(
        "/zz/demo/t1/s4/b",
        "paragraph",
        num("b", "(b)"),
        vec![
            part(PartRole::Chapeau, "a list of—"),
            Item::Child(provision(
                "/zz/demo/t1/s4/b/1",
                "subparagraph",
                num("1", "(1)"),
                vec![part(PartRole::Content, "the colors of each widget.")],
            )),
        ],
    );
    b.status = Some("repealed".to_owned());
    root.items.push(Item::Child(b));
    let quoted = Block::Quoted(Quoted {
        origin: Some("/zz/demo/act/1".to_owned()),
        items: vec![QuotedItem::Level(QuotedLevel {
            kind: "paragraph".to_owned(),
            num: num("a", "“(a)"),
            heading: None,
            items: vec![QuotedItem::Part {
                role: PartRole::Content,
                blocks: vec![para("a record of each widget.”")],
            }],
        })],
    });
    UnitModule {
        address: address("/zz/demo/t1/s4"),
        aliases: Vec::new(),
        within: Some(address("/zz/demo/t1")),
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        root,
        credit: Some(plain("(Added by an invented act.)")),
        notes: vec![
            note(
                "/zz/demo/t1/s4/a/note/editorialNotes/nolaw/codification",
                Layer::Editorial,
                "editorialNotes",
                "Codification",
                vec![para("An invented act read:"), quoted],
            ),
            note(
                "/zz/demo/t1/s4/note/demoNotes/nolaw/short-title",
                Layer::Enacted,
                "demoNotes",
                "Short Title",
                vec![para(
                    "This invented section may be cited as the Record Act.",
                )],
            ),
        ],
        footnotes: vec![Footnote {
            ordinal: 1,
            shown: "1".to_owned(),
            blocks: vec![para("So in the invented original.")],
        }],
        definitions: Vec::new(),
    }
}

/// An invented section with a link and a table.
fn section_8() -> UnitModule {
    let mut root = provision(
        "/zz/demo/t1/s8",
        "section",
        num("8", "§ 8."),
        vec![Item::Part {
            role: PartRole::Content,
            blocks: vec![
                Block::Para(Text {
                    runs: vec![
                        words("A widget is described in "),
                        Run::Ref {
                            words: "section 4".to_owned(),
                            target: "/zz/demo/t1/s4".to_owned(),
                        },
                        words("."),
                    ],
                }),
                Block::Table(Table {
                    rows: vec![
                        Row {
                            group: RowGroup::Head,
                            cells: vec![cell(true, "Kind"), cell(true, "Weight")],
                        },
                        Row {
                            group: RowGroup::Body,
                            cells: vec![cell(false, "Small"), cell(false, "1 gram")],
                        },
                    ],
                }),
            ],
        }],
    );
    root.heading = Some(plain("Widgets"));
    UnitModule {
        address: address("/zz/demo/t1/s8"),
        aliases: Vec::new(),
        within: Some(address("/zz/demo/t1")),
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        root,
        credit: None,
        notes: Vec::new(),
        footnotes: Vec::new(),
        definitions: Vec::new(),
    }
}

fn models() -> Vec<Module> {
    vec![
        Module::Container(title()),
        Module::Unit(section_4()),
        Module::Unit(section_8()),
    ]
}

/// Writes the models into `out` as a generated workspace.
fn write_workspace(out: &Path) {
    let manifest = Manifest {
        name: "demo".to_owned(),
        kind: WorkspaceKind::Generated,
        prefix: address("/zz/demo"),
        release: Some("42-7".to_owned()),
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        dependencies: Vec::new(),
    };
    let writer = GeneratedWriter::create(out, manifest).expect("create the workspace");
    for model in models() {
        writer.write(&model).expect("write a module");
    }
    writer
        .finish(
            Generator {
                name: "demo-writer".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            Vec::new(),
            AddressCounts::default(),
            NotImported::default(),
        )
        .expect("finish the workspace");
}

/// The address of the record `line` and what it is a record of.
fn shape(line: &str) -> (String, &'static str) {
    let record: Value =
        serde_json::from_str(line).unwrap_or_else(|error| panic!("{line}: {error}"));
    let address = record["address"].as_str().expect("an address").to_owned();
    let what = match record.get("record").and_then(Value::as_str) {
        Some("credit") => "credit",
        Some("footnote") => "footnote",
        Some(other) => panic!("a record of {other}"),
        None if record.get("group").is_some() => "note",
        None if record.get("members").is_some() => "container",
        None => "provision",
    };
    (address, what)
}

/// The lines of `text`, each without its line end; every line ends in one.
fn lines(text: &str) -> Vec<&str> {
    assert!(text.is_empty() || text.ends_with('\n'), "{text:?}");
    text.lines().collect()
}

/// The records of the whole written workspace, in the order they are
/// printed: by the bytes of their address across all modules, the
/// container's note before the units it lists.
const WHOLE: [(&str, &str); 11] = [
    ("/zz/demo/t1", "container"),
    (
        "/zz/demo/t1/note/editorialNotes/nolaw/prior-provisions",
        "note",
    ),
    ("/zz/demo/t1/s4", "provision"),
    ("/zz/demo/t1/s4", "credit"),
    ("/zz/demo/t1/s4", "footnote"),
    ("/zz/demo/t1/s4/a", "provision"),
    (
        "/zz/demo/t1/s4/a/note/editorialNotes/nolaw/codification",
        "note",
    ),
    ("/zz/demo/t1/s4/b", "provision"),
    ("/zz/demo/t1/s4/b/1", "provision"),
    ("/zz/demo/t1/s4/note/demoNotes/nolaw/short-title", "note"),
    ("/zz/demo/t1/s8", "provision"),
];

#[test]
fn text_prints_one_record_for_each_provision_and_note_in_address_order() {
    // The demo workspace: one record for each container and each section,
    // in the byte order of their addresses, as the committed file holds them.
    let json = printed(&text(&demo_workspace(), &["--json"]));
    let committed = fs::read_to_string(demo_records()).expect("read the committed records");
    assert_eq!(
        json, committed,
        "conformance/fixtures/text/demo.records.jsonl is not what `fidryn text --json` prints \
         of the demo workspace"
    );
    let shapes: Vec<(String, &str)> = lines(&json).into_iter().map(shape).collect();
    let expected = [
        ("/zz/demo/t1", "container"),
        ("/zz/demo/t1/ch1", "container"),
        ("/zz/demo/t1/s1", "provision"),
        ("/zz/demo/t1/s2", "provision"),
        ("/zz/demo/t1/s3", "provision"),
    ];
    assert_eq!(
        shapes,
        expected.map(|(at, what)| (at.to_owned(), what)).to_vec()
    );

    // A workspace whose records of one unit interleave with a container's
    // note: each provision, note, credit and footnote once, in the byte
    // order of the addresses across the whole workspace.
    let scratch = Scratch::new("whole");
    let out = scratch.join("workspace");
    write_workspace(&out);
    let json = printed(&text(&out, &["--json"]));
    let printed_lines = lines(&json);
    let shapes: Vec<(String, &str)> = printed_lines.iter().map(|line| shape(line)).collect();
    assert_eq!(
        shapes,
        WHOLE.map(|(at, what)| (at.to_owned(), what)).to_vec()
    );
    // Exactly the records the model gives, each one compact line.
    let mut given: Vec<String> = models()
        .iter()
        .flat_map(text_records)
        .map(|record| record.to_json())
        .collect();
    let mut sorted_lines: Vec<String> = printed_lines
        .iter()
        .map(|line| (*line).to_owned())
        .collect();
    given.sort();
    sorted_lines.sort();
    assert_eq!(sorted_lines, given);
    for line in printed_lines {
        let value: Value = serde_json::from_str(line).expect("a record is JSON");
        assert_eq!(serde_json::to_string(&value).expect("compact"), line);
    }
}

#[test]
fn text_of_one_address_prints_it_and_what_lies_below_it() {
    let scratch = Scratch::new("below");
    let out = scratch.join("workspace");
    write_workspace(&out);
    let whole = printed(&text(&out, &["--json"]));
    let whole = lines(&whole);
    // Each address prints the records whose address is it or lies below it,
    // at a segment boundary, as the whole output holds them.
    let selected = |at: &str| -> Vec<String> {
        lines(&printed(&text(&out, &[at, "--json"])))
            .into_iter()
            .map(str::to_owned)
            .collect()
    };
    let below = |at: &str| -> Vec<String> {
        whole
            .iter()
            .zip(WHOLE)
            .filter(|(_, (address, _))| {
                *address == at
                    || address
                        .strip_prefix(at)
                        .is_some_and(|rest| rest.starts_with('/'))
            })
            .map(|(line, _)| (*line).to_owned())
            .collect()
    };
    for (at, count) in [
        ("/zz/demo/t1", 11),
        ("/zz/demo/t1/s4", 8),
        ("/zz/demo/t1/s4/a", 2),
        ("/zz/demo/t1/s4/b", 2),
        ("/zz/demo/t1/s4/b/1", 1),
        ("/zz/demo/t1/s4/a/note/editorialNotes/nolaw/codification", 1),
        ("/zz/demo/t1/s8", 1),
    ] {
        let printed = selected(at);
        assert_eq!(printed.len(), count, "{at}: {printed:#?}");
        assert_eq!(printed, below(at), "{at}");
    }

    // Without `--json`, the provision and what lies below it, from depth 0.
    assert_eq!(
        printed(&text(&out, &["/zz/demo/t1/s4/b"])),
        "/zz/demo/t1/s4/b (repealed)\n  (b)\n  a list of—\n  /zz/demo/t1/s4/b/1\n    (1)\n    \
         the colors of each widget.\n"
    );
    assert_eq!(
        printed(&text(&out, &["/zz/demo/t1/s4/a"])),
        "/zz/demo/t1/s4/a\n  (a)\n  a record of each widget; and\n\
         /zz/demo/t1/s4/a/note/editorialNotes/nolaw/codification (editorial)\n  Codification\n  \
         An invented act read:\n    “(a)\n      a record of each widget.”\n"
    );

    // An address that is not in the workspace, a beginning of an address
    // that is the address of nothing, and a text that is no address are each
    // refused with exit 1, and nothing is printed to stdout.
    for (at, said) in [
        (
            "/zz/demo/t1/s5",
            "the workspace holds no provision, container or note at `/zz/demo/t1/s5`",
        ),
        (
            "/zz/demo/t1/s4/a/note",
            "the workspace holds no provision, container or note at `/zz/demo/t1/s4/a/note`",
        ),
        (
            "/zz/demo",
            "the workspace holds no provision, container or note at `/zz/demo`",
        ),
        ("zz/demo/t1", "`zz/demo/t1` is no address"),
    ] {
        for flags in [&[at][..], &[at, "--json"][..]] {
            let refused = text(&out, flags);
            assert_eq!(refused.status.code(), Some(1), "{flags:?}");
            assert_eq!(stdout(&refused), "", "{flags:?}");
            assert!(
                stderr(&refused).contains(said),
                "{flags:?}: {}",
                stderr(&refused)
            );
        }
    }

    // A directory that is no workspace is refused in the same way.
    let refused = text(&scratch.join("nothing"), &["--json"]);
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(stdout(&refused), "");
    assert!(stderr(&refused).contains("E730"), "{}", stderr(&refused));
}

#[test]
fn text_for_a_person_prints_each_provision_indented_by_depth_and_notes_after_their_unit() {
    let scratch = Scratch::new("person");
    let out = scratch.join("workspace");
    write_workspace(&out);
    let expected = "\
/zz/demo/t1
  Title 1— Invented Things
/zz/demo/t1/note/editorialNotes/nolaw/prior-provisions (editorial)
  Prior Provisions
  An invented title stood here.

/zz/demo/t1/s4
  § 4.[1] Records
  An invented maker shall keep—
  /zz/demo/t1/s4/a
    (a)
    a record of each widget; and
  /zz/demo/t1/s4/b (repealed)
    (b)
    a list of—
    /zz/demo/t1/s4/b/1
      (1)
      the colors of each widget.
  (Added by an invented act.)
  [1] So in the invented original.
/zz/demo/t1/s4/a/note/editorialNotes/nolaw/codification (editorial)
  Codification
  An invented act read:
    “(a)
      a record of each widget.”
/zz/demo/t1/s4/note/demoNotes/nolaw/short-title (enacted)
  Short Title
  This invented section may be cited as the Record Act.

/zz/demo/t1/s8
  § 8. Widgets
  A widget is described in section 4.
  Kind | Weight
  Small | 1 gram
";
    assert_eq!(printed(&text(&out, &[])), expected);
    let demo = printed(&text(&demo_workspace(), &[]));
    assert_eq!(
        demo,
        "/zz/demo/t1\n  Title 1— Invented Things\n\n/zz/demo/t1/ch1\n  Chapter 1— Widgets\n\n\
         /zz/demo/t1/s1\n  § 1. Definitions\n  In this invented chapter, a widget is a small \
         device.\n\n/zz/demo/t1/s2\n  § 2. Sizes\n  A widget is small or large.\n\n\
         /zz/demo/t1/s3\n  § 3. Colors\n  A widget may be of any color.\n"
    );
}
