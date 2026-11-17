//! Tests of lowering and of the checks of one module. Every module here is
//! invented, in the invented corpus `/zz/demo`, and so is every text.

use super::{Lowered, check_module, check_source, lower};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::hash::{hash_provision, layer_hashes_of_unit};
use fidryn_provision::normalize::is_normalized;
use fidryn_provision::render::render;
use fidryn_provision::{
    Address, Basis, BindTarget, Binding, Block, Cell, Citation, ContainerModule, CuratedModule,
    Definition, DefinitionKind, Footnote, Hash32, Import, Item, Layer, Module, Note, Num, PartRole,
    Provision, Quoted, QuotedItem, QuotedLevel, Row, RowGroup, Run, Table, Text, UnitModule,
};
use fidryn_syntax::e2::parse;
use std::ops::Range;
use std::path::Path;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/e2")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn address(text: &str) -> Address {
    Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn words(text: &str) -> Run {
    Run::Words(text.to_owned())
}

fn link(words: &str, target: &str) -> Run {
    Run::Ref {
        words: words.to_owned(),
        target: target.to_owned(),
    }
}

fn mark(shown: &str, footnote: u32) -> Run {
    Run::Mark {
        shown: shown.to_owned(),
        footnote,
    }
}

fn text(runs: Vec<Run>) -> Text {
    Text::from_runs(runs)
}

fn plain(words: &str) -> Text {
    text(vec![self::words(words)])
}

fn para(runs: Vec<Run>) -> Block {
    Block::Para(text(runs))
}

fn num(value: &str, shown: &str) -> Option<Num> {
    Some(Num {
        value: value.to_owned(),
        shown: plain(shown),
    })
}

fn content(blocks: Vec<Block>) -> Item {
    Item::Part {
        role: PartRole::Content,
        blocks,
    }
}

fn cell(header: bool, words: &str) -> Cell {
    Cell {
        header,
        colspan: 1,
        rowspan: 1,
        blocks: vec![para(vec![self::words(words)])],
    }
}

/// The subsection of the unit that holds its definition.
fn subsection_a() -> Provision {
    Provision {
        address: address("/zz/demo/t1/s8/a"),
        kind: "subsection".to_owned(),
        num: num("a", "(a)"),
        heading: None,
        status: None,
        items: vec![content(vec![para(vec![
            words("As used in "),
            link("section 2 of this title", "/zz/demo/t1/s2"),
            words(", the term “widget” means a small device."),
        ])])],
    }
}

/// The subsection of the unit that holds a table and a footnote marker.
fn subsection_b() -> Provision {
    let table = Table {
        rows: vec![
            Row {
                group: RowGroup::Head,
                cells: vec![cell(true, "Size"), cell(true, "Width")],
            },
            Row {
                group: RowGroup::Body,
                cells: vec![cell(false, "small"), cell(false, "1 inch")],
            },
            Row {
                group: RowGroup::Body,
                cells: vec![
                    cell(false, "large"),
                    Cell {
                        header: false,
                        colspan: 1,
                        rowspan: 1,
                        blocks: Vec::new(),
                    },
                ],
            },
            Row {
                group: RowGroup::Foot,
                cells: vec![Cell {
                    header: false,
                    colspan: 2,
                    rowspan: 1,
                    blocks: vec![para(vec![words("Every width is invented.")])],
                }],
            },
        ],
    };
    Provision {
        address: address("/zz/demo/t1/s8/b"),
        kind: "subsection".to_owned(),
        num: num("b", "(b)"),
        heading: Some(plain("Sizes")),
        status: None,
        items: vec![content(vec![
            para(vec![
                words("A widget comes in the sizes of this table."),
                mark("1", 1),
            ]),
            Block::Table(table),
        ])],
    }
}

