//! Tests of the edition-2 parser. Every module here holds invented text.

use super::ast::{
    BindTarget, Block, Body, Cell, CellKind, Decl, Definition, ImportForm, Level, Module,
    NoteLayer, Num, Part, Provision, ProvisionItem, QItem, Role, RowGroup, Run, Str, Text, Words,
};
use super::{E2_MAX_SOURCE_BYTES, E2_MAX_TOKENS, MAX_NESTING, parse};
use crate::lexer::{TokenKind, lex};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The example of a unit in the reference of the syntax, with invented text.
const EXAMPLE: &str = r#"edition 2
module "/zz/demo/t1/s8" {
  within "/zz/demo/t1/ch1"
  source "demo01.xml"
  positive_law true
  provision "/zz/demo/t1/s8" section {
    num "8" "§ 8."
    heading "Words that include a widget"
    provision "/zz/demo/t1/s8/a" subsection {
      num "a" "(a)"
      content "As used in " ["section 2 of this title" -> "/zz/demo/t1/s2"] ", the term “widget” means a small device."
      hash "<64 hexadecimal digits>"
    }
    hash "<64 hexadecimal digits>"
  }
  credit "(Added by an invented act.)"
  definition "/zz/demo/t1/s8/a" {
    term "widget"
    kind means
    words "As used in section 2 of this title"
    region "/zz/demo/t1/s2"
    basis enacted
  }
  layers {
    operative "<64 hexadecimal digits>"
    enacted "<64 hexadecimal digits>"
    editorial "<64 hexadecimal digits>"
  }
}
"#;

const HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// A container module with every field a container module can hold. The
/// parser does not judge which declarations a shape may hold, so a definition
/// with every field of its own stands here too.
const CONTAINER: &str = r#"edition 2
// Invented text for a test.
module "/zz/demo/t1/ch1" {
  alias "/zz/demo/t1/chI"
  alias "/zz/demo/t1/ch1old"
  within "/zz/demo/t1"
  source "demo01.xml"
  positive_law false
  container "/zz/demo/t1/ch1" chapter {
    num "1" "CHAPTER 1—" mark "1" 1
    heading "Widgets and gadgets"
    status repealed
    member "/zz/demo/t1/s1"
    member "/zz/demo/t1/s2"
    hash "0000000000000000000000000000000000000000000000000000000000000000"
  }
  credit "(An invented credit.)"
  note "/zz/demo/t1/ch1/note-codification" editorial {
    group editorial_notes
    topic "codification"
    topic "amendments"
    heading "Codification"
    p "An invented note."
    subheading "Further"
    image "figure.eps"
    hash "1111111111111111111111111111111111111111111111111111111111111111"
  }
  footnote 1 "1" {
    p "An invented footnote."
  }
  definition "/zz/demo/t1/s1" {
    term "gadget"
    term "gadgets"
    kind borrowed
    words unstated
    region "/zz/demo/t1"
    basis restated
    borrowed_from "/zz/demo/t2/s5"
    see_note "/zz/demo/t1/ch1/note-codification"
  }
  layers {
    operative "2222222222222222222222222222222222222222222222222222222222222222"
    enacted "3333333333333333333333333333333333333333333333333333333333333333"
    editorial "4444444444444444444444444444444444444444444444444444444444444444"
  }
}
"#;

/// A curated module with every field a curated module can hold.
const CURATED: &str = r#"edition 2
// Invented text for a test.
module "/zz/demo/curated" {
  import "/zz/demo/t1/s8" as widgets
  import "/zz/demo/t1/s9" { gadget, sprocket }
  citation widget_rule -> "/zz/demo/t1/s8/a"
  binding by_address {
    binds "/zz/demo/t1/s8/a"
    release "demo-1"
    provision_hash "5555555555555555555555555555555555555555555555555555555555555555"
    notes_hash "6666666666666666666666666666666666666666666666666666666666666666"
    pending
  }
  binding by_name {
    binds widget_rule
    release "demo-1"
    provision_hash "5555555555555555555555555555555555555555555555555555555555555555"
    notes_hash "6666666666666666666666666666666666666666666666666666666666666666"
    pending
  }
  binding by_qualified_name {
    binds widgets.widget_rule
    release "demo-1"
    provision_hash "5555555555555555555555555555555555555555555555555555555555555555"
    notes_hash "6666666666666666666666666666666666666666666666666666666666666666"
    pending
  }
}
"#;

/// A unit with a table, an image and quoted content.
const TABLES: &str = r#"edition 2
// Invented text for a test.
module "/zz/demo/t1/s9" {
  provision "/zz/demo/t1/s9" section {
    content {
      p "A table follows."
      table {
        head {
          th "Name"
          th "Size"
        }
        body {
          td span 2 1 "A widget"
          td ""
          td {
            p "First"
            subheading "Second"
          }
        }
        foot {
          td {
          }
        }
      }
      image "figure1.eps"
    }
    hash "0000000000000000000000000000000000000000000000000000000000000000"
  }
  note "/zz/demo/t1/s9/note-amendment" enacted {
    group statutory_notes
    quoted "An invented act, sec. 2" {
      level section {
        num "2" "SEC. 2."
        heading "Gadgets"
        level subsection {
          num "a" "(a)"
          content "An invented quoted text."
        }
      }
      p "A paragraph."
    }
    hash "0000000000000000000000000000000000000000000000000000000000000000"
  }
}
"#;

fn parse_ok(source: &str) -> Module {
    let parsed = parse(source);
    assert!(
        parsed.diagnostics.is_empty(),
        "{:?}\n{source}",
        parsed.diagnostics
    );
    let module = parsed.module.expect("a module when there is no diagnostic");
    check_spans(source, &module);
    module
}

fn parse_err(source: &str) -> Diagnostic {
    let parsed = parse(source);
    assert!(parsed.module.is_none(), "{source}");
    assert_eq!(parsed.diagnostics.len(), 1, "{:?}", parsed.diagnostics);
    let diagnostic = parsed.diagnostics.into_iter().next().expect("one");
    assert_eq!(diagnostic.code, DiagnosticCode::E100, "{diagnostic:?}");
    diagnostic
}

/// The text a span covers.
fn text_at(source: &str, span: Span) -> &str {
    &source[span.start as usize..span.end as usize]
}

