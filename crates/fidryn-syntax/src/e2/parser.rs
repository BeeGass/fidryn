//! The parser of edition-2 modules: recursive descent over the tokens of the
//! lexer, stopping at the first error.

use super::ast::{
    Alias, BindTarget, Binding, Block, Body, Cell, CellKind, Citation, Container, Credit, Decl,
    Definition, Footnote, Ident, Import, ImportForm, Int, Layers, Level, Module, Note, NoteLayer,
    Num, Part, PositiveLaw, Provision, ProvisionItem, QItem, Quoted, Role, Row, RowGroup, Run,
    Source, Spanned, Spanning, Str, Table, Text, Within, Words,
};
use super::{E2_MAX_SOURCE_BYTES, E2_MAX_TOKENS, MAX_NESTING, Parse};
use crate::edition::unknown_edition;
use crate::lexer::{Token, TokenKind, lex_bounded};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};

/// Parse an edition-2 module. The source begins with the marker `edition 2`.
pub fn parse(source: &str) -> Parse {
    match read(source) {
        Ok(module) => Parse {
            module: Some(module),
            diagnostics: Vec::new(),
        },
        Err(diagnostic) => Parse {
            module: None,
            diagnostics: vec![diagnostic],
        },
    }
}

type Result<T> = std::result::Result<T, Diagnostic>;

const LIMITS: &str = "module exceeds resource limits";

fn read(source: &str) -> Result<Module> {
    let at_start = Span { start: 0, end: 0 };
    if source.len() > E2_MAX_SOURCE_BYTES {
        return Err(error(at_start, LIMITS));
    }
    let mut tokens = lex_bounded(source, E2_MAX_TOKENS).map_err(|()| error(at_start, LIMITS))?;
    tokens.retain(|token| {
        !matches!(
            token.kind,
            TokenKind::Whitespace | TokenKind::Newline | TokenKind::Comment | TokenKind::DocComment
        )
    });
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        depth: 0,
    };
    parser.file()
}

fn error(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::E100, message).with_span(span)
}

fn span(token: Token) -> Span {
    Span {
        start: token.start,
        end: token.end,
    }
}

fn join(first: Span, last: Span) -> Span {
    Span {
        start: first.start,
        end: last.end,
    }
}

/// One place in the fixed order of a block's fields: the words that begin a
/// field there, whether the place must be filled, and whether it may be
/// filled more than once.
struct Slot {
    words: &'static [&'static str],
    required: bool,
    repeated: bool,
}

const fn optional(words: &'static [&'static str]) -> Slot {
    Slot {
        words,
        required: false,
        repeated: false,
    }
}

const fn required(words: &'static [&'static str]) -> Slot {
    Slot {
        words,
        required: true,
        repeated: false,
    }
}

const fn any(words: &'static [&'static str]) -> Slot {
    Slot {
        words,
        required: false,
        repeated: true,
    }
}

const fn some(words: &'static [&'static str]) -> Slot {
    Slot {
        words,
        required: true,
        repeated: true,
    }
}

const BLOCKS: &[&str] = &["p", "subheading", "table", "quoted", "image"];
const PROVISION_ITEMS: &[&str] = &["chapeau", "content", "continuation", "proviso", "provision"];
const QUOTED_ITEMS: &[&str] = &[
    "p",
    "subheading",
    "table",
    "quoted",
    "image",
    "chapeau",
    "content",
    "continuation",
    "proviso",
    "level",
];