/// The unit of the reference, with a second subsection that holds a table,
/// a footnote and an editorial note.
fn golden_unit() -> UnitModule {
    let root = Provision {
        address: address("/zz/demo/t1/s8"),
        kind: "section".to_owned(),
        num: num("8", "§ 8."),
        heading: Some(plain("Words that include a widget")),
        status: None,
        items: vec![Item::Child(subsection_a()), Item::Child(subsection_b())],
    };
    UnitModule {
        address: address("/zz/demo/t1/s8"),
        aliases: Vec::new(),
        within: Some(address("/zz/demo/t1/ch1")),
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        root,
        credit: Some(plain("(Added by an invented act.)")),
        notes: vec![Note {
            address: address("/zz/demo/t1/s8/note/editorialNotes/nolaw/codification"),
            layer: Layer::Editorial,
            group: "editorialNotes".to_owned(),
            topics: vec!["codification".to_owned()],
            heading: Some(plain("Codification")),
            blocks: vec![para(vec![words(
                "This invented section was enacted as part of an invented code.",
            )])],
        }],
        footnotes: vec![Footnote {
            ordinal: 1,
            shown: "1".to_owned(),
            blocks: vec![para(vec![words("So in the invented original.")])],
        }],
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

fn golden_container() -> ContainerModule {
    ContainerModule {
        address: address("/zz/demo/t1/ch1"),
        aliases: Vec::new(),
        within: Some(address("/zz/demo/t1")),
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        kind: "chapter".to_owned(),
        num: Some(Num {
            value: "1".to_owned(),
            shown: text(vec![words("CHAPTER 1—"), mark("1", 1)]),
        }),
        heading: Some(plain("Widgets")),
        status: None,
        members: vec![address("/zz/demo/t1/s8"), address("/zz/demo/t1/s9")],
        credit: Some(plain("(Invented act, ch. 1.)")),
        notes: vec![Note {
            address: address("/zz/demo/t1/ch1/note/statutoryNotes/nolaw/short-title"),
            layer: Layer::Enacted,
            group: "statutoryNotes".to_owned(),
            topics: vec!["shortTitle".to_owned()],
            heading: Some(plain("Short Title")),
            blocks: vec![para(vec![words(
                "This invented chapter may be cited as the Widget Act.",
            )])],
        }],
        footnotes: vec![Footnote {
            ordinal: 1,
            shown: "1".to_owned(),
            blocks: vec![para(vec![words("An invented footnote of the chapter.")])],
        }],
    }
}

/// A curated module that binds two provisions of the golden unit, with the
/// hashes of that unit.
fn golden_curated() -> CuratedModule {
    let notes_hash = layer_hashes_of_unit(&golden_unit()).enacted;
    CuratedModule {
        address: address("/zz/demo/curated/widgets"),
        imports: vec![
            Import {
                module: address("/zz/demo/curated/gadgets"),
                alias: Some("gadgets".to_owned()),
                names: Vec::new(),
            },
            Import {
                module: address("/zz/demo/curated/sprockets"),
                alias: None,
                names: vec!["sprocket_rule".to_owned(), "sprocket_size".to_owned()],
            },
        ],
        citations: vec![
            Citation {
                name: "widget_rule".to_owned(),
                target: address("/zz/demo/t1/s8/a"),
            },
            Citation {
                name: "widget_sizes".to_owned(),
                target: address("/zz/demo/t1/s8/b"),
            },
        ],
        bindings: vec![
            Binding {
                name: "widget_binding".to_owned(),
                binds: BindTarget::Name {
                    module: None,
                    name: "widget_rule".to_owned(),
                },
                release: "demo-1".to_owned(),
                provision_hash: hash_provision(&subsection_a()),
                notes_hash,
            },
            Binding {
                name: "sizes_binding".to_owned(),
                binds: BindTarget::Address(address("/zz/demo/t1/s8/b")),
                release: "demo-1".to_owned(),
                provision_hash: hash_provision(&subsection_b()),
                notes_hash,
            },
        ],
    }
}

fn goldens() -> Vec<(&'static str, Module)> {
    vec![
        ("unit.fr", Module::Unit(golden_unit())),
        ("container.fr", Module::Container(golden_container())),
        ("curated.fr", Module::Curated(golden_curated())),
    ]
}

#[test]
fn render_of_the_golden_models_is_the_golden_text() {
    for (name, module) in goldens() {
        let golden = fixture(name);
        assert_eq!(render(&module), golden, "{name}");
        match check_source(&golden) {
            Ok(lowered) => assert_eq!(lowered.module, module, "{name}"),
            Err(diagnostics) => panic!("{name}: {diagnostics:#?}"),
        }
    }
}

#[test]
fn render_is_deterministic() {
    let mut generator = Generator::new(0x5EED_0006_0001);
    let mut modules: Vec<Module> = goldens().into_iter().map(|(_, module)| module).collect();
    modules.push(Module::Unit(generator.unit(1)));
    modules.push(Module::Container(generator.container(1)));
    for module in modules {
        let first = render(&module);
        assert_eq!(render(&module), first);
        assert_eq!(render(&module.clone()), first);
        assert!(first.ends_with("}\n"), "{first}");
        assert!(
            first
                .lines()
                .all(|line| !line.is_empty() && !line.ends_with(' ')),
            "{first}"
        );
    }
}

/// Lower `text`, which must parse.
fn lowered_from(text: &str) -> Result<Lowered, Vec<Diagnostic>> {
    let parsed = parse(text);
    assert!(
        parsed.diagnostics.is_empty(),
        "{:?}\n{text}",
        parsed.diagnostics
    );
    lower(&parsed)
}

#[test]
fn rendering_then_reading_gives_the_model_back() {
    let mut generator = Generator::new(0x5EED_0006_0002);
    let mut modules = Vec::new();
    for index in 0..500 {
        modules.push(Module::Unit(generator.unit(index)));
    }
    for index in 0..100 {
        modules.push(Module::Container(generator.container(index)));
    }
    for index in 0..50 {
        modules.push(Module::Curated(generator.curated(index)));
    }
    assert!(
        generator.deepest >= 8,
        "children {} deep",
        generator.deepest
    );
    let mut seen = Coverage::default();
    for module in modules {
        seen.add(&module);
        let text = render(&module);
        let lowered = lowered_from(&text).unwrap_or_else(|diagnostics| {
            panic!("{diagnostics:#?}\n{text}");
        });
        assert_eq!(lowered.module, module, "\n{text}");
        let diagnostics = check_module(&lowered);
        assert!(diagnostics.is_empty(), "{diagnostics:#?}\n{text}");
    }
    seen.assert_complete();
}

/// What the generated modules hold, so that the round trip is known to
/// cover each form.
#[derive(Default)]
struct Coverage {
    runs: [bool; 5],
    short_parts: bool,
    long_parts: bool,
    empty_cells: bool,
    spans: bool,
    images: bool,
    levels: bool,
    layers: [bool; 2],
    footnotes: bool,
    definitions: bool,
    unstated: bool,
    aliases: bool,
    statuses: bool,
    absent_nums: bool,
    absent_headings: bool,
    empty_shown: bool,
    characters: [bool; 7],
}

impl Coverage {
    fn add(&mut self, module: &Module) {
        let mut addresses = Vec::new();
        match module {
            Module::Unit(unit) => {
                self.aliases |= !unit.aliases.is_empty();
                self.footnotes |= !unit.footnotes.is_empty();
                self.definitions |= !unit.definitions.is_empty();
                self.unstated |= unit
                    .definitions
                    .iter()
                    .any(|definition| definition.words.is_none());
                self.provision(&unit.root, &mut addresses);
                self.matter(unit.credit.as_ref(), &unit.notes, &unit.footnotes);
            }
            Module::Container(container) => {
                addresses.push(container.address.as_str().to_owned());
                self.matter(
                    container.credit.as_ref(),
                    &container.notes,
                    &container.footnotes,
                );
            }
            Module::Curated(_) => {}
        }
        for (seen, character) in self
            .characters
            .iter_mut()
            .zip(['a', 'A', '7', '-', '.', '~', '\u{2013}'])
        {
            *seen |= addresses.iter().any(|address| address.contains(character));
        }
    }

    fn matter(&mut self, credit: Option<&Text>, notes: &[Note], footnotes: &[Footnote]) {
        if let Some(credit) = credit {
            self.text(credit);
        }
        for note in notes {
            self.layers[usize::from(note.layer == Layer::Editorial)] = true;
            self.blocks(&note.blocks);
        }
        for footnote in footnotes {
            self.blocks(&footnote.blocks);
        }
    }

    fn provision(&mut self, provision: &Provision, addresses: &mut Vec<String>) {
        addresses.push(provision.address.as_str().to_owned());
        self.statuses |= provision.status.is_some();
        self.absent_nums |= provision.num.is_none();
        self.absent_headings |= provision.heading.is_none();
        if let Some(num) = &provision.num {
            self.empty_shown |= num.shown.runs.is_empty();
        }
        for item in &provision.items {
            match item {
                Item::Part { blocks, .. } => {
                    if matches!(blocks.as_slice(), [Block::Para(_)]) {
                        self.short_parts = true;
                    } else {
                        self.long_parts = true;
                    }
                    self.blocks(blocks);
                }
                Item::Child(child) => self.provision(child, addresses),
            }
        }
    }

    fn text(&mut self, text: &Text) {
        for run in &text.runs {
            let index = match run {
                Run::Words(_) => 0,
                Run::Ref { .. } => 1,
                Run::Sup(_) => 2,
                Run::Sub(_) => 3,
                Run::Mark { .. } => 4,
            };
            self.runs[index] = true;
        }
    }

    fn blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            match block {
                Block::Para(text) | Block::Subheading(text) => self.text(text),
                Block::Table(table) => {
                    for cell in table.rows.iter().flat_map(|row| &row.cells) {
                        self.empty_cells |= cell.blocks.is_empty();
                        self.spans |= cell.colspan != 1 || cell.rowspan != 1;
                        self.blocks(&cell.blocks);
                    }
                }
                Block::Quoted(quoted) => self.quoted(&quoted.items),
                Block::Image { .. } => self.images = true,
            }
        }
    }

    fn quoted(&mut self, items: &[QuotedItem]) {
        for item in items {
            match item {
                QuotedItem::Block(block) => self.blocks(std::slice::from_ref(block)),
                QuotedItem::Part { blocks, .. } => self.blocks(blocks),
                QuotedItem::Level(level) => {
                    self.levels = true;
                    self.quoted(&level.items);
                }
            }
        }
    }

    fn assert_complete(&self) {
        let checks = [
            ("every run form", self.runs.iter().all(|seen| *seen)),
            ("a part of one paragraph", self.short_parts),
            ("a part of several blocks", self.long_parts),
            ("an empty cell", self.empty_cells),
            ("a cell that spans", self.spans),
            ("an image", self.images),
            ("a quoted level", self.levels),
            ("notes of both layers", self.layers.iter().all(|seen| *seen)),
            ("a footnote", self.footnotes),
            ("a definition", self.definitions),
            ("unstated words", self.unstated),
            ("an alias", self.aliases),
            ("a status", self.statuses),
            ("an absent number", self.absent_nums),
            ("an absent heading", self.absent_headings),
            ("a number with an empty shown form", self.empty_shown),
            (
                "every character of an address",
                self.characters.iter().all(|seen| *seen),
            ),
        ];
        for (what, seen) in checks {
            assert!(seen, "the generated modules hold no {what}");
        }
    }
}