/// The span of the only occurrence of `needle` in `source`.
fn only(source: &str, needle: &str) -> Span {
    let start = source.find(needle).expect("needle");
    assert_eq!(source.rfind(needle), Some(start), "{needle} occurs twice");
    Span {
        start: start as u32,
        end: (start + needle.len()) as u32,
    }
}

/// A unit around `body`, the lines of its top provision.
fn unit(body: &str) -> String {
    format!(
        "edition 2\nmodule \"/zz/demo/t1/s1\" {{\n  provision \"/zz/demo/t1/s1\" section {{\n{body}\n    hash \"{HASH}\"\n  }}\n}}\n"
    )
}

/// A module around `decls`.
fn module(decls: &str) -> String {
    format!("edition 2\nmodule \"/zz/demo/t1/s1\" {{\n{decls}\n}}\n")
}

fn provisions(module: &Module) -> Vec<&Provision> {
    module
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Provision(provision) => Some(provision),
            _ => None,
        })
        .collect()
}

fn children(provision: &Provision) -> Vec<&Provision> {
    provision
        .items
        .iter()
        .filter_map(|item| match item {
            ProvisionItem::Provision(child) => Some(child),
            ProvisionItem::Part(_) => None,
        })
        .collect()
}

fn parts(provision: &Provision) -> Vec<&Part> {
    provision
        .items
        .iter()
        .filter_map(|item| match item {
            ProvisionItem::Part(part) => Some(part),
            ProvisionItem::Provision(_) => None,
        })
        .collect()
}

fn words(text: &Text) -> Vec<&str> {
    text.runs
        .iter()
        .map(|run| match run {
            Run::Words(words) => words.value.as_str(),
            Run::Ref { words, .. } | Run::Sup { words, .. } | Run::Sub { words, .. } => {
                words.value.as_str()
            }
            Run::Mark { shown, .. } => shown.value.as_str(),
        })
        .collect()
}

fn short_text(part: &Part) -> &Text {
    match &part.body {
        Body::Text(text) => text,
        Body::Blocks { .. } => panic!("a part in the short form"),
    }
}