const PROVISION: &[Slot] = &[
    optional(&["num"]),
    optional(&["heading"]),
    optional(&["status"]),
    any(PROVISION_ITEMS),
    required(&["hash"]),
];
const CONTAINER: &[Slot] = &[
    optional(&["num"]),
    optional(&["heading"]),
    optional(&["status"]),
    any(&["member"]),
    required(&["hash"]),
];
const NOTE: &[Slot] = &[
    required(&["group"]),
    any(&["topic"]),
    optional(&["heading"]),
    any(BLOCKS),
    required(&["hash"]),
];
const DEFINITION: &[Slot] = &[
    some(&["term"]),
    required(&["kind"]),
    required(&["words"]),
    optional(&["condition"]),
    optional(&["region"]),
    required(&["basis"]),
    optional(&["borrowed_from", "borrowed_words"]),
    optional(&["see_note"]),
];
const LAYERS: &[Slot] = &[
    required(&["operative"]),
    required(&["enacted"]),
    required(&["editorial"]),
];
const BINDING: &[Slot] = &[
    required(&["binds"]),
    required(&["release"]),
    required(&["provision_hash"]),
    required(&["notes_hash"]),
    required(&["pending"]),
];
const BLOCK_LIST: &[Slot] = &[any(BLOCKS)];
const TABLE: &[Slot] = &[any(&["head", "body", "foot"])];
const ROW: &[Slot] = &[any(&["td", "th"])];
const QUOTED: &[Slot] = &[any(QUOTED_ITEMS)];
const LEVEL: &[Slot] = &[
    optional(&["num"]),
    optional(&["heading"]),
    any(QUOTED_ITEMS),
];

/// Where a block stands in the order of its fields.
struct Order<'a> {
    block: &'a str,
    slots: &'static [Slot],
    /// The first place a field may still fill.
    next: usize,
    /// The word that first filled each place.
    seen: Vec<Option<&'static str>>,
}

impl<'a> Order<'a> {
    fn new(block: &'a str, slots: &'static [Slot]) -> Self {
        Self {
            block,
            slots,
            next: 0,
            seen: vec![None; slots.len()],
        }
    }

    fn missing(&self, index: usize, at: Span) -> Diagnostic {
        let name = self.slots[index].words.first().copied().unwrap_or("");
        error(at, format!("missing field `{name}` in `{}`", self.block))
    }

    /// Accept the field that begins with `word`, written at `at`. A place
    /// that takes one of several words takes one of them once.
    fn accept(&mut self, word: &str, at: Span) -> Result<()> {
        let block = self.block;
        let Some((index, known)) = self.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.words
                .iter()
                .find(|known| **known == word)
                .map(|known| (index, *known))
        }) else {
            return Err(error(at, format!("unknown field `{word}` in `{block}`")));
        };
        if index < self.next {
            let message = match self.seen[index] {
                Some(first) if !self.slots[index].repeated && first != word => format!(
                    "field `{word}` is given with `{first}` in `{block}`, which takes one of them"
                ),
                Some(_) if !self.slots[index].repeated => {
                    format!("field `{word}` is given twice in `{block}`")
                }
                _ => format!("field `{word}` is out of order in `{block}`"),
            };
            return Err(error(at, message));
        }
        if let Some(skipped) = (self.next..index)
            .find(|&skipped| self.slots[skipped].required && self.seen[skipped].is_none())
        {
            return Err(self.missing(skipped, at));
        }
        if self.seen[index].is_none() {
            self.seen[index] = Some(known);
        }
        self.next = if self.slots[index].repeated {
            index
        } else {
            index + 1
        };
        Ok(())
    }

    /// The block closes at `at`.
    fn close(&self, at: Span) -> Result<()> {
        match (self.next..self.slots.len())
            .find(|&index| self.slots[index].required && self.seen[index].is_none())
        {
            Some(index) => Err(self.missing(index, at)),
            None => Ok(()),
        }
    }
}

struct Parser<'s> {
    source: &'s str,
    /// The tokens without white space, line ends and comments; the last is
    /// the end of the source.
    tokens: Vec<Token>,
    pos: usize,
    /// The blocks open around the current token.
    depth: usize,
}

impl<'s> Parser<'s> {
    fn peek(&self) -> Token {
        self.tokens
            .get(self.pos)
            .or_else(|| self.tokens.last())
            .copied()
            .unwrap_or(Token {
                kind: TokenKind::Eof,
                start: self.source.len() as u32,
                end: self.source.len() as u32,
            })
    }

    fn bump(&mut self) -> Token {
        let token = self.peek();
        if token.kind != TokenKind::Eof {
            self.pos += 1;
        }
        token
    }