/// A seeded xorshift generator: the same sequence on every run and every
/// host.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A number in `0..bound`.
    fn below(&mut self, bound: usize) -> usize {
        let bound = u64::try_from(bound).expect("a bound fits in u64");
        usize::try_from(self.next() % bound).expect("a value below a usize bound")
    }

    fn chance(&mut self, percent: u64) -> bool {
        self.next() % 100 < percent
    }

    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Pieces of words: normalized, without a space at either end, and with no
/// character that composes with the one before it.
const PIECES: &[&str] = &[
    "widget",
    "gadget",
    "Sprocket",
    "§ 8",
    "(a)",
    "“term”",
    "—",
    "s1395i–3",
    "1 inch",
    "é",
    "[sic]",
    "C3H8",
    "x²",
    "back\\slash",
    "a \"quote\"",
    "\u{1}",
    "\u{7f}",
    "zero\u{200b}width",
    "\u{feff}mark",
];
const TARGETS: &[&str] = &[
    "/zz/demo/t1/s2",
    "/zz/demo/t1/s8/a",
    "/yy/stat/1/2",
    "an \"invented\" act\\1",
];
const MARKERS: &[&str] = &["1", "2", "*", "†"];
const KINDS: &[&str] = &[
    "section",
    "subsection",
    "paragraph",
    "clause",
    "days",
    "level_2",
];
const STATUSES: &[&str] = &["repealed", "omitted", "transferred"];
const GROUPS: &[&str] = &["editorialNotes", "statutoryNotes", "ungrouped", "demoNotes"];
const TOPICS: &[&str] = &["codification", "amendments", "a\"b", "c\\d", "\u{1}"];
const IMAGES: &[&str] = &["figure1.eps", "a \"b\".eps"];
const ORIGINS: &[&str] = &["/yy/act/1947/ch388", "an invented act, sec. 2"];
const SOURCES: &[&str] = &["demo01.xml", "dir/demo \"2\".xml", "back\\slash.xml"];
const ROLES: &[PartRole] = &[
    PartRole::Chapeau,
    PartRole::Content,
    PartRole::Continuation,
    PartRole::Proviso,
];
/// Characters of the segments of an address, every one S2 allows among
/// them.
const SEGMENT_CHARACTERS: &[char] = &['a', 'z', 'A', 'Q', '7', '-', '.', '~', '\u{2013}'];

/// A generator of valid modules: every text normalized, every address in
/// its place and declared once, every marker naming a footnote, and every
/// definition standing in its provision.
struct Generator {
    rng: Rng,
    /// The number of footnotes of the module being generated.
    footnotes: usize,
    /// The deepest provision generated, counted from the top at 0.
    deepest: usize,
}

impl Generator {
    fn new(seed: u64) -> Generator {
        Generator {
            rng: Rng(seed),
            footnotes: 0,
            deepest: 0,
        }
    }