/// Every span of the tree lies inside the span of its parent and inside the
/// source; every string's span is its token, quotation marks included; every
/// word's span is the word.
fn check_spans(source: &str, module: &Module) {
    struct Walk<'a> {
        source: &'a str,
        stack: Vec<Span>,
    }
    impl Walk<'_> {
        fn enter(&mut self, span: Span, what: &str) {
            assert!(span.start <= span.end, "{what}: {span:?}");
            assert!(span.end as usize <= self.source.len(), "{what}: {span:?}");
            if let Some(parent) = self.stack.last() {
                assert!(
                    parent.start <= span.start && span.end <= parent.end,
                    "{what} {span:?} outside its parent {parent:?}"
                );
            }
            self.stack.push(span);
        }
        fn leave(&mut self) {
            self.stack.pop();
        }
        fn leaf(&mut self, span: Span, what: &str) {
            self.enter(span, what);
            self.leave();
        }
        fn string(&mut self, string: &Str) {
            let token = text_at(self.source, string.span);
            assert!(
                token.starts_with('"') && token.ends_with('"') && token.len() >= 2,
                "{token}"
            );
            assert_eq!(
                crate::decode_string_literal(token).expect("a string token"),
                string.value
            );
            self.leaf(string.span, "string");
        }
        fn ident(&mut self, ident: &super::ast::Ident) {
            assert_eq!(text_at(self.source, ident.span), ident.text);
            self.leaf(ident.span, "word");
        }
        fn int(&mut self, int: &super::ast::Int) {
            assert_eq!(text_at(self.source, int.span).parse::<u32>(), Ok(int.value));
            self.leaf(int.span, "integer");
        }
        fn text(&mut self, text: &Text) {
            assert!(!text.runs.is_empty());
            self.enter(text.span, "text");
            for run in &text.runs {
                self.enter(run.span(), "run");
                match run {
                    Run::Words(words) => self.string(words),
                    Run::Ref { words, target, .. } => {
                        self.string(words);
                        self.string(target);
                    }
                    Run::Sup { words, .. } | Run::Sub { words, .. } => self.string(words),
                    Run::Mark {
                        shown, footnote, ..
                    } => {
                        self.string(shown);
                        self.int(footnote);
                    }
                }
                self.leave();
            }
            self.leave();
        }
        fn num(&mut self, num: &Num) {
            self.enter(num.span, "num");
            self.string(&num.value);
            self.text(&num.shown);
            self.leave();
        }
        fn body(&mut self, body: &Body) {
            match body {
                Body::Text(text) => self.text(text),
                Body::Blocks { blocks, span } => {
                    self.enter(*span, "blocks");
                    assert_eq!(&text_at(self.source, *span)[..1], "{");
                    for block in blocks {
                        self.block(block);
                    }
                    self.leave();
                }
            }
        }
        fn part(&mut self, part: &Part) {
            self.enter(part.span, "part");
            self.leaf(part.role.span, "role");
            self.body(&part.body);
            self.leave();
        }
        fn block(&mut self, block: &Block) {
            self.enter(block.span(), "block");
            match block {
                Block::Para { text, .. } | Block::Subheading { text, .. } => self.text(text),
                Block::Image { source, .. } => self.string(source),
                Block::Table(table) => {
                    for row in &table.rows {
                        self.enter(row.span, "row");
                        self.leaf(row.group.span, "row group");
                        for cell in &row.cells {
                            self.cell(cell);
                        }
                        self.leave();
                    }
                }
                Block::Quoted(quoted) => {
                    if let Some(origin) = &quoted.origin {
                        self.string(origin);
                    }
                    for item in &quoted.items {
                        self.qitem(item);
                    }
                }
            }
            self.leave();
        }
        fn cell(&mut self, cell: &Cell) {
            self.enter(cell.span, "cell");
            self.leaf(cell.kind.span, "cell kind");
            if let Some(spanning) = &cell.spanning {
                self.enter(spanning.span, "span");
                self.int(&spanning.colspan);
                self.int(&spanning.rowspan);
                self.leave();
            }
            self.body(&cell.body);
            self.leave();
        }
        fn qitem(&mut self, item: &QItem) {
            match item {
                QItem::Block(block) => self.block(block),
                QItem::Part(part) => self.part(part),
                QItem::Level(level) => self.level(level),
            }
        }
        fn level(&mut self, level: &Level) {
            self.enter(level.span, "level");
            self.ident(&level.kind);
            if let Some(num) = &level.num {
                self.num(num);
            }
            if let Some(heading) = &level.heading {
                self.text(heading);
            }
            for item in &level.items {
                self.qitem(item);
            }
            self.leave();
        }
        fn provision(&mut self, provision: &Provision) {
            self.enter(provision.span, "provision");
            self.string(&provision.address);
            self.ident(&provision.kind);
            if let Some(num) = &provision.num {
                self.num(num);
            }
            if let Some(heading) = &provision.heading {
                self.text(heading);
            }
            if let Some(status) = &provision.status {
                self.ident(status);
            }
            for item in &provision.items {
                match item {
                    ProvisionItem::Part(part) => self.part(part),
                    ProvisionItem::Provision(child) => self.provision(child),
                }
            }
            self.string(&provision.hash);
            self.leave();
        }
        fn definition(&mut self, definition: &Definition) {
            self.string(&definition.at);
            for term in &definition.terms {
                self.string(term);
            }
            self.ident(&definition.kind);
            match &definition.words {
                Words::Stated(words) => self.string(words),
                Words::Unstated(span) => {
                    assert_eq!(text_at(self.source, *span), "unstated");
                    self.leaf(*span, "unstated");
                }
            }
            for string in [
                &definition.region,
                &definition.borrowed_from,
                &definition.see_note,
            ]
            .into_iter()
            .flatten()
            {
                self.string(string);
            }
            self.ident(&definition.basis);
        }
        fn decl(&mut self, decl: &Decl) {
            self.enter(decl.span(), "declaration");
            match decl {
                Decl::Alias(alias) => self.string(&alias.address),
                Decl::Within(within) => self.string(&within.address),
                Decl::Source(source) => self.string(&source.name),
                Decl::PositiveLaw(positive_law) => {
                    let word = text_at(self.source, positive_law.value.span);
                    assert_eq!(word == "true", positive_law.value.value);
                    self.leaf(positive_law.value.span, "flag");
                }
                Decl::Provision(provision) => {
                    self.leave();
                    self.provision(provision);
                    return;
                }
                Decl::Container(container) => {
                    self.string(&container.address);
                    self.ident(&container.kind);
                    if let Some(num) = &container.num {
                        self.num(num);
                    }
                    if let Some(heading) = &container.heading {
                        self.text(heading);
                    }
                    if let Some(status) = &container.status {
                        self.ident(status);
                    }
                    for member in &container.members {
                        self.string(member);
                    }
                    self.string(&container.hash);
                }
                Decl::Credit(credit) => self.text(&credit.text),
                Decl::Note(note) => {
                    self.string(&note.address);
                    self.leaf(note.layer.span, "layer");
                    self.ident(&note.group);
                    for topic in &note.topics {
                        self.string(topic);
                    }
                    if let Some(heading) = &note.heading {
                        self.text(heading);
                    }
                    for block in &note.blocks {
                        self.block(block);
                    }
                    self.string(&note.hash);
                }
                Decl::Footnote(footnote) => {
                    self.int(&footnote.ordinal);
                    self.string(&footnote.shown);
                    for block in &footnote.blocks {
                        self.block(block);
                    }
                }
                Decl::Definition(definition) => self.definition(definition),
                Decl::Layers(layers) => {
                    self.string(&layers.operative);
                    self.string(&layers.enacted);
                    self.string(&layers.editorial);
                }
                Decl::Import(import) => {
                    self.string(&import.module);
                    match &import.form {
                        ImportForm::As(alias) => self.ident(alias),
                        ImportForm::Names(names) => {
                            for name in names {
                                self.ident(name);
                            }
                        }
                    }
                }
                Decl::Citation(citation) => {
                    self.ident(&citation.name);
                    self.string(&citation.target);
                }
                Decl::Binding(binding) => {
                    self.ident(&binding.name);
                    self.enter(binding.binds.span(), "binds");
                    match &binding.binds {
                        BindTarget::Address(address) => self.string(address),
                        BindTarget::Name {
                            qualifier, name, ..
                        } => {
                            if let Some(qualifier) = qualifier {
                                self.ident(qualifier);
                            }
                            self.ident(name);
                        }
                    }
                    self.leave();
                    self.string(&binding.release);
                    self.string(&binding.provision_hash);
                    self.string(&binding.notes_hash);
                    assert_eq!(text_at(self.source, binding.pending), "pending");
                    self.leaf(binding.pending, "pending");
                }
            }
            self.leave();
        }
    }
    let mut walk = Walk {
        source,
        stack: Vec::new(),
    };
    assert!(text_at(source, module.span).starts_with("module"));
    assert!(text_at(source, module.span).ends_with('}'));
    walk.enter(module.span, "module");
    walk.string(&module.address);
    for decl in &module.decls {
        walk.decl(decl);
    }
    walk.leave();
}