    fn text(&self, token: Token) -> &'s str {
        self.source
            .get(token.start as usize..token.end as usize)
            .unwrap_or("")
    }

    fn is_word(token: Token) -> bool {
        matches!(token.kind, TokenKind::Ident | TokenKind::DurationUnit)
    }

    fn at_word(&self, word: &str) -> bool {
        let token = self.peek();
        Self::is_word(token) && self.text(token) == word
    }

    /// The error for `token` where `what` was expected.
    fn expected(&self, token: Token, what: &str) -> Diagnostic {
        if token.kind == TokenKind::Error {
            error(span(token), format!("invalid token `{}`", self.text(token)))
        } else {
            error(span(token), format!("expected {what}"))
        }
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Result<Token> {
        let token = self.peek();
        if token.kind != kind {
            return Err(self.expected(token, what));
        }
        Ok(self.bump())
    }

    fn string(&mut self) -> Result<Str> {
        let token = self.expect(TokenKind::String, "a string")?;
        match crate::decode_string_literal(self.text(token)) {
            Ok(value) => Ok(Str {
                value,
                span: span(token),
            }),
            Err(why) => Err(error(span(token), format!("invalid JSON string: {why}"))),
        }
    }

    fn ident(&mut self) -> Result<Ident> {
        let token = self.peek();
        if !Self::is_word(token) {
            return Err(self.expected(token, "an identifier"));
        }
        self.bump();
        Ok(Ident {
            text: self.text(token).to_owned(),
            span: span(token),
        })
    }

    fn int(&mut self) -> Result<Int> {
        let token = self.expect(TokenKind::Int, "an integer")?;
        match self.text(token).parse::<u32>() {
            Ok(value) => Ok(Int {
                value,
                span: span(token),
            }),
            Err(_) => Err(error(span(token), "integer literal is out of range")),
        }
    }

    /// One of the closed set `choices`, each a word and its value.
    fn choice<T: Copy>(&mut self, choices: &[(&str, T)], what: &str) -> Result<Spanned<T>> {
        let token = self.peek();
        if Self::is_word(token)
            && let Some((_, value)) = choices.iter().find(|(word, _)| *word == self.text(token))
        {
            self.bump();
            return Ok(Spanned {
                value: *value,
                span: span(token),
            });
        }
        Err(self.expected(token, what))
    }

    /// The `{` that opens a block, counted against the limit of nesting.
    fn open_block(&mut self) -> Result<Token> {
        let token = self.expect(TokenKind::LBrace, "`{`")?;
        self.depth += 1;
        if self.depth > MAX_NESTING {
            return Err(error(span(token), LIMITS));
        }
        Ok(token)
    }

    /// The fields of a block whose `{` is open, up to and with its `}`.
    /// `field` reads one field from its first word, which the order of the
    /// block has accepted. Returns the `}`.
    fn fields<F>(&mut self, block: &str, slots: &'static [Slot], mut field: F) -> Result<Token>
    where
        F: FnMut(&mut Self, Token, &'s str) -> Result<()>,
    {
        let mut order = Order::new(block, slots);
        loop {
            let token = self.peek();
            if token.kind == TokenKind::RBrace {
                order.close(span(token))?;
                self.depth = self.depth.saturating_sub(1);
                return Ok(self.bump());
            }
            if !Self::is_word(token) {
                return Err(self.expected(token, &format!("a field or `}}` in `{block}`")));
            }
            let word = self.text(token);
            order.accept(word, span(token))?;
            field(self, token, word)?;
        }
    }

    /// The error for a word an order accepted that no reader handles. The
    /// tables of fields and the readers name the same words, so this is not
    /// reached.
    fn unhandled(&self, token: Token, block: &str) -> Diagnostic {
        error(
            span(token),
            format!("unknown field `{}` in `{block}`", self.text(token)),
        )
    }

    fn file(&mut self) -> Result<Module> {
        let marker = self.peek();
        if !self.at_word("edition") {
            return Err(self.expected(marker, "`edition 2`"));
        }
        self.bump();
        let edition = self.peek();
        if edition.kind == TokenKind::Eof {
            return Err(unknown_edition(None, span(marker)));
        }
        if edition.kind != TokenKind::Int || self.text(edition) != "2" {
            return Err(unknown_edition(Some(self.text(edition)), span(edition)));
        }
        self.bump();
        let module = self.module()?;
        let after = self.peek();
        if after.kind != TokenKind::Eof {
            return Err(error(span(after), "text after the end of the module"));
        }
        Ok(module)
    }

    fn module(&mut self) -> Result<Module> {
        let start = self.peek();
        if !self.at_word("module") {
            return Err(self.expected(start, "`module`"));
        }
        self.bump();
        let address = self.string()?;
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut decls = Vec::new();
        loop {
            let token = self.peek();
            if token.kind == TokenKind::RBrace {
                self.bump();
                return Ok(Module {
                    address,
                    decls,
                    span: join(span(start), span(token)),
                });
            }
            if !Self::is_word(token) {
                return Err(self.expected(token, "a declaration or `}`"));
            }
            decls.push(self.decl(token)?);
        }
    }

    fn decl(&mut self, token: Token) -> Result<Decl> {
        let start = span(token);
        let decl = match self.text(token) {
            "alias" => {
                self.bump();
                let address = self.string()?;
                Decl::Alias(Alias {
                    span: join(start, address.span),
                    address,
                })
            }
            "within" => {
                self.bump();
                let address = self.string()?;
                Decl::Within(Within {
                    span: join(start, address.span),
                    address,
                })
            }
            "source" => {
                self.bump();
                let name = self.string()?;
                Decl::Source(Source {
                    span: join(start, name.span),
                    name,
                })
            }
            "positive_law" => {
                self.bump();
                let value =
                    self.choice(&[("true", true), ("false", false)], "`true` or `false`")?;
                Decl::PositiveLaw(PositiveLaw {
                    span: join(start, value.span),
                    value,
                })
            }
            "provision" => Decl::Provision(self.provision()?),
            "container" => Decl::Container(self.container()?),
            "credit" => {
                self.bump();
                let text = self.text_runs()?;
                Decl::Credit(Credit {
                    span: join(start, text.span),
                    text,
                })
            }
            "note" => Decl::Note(self.note()?),
            "footnote" => Decl::Footnote(self.footnote()?),
            "definition" => Decl::Definition(self.definition()?),
            "layers" => Decl::Layers(self.layers()?),
            "import" => Decl::Import(self.import()?),
            "citation" => {
                self.bump();
                let name = self.ident()?;
                self.expect(TokenKind::Arrow, "`->`")?;
                let target = self.string()?;
                Decl::Citation(Citation {
                    span: join(start, target.span),
                    name,
                    target,
                })
            }
            "binding" => Decl::Binding(self.binding()?),
            word => return Err(error(start, format!("unknown declaration `{word}`"))),
        };
        Ok(decl)
    }

    /// `Text = Run { Run }`.
    fn text_runs(&mut self) -> Result<Text> {
        let Some(first) = self.run()? else {
            return Err(self.expected(self.peek(), "text"));
        };
        let start = first.span();
        let mut end = start;
        let mut runs = vec![first];
        while let Some(run) = self.run()? {
            end = run.span();
            runs.push(run);
        }
        Ok(Text {
            runs,
            span: join(start, end),
        })
    }

    /// One run, or `None` when the next token begins none.
    fn run(&mut self) -> Result<Option<Run>> {
        let token = self.peek();
        let start = span(token);
        let run = match token.kind {
            TokenKind::String => Run::Words(self.string()?),
            TokenKind::LBracket => {
                self.bump();
                let words = self.string()?;
                self.expect(TokenKind::Arrow, "`->`")?;
                let target = self.string()?;
                let close = self.expect(TokenKind::RBracket, "`]`")?;
                Run::Ref {
                    words,
                    target,
                    span: join(start, span(close)),
                }
            }
            _ if Self::is_word(token) => match self.text(token) {
                "sup" => {
                    self.bump();
                    let words = self.string()?;
                    Run::Sup {
                        span: join(start, words.span),
                        words,
                    }
                }
                "sub" => {
                    self.bump();
                    let words = self.string()?;
                    Run::Sub {
                        span: join(start, words.span),
                        words,
                    }
                }
                "mark" => {
                    self.bump();
                    let shown = self.string()?;
                    let footnote = self.int()?;
                    Run::Mark {
                        span: join(start, footnote.span),
                        shown,
                        footnote,
                    }
                }
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        Ok(Some(run))
    }

    /// `num String Text`, from the word `num`.
    fn num(&mut self) -> Result<Num> {
        let start = span(self.bump());
        let value = self.string()?;
        let shown = self.text_runs()?;
        Ok(Num {
            span: join(start, shown.span),
            value,
            shown,
        })
    }

    /// The word of a field followed by a text: `heading Text`.
    fn field_text(&mut self) -> Result<Text> {
        self.bump();
        self.text_runs()
    }

    /// The word of a field followed by a string: `hash String`.
    fn field_string(&mut self) -> Result<Str> {
        self.bump();
        self.string()
    }

    /// The word of a field followed by a word: `status Ident`.
    fn field_ident(&mut self) -> Result<Ident> {
        self.bump();
        self.ident()
    }

    /// A field the block requires and the order has seen, or the error the
    /// order gives for it.
    fn present<T>(&self, value: Option<T>, close: Token, block: &str, name: &str) -> Result<T> {
        value.ok_or_else(|| error(span(close), format!("missing field `{name}` in `{block}`")))
    }

    fn provision(&mut self) -> Result<Provision> {
        let start = span(self.bump());
        let address = self.string()?;
        let kind = self.ident()?;
        self.open_block()?;
        let (mut num, mut heading, mut status, mut hash) = (None, None, None, None);
        let mut items = Vec::new();
        let close = self.fields("provision", PROVISION, |p, token, word| {
            match word {
                "num" => num = Some(p.num()?),
                "heading" => heading = Some(p.field_text()?),
                "status" => status = Some(p.field_ident()?),
                "hash" => hash = Some(p.field_string()?),
                "provision" => items.push(ProvisionItem::Provision(p.provision()?)),
                "chapeau" | "content" | "continuation" | "proviso" => {
                    items.push(ProvisionItem::Part(p.part()?));
                }
                _ => return Err(p.unhandled(token, "provision")),
            }
            Ok(())
        })?;
        let hash = self.present(hash, close, "provision", "hash")?;
        Ok(Provision {
            address,
            kind,
            num,
            heading,
            status,
            items,
            hash,
            span: join(start, span(close)),
        })
    }

    /// `Role ( Text | { Block { Block } } )`, from the word of the role.
    fn part(&mut self) -> Result<Part> {
        let role = self.choice(
            &[
                ("chapeau", Role::Chapeau),
                ("content", Role::Content),
                ("continuation", Role::Continuation),
                ("proviso", Role::Proviso),
            ],
            "a part",
        )?;
        let block = self.source_at(role.span);
        let body = if self.peek().kind == TokenKind::LBrace {
            let (blocks, braces, close) = self.block_list(block)?;
            if blocks.is_empty() {
                return Err(error(close, "expected a block"));
            }
            Body::Blocks {
                blocks,
                span: braces,
            }
        } else {
            Body::Text(self.text_runs()?)
        };
        Ok(Part {
            span: join(role.span, body.span()),
            role,
            body,
        })
    }

    /// `{ { Block } }`: the blocks, the span from `{` to `}`, and the span of
    /// the `}`.
    fn block_list(&mut self, block: &str) -> Result<(Vec<Block>, Span, Span)> {
        let open = self.open_block()?;
        let mut blocks = Vec::new();
        let close = self.fields(block, BLOCK_LIST, |p, token, _| {
            blocks.push(p.block(token)?);
            Ok(())
        })?;
        Ok((blocks, join(span(open), span(close)), span(close)))
    }

    /// `p Text | subheading Text | Table | Quoted | image String`, from its
    /// word.
    fn block(&mut self, token: Token) -> Result<Block> {
        let start = span(token);
        let block = match self.text(token) {
            "p" => {
                let text = self.field_text()?;
                Block::Para {
                    span: join(start, text.span),
                    text,
                }
            }
            "subheading" => {
                let text = self.field_text()?;
                Block::Subheading {
                    span: join(start, text.span),
                    text,
                }
            }
            "table" => Block::Table(self.table()?),
            "quoted" => Block::Quoted(self.quoted()?),
            "image" => {
                let source = self.field_string()?;
                Block::Image {
                    span: join(start, source.span),
                    source,
                }
            }
            _ => return Err(self.unhandled(token, "a block")),
        };
        Ok(block)
    }

    fn table(&mut self) -> Result<Table> {
        let start = span(self.bump());
        self.open_block()?;
        let mut rows = Vec::new();
        let close = self.fields("table", TABLE, |p, _, _| {
            rows.push(p.row()?);
            Ok(())
        })?;
        Ok(Table {
            rows,
            span: join(start, span(close)),
        })
    }

    fn row(&mut self) -> Result<Row> {
        let group = self.choice(
            &[
                ("head", RowGroup::Head),
                ("body", RowGroup::Body),
                ("foot", RowGroup::Foot),
            ],
            "a row",
        )?;
        let block = self.source_at(group.span);
        self.open_block()?;
        let mut cells = Vec::new();
        let close = self.fields(block, ROW, |p, _, _| {
            cells.push(p.cell()?);
            Ok(())
        })?;
        Ok(Row {
            span: join(group.span, span(close)),
            group,
            cells,
        })
    }

    /// `( td | th ) [ span Int Int ] ( Text | { { Block } } )`.
    fn cell(&mut self) -> Result<Cell> {
        let kind = self.choice(
            &[("td", CellKind::Data), ("th", CellKind::Header)],
            "a cell",
        )?;
        let block = self.source_at(kind.span);
        let spanning = if self.at_word("span") {
            let start = span(self.bump());
            let colspan = self.int()?;
            let rowspan = self.int()?;
            Some(Spanning {
                span: join(start, rowspan.span),
                colspan,
                rowspan,
            })
        } else {
            None
        };
        let body = if self.peek().kind == TokenKind::LBrace {
            let (blocks, braces, _) = self.block_list(block)?;
            Body::Blocks {
                blocks,
                span: braces,
            }
        } else {
            Body::Text(self.text_runs()?)
        };
        Ok(Cell {
            span: join(kind.span, body.span()),
            kind,
            spanning,
            body,
        })
    }

    /// `quoted [ String ] { QItem }`.
    fn quoted(&mut self) -> Result<Quoted> {
        let start = span(self.bump());
        let origin = if self.peek().kind == TokenKind::String {
            Some(self.string()?)
        } else {
            None
        };
        self.open_block()?;
        let mut items = Vec::new();
        let close = self.fields("quoted", QUOTED, |p, token, _| {
            items.push(p.quoted_item(token)?);
            Ok(())
        })?;
        Ok(Quoted {
            origin,
            items,
            span: join(start, span(close)),
        })
    }

    fn quoted_item(&mut self, token: Token) -> Result<QItem> {
        Ok(match self.text(token) {
            "level" => QItem::Level(self.level()?),
            "chapeau" | "content" | "continuation" | "proviso" => QItem::Part(self.part()?),
            _ => QItem::Block(self.block(token)?),
        })
    }

    /// `level Ident { [Num] [heading Text] { QItem } }`.
    fn level(&mut self) -> Result<Level> {
        let start = span(self.bump());
        let kind = self.ident()?;
        self.open_block()?;
        let (mut num, mut heading) = (None, None);
        let mut items = Vec::new();
        let close = self.fields("level", LEVEL, |p, token, word| {
            match word {
                "num" => num = Some(p.num()?),
                "heading" => heading = Some(p.field_text()?),
                _ => items.push(p.quoted_item(token)?),
            }
            Ok(())
        })?;
        Ok(Level {
            kind,
            num,
            heading,
            items,
            span: join(start, span(close)),
        })
    }

    fn container(&mut self) -> Result<Container> {
        let start = span(self.bump());
        let address = self.string()?;
        let kind = self.ident()?;
        self.open_block()?;
        let (mut num, mut heading, mut status, mut hash) = (None, None, None, None);
        let mut members = Vec::new();
        let close = self.fields("container", CONTAINER, |p, token, word| {
            match word {
                "num" => num = Some(p.num()?),
                "heading" => heading = Some(p.field_text()?),
                "status" => status = Some(p.field_ident()?),
                "member" => members.push(p.field_string()?),
                "hash" => hash = Some(p.field_string()?),
                _ => return Err(p.unhandled(token, "container")),
            }
            Ok(())
        })?;
        let hash = self.present(hash, close, "container", "hash")?;
        Ok(Container {
            address,
            kind,
            num,
            heading,
            status,
            members,
            hash,
            span: join(start, span(close)),
        })
    }

    fn note(&mut self) -> Result<Note> {
        let start = span(self.bump());
        let address = self.string()?;
        let layer = self.choice(
            &[
                ("enacted", NoteLayer::Enacted),
                ("editorial", NoteLayer::Editorial),
            ],
            "`enacted` or `editorial`",
        )?;
        self.open_block()?;
        let (mut group, mut heading, mut hash) = (None, None, None);
        let (mut topics, mut blocks) = (Vec::new(), Vec::new());
        let close = self.fields("note", NOTE, |p, token, word| {
            match word {
                "group" => group = Some(p.field_ident()?),
                "topic" => topics.push(p.field_string()?),
                "heading" => heading = Some(p.field_text()?),
                "hash" => hash = Some(p.field_string()?),
                _ => blocks.push(p.block(token)?),
            }
            Ok(())
        })?;
        let group = self.present(group, close, "note", "group")?;
        let hash = self.present(hash, close, "note", "hash")?;
        Ok(Note {
            address,
            layer,
            group,
            topics,
            heading,
            blocks,
            hash,
            span: join(start, span(close)),
        })
    }

    fn footnote(&mut self) -> Result<Footnote> {
        let start = span(self.bump());
        let ordinal = self.int()?;
        let shown = self.string()?;
        let (blocks, braces, _) = self.block_list("footnote")?;
        Ok(Footnote {
            ordinal,
            shown,
            blocks,
            span: join(start, braces),
        })
    }

    fn definition(&mut self) -> Result<Definition> {
        let start = span(self.bump());
        let at = self.string()?;
        self.open_block()?;
        let mut terms = Vec::new();
        let (mut kind, mut words, mut condition, mut region, mut basis) =
            (None, None, None, None, None);
        let (mut borrowed_from, mut borrowed_words, mut see_note) = (None, None, None);
        let close = self.fields("definition", DEFINITION, |p, token, word| {
            match word {
                "term" => terms.push(p.field_string()?),
                "kind" => kind = Some(p.field_ident()?),
                "words" => {
                    p.bump();
                    let value = p.peek();
                    words = Some(if value.kind == TokenKind::String {
                        Words::Stated(p.string()?)
                    } else if p.at_word("unstated") {
                        p.bump();
                        Words::Unstated(span(value))
                    } else {
                        return Err(p.expected(value, "a string or `unstated`"));
                    });
                }
                "condition" => condition = Some(p.field_string()?),
                "region" => region = Some(p.field_string()?),
                "basis" => basis = Some(p.field_ident()?),
                "borrowed_from" => borrowed_from = Some(p.field_string()?),
                "borrowed_words" => borrowed_words = Some(p.field_string()?),
                "see_note" => see_note = Some(p.field_string()?),
                _ => return Err(p.unhandled(token, "definition")),
            }
            Ok(())
        })?;
        Ok(Definition {
            at,
            terms,
            kind: self.present(kind, close, "definition", "kind")?,
            words: self.present(words, close, "definition", "words")?,
            condition,
            region,
            basis: self.present(basis, close, "definition", "basis")?,
            borrowed_from,
            borrowed_words,
            see_note,
            span: join(start, span(close)),
        })
    }

    fn layers(&mut self) -> Result<Layers> {
        let start = span(self.bump());
        self.open_block()?;
        let (mut operative, mut enacted, mut editorial) = (None, None, None);
        let close = self.fields("layers", LAYERS, |p, token, word| {
            match word {
                "operative" => operative = Some(p.field_string()?),
                "enacted" => enacted = Some(p.field_string()?),
                "editorial" => editorial = Some(p.field_string()?),
                _ => return Err(p.unhandled(token, "layers")),
            }
            Ok(())
        })?;
        Ok(Layers {
            operative: self.present(operative, close, "layers", "operative")?,
            enacted: self.present(enacted, close, "layers", "enacted")?,
            editorial: self.present(editorial, close, "layers", "editorial")?,
            span: join(start, span(close)),
        })
    }

    /// `import String ( as Ident | { Ident { , Ident } } )`.
    fn import(&mut self) -> Result<Import> {
        let start = span(self.bump());
        let module = self.string()?;
        let token = self.peek();
        let (form, end) = if self.at_word("as") {
            self.bump();
            let alias = self.ident()?;
            let end = alias.span;
            (ImportForm::As(alias), end)
        } else if token.kind == TokenKind::LBrace {
            self.bump();
            let mut names = vec![self.ident()?];
            loop {
                let token = self.peek();
                match token.kind {
                    TokenKind::Comma => {
                        self.bump();
                        names.push(self.ident()?);
                    }
                    TokenKind::RBrace => {
                        self.bump();
                        break (ImportForm::Names(names), span(token));
                    }
                    _ => return Err(self.expected(token, "`,` or `}`")),
                }
            }
        } else {
            return Err(self.expected(token, "`as` or `{`"));
        };
        Ok(Import {
            module,
            form,
            span: join(start, end),
        })
    }

    fn binding(&mut self) -> Result<Binding> {
        let start = span(self.bump());
        let name = self.ident()?;
        self.open_block()?;
        let (mut binds, mut release, mut pending) = (None, None, None);
        let (mut provision_hash, mut notes_hash) = (None, None);
        let close = self.fields("binding", BINDING, |p, token, word| {
            match word {
                "binds" => {
                    p.bump();
                    binds = Some(p.bind_target()?);
                }
                "release" => release = Some(p.field_string()?),
                "provision_hash" => provision_hash = Some(p.field_string()?),
                "notes_hash" => notes_hash = Some(p.field_string()?),
                "pending" => pending = Some(span(p.bump())),
                _ => return Err(p.unhandled(token, "binding")),
            }
            Ok(())
        })?;
        Ok(Binding {
            name,
            binds: self.present(binds, close, "binding", "binds")?,
            release: self.present(release, close, "binding", "release")?,
            provision_hash: self.present(provision_hash, close, "binding", "provision_hash")?,
            notes_hash: self.present(notes_hash, close, "binding", "notes_hash")?,
            pending: self.present(pending, close, "binding", "pending")?,
            span: join(start, span(close)),
        })
    }

    /// `String | Ident [ . Ident ]`.
    fn bind_target(&mut self) -> Result<BindTarget> {
        let token = self.peek();
        if token.kind == TokenKind::String {
            return Ok(BindTarget::Address(self.string()?));
        }
        if !Self::is_word(token) {
            return Err(self.expected(token, "a string or an identifier"));
        }
        let first = self.ident()?;
        if self.peek().kind != TokenKind::Dot {
            return Ok(BindTarget::Name {
                span: first.span,
                qualifier: None,
                name: first,
            });
        }
        self.bump();
        let name = self.ident()?;
        Ok(BindTarget::Name {
            span: join(first.span, name.span),
            qualifier: Some(first),
            name,
        })
    }

    /// The source text of `span`.
    fn source_at(&self, span: Span) -> &'s str {
        self.source
            .get(span.start as usize..span.end as usize)
            .unwrap_or("")
    }
}