    fn piece(&mut self) -> String {
        let count = 1 + self.rng.below(3);
        (0..count)
            .map(|_| *self.rng.pick(PIECES))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// A normalized text. A single space stands between some runs, at the
    /// end of the run before or at the start of the run after.
    fn text(&mut self) -> Text {
        let count = 1 + self.rng.below(4);
        let mut runs: Vec<Run> = Vec::new();
        let mut has_words = false;
        for _ in 0..count {
            let kind = self.rng.below(10);
            if kind == 9 && self.footnotes > 0 {
                let footnote = 1 + self.rng.below(self.footnotes);
                runs.push(mark(
                    self.rng.pick(MARKERS),
                    u32::try_from(footnote).expect("a footnote number"),
                ));
                continue;
            }
            let mut piece = self.piece();
            if has_words && self.rng.chance(60) {
                let before = match runs.last_mut() {
                    Some(Run::Words(words) | Run::Sup(words) | Run::Sub(words)) => Some(words),
                    Some(Run::Ref { words, .. }) => Some(words),
                    _ => None,
                };
                match before {
                    Some(words) if self.rng.chance(50) => words.push(' '),
                    _ => piece.insert(0, ' '),
                }
            }
            has_words = true;
            runs.push(match kind {
                5 | 6 => link(&piece, self.rng.pick(TARGETS)),
                7 => Run::Sup(piece),
                8 => Run::Sub(piece),
                _ => Run::Words(piece),
            });
        }
        let text = Text::from_runs(runs);
        assert!(is_normalized(&text.plain()), "{text:?}");
        text
    }

    fn maybe_text(&mut self, percent: u64) -> Option<Text> {
        self.rng.chance(percent).then(|| self.text())
    }

    fn num(&mut self) -> Option<Num> {
        if self.rng.chance(20) {
            return None;
        }
        let value = (*self.rng.pick(&["8", "a", "iv", "A", "1395i–3", "2a"])).to_owned();
        let shown = if self.rng.chance(15) {
            Text::default()
        } else {
            self.text()
        };
        Some(Num { value, shown })
    }

    /// A segment of an address that ends in `serial`, so that it is never
    /// `.`, `..` or `note`.
    fn segment(&mut self, serial: usize) -> String {
        let count = 1 + self.rng.below(3);
        let mut segment: String = (0..count)
            .map(|_| *self.rng.pick(SEGMENT_CHARACTERS))
            .collect();
        segment.push_str(&serial.to_string());
        segment
    }

    fn block(&mut self, depth: usize) -> Block {
        match self.rng.below(12) {
            6 => Block::Subheading(self.text()),
            7 | 8 if depth < 3 => Block::Table(self.table(depth + 1)),
            9 => Block::Image {
                source: (*self.rng.pick(IMAGES)).to_owned(),
            },
            10 | 11 if depth < 3 => Block::Quoted(self.quoted(depth + 1)),
            _ => Block::Para(self.text()),
        }
    }

    fn blocks(&mut self, most: usize, depth: usize) -> Vec<Block> {
        let count = self.rng.below(most + 1);
        (0..count).map(|_| self.block(depth)).collect()
    }

    /// The blocks of a text part: at least one.
    fn part_blocks(&mut self, depth: usize) -> Vec<Block> {
        if self.rng.chance(50) {
            vec![Block::Para(self.text())]
        } else {
            let count = 1 + self.rng.below(3);
            (0..count).map(|_| self.block(depth)).collect()
        }
    }

    fn table(&mut self, depth: usize) -> Table {
        let rows = self.rng.below(4);
        Table {
            rows: (0..rows)
                .map(|_| {
                    let group = *self
                        .rng
                        .pick(&[RowGroup::Head, RowGroup::Body, RowGroup::Foot]);
                    let cells = self.rng.below(4);
                    Row {
                        group,
                        cells: (0..cells).map(|_| self.cell(depth)).collect(),
                    }
                })
                .collect(),
        }
    }

    fn cell(&mut self, depth: usize) -> Cell {
        let header = self.rng.chance(30);
        let colspan = if self.rng.chance(20) {
            2 + self.rng.below(2)
        } else {
            1
        };
        let rowspan = if self.rng.chance(20) {
            2 + self.rng.below(2)
        } else {
            1
        };
        let blocks = match self.rng.below(4) {
            0 => Vec::new(),
            1 | 2 => vec![Block::Para(self.text())],
            _ => {
                let count = 1 + self.rng.below(2);
                (0..count).map(|_| self.block(depth)).collect()
            }
        };
        Cell {
            header,
            colspan: u32::try_from(colspan).expect("a span"),
            rowspan: u32::try_from(rowspan).expect("a span"),
            blocks,
        }
    }

    fn quoted(&mut self, depth: usize) -> Quoted {
        let origin = self
            .rng
            .chance(50)
            .then(|| (*self.rng.pick(ORIGINS)).to_owned());
        Quoted {
            origin,
            items: self.quoted_items(depth),
        }
    }

    fn quoted_items(&mut self, depth: usize) -> Vec<QuotedItem> {
        let count = self.rng.below(4);
        (0..count)
            .map(|_| match self.rng.below(3) {
                0 => QuotedItem::Block(self.block(depth)),
                1 => QuotedItem::Part {
                    role: *self.rng.pick(ROLES),
                    blocks: self.part_blocks(depth),
                },
                _ if depth < 5 => QuotedItem::Level(QuotedLevel {
                    kind: (*self.rng.pick(KINDS)).to_owned(),
                    num: self.num(),
                    heading: self.maybe_text(50),
                    items: self.quoted_items(depth + 1),
                }),
                _ => QuotedItem::Block(Block::Para(self.text())),
            })
            .collect()
    }

    /// A provision at `address`, `depth` below the top, with a chain of
    /// `chain` descendants below it.
    fn provision(&mut self, address: Address, depth: usize, chain: usize) -> Provision {
        self.deepest = self.deepest.max(depth);
        let mut items = Vec::new();
        for _ in 0..self.rng.below(3) {
            items.push(Item::Part {
                role: *self.rng.pick(ROLES),
                blocks: self.part_blocks(0),
            });
        }
        let children = match (chain, depth) {
            (0, 8..) => 0,
            (0, 0 | 1) => self.rng.below(3),
            (0, _) => self.rng.below(2),
            _ => 1,
        };
        for serial in 0..children {
            let segment = self.segment(serial);
            let child_address = address.child(&segment).expect("a segment");
            let child = self.provision(child_address, depth + 1, chain.saturating_sub(1));
            let at = self.rng.below(items.len() + 1);
            items.insert(at, Item::Child(child));
        }
        Provision {
            address,
            kind: (*self.rng.pick(KINDS)).to_owned(),
            num: self.num(),
            heading: self.maybe_text(50),
            status: self
                .rng
                .chance(20)
                .then(|| (*self.rng.pick(STATUSES)).to_owned()),
            items,
        }
    }

    fn notes(&mut self, module: &Address) -> Vec<Note> {
        let count = self.rng.below(4);
        (0..count)
            .map(|serial| {
                let group = (*self.rng.pick(GROUPS)).to_owned();
                let heading = self.segment(serial);
                let topics = self.rng.below(3);
                Note {
                    address: address(&format!("{module}/note/{group}/nolaw/{heading}")),
                    layer: if self.rng.chance(50) {
                        Layer::Enacted
                    } else {
                        Layer::Editorial
                    },
                    group,
                    topics: (0..topics)
                        .map(|_| (*self.rng.pick(TOPICS)).to_owned())
                        .collect(),
                    heading: self.maybe_text(70),
                    blocks: self.blocks(3, 0),
                }
            })
            .collect()
    }

    fn footnotes(&mut self) -> Vec<Footnote> {
        (1..=self.footnotes)
            .map(|ordinal| Footnote {
                ordinal: u32::try_from(ordinal).expect("a footnote number"),
                shown: (*self.rng.pick(MARKERS)).to_owned(),
                blocks: self.blocks(2, 0),
            })
            .collect()
    }

    fn aliases(&mut self, serial: usize) -> Vec<Address> {
        let count = self.rng.below(3);
        (0..count)
            .map(|k| address(&format!("/zz/demo/t1/alias{serial}x{k}")))
            .collect()
    }

    fn unit(&mut self, serial: usize) -> UnitModule {
        self.footnotes = self.rng.below(4);
        let title = 1 + self.rng.below(50);
        let segment = self.segment(serial);
        let address = address(&format!("/zz/demo/t{title}/{segment}"));
        let chain = if serial.is_multiple_of(5) {
            8
        } else {
            self.rng.below(4)
        };
        let root = self.provision(address.clone(), 0, chain);
        let definitions = self.definitions(&root);
        UnitModule {
            aliases: self.aliases(serial),
            within: self
                .rng
                .chance(70)
                .then(|| self::address(&format!("/zz/demo/t{title}/ch1"))),
            source: (*self.rng.pick(SOURCES)).to_owned(),
            positive_law: *self.rng.pick(&[None, Some(true), Some(false)]),
            credit: self.maybe_text(50),
            notes: self.notes(&address),
            footnotes: self.footnotes(),
            definitions,
            address,
            root,
        }
    }

    fn container(&mut self, serial: usize) -> ContainerModule {
        self.footnotes = self.rng.below(3);
        let segment = self.segment(serial);
        let address = address(&format!("/zz/demo/t1/ch{segment}"));
        let members = self.rng.below(5);
        ContainerModule {
            aliases: self.aliases(serial),
            within: self.rng.chance(70).then(|| self::address("/zz/demo/t1")),
            source: (*self.rng.pick(SOURCES)).to_owned(),
            positive_law: *self.rng.pick(&[None, Some(true), Some(false)]),
            kind: (*self.rng.pick(&["chapter", "subchapter", "part", "title"])).to_owned(),
            num: self.num(),
            heading: self.maybe_text(60),
            status: self
                .rng
                .chance(20)
                .then(|| (*self.rng.pick(STATUSES)).to_owned()),
            members: (0..members)
                .map(|k| self::address(&format!("/zz/demo/t1/s{k}")))
                .collect(),
            credit: self.maybe_text(50),
            notes: self.notes(&address),
            footnotes: self.footnotes(),
            address,
        }
    }

    fn hash(&mut self) -> Hash32 {
        let mut bytes = [0_u8; 32];
        for chunk in bytes.chunks_mut(8) {
            chunk.copy_from_slice(&self.rng.next().to_le_bytes());
        }
        Hash32(bytes)
    }

    fn curated(&mut self, serial: usize) -> CuratedModule {
        let imports = self.rng.below(3);
        let citations = self.rng.below(3);
        let bindings = self.rng.below(3);
        CuratedModule {
            address: address(&format!("/zz/demo/curated/m{serial}")),
            imports: (0..imports)
                .map(|k| {
                    let module = address(&format!("/zz/demo/curated/other{k}"));
                    if self.rng.chance(50) {
                        Import {
                            module,
                            alias: Some(format!("other{k}")),
                            names: Vec::new(),
                        }
                    } else {
                        let names = 1 + self.rng.below(3);
                        Import {
                            module,
                            alias: None,
                            names: (0..names).map(|n| format!("name_{n}")).collect(),
                        }
                    }
                })
                .collect(),
            citations: (0..citations)
                .map(|k| Citation {
                    name: format!("citation_{k}"),
                    target: address(&format!("/zz/demo/t1/s{k}/a")),
                })
                .collect(),
            bindings: (0..bindings)
                .map(|k| Binding {
                    name: format!("binding_{k}"),
                    binds: match self.rng.below(3) {
                        0 => BindTarget::Address(address(&format!("/zz/demo/t1/s{k}"))),
                        1 => BindTarget::Name {
                            module: None,
                            name: "citation_0".to_owned(),
                        },
                        _ => BindTarget::Name {
                            module: Some("other0".to_owned()),
                            name: "name_0".to_owned(),
                        },
                    },
                    release: (*self.rng.pick(&["demo-1", "a \"b\"\\c"])).to_owned(),
                    provision_hash: self.hash(),
                    notes_hash: self.hash(),
                })
                .collect(),
        }
    }

    /// Definitions of provisions of the tree below `root`, each term and
    /// stated reach taken from the text of its provision or of the chapeau
    /// of the provision's parent, in the order that `render` writes them.
    fn definitions(&mut self, root: &Provision) -> Vec<Definition> {
        let placed = placed(root);
        let candidates: Vec<usize> = (0..placed.len())
            .filter(|&index| !words_of(placed[index].0, |_| true).is_empty())
            .collect();
        if candidates.is_empty() {
            return Vec::new();
        }
        let count = self.rng.below(3);
        let mut definitions: Vec<(usize, Definition)> = (0..count)
            .map(|_| {
                let index = *self.rng.pick(&candidates);
                let (provision, parent) = placed[index];
                let own = words_of(provision, |_| true);
                let chapeau = parent
                    .map(|parent| words_of(parent, |role| role == PartRole::Chapeau))
                    .unwrap_or_default();
                let terms = 1 + self.rng.below(2);
                let words = match self.rng.below(3) {
                    0 => None,
                    1 if !chapeau.is_empty() => Some(self.rng.pick(&chapeau).clone()),
                    _ => Some(self.rng.pick(&own).clone()),
                };
                let kind = *self.rng.pick(&[
                    DefinitionKind::Means,
                    DefinitionKind::Includes,
                    DefinitionKind::DoesNotInclude,
                    DefinitionKind::Borrowed,
                ]);
                let borrowed_from = (kind == DefinitionKind::Borrowed || self.rng.chance(20))
                    .then(|| (*self.rng.pick(TARGETS)).to_owned());
                let definition = Definition {
                    at: provision.address.clone(),
                    terms: (0..terms).map(|_| self.rng.pick(&own).clone()).collect(),
                    kind,
                    words,
                    region: self.rng.chance(50).then(|| address("/zz/demo/t1/s2")),
                    basis: *self
                        .rng
                        .pick(&[Basis::Enacted, Basis::Restated, Basis::Unresolved]),
                    borrowed_from,
                    see_note: self
                        .rng
                        .chance(30)
                        .then(|| address("/zz/demo/t1/s8/note/editorialNotes/nolaw/x")),
                };
                (index, definition)
            })
            .collect();
        definitions.sort_by(|(a, first), (b, second)| {
            a.cmp(b)
                .then_with(|| first.terms.first().cmp(&second.terms.first()))
        });
        definitions
            .into_iter()
            .map(|(_, definition)| definition)
            .collect()
    }
}

/// Every provision of the tree below `root` with its parent, in document
/// order.
fn placed(root: &Provision) -> Vec<(&Provision, Option<&Provision>)> {
    fn walk<'a>(
        provision: &'a Provision,
        parent: Option<&'a Provision>,
        out: &mut Vec<(&'a Provision, Option<&'a Provision>)>,
    ) {
        out.push((provision, parent));
        for item in &provision.items {
            if let Item::Child(child) = item {
                walk(child, Some(provision), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, None, &mut out);
    out
}

/// The words, split at spaces, of the paragraphs at the top of the text
/// parts of `provision` whose role `wanted` accepts.
fn words_of(provision: &Provision, wanted: impl Fn(PartRole) -> bool) -> Vec<String> {
    let mut out = Vec::new();
    for item in &provision.items {
        if let Item::Part { role, blocks } = item
            && wanted(*role)
        {
            for block in blocks {
                if let Block::Para(text) = block {
                    out.extend(
                        text.plain()
                            .split(' ')
                            .filter(|word| !word.is_empty())
                            .map(str::to_owned),
                    );
                }
            }
        }
    }
    out
}

/// Where a defect's diagnostic must point.
enum At {
    /// Inside the lines of the change, and on the changed text.
    Change,
    /// On the value of the first `hash` after this text.
    HashAfter(&'static str),
}

struct Defect {
    what: &'static str,
    find: String,
    replace: String,
    code: DiagnosticCode,
    message: Option<&'static str>,
    at: At,
}

fn defect(what: &'static str, find: &str, replace: &str, code: DiagnosticCode) -> Defect {
    Defect {
        what,
        find: find.to_owned(),
        replace: replace.to_owned(),
        code,
        message: None,
        at: At::Change,
    }
}

/// `hash` with its first digit changed.
fn changed_digit(hash: &str) -> String {
    let first = if hash.starts_with('0') { "1" } else { "0" };
    format!("{first}{}", &hash[1..])
}

fn unit_defects() -> Vec<Defect> {
    let unit = golden_unit();
    let hash_a = hash_provision(&subsection_a()).to_string();
    let editorial = layer_hashes_of_unit(&unit).editorial.to_string();
    let root = hash_provision(&unit.root).to_string();
    vec![
        Defect {
            at: At::HashAfter("provision \"/zz/demo/t1/s8/a\""),
            ..defect(
                "one letter of a content string",
                "a small device",
                "a small devise",
                DiagnosticCode::E710,
            )
        },
        defect(
            "one digit of a hash",
            &hash_a,
            &changed_digit(&hash_a),
            DiagnosticCode::E710,
        ),
        Defect {
            message: Some("text is not normalized"),
            ..defect(
                "two spaces between two words of a string",
                "a small device",
                "a  small device",
                DiagnosticCode::E710,
            )
        },
        defect(
            "a child's address moved outside its parent's",
            "provision \"/zz/demo/t1/s8/b\"",
            "provision \"/zz/demo/t1/s9/b\"",
            DiagnosticCode::E700,
        ),
        Defect {
            message: Some("declared twice"),
            ..defect(
                "one address written twice",
                "provision \"/zz/demo/t1/s8/b\"",
                "provision \"/zz/demo/t1/s8/a\"",
                DiagnosticCode::E700,
            )
        },
        defect(
            "the top provision's address not the module's",
            "provision \"/zz/demo/t1/s8\" section",
            "provision \"/zz/demo/t1/s7\" section",
            DiagnosticCode::E700,
        ),
        defect(
            "a note's address outside the module's",
            "note \"/zz/demo/t1/s8/note/",
            "note \"/zz/demo/t1/s9/note/",
            DiagnosticCode::E700,
        ),
        defect(
            "`mark \"1\" 2` where there is no footnote 2",
            "mark \"1\" 1",
            "mark \"1\" 2",
            DiagnosticCode::E710,
        ),
        defect(
            "one digit of a value of `layers`",
            &format!("editorial \"{editorial}\""),
            &format!("editorial \"{}\"", changed_digit(&editorial)),
            DiagnosticCode::E710,
        ),
        defect(
            "a definition's term that is not in the provision's text",
            "term \"widget\"",
            "term \"gizmo\"",
            DiagnosticCode::E710,
        ),
        defect(
            "a definition that names a provision of another module",
            "definition \"/zz/demo/t1/s8/a\"",
            "definition \"/zz/demo/t9/s8/a\"",
            DiagnosticCode::E710,
        ),
        defect(
            "an `import` added to the unit",
            "  within \"/zz/demo/t1/ch1\"\n",
            "  import \"/zz/demo/t1/s9\" as other\n  within \"/zz/demo/t1/ch1\"\n",
            DiagnosticCode::E100,
        ),
        defect(
            "a second top `provision`",
            "  credit \"",
            &format!(
                "  provision \"/zz/demo/t1/s9\" section {{\n    hash \"{root}\"\n  }}\n  credit \""
            ),
            DiagnosticCode::E100,
        ),
    ]
}

fn curated_defects() -> Vec<Defect> {
    let hash_a = hash_provision(&subsection_a()).to_string();
    vec![
        defect(
            "a `within` added",
            "module \"/zz/demo/curated/widgets\" {\n",
            "module \"/zz/demo/curated/widgets\" {\n  within \"/zz/demo/curated\"\n",
            DiagnosticCode::E100,
        ),
        defect(
            "a citation name declared twice",
            "citation widget_sizes",
            "citation widget_rule",
            DiagnosticCode::E200,
        ),
        defect(
            "a `provision_hash` of 63 digits",
            &format!("provision_hash \"{hash_a}\""),
            &format!("provision_hash \"{}\"", &hash_a[1..]),
            DiagnosticCode::E710,
        ),
    ]
}

/// Every defect with the source it changes, the changed source, the bytes
/// of the change in it, and the diagnostics of the changed source.
fn defects() -> Vec<(Defect, String, Range<usize>, Vec<Diagnostic>)> {
    let mut out = Vec::new();
    for (file, defects) in [
        ("unit.fr", unit_defects()),
        ("curated.fr", curated_defects()),
    ] {
        let golden = fixture(file);
        for defect in defects {
            let at = golden
                .find(&defect.find)
                .unwrap_or_else(|| panic!("{}: `{}` is not in {file}", defect.what, defect.find));
            assert_eq!(
                golden.rfind(&defect.find),
                Some(at),
                "{}: `{}` stands twice in {file}",
                defect.what,
                defect.find
            );
            let changed = format!(
                "{}{}{}",
                &golden[..at],
                defect.replace,
                &golden[at + defect.find.len()..]
            );
            let change = at..at + defect.replace.len();
            let diagnostics = match check_source(&changed) {
                Ok(_) => panic!("{}: no diagnostic", defect.what),
                Err(diagnostics) => diagnostics,
            };
            out.push((defect, changed, change, diagnostics));
        }
    }
    out
}

/// Whether `span` covers part of `change` and lies on its lines.
fn on_the_change(source: &str, change: &Range<usize>, span: Span) -> bool {
    let lines_start = source[..change.start].rfind('\n').map_or(0, |at| at + 1);
    let lines_end = source[change.end..]
        .find('\n')
        .map_or(source.len(), |at| change.end + at);
    let (start, end) = (span.start as usize, span.end as usize);
    lines_start <= start && end <= lines_end && start < change.end && change.start < end
}

/// The span of the value of the first `hash` after `needle` in `source`.
fn hash_after(source: &str, needle: &str) -> Span {
    let from = source.find(needle).expect("the needle") + needle.len();
    let start = from
        + source[from..]
            .find("hash \"")
            .expect("a hash after the needle")
        + 5;
    let end = start + 66;
    Span {
        start: u32::try_from(start).expect("a span"),
        end: u32::try_from(end).expect("a span"),
    }
}

#[test]
fn each_defect_is_reported_with_its_code_at_its_text() {
    let defects = defects();
    assert_eq!(defects.len(), 16);
    for (defect, changed, change, diagnostics) in defects {
        let found = diagnostics.iter().any(|diagnostic| {
            let Some(span) = diagnostic.primary_span else {
                return false;
            };
            diagnostic.code == defect.code
                && defect
                    .message
                    .is_none_or(|message| diagnostic.message.contains(message))
                && match defect.at {
                    At::Change => on_the_change(&changed, &change, span),
                    At::HashAfter(needle) => span == hash_after(&changed, needle),
                }
        });
        assert!(
            found,
            "{}: no {} at the change among {diagnostics:#?}",
            defect.what, defect.code
        );
    }
}

#[test]
fn every_diagnostic_of_the_one_module_check_has_a_span() {
    for (defect, changed, _, diagnostics) in defects() {
        assert!(!diagnostics.is_empty(), "{}", defect.what);
        for diagnostic in diagnostics {
            let span = diagnostic
                .primary_span
                .unwrap_or_else(|| panic!("{}: {diagnostic:?} has no span", defect.what));
            assert!(
                span.start < span.end && span.end as usize <= changed.len(),
                "{}: {diagnostic:?}",
                defect.what
            );
        }
    }
}

const ZEROS: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A unit with `body` in its top provision and `after` after it, with hashes
/// of the right form and the wrong value.
fn unit_with(body: &str, after: &str) -> String {
    format!(
        "edition 2\nmodule \"/zz/demo/t1/s1\" {{\n  source \"demo01.xml\"\n  \
         provision \"/zz/demo/t1/s1\" section {{\n{body}\n    hash \"{ZEROS}\"\n  }}\n{after}\n  \
         layers {{\n    operative \"{ZEROS}\"\n    enacted \"{ZEROS}\"\n    editorial \"{ZEROS}\"\n  \
         }}\n}}\n"
    )
}

/// The diagnostics of lowering `source`, which parses.
fn lowering_errors(source: &str) -> Vec<Diagnostic> {
    match lowered_from(source) {
        Ok(lowered) => panic!("lowered: {:?}\n{source}", lowered.module),
        Err(diagnostics) => diagnostics,
    }
}

/// The text of `source` at the primary span of `diagnostic`.
fn text_at<'a>(source: &'a str, diagnostic: &Diagnostic) -> &'a str {
    let span = diagnostic.primary_span.expect("a span");
    &source[span.start as usize..span.end as usize]
}

#[test]
fn an_empty_string_is_a_cell_with_no_block_or_a_number_shown_as_nothing() {
    let source = unit_with(
        "    num \"1\" \"\"\n    content {\n      table {\n        body {\n          td \"\"\n        }\n      }\n    }",
        "",
    );
    let Module::Unit(unit) = lowered_from(&source).expect("lowered").module else {
        panic!("not a unit");
    };
    assert_eq!(unit.root.num.expect("a number").shown, Text::default());
    let Item::Part { blocks, .. } = &unit.root.items[0] else {
        panic!("not a part");
    };
    let [Block::Table(table)] = blocks.as_slice() else {
        panic!("not a table");
    };
    assert!(table.rows[0].cells[0].blocks.is_empty());

    for body in [
        "    heading \"\"",
        "    content \"\"",
        "    content \"a\" \"\"",
        "    content \"a\" sup \"\"",
        "    content \"a\" sub \"\"",
        "    content [\"\" -> \"/zz/demo/t1/s2\"]",
        "    num \"1\" \"\" \"a\"",
        "    content {\n      table {\n        body {\n          td \"\" \"a\"\n        }\n      }\n    }",
        "    content {\n      p \"\"\n    }",
    ] {
        let source = unit_with(body, "");
        let errors = lowering_errors(&source);
        assert_eq!(errors.len(), 1, "{body}: {errors:#?}");
        assert_eq!(errors[0].code, DiagnosticCode::E710, "{body}");
        assert_eq!(errors[0].message, "a run has no words", "{body}");
        assert_eq!(text_at(&source, &errors[0]), "\"\"", "{body}");
    }
}

#[test]
fn a_topic_is_one_word_and_an_origin_is_not_empty() {
    for (topic, refused) in [
        ("codification", false),
        ("a\\\"b", false),
        ("", true),
        ("two words", true),
        ("tab\\there", true),
        ("no\u{a0}break", true),
    ] {
        let note = format!(
            "  note \"/zz/demo/t1/s1/note/demoNotes/nolaw/x\" enacted {{\n    group demoNotes\n    \
             topic \"{topic}\"\n    hash \"{ZEROS}\"\n  }}"
        );
        let source = unit_with("    content \"An invented text.\"", &note);
        let parsed = parse(&source);
        let lowered = lower(&parsed);
        match (refused, lowered) {
            (false, Ok(_)) => {}
            (true, Err(errors)) => {
                assert_eq!(errors.len(), 1, "{topic:?}: {errors:#?}");
                assert_eq!(errors[0].code, DiagnosticCode::E710);
                assert_eq!(errors[0].message, "a topic is one word");
                assert_eq!(text_at(&source, &errors[0]), format!("\"{topic}\""));
            }
            (refused, lowered) => panic!("{topic:?}: refused {refused}, {lowered:?}"),
        }
    }
    let source = unit_with(
        "    content {\n      quoted \"\" {\n        p \"An invented quotation.\"\n      }\n    }",
        "",
    );
    let errors = lowering_errors(&source);
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_eq!(errors[0].code, DiagnosticCode::E710);
    assert_eq!(text_at(&source, &errors[0]), "\"\"");
    let source = unit_with(
        "    content {\n      quoted {\n        p \"An invented quotation.\"\n      }\n    }",
        "",
    );
    assert!(lowered_from(&source).is_ok());
}

#[test]
fn lowering_reports_every_error_it_finds() {
    let source = unit_with(
        "    provision \"/zz/demo/t1/s1/a b\" subsection {\n      content \"An invented text.\"\n      \
         hash \"{ZEROS}0\"\n    }",
        "  within \"zz/demo\"\n  definition \"/zz/demo/t1/s1\" {\n    term \"text\"\n    kind defines\n    \
         words unstated\n    basis guessed\n  }",
    )
    .replace("{ZEROS}", ZEROS);
    let errors = lowering_errors(&source);
    let found: Vec<(DiagnosticCode, &str)> = errors
        .iter()
        .map(|diagnostic| (diagnostic.code, text_at(&source, diagnostic)))
        .collect();
    let hash = format!("\"{ZEROS}0\"");
    assert_eq!(
        found,
        [
            (DiagnosticCode::E700, "\"/zz/demo/t1/s1/a b\""),
            (DiagnosticCode::E710, hash.as_str()),
            (DiagnosticCode::E700, "\"zz/demo\""),
            (DiagnosticCode::E710, "defines"),
            (DiagnosticCode::E710, "guessed"),
        ]
    );
}

#[test]
fn declarations_belong_to_the_shape_of_their_module() {
    let container = fixture("container.fr");
    let with_definition = container.replace(
        "  layers {",
        "  definition \"/zz/demo/t1/ch1\" {\n    term \"Widgets\"\n    kind means\n    \
         words unstated\n    basis unresolved\n  }\n  layers {",
    );
    let errors = lowering_errors(&with_definition);
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_eq!(errors[0].code, DiagnosticCode::E710);
    assert_eq!(text_at(&with_definition, &errors[0]), "\"/zz/demo/t1/ch1\"");

    let unit = fixture("unit.fr");
    for (what, changed, expected) in [
        (
            "no layers",
            unit[..unit.find("  layers {").expect("layers")].to_owned() + "}\n",
            "missing declaration `layers` in a unit",
        ),
        (
            "no source",
            unit.replace("  source \"demo01.xml\"\n", ""),
            "missing declaration `source` in a unit",
        ),
        (
            "a second credit",
            unit.replace(
                "  credit \"(Added by an invented act.)\"\n",
                "  credit \"(Added by an invented act.)\"\n  credit \"(Again.)\"\n",
            ),
            "declaration `credit` is given twice",
        ),
        (
            "a container beside the provision",
            unit.replace(
                "  credit \"",
                &format!("  container \"/zz/demo/t1/s8\" chapter {{\n    hash \"{ZEROS}\"\n  }}\n  credit \""),
            ),
            "a module has one `provision` or one `container` at the top, not both",
        ),
    ] {
        let errors = lowering_errors(&changed);
        assert_eq!(errors.len(), 1, "{what}: {errors:#?}");
        assert_eq!(errors[0].code, DiagnosticCode::E100, "{what}");
        assert_eq!(errors[0].message, expected, "{what}");
    }
    let container_address = container.replacen(
        "container \"/zz/demo/t1/ch1\"",
        "container \"/zz/demo/t1/ch2\"",
        1,
    );
    let errors = lowering_errors(&container_address);
    assert_eq!(errors.len(), 1, "{errors:#?}");
    assert_eq!(errors[0].code, DiagnosticCode::E700);
}

#[test]
fn a_parse_error_is_returned_as_it_is() {
    let parsed = parse("edition 2\nmodule \"/zz/demo/t1/s1\" {\n  unknown\n}\n");
    assert_eq!(lower(&parsed), Err(parsed.diagnostics.clone()));
    let errors = check_source("edition 2\nmodule \"/zz/demo/t1/s1\" {\n  unknown\n}\n")
        .expect_err("an unknown declaration");
    assert_eq!(errors[0].message, "unknown declaration `unknown`");
    let errors = check_source("edition 3\n").expect_err("an unknown edition");
    assert!(
        errors[0].message.starts_with("unknown edition `3`"),
        "{errors:?}"
    );
}