#[test]
fn the_example_of_the_specification_parses() {
    let module = parse_ok(EXAMPLE);
    assert_eq!(module.address.value, "/zz/demo/t1/s8");
    assert_eq!(text_at(EXAMPLE, module.address.span), "\"/zz/demo/t1/s8\"");
    assert_eq!(module.decls.len(), 7);
    assert!(
        matches!(&module.decls[0], Decl::Within(within) if within.address.value == "/zz/demo/t1/ch1")
    );
    assert!(matches!(&module.decls[1], Decl::Source(source) if source.name.value == "demo01.xml"));
    assert!(matches!(&module.decls[2], Decl::PositiveLaw(flag) if flag.value.value));

    let tops = provisions(&module);
    assert_eq!(tops.len(), 1);
    let top = tops[0];
    assert_eq!(top.address.value, "/zz/demo/t1/s8");
    assert_eq!(top.kind.text, "section");
    let num = top.num.as_ref().expect("num");
    assert_eq!(num.value.value, "8");
    assert_eq!(words(&num.shown), ["§ 8."]);
    assert_eq!(
        words(top.heading.as_ref().expect("heading")),
        ["Words that include a widget"]
    );
    assert_eq!(top.status, None);
    assert_eq!(top.hash.value, "<64 hexadecimal digits>");

    let kids = children(top);
    assert_eq!(kids.len(), 1);
    assert_eq!(top.items.len(), 1);
    let child = kids[0];
    assert_eq!(child.address.value, "/zz/demo/t1/s8/a");
    assert_eq!(child.kind.text, "subsection");
    let content = parts(child);
    assert_eq!(content.len(), 1);
    assert_eq!(content[0].role.value, Role::Content);
    assert_eq!(text_at(EXAMPLE, content[0].role.span), "content");
    let runs = &short_text(content[0]).runs;
    assert_eq!(runs.len(), 3);
    assert!(matches!(&runs[0], Run::Words(words) if words.value == "As used in "));
    assert!(matches!(
        &runs[1],
        Run::Ref { words, target, span }
            if words.value == "section 2 of this title"
                && target.value == "/zz/demo/t1/s2"
                && text_at(EXAMPLE, *span) == r#"["section 2 of this title" -> "/zz/demo/t1/s2"]"#
    ));
    assert!(matches!(
        &runs[2],
        Run::Words(words) if words.value == ", the term “widget” means a small device."
    ));

    let credits: Vec<_> = module
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Credit(credit) => Some(credit),
            _ => None,
        })
        .collect();
    assert_eq!(credits.len(), 1);
    assert_eq!(words(&credits[0].text), ["(Added by an invented act.)"]);

    let definitions: Vec<_> = module
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Definition(definition) => Some(definition),
            _ => None,
        })
        .collect();
    assert_eq!(definitions.len(), 1);
    let definition = definitions[0];
    assert_eq!(definition.at.value, "/zz/demo/t1/s8/a");
    assert_eq!(
        definition
            .terms
            .iter()
            .map(|term| term.value.as_str())
            .collect::<Vec<_>>(),
        ["widget"]
    );
    assert_eq!(definition.kind.text, "means");
    assert!(
        matches!(&definition.words, Words::Stated(words) if words.value == "As used in section 2 of this title")
    );
    assert_eq!(
        definition
            .region
            .as_ref()
            .map(|region| region.value.as_str()),
        Some("/zz/demo/t1/s2")
    );
    assert_eq!(definition.basis.text, "enacted");
    assert_eq!(definition.borrowed_from, None);
    assert_eq!(definition.see_note, None);

    let Some(Decl::Layers(layers)) = module.decls.last() else {
        panic!("layers last");
    };
    for value in [&layers.operative, &layers.enacted, &layers.editorial] {
        assert_eq!(value.value, "<64 hexadecimal digits>");
    }
    assert!(text_at(EXAMPLE, layers.span).starts_with("layers {"));
    assert!(text_at(EXAMPLE, layers.span).ends_with('}'));
}

#[test]
fn a_container_and_a_curated_module_parse() {
    let module = parse_ok(CONTAINER);
    assert_eq!(module.address.value, "/zz/demo/t1/ch1");
    let aliases: Vec<_> = module
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Alias(alias) => Some(alias.address.value.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(aliases, ["/zz/demo/t1/chI", "/zz/demo/t1/ch1old"]);
    assert!(
        matches!(&module.decls[2], Decl::Within(within) if within.address.value == "/zz/demo/t1")
    );
    assert!(matches!(&module.decls[3], Decl::Source(source) if source.name.value == "demo01.xml"));
    assert!(matches!(&module.decls[4], Decl::PositiveLaw(flag) if !flag.value.value));
    let Decl::Container(container) = &module.decls[5] else {
        panic!("a container");
    };
    assert_eq!(container.address.value, "/zz/demo/t1/ch1");
    assert_eq!(container.kind.text, "chapter");
    let num = container.num.as_ref().expect("num");
    assert_eq!(num.value.value, "1");
    assert_eq!(words(&num.shown), ["CHAPTER 1—", "1"]);
    assert!(matches!(
        &num.shown.runs[1],
        Run::Mark { footnote, .. } if footnote.value == 1
    ));
    assert_eq!(
        words(container.heading.as_ref().expect("heading")),
        ["Widgets and gadgets"]
    );
    assert_eq!(
        container.status.as_ref().map(|status| status.text.as_str()),
        Some("repealed")
    );
    assert_eq!(
        container
            .members
            .iter()
            .map(|member| member.value.as_str())
            .collect::<Vec<_>>(),
        ["/zz/demo/t1/s1", "/zz/demo/t1/s2"]
    );
    assert_eq!(container.hash.value, HASH);
    assert!(
        matches!(&module.decls[6], Decl::Credit(credit) if words(&credit.text) == ["(An invented credit.)"])
    );
    let Decl::Note(note) = &module.decls[7] else {
        panic!("a note");
    };
    assert_eq!(note.address.value, "/zz/demo/t1/ch1/note-codification");
    assert_eq!(note.layer.value, NoteLayer::Editorial);
    assert_eq!(note.group.text, "editorial_notes");
    assert_eq!(
        note.topics
            .iter()
            .map(|topic| topic.value.as_str())
            .collect::<Vec<_>>(),
        ["codification", "amendments"]
    );
    assert_eq!(
        words(note.heading.as_ref().expect("heading")),
        ["Codification"]
    );
    assert_eq!(note.blocks.len(), 3);
    assert!(
        matches!(&note.blocks[0], Block::Para { text, .. } if words(text) == ["An invented note."])
    );
    assert!(
        matches!(&note.blocks[1], Block::Subheading { text, .. } if words(text) == ["Further"])
    );
    assert!(matches!(&note.blocks[2], Block::Image { source, .. } if source.value == "figure.eps"));
    assert_eq!(note.hash.value, "1".repeat(64));
    let Decl::Footnote(footnote) = &module.decls[8] else {
        panic!("a footnote");
    };
    assert_eq!(footnote.ordinal.value, 1);
    assert_eq!(footnote.shown.value, "1");
    assert_eq!(footnote.blocks.len(), 1);
    let Decl::Definition(definition) = &module.decls[9] else {
        panic!("a definition");
    };
    assert_eq!(
        definition
            .terms
            .iter()
            .map(|term| term.value.as_str())
            .collect::<Vec<_>>(),
        ["gadget", "gadgets"]
    );
    assert_eq!(definition.kind.text, "borrowed");
    assert!(
        matches!(definition.words, Words::Unstated(span) if text_at(CONTAINER, span) == "unstated")
    );
    assert_eq!(
        definition.region.as_ref().map(|value| value.value.as_str()),
        Some("/zz/demo/t1")
    );
    assert_eq!(definition.basis.text, "restated");
    assert_eq!(
        definition
            .borrowed_from
            .as_ref()
            .map(|value| value.value.as_str()),
        Some("/zz/demo/t2/s5")
    );
    assert_eq!(
        definition
            .see_note
            .as_ref()
            .map(|value| value.value.as_str()),
        Some("/zz/demo/t1/ch1/note-codification")
    );
    let Decl::Layers(layers) = &module.decls[10] else {
        panic!("layers");
    };
    assert_eq!(layers.operative.value, "2".repeat(64));
    assert_eq!(layers.enacted.value, "3".repeat(64));
    assert_eq!(layers.editorial.value, "4".repeat(64));
    assert_eq!(module.decls.len(), 11);

    let module = parse_ok(CURATED);
    assert_eq!(module.address.value, "/zz/demo/curated");
    assert_eq!(module.decls.len(), 6);
    let Decl::Import(import) = &module.decls[0] else {
        panic!("an import");
    };
    assert_eq!(import.module.value, "/zz/demo/t1/s8");
    assert!(matches!(&import.form, ImportForm::As(alias) if alias.text == "widgets"));
    let Decl::Import(import) = &module.decls[1] else {
        panic!("an import");
    };
    assert_eq!(import.module.value, "/zz/demo/t1/s9");
    let ImportForm::Names(names) = &import.form else {
        panic!("names");
    };
    assert_eq!(
        names
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["gadget", "sprocket"]
    );
    assert!(text_at(CURATED, import.span).ends_with("sprocket }"));
    let Decl::Citation(citation) = &module.decls[2] else {
        panic!("a citation");
    };
    assert_eq!(citation.name.text, "widget_rule");
    assert_eq!(citation.target.value, "/zz/demo/t1/s8/a");
    let bindings: Vec<_> = module.decls[3..]
        .iter()
        .map(|decl| match decl {
            Decl::Binding(binding) => binding,
            other => panic!("a binding, not {other:?}"),
        })
        .collect();
    assert_eq!(bindings[0].name.text, "by_address");
    assert!(
        matches!(&bindings[0].binds, BindTarget::Address(address) if address.value == "/zz/demo/t1/s8/a")
    );
    assert!(matches!(
        &bindings[1].binds,
        BindTarget::Name { qualifier: None, name, .. } if name.text == "widget_rule"
    ));
    assert!(matches!(
        &bindings[2].binds,
        BindTarget::Name { qualifier: Some(qualifier), name, span }
            if qualifier.text == "widgets"
                && name.text == "widget_rule"
                && text_at(CURATED, *span) == "widgets.widget_rule"
    ));
    for binding in bindings {
        assert_eq!(binding.release.value, "demo-1");
        assert_eq!(binding.provision_hash.value, "5".repeat(64));
        assert_eq!(binding.notes_hash.value, "6".repeat(64));
        assert_eq!(text_at(CURATED, binding.pending), "pending");
        assert!(text_at(CURATED, binding.span).starts_with("binding "));
    }
}

#[test]
fn every_run_form_parses() {
    let source = unit(
        r#"    content "words" ["linked" -> "/zz/demo/t1/s2"] sup "up" sub "down" mark "*" 2"#,
    );
    let module = parse_ok(&source);
    let top = provisions(&module)[0];
    let runs = &short_text(parts(top)[0]).runs;
    assert_eq!(runs.len(), 5);
    assert!(matches!(&runs[0], Run::Words(words) if words.value == "words"));
    assert!(matches!(
        &runs[1],
        Run::Ref { words, target, .. } if words.value == "linked" && target.value == "/zz/demo/t1/s2"
    ));
    assert!(matches!(
        &runs[2],
        Run::Sup { words, span } if words.value == "up" && text_at(&source, *span) == r#"sup "up""#
    ));
    assert!(matches!(
        &runs[3],
        Run::Sub { words, span } if words.value == "down" && text_at(&source, *span) == r#"sub "down""#
    ));
    assert!(matches!(
        &runs[4],
        Run::Mark { shown, footnote, span }
            if shown.value == "*" && footnote.value == 2 && text_at(&source, *span) == r#"mark "*" 2"#
    ));
    // Each role, and a text that spans lines.
    let source = unit(
        "    chapeau \"a\"\n    content \"b\"\n      \"c\"\n    continuation \"d\"\n    proviso \"e\"",
    );
    let module = parse_ok(&source);
    let top = provisions(&module)[0];
    let roles: Vec<_> = parts(top).iter().map(|part| part.role.value).collect();
    assert_eq!(
        roles,
        [
            Role::Chapeau,
            Role::Content,
            Role::Continuation,
            Role::Proviso
        ]
    );
    assert_eq!(words(short_text(parts(top)[1])), ["b", "c"]);
}

#[test]
fn tables_images_and_quoted_content_parse() {
    let module = parse_ok(TABLES);
    let top = provisions(&module)[0];
    let part = parts(top)[0];
    assert_eq!(part.role.value, Role::Content);
    let Body::Blocks { blocks, .. } = &part.body else {
        panic!("a part in the brace form");
    };
    assert_eq!(blocks.len(), 3);
    assert!(matches!(&blocks[0], Block::Para { text, .. } if words(text) == ["A table follows."]));
    let Block::Table(table) = &blocks[1] else {
        panic!("a table");
    };
    assert_eq!(
        table
            .rows
            .iter()
            .map(|row| row.group.value)
            .collect::<Vec<_>>(),
        [RowGroup::Head, RowGroup::Body, RowGroup::Foot]
    );
    let head = &table.rows[0];
    assert_eq!(head.cells.len(), 2);
    for (cell, label) in head.cells.iter().zip(["Name", "Size"]) {
        assert_eq!(cell.kind.value, CellKind::Header);
        assert_eq!(cell.spanning, None);
        assert!(matches!(&cell.body, Body::Text(text) if words(text) == [label]));
    }
    let body = &table.rows[1];
    assert_eq!(body.cells.len(), 3);
    let spanning = body.cells[0].spanning.expect("span");
    assert_eq!((spanning.colspan.value, spanning.rowspan.value), (2, 1));
    assert_eq!(text_at(TABLES, spanning.span), "span 2 1");
    assert_eq!(body.cells[0].kind.value, CellKind::Data);
    assert!(matches!(&body.cells[0].body, Body::Text(text) if words(text) == ["A widget"]));
    // The empty cell is an empty string, kept as written.
    assert!(matches!(&body.cells[1].body, Body::Text(text) if words(text) == [""]));
    let Body::Blocks {
        blocks: cell_blocks,
        span,
    } = &body.cells[2].body
    else {
        panic!("a cell in the brace form");
    };
    assert_eq!(cell_blocks.len(), 2);
    assert!(text_at(TABLES, *span).starts_with('{'));
    assert!(matches!(&cell_blocks[1], Block::Subheading { text, .. } if words(text) == ["Second"]));
    let foot = &table.rows[2];
    assert!(matches!(&foot.cells[0].body, Body::Blocks { blocks, .. } if blocks.is_empty()));
    assert!(
        matches!(&blocks[2], Block::Image { source, span } if source.value == "figure1.eps" && text_at(TABLES, *span) == r#"image "figure1.eps""#)
    );

    let Some(Decl::Note(note)) = module.decls.last() else {
        panic!("a note");
    };
    assert_eq!(note.layer.value, NoteLayer::Enacted);
    let [Block::Quoted(quoted)] = note.blocks.as_slice() else {
        panic!("quoted content");
    };
    assert_eq!(
        quoted.origin.as_ref().map(|origin| origin.value.as_str()),
        Some("An invented act, sec. 2")
    );
    assert_eq!(quoted.items.len(), 2);
    let QItem::Level(section) = &quoted.items[0] else {
        panic!("a level");
    };
    assert_eq!(section.kind.text, "section");
    assert_eq!(
        section.num.as_ref().map(|num| num.value.value.as_str()),
        Some("2")
    );
    assert_eq!(
        words(section.heading.as_ref().expect("heading")),
        ["Gadgets"]
    );
    let [QItem::Level(subsection)] = section.items.as_slice() else {
        panic!("a level two deep");
    };
    assert_eq!(subsection.kind.text, "subsection");
    let [QItem::Part(part)] = subsection.items.as_slice() else {
        panic!("a part");
    };
    assert_eq!(part.role.value, Role::Content);
    assert!(
        matches!(&quoted.items[1], QItem::Block(Block::Para { text, .. }) if words(text) == ["A paragraph."])
    );
}

#[test]
fn an_unknown_field_is_an_error_at_its_word() {
    let cases: Vec<(&str, String)> = vec![
        ("provision", unit("    color \"red\"")),
        (
            "container",
            module(&format!(
                "  container \"/zz/demo/t1\" title {{\n    num \"1\" \"Title 1\"\n    color \"red\"\n    hash \"{HASH}\"\n  }}"
            )),
        ),
        (
            "note",
            module(&format!(
                "  note \"/zz/demo/t1/s1/n\" enacted {{\n    group statutory_notes\n    color \"red\"\n    hash \"{HASH}\"\n  }}"
            )),
        ),
        (
            "table",
            unit("    content {\n      table {\n        color \"red\"\n      }\n    }"),
        ),
        (
            "body",
            unit(
                "    content {\n      table {\n        body {\n          td \"a\"\n          color \"red\"\n        }\n      }\n    }",
            ),
        ),
        (
            "td",
            unit(
                "    content {\n      table {\n        body {\n          td {\n            p \"a\"\n            color \"red\"\n          }\n        }\n      }\n    }",
            ),
        ),
        (
            "quoted",
            unit(
                "    content {\n      quoted {\n        p \"a\"\n        color \"red\"\n      }\n    }",
            ),
        ),
        (
            "level",
            unit(
                "    content {\n      quoted {\n        level clause {\n          heading \"a\"\n          color \"red\"\n        }\n      }\n    }",
            ),
        ),
        (
            "definition",
            module(
                "  definition \"/zz/demo/t1/s1\" {\n    term \"widget\"\n    color \"red\"\n    kind means\n    words unstated\n    basis enacted\n  }",
            ),
        ),
        (
            "layers",
            module(&format!(
                "  layers {{\n    operative \"{HASH}\"\n    color \"red\"\n    enacted \"{HASH}\"\n    editorial \"{HASH}\"\n  }}"
            )),
        ),
        (
            "binding",
            module(&format!(
                "  binding b {{\n    binds \"/zz/demo/t1/s1\"\n    color \"red\"\n    release \"demo-1\"\n    provision_hash \"{HASH}\"\n    notes_hash \"{HASH}\"\n    pending\n  }}"
            )),
        ),
        ("content", unit("    content {\n      color \"red\"\n    }")),
        (
            "footnote",
            module("  footnote 1 \"1\" {\n    color \"red\"\n  }"),
        ),
    ];
    for (block, source) in &cases {
        let error = parse_err(source);
        assert_eq!(
            error.message,
            format!("unknown field `color` in `{block}`"),
            "{source}"
        );
        assert_eq!(error.primary_span, Some(only(source, "color")), "{source}");
    }
    // The module itself holds declarations.
    let source = module("  within \"/zz/demo/t1\"\n  color \"red\"");
    let error = parse_err(&source);
    assert_eq!(error.message, "unknown declaration `color`");
    assert_eq!(error.primary_span, Some(only(&source, "color")));
}

#[test]
fn a_field_out_of_order_or_twice_is_an_error() {
    let source = unit("    heading \"Widgets\"\n    num \"1\" \"(1)\"");
    let error = parse_err(&source);
    assert_eq!(error.message, "field `num` is out of order in `provision`");
    assert_eq!(error.primary_span, Some(only(&source, "num")));

    let source = unit("    heading \"Widgets\"\n    heading \"Gadgets\"");
    let error = parse_err(&source);
    assert_eq!(
        error.message,
        "field `heading` is given twice in `provision`"
    );
    let second = source.rfind("heading").expect("heading") as u32;
    assert_eq!(
        error.primary_span,
        Some(Span {
            start: second,
            end: second + 7
        })
    );

    // A repeated field that comes back after a later field is out of order.
    let source = module(
        "  definition \"/zz/demo/t1/s1\" {\n    term \"widget\"\n    kind means\n    term \"gadget\"\n    words unstated\n    basis enacted\n  }",
    );
    let error = parse_err(&source);
    assert_eq!(
        error.message,
        "field `term` is out of order in `definition`"
    );
    // A text part after the hash.
    let source = unit("").replace(
        &format!("    hash \"{HASH}\"\n"),
        &format!("    hash \"{HASH}\"\n    content \"late\"\n"),
    );
    let error = parse_err(&source);
    assert_eq!(
        error.message,
        "field `content` is out of order in `provision`"
    );
}

#[test]
fn a_missing_required_field_is_an_error() {
    let source = "edition 2\nmodule \"/zz/demo/t1/s1\" {\n  provision \"/zz/demo/t1/s1\" section {\n    content \"a\"\n  }\n}\n";
    let error = parse_err(source);
    assert_eq!(error.message, "missing field `hash` in `provision`");
    let close = source.find("  }\n}").expect("close") as u32 + 2;
    assert_eq!(
        error.primary_span,
        Some(Span {
            start: close,
            end: close + 1
        })
    );

    let source = module(&format!(
        "  note \"/zz/demo/t1/s1/n\" enacted {{\n    topic \"amendments\"\n    hash \"{HASH}\"\n  }}"
    ));
    let error = parse_err(&source);
    assert_eq!(error.message, "missing field `group` in `note`");
    assert_eq!(error.primary_span, Some(only(&source, "topic")));

    let source = module(
        "  definition \"/zz/demo/t1/s1\" {\n    kind means\n    words unstated\n    basis enacted\n  }",
    );
    let error = parse_err(&source);
    assert_eq!(error.message, "missing field `term` in `definition`");
    assert_eq!(error.primary_span, Some(only(&source, "kind")));

    let source = module(&format!(
        "  binding b {{\n    binds \"/zz/demo/t1/s1\"\n    release \"demo-1\"\n    provision_hash \"{HASH}\"\n    notes_hash \"{HASH}\"\n  }}"
    ));
    let error = parse_err(&source);
    assert_eq!(error.message, "missing field `pending` in `binding`");
    let close = source.find("  }\n}").expect("close") as u32 + 2;
    assert_eq!(
        error.primary_span,
        Some(Span {
            start: close,
            end: close + 1
        })
    );
}

#[test]
fn text_needs_a_run() {
    let source = unit("    content }");
    let error = parse_err(&source);
    assert_eq!(error.message, "expected text");
    let close = source.find("content }").expect("content") as u32 + 8;
    assert_eq!(
        error.primary_span,
        Some(Span {
            start: close,
            end: close + 1
        })
    );
    for body in [
        "    heading hash",
        "    content {\n      p\n    }",
        "    num \"1\" status repealed",
    ] {
        let error = parse_err(&unit(body));
        assert_eq!(error.message, "expected text", "{body}");
    }
}

#[test]
fn strings_are_json_strings() {
    let source = unit(r#"    content "\"\\é""#);
    let module = parse_ok(&source);
    let text = short_text(parts(provisions(&module)[0])[0]);
    let [Run::Words(run)] = text.runs.as_slice() else {
        panic!("one run");
    };
    assert_eq!(run.value, "\"\\é");
    assert_eq!(run.value.chars().count(), 3);
    assert_eq!(text_at(&source, run.span), r#""\"\\é""#);

    // The spellings of the canonical form.
    let source = unit(r#"    content "\u0007 \u007f “curly” – \/ \t""#);
    let module = parse_ok(&source);
    let text = short_text(parts(provisions(&module)[0])[0]);
    assert_eq!(words(text), ["\u{7} \u{7f} “curly” – / \t"]);

    for (body, bad) in [
        (r#"    content "\q""#, r#""\q""#),
        ("    content \"a\nb\"", "\"a\nb\""),
        (r#"    content "\ud800""#, r#""\ud800""#),
    ] {
        let source = unit(body);
        let error = parse_err(&source);
        assert!(
            error.message.starts_with("invalid JSON string"),
            "{body}: {}",
            error.message
        );
        assert_eq!(error.primary_span, Some(only(&source, bad)), "{body}");
    }
    // A string that never ends.
    let error = parse_err("edition 2\nmodule \"/zz/demo/t1/s1");
    assert!(error.message.starts_with("invalid JSON string"));
}

#[test]
fn text_after_the_module_is_an_error() {
    let source = format!("{}module \"/zz/demo/t1/s2\" {{\n}}\n", module(""));
    let error = parse_err(&source);
    assert_eq!(error.message, "text after the end of the module");
    let second = source.rfind("module").expect("module") as u32;
    assert_eq!(
        error.primary_span,
        Some(Span {
            start: second,
            end: second + 6
        })
    );
    let error = parse_err(&format!("{}}}", module("")));
    assert_eq!(error.message, "text after the end of the module");
    // Comments and blank lines after the module are no text.
    parse_ok(&format!(
        "{}\n// An invented closing comment.\n\n",
        module("")
    ));
}

#[test]
fn limits_are_enforced() {
    const LIMITS: &str = "module exceeds resource limits";
    let empty = module("");

    // Bytes: the limit itself is read, one byte more is refused before lexing.
    let mut source = empty.clone();
    source.push_str(&" ".repeat(E2_MAX_SOURCE_BYTES - empty.len()));
    assert_eq!(source.len(), E2_MAX_SOURCE_BYTES);
    parse_ok(&source);
    source.push(' ');
    let error = parse_err(&source);
    assert_eq!(error.message, LIMITS);
    assert!(error.primary_span.is_some());
    drop(source);

    // Tokens, white space and line ends included.
    let padded = |count: usize| format!("{empty}{}", "\n".repeat(count));
    let wrapper = lex(&padded(0)).len();
    assert_eq!(lex(&padded(3)).len(), wrapper + 3);
    let largest = E2_MAX_TOKENS - wrapper;
    parse_ok(&padded(largest));
    let error = parse_err(&padded(largest + 1));
    assert_eq!(error.message, LIMITS);

    // Nesting: provisions 64 deep are read, 65 deep are not.
    let nested = |depth: usize| {
        let mut source = String::from("edition 2\nmodule \"/zz/demo/t1/s1\" {\n");
        let mut address = String::from("/zz/demo/t1/s1");
        for level in 0..depth {
            if level > 0 {
                address.push_str("/a");
            }
            source.push_str(&format!("provision \"{address}\" level {{\n"));
        }
        for _ in 0..depth {
            source.push_str(&format!("hash \"{HASH}\"\n}}\n"));
        }
        source.push_str("}\n");
        source
    };
    let module = parse_ok(&nested(MAX_NESTING));
    let mut depth = 0;
    let mut provision = provisions(&module)[0];
    loop {
        depth += 1;
        match children(provision).as_slice() {
            [] => break,
            [child] => provision = child,
            _ => panic!("one child"),
        }
    }
    assert_eq!(depth, MAX_NESTING);
    let source = nested(MAX_NESTING + 1);
    let error = parse_err(&source);
    assert_eq!(error.message, LIMITS);
    // At the brace that opens the sixty-fifth provision.
    let deepest = source
        .match_indices(" level {")
        .nth(MAX_NESTING)
        .expect("the deepest provision")
        .0 as u32
        + 7;
    assert_eq!(
        error.primary_span,
        Some(Span {
            start: deepest,
            end: deepest + 1
        })
    );

    // Every kind of brace counts: tables in cells, quoted content and levels,
    // to the limit and one past it.
    let blocks = |depth: usize| {
        // A top provision and a part in the brace form take two levels; each
        // round of table, row and cell takes three more, and the last cell
        // holds quoted content with levels down to `depth`.
        let mut open = String::new();
        let mut close = String::new();
        let mut level = 2;
        while level + 3 <= depth.saturating_sub(1) {
            open.push_str("table {\nbody {\ntd {\n");
            close.insert_str(0, "}\n}\n}\n");
            level += 3;
        }
        open.push_str("quoted {\n");
        close.insert_str(0, "}\n");
        level += 1;
        while level < depth {
            open.push_str("level clause {\n");
            close.insert_str(0, "}\n");
            level += 1;
        }
        open.push_str("p \"deepest\"\n");
        unit(&format!("content {{\n{open}{close}}}"))
    };
    parse_ok(&blocks(MAX_NESTING));
    let error = parse_err(&blocks(MAX_NESTING + 1));
    assert_eq!(error.message, LIMITS);
}

#[test]
fn a_module_of_the_size_of_the_largest_unit_parses_within_the_limits() {
    // The largest unit measured holds 846,800 characters of text in 1,600
    // provisions and 167 notes, and its small levels nest 8 deep. This one
    // holds twice as much of each.
    const PROVISIONS: usize = 2 * 1_600;
    const CHARACTERS: usize = 2 * 846_800;
    const NOTES: usize = 2 * 167;
    const DEPTH: usize = 8;
    let per_provision = CHARACTERS / PROVISIONS + 1;
    let sentence = "The “widget” of § 8 — an invented sentence of the demo corpus. ";
    let words_of = |length: usize| -> String { sentence.chars().cycle().take(length).collect() };
    let mut source = String::from(
        "edition 2\n// Invented text for a test.\nmodule \"/zz/demo/t1/s1\" {\n  within \"/zz/demo/t1/ch1\"\n",
    );
    let close = |source: &mut String| source.push_str(&format!("hash \"{HASH}\"\n}}\n"));
    // Characters of the parts' words only; numbers, headings and markers come
    // on top.
    let mut characters = 0;
    let mut written = 0;
    // The addresses of the open provisions, the top first.
    let mut open: Vec<String> = Vec::new();
    while written < PROVISIONS {
        // Chains of small levels below the top, each as deep as the deepest.
        if open.len() == DEPTH {
            while open.len() > 1 {
                close(&mut source);
                open.pop();
            }
        }
        let address = match open.last() {
            Some(parent) => format!("{parent}/p{written}"),
            None => String::from("/zz/demo/t1/s1"),
        };
        let text = words_of(per_provision);
        characters += text.chars().count() + "a link".len();
        source.push_str(&format!(
            "provision \"{address}\" level {{\nnum \"{written}\" \"({written})\"\nheading \"H{written}\"\ncontent \"{text}\" [\"a link\" -> \"/zz/demo/t1/s2\"] mark \"1\" 1\n"
        ));
        open.push(address);
        written += 1;
    }
    for _ in 0..open.len() {
        close(&mut source);
    }
    for note in 0..NOTES {
        let text = words_of(80);
        characters += text.chars().count();
        source.push_str(&format!(
            "note \"/zz/demo/t1/s1/n{note}\" enacted {{\ngroup statutory_notes\ntopic \"amendments\"\np \"{text}\"\nhash \"{HASH}\"\n}}\n"
        ));
    }
    source.push_str("}\n");
    assert!(characters >= CHARACTERS, "{characters}");
    assert!(source.len() < E2_MAX_SOURCE_BYTES);

    let module = parse_ok(&source);
    fn count(provision: &Provision) -> usize {
        1 + children(provision).into_iter().map(count).sum::<usize>()
    }
    let tops = provisions(&module);
    assert_eq!(tops.len(), 1);
    assert_eq!(count(tops[0]), PROVISIONS);
    let notes = module
        .decls
        .iter()
        .filter(|decl| matches!(decl, Decl::Note(_)))
        .count();
    assert_eq!(notes, NOTES);
}

#[test]
fn no_input_makes_the_parser_panic() {
    fn holds_its_contract(source: &str) {
        let parsed = catch_unwind(AssertUnwindSafe(|| parse(source)))
            .unwrap_or_else(|_| panic!("the parser panicked on {source:?}"));
        assert!(parsed.diagnostics.len() <= 1, "{source:?}");
        assert_eq!(
            parsed.module.is_some(),
            parsed.diagnostics.is_empty(),
            "{source:?}"
        );
        for diagnostic in &parsed.diagnostics {
            assert_eq!(diagnostic.code, DiagnosticCode::E100);
            let span = diagnostic.primary_span.expect("a span");
            assert!(span.start <= span.end && span.end as usize <= source.len());
            assert!(source.is_char_boundary(span.start as usize));
            assert!(source.is_char_boundary(span.end as usize));
        }
    }
    for source in [EXAMPLE, CONTAINER, CURATED, TABLES] {
        holds_its_contract(source);
        for (index, _) in source.char_indices().skip(1) {
            holds_its_contract(&source[..index]);
        }
        for token in lex(source) {
            if token.kind == TokenKind::Eof {
                continue;
            }
            let removed = format!(
                "{}{}",
                &source[..token.start as usize],
                &source[token.end as usize..]
            );
            holds_its_contract(&removed);
        }
    }
    holds_its_contract("");
}
