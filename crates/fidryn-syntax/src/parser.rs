//! Recursive-descent parser with automatic semicolon insertion and a Pratt expression parser.

use crate::ast::{
    BinOp, ConsequenceAst, Decl, Expr, GoalAst, Header, HeaderKind, Item, JudgmentAst,
    JudgmentAuthorityAst, Module,
};
use crate::lexer::{Token, TokenKind, lex_bounded};
use crate::syntax::{FidrynLanguage, SyntaxKind};
use fidryn_core::Span;
use fidryn_core::{Diagnostic, DiagnosticCode};
use rowan::GreenNodeBuilder;
use std::collections::{BTreeMap, HashMap};

const CONTINUATION: &[&str] = &[
    "and",
    "or",
    "where",
    "then",
    "else",
    "from",
    "decides",
    "occupied_by",
    "establishes",
    "using",
    "for",
    "rank",
    "as",
    "to",
    "version",
    "fuel",
    "in",
];

const SOURCE_FIELDS: &[&str] = &[
    "kind",
    "authority",
    "citation",
    "artifact",
    "digest",
    "effective",
    "retrieved_at",
    "provision_intervals",
    "amendment",
];

const CONSEQUENCE_OPS: &[&str] = &[
    "establish",
    "terminate",
    "suspend",
    "constitute",
    "create",
    "derive",
    "activate",
    "defeat",
    "make",
];

const PREFIX_RBP: u8 = 16;
const MAX_EXPRESSION_DEPTH: usize = 128;
const MAX_EXPRESSION_NODES: usize = 10_000;
const MAX_SOURCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_SOURCE_TOKENS: usize = 262_144;

pub struct Parse {
    pub source: String,
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
    pub green: rowan::GreenNode,
    module: Option<Module>,
}

impl Parse {
    pub fn module(&self) -> Option<&Module> {
        self.module.as_ref()
    }

    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.code.severity() == fidryn_core::Severity::Error)
    }

    pub fn text(&self, token: Token) -> &str {
        self.source
            .get(token.start as usize..token.end as usize)
            .unwrap_or("")
    }

    pub fn syntax(&self) -> rowan::SyntaxNode<FidrynLanguage> {
        rowan::SyntaxNode::new_root(self.green.clone())
    }
}

pub fn parse_file(source: &str) -> Parse {
    if source.len() > MAX_SOURCE_BYTES {
        return refused_parse("source exceeds the byte budget of 8388608");
    }
    let tokens = match lex_bounded(source, MAX_SOURCE_TOKENS) {
        Ok(tokens) => tokens,
        Err(()) => return refused_parse("source exceeds the token budget of 262144"),
    };
    let mut diagnostics = Vec::new();
    let mut previous = TokenKind::Eof;
    for t in &tokens {
        if t.kind == TokenKind::Error {
            let text = source.get(t.start as usize..t.end as usize).unwrap_or("");
            diagnostics.push(
                Diagnostic::new(DiagnosticCode::E100, format!("invalid token `{text}`")).with_span(
                    Span {
                        start: t.start,
                        end: t.end,
                    },
                ),
            );
        }
        if t.kind == TokenKind::String
            && let Err(error) = serde_json::from_str::<String>(
                source.get(t.start as usize..t.end as usize).unwrap_or(""),
            )
        {
            diagnostics.push(
                Diagnostic::new(
                    DiagnosticCode::E100,
                    format!("invalid JSON string: {error}"),
                )
                .with_span(Span {
                    start: t.start,
                    end: t.end,
                }),
            );
        }
        if t.kind == TokenKind::Decimal
            && source
                .get(t.start as usize..t.end as usize)
                .unwrap_or("")
                .parse::<rust_decimal::Decimal>()
                .is_err()
        {
            diagnostics.push(
                Diagnostic::new(DiagnosticCode::E100, "decimal literal is out of range").with_span(
                    Span {
                        start: t.start,
                        end: t.end,
                    },
                ),
            );
        }
        if t.kind == TokenKind::Int {
            let raw = source.get(t.start as usize..t.end as usize).unwrap_or("");
            let largest = if previous == TokenKind::Minus {
                (i64::MAX as u128) + 1
            } else {
                i64::MAX as u128
            };
            if raw.parse::<u128>().map_or(true, |value| value > largest) {
                diagnostics.push(
                    Diagnostic::new(
                        DiagnosticCode::E100,
                        "integer literal is out of range for Int",
                    )
                    .with_span(Span {
                        start: t.start,
                        end: t.end,
                    }),
                );
            }
        }
        if !matches!(
            t.kind,
            TokenKind::Whitespace | TokenKind::Newline | TokenKind::Comment | TokenKind::DocComment
        ) {
            previous = t.kind;
        }
    }
    let mut p = Parser {
        source,
        tokens: &tokens,
        lines: LineIndex::new(&tokens),
        idx: 0,
        diagnostics,
        builder: GreenNodeBuilder::new(),
        node_depth: 0,
        token_cursor: 0,
        expression_depth: 0,
        judgment_body: false,
        brace_opens_enclosing_block: false,
        observation_body: false,
        angle_scans: HashMap::new(),
    };
    let module = p.parse_module();
    let green = p.take_green();
    let diagnostics = p.diagnostics;
    Parse {
        source: source.to_owned(),
        tokens,
        diagnostics,
        green,
        module,
    }
}

fn refused_parse(message: &str) -> Parse {
    let mut builder = GreenNodeBuilder::new();
    builder.start_node(SyntaxKind::MODULE.into());
    builder.finish_node();
    Parse {
        source: String::new(),
        tokens: Vec::new(),
        diagnostics: vec![Diagnostic::new(DiagnosticCode::E100, message)],
        green: builder.finish(),
        module: None,
    }
}

pub fn format_module(source: &str) -> Result<String, Diagnostic> {
    let parsed = parse_file(source);
    if parsed.has_errors() {
        return Err(parsed
            .diagnostics
            .into_iter()
            .next()
            .unwrap_or_else(|| Diagnostic::new(DiagnosticCode::E100, "parse error")));
    }
    Ok(normalize_whitespace(source))
}

fn normalize_whitespace(source: &str) -> String {
    let mut out = String::new();
    for (i, line) in source.lines().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line.trim_end());
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Where the tokens around each position stand, computed once for the whole
/// source.
///
/// The line rule asks, for a newline, which token stands before the run of
/// white space, comments and newlines the newline is in, and which token
/// stands after that run. Walking the run for each of its newlines takes
/// time that grows with the square of its length, so a long run of blank
/// lines took minutes. With this index each question is one lookup.
///
/// Every list has one entry for each token and one for the end of the
/// tokens. `with_tokens_until` hides the tokens from some index on. That
/// changes nothing before the index, and an entry that points at a hidden
/// token is read as "none".
struct LineIndex {
    /// The last token before the index that is no white space, comment or
    /// newline, or `NO_TOKEN`.
    significant_before: Vec<u32>,
    /// The last newline before the index that only white space and comments
    /// separate from it, or `NO_TOKEN`.
    newline_before: Vec<u32>,
    /// The first token at or after the index that is no white space or
    /// comment, or the number of tokens. It may be a newline.
    non_trivia_from: Vec<u32>,
    /// The first token at or after the index that is no white space,
    /// comment or newline, or the number of tokens.
    significant_from: Vec<u32>,
}

/// The entry of a `LineIndex` list that stands for "no such token".
const NO_TOKEN: u32 = u32::MAX;

impl LineIndex {
    fn new(tokens: &[Token]) -> Self {
        let len = tokens.len();
        let mut significant_before = vec![NO_TOKEN; len + 1];
        let mut newline_before = vec![NO_TOKEN; len + 1];
        let mut last_significant = NO_TOKEN;
        let mut last_newline = NO_TOKEN;
        for (index, token) in tokens.iter().enumerate() {
            significant_before[index] = last_significant;
            newline_before[index] = last_newline;
            match token.kind {
                TokenKind::Newline => last_newline = index as u32,
                TokenKind::Whitespace | TokenKind::Comment | TokenKind::DocComment => {}
                _ => {
                    last_significant = index as u32;
                    last_newline = NO_TOKEN;
                }
            }
        }
        significant_before[len] = last_significant;
        newline_before[len] = last_newline;
        let mut non_trivia_from = vec![len as u32; len + 1];
        let mut significant_from = vec![len as u32; len + 1];
        for (index, token) in tokens.iter().enumerate().rev() {
            match token.kind {
                TokenKind::Whitespace | TokenKind::Comment | TokenKind::DocComment => {
                    non_trivia_from[index] = non_trivia_from[index + 1];
                    significant_from[index] = significant_from[index + 1];
                }
                TokenKind::Newline => {
                    non_trivia_from[index] = index as u32;
                    significant_from[index] = significant_from[index + 1];
                }
                _ => {
                    non_trivia_from[index] = index as u32;
                    significant_from[index] = index as u32;
                }
            }
        }
        Self {
            significant_before,
            newline_before,
            non_trivia_from,
            significant_from,
        }
    }
}

struct Parser<'a> {
    source: &'a str,
    tokens: &'a [Token],
    lines: LineIndex,
    idx: usize,
    diagnostics: Vec<Diagnostic>,
    builder: GreenNodeBuilder<'static>,
    node_depth: usize,
    token_cursor: usize,
    expression_depth: usize,
    judgment_body: bool,
    /// True while the parser reads an expression that the block of the
    /// enclosing construct follows: the condition of an `if`, the domain of
    /// a quantifier, the `from` basis of a rule, an expression of a judgment
    /// or observation header. A `{` after such an expression opens that
    /// block, so a name followed by `{` is the name alone and is not applied
    /// to the block.
    brace_opens_enclosing_block: bool,
    observation_body: bool,
    /// What `scan_type_arguments` has found for a `<`, by the number of
    /// tokens the parser could see and the index of the `<` token.
    /// `with_tokens_until` changes what the parser can see, and what a scan
    /// found under one view of the tokens does not hold under another.
    angle_scans: HashMap<(usize, usize), AngleScan>,
}

#[derive(Clone, Copy)]
enum InfixOp {
    Bin(BinOp),
    Word(&'static str),
}

/// What `skip_statement` steps over, which decides where it stops.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Skip {
    /// A statement. It ends by the line rule alone.
    Statement,
    /// The header of a declaration. It also stops before a line that begins
    /// with a declaration keyword.
    Header,
    /// The header of a module-level `for_all` or `exists`. As `Header`,
    /// except that below a line that ends in `:` a line that begins with
    /// `for_all` or `exists` is the body of the quantifier.
    QuantifierHeader,
}

/// What stands where type arguments may begin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypeArgs {
    /// Type arguments, from `<` to the `>` that closes them.
    Read,
    /// Type arguments that a `>` closes and that are nested deeper than the
    /// limit of 128.
    TooDeep,
    /// Something else. A `<` there is a comparison.
    Absent,
}

/// What a scan for type arguments found at one `<`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AngleScan {
    /// No `>` closes what the `<` opens: before one came, the scan met a
    /// token that a list of type arguments does not hold, or the end of the
    /// tokens the parser can see.
    Open,
    /// A `>` closes it. `after` is the index of the token after that `>`,
    /// and `depth` says how deep the arguments nest: 1 when none of them has
    /// arguments of its own.
    Closed { after: usize, depth: usize },
}

/// What a verify block does with a field, by the word the field begins with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VerifyField {
    /// `assert`: the expression after it is an assertion.
    Assert,
    /// `for_all` or `exists`: the expression it begins is an assertion.
    Quantifier,
    /// `bounds`: the numbers of synthetic persons, events and time points.
    Bounds,
    /// `interpretations`, `judgments`, `choices` or `assuming`: recognized
    /// and not read.
    NotRead,
    /// `for`, the first word of a `for every` line: kept as an assertion
    /// that cannot be decided.
    ForEvery,
}

impl VerifyField {
    /// The field a word begins. These nine words are the ones that begin a
    /// field of a verify block.
    fn begun_by(word: &str) -> Option<Self> {
        match word {
            "assert" => Some(Self::Assert),
            "for_all" | "exists" => Some(Self::Quantifier),
            "bounds" => Some(Self::Bounds),
            "interpretations" | "judgments" | "choices" | "assuming" => Some(Self::NotRead),
            "for" => Some(Self::ForEvery),
            _ => None,
        }
    }

    /// True for a field whose expression the block reads as an assertion.
    fn is_read(self) -> bool {
        matches!(self, Self::Assert | Self::Quantifier)
    }
}

impl<'a> Parser<'a> {
    fn peek_raw(&self) -> Token {
        self.tokens
            .get(self.idx)
            .copied()
            .unwrap_or_else(|| self.end_of_input())
    }

    /// The token that stands for the end of the input. It sits at the end of
    /// the last token the parser can see, which is the end of the source
    /// unless `with_tokens_until` has hidden the rest.
    fn end_of_input(&self) -> Token {
        let at = self
            .tokens
            .last()
            .map_or(self.source.len() as u32, |token| token.end);
        Token {
            kind: TokenKind::Eof,
            start: at,
            end: at,
        }
    }

    /// Runs `read` with the tokens from `end` on hidden, so that it sees the
    /// end of the input where the token at `end` stands.
    fn with_tokens_until<R>(&mut self, end: usize, read: impl FnOnce(&mut Self) -> R) -> R {
        let all = self.tokens;
        self.tokens = &all[..end.min(all.len())];
        let result = read(self);
        self.tokens = all;
        result
    }

    fn is_trivia(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Whitespace | TokenKind::Comment | TokenKind::DocComment
        )
    }

    /// The last token before `index` that is no white space, comment or
    /// newline.
    fn significant_before(&self, index: usize) -> Option<Token> {
        let at = *self.lines.significant_before.get(index)?;
        self.tokens.get(at as usize).copied()
    }

    /// The last newline before `index` that only white space and comments
    /// separate from it, as an index.
    fn newline_before(&self, index: usize) -> Option<usize> {
        let at = *self.lines.newline_before.get(index)?;
        (at != NO_TOKEN).then_some(at as usize)
    }

    /// The first token at or after `index` that is no white space or
    /// comment, among the tokens the parser can see. It may be a newline.
    fn non_trivia_from(&self, index: usize) -> Option<Token> {
        let at = *self.lines.non_trivia_from.get(index)?;
        self.tokens.get(at as usize).copied()
    }

    /// The first token at or after `index` that is no white space, comment
    /// or newline, among the tokens the parser can see.
    fn significant_from(&self, index: usize) -> Option<Token> {
        let at = *self.lines.significant_from.get(index)?;
        self.tokens.get(at as usize).copied()
    }

    fn peek(&mut self) -> Token {
        // A loop and not a call of itself for each newline: a run of blank
        // lines can be as long as the token budget.
        loop {
            self.apply_asi();
            let t = self.peek_raw();
            if t.kind != TokenKind::Newline {
                return t;
            }
            self.idx += 1;
        }
    }

    fn apply_asi(&mut self) {
        loop {
            while Self::is_trivia(self.peek_raw().kind) {
                self.idx += 1;
            }
            if self.peek_raw().kind != TokenKind::Newline || self.newline_ends_statement(self.idx) {
                return;
            }
            self.idx += 1;
        }
    }

    /// True when the newline token at `index` ends a statement. It continues
    /// the statement instead when the line after it continues it, or when the
    /// last token before it cannot end a statement or is a prefix word.
    fn newline_ends_statement(&self, index: usize) -> bool {
        let needs_operand = self.significant_before(index).is_some_and(|token| {
            !Self::can_end_statement(token.kind)
                || token.kind == TokenKind::Ident && Self::is_prefix_word(self.text(token))
        });
        !(self.next_line_continues(index) || needs_operand)
    }

    /// True when the line after the newline token at `index` continues the
    /// statement: its first token is a continuation. A blank line and a line
    /// that holds only a comment have no token. Below such lines the first
    /// line that has a token continues the statement only when that token is
    /// one no statement begins with.
    fn next_line_continues(&self, index: usize) -> bool {
        match self.non_trivia_from(index + 1) {
            Some(first) if first.kind == TokenKind::Newline => self
                .significant_from(index + 1)
                .is_some_and(|token| self.only_continues(token)),
            Some(first) => self.is_continuation(first),
            None => false,
        }
    }

    /// True when a statement-ending newline lies between the last consumed
    /// token and the cursor. `peek` steps over that newline, so this is how
    /// the parser still knows the statement ended.
    fn passed_statement_end(&self) -> bool {
        // The newlines in question are the ones of the run of white space,
        // comments and newlines before the cursor. All of them have the
        // same token before the run. All but the last one have a newline
        // after them and the same token after the run, so they answer
        // alike, and the one before the last answers for them.
        let Some(last) = self.newline_before(self.idx) else {
            return false;
        };
        self.newline_ends_statement(last)
            || self
                .newline_before(last)
                .is_some_and(|earlier| self.newline_ends_statement(earlier))
    }

    fn peek_significant(&self) -> Token {
        self.significant_from(self.idx)
            .unwrap_or_else(|| self.end_of_input())
    }

    fn peek_nth(&self, n: usize) -> Token {
        let mut from = self.idx;
        for _ in 0..n {
            match self.lines.significant_from.get(from) {
                Some(&at) if (at as usize) < self.tokens.len() => from = at as usize + 1,
                _ => return self.end_of_input(),
            }
        }
        self.significant_from(from)
            .unwrap_or_else(|| self.end_of_input())
    }

    /// True for a continuation that no statement begins with. Each of the
    /// others can begin a line in its own right: the remaining words of
    /// `CONTINUATION`, such as `then` and `for`, and `{`, `(`, `[` and `!`.
    fn only_continues(&self, tok: Token) -> bool {
        match tok.kind {
            TokenKind::Dot
            | TokenKind::Comma
            | TokenKind::Arrow
            | TokenKind::FatArrow
            | TokenKind::Colon
            | TokenKind::Eq
            | TokenKind::Pipe
            | TokenKind::Lt => true,
            TokenKind::Ident => matches!(self.text(tok), "and" | "or" | "in"),
            _ => false,
        }
    }

    fn is_continuation(&self, tok: Token) -> bool {
        match tok.kind {
            TokenKind::Dot
            | TokenKind::Comma
            | TokenKind::Arrow
            | TokenKind::FatArrow
            | TokenKind::Colon
            | TokenKind::Bang
            | TokenKind::Eq
            | TokenKind::Pipe
            | TokenKind::Lt
            | TokenKind::LBrace
            | TokenKind::LParen
            | TokenKind::LBracket => true,
            TokenKind::Ident => CONTINUATION.contains(&self.text(tok)),
            _ => false,
        }
    }

    fn can_end_statement(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Ident
                | TokenKind::String
                | TokenKind::Int
                | TokenKind::Decimal
                | TokenKind::Date
                | TokenKind::DateTime
                | TokenKind::DurationUnit
                | TokenKind::PlusInf
                | TokenKind::MinusInf
                | TokenKind::RBrace
                | TokenKind::RParen
                | TokenKind::RBracket
                | TokenKind::Star
                | TokenKind::Gt
        )
    }

    fn text(&self, token: Token) -> &'a str {
        self.source
            .get(token.start as usize..token.end as usize)
            .unwrap_or("")
    }

    fn src_slice(&self, start: u32, end: u32) -> String {
        let s = start as usize;
        let e = (end as usize).min(self.source.len());
        if s > e {
            return String::new();
        }
        self.source.get(s..e).unwrap_or("").to_owned()
    }

    fn bump(&mut self) -> Token {
        let t = self.peek();
        if t.kind != TokenKind::Eof {
            self.idx += 1;
            while Self::is_trivia(self.peek_raw().kind) {
                self.idx += 1;
            }
        }
        t
    }

    fn at_ident(&mut self, name: &str) -> bool {
        let t = self.peek();
        t.kind == TokenKind::Ident && self.text(t) == name
    }

    fn at_name(&mut self) -> bool {
        matches!(self.peek().kind, TokenKind::Ident | TokenKind::DurationUnit)
    }

    fn eat_ident(&mut self, name: &str) -> bool {
        if self.at_ident(name) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_kind(&mut self, kind: TokenKind, msg: &str) -> Token {
        let t = self.peek();
        if t.kind == kind {
            self.bump()
        } else {
            self.error(t, msg);
            t
        }
    }

    fn error(&mut self, at: Token, msg: &str) {
        self.diagnostics
            .push(Diagnostic::new(DiagnosticCode::E100, msg).with_span(Span {
                start: at.start,
                end: at.end,
            }));
    }

    fn error_here(&mut self, msg: &str) {
        let t = self.peek();
        self.error(t, msg);
    }

    /// The last token the parser has read that is not white space, a
    /// comment or a line end.
    fn last_read(&self) -> Option<Token> {
        self.significant_before(self.idx.min(self.tokens.len()))
    }

    /// Reports `msg` at the last token read.
    fn error_at_last_read(&mut self, msg: &str) {
        match self.last_read() {
            Some(last) => self.error(last, msg),
            None => self.error_here(msg),
        }
    }

    /// True when the last diagnostic points at the last token read.
    fn last_read_is_reported(&self) -> bool {
        let reported = self
            .diagnostics
            .last()
            .and_then(|diagnostic| diagnostic.primary_span);
        match (self.last_read(), reported) {
            (Some(token), Some(span)) => span.start == token.start && span.end == token.end,
            _ => false,
        }
    }

    /// Reports `msg` for something that is missing where the parser stands.
    /// When the statement has ended there, or a declaration begins on the
    /// line below, the next token belongs to what follows, so the
    /// diagnostic points at the last token read. Otherwise it points at the
    /// next token.
    fn error_where_statement_stops(&mut self, msg: &str) {
        if self.at_stmt_break() || self.declaration_begins_below() {
            self.error_at_last_read(msg);
        } else {
            self.error_here(msg);
        }
    }

    fn eat_semi(&mut self) {
        if self.peek().kind == TokenKind::Semicolon {
            self.bump();
        }
    }

    fn eat_kind(&mut self, kind: TokenKind) -> bool {
        if self.peek().kind == kind {
            self.bump();
            true
        } else {
            false
        }
    }

    /// True when the next token ends the statement: a newline that does not
    /// continue it, `;` or the end of the input.
    fn next_token_ends_statement(&mut self) -> bool {
        self.apply_asi();
        matches!(
            self.peek_raw().kind,
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof
        )
    }

    /// True when the statement has ended: the next token ends it, or a
    /// lookahead has already stepped over the newline that ended it.
    ///
    /// Call this only after a token of the statement has been consumed. At
    /// the start of a statement the cursor has usually passed the newline
    /// that ended the statement before it, so this is true there although
    /// the new statement has not ended. `parse_prefix` can run at the start
    /// of a statement, which is why it asks `next_token_ends_statement`
    /// instead.
    fn at_stmt_break(&mut self) -> bool {
        self.next_token_ends_statement() || self.passed_statement_end()
    }

    /// True when the next token is a keyword that begins a declaration and
    /// stands on a later line than the last token read. The line rule reads
    /// on to that line when the line above it ends in a token that cannot
    /// end a statement, such as `:`. A header that did so would take the
    /// declaration below it for its own text.
    fn declaration_begins_below(&mut self) -> bool {
        let next = self.peek();
        next.kind == TokenKind::Ident
            && is_declaration_keyword(self.text(next))
            && self.line_end_passed()
    }

    /// True when the end of a line lies between the last token read and the
    /// cursor, whether or not it ended a statement.
    fn line_end_passed(&self) -> bool {
        self.newline_before(self.idx.min(self.tokens.len()))
            .is_some()
    }

    /// True when the first token after the newline token at `index` is a
    /// keyword that begins a declaration. Blank lines and comment lines in
    /// between have no token.
    fn declaration_begins_after(&self, index: usize) -> bool {
        self.significant_from(index + 1).is_some_and(|token| {
            token.kind == TokenKind::Ident && is_declaration_keyword(self.text(token))
        })
    }

    fn start_node(&mut self, kind: SyntaxKind) {
        if kind != SyntaxKind::MODULE {
            self.flush_to_idx();
        }
        self.builder.start_node(kind.into());
        self.node_depth += 1;
    }

    fn finish_node(&mut self) {
        self.flush_to_idx();
        if self.node_depth == 0 {
            return;
        }
        self.builder.finish_node();
        self.node_depth -= 1;
    }

    fn wrap_item<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.start_node(SyntaxKind::ITEM);
        let r = f(self);
        self.finish_node();
        r
    }

    fn flush_to_idx(&mut self) {
        let end = self.idx.min(self.tokens.len());
        while self.token_cursor < end {
            self.push_token(self.token_cursor);
            self.token_cursor += 1;
        }
    }

    fn flush_remaining(&mut self) {
        while self.token_cursor < self.tokens.len() {
            self.push_token(self.token_cursor);
            self.token_cursor += 1;
        }
    }

    fn push_token(&mut self, i: usize) {
        let Some(t) = self.tokens.get(i).copied() else {
            return;
        };
        if t.kind == TokenKind::Eof {
            return;
        }
        let text = self
            .source
            .get(t.start as usize..t.end as usize)
            .unwrap_or("");
        let kind = SyntaxKind::from(t.kind);
        match t.kind {
            TokenKind::Whitespace | TokenKind::Newline => {
                self.builder.start_node(SyntaxKind::WHITESPACE.into());
                self.builder.token(kind.into(), text);
                self.builder.finish_node();
            }
            TokenKind::Comment | TokenKind::DocComment => {
                self.builder.start_node(SyntaxKind::COMMENT.into());
                self.builder.token(kind.into(), text);
                self.builder.finish_node();
            }
            TokenKind::Error => {
                self.builder.start_node(SyntaxKind::ERROR.into());
                self.builder.token(kind.into(), text);
                self.builder.finish_node();
            }
            _ => self.builder.token(kind.into(), text),
        }
    }

    fn take_green(&mut self) -> rowan::GreenNode {
        if self.node_depth == 0 {
            self.start_node(SyntaxKind::MODULE);
        }
        let leftover = self.peek();
        if leftover.kind != TokenKind::Eof {
            self.start_node(SyntaxKind::ERROR);
            self.flush_remaining();
            self.finish_node();
        } else {
            self.flush_remaining();
        }
        while self.node_depth > 0 {
            self.builder.finish_node();
            self.node_depth -= 1;
        }
        let builder = std::mem::take(&mut self.builder);
        builder.finish()
    }

    fn parse_type_params_opt(&mut self) -> Vec<String> {
        if self.peek().kind != TokenKind::Lt {
            return Vec::new();
        }
        self.bump();
        let mut params = Vec::new();
        while !matches!(
            self.peek().kind,
            TokenKind::Gt | TokenKind::Eof | TokenKind::LBrace
        ) {
            if self.peek().kind == TokenKind::Comma {
                self.bump();
                continue;
            }
            if self.at_name() {
                params.push(self.parse_ident_name());
            } else {
                self.error_here("expected type parameter");
                break;
            }
            if self.peek().kind == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        self.expect_kind(TokenKind::Gt, "expected `>`");
        params
    }

    fn parse_type_args_opt(&mut self) -> Vec<String> {
        if self.peek().kind != TokenKind::Lt {
            return Vec::new();
        }
        self.bump();
        let mut args = Vec::new();
        while !matches!(
            self.peek().kind,
            TokenKind::Gt | TokenKind::Eof | TokenKind::LBrace
        ) {
            if self.peek().kind == TokenKind::Comma {
                self.bump();
                continue;
            }
            let ty = self.parse_type_string();
            if ty.is_empty() {
                break;
            }
            args.push(ty);
            if self.peek().kind == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        self.expect_kind(TokenKind::Gt, "expected `>`");
        args
    }

    fn parse_module(&mut self) -> Option<Module> {
        self.start_node(SyntaxKind::MODULE);
        if !self.eat_ident("module") {
            let t = self.peek();
            self.error(t, "expected `module`");
            return None;
        }
        let name = self.parse_qname();
        let type_params = self.parse_type_params_opt();
        if !self.eat_ident("version") {
            let t = self.peek();
            self.error(t, "expected `version`");
        }
        let version_tok = self.expect_kind(TokenKind::String, "expected version string");
        let version = trim_quotes(self.text(version_tok)).to_owned();
        self.expect_kind(TokenKind::LBrace, "expected `{`");
        let mut items = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                self.start_node(SyntaxKind::ERROR);
                self.recover_declaration();
                self.finish_node();
            }
        }
        let end = self.expect_kind(TokenKind::RBrace, "expected `}`");
        let leftover = self.peek();
        if leftover.kind != TokenKind::Eof {
            self.error(leftover, "expected end of file after module");
        }
        Some(Module {
            span: Span {
                start: 0,
                end: end.end,
            },
            name,
            version,
            type_params,
            items,
        })
    }

    fn parse_qname(&mut self) -> String {
        let mut name = String::new();
        let t = self.peek();
        if !matches!(
            t.kind,
            TokenKind::Ident | TokenKind::DurationUnit | TokenKind::Error
        ) {
            self.error(t, "expected identifier");
            return name;
        }
        loop {
            let t = self.peek();
            match t.kind {
                TokenKind::Ident | TokenKind::DurationUnit | TokenKind::Error => {
                    if t.kind == TokenKind::Error {
                        self.error(
                            t,
                            "identifiers must be ASCII letters, digits, or underscore",
                        );
                    }
                    if !name.is_empty() {
                        name.push('.');
                    }
                    name.push_str(self.text(t));
                    self.bump();
                }
                _ => {
                    self.error(t, "expected identifier");
                    break;
                }
            }
            if !self.at_stmt_break() && self.peek().kind == TokenKind::Dot {
                self.bump();
                // The line rule reads on after the `.`. A declaration
                // keyword on the line below is not the next part of this
                // name: it begins a declaration of its own.
                if self.declaration_begins_below() {
                    self.error_at_last_read("expected identifier");
                    break;
                }
            } else {
                break;
            }
        }
        name
    }

    fn parse_ident_name(&mut self) -> String {
        let t = self.peek();
        match t.kind {
            TokenKind::Ident | TokenKind::DurationUnit => {
                let s = self.text(t).to_owned();
                self.bump();
                s
            }
            TokenKind::Error => {
                self.error(
                    t,
                    "identifiers must be ASCII letters, digits, or underscore",
                );
                let s = self.text(t).to_owned();
                self.bump();
                s
            }
            _ => {
                self.error(t, "expected identifier");
                String::new()
            }
        }
    }

    fn parse_item(&mut self) -> Option<Item> {
        let t = self.peek();
        if t.kind != TokenKind::Ident {
            self.error(t, "expected declaration");
            return None;
        }
        let kw = self.text(t);
        match kw {
            "jurisdiction" => {
                Some(self.wrap_item(|p| Item::Header(p.parse_header(HeaderKind::Jurisdiction))))
            }
            "source_snapshot" => {
                Some(self.wrap_item(|p| Item::Header(p.parse_header(HeaderKind::SourceSnapshot))))
            }
            "source_manifest" => {
                Some(self.wrap_item(|p| Item::Header(p.parse_header(HeaderKind::SourceManifest))))
            }
            "effective_at" => {
                Some(self.wrap_item(|p| Item::Header(p.parse_header(HeaderKind::EffectiveAt))))
            }
            "recorded_at" => {
                Some(self.wrap_item(|p| Item::Header(p.parse_header(HeaderKind::RecordedAt))))
            }
            "outside_scope" => {
                Some(self.wrap_item(|p| Item::Header(p.parse_header(HeaderKind::OutsideScope))))
            }
            "source" => Some(self.wrap_item(|p| Item::Source(p.parse_source_decl()))),
            "import" => Some(self.wrap_item(|p| Item::Import(p.parse_import_decl()))),
            "entity" => Some(self.wrap_item(|p| Item::Entity(p.parse_decl("entity")))),
            "type" => Some(self.wrap_item(|p| Item::Type(p.parse_type_decl()))),
            "record_type" | "evidence_type" => {
                let kw = kw.to_owned();
                Some(self.wrap_item(|p| Item::RecordType(p.parse_record_decl(&kw))))
            }
            "office" => Some(self.wrap_item(|p| Item::Office(p.parse_decl("office")))),
            "proposition" => {
                Some(self.wrap_item(|p| Item::Proposition(p.parse_decl("proposition"))))
            }
            "observation" => {
                Some(self.wrap_item(|p| Item::Observation(p.parse_observation_decl())))
            }
            "nomination" => Some(self.wrap_item(|p| Item::Nomination(p.parse_decl("nomination")))),
            "fn" | "calc" => {
                let kw = kw.to_owned();
                Some(self.wrap_item(|p| Item::Function(p.parse_function_decl(&kw))))
            }
            "effect" => Some(self.wrap_item(|p| Item::Effect(p.parse_decl("effect")))),
            "for_all" | "exists" => {
                let kw = kw.to_owned();
                Some(self.wrap_item(|p| Item::Verify(p.parse_decl(&kw))))
            }
            "rule" => Some(self.wrap_item(|p| Item::Rule(p.parse_rule_decl()))),
            "power" => Some(self.wrap_item(|p| Item::Power(p.parse_decl("power")))),
            "duty" => Some(self.wrap_item(|p| Item::Duty(p.parse_decl("duty")))),
            "judgment" => Some(self.wrap_item(|p| Item::Judgment(p.parse_judgment_decl()))),
            "decision" => Some(self.wrap_item(|p| Item::Decision(p.parse_decl("decision")))),
            "legal_act" => Some(self.wrap_item(|p| Item::LegalAct(p.parse_decl("legal_act")))),
            "clause" => Some(self.wrap_item(|p| Item::Clause(p.parse_decl("clause")))),
            "interpretation_family" => Some(
                self.wrap_item(|p| Item::Interpretation(p.parse_decl("interpretation_family"))),
            ),
            "conflict_doctrine" => {
                Some(self.wrap_item(|p| Item::ConflictDoctrine(p.parse_decl("conflict_doctrine"))))
            }
            "query" => Some(self.wrap_item(|p| Item::Query(p.parse_query_decl()))),
            "scenario" => Some(self.wrap_item(|p| Item::Scenario(p.parse_decl("scenario")))),
            "verify" => Some(self.wrap_item(|p| Item::Verify(p.parse_verify_decl()))),
            _ => {
                self.error(t, &format!("unknown declaration `{kw}`"));
                None
            }
        }
    }

    fn parse_header(&mut self, kind: HeaderKind) -> Header {
        self.start_node(SyntaxKind::HEADER);
        let start = self.bump();
        let value_start = self.peek().start;
        let value = if self.keyword_stands_alone() {
            // The keyword has no value in its statement. The line below is
            // not read as one: it stays a declaration of its own. That holds
            // for a keyword that takes a string as well: a string does not
            // continue a line.
            let message = format!("expected a value after `{}`", self.text(start));
            self.error_at_last_read(&message);
            self.eat_semi();
            String::new()
        } else if matches!(
            kind,
            HeaderKind::SourceSnapshot | HeaderKind::SourceManifest
        ) {
            let token = self.peek();
            self.expect_kind(TokenKind::String, "expected a JSON string");
            let value = self.src_slice(token.start, token.end);
            if !self.at_stmt_break()
                && !matches!(
                    self.peek().kind,
                    TokenKind::Semicolon | TokenKind::RBrace | TokenKind::Eof
                )
            {
                self.error_here("unexpected value after header string");
                self.skip_rest_of_header();
            }
            self.eat_semi();
            value
        } else {
            self.finish_header();
            self.src_slice(value_start, self.last_end(start.end))
                .trim()
                .trim_end_matches(';')
                .trim_end()
                .to_owned()
        };
        let end = self.last_end(start.end);
        self.finish_node();
        Header {
            span: Span {
                start: start.start,
                end,
            },
            kind,
            value,
        }
    }

    fn parse_decl(&mut self, keyword: &str) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, keyword, "expected a name");
        }
        let mut name = None;
        let t = self.peek();
        if t.kind == TokenKind::Ident {
            name = Some(self.parse_qname());
        } else {
            // What stands after the keyword in its statement is not a name.
            // The rest of the header is read as it stands, so it takes
            // nothing of the declaration below it.
            self.error(t, "expected a name");
        }
        let sig_start = self.peek().start;
        let params = if matches!(
            keyword,
            "proposition" | "decision" | "clause" | "office" | "legal_act" | "duty" | "power"
        ) && self.peek().kind == TokenKind::LParen
        {
            self.parse_paren_params()
        } else {
            Vec::new()
        };
        let names_type = keyword == "entity" && self.eat_kind(TokenKind::Colon)
            || keyword == "decision" && self.eat_kind(TokenKind::Arrow);
        let result_type = if names_type {
            self.parse_header_type()
        } else {
            None
        };
        // A header that stopped short where its type should stand has been
        // reported, and nothing of it is left to step over.
        if !names_type || result_type.is_some() {
            if matches!(keyword, "for_all" | "exists") {
                self.finish_header_as(Skip::QuantifierHeader);
            } else {
                self.finish_header();
            }
        }
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            keyword,
            name,
            Some(self.src_slice(sig_start, end).trim().to_owned()),
            self.src_slice(start.start, end),
        );
        decl.params = params;
        decl.result_type = result_type;
        self.finish_node();
        decl
    }

    fn judgment_expr(&mut self, message: &str) -> Expr {
        self.parse_expr().unwrap_or_else(|| {
            self.error_here(message);
            Expr::Bool(false)
        })
    }

    /// An expression of a judgment or observation header, which the `{` of
    /// the declaration's body follows. With no expression there, `message`
    /// is reported and `stopped_short` is set: the header ends at that
    /// place.
    fn header_expr(&mut self, message: &str, stopped_short: &mut bool) -> Expr {
        self.parse_expr_before_block().unwrap_or_else(|| {
            self.error_where_statement_stops(message);
            *stopped_short = true;
            Expr::Bool(false)
        })
    }

    fn parse_observation_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, "observation", "expected a name");
        }
        let name = Some(self.parse_ident_name());
        let params = if self.peek().kind == TokenKind::LParen {
            self.parse_paren_params()
        } else {
            Vec::new()
        };
        if params.len() != 1 {
            self.error_where_statement_stops(
                "observation requires exactly one typed record binder",
            );
        }
        // With no `establishes`, or with no expression where the header
        // needs one, the header has stopped short: that is reported, and
        // nothing more of the header and no body is read from what follows.
        let mut stopped_short = false;
        let establishes = if self.eat_ident("establishes") {
            self.header_expr("expected observation proposition", &mut stopped_short)
        } else {
            self.error_where_statement_stops("expected `establishes`");
            stopped_short = true;
            Expr::Bool(false)
        };
        let source = if !stopped_short && self.eat_ident("from") {
            Some(self.header_expr("expected observation source", &mut stopped_short))
        } else {
            None
        };
        let mut observation = crate::ast::ObservationAst {
            establishes,
            validators: Vec::new(),
            unsupported_clauses: Vec::new(),
        };
        if !stopped_short && self.peek().kind == TokenKind::LBrace {
            self.bump();
            self.observation_body = true;
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.eat_kind(TokenKind::Semicolon) {
                    continue;
                }
                let clause_start = self.peek().start;
                if self.eat_ident("authenticate") || self.eat_ident("require") {
                    observation
                        .validators
                        .push(self.judgment_expr("expected observation validation condition"));
                } else {
                    self.skip_balanced_until_end();
                    observation.unsupported_clauses.push(
                        self.src_slice(clause_start, self.last_end(clause_start))
                            .to_owned(),
                    );
                }
                self.eat_semi();
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
            self.observation_body = false;
        } else {
            self.header_without_block(stopped_short);
        }
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            "observation",
            name,
            None,
            self.src_slice(start.start, end),
        );
        decl.params = params;
        decl.source_basis = source;
        decl.observation = Some(observation);
        self.finish_node();
        decl
    }

    fn parse_type_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, "type", "expected a name");
        }
        let name = Some(self.parse_ident_name());
        let parameters = self.parse_type_args_opt();
        // The definition is `=` and a type. With either one missing, that
        // is reported once and the declaration ends there: no variant is
        // read from what follows.
        let mut next = if self.peek().kind == TokenKind::Eq {
            self.bump();
            self.parse_defining_type()
        } else {
            self.error_where_statement_stops("expected `=` in type declaration");
            self.skip_rest_of_line();
            None
        };
        let first = next.clone().unwrap_or_default();
        let mut variants = Vec::new();
        while let Some(current) = next.take() {
            let mut arguments = Vec::new();
            let mut fields = Vec::new();
            if !self.at_stmt_break() && self.eat_kind(TokenKind::LParen) {
                while !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
                    arguments.push(self.parse_type_string());
                    if !self.eat_kind(TokenKind::Comma) {
                        break;
                    }
                }
                self.expect_kind(TokenKind::RParen, "expected `)`");
            }
            if self.eat_kind(TokenKind::LBrace) {
                while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                    if self.eat_kind(TokenKind::Comma) || self.eat_kind(TokenKind::Semicolon) {
                        continue;
                    }
                    let field = self.parse_ident_name();
                    if field.is_empty() {
                        // The token begins no field, which has been
                        // reported. It is stepped over, so that the loop
                        // moves on.
                        self.skip_balanced_until_end();
                        continue;
                    }
                    self.expect_kind(TokenKind::Colon, "expected `:`");
                    let ty = self.parse_type_string();
                    if fields.iter().any(|(name, _)| name == &field) {
                        self.error_here("duplicate variant field");
                    }
                    fields.push((field, ty));
                }
                self.expect_kind(TokenKind::RBrace, "expected `}`");
            }
            variants.push(crate::ast::TypeConstructorAst {
                name: current,
                arguments,
                fields,
            });
            if self.eat_kind(TokenKind::Pipe) {
                next = self.parse_defining_type();
            }
        }
        let definition = if variants.len() <= 1
            && variants
                .iter()
                .all(|variant| variant.arguments.is_empty() && variant.fields.is_empty())
        {
            crate::ast::TypeDefinitionAst::Alias(first)
        } else {
            crate::ast::TypeDefinitionAst::Sum(variants)
        };
        self.eat_semi();
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            "type",
            name,
            None,
            self.src_slice(start.start, end),
        );
        decl.type_args = parameters;
        decl.type_definition = Some(definition);
        self.finish_node();
        decl
    }

    fn expect_word(&mut self, word: &str) {
        if !self.eat_ident(word) {
            self.error_here(&format!("expected `{word}`"));
        }
    }

    fn parse_judgment_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, "judgment", "expected a name");
        }
        let name = Some(self.parse_ident_name());
        let sig_start = self.peek().start;
        let params = if self.peek().kind == TokenKind::LParen {
            self.parse_paren_params()
        } else {
            Vec::new()
        };
        // With no `decides`, or with no expression where the header needs
        // one, the header has stopped short: that is reported, and nothing
        // more of the header and no body is read from what follows.
        let mut stopped_short = false;
        let decides = if self.eat_ident("decides") {
            self.header_expr(
                "expected the proposition decided by the judgment",
                &mut stopped_short,
            )
        } else {
            self.error_where_statement_stops("expected `decides`");
            stopped_short = true;
            Expr::Bool(false)
        };
        let source = if !stopped_short && self.eat_ident("from") {
            Some(self.header_expr("expected judgment source", &mut stopped_short))
        } else {
            None
        };
        let mut judgment = JudgmentAst {
            decides,
            authority: None,
            records: Vec::new(),
            record_patterns: Vec::new(),
            constraints: Vec::new(),
            establish_when: None,
            reject_when: None,
            otherwise: None,
            on_established: Vec::new(),
            standard: None,
            review: None,
            unsupported_clauses: Vec::new(),
        };
        if stopped_short || self.peek().kind != TokenKind::LBrace {
            self.header_without_block(stopped_short);
        } else {
            self.bump();
            self.judgment_body = true;
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.eat_kind(TokenKind::Semicolon) {
                    continue;
                }
                let clause_start = self.peek().start;
                if self.eat_ident("authority") {
                    if judgment.authority.is_some() {
                        self.error_here("duplicate judgment authority");
                    }
                    judgment.authority = Some(if self.eat_ident("concurrence") {
                        self.expect_kind(TokenKind::LParen, "expected `(`");
                        let token = self.bump();
                        let minimum = self.text(token).parse::<u32>().unwrap_or(0);
                        if token.kind != TokenKind::Int || minimum == 0 {
                            self.error_here("concurrence requires a positive integer threshold");
                        }
                        let distinct = self.eat_ident("distinct");
                        let issuer_type = self.parse_type_string();
                        self.expect_kind(TokenKind::RParen, "expected `)`");
                        JudgmentAuthorityAst::Concurrence {
                            minimum,
                            distinct,
                            issuer_type,
                        }
                    } else {
                        JudgmentAuthorityAst::Actor(
                            self.judgment_expr("expected judgment authority"),
                        )
                    });
                } else if self.eat_ident("record") {
                    if !self.at_ident("requires") {
                        self.skip_rest_of_statement();
                        judgment.unsupported_clauses.push(
                            self.src_slice(clause_start, self.last_end(clause_start))
                                .to_owned(),
                        );
                        continue;
                    }
                    self.expect_word("requires");
                    if self.peek_nth(1).kind == TokenKind::Colon {
                        let binder = self.parse_ident_name();
                        self.expect_kind(TokenKind::Colon, "expected `:`");
                        let ty = self.parse_type_string();
                        judgment.records.push((binder, ty));
                    } else {
                        judgment
                            .record_patterns
                            .push(self.judgment_expr("expected record requirement"));
                    }
                } else if self.eat_ident("where") {
                    judgment
                        .constraints
                        .push(self.judgment_expr("expected judgment constraint"));
                } else if self.eat_ident("establish") {
                    self.expect_word("when");
                    if self.at_ident("determination") || self.at_ident("established") {
                        self.skip_rest_of_statement();
                        judgment.unsupported_clauses.push(
                            self.src_slice(clause_start, self.last_end(clause_start))
                                .to_owned(),
                        );
                        continue;
                    }
                    let condition = self.judgment_expr("expected establishment condition");
                    judgment.establish_when =
                        Some(or_opt_expr(judgment.establish_when.take(), condition));
                } else if self.eat_ident("reject") {
                    self.expect_word("when");
                    if self.at_ident("determination") || self.at_ident("established") {
                        self.skip_rest_of_statement();
                        judgment.unsupported_clauses.push(
                            self.src_slice(clause_start, self.last_end(clause_start))
                                .to_owned(),
                        );
                        continue;
                    }
                    let condition = self.judgment_expr("expected rejection condition");
                    judgment.reject_when =
                        Some(or_opt_expr(judgment.reject_when.take(), condition));
                } else if self.eat_ident("otherwise") {
                    if !self.at_ident("suspend") || self.text(self.peek_nth(1)) != "NeedJudgment" {
                        self.skip_rest_of_statement();
                        judgment.unsupported_clauses.push(
                            self.src_slice(clause_start, self.last_end(clause_start))
                                .to_owned(),
                        );
                        continue;
                    }
                    self.expect_word("suspend");
                    self.expect_word("NeedJudgment");
                    self.expect_kind(TokenKind::LBrace, "expected `{`");
                    let mut issue = None;
                    let mut protocol = None;
                    while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                        if self.eat_kind(TokenKind::Semicolon) || self.eat_kind(TokenKind::Comma) {
                            continue;
                        }
                        let field = self.parse_ident_name();
                        if field.is_empty() {
                            // The token begins no field, which has been
                            // reported. It is stepped over, so that the loop
                            // moves on.
                            self.skip_balanced_until_end();
                            continue;
                        }
                        self.expect_kind(TokenKind::Colon, "expected `:`");
                        let value = self.judgment_expr("expected request field");
                        match field.as_str() {
                            "issue" if issue.is_none() => issue = Some(value),
                            "protocol" if protocol.is_none() => protocol = Some(value),
                            _ => self.error_here("unknown or duplicate judgment request field"),
                        }
                    }
                    self.expect_kind(TokenKind::RBrace, "expected `}`");
                    judgment.otherwise = match (issue, protocol) {
                        (Some(issue), Some(protocol)) => Some((issue, protocol)),
                        _ => {
                            self.error_here("judgment suspension requires issue and protocol");
                            None
                        }
                    };
                } else if self.eat_ident("on") {
                    if !self.at_ident("established") {
                        self.skip_rest_of_statement();
                        judgment.unsupported_clauses.push(
                            self.src_slice(clause_start, self.last_end(clause_start))
                                .to_owned(),
                        );
                        continue;
                    }
                    self.expect_word("established");
                    self.expect_word("make");
                    let issue = self.parse_bp(PREFIX_RBP).unwrap_or_else(|| {
                        self.error_here("expected the issue made operative");
                        Expr::Bool(false)
                    });
                    self.expect_word("operative");
                    self.expect_word("in");
                    let context = self.judgment_expr("expected operative context");
                    judgment.on_established.push((issue, context));
                } else if self.eat_ident("standard") {
                    judgment.standard = Some(self.judgment_expr("expected judgment standard"));
                } else if self.eat_ident("review") {
                    if !self.at_ident("by") {
                        self.skip_rest_of_statement();
                        judgment.unsupported_clauses.push(
                            self.src_slice(clause_start, self.last_end(clause_start))
                                .to_owned(),
                        );
                        continue;
                    }
                    self.expect_word("by");
                    let by = self.judgment_expr("expected reviewer");
                    self.expect_word("under");
                    let standard = self.judgment_expr("expected review standard");
                    judgment.review = Some((by, standard));
                } else {
                    self.skip_balanced_until_end();
                    judgment.unsupported_clauses.push(
                        self.src_slice(clause_start, self.last_end(clause_start))
                            .to_owned(),
                    );
                }
                self.eat_semi();
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
            self.judgment_body = false;
            if judgment.authority.is_none() {
                self.error_here("judgment requires an authority policy");
            }
        }
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            "judgment",
            name,
            Some(self.src_slice(sig_start, end).trim().to_owned()),
            self.src_slice(start.start, end),
        );
        decl.params = params;
        decl.source_basis = source;
        decl.judgment = Some(judgment);
        self.finish_node();
        decl
    }

    fn parse_import_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, "import", "expected import name");
        }
        let name = if self.at_name() || self.peek().kind == TokenKind::Error {
            Some(self.parse_qname())
        } else {
            self.error_here("expected import name");
            None
        };
        let type_args = self.parse_type_args_opt();
        let sig_start = self.peek().start;
        self.finish_header();
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            "import",
            name,
            Some(self.src_slice(sig_start, end).trim().to_owned()),
            self.src_slice(start.start, end),
        );
        decl.type_args = type_args;
        self.finish_node();
        decl
    }

    fn parse_record_decl(&mut self, keyword: &str) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, keyword, "expected a name");
        }
        let name = Some(self.parse_ident_name());
        let mut fields = BTreeMap::new();
        if self.peek().kind == TokenKind::LBrace {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.eat_kind(TokenKind::Semicolon) || self.eat_kind(TokenKind::Comma) {
                    continue;
                }
                let field = self.parse_ident_name();
                if field.is_empty() {
                    self.skip_balanced_until_end();
                    continue;
                }
                self.expect_kind(TokenKind::Colon, "expected `:` after record field");
                let mut ty = self.parse_type_string();
                while self.eat_kind(TokenKind::Pipe) {
                    ty.push_str(" | ");
                    ty.push_str(&self.parse_type_string());
                }
                if fields.insert(field.clone(), ty).is_some() {
                    self.error_here(&format!("duplicate record field `{field}`"));
                }
                self.eat_semi();
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
        } else {
            self.header_without_block(false);
        }
        let mut decl = self.finish_decl(start, keyword, name);
        decl.fields = fields;
        self.finish_node();
        decl
    }

    /// The header of a declaration whose body is a `{ ... }` block, when no
    /// block stands where the body should begin. Unless the header has
    /// already been reported as stopping short, that is `E100`. What is
    /// left of the header's statement is stepped over, and nothing below
    /// it: the declarations that follow are not read as its clauses or
    /// fields.
    fn header_without_block(&mut self, reported: bool) {
        if !reported {
            self.error_where_statement_stops("expected `{`");
        }
        self.skip_rest_of_header();
    }

    fn finish_decl(&self, start: Token, keyword: &str, name: Option<String>) -> Decl {
        let end = self.last_end(start.end);
        Decl::new(
            Span {
                start: start.start,
                end,
            },
            keyword,
            name,
            None,
            self.src_slice(start.start, end),
        )
    }

    /// True when nothing follows the keyword of a declaration: its statement
    /// has ended, or the block the declaration stands in closes. A name on
    /// the line below is not the name of this declaration.
    fn keyword_stands_alone(&mut self) -> bool {
        self.at_stmt_break() || matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof)
    }

    /// The declaration of a keyword that stands alone: `E100` with
    /// `message` at the keyword, and no name. The line below is not read,
    /// so it stays a declaration of its own.
    fn keyword_alone(&mut self, start: Token, keyword: &str, message: &str) -> Decl {
        self.error_at_last_read(message);
        // A `;` after the keyword ends its statement. Looking for it also
        // steps over the end of the line, so that the text kept for the
        // declaration ends with its line.
        self.eat_semi();
        let decl = self.finish_decl(start, keyword, None);
        self.finish_node();
        decl
    }

    /// The type a header names after `:`, `->`, `=` or `|`. The line rule
    /// reads on to the next line after such a token. When that line begins
    /// a declaration, the header has stopped short: that is `E100` at the
    /// token, and the line is left to the declaration it begins.
    fn parse_header_type(&mut self) -> Option<String> {
        if self.declaration_begins_below() {
            self.error_at_last_read("expected a type");
            return None;
        }
        Some(self.parse_type_string())
    }

    /// The type that defines a `type` declaration or one of its variants.
    /// With none, that has been reported, and what is left of the line is
    /// stepped over: the declaration ends there.
    fn parse_defining_type(&mut self) -> Option<String> {
        let defining = self.parse_header_type().filter(|ty| !ty.is_empty());
        if defining.is_none() {
            self.skip_rest_of_line();
        }
        defining
    }

    /// The end of the text the parser keeps for what it has read: a
    /// declaration, a clause or a value. The text runs to the cursor, but no
    /// further than the end of the line of the last token consumed. So it
    /// holds none of the blank and comment lines a lookahead has stepped
    /// over. A comment at the end of that line is not part of it either: the
    /// text then ends with the token.
    fn last_end(&self, fallback: u32) -> u32 {
        let read = &self.tokens[..self.idx.min(self.tokens.len())];
        let last = match self.lines.significant_before.get(read.len()) {
            Some(&at) if at != NO_TOKEN => at as usize,
            _ => return fallback,
        };
        let after = &read[last + 1..];
        let rest_of_line = match after
            .iter()
            .position(|token| token.kind == TokenKind::Newline)
        {
            Some(line_end) => &after[..=line_end],
            None => after,
        };
        let comment = rest_of_line
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Comment | TokenKind::DocComment));
        match rest_of_line.last() {
            Some(token) if !comment => token.end,
            _ => read[last].end,
        }
    }

    fn parse_type_string(&mut self) -> String {
        let start = self.peek().start;
        if !self.at_name() {
            self.error_here("expected a type");
            return String::new();
        }
        let _ = self.parse_qname();
        if !self.at_stmt_break() && self.peek().kind == TokenKind::Lt {
            let mut depth = 1u32;
            self.bump();
            while depth > 0 {
                // The line rule reads on after `<`. A declaration keyword
                // on a line below is not a type argument: it begins a
                // declaration of its own.
                if self.declaration_begins_below() {
                    break;
                }
                match self.peek().kind {
                    TokenKind::Lt => {
                        depth += 1;
                        if depth > MAX_EXPRESSION_DEPTH as u32 {
                            self.error_here("type nesting exceeds the limit of 128");
                        }
                        self.bump();
                    }
                    TokenKind::Gt => {
                        depth -= 1;
                        self.bump();
                    }
                    TokenKind::Eof
                    | TokenKind::RBrace
                    | TokenKind::Semicolon
                    | TokenKind::Bang
                    | TokenKind::Arrow
                    | TokenKind::LBrace => break,
                    _ => {
                        self.bump();
                    }
                }
            }
            if depth > 0 {
                self.error_where_statement_stops("expected `>`");
            }
        }
        self.src_slice(start, self.last_end(start))
            .trim()
            .to_owned()
    }

    fn parse_paren_params(&mut self) -> Vec<(String, String)> {
        let mut params = Vec::new();
        if self.peek().kind != TokenKind::LParen {
            self.error_here("expected `(`");
            return params;
        }
        self.bump();
        while !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
            if self.peek().kind == TokenKind::Comma {
                self.bump();
                continue;
            }
            if !self.at_name() && self.peek().kind != TokenKind::Error {
                self.error_here("expected parameter");
                break;
            }
            let name = self.parse_ident_name();
            if name.is_empty() {
                if !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
                    self.bump();
                }
                continue;
            }
            self.expect_kind(TokenKind::Colon, "expected `:`");
            let ty = self.parse_type_string();
            params.push((name, ty));
            if self.peek().kind == TokenKind::Comma {
                self.bump();
            }
        }
        self.expect_kind(TokenKind::RParen, "expected `)`");
        params
    }

    fn parse_effect_row_opt(&mut self) -> Vec<String> {
        if self.peek().kind != TokenKind::Bang {
            return Vec::new();
        }
        self.bump();
        self.expect_kind(TokenKind::LBrace, "expected `{`");
        let mut names = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            if self.peek().kind == TokenKind::Comma {
                self.bump();
                continue;
            }
            if self.at_name() {
                names.push(self.parse_ident_name());
            } else {
                self.error_here("expected effect name");
                break;
            }
            if self.peek().kind == TokenKind::Comma {
                self.bump();
            }
        }
        self.expect_kind(TokenKind::RBrace, "expected `}`");
        names
    }

    fn parse_fuel_clause(&mut self) -> Option<u32> {
        if !self.eat_ident("fuel") {
            return None;
        }
        self.expect_kind(TokenKind::Eq, "expected `=`");
        let t = self.peek();
        if t.kind == TokenKind::Int {
            let n = self.text(t).parse::<u32>().ok();
            self.bump();
            n
        } else {
            self.error(t, "expected integer fuel");
            None
        }
    }

    fn parse_source_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, "source", "expected source name");
        }
        let name = if self.at_name() || self.peek().kind == TokenKind::Error {
            Some(self.parse_qname())
        } else {
            self.error_here("expected source name");
            None
        };
        let mut fields = BTreeMap::new();
        if self.peek().kind == TokenKind::LBrace {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if !self.at_name() {
                    self.error_here("expected source field");
                    if self.peek().kind != TokenKind::Eof {
                        self.skip_balanced_until_end();
                    }
                    continue;
                }
                let field_tok = self.peek();
                let field = self.text(field_tok).to_owned();
                if !SOURCE_FIELDS.contains(&field.as_str()) {
                    self.error(field_tok, &format!("unknown SourceField `{field}`"));
                    self.bump();
                    if !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                        self.skip_rest_of_statement();
                    }
                    continue;
                }
                self.bump();
                let value_start = self.peek().start;
                self.skip_rest_of_statement();
                let value_end = self.last_end(value_start);
                fields.insert(
                    field,
                    self.src_slice(value_start, value_end)
                        .trim()
                        .trim_end_matches(';')
                        .trim()
                        .to_owned(),
                );
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
        } else {
            self.finish_header();
        }
        let mut decl = self.finish_decl(start, "source", name);
        decl.fields = fields;
        self.finish_node();
        decl
    }

    fn parse_query_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        let automatic = !self.keyword_stands_alone() && self.eat_ident("automatic");
        if self.keyword_stands_alone() {
            let mut decl = self.keyword_alone(start, "query", "expected query name");
            decl.automatic = automatic;
            return decl;
        }
        let name = if self.at_name() || self.peek().kind == TokenKind::Error {
            Some(self.parse_ident_name())
        } else {
            self.error_here("expected query name");
            None
        };
        let sig_start = self.peek().start;
        let params = if self.peek().kind == TokenKind::LParen {
            self.parse_paren_params()
        } else {
            self.error_here("expected `(`");
            Vec::new()
        };
        let mut result_type = None;
        let mut stopped_short = false;
        if self.peek().kind == TokenKind::Arrow {
            self.bump();
            result_type = self.parse_header_type();
            stopped_short = result_type.is_none();
        }
        let effects = self.parse_effect_row_opt();
        // A header that stopped short where its type should stand has been
        // reported. What follows it is the declaration below, not a block.
        let (expr, goal) = if stopped_short {
            (None, None)
        } else {
            self.parse_query_block()
        };
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            "query",
            name,
            Some(self.src_slice(sig_start, end).trim().to_owned()),
            self.src_slice(start.start, end),
        );
        decl.automatic = automatic;
        decl.result_type = result_type;
        decl.params = params;
        decl.effects = effects;
        decl.expr = expr;
        decl.goal = goal;
        self.finish_node();
        decl
    }

    fn parse_query_block(&mut self) -> (Option<Expr>, Option<GoalAst>) {
        if self.peek().kind != TokenKind::LBrace {
            self.finish_header();
            return (None, None);
        }
        self.bump();
        let mut stmts = Vec::new();
        let mut goal = None;
        let mut returned = false;
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            if returned || goal.is_some() {
                self.error_here("query statement follows its result");
            }
            returned |= self.at_ident("return");
            let first = self.peek();
            let before = self.idx;
            if self.eat_ident("goal") {
                let parsed = self.parse_goal();
                if goal.is_none() {
                    goal = Some(parsed);
                } else {
                    self.error_here("query can declare only one goal");
                }
            } else if let Some(e) = self.parse_eval_stmt() {
                stmts.push(e);
            } else {
                self.unreadable_statement(first, before, "expected query statement");
            }
        }
        self.expect_kind(TokenKind::RBrace, "expected `}`");
        let expr = if let Some(g) = goal.as_mut() {
            if g.kind == "Evaluate" && !stmts.is_empty() {
                g.expr = seq_exprs(extend_expr(stmts, g.expr.take()));
                None
            } else {
                seq_exprs(stmts)
            }
        } else {
            seq_exprs(stmts)
        };
        (expr, goal)
    }

    /// Reports a statement of a block that could not be read, at its first
    /// token, and steps over what is left of it. `before` is the cursor at
    /// that first token. The four blocks of statements call this: a query
    /// body, a `goal Evaluate` block, a function body and a transaction
    /// block. It is called wherever the statement stands, so the last
    /// statement of a block is reported like any other.
    ///
    /// When nothing of the statement was consumed it begins at the cursor,
    /// and one statement is stepped over. Otherwise only the rest of it is.
    /// When it has ended, by its line or by a `;` that was read, nothing
    /// is: the cursor stands at the statement after it.
    fn unreadable_statement(&mut self, first: Token, before: usize, message: &str) {
        self.error(first, message);
        if self.idx == before {
            self.skip_balanced_until_end();
        } else if self
            .last_read()
            .is_none_or(|token| token.kind != TokenKind::Semicolon)
        {
            self.skip_rest_of_statement();
        }
    }

    fn parse_eval_stmt(&mut self) -> Option<Expr> {
        if self.eat_ident("return") {
            let expr = self.parse_expr();
            self.eat_semi();
            return expr;
        }
        if self.eat_ident("require") {
            return self.parse_require_tail();
        }
        if self.at_ident("for_all") || self.at_ident("exists") {
            let expr = self.parse_expr();
            self.eat_semi();
            return expr;
        }
        let expr = self.parse_expr()?;
        self.eat_semi();
        Some(expr)
    }

    fn parse_require_tail(&mut self) -> Option<Expr> {
        let expr = self.parse_expr();
        if expr.is_none() {
            self.error_here("require expects a Boolean expression");
        }
        if self.eat_ident("using") {
            // The protocol name stands in the statement of `using`. When
            // that statement has ended, the word on the line below begins
            // the next statement and is not read as the name.
            if !self.at_stmt_break() && self.at_name() {
                let _ = self.parse_ident_name();
            } else {
                self.error_where_statement_stops("expected a protocol name after `using`");
            }
        }
        self.eat_semi();
        expr.map(|inner| Expr::Require(Box::new(inner)))
    }

    fn parse_goal(&mut self) -> GoalAst {
        let kind = if self.at_name() {
            self.parse_ident_name()
        } else {
            self.error_here("expected query plan");
            String::new()
        };
        let mut goal = GoalAst {
            kind,
            expr: None,
            fields: BTreeMap::new(),
        };
        if self.peek().kind != TokenKind::LBrace {
            self.error_here("expected `{`");
            return goal;
        }
        if goal.kind == "Evaluate" {
            self.bump();
            let mut stmts = Vec::new();
            let mut returned = false;
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if returned {
                    self.error_here("evaluation statement follows `return`");
                }
                returned |= self.at_ident("return");
                let first = self.peek();
                let before = self.idx;
                if let Some(e) = self.parse_eval_stmt() {
                    stmts.push(e);
                } else {
                    self.unreadable_statement(first, before, "expected evaluation statement");
                }
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
            goal.expr = seq_exprs(stmts);
        } else {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.peek().kind == TokenKind::Comma || self.peek().kind == TokenKind::Semicolon
                {
                    self.bump();
                    continue;
                }
                if !self.at_name() {
                    if self.parse_expr().is_none()
                        && !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof)
                    {
                        self.skip_balanced_until_end();
                    }
                    continue;
                }
                let field = self.parse_ident_name();
                if matches!(self.peek().kind, TokenKind::Colon | TokenKind::Eq) {
                    self.bump();
                }
                let value = if self.peek().kind == TokenKind::LBrace {
                    self.parse_braced_expr()
                } else {
                    self.parse_expr().unwrap_or(Expr::Ident(String::new()))
                };
                goal.fields.insert(field, value);
                self.eat_semi();
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
        }
        goal
    }

    fn parse_verify_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, "verify", "expected verification name");
        }
        let name = if self.at_name() || self.peek().kind == TokenKind::Error {
            Some(self.parse_ident_name())
        } else {
            self.error_here("expected verification name");
            None
        };
        let sig_start = self.peek().start;
        let mut fields = BTreeMap::new();
        let mut expr = None;
        let mut assertions = Vec::new();
        if self.peek().kind == TokenKind::LBrace {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.eat_kind(TokenKind::Semicolon) {
                    continue;
                }
                let clause_start = self.peek().start;
                match self.verify_field_ahead() {
                    Some(field @ VerifyField::Assert) => {
                        let (parsed, assertion) =
                            self.until_next_verify_field(field, Self::parse_assert_field);
                        if expr.is_none() {
                            expr = parsed;
                        }
                        assertions.push(assertion);
                        self.eat_semi();
                        let text = self
                            .src_slice(clause_start, self.last_end(clause_start))
                            .trim()
                            .trim_end_matches(';')
                            .trim()
                            .to_owned();
                        if !text.is_empty() && !fields.contains_key("assert") {
                            fields.insert("assert".to_owned(), text);
                        }
                    }
                    Some(field @ VerifyField::Bounds) => {
                        let first = self.idx;
                        if self.peek_nth(1).kind == TokenKind::LBrace {
                            self.bump();
                            let _ = self.parse_braced_expr();
                        } else {
                            self.until_next_verify_field(field, Self::skip_balanced_until_end);
                        }
                        // The field is recorded as the text of its tokens.
                        // The source between them is left out: it holds the
                        // comments inside the field, beside it and below it.
                        fields.insert("bounds".to_owned(), self.token_text_from(first));
                        self.eat_semi();
                    }
                    Some(field @ VerifyField::Quantifier) => {
                        if let Some(e) = self.until_next_verify_field(field, Self::parse_expr) {
                            assertions.push(e);
                        }
                        self.eat_semi();
                    }
                    Some(field @ VerifyField::NotRead) => {
                        self.until_next_verify_field(field, Self::skip_balanced_until_end);
                    }
                    Some(VerifyField::ForEvery) | None => {
                        // A `for every` line, or a line the block does not
                        // recognize. It is not decided, so it has no early
                        // end: what the line rule joins to it stays with it.
                        self.skip_balanced_until_end();
                        assertions.push(self.unsupported_assertion(clause_start));
                    }
                }
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
        } else {
            self.error_where_statement_stops("expected `{` to begin the verify block");
            self.skip_rest_of_header();
        }
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            "verify",
            name,
            Some(self.src_slice(sig_start, end).trim().to_owned()),
            self.src_slice(start.start, end),
        );
        decl.expr = expr;
        decl.assertions = assertions;
        decl.fields = fields;
        self.finish_node();
        decl
    }

    /// The source text the parser has read from `start` on, as an assertion
    /// that `verify` cannot decide, so the property is never proved.
    fn unsupported_assertion(&self, start: u32) -> Expr {
        Expr::Apply {
            callee: Box::new(Expr::Ident("$unsupported_assertion".into())),
            args: vec![Expr::String(self.src_slice(start, self.last_end(start)))],
        }
    }

    /// The text of the tokens from the one at index `first` up to the cursor,
    /// with one space between them and without a `;` that ends them. White
    /// space, comments and line breaks are left out, so the text holds no
    /// comment, wherever one stands among those tokens or after them.
    fn token_text_from(&self, first: usize) -> String {
        let mut tokens: Vec<&Token> = self
            .tokens
            .get(first..self.idx)
            .unwrap_or_default()
            .iter()
            .filter(|token| !Self::is_trivia(token.kind) && token.kind != TokenKind::Newline)
            .collect();
        if tokens
            .last()
            .is_some_and(|token| token.kind == TokenKind::Semicolon)
        {
            tokens.pop();
        }
        let texts: Vec<&str> = tokens.iter().map(|token| self.text(**token)).collect();
        texts.join(" ")
    }

    /// The verify field the next token begins, when that token is one of the
    /// nine words that begin one.
    fn verify_field_ahead(&mut self) -> Option<VerifyField> {
        let t = self.peek();
        if t.kind == TokenKind::Ident {
            VerifyField::begun_by(self.text(t))
        } else {
            None
        }
    }

    /// Reads or steps over the verify field `field`, whose first word is at
    /// the cursor, with `read`. A field ends where the next one begins, as if
    /// a `;` stood there, so `read` sees the end of the input at that place.
    /// A statement would otherwise run on into the next field when its last
    /// token cannot end a statement, when the next field is on the same
    /// line, or when the next field begins with `for`, which continues a
    /// statement.
    fn until_next_verify_field<R>(
        &mut self,
        field: VerifyField,
        read: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let end = self.next_verify_field(field);
        self.with_tokens_until(end, read)
    }

    /// The index of the token where the next field of a verify block begins,
    /// seen from the first word of `field` at the cursor, or the number of
    /// tokens when no field begins before the block ends.
    ///
    /// The word `assert` begins a field wherever it stands. Each other field
    /// word begins one when it begins a line, with one exception: below a
    /// field that is read, a line that begins with `for_all` or `exists` can
    /// be the rest of that field's expression, as the body of a quantifier
    /// is, so it does not end that field. Nothing inside the parentheses,
    /// brackets or braces of the field begins a field. A closing parenthesis
    /// or bracket that closes nothing is stepped over, and a `}` that closes
    /// nothing ends the block.
    fn next_verify_field(&self, field: VerifyField) -> usize {
        let mut depth = 0usize;
        let mut begins_line = false;
        for (index, token) in self.tokens.iter().enumerate().skip(self.idx + 1) {
            if Self::is_trivia(token.kind) {
                continue;
            }
            match token.kind {
                TokenKind::LBrace | TokenKind::LParen | TokenKind::LBracket => depth += 1,
                TokenKind::RBrace if depth == 0 => break,
                TokenKind::RBrace | TokenKind::RParen | TokenKind::RBracket => {
                    depth = depth.saturating_sub(1);
                }
                TokenKind::Ident if depth == 0 => match VerifyField::begun_by(self.text(*token)) {
                    Some(VerifyField::Assert) => return index,
                    Some(next)
                        if begins_line && !(field.is_read() && next == VerifyField::Quantifier) =>
                    {
                        return index;
                    }
                    _ => {}
                },
                _ => {}
            }
            begins_line = token.kind == TokenKind::Newline;
        }
        self.tokens.len()
    }

    /// Reads an `assert` field: the expression it parsed, if any, and the
    /// assertion the field gives. With no expression, or with more text after
    /// the expression that does not begin a field, the whole field is one
    /// assertion that cannot be decided.
    fn parse_assert_field(&mut self) -> (Option<Expr>, Expr) {
        let clause_start = self.peek().start;
        self.bump();
        let parsed = self.parse_expr();
        let ended = self.verify_field_ahead().is_some()
            || matches!(
                self.peek().kind,
                TokenKind::RBrace | TokenKind::Eof | TokenKind::Semicolon
            );
        if !ended {
            // Text that begins with a word, a duration unit included, joins
            // the assertion. Anything else is rejected.
            if !self.at_name() {
                self.error_here("unexpected tokens after verification assertion");
            }
            self.skip_balanced_until_end();
        }
        let assertion = match &parsed {
            Some(expression) if ended => expression.clone(),
            _ => self.unsupported_assertion(clause_start),
        };
        (parsed, assertion)
    }

    fn parse_function_decl(&mut self, keyword: &str) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, keyword, "expected function name");
        }
        let name = if self.at_name() || self.peek().kind == TokenKind::Error {
            Some(self.parse_ident_name())
        } else {
            self.error_here("expected function name");
            None
        };
        let sig_start = self.peek().start;
        let params = if self.peek().kind == TokenKind::LParen {
            self.parse_paren_params()
        } else {
            self.error_here("expected `(`");
            Vec::new()
        };
        let mut result_type = None;
        let mut stopped_short = false;
        if self.peek().kind == TokenKind::Arrow {
            self.bump();
            result_type = self.parse_header_type();
            stopped_short = result_type.is_none();
        }
        let effects = self.parse_effect_row_opt();
        let mut fuel = if self.at_ident("fuel") {
            self.parse_fuel_clause()
        } else {
            None
        };
        // A header that stopped short where its type should stand has been
        // reported. What follows it is the declaration below, not a body.
        let (expr, body_fuel) = if stopped_short {
            (None, None)
        } else {
            self.parse_fn_block()
        };
        if fuel.is_none() {
            fuel = body_fuel;
        }
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            keyword,
            name,
            Some(self.src_slice(sig_start, end).trim().to_owned()),
            self.src_slice(start.start, end),
        );
        decl.result_type = result_type;
        decl.params = params;
        decl.effects = effects;
        decl.fuel = fuel;
        decl.expr = expr;
        self.finish_node();
        decl
    }

    fn parse_fn_block(&mut self) -> (Option<Expr>, Option<u32>) {
        if self.peek().kind != TokenKind::LBrace {
            self.error_where_statement_stops("expected `{` to begin the function body");
            self.skip_rest_of_header();
            return (None, None);
        }
        self.bump();
        let mut stmts = Vec::new();
        let mut fuel = None;
        let mut returned = false;
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            if returned && !self.at_ident("else") {
                self.error_here("function statement follows `return`");
            }
            let first = self.peek();
            let before = self.idx;
            if self.at_ident("fuel") {
                fuel = self.parse_fuel_clause();
                self.eat_semi();
            } else if self.at_ident("else") {
                // `else` is an expr stop-word, so Pratt will not start here.
                // skip_balanced would swallow the else-branch as leftover.
                self.take_fn_block_else(&mut stmts);
            } else {
                returned |= self.at_ident("return");
                match self.parse_eval_stmt() {
                    Some(expr) => stmts.push(expr),
                    None => {
                        self.unreadable_statement(first, before, "expected function statement");
                    }
                }
            }
        }
        self.expect_kind(TokenKind::RBrace, "expected `}`");
        let expr = seq_exprs(stmts);
        (expr, fuel)
    }

    fn take_fn_block_else(&mut self, stmts: &mut [Expr]) {
        let can_attach = matches!(stmts.last(), Some(Expr::If { else_: None, .. }));
        if !can_attach {
            self.error_here("unexpected `else`");
        }
        let Some(else_expr) = self.parse_else_branch() else {
            return;
        };
        if let Some(Expr::If { else_, .. }) = stmts.last_mut()
            && else_.is_none()
        {
            *else_ = Some(Box::new(else_expr));
        }
        self.eat_semi();
    }

    fn parse_rule_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        if self.keyword_stands_alone() {
            return self.keyword_alone(start, "rule", "expected rule name");
        }
        let name = if self.at_name() || self.peek().kind == TokenKind::Error {
            Some(self.parse_ident_name())
        } else {
            self.error_here("expected rule name");
            None
        };
        let sig_start = self.peek().start;
        let params = if self.peek().kind == TokenKind::LParen {
            self.parse_paren_params()
        } else {
            Vec::new()
        };
        let mut rule_kind = None;
        let mut stopped_short = false;
        if self.peek().kind == TokenKind::Colon {
            self.bump();
            if self.declaration_begins_below() {
                // The header stopped at `:`, and the line below is not its
                // kind: it begins a declaration of its own.
                self.error_at_last_read("expected rule kind");
                stopped_short = true;
            } else if self.at_name() {
                rule_kind = Some(self.parse_ident_name());
            } else {
                self.error_here("expected rule kind");
            }
        }
        let mut source_basis = None;
        if self.eat_ident("from") {
            source_basis = self.parse_expr_before_block();
        }
        let mut guard = None;
        let mut selection = None;
        let mut consequences = Vec::new();
        let mut fallback = Vec::new();
        let mut require = None;
        // A header that stopped short where its kind should stand has been
        // reported. What follows it is the declaration below, not a body.
        let has_body = !stopped_short && self.peek().kind == TokenKind::LBrace;
        if has_body {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.eat_ident("when") {
                    match self.parse_expr() {
                        Some(e) => {
                            guard = Some(and_opt_expr(guard.take(), e));
                            self.eat_semi();
                        }
                        None => self.rule_statement_lacks("expected a condition after `when`"),
                    }
                } else if self.eat_ident("then") {
                    if let Some(item) = self.parse_rule_consequence() {
                        consequences.push(item);
                    } else {
                        self.error_where_statement_stops("expected a consequence after `then`");
                        self.skip_rest_of_statement();
                    }
                } else if self.eat_ident("otherwise") {
                    if let Some(item) = self.parse_rule_consequence() {
                        fallback.push(item);
                    } else {
                        self.error_where_statement_stops(
                            "expected a consequence after `otherwise`",
                        );
                        self.skip_rest_of_statement();
                    }
                } else if self.eat_ident("require") {
                    match self.parse_expr() {
                        Some(e) => {
                            require = Some(and_opt_expr(require.take(), e));
                            self.eat_semi();
                        }
                        None => {
                            self.rule_statement_lacks("expected a condition after `require`");
                        }
                    }
                } else if self.eat_ident("select") {
                    self.expect_word("unique");
                    let binder = self.parse_ident_name();
                    self.expect_word("from");
                    let finite_domain = self.judgment_expr("expected finite selection domain");
                    self.expect_word("where");
                    let predicate = self.judgment_expr("expected selection predicate");
                    let mut ordering = Vec::new();
                    let mut ascending = Vec::new();
                    if self.eat_ident("order_by") {
                        loop {
                            ordering.push(self.parse_ident_name());
                            ascending.push(if self.eat_ident("descending") {
                                false
                            } else {
                                self.eat_ident("ascending");
                                true
                            });
                            if !self.eat_kind(TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    if selection.is_some() {
                        self.error_here("rule can declare only one selection");
                    }
                    selection = Some(crate::ast::SelectionAst {
                        binder,
                        finite_domain,
                        predicate,
                        ordering,
                        ascending,
                        require_unique_keys: true,
                    });
                    self.eat_semi();
                } else if self.eat_ident("source") {
                    match self.parse_expr() {
                        Some(e) => {
                            if source_basis.is_none() {
                                source_basis = Some(e);
                            }
                            self.eat_semi();
                        }
                        None => self.rule_statement_lacks("expected a source after `source`"),
                    }
                } else if self.at_ident("valid_time") || self.at_ident("record_time") {
                    self.parse_rule_interval();
                } else {
                    // Text that is no statement of a rule: a line the rule
                    // does not recognize, or what follows a statement on
                    // its line.
                    self.error_here("expected a rule statement");
                    self.skip_balanced_until_end();
                }
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
        } else if !stopped_short {
            self.error_where_statement_stops("expected `{` to begin the rule body");
            self.skip_rest_of_header();
        }
        let end = self.last_end(start.end);
        let mut decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            "rule",
            name,
            Some(self.src_slice(sig_start, end).trim().to_owned()),
            self.src_slice(start.start, end),
        );
        decl.params = params;
        decl.rule_kind = rule_kind;
        decl.guard = guard;
        decl.consequences = consequences;
        decl.fallback = fallback;
        decl.require = require;
        decl.selection = selection;
        decl.source_basis = source_basis;
        self.finish_node();
        decl
    }

    fn parse_rule_consequence(&mut self) -> Option<ConsequenceAst> {
        if !self.at_consequence_op() {
            return None;
        }
        let start = self.peek().start;
        let verb = self.parse_ident_name();
        let expr = self.parse_expr().unwrap_or(Expr::Ident(String::new()));
        if self.eat_ident("as_to") {
            let _ = self.parse_expr();
        }
        // The consequence as it is written, from its verb on. A diagnostic
        // about the consequence quotes it.
        let text = self
            .src_slice(start, self.last_end(start))
            .trim()
            .to_owned();
        self.eat_semi();
        Some(ConsequenceAst { verb, expr, text })
    }

    /// Reports a `when`, `require` or `source` statement of a rule that has
    /// nothing to read after its word, and steps over what is left of the
    /// statement. With no diagnostic a rule whose `when` has lost its
    /// condition has no guard, and fires whatever the case holds.
    fn rule_statement_lacks(&mut self, message: &str) {
        self.error_where_statement_stops(message);
        self.skip_rest_of_statement();
    }

    /// A `valid_time` or `record_time` line of a rule: the word and an
    /// interval. The interval is read so that the line is one statement.
    /// Nothing of it is kept, because the interpreter does not apply these
    /// lines.
    fn parse_rule_interval(&mut self) {
        let word = self.bump();
        if !self.at_stmt_break() && self.peek().kind == TokenKind::LBracket {
            let open = self.peek();
            let reported = self.diagnostics.len();
            let interval = self.inside_delimiters(Self::parse_interval_expr);
            // `parse_interval_expr` puts an empty name where a bound is
            // missing and reports nothing. The interval of a rule needs both
            // of its bounds.
            let lacks_a_bound = matches!(
                &interval,
                Expr::Apply { args, .. }
                    if args
                        .iter()
                        .any(|bound| matches!(bound, Expr::Ident(name) if name.is_empty()))
            );
            if lacks_a_bound && self.diagnostics.len() == reported {
                let message = format!(
                    "expected two bounds in the interval after `{}`",
                    self.text(word)
                );
                self.error(open, &message);
            }
            self.eat_semi();
        } else {
            let message = format!("expected an interval after `{}`", self.text(word));
            self.error_where_statement_stops(&message);
            self.skip_rest_of_statement();
        }
    }

    fn at_consequence_op(&mut self) -> bool {
        let t = self.peek();
        t.kind == TokenKind::Ident && CONSEQUENCE_OPS.contains(&self.text(t))
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        self.flush_to_idx();
        let checkpoint = self.builder.checkpoint();
        let expr = self.parse_bp(0);
        self.flush_to_idx();
        if expr.is_some() {
            self.builder
                .start_node_at(checkpoint, SyntaxKind::EXPR.into());
            self.builder.finish_node();
        }
        expr
    }

    /// Parses an expression that the block of the enclosing construct
    /// follows, in the mode `brace_opens_enclosing_block`, and puts the mode
    /// of the expression around it back afterwards.
    fn parse_expr_before_block(&mut self) -> Option<Expr> {
        let outer = std::mem::replace(&mut self.brace_opens_enclosing_block, true);
        let expr = self.parse_expr();
        self.brace_opens_enclosing_block = outer;
        expr
    }

    /// Runs `read` on what stands between a pair of delimiters. A name
    /// followed by `{` takes its block there, whatever expression the
    /// delimiters stand in, and the mode of that expression is put back when
    /// they close.
    fn inside_delimiters<R>(&mut self, read: impl FnOnce(&mut Self) -> R) -> R {
        let outer = std::mem::replace(&mut self.brace_opens_enclosing_block, false);
        let result = read(self);
        self.brace_opens_enclosing_block = outer;
        result
    }

    fn parse_bp(&mut self, min_bp: u8) -> Option<Expr> {
        if self.expression_depth >= MAX_EXPRESSION_DEPTH {
            self.error_here("expression nesting exceeds the limit of 128");
            self.skip_one_transaction_step();
            return None;
        }
        self.expression_depth += 1;
        let result = self.parse_bp_inner(min_bp);
        self.expression_depth -= 1;
        result.filter(|expression| self.expression_within_budget(expression))
    }

    fn expression_within_budget(&mut self, expression: &Expr) -> bool {
        let mut pending = vec![(expression, 1usize)];
        let mut nodes = 0;
        while let Some((expression, depth)) = pending.pop() {
            nodes += 1;
            if depth > MAX_EXPRESSION_DEPTH || nodes > MAX_EXPRESSION_NODES {
                self.error_here("expression tree exceeds depth or node budget");
                return false;
            }
            let child_depth = depth + 1;
            match expression {
                Expr::Apply { callee, args } => {
                    pending.push((callee, child_depth));
                    pending.extend(args.iter().map(|arg| (arg, child_depth)));
                }
                Expr::Binary { left, right, .. } => {
                    pending.push((left, child_depth));
                    pending.push((right, child_depth));
                }
                Expr::Field { base, .. } => pending.push((base, child_depth)),
                Expr::Call { args, .. } | Expr::Block(args) | Expr::Set(args) => {
                    pending.extend(args.iter().map(|arg| (arg, child_depth)))
                }
                Expr::If { cond, then, else_ } => {
                    pending.push((cond, child_depth));
                    pending.push((then, child_depth));
                    pending.extend(else_.as_deref().map(|branch| (branch, child_depth)));
                }
                Expr::Require(inner)
                | Expr::Grouped(inner)
                | Expr::NamedArg { value: inner, .. } => pending.push((inner, child_depth)),
                Expr::Record(fields) => {
                    pending.extend(fields.values().map(|value| (value, child_depth)))
                }
                Expr::Bool(_)
                | Expr::Int(_)
                | Expr::Decimal(_)
                | Expr::String(_)
                | Expr::Ident(_)
                | Expr::Money { .. }
                | Expr::Duration { .. } => {}
            }
        }
        true
    }

    fn parse_bp_inner(&mut self, min_bp: u8) -> Option<Expr> {
        let mut lhs = self.parse_prefix()?;
        loop {
            if self.at_stmt_break() {
                break;
            }
            if let Some((lbp, rbp, op)) = self.infix_info() {
                if lbp < min_bp {
                    break;
                }
                if matches!(
                    op,
                    InfixOp::Bin(
                        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
                    )
                ) && matches!(
                    lhs,
                    Expr::Binary {
                        op: BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge,
                        ..
                    }
                ) {
                    self.error_here("chained comparisons require explicit logical conjunction");
                }
                self.bump();
                if let Some(rhs) = self.parse_bp(rbp) {
                    lhs = match op {
                        InfixOp::Bin(bin) => Expr::Binary {
                            op: bin,
                            left: Box::new(lhs),
                            right: Box::new(rhs),
                        },
                        InfixOp::Word(word) => Expr::Apply {
                            callee: Box::new(Expr::Ident(word.to_owned())),
                            args: vec![lhs, rhs],
                        },
                    };
                    if !self.expression_within_budget(&lhs) {
                        self.skip_one_transaction_step();
                        return None;
                    }
                    continue;
                }
                self.error_here("expected expression after operator");
                break;
            }
            if min_bp <= 14
                && self.can_start_juxtaposition()
                && let Some(rhs) = self.parse_bp(15)
            {
                lhs = Expr::Apply {
                    callee: Box::new(lhs),
                    args: vec![rhs],
                };
                if !self.expression_within_budget(&lhs) {
                    self.skip_one_transaction_step();
                    return None;
                }
                continue;
            }
            break;
        }
        Some(lhs)
    }

    fn infix_info(&mut self) -> Option<(u8, u8, InfixOp)> {
        let t = self.peek();
        match t.kind {
            TokenKind::FatArrow => Some((1, 1, InfixOp::Word("=>"))),
            TokenKind::EqEq => Some((6, 7, InfixOp::Bin(BinOp::Eq))),
            TokenKind::Ne => Some((6, 7, InfixOp::Bin(BinOp::Ne))),
            TokenKind::Lt => Some((6, 7, InfixOp::Bin(BinOp::Lt))),
            TokenKind::Le => Some((6, 7, InfixOp::Bin(BinOp::Le))),
            TokenKind::Gt => Some((6, 7, InfixOp::Bin(BinOp::Gt))),
            TokenKind::Ge => Some((6, 7, InfixOp::Bin(BinOp::Ge))),
            TokenKind::Plus => Some((10, 11, InfixOp::Bin(BinOp::Add))),
            TokenKind::Minus => Some((10, 11, InfixOp::Bin(BinOp::Sub))),
            TokenKind::Star => Some((12, 13, InfixOp::Bin(BinOp::Mul))),
            TokenKind::Slash => Some((12, 13, InfixOp::Bin(BinOp::Div))),
            TokenKind::Ident => match self.text(t) {
                "or" => Some((2, 3, InfixOp::Bin(BinOp::Or))),
                "and" => Some((4, 5, InfixOp::Bin(BinOp::And))),
                "in" => Some((8, 9, InfixOp::Word("in"))),
                "as" => Some((8, 9, InfixOp::Word("as"))),
                "is" => Some((8, 9, InfixOp::Word("is"))),
                _ => None,
            },
            _ => None,
        }
    }

    fn is_expr_stop_word(word: &str) -> bool {
        matches!(
            word,
            "then"
                | "otherwise"
                | "when"
                | "else"
                | "goal"
                | "return"
                | "require"
                | "select"
                | "using"
                | "where"
                | "from"
                | "order_by"
                | "as_to"
                | "fuel"
                | "source"
                | "valid_time"
                | "record_time"
                | "module"
                | "under"
        )
    }

    fn is_prefix_word(word: &str) -> bool {
        matches!(
            word,
            "not"
                | "operative"
                | "effective"
                | "established"
                | "determined"
                | "always"
                | "active"
                | "every"
                | "unique"
                | "closed"
        )
    }

    fn can_start_juxtaposition(&mut self) -> bool {
        if self.at_stmt_break() {
            return false;
        }
        let t = self.peek();
        match t.kind {
            TokenKind::Ident | TokenKind::DurationUnit => {
                let w = self.text(t);
                if self.observation_body && w == "authenticate" {
                    return false;
                }
                if self.judgment_body
                    && matches!(
                        w,
                        "authority"
                            | "record"
                            | "establish"
                            | "reject"
                            | "on"
                            | "review"
                            | "standard"
                    )
                {
                    return false;
                }
                !Self::is_expr_stop_word(w)
                    && !matches!(w, "and" | "or" | "in" | "as" | "is")
                    && !Self::is_prefix_word(w)
                    && !matches!(self.peek_nth(1).kind, TokenKind::Colon | TokenKind::Eq)
            }
            TokenKind::Int
            | TokenKind::Decimal
            | TokenKind::String
            | TokenKind::Date
            | TokenKind::DateTime
            | TokenKind::PlusInf
            | TokenKind::MinusInf
            | TokenKind::LParen
            | TokenKind::LBracket => true,
            _ => false,
        }
    }

    fn parse_prefix(&mut self) -> Option<Expr> {
        // An operand begins here. At the start of a statement the cursor has
        // already passed the end of the statement before it, so only the
        // next token says whether there is an operand.
        if self.next_token_ends_statement() {
            return None;
        }
        let t = self.peek();
        match t.kind {
            TokenKind::RBrace
            | TokenKind::RParen
            | TokenKind::RBracket
            | TokenKind::Comma
            | TokenKind::Colon
            | TokenKind::Semicolon
            | TokenKind::Eof
            | TokenKind::Arrow
            | TokenKind::Bang => None,
            TokenKind::Ident if self.text(t) == "if" => {
                let e = self.parse_if_expr();
                Some(self.parse_postfix(e))
            }
            TokenKind::Ident if matches!(self.text(t), "for_all" | "exists") => {
                let e = self.parse_quantifier_expr();
                Some(self.parse_postfix(e))
            }
            TokenKind::Ident if self.text(t) == "transaction" => {
                let e = self.parse_transaction_expr();
                Some(self.parse_postfix(e))
            }
            TokenKind::Ident if Self::is_prefix_word(self.text(t)) => {
                let name = self.text(t).to_owned();
                self.bump();
                let rhs = self.parse_bp(PREFIX_RBP)?;
                // `operative P in Context` is the proposition plus its legal
                // context. `in` binds looser than this prefix, so without this
                // the outer Pratt loop would parse a proposition named `in`.
                let mut args = vec![rhs];
                if matches!(
                    name.as_str(),
                    "operative" | "determined" | "assumed" | "necessarily"
                ) && self.eat_ident("in")
                    && let Some(ctx) = self.parse_bp(PREFIX_RBP)
                {
                    args.push(ctx);
                }
                Some(Expr::Apply {
                    callee: Box::new(Expr::Ident(name)),
                    args,
                })
            }
            TokenKind::Ident if Self::is_expr_stop_word(self.text(t)) => None,
            TokenKind::Minus => {
                self.bump();
                if self.peek().kind == TokenKind::Int {
                    return Some(self.parse_signed_int_or_duration(true));
                }
                let rhs = self.parse_bp(PREFIX_RBP)?;
                Some(match rhs {
                    Expr::Int(n) => match n.checked_neg() {
                        Some(value) => Expr::Int(value),
                        None => {
                            self.error(t, "integer negation is out of range");
                            Expr::Int(n)
                        }
                    },
                    other => Expr::Apply {
                        callee: Box::new(Expr::Ident("-".to_owned())),
                        args: vec![other],
                    },
                })
            }
            _ => self.parse_atom().map(|e| self.parse_postfix(e)),
        }
    }

    fn parse_if_expr(&mut self) -> Expr {
        self.bump();
        let cond = self
            .parse_expr_before_block()
            .unwrap_or(Expr::Ident(String::new()));
        // When the statement has ended after the condition, what follows is
        // not the branch. A branch without braces is an expression in the
        // same statement.
        let branch = if self.at_stmt_break() {
            None
        } else if self.peek().kind == TokenKind::LBrace {
            Some(self.parse_brace_unwrapped())
        } else {
            self.parse_expr()
        };
        let then = branch.unwrap_or_else(|| {
            self.error_here("expected `{` after the `if` condition");
            Expr::Block(Vec::new())
        });
        // `else` is an expr stop-word, so Pratt will not consume it. Use
        // eat_ident after the then-branch (ASI treats `else` as continuation).
        let else_ = self.parse_else_branch().map(Box::new);
        Expr::If {
            cond: Box::new(cond),
            then: Box::new(then),
            else_,
        }
    }

    fn parse_else_branch(&mut self) -> Option<Expr> {
        if !self.eat_ident("else") {
            return None;
        }
        // When the statement has ended after the word `else`, what follows
        // is not the branch. A branch without braces is an expression in the
        // same statement: `else`, then `(2)` on the next line, is one.
        let branch = if self.at_stmt_break() {
            None
        } else if self.at_ident("if") {
            // The `if` of an `else if` nests in the `if` before it, and this
            // function and `parse_if_expr` call each other once for each
            // link of the chain. So each link counts as a level of
            // expression nesting, and the condition of the link that passes
            // the limit is rejected by `parse_bp`. With no count, a long
            // chain used the stack up before the tree it built was measured.
            self.expression_depth += 1;
            let nested = self.parse_if_expr();
            self.expression_depth -= 1;
            Some(nested)
        } else if self.peek().kind == TokenKind::LBrace {
            Some(self.parse_brace_unwrapped())
        } else {
            self.parse_expr()
        };
        Some(branch.unwrap_or_else(|| {
            self.error_here("expected `{` after `else`");
            Expr::Block(Vec::new())
        }))
    }

    fn parse_quantifier_expr(&mut self) -> Expr {
        let kind = self.parse_ident_name();
        let binder = self.parse_ident_name();
        if !self.eat_ident("in") {
            self.error_here("expected `in`");
        }
        let domain = self
            .parse_expr_before_block()
            .unwrap_or(Expr::Ident(String::new()));
        let body = if self.peek().kind == TokenKind::Colon {
            self.bump();
            self.parse_expr().unwrap_or(Expr::Block(Vec::new()))
        } else if self.peek().kind == TokenKind::LBrace {
            self.parse_brace_unwrapped()
        } else {
            Expr::Block(Vec::new())
        };
        Expr::Apply {
            callee: Box::new(Expr::Ident(kind)),
            args: vec![Expr::Ident(binder), domain, body],
        }
    }

    fn parse_atom(&mut self) -> Option<Expr> {
        let t = self.peek();
        match t.kind {
            TokenKind::Ident | TokenKind::DurationUnit => Some(self.parse_ident_atom()),
            TokenKind::Int => Some(self.parse_int_or_duration()),
            TokenKind::Decimal => {
                let s = self.text(t).to_owned();
                self.bump();
                Some(Expr::Decimal(s))
            }
            TokenKind::String => {
                let s = serde_json::from_str::<String>(self.text(t)).unwrap_or_default();
                self.bump();
                Some(Expr::String(s))
            }
            TokenKind::Date | TokenKind::DateTime | TokenKind::PlusInf | TokenKind::MinusInf => {
                let s = self.text(t).to_owned();
                self.bump();
                Some(Expr::Ident(s))
            }
            TokenKind::LParen => {
                self.bump();
                let inner = self.inside_delimiters(Self::parse_expr);
                self.expect_kind(TokenKind::RParen, "expected `)`");
                inner.map(|expr| Expr::Grouped(Box::new(expr)))
            }
            TokenKind::LBrace => Some(self.parse_braced_expr()),
            TokenKind::LBracket => Some(self.inside_delimiters(Self::parse_interval_expr)),
            TokenKind::Error => {
                self.error(
                    t,
                    "identifiers must be ASCII letters, digits, or underscore",
                );
                let s = self.text(t).to_owned();
                self.bump();
                Some(Expr::Ident(s))
            }
            _ => {
                self.error(t, "expected expression");
                None
            }
        }
    }

    fn parse_ident_atom(&mut self) -> Expr {
        let t = self.peek();
        let raw = self.text(t);
        if raw == "true" {
            self.bump();
            return Expr::Bool(true);
        }
        if raw == "false" {
            self.bump();
            return Expr::Bool(false);
        }
        let mut name = self.parse_ident_name();
        if self.at_stmt_break() {
            return Expr::Ident(name);
        }
        match self.call_generics_ahead() {
            TypeArgs::Read => name.push_str(&self.consume_generic_args_text()),
            TypeArgs::TooDeep => {
                // The arguments are not read. What is left of the statement
                // is stepped over, so that it is not read as a chain of
                // comparisons.
                self.error_here("type nesting exceeds the limit of 128");
                self.skip_one_transaction_step();
                return Expr::Ident(name);
            }
            TypeArgs::Absent => {}
        }
        if self.peek().kind == TokenKind::LParen
            && self.peek_nth(1).kind == TokenKind::Decimal
            && self.peek_nth(2).kind == TokenKind::RParen
        {
            self.bump();
            let amount_tok = self.peek();
            let amount = self.text(amount_tok).to_owned();
            self.bump();
            self.bump();
            return Expr::Money {
                currency: name,
                amount,
            };
        }
        if !self.brace_opens_enclosing_block && self.peek().kind == TokenKind::LBrace {
            if name == "transaction" {
                return Expr::Apply {
                    callee: Box::new(Expr::Ident(name)),
                    args: self.parse_transaction_block(),
                };
            }
            let block = self.parse_block_expr();
            return Expr::Apply {
                callee: Box::new(Expr::Ident(name)),
                args: vec![block],
            };
        }
        Expr::Ident(name)
    }

    fn parse_transaction_expr(&mut self) -> Expr {
        self.bump();
        if self.peek().kind == TokenKind::LBrace {
            Expr::Apply {
                callee: Box::new(Expr::Ident("transaction".to_owned())),
                args: self.inside_delimiters(Self::parse_transaction_block),
            }
        } else {
            Expr::Ident("transaction".to_owned())
        }
    }

    fn parse_transaction_block(&mut self) -> Vec<Expr> {
        self.bump();
        let mut stmts = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            if matches!(self.peek().kind, TokenKind::Comma | TokenKind::Semicolon) {
                self.bump();
                continue;
            }
            let first = self.peek();
            let before = self.idx;
            if let Some(e) = self.parse_eval_stmt() {
                stmts.push(e);
            } else {
                self.unreadable_statement(first, before, "expected transaction step");
            }
        }
        self.expect_kind(TokenKind::RBrace, "expected `}`");
        stmts
    }

    fn skip_one_transaction_step(&mut self) {
        let mut depth_brace = 0u32;
        let mut depth_paren = 0u32;
        let mut depth_brack = 0u32;
        loop {
            let t = self.peek();
            match t.kind {
                TokenKind::Eof => return,
                TokenKind::LBrace => {
                    depth_brace += 1;
                    self.bump();
                }
                TokenKind::RBrace => {
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 {
                        return;
                    }
                    depth_brace = depth_brace.saturating_sub(1);
                    self.bump();
                }
                TokenKind::LParen => {
                    depth_paren += 1;
                    self.bump();
                }
                TokenKind::RParen => {
                    depth_paren = depth_paren.saturating_sub(1);
                    self.bump();
                }
                TokenKind::LBracket => {
                    depth_brack += 1;
                    self.bump();
                }
                TokenKind::RBracket => {
                    depth_brack = depth_brack.saturating_sub(1);
                    self.bump();
                }
                TokenKind::Semicolon | TokenKind::Comma
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 =>
                {
                    self.bump();
                    return;
                }
                _ => {
                    self.bump();
                }
            }
        }
    }

    fn parse_int_or_duration(&mut self) -> Expr {
        self.parse_signed_int_or_duration(false)
    }

    fn parse_signed_int_or_duration(&mut self, negative: bool) -> Expr {
        let t = self.bump();
        let raw = if negative {
            format!("-{}", self.text(t))
        } else {
            self.text(t).to_owned()
        };
        let n = match raw.parse::<i64>() {
            Ok(value) => value,
            Err(_) => {
                self.error(t, "integer literal is out of range for Int");
                0
            }
        };
        if !self.at_stmt_break() && self.peek().kind == TokenKind::DurationUnit {
            let unit_tok = self.peek();
            let unit = self.text(unit_tok).to_owned();
            self.bump();
            Expr::Duration { n, unit }
        } else {
            Expr::Int(n)
        }
    }

    fn parse_interval_expr(&mut self) -> Expr {
        self.bump();
        let start = self.parse_expr().unwrap_or(Expr::Ident(String::new()));
        if self.peek().kind == TokenKind::Comma {
            self.bump();
        } else {
            self.error_here("expected `,`");
        }
        let end = self.parse_expr().unwrap_or(Expr::Ident(String::new()));
        match self.peek().kind {
            TokenKind::RBracket | TokenKind::RParen => {
                self.bump();
            }
            _ => {
                self.error_here("expected `]` or `)`");
            }
        }
        Expr::Apply {
            callee: Box::new(Expr::Ident("interval".to_owned())),
            args: vec![start, end],
        }
    }

    fn parse_brace_unwrapped(&mut self) -> Expr {
        match self.parse_block_expr() {
            Expr::Block(mut items) if items.len() == 1 => {
                items.pop().unwrap_or(Expr::Block(Vec::new()))
            }
            other => other,
        }
    }

    fn parse_braced_expr(&mut self) -> Expr {
        self.inside_delimiters(|parser| parser.parse_brace_contents(true))
    }

    fn parse_block_expr(&mut self) -> Expr {
        self.inside_delimiters(|parser| parser.parse_brace_contents(false))
    }

    fn parse_brace_contents(&mut self, collection: bool) -> Expr {
        self.bump();
        let mut items = Vec::new();
        let mut fields = BTreeMap::new();
        let mut is_record = false;
        let mut is_sequence = !collection;
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            if self.peek().kind == TokenKind::Semicolon {
                is_sequence = true;
                self.bump();
                continue;
            }
            if self.peek().kind == TokenKind::Comma {
                self.bump();
                continue;
            }
            if self.at_name() && matches!(self.peek_nth(1).kind, TokenKind::Colon | TokenKind::Eq) {
                let name = self.parse_ident_name();
                self.bump();
                let val = self.parse_expr().unwrap_or(Expr::Ident(String::new()));
                if collection {
                    is_record = true;
                    if fields.insert(name.clone(), val).is_some() {
                        self.error_here(&format!("duplicate record field `{name}`"));
                    }
                } else {
                    items.push(Expr::NamedArg {
                        name,
                        value: Box::new(val),
                    });
                }
            } else if let Some(e) = self.parse_expr() {
                items.push(e);
            } else {
                break;
            }
            if self.peek().kind == TokenKind::Semicolon {
                is_sequence = true;
                self.bump();
            } else if self.peek().kind == TokenKind::Comma {
                self.bump();
            }
        }
        self.expect_kind(TokenKind::RBrace, "expected `}`");
        if is_record {
            if !items.is_empty() {
                self.error_here("record fields cannot be mixed with positional elements");
            }
            Expr::Record(fields)
        } else if is_sequence {
            Expr::Block(items)
        } else {
            Expr::Set(items)
        }
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> Expr {
        loop {
            if self.at_stmt_break() {
                break;
            }
            match self.peek().kind {
                TokenKind::LParen => {
                    self.bump();
                    let args = self.inside_delimiters(Self::parse_args_until_rparen);
                    expr = match expr {
                        Expr::Ident(name) => Expr::Call { callee: name, args },
                        other => Expr::Apply {
                            callee: Box::new(other),
                            args,
                        },
                    };
                }
                TokenKind::Dot => {
                    self.bump();
                    let name = self.parse_ident_name();
                    expr = Expr::Field {
                        base: Box::new(expr),
                        name,
                    };
                }
                _ => break,
            }
            if !self.expression_within_budget(&expr) {
                self.skip_one_transaction_step();
                break;
            }
        }
        expr
    }

    fn parse_args_until_rparen(&mut self) -> Vec<Expr> {
        let mut args = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RParen | TokenKind::Eof) {
            if self.peek().kind == TokenKind::Comma {
                self.bump();
                continue;
            }
            if let Some(arg) = self.parse_arg() {
                args.push(arg);
            } else {
                break;
            }
            if self.peek().kind == TokenKind::Comma {
                self.bump();
            }
        }
        self.expect_kind(TokenKind::RParen, "expected `)`");
        args
    }

    fn parse_arg(&mut self) -> Option<Expr> {
        if self.at_name() && self.peek_nth(1).kind == TokenKind::Eq {
            let name = self.parse_ident_name();
            self.bump();
            let val = self.parse_expr()?;
            return Some(Expr::NamedArg {
                name,
                value: Box::new(val),
            });
        }
        self.parse_expr()
    }

    /// What follows a name where the type arguments of a call may stand:
    /// `Read` when type arguments stand there with `(` or `{` after them,
    /// and `TooDeep` when such arguments are nested deeper than the limit.
    /// A `<` that no `>` closes begins no type arguments, however far the
    /// scan for one runs: it is a comparison. The cursor is left where it
    /// was.
    fn call_generics_ahead(&mut self) -> TypeArgs {
        if self.peek().kind != TokenKind::Lt {
            return TypeArgs::Absent;
        }
        // `peek` has left the cursor at the `<`.
        match self.scan_type_arguments(self.idx) {
            AngleScan::Closed { after, depth }
                if self.significant_from(after).is_some_and(|next| {
                    matches!(next.kind, TokenKind::LParen | TokenKind::LBrace)
                }) =>
            {
                if depth > MAX_EXPRESSION_DEPTH {
                    TypeArgs::TooDeep
                } else {
                    TypeArgs::Read
                }
            }
            _ => TypeArgs::Absent,
        }
    }

    /// Steps over the type arguments at the cursor, which
    /// `call_generics_ahead` has found there, and returns their text.
    fn consume_generic_args_text(&mut self) -> String {
        let start = self.peek().start;
        if let AngleScan::Closed { after, .. } = self.scan_type_arguments(self.idx) {
            self.idx = after;
            while Self::is_trivia(self.peek_raw().kind) {
                self.idx += 1;
            }
        }
        self.src_slice(start, self.last_end(start))
    }

    /// The index of the first token at or after `index` that is no white
    /// space, comment or newline, when the parser can see it and it is of
    /// `kind`.
    fn index_of_kind_from(&self, index: usize, kind: TokenKind) -> Option<usize> {
        let at = *self.lines.significant_from.get(index)? as usize;
        (self.tokens.get(at)?.kind == kind).then_some(at)
    }

    /// The same for a token that can be a name or a part of one.
    fn index_of_name_from(&self, index: usize) -> Option<usize> {
        self.index_of_kind_from(index, TokenKind::Ident)
            .or_else(|| self.index_of_kind_from(index, TokenKind::DurationUnit))
    }

    /// What the `<` at token index `open` begins: type arguments that a `>`
    /// closes, or not.
    ///
    /// Type arguments are `<`, one or more types separated by commas, and
    /// `>`. A type is a name, which may have parts after `.` and type
    /// arguments of its own. Line ends do not count.
    ///
    /// The scan does not call itself for the arguments of a type. It keeps
    /// the `<` that are open in a list, so no depth of nesting uses the
    /// stack up, and it reads on to the `>` that closes `open` before
    /// anything is said about depth.
    ///
    /// What the scan finds for each `<` it passes is kept, so that the
    /// parser, which asks at each `<` in the order of the source, does not
    /// scan the same stretch of tokens again for each of them. In
    /// `g(x < y, x < y)` the scan that begins at the first `<` passes the
    /// second one, because `x <` after a comma looks like a type with
    /// arguments, and it ends at the `)` with both open. A scan from the
    /// second `<` would pass the same tokens in the same way and end at the
    /// same `)`, so its answer is known. Without that, each comparison of
    /// such a list scanned on to the end of the list, and the time grew
    /// with the square of its length.
    ///
    /// What is kept is kept for the tokens the parser can see now. A scan
    /// ends where they end, so under another view of the tokens the same `<`
    /// can have another answer.
    fn scan_type_arguments(&mut self, open: usize) -> AngleScan {
        let view = self.tokens.len();
        if let Some(&known) = self.angle_scans.get(&(view, open)) {
            return known;
        }
        // Each `<` that is open, with the depth of the deepest arguments
        // that have closed inside it.
        let mut opened = vec![(open, 0usize)];
        let mut at = open + 1;
        // True where a type must stand: after `<` and after `,`.
        let mut type_expected = true;
        loop {
            if type_expected {
                let Some(name) = self.index_of_name_from(at) else {
                    break;
                };
                at = name + 1;
                let mut whole = true;
                while let Some(dot) = self.index_of_kind_from(at, TokenKind::Dot) {
                    match self.index_of_name_from(dot + 1) {
                        Some(part) => at = part + 1,
                        None => {
                            whole = false;
                            break;
                        }
                    }
                }
                if !whole {
                    break;
                }
                match self.index_of_kind_from(at, TokenKind::Lt) {
                    Some(nested) => {
                        opened.push((nested, 0));
                        at = nested + 1;
                    }
                    None => type_expected = false,
                }
            } else if let Some(comma) = self.index_of_kind_from(at, TokenKind::Comma) {
                at = comma + 1;
                type_expected = true;
            } else if let Some(close) = self.index_of_kind_from(at, TokenKind::Gt) {
                let Some((closed, inner)) = opened.pop() else {
                    break;
                };
                at = close + 1;
                let found = AngleScan::Closed {
                    after: at,
                    depth: inner + 1,
                };
                self.angle_scans.insert((view, closed), found);
                match opened.last_mut() {
                    Some(outer) => outer.1 = outer.1.max(inner + 1),
                    None => return found,
                }
            } else {
                break;
            }
        }
        // The scan met a token that no type arguments hold, or the end of the
        // tokens. Each `<` that is still open stays open: a scan from it
        // ends at the same place.
        for (unclosed, _) in opened {
            self.angle_scans.insert((view, unclosed), AngleScan::Open);
        }
        AngleScan::Open
    }

    /// Steps over what is left of the current statement. Call this only after
    /// a token of the statement has been consumed. When a lookahead has
    /// already stepped over the statement's end, nothing is left: the token
    /// at the cursor begins the next statement, and it stays there.
    fn skip_rest_of_statement(&mut self) {
        if !self.passed_statement_end() {
            self.skip_balanced_until_end();
        }
    }

    /// Steps over what is left of the header of a declaration, as
    /// `skip_rest_of_statement` does, with one difference: outside brackets,
    /// a line that begins with a declaration keyword is never part of the
    /// header above it. The line rule would join the two when the header's
    /// line ends in a token that cannot end a statement, such as `:`.
    /// Returns true when the header stops short in that way.
    fn skip_rest_of_header(&mut self) -> bool {
        self.skip_rest_of_header_as(Skip::Header)
    }

    /// `skip_rest_of_header` for the header kind `skip` names.
    fn skip_rest_of_header_as(&mut self, skip: Skip) -> bool {
        if self.passed_statement_end() {
            return false;
        }
        if !self.next_token_ends_statement()
            && self.declaration_begins_below()
            && !(skip == Skip::QuantifierHeader && self.quantifier_body_begins_below())
        {
            return true;
        }
        self.skip_statement(skip)
    }

    /// Steps over what is left of a header the parser does not read, and
    /// reports `E100` when the header stops short of the declaration below,
    /// unless the token it stops at has just been reported.
    fn finish_header(&mut self) {
        self.finish_header_as(Skip::Header);
    }

    /// `finish_header` for the header kind `skip` names.
    fn finish_header_as(&mut self, skip: Skip) {
        if self.skip_rest_of_header_as(skip) && !self.last_read_is_reported() {
            self.error_at_last_read("expected the rest of the declaration");
        }
    }

    /// True when the last token read is `:` and the next token, on a line
    /// below it, is `for_all` or `exists`. Below the header of a quantifier
    /// that is the body of the quantifier.
    fn quantifier_body_begins_below(&mut self) -> bool {
        let next = self.peek();
        self.last_read()
            .is_some_and(|token| token.kind == TokenKind::Colon)
            && next.kind == TokenKind::Ident
            && matches!(self.text(next), "for_all" | "exists")
    }

    /// True when the first token after the newline token at `index` is
    /// `for_all` or `exists`.
    fn quantifier_begins_after(&self, index: usize) -> bool {
        self.significant_from(index + 1).is_some_and(|token| {
            token.kind == TokenKind::Ident && matches!(self.text(token), "for_all" | "exists")
        })
    }

    /// Steps over the tokens that are left on the line of the last token
    /// read, up to a `}` that closes no `{` of that line. A header that
    /// cannot be read is stepped over like this, so that a bracket it
    /// leaves open does not take the lines below. When the cursor has
    /// passed the end of that line, nothing is left.
    fn skip_rest_of_line(&mut self) {
        if self.line_end_passed() {
            return;
        }
        let mut depth = 0u32;
        loop {
            match self.peek_raw().kind {
                TokenKind::Newline | TokenKind::Eof => return,
                TokenKind::RBrace if depth == 0 => return,
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth -= 1,
                _ => {}
            }
            self.idx += 1;
        }
    }

    /// Steps over one statement that begins at the cursor, or over what is
    /// left of the current one, without reading it.
    fn skip_balanced_until_end(&mut self) {
        self.skip_statement(Skip::Statement);
    }

    /// Steps over a statement as `skip_balanced_until_end` describes. For
    /// the header of a declaration it also stops, and returns true, at the
    /// end of a line outside brackets that does not end the statement when
    /// the next line that has a token begins with a declaration keyword.
    /// For the header of a quantifier there is one exception: below a line
    /// that ends in `:`, a line that begins with `for_all` or `exists` is
    /// the body of the quantifier, and is stepped over with it.
    fn skip_statement(&mut self, skip: Skip) -> bool {
        let mut depth_brace = 0u32;
        let mut depth_paren = 0u32;
        let mut depth_brack = 0u32;
        let mut last_sig = self
            .significant_before(self.idx)
            .map_or(TokenKind::Eof, |token| token.kind);
        loop {
            let t = self.peek_raw();
            if Self::is_trivia(t.kind) {
                self.idx += 1;
                continue;
            }
            match t.kind {
                TokenKind::Eof => return false,
                TokenKind::LBrace => {
                    depth_brace += 1;
                    last_sig = t.kind;
                    self.idx += 1;
                }
                TokenKind::RBrace => {
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 {
                        return false;
                    }
                    depth_brace = depth_brace.saturating_sub(1);
                    last_sig = t.kind;
                    self.idx += 1;
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 {
                        if self.is_continuation(self.peek_significant()) {
                            continue;
                        }
                        return false;
                    }
                }
                TokenKind::LParen => {
                    depth_paren += 1;
                    last_sig = t.kind;
                    self.idx += 1;
                }
                TokenKind::RParen => {
                    if depth_paren > 0 {
                        depth_paren = depth_paren.saturating_sub(1);
                    } else if depth_brack > 0 {
                        depth_brack = depth_brack.saturating_sub(1);
                    }
                    last_sig = t.kind;
                    self.idx += 1;
                }
                TokenKind::LBracket => {
                    depth_brack += 1;
                    last_sig = t.kind;
                    self.idx += 1;
                }
                TokenKind::RBracket => {
                    depth_brack = depth_brack.saturating_sub(1);
                    last_sig = t.kind;
                    self.idx += 1;
                }
                TokenKind::Semicolon => {
                    last_sig = t.kind;
                    self.idx += 1;
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 {
                        return false;
                    }
                }
                TokenKind::Newline => {
                    let outside_brackets = depth_brace == 0 && depth_paren == 0 && depth_brack == 0;
                    if outside_brackets
                        && Self::can_end_statement(last_sig)
                        && !self.next_line_continues(self.idx)
                    {
                        self.idx += 1;
                        return false;
                    }
                    let quantifier_body = skip == Skip::QuantifierHeader
                        && last_sig == TokenKind::Colon
                        && self.quantifier_begins_after(self.idx);
                    if skip != Skip::Statement
                        && outside_brackets
                        && !quantifier_body
                        && self.declaration_begins_after(self.idx)
                    {
                        self.idx += 1;
                        return true;
                    }
                    self.idx += 1;
                }
                _ => {
                    last_sig = t.kind;
                    self.idx += 1;
                }
            }
        }
    }

    fn recover_declaration(&mut self) {
        let mut depth = 0i32;
        if self.peek_raw().kind == TokenKind::Ident {
            self.idx += 1;
        }
        loop {
            let t = self.peek_raw();
            match t.kind {
                TokenKind::Eof => return,
                TokenKind::LBrace => {
                    depth += 1;
                    self.idx += 1;
                }
                TokenKind::RBrace => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                    self.idx += 1;
                    if depth == 0 {
                        return;
                    }
                }
                TokenKind::Ident if depth == 0 && is_declaration_keyword(self.text(t)) => {
                    return;
                }
                TokenKind::Newline if depth == 0 => {
                    self.idx += 1;
                    return;
                }
                _ => self.idx += 1,
            }
        }
    }
}

fn and_opt_expr(prev: Option<Expr>, next: Expr) -> Expr {
    match prev {
        Some(prev) => Expr::Binary {
            op: BinOp::And,
            left: Box::new(prev),
            right: Box::new(next),
        },
        None => next,
    }
}

fn or_opt_expr(prev: Option<Expr>, next: Expr) -> Expr {
    match prev {
        Some(prev) => Expr::Binary {
            op: BinOp::Or,
            left: Box::new(prev),
            right: Box::new(next),
        },
        None => next,
    }
}

fn is_declaration_keyword(word: &str) -> bool {
    matches!(
        word,
        "jurisdiction"
            | "source_snapshot"
            | "source_manifest"
            | "effective_at"
            | "recorded_at"
            | "outside_scope"
            | "source"
            | "import"
            | "entity"
            | "type"
            | "record_type"
            | "evidence_type"
            | "office"
            | "proposition"
            | "observation"
            | "nomination"
            | "fn"
            | "calc"
            | "effect"
            | "for_all"
            | "exists"
            | "rule"
            | "power"
            | "duty"
            | "judgment"
            | "decision"
            | "legal_act"
            | "clause"
            | "interpretation_family"
            | "conflict_doctrine"
            | "query"
            | "scenario"
            | "verify"
    )
}

fn trim_quotes(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(s)
}

fn seq_exprs(stmts: Vec<Expr>) -> Option<Expr> {
    match stmts.len() {
        0 => None,
        1 => stmts.into_iter().next(),
        _ => Some(Expr::Block(stmts)),
    }
}

fn extend_expr(mut stmts: Vec<Expr>, extra: Option<Expr>) -> Vec<Expr> {
    match extra {
        Some(Expr::Block(mut more)) => stmts.append(&mut more),
        Some(e) => stmts.push(e),
        None => {}
    }
    stmts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Item;

    #[test]
    fn parses_half_open_interval_without_eating_module_end() {
        let src = r#"
module Examples.Mini version "0.1.0" {
    source Instrument {
        effective [execution_time, +inf)
    }
    entity Bryan : NaturalPerson
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.module().unwrap().items.len(), 2);
    }

    #[test]
    fn parses_minimal_module() {
        let src = r#"
module Examples.Mini version "0.1.0" {
    jurisdiction Massachusetts
    entity Bryan : NaturalPerson
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let m = parsed.module().unwrap();
        assert_eq!(m.name, "Examples.Mini");
        assert!(m.type_params.is_empty());
        assert_eq!(m.items.len(), 2);
    }

    #[test]
    fn unknown_declaration_is_e100() {
        let src = r#"
module X version "0.1.0" {
    widget Foo
}
"#;
        let parsed = parse_file(src);
        assert!(parsed.has_errors());
        assert_eq!(parsed.diagnostics[0].code, DiagnosticCode::E100);
    }

    #[test]
    fn parses_multiline_variant_type() {
        let src = r#"
module X version "0.1.0" {
    type WithholdingBasis =
        HarmBased {
            exemption: FOIAExemption
        }
      | ValidExemption3 {
            exemption: Exemption3
        }
    entity Bryan : NaturalPerson
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let m = parsed.module().unwrap();
        assert_eq!(m.items.len(), 2);
        assert!(matches!(m.items[0], Item::Type(_)));
        assert!(matches!(m.items[1], Item::Entity(_)));
    }

    #[test]
    fn unknown_declaration_recovers_so_later_items_parse() {
        let src = r#"
module X version "0.1.0" {
    widget Foo
    entity Bryan : NaturalPerson
    gadget {
        inner
    }
    entity Alice : NaturalPerson
}
"#;
        let parsed = parse_file(src);
        assert!(parsed.has_errors());
        assert!(
            parsed
                .diagnostics
                .iter()
                .all(|d| d.code == DiagnosticCode::E100)
        );
        let m = parsed.module().expect("module after recovery");
        let entities: Vec<_> = m
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Entity(d) => d.name.as_deref(),
                _ => None,
            })
            .collect();
        assert_eq!(entities, ["Bryan", "Alice"]);
    }

    fn parse_example_or_skip(rel: &str) {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
        let Ok(src) = std::fs::read_to_string(&path) else {
            return;
        };
        let parsed = parse_file(&src);
        assert!(
            !parsed.has_errors(),
            "{rel} diagnostics: {:?}",
            parsed.diagnostics
        );
        let module = parsed
            .module()
            .unwrap_or_else(|| panic!("{rel}: no module"));
        assert!(!module.items.is_empty(), "{rel}: expected declarations");
        let reconstructed: String = parsed
            .tokens
            .iter()
            .filter(|t| t.kind != TokenKind::Eof)
            .map(|t| parsed.text(*t))
            .collect();
        assert_eq!(reconstructed, src, "{rel}: tokens must be lossless");
        assert_eq!(
            parsed.syntax().text(),
            src.as_str(),
            "{rel}: green tree must be lossless"
        );
        let formatted = format_module(&src).unwrap_or_else(|d| panic!("{rel} format: {d}"));
        let again = format_module(&formatted).unwrap_or_else(|d| panic!("{rel} reformat: {d}"));
        assert_eq!(formatted, again, "{rel}: format should be stable");
    }

    #[test]
    fn parses_example_modules_without_e100() {
        parse_example_or_skip("../../examples/trust/bryan-revocable-trust.fr");
        parse_example_or_skip("../../examples/prenup/ava-noah.fr");
        parse_example_or_skip("../../examples/foia/foia-request.fr");
        parse_example_or_skip("../../examples/massachusetts-llc/harbor-robotics.fr");
    }

    fn visit_fr(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit_fr(&path, files);
            } else if path.extension().and_then(|e| e.to_str()) == Some("fr") {
                files.push(path);
            }
        }
    }

    #[test]
    fn parses_every_example_fr_file() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
        let mut files = Vec::new();
        visit_fr(&root, &mut files);
        let prelude = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../prelude");
        visit_fr(&prelude, &mut files);
        assert!(
            files.len() >= 8,
            "expected a corpus of .fr modules, found {}",
            files.len()
        );
        for path in files {
            let rel = path.display().to_string();
            let src = std::fs::read_to_string(&path).unwrap();
            let parsed = parse_file(&src);
            assert!(
                !parsed.has_errors(),
                "{rel} diagnostics: {:?}",
                parsed.diagnostics
            );
            assert_eq!(
                parsed.syntax().text(),
                src.as_str(),
                "{rel}: green tree must be lossless"
            );
        }
    }

    #[test]
    fn parses_effect_fuel_and_quantifiers() {
        let src = r#"
module Examples.Lang version "0.1.0" {
    effect DocketLookup {
        request(docket_id: String) -> Docket
    }
    fn countdown(n: Int) -> Int fuel = 4 {
        countdown(n - 1)
    }
    calc double(n: Int) -> Int {
        n + n
    }
    query all_ok() -> Bool {
        for_all x in People: Eligible(x)
        exists y in People: Trustee(y)
    }
    verify ClosedWorld {
        for_all x in People: Eligible(x)
        exists y in People: Trustee(y)
    }
    for_all z in People: Person(z)
    exists w in People: Trustee(w)
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let m = parsed.module().unwrap();
        let kinds: Vec<_> = m.items.iter().map(Item::keyword).collect();
        assert_eq!(
            kinds,
            [
                "effect", "fn", "calc", "query", "verify", "for_all", "exists",
            ]
        );
        assert!(matches!(m.items[0], Item::Effect(_)));
        assert!(matches!(m.items[1], Item::Function(_)));
        assert_eq!(m.items[1].keyword(), "fn");
        let fuel_fn = match &m.items[1] {
            Item::Function(d) => d,
            other => panic!("{other:?}"),
        };
        assert!(fuel_fn.source.contains("fuel = 4"), "{}", fuel_fn.source);
        let effect = match &m.items[0] {
            Item::Effect(d) => d,
            other => panic!("{other:?}"),
        };
        assert_eq!(effect.name.as_deref(), Some("DocketLookup"));
        assert!(effect.source.contains("request(docket_id: String)"));
        let query = match &m.items[3] {
            Item::Query(d) => d,
            other => panic!("{other:?}"),
        };
        assert!(query.source.contains("for_all x in People"));
        assert!(query.source.contains("exists y in People"));
    }

    #[test]
    fn fuel_after_effect_row_stays_on_the_function() {
        let src = r#"
module Examples.FuelRow version "0.1.0" {
    fn walk(n: Int) -> Int ! {Observe} fuel = 8 {
        walk(n - 1)
    }
    entity Bryan : NaturalPerson
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let m = parsed.module().unwrap();
        assert_eq!(m.items.len(), 2);
        match &m.items[0] {
            Item::Function(d) => {
                assert!(d.source.contains("fuel = 8"));
                assert!(d.source.contains("walk(n - 1)"));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(m.items[1], Item::Entity(_)));
    }

    fn wrap_module(body: &str) -> String {
        format!("module X version \"0.1.0\" {{\n{body}\n}}\n")
    }

    fn parse_body(body: &str) -> Parse {
        let parsed = parse_file(&wrap_module(body));
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        parsed
    }

    fn first_query(parsed: &Parse) -> &Decl {
        match &parsed.module().unwrap().items[0] {
            Item::Query(d) => d,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn evaluate_true_and_false_bodies_differ_in_ast() {
        let t = parse_body("query q() -> Bool { goal Evaluate { true } }");
        let f = parse_body("query q() -> Bool { goal Evaluate { false } }");
        let tg = first_query(&t).goal.as_ref().expect("true goal");
        let fg = first_query(&f).goal.as_ref().expect("false goal");
        assert_eq!(tg.kind, "Evaluate");
        assert_eq!(fg.kind, "Evaluate");
        assert_eq!(tg.expr.as_ref(), Some(&Expr::Bool(true)));
        assert_eq!(fg.expr.as_ref(), Some(&Expr::Bool(false)));
        assert_ne!(tg.expr, fg.expr);
    }

    fn expect_require_then(expr: &Expr, required: Expr, then: Expr) {
        match expr {
            Expr::Block(items) if items.len() == 2 => {
                match &items[0] {
                    Expr::Require(inner) => assert_eq!(inner.as_ref(), &required),
                    other => panic!("{other:?}"),
                }
                assert_eq!(items[1], then);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn require_false_return_true_is_not_just_true() {
        let parsed = parse_body("query q() -> Bool { require false; return true }");
        let q = first_query(&parsed);
        let expr = q.expr.as_ref().expect("query expr");
        expect_require_then(expr, Expr::Bool(false), Expr::Bool(true));
        assert_ne!(expr, &Expr::Bool(true));
        assert!(q.goal.is_none());
    }

    #[test]
    fn evaluate_require_false_return_true_keeps_both() {
        let parsed =
            parse_body("query q() -> Bool { goal Evaluate { require false; return true } }");
        let q = first_query(&parsed);
        let goal = q.goal.as_ref().expect("evaluate goal");
        assert_eq!(goal.kind, "Evaluate");
        let expr = goal.expr.as_ref().expect("goal expr");
        expect_require_then(expr, Expr::Bool(false), Expr::Bool(true));
        assert_ne!(expr, &Expr::Bool(true));
    }

    fn expect_transaction_steps(expr: &Expr) -> &[Expr] {
        match expr {
            Expr::Apply { callee, args } => {
                assert_eq!(callee.as_ref(), &Expr::Ident("transaction".to_owned()));
                args
            }
            other => panic!("{other:?}"),
        }
    }

    fn expect_duty_step(expr: &Expr, name: &str, action: &str) {
        match expr {
            Expr::Call { callee, args } => {
                assert_eq!(callee, "duty_step");
                assert_eq!(
                    args,
                    &vec![Expr::Ident(name.to_owned()), Expr::Ident(action.to_owned())]
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_transaction_block_keeps_both_steps() {
        let parsed = parse_body(
            r#"
    query q() -> Bool {
        transaction {
            duty_step(pay, attach);
            duty_step(pay, discharge)
        }
    }
"#,
        );
        let expr = first_query(&parsed).expr.as_ref().expect("query expr");
        let steps = expect_transaction_steps(expr);
        assert_eq!(steps.len(), 2, "{steps:?}");
        expect_duty_step(&steps[0], "pay", "attach");
        expect_duty_step(&steps[1], "pay", "discharge");
        assert!(!matches!(expr, Expr::Block(_)), "{expr:?}");
        assert!(!matches!(expr, Expr::Call { .. }), "{expr:?}");
    }

    #[test]
    fn parses_transaction_call_form() {
        let parsed = parse_body(
            r#"
    query q() -> Bool {
        transaction(duty_step(pay, attach), duty_step(pay, discharge))
    }
"#,
        );
        match first_query(&parsed).expr.as_ref() {
            Some(Expr::Call { callee, args }) => {
                assert_eq!(callee, "transaction");
                assert_eq!(args.len(), 2, "{args:?}");
                expect_duty_step(&args[0], "pay", "attach");
                expect_duty_step(&args[1], "pay", "discharge");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_single_step_transaction_block_without_unwrapping() {
        let parsed = parse_body(
            r#"
    query q() -> Bool {
        transaction {
            duty_step(pay, attach)
        }
    }
"#,
        );
        let expr = first_query(&parsed).expr.as_ref().expect("query expr");
        let steps = expect_transaction_steps(expr);
        assert_eq!(steps.len(), 1, "{steps:?}");
        expect_duty_step(&steps[0], "pay", "attach");
    }

    #[test]
    fn parses_transaction_block_inside_evaluate_goal() {
        let parsed = parse_body(
            r#"
    query q() -> Bool {
        goal Evaluate {
            transaction {
                duty_step(pay, attach);
                duty_step(pay, perform)
            }
        }
    }
"#,
        );
        let goal = first_query(&parsed).goal.as_ref().expect("evaluate goal");
        assert_eq!(goal.kind, "Evaluate");
        let steps = expect_transaction_steps(goal.expr.as_ref().expect("goal expr"));
        assert_eq!(steps.len(), 2, "{steps:?}");
        expect_duty_step(&steps[0], "pay", "attach");
        expect_duty_step(&steps[1], "pay", "perform");
    }

    #[test]
    fn parses_require_inside_transaction_block() {
        let parsed = parse_body(
            r#"
    query q() -> Int {
        transaction {
            require true;
            return 7
        }
    }
"#,
        );
        let steps =
            expect_transaction_steps(first_query(&parsed).expr.as_ref().expect("query expr"));
        assert_eq!(steps.len(), 2, "{steps:?}");
        match &steps[0] {
            Expr::Require(inner) => assert_eq!(inner.as_ref(), &Expr::Bool(true)),
            other => panic!("{other:?}"),
        }
        assert_eq!(steps[1], Expr::Int(7));
    }

    #[test]
    fn query_quantifiers_are_kept_with_return() {
        let parsed =
            parse_body("query q() -> Bool { for_all x in People: Eligible(x); return true }");
        let q = first_query(&parsed);
        match q.expr.as_ref() {
            Some(Expr::Block(items)) if items.len() == 2 => {
                match &items[0] {
                    Expr::Apply { callee, args } => {
                        assert_eq!(callee.as_ref(), &Expr::Ident("for_all".to_owned()));
                        assert_eq!(args.len(), 3, "{args:?}");
                    }
                    other => panic!("{other:?}"),
                }
                assert_eq!(items[1], Expr::Bool(true));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_verify_trivial_assert_true() {
        let parsed = parse_body("verify Trivial { assert true }");
        match &parsed.module().unwrap().items[0] {
            Item::Verify(d) => {
                assert_eq!(d.keyword, "verify");
                assert_eq!(d.name.as_deref(), Some("Trivial"));
                assert_eq!(d.expr.as_ref(), Some(&Expr::Bool(true)));
                assert_eq!(
                    d.fields.get("assert").map(String::as_str),
                    Some("assert true")
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unknown_source_field_is_error() {
        let src = r#"module X version "0.1.0" { source S { completely_unknown_field true } }"#;
        let parsed = parse_file(src);
        assert!(parsed.has_errors(), "unknown SourceField must be E100");
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| { d.code == DiagnosticCode::E100 && d.message.contains("SourceField") }),
            "{:?}",
            parsed.diagnostics
        );
    }

    #[test]
    fn trailing_tokens_after_module_are_error() {
        let src = r#"module X version "0.1.0" {} unexpected_tokens"#;
        let parsed = parse_file(src);
        assert!(parsed.has_errors(), "EOF required after module");
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::E100),
            "{:?}",
            parsed.diagnostics
        );
    }

    #[test]
    fn non_ascii_ident_does_not_panic_and_has_errors() {
        for src in [
            "module Café version \"0.1.0\" {}",
            "module X version \"0.1.0\" { query λ() -> Bool { return true } }",
            "module 日本語 version \"0.1.0\" {}",
        ] {
            let parsed = parse_file(src);
            assert!(parsed.has_errors(), "{src}");
            assert!(
                parsed
                    .diagnostics
                    .iter()
                    .any(|d| d.code == DiagnosticCode::E100),
                "{src}: {:?}",
                parsed.diagnostics
            );
        }
    }

    #[test]
    fn parses_evaluate_query_goal() {
        let parsed = parse_body("query q() -> Bool { goal Evaluate { true } }");
        let q = first_query(&parsed);
        assert_eq!(q.name.as_deref(), Some("q"));
        assert_eq!(q.result_type.as_deref(), Some("Bool"));
        let goal = q.goal.as_ref().expect("goal");
        assert_eq!(goal.kind, "Evaluate");
        assert_eq!(goal.expr.as_ref(), Some(&Expr::Bool(true)));
    }

    #[test]
    fn parses_automatic_query_return_compare() {
        let parsed = parse_body("query automatic f(x: Decimal) -> Bool { return x <= cap() }");
        let q = first_query(&parsed);
        assert!(q.automatic);
        assert_eq!(q.name.as_deref(), Some("f"));
        assert_eq!(q.params, vec![("x".to_owned(), "Decimal".to_owned())]);
        assert_eq!(q.result_type.as_deref(), Some("Bool"));
        match q.expr.as_ref() {
            Some(Expr::Binary {
                op: BinOp::Le,
                left,
                right,
            }) => {
                assert_eq!(left.as_ref(), &Expr::Ident("x".to_owned()));
                assert_eq!(
                    right.as_ref(),
                    &Expr::Call {
                        callee: "cap".to_owned(),
                        args: vec![],
                    }
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_query_effect_row() {
        let parsed =
            parse_body("query q() -> Bool ! {Observe, Determine} { goal Evaluate { true } }");
        let q = first_query(&parsed);
        assert_eq!(q.effects, ["Observe", "Determine"]);
        assert_eq!(
            q.goal.as_ref().and_then(|g| g.expr.as_ref()),
            Some(&Expr::Bool(true))
        );
    }

    #[test]
    fn parses_calc_money_signature_and_return() {
        let parsed = parse_body("calc tax_on(amount: Money<USD>) -> Money<USD> { return amount }");
        let d = match &parsed.module().unwrap().items[0] {
            Item::Function(d) => d,
            other => panic!("{other:?}"),
        };
        assert_eq!(d.keyword, "calc");
        assert_eq!(d.name.as_deref(), Some("tax_on"));
        assert_eq!(
            d.params,
            vec![("amount".to_owned(), "Money<USD>".to_owned())]
        );
        assert_eq!(d.result_type.as_deref(), Some("Money<USD>"));
        assert_eq!(d.expr.as_ref(), Some(&Expr::Ident("amount".to_owned())));
    }

    #[test]
    fn parses_fn_fuel_and_call_body() {
        let parsed = parse_body("fn first(n: Int) -> Int fuel = 4 { second(n) }");
        let d = match &parsed.module().unwrap().items[0] {
            Item::Function(d) => d,
            other => panic!("{other:?}"),
        };
        assert_eq!(d.fuel, Some(4));
        match d.expr.as_ref() {
            Some(Expr::Call { callee, args }) => {
                assert_eq!(callee, "second");
                assert_eq!(args, &vec![Expr::Ident("n".to_owned())]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_derive_rule_guard_and_consequences() {
        let parsed = parse_body("rule R : derive { when operative P() then derive Q() }");
        let d = match &parsed.module().unwrap().items[0] {
            Item::Rule(d) => d,
            other => panic!("{other:?}"),
        };
        assert_eq!(d.name.as_deref(), Some("R"));
        assert_eq!(d.rule_kind.as_deref(), Some("derive"));
        match d.guard.as_ref() {
            Some(Expr::Apply { callee, args }) => {
                assert_eq!(callee.as_ref(), &Expr::Ident("operative".to_owned()));
                assert_eq!(
                    args,
                    &vec![Expr::Call {
                        callee: "P".to_owned(),
                        args: vec![],
                    }]
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(!d.consequences.is_empty());
        assert_eq!(d.consequences[0].verb, "derive");
        assert_eq!(
            d.consequences[0].expr,
            Expr::Call {
                callee: "Q".to_owned(),
                args: vec![],
            }
        );
    }

    #[test]
    fn parses_operative_in_as_context_not_as_infix() {
        let parsed =
            parse_body("rule R : derive { when operative P() in Context(X) then derive Q() }");
        let d = match &parsed.module().unwrap().items[0] {
            Item::Rule(d) => d,
            other => panic!("{other:?}"),
        };
        match d.guard.as_ref() {
            Some(Expr::Apply { callee, args }) => {
                assert_eq!(callee.as_ref(), &Expr::Ident("operative".to_owned()));
                assert_eq!(args.len(), 2, "{args:?}");
                assert_eq!(
                    args[0],
                    Expr::Call {
                        callee: "P".to_owned(),
                        args: vec![],
                    }
                );
                assert_eq!(
                    args[1],
                    Expr::Call {
                        callee: "Context".to_owned(),
                        args: vec![Expr::Ident("X".to_owned())],
                    }
                );
            }
            other => panic!("guard must stay operative, not a proposition named in: {other:?}"),
        }
    }

    #[test]
    fn parses_otherwise_as_fallback_not_then() {
        let parsed =
            parse_body("rule R : derive { when true then derive P() otherwise derive Q() }");
        let d = match &parsed.module().unwrap().items[0] {
            Item::Rule(d) => d,
            other => panic!("{other:?}"),
        };
        assert_eq!(d.guard.as_ref(), Some(&Expr::Bool(true)));
        assert_eq!(d.consequences.len(), 1);
        assert_eq!(d.consequences[0].verb, "derive");
        assert_eq!(
            d.consequences[0].expr,
            Expr::Call {
                callee: "P".to_owned(),
                args: vec![],
            }
        );
        assert_eq!(d.fallback.len(), 1, "otherwise must not append to then");
        assert_eq!(d.fallback[0].verb, "derive");
        assert_eq!(
            d.fallback[0].expr,
            Expr::Call {
                callee: "Q".to_owned(),
                args: vec![],
            }
        );
    }

    #[test]
    fn parses_rule_require_and_from() {
        let parsed = parse_body(
            r#"rule R(x: Person) : derive from Instrument.clause("1") { when true require false then derive P() }"#,
        );
        let d = match &parsed.module().unwrap().items[0] {
            Item::Rule(d) => d,
            other => panic!("{other:?}"),
        };
        assert_eq!(d.params, vec![("x".to_owned(), "Person".to_owned())]);
        assert_eq!(d.require.as_ref(), Some(&Expr::Bool(false)));
        assert!(
            d.source_basis.is_some(),
            "from source_basis must not be dropped: {:?}",
            d.source_basis
        );
        assert_eq!(d.consequences.len(), 1);
        assert!(d.fallback.is_empty());
    }

    #[test]
    fn parses_money_and_duration_literals() {
        let parsed = parse_body(
            r#"
    calc floor() -> Money<USD> { return USD(1000.00) }
    calc wait() -> Duration<counted_days> { return 21 counted_days }
    calc work() -> Duration<working_days> { return 5 working_days }
"#,
        );
        let items = &parsed.module().unwrap().items;
        match &items[0] {
            Item::Function(d) => match d.expr.as_ref() {
                Some(Expr::Money { currency, amount }) => {
                    assert_eq!(currency, "USD");
                    assert_eq!(amount, "1000.00");
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
        match &items[1] {
            Item::Function(d) => match d.expr.as_ref() {
                Some(Expr::Duration { n, unit }) => {
                    assert_eq!(*n, 21);
                    assert_eq!(unit, "counted_days");
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
        match &items[2] {
            Item::Function(d) => match d.expr.as_ref() {
                Some(Expr::Duration { n, unit }) => {
                    assert_eq!(*n, 5);
                    assert_eq!(unit, "working_days");
                }
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    }

    fn first_fn(parsed: &Parse) -> &Decl {
        match &parsed.module().unwrap().items[0] {
            Item::Function(d) => d,
            other => panic!("{other:?}"),
        }
    }

    fn expect_if(expr: &Expr) -> (&Expr, &Expr, &Expr) {
        match expr {
            Expr::If {
                cond,
                then,
                else_: Some(else_),
            } => (cond.as_ref(), then.as_ref(), else_.as_ref()),
            other => panic!("expected If with else, got {other:?}"),
        }
    }

    fn expect_mul(expr: &Expr, ident: &str, decimal: &str) {
        match expr {
            Expr::Binary {
                op: BinOp::Mul,
                left,
                right,
            } => {
                assert_eq!(left.as_ref(), &Expr::Ident(ident.to_owned()));
                assert_eq!(right.as_ref(), &Expr::Decimal(decimal.to_owned()));
            }
            other => panic!("expected {ident} * {decimal}, got {other:?}"),
        }
    }

    fn expect_le_amount(expr: &Expr, decimal: &str) {
        match expr {
            Expr::Binary {
                op: BinOp::Le,
                left,
                right,
            } => {
                assert_eq!(left.as_ref(), &Expr::Ident("amount".to_owned()));
                assert_eq!(right.as_ref(), &Expr::Decimal(decimal.to_owned()));
            }
            other => panic!("expected amount <= {decimal}, got {other:?}"),
        }
    }

    #[test]
    fn parses_mul_tighter_than_add() {
        let parsed = parse_body("calc f() -> Int { 1 + 2 * 3 }");
        match first_fn(&parsed).expr.as_ref() {
            Some(Expr::Binary {
                op: BinOp::Add,
                left,
                right,
            }) => {
                assert_eq!(left.as_ref(), &Expr::Int(1));
                match right.as_ref() {
                    Expr::Binary {
                        op: BinOp::Mul,
                        left,
                        right,
                    } => {
                        assert_eq!(left.as_ref(), &Expr::Int(2));
                        assert_eq!(right.as_ref(), &Expr::Int(3));
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_subtraction_left_associative() {
        let parsed = parse_body("calc f() -> Int { 1 - 2 - 3 }");
        match first_fn(&parsed).expr.as_ref() {
            Some(Expr::Binary {
                op: BinOp::Sub,
                left,
                right,
            }) => {
                match left.as_ref() {
                    Expr::Binary {
                        op: BinOp::Sub,
                        left,
                        right,
                    } => {
                        assert_eq!(left.as_ref(), &Expr::Int(1));
                        assert_eq!(right.as_ref(), &Expr::Int(2));
                    }
                    other => panic!("{other:?}"),
                }
                assert_eq!(right.as_ref(), &Expr::Int(3));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn malformed_calc_body_is_error_and_later_entity_still_parses() {
        let src = r#"
module X version "0.1.0" {
    calc f() -> Int { 1 + }
    entity Bryan : NaturalPerson
}
"#;
        let parsed = parse_file(src);
        assert!(parsed.has_errors(), "malformed calc body must be an error");
        assert!(
            parsed
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::E100),
            "{:?}",
            parsed.diagnostics
        );
        let items = &parsed.module().unwrap().items;
        assert!(
            items.iter().any(|item| matches!(item, Item::Entity(_))),
            "recovery should keep the later entity: {items:?}"
        );
    }

    #[test]
    fn parses_calc_if_else_with_braced_mul() {
        let parsed = parse_body(
            "calc f(amount: Decimal) -> Decimal { if amount <= 10.00 { amount * 0.10 } else { amount * 0.24 } }",
        );
        let m = parsed.module().unwrap();
        assert_eq!(m.items.len(), 1);
        let decl = first_fn(&parsed);
        assert_eq!(decl.keyword, "calc");
        assert_eq!(decl.name.as_deref(), Some("f"));
        let (cond, then, else_) = expect_if(decl.expr.as_ref().expect("calc body"));
        expect_le_amount(cond, "10.00");
        expect_mul(then, "amount", "0.10");
        expect_mul(else_, "amount", "0.24");
    }

    #[test]
    fn parses_calc_if_else_with_newline_before_else() {
        let parsed = parse_body(
            r#"
    calc f(amount: Decimal) -> Decimal {
        if amount <= 10.00 { amount * 0.10 }
        else { amount * 0.24 }
    }
"#,
        );
        assert_eq!(parsed.module().unwrap().items.len(), 1);
        let (cond, then, else_) = expect_if(first_fn(&parsed).expr.as_ref().expect("calc body"));
        expect_le_amount(cond, "10.00");
        expect_mul(then, "amount", "0.10");
        expect_mul(else_, "amount", "0.24");
    }

    #[test]
    fn parses_nested_else_if_block() {
        let parsed = parse_body(
            r#"
    calc f(amount: Decimal) -> Decimal {
        if amount <= 10.00 { amount * 0.10 } else { if amount <= 20.00 { amount * 0.12 } else { amount * 0.24 } }
    }
"#,
        );
        assert_eq!(parsed.module().unwrap().items.len(), 1);
        let (cond, then, else_) = expect_if(first_fn(&parsed).expr.as_ref().expect("calc body"));
        expect_le_amount(cond, "10.00");
        expect_mul(then, "amount", "0.10");
        let (inner_cond, inner_then, inner_else) = expect_if(else_);
        expect_le_amount(inner_cond, "20.00");
        expect_mul(inner_then, "amount", "0.12");
        expect_mul(inner_else, "amount", "0.24");
    }

    #[test]
    fn parses_braced_positional_then_named_field() {
        let parsed = parse_body(
            r#"
    calc f() -> Packet {
        return Packet {
            certificate
            fee: current_fee(X)
        }
    }
"#,
        );
        assert_eq!(parsed.module().unwrap().items.len(), 1);
        match first_fn(&parsed).expr.as_ref() {
            Some(Expr::Apply { callee, args }) => {
                assert_eq!(callee.as_ref(), &Expr::Ident("Packet".to_owned()));
                match args.as_slice() {
                    [Expr::Block(items)] => {
                        assert_eq!(items[0], Expr::Ident("certificate".to_owned()));
                        match &items[1] {
                            Expr::NamedArg { name, value } => {
                                assert_eq!(name, "fee");
                                assert_eq!(
                                    value.as_ref(),
                                    &Expr::Call {
                                        callee: "current_fee".to_owned(),
                                        args: vec![Expr::Ident("X".to_owned())],
                                    }
                                );
                            }
                            other => panic!("{other:?}"),
                        }
                    }
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_else_if_chain() {
        let parsed = parse_body(
            "calc f(amount: Decimal) -> Decimal { if amount <= 10.00 { amount * 0.10 } else if amount <= 20.00 { amount * 0.12 } else { amount * 0.24 } }",
        );
        assert_eq!(parsed.module().unwrap().items.len(), 1);
        let (cond, then, else_) = expect_if(first_fn(&parsed).expr.as_ref().expect("calc body"));
        expect_le_amount(cond, "10.00");
        expect_mul(then, "amount", "0.10");
        let (inner_cond, inner_then, inner_else) = expect_if(else_);
        expect_le_amount(inner_cond, "20.00");
        expect_mul(inner_then, "amount", "0.12");
        expect_mul(inner_else, "amount", "0.24");
    }

    fn assert_green_round_trip(src: &str) {
        let parsed = parse_file(src);
        assert_eq!(parsed.syntax().text(), src);
        assert_eq!(parsed.syntax().kind(), SyntaxKind::MODULE);
    }

    #[test]
    fn green_tree_round_trips_tiny_module() {
        let src = "module Mini version \"0.1.0\" {\n    entity Bryan : NaturalPerson\n}\n";
        assert_green_round_trip(src);
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let root = parsed.syntax();
        assert!(
            root.children().any(|n| n.kind() == SyntaxKind::ITEM),
            "{root:#?}"
        );
        assert!(
            root.descendants().any(|n| n.kind() == SyntaxKind::DECL),
            "{root:#?}"
        );
    }

    #[test]
    fn green_tree_round_trips_comments_and_whitespace() {
        let src = "module M version \"0.1.0\" { // c\n  entity X: T\n}\n";
        assert_green_round_trip(src);
        let parsed = parse_file(src);
        assert!(
            parsed
                .syntax()
                .descendants()
                .any(|n| n.kind() == SyntaxKind::COMMENT),
            "{:#?}",
            parsed.syntax()
        );
        assert!(
            parsed
                .syntax()
                .descendants()
                .any(|n| n.kind() == SyntaxKind::WHITESPACE),
            "{:#?}",
            parsed.syntax()
        );
    }

    #[test]
    fn green_tree_round_trips_bryan_revocable_trust() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("examples/trust/bryan-revocable-trust.fr");
        let src = std::fs::read_to_string(&path).expect("bryan-revocable-trust.fr");
        let parsed = parse_file(&src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.syntax().text(), src.as_str());
        assert_eq!(parsed.syntax().kind(), SyntaxKind::MODULE);
    }

    #[test]
    fn parses_module_type_parameters() {
        let src = r#"module Box<T> version "0.1.0" { entity X: T }"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let m = parsed.module().unwrap();
        assert_eq!(m.name, "Box");
        assert_eq!(m.type_params, ["T"]);
        assert_eq!(parsed.syntax().text(), src);
        match &m.items[0] {
            Item::Entity(d) => assert_eq!(d.name.as_deref(), Some("X")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn parses_multiple_module_type_parameters() {
        let src = r#"module Map<K, V> version "0.1.0" { entity X: K }"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        assert_eq!(parsed.module().unwrap().type_params, ["K", "V"]);
    }

    #[test]
    fn parses_import_type_args() {
        let src = r#"
module M version "0.1.0" {
    import Box<NaturalPerson> version "0.1.0"
    entity X: NaturalPerson
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        let m = parsed.module().unwrap();
        match &m.items[0] {
            Item::Import(d) => {
                assert_eq!(d.name.as_deref(), Some("Box"));
                assert_eq!(d.type_args, ["NaturalPerson"]);
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(parsed.syntax().text(), src);
    }

    #[test]
    fn parses_import_nested_type_args() {
        let src = r#"
module M version "0.1.0" {
    import Box<FiniteSet<NaturalPerson>> version "0.1.0"
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        match &parsed.module().unwrap().items[0] {
            Item::Import(d) => {
                assert_eq!(d.name.as_deref(), Some("Box"));
                assert_eq!(d.type_args, ["FiniteSet<NaturalPerson>"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn existing_import_without_type_args_stays_empty() {
        let src = r#"
module M version "0.1.0" {
    import MA.TrustLaw.Fixture version "2026-08-23"
}
"#;
        let parsed = parse_file(src);
        assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics);
        match &parsed.module().unwrap().items[0] {
            Item::Import(d) => {
                assert_eq!(d.name.as_deref(), Some("MA.TrustLaw.Fixture"));
                assert!(d.type_args.is_empty());
            }
            other => panic!("{other:?}"),
        }
    }

    /// Expressions a statement can end in. After each of the last three the
    /// parser looks at the next token before it decides the expression is
    /// complete: for generic arguments, for a block and for a quantifier body.
    const STATEMENT_ENDINGS: &[&str] = &[
        "1",
        "-1",
        "1 // a comment",
        "1.5",
        "\"s\"",
        "x",
        "true",
        "f(x)",
        "(x)",
        "USD(1.00)",
        "3 days",
        "2026-01-01",
        "+inf",
        "a.b",
        "{1, 2}",
        "operative P",
        "determined P",
        "not P",
        "if c > 0 { a }",
        "if c > 0 { a } else { b }",
        "if c { a }",
        "if c { a } else { b }",
        "for_all x in {1, 2}: x > 0",
        "a < b",
        "transaction",
        "for_all x in D",
    ];

    /// Lines that begin with a name, the word `assert`, a number, a negative
    /// number and a duration unit, and a name below a blank line and below a
    /// comment line. None of them continues the line before it.
    const EXPRESSION_LINES: &[&str] = &["y", "assert", "2", "-1", "days", "\ny", "// a comment\ny"];
    const LINES_AFTER_REQUIRE: &[&str] = &["return y", "y", "assert", "2", "-1", "days"];

    /// A place where two statements stand on consecutive lines.
    struct StatementContext {
        name: &'static str,
        /// The lines before the first statement.
        open: &'static str,
        /// The lines after the last statement.
        close: &'static str,
        /// What stands before the first statement's expression on its line.
        lead: &'static str,
        /// The second lines that are valid here.
        second_lines: &'static [&'static str],
        /// The statements the parser read, in source order.
        statements: fn(&Parse) -> Vec<String>,
    }

    const STATEMENT_CONTEXTS: &[StatementContext] = &[
        StatementContext {
            name: "two assert lines in verify",
            open: "verify V {",
            close: "}",
            lead: "assert ",
            second_lines: &["assert q", "assert 2 == 2", "assert -1 < 0", "assert days"],
            statements: verify_statements,
        },
        StatementContext {
            name: "assert then bounds in verify",
            open: "verify V {",
            close: "}",
            lead: "assert ",
            second_lines: &["bounds { persons: 1 }", "bounds persons 1"],
            statements: verify_statements,
        },
        StatementContext {
            name: "two expression statements in a calc body",
            open: "calc f() -> Int {",
            close: "}",
            lead: "",
            second_lines: EXPRESSION_LINES,
            statements: body_statements,
        },
        StatementContext {
            name: "require then a statement in a query body",
            open: "query q() -> Bool {",
            close: "}",
            lead: "require ",
            second_lines: LINES_AFTER_REQUIRE,
            statements: body_statements,
        },
        StatementContext {
            name: "require then a statement in goal Evaluate",
            open: "query q() -> Bool {\ngoal Evaluate {",
            close: "}\n}",
            lead: "require ",
            second_lines: LINES_AFTER_REQUIRE,
            statements: goal_statements,
        },
        StatementContext {
            name: "two steps in a transaction",
            open: "query q() -> Bool {\ntransaction {",
            close: "}\n}",
            lead: "",
            second_lines: EXPRESSION_LINES,
            statements: transaction_steps,
        },
        StatementContext {
            name: "when then then in a rule",
            open: "rule R : derive {",
            close: "}",
            lead: "when ",
            second_lines: &["then derive Q"],
            statements: rule_statements,
        },
        StatementContext {
            name: "two then lines in a rule",
            open: "rule R : derive {",
            close: "}",
            lead: "then derive ",
            second_lines: &["then derive Q"],
            statements: rule_statements,
        },
    ];

    fn first_decl(parsed: &Parse) -> &Decl {
        parsed
            .module()
            .and_then(|module| module.items.first())
            .and_then(Item::decl)
            .expect("a first declaration")
    }

    /// The statements of a body the parser folds into one expression.
    fn sequence(body: Option<&Expr>) -> Vec<String> {
        match body {
            Some(Expr::Block(statements)) => statements.iter().map(|s| format!("{s:?}")).collect(),
            Some(statement) => vec![format!("{statement:?}")],
            None => Vec::new(),
        }
    }

    fn verify_statements(parsed: &Parse) -> Vec<String> {
        let decl = first_decl(parsed);
        let mut statements: Vec<String> =
            decl.assertions.iter().map(|a| format!("{a:?}")).collect();
        statements.extend(decl.fields.get("bounds").cloned());
        statements
    }

    fn body_statements(parsed: &Parse) -> Vec<String> {
        sequence(first_decl(parsed).expr.as_ref())
    }

    fn goal_statements(parsed: &Parse) -> Vec<String> {
        let goal = first_decl(parsed).goal.as_ref().expect("a goal");
        sequence(goal.expr.as_ref())
    }

    fn transaction_steps(parsed: &Parse) -> Vec<String> {
        let body = first_decl(parsed).expr.as_ref().expect("a query body");
        expect_transaction_steps(body)
            .iter()
            .map(|step| format!("{step:?}"))
            .collect()
    }

    fn rule_statements(parsed: &Parse) -> Vec<String> {
        let decl = first_decl(parsed);
        let guard = decl.guard.iter().map(|guard| format!("when {guard:?}"));
        let consequences = decl
            .consequences
            .iter()
            .map(|then| format!("then {} {:?}", then.verb, then.expr));
        guard.chain(consequences).collect()
    }

    /// The consequences of a rule as their verbs and expressions.
    fn consequence_pairs(items: &[ConsequenceAst]) -> Vec<(String, Expr)> {
        items
            .iter()
            .map(|item| (item.verb.clone(), item.expr.clone()))
            .collect()
    }

    #[test]
    fn a_statement_ends_at_its_line_whatever_the_next_line_begins_with() {
        let mut failures = Vec::new();
        for context in STATEMENT_CONTEXTS {
            // The statements of the context holding these lines, or its parse errors.
            let read = |lines: &[&str]| {
                let body = format!("{}\n{}\n{}", context.open, lines.join("\n"), context.close);
                let parsed = parse_file(&wrap_module(&body));
                if parsed.has_errors() {
                    let messages: Vec<&str> = parsed
                        .diagnostics
                        .iter()
                        .map(|diagnostic| diagnostic.message.as_str())
                        .collect();
                    Err(format!("rejected: {messages:?}"))
                } else {
                    Ok((context.statements)(&parsed))
                }
            };
            for ending in STATEMENT_ENDINGS {
                let first = format!("{}{ending}", context.lead);
                for &second in context.second_lines {
                    let expected = match (read(&[first.as_str()]), read(&[second])) {
                        (Ok(alone), Ok(next)) if alone.len() == 1 && next.len() == 1 => {
                            Ok([alone, next].concat())
                        }
                        apart => Err(format!("each line alone is not one statement: {apart:?}")),
                    };
                    let together = read(&[first.as_str(), second]);
                    if expected.is_err() || together != expected {
                        failures.push(format!(
                            "{}: `{first}` then `{second}`\n    read:     {together:?}\n    expected: {expected:?}",
                            context.name
                        ));
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} pairs of lines are not read as the two statements they are:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_line_that_begins_with_a_continuation_joins_the_line_before_it() {
        let returned = |expression: &str| {
            let parsed = parse_body(&format!("query q() -> Bool {{\nreturn {expression}\n}}"));
            first_query(&parsed).expr.clone().expect("a returned value")
        };
        let name = |text: &str| Box::new(Expr::Ident(text.to_owned()));
        for (two_lines, one_line, tree) in [
            (
                "a\nand b",
                "a and b",
                Expr::Binary {
                    op: BinOp::And,
                    left: name("a"),
                    right: name("b"),
                },
            ),
            (
                "a\n\nand b",
                "a and b",
                Expr::Binary {
                    op: BinOp::And,
                    left: name("a"),
                    right: name("b"),
                },
            ),
            (
                "a\n// why\nand b",
                "a and b",
                Expr::Binary {
                    op: BinOp::And,
                    left: name("a"),
                    right: name("b"),
                },
            ),
            (
                "a\nor b",
                "a or b",
                Expr::Binary {
                    op: BinOp::Or,
                    left: name("a"),
                    right: name("b"),
                },
            ),
            (
                "a\n.b",
                "a.b",
                Expr::Field {
                    base: name("a"),
                    name: "b".to_owned(),
                },
            ),
            (
                "f\n(x)",
                "f(x)",
                Expr::Call {
                    callee: "f".to_owned(),
                    args: vec![*name("x")],
                },
            ),
            (
                "if c > 0 { a }\nelse { b }",
                "if c > 0 { a } else { b }",
                Expr::If {
                    cond: Box::new(Expr::Binary {
                        op: BinOp::Gt,
                        left: name("c"),
                        right: Box::new(Expr::Int(0)),
                    }),
                    then: name("a"),
                    else_: Some(name("b")),
                },
            ),
        ] {
            assert_eq!(returned(two_lines), tree, "{two_lines:?}");
            assert_eq!(returned(one_line), tree, "{one_line:?}");
        }
        // The same after each operand the parser looks past.
        for (two_lines, one_line) in [
            ("x == 1\nand y == 2", "x == 1 and y == 2"),
            (
                "operative P\nand operative Q",
                "operative P and operative Q",
            ),
            ("operative P\nin C", "operative P in C"),
            ("if c > 0\n{ a }", "if c > 0 { a }"),
            ("if c\n{ a }", "if c { a }"),
            ("for_all x in D\n: x > 0", "for_all x in D: x > 0"),
            ("for_all x in D\n{ x > 0 }", "for_all x in D { x > 0 }"),
        ] {
            assert_eq!(returned(two_lines), returned(one_line), "{two_lines:?}");
        }
    }

    #[test]
    fn an_if_condition_followed_by_a_statement_end_is_e100() {
        let parsed = parse_file(&wrap_module("calc f(c: Bool) -> Int {\nif c\n1\n}"));
        let diagnostics: Vec<(DiagnosticCode, &str)> = parsed
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
            .collect();
        assert_eq!(
            diagnostics,
            [(
                DiagnosticCode::E100,
                "expected `{` after the `if` condition"
            )]
        );
        // The next line is a statement of its own, not the branch.
        assert_eq!(
            first_fn(&parsed).expr,
            Some(Expr::Block(vec![
                Expr::If {
                    cond: Box::new(Expr::Ident("c".to_owned())),
                    then: Box::new(Expr::Block(Vec::new())),
                    else_: None,
                },
                Expr::Int(1),
            ]))
        );
    }

    #[test]
    fn a_header_without_its_string_does_not_take_the_next_line() {
        let parsed = parse_file(&wrap_module(
            "source_snapshot\nentity Bryan : NaturalPerson",
        ));
        let messages: Vec<&str> = parsed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect();
        assert_eq!(messages, ["expected a value after `source_snapshot`"]);
        let kinds: Vec<&str> = parsed
            .module()
            .expect("a module")
            .items
            .iter()
            .map(Item::keyword)
            .collect();
        assert_eq!(kinds, ["source_snapshot", "entity"]);
    }

    /// An assertion of a verify block: the text of a line `verify` cannot
    /// decide, or the tree of an expression it can.
    fn shown(assertion: &Expr) -> String {
        match assertion {
            Expr::Apply { callee, args }
                if **callee == Expr::Ident("$unsupported_assertion".to_owned()) =>
            {
                match args.as_slice() {
                    [Expr::String(text)] => format!("undecided: {}", text.trim()),
                    other => format!("undecided: {other:?}"),
                }
            }
            decided => format!("{decided:?}"),
        }
    }

    #[test]
    fn a_verify_block_keeps_every_line_that_makes_a_claim_in_source_order() {
        let parsed = parse_body(
            "verify V {\n\
             interpretations all_admissible\n\
             for every x in {1, 2}: x == 3\n\
             judgments all_lawful\n\
             choices all_lawful\n\
             for_all x in {1, 2}: x > 0\n\
             assert true\n\
             bounds persons 1\n\
             for every y in {1}: y == 2\n\
             asert false\n\
             assuming fair\n\
             exists y in {1}: y == 1\n\
             }",
        );
        let decl = first_decl(&parsed);
        let read: Vec<String> = decl.assertions.iter().map(shown).collect();
        // The tree the same expression has on an `assert` line.
        let asserted = |expression: &str| {
            let alone = parse_body(&format!("verify V {{ assert {expression} }}"));
            shown(&first_decl(&alone).assertions[0])
        };
        assert_eq!(
            read,
            [
                "undecided: for every x in {1, 2}: x == 3".to_owned(),
                asserted("for_all x in {1, 2}: x > 0"),
                asserted("true"),
                "undecided: for every y in {1}: y == 2".to_owned(),
                "undecided: asert false".to_owned(),
                asserted("exists y in {1}: y == 1"),
            ]
        );
        assert_eq!(
            decl.fields.get("bounds").map(String::as_str),
            Some("bounds persons 1")
        );
        // Reading each field with the rest of the block hidden loses no token.
        assert_eq!(parsed.syntax().text(), parsed.source.as_str());
    }

    #[test]
    fn a_bounds_field_is_recorded_as_the_text_of_its_tokens() {
        // The lines of a verify block, and what the parser records for its
        // `bounds` field: the field's tokens with one space between them.
        let rows = [
            // A comment inside the field is left out,
            (
                "bounds {\n// persons: 9\npersons: 2\n}\nassert true",
                "bounds { persons : 2 }",
            ),
            (
                "bounds { events: 1, // persons: 5\npersons: 2 }\nassert true",
                "bounds { events : 1 , persons : 2 }",
            ),
            (
                "bounds\n// persons: 7\n{ persons: 3 }\nassert true",
                "bounds { persons : 3 }",
            ),
            // and so is one beside it,
            (
                "bounds events 1 // persons: 2\nassert true",
                "bounds events 1",
            ),
            (
                "bounds { events: 1 }; // persons: 2\nassert true",
                "bounds { events : 1 }",
            ),
            // and one below it, whatever stands between or after.
            (
                "bounds events 1\n// persons: 2\nassert true",
                "bounds events 1",
            ),
            (
                "bounds { events: 1 }\n\n// persons: 2\n\nassert true",
                "bounds { events : 1 }",
            ),
            (
                "bounds { events: 1 }\n// persons: 2\nassuming fair",
                "bounds { events : 1 }",
            ),
            (
                "assert true\nbounds { events: 1 }\n/// persons: 2",
                "bounds { events : 1 }",
            ),
            // A string is recorded whole, and the `;` that ends the field is
            // left out.
            (
                "bounds { label: \"persons: 2\" }\nassert true",
                "bounds { label : \"persons: 2\" }",
            ),
            (
                "bounds persons 2 events 1;\nassert true",
                "bounds persons 2 events 1",
            ),
        ];
        let recorded: Vec<Option<String>> = rows
            .iter()
            .map(|(lines, _)| {
                let parsed = parse_body(&format!("verify V {{\n{lines}\n}}"));
                first_decl(&parsed).fields.get("bounds").cloned()
            })
            .collect();
        let expected: Vec<Option<String>> = rows
            .iter()
            .map(|(_, tokens)| Some((*tokens).to_owned()))
            .collect();
        assert_eq!(recorded, expected);
    }

    #[test]
    fn an_empty_statement_in_a_verify_block_is_not_an_assertion() {
        let parsed = parse_body("verify V { assert true;; assert false; }");
        assert_eq!(
            first_decl(&parsed).assertions,
            [Expr::Bool(true), Expr::Bool(false)]
        );
    }

    /// The assertions of a verify block of these lines, which parses without
    /// a diagnostic.
    fn verify_assertions(lines: &str) -> Vec<String> {
        let parsed = parse_body(&format!("verify V {{\n{lines}\n}}"));
        first_decl(&parsed).assertions.iter().map(shown).collect()
    }

    /// The assertion an `assert` line with this expression gives.
    fn asserted(expression: &str) -> String {
        verify_assertions(&format!("assert {expression}"))
            .pop()
            .expect("an assertion")
    }

    const FOR_EVERY: &str = "for every x in {1, 2}: x == 3";

    fn undecided(text: &str) -> String {
        format!("undecided: {text}")
    }

    #[test]
    fn a_verify_field_ends_where_the_next_field_begins() {
        // The word `assert` begins a field wherever it stands.
        assert_eq!(
            verify_assertions("assert true or 1 == 2 assert false"),
            [asserted("true or 1 == 2"), asserted("false")]
        );
        assert_eq!(
            verify_assertions("assuming fair assert true"),
            [asserted("true")]
        );
        // Another field word in the middle of a line does not end the field
        // it stands in, so this `for every` is part of a field that is not
        // read.
        assert_eq!(
            verify_assertions(&format!("assuming fair {FOR_EVERY}\nassert true")),
            [asserted("true")]
        );
        // Below a field that is not read, a line that begins with a field
        // word is a field of its own, although the line above it does not
        // end a statement.
        for unread in [
            "assuming fair +",
            "interpretations all_admissible,",
            "judgments all_lawful +",
            "choices all_lawful,",
            "bounds persons 2,",
        ] {
            for (line, kept) in [
                ("assert false", asserted("false")),
                (
                    "for_all x in {1, 2}: x == 3",
                    asserted("for_all x in {1, 2}: x == 3"),
                ),
                (
                    "exists x in {1, 2}: x == 3",
                    asserted("exists x in {1, 2}: x == 3"),
                ),
                (FOR_EVERY, undecided(FOR_EVERY)),
            ] {
                assert_eq!(
                    verify_assertions(&format!("{unread}\n{line}")),
                    [kept],
                    "`{unread}` then `{line}`"
                );
            }
        }
        // A closing parenthesis or bracket that closes nothing does not hide
        // the line below it.
        for unread in [
            "assuming fair_response(Court))",
            "interpretations all_admissible ]",
            "bounds persons 2 )",
            "assuming fair ) +",
        ] {
            assert_eq!(
                verify_assertions(&format!("{unread}\n{FOR_EVERY}\nassert true")),
                [undecided(FOR_EVERY), asserted("true")],
                "below `{unread}`"
            );
        }
        // Below a line that is read, a `for every` line is a field of its
        // own, although `for` continues a statement.
        assert_eq!(
            verify_assertions(&format!("assert true\n{FOR_EVERY}")),
            [asserted("true"), undecided(FOR_EVERY)]
        );
        assert_eq!(
            verify_assertions(&format!("for_all x in {{1, 2}}: x < 3\n{FOR_EVERY}")),
            [asserted("for_all x in {1, 2}: x < 3"), undecided(FOR_EVERY)]
        );
        assert_eq!(
            verify_assertions("assert true or x\nfor y"),
            [asserted("true or x"), undecided("for y")]
        );
    }

    /// First lines of a verify field that is read: complete, cut off, and
    /// with tokens left over after the expression.
    const READ_FIELD_LINES: &[&str] = &[
        "assert true",
        "assert true or x",
        "assert 1 == 1",
        "assert f(x)",
        "assert a.b",
        "assert 3 days",
        "assert operative P",
        "assert x in {1, 2}",
        "assert if c > 0 { a }",
        "for_all x in {1, 2}: x < 3",
        "exists x in {1, 2}: x == 2",
        "assert",
        "assert true or",
        "assert true or 1 +",
        "assert true or 1 ==",
        "assert true or not",
        "assert true or operative",
        "assert 1 ==",
        "assert f(x) =>",
        "for_all x in {1, 2}:",
        "for_all x in {1, 2}: true or 1 +",
        "for_all x in",
        "exists",
        "assert true )",
        "assert true ]",
        "assert true,",
        "assert true every x",
    ];

    /// First lines of a verify field that is not read: complete, ending in a
    /// token that cannot end a statement, and with a closer that closes
    /// nothing.
    const FIELD_LINES_NOT_READ: &[&str] = &[
        "assuming fair",
        "assuming fair +",
        "assuming fair )",
        "assuming fair ) +",
        "assuming fair_response(Court))",
        "assuming",
        "interpretations all_admissible",
        "interpretations all_admissible ]",
        "interpretations all_admissible,",
        "judgments all_lawful",
        "judgments all_lawful +",
        "choices all_lawful",
        "choices",
        "bounds persons 2",
        "bounds persons 2 )",
        "bounds persons 2,",
        "bounds",
    ];

    /// Lines that begin a field below any field.
    const LINES_THAT_BEGIN_A_FIELD: &[&str] = &[
        "assert false",
        "bounds persons 1",
        "assuming fair",
        "interpretations all_admissible",
        "judgments all_lawful",
        "choices all_lawful",
        FOR_EVERY,
    ];

    /// Lines that begin a field below a field that is not read, and can
    /// continue a field that is read.
    const QUANTIFIER_LINES: &[&str] = &["for_all y in {1}: y == 2", "exists y in {1}: y == 2"];

    #[test]
    fn a_verify_field_is_read_as_if_a_semicolon_stood_where_the_next_field_begins() {
        // What the parser makes of a verify block: its assertions, its
        // bounds and the messages of its diagnostics.
        let read = |lines: &str| {
            let parsed = parse_file(&wrap_module(&format!("verify V {{\n{lines}\n}}")));
            let decl = first_decl(&parsed);
            let assertions: Vec<String> = decl
                .assertions
                .iter()
                .map(|assertion| shown(assertion).trim_end_matches(';').trim_end().to_owned())
                .collect();
            let messages: Vec<String> = parsed
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect();
            (assertions, decl.fields.get("bounds").cloned(), messages)
        };
        let mut failures = Vec::new();
        let mut compare = |without: String, with: String| {
            let (read_without, read_with) = (read(&without), read(&with));
            if read_without != read_with {
                failures.push(format!(
                    "{without:?}\n    read:              {read_without:?}\n    with a semicolon:  {read_with:?}"
                ));
            }
        };
        for first in READ_FIELD_LINES {
            for next in LINES_THAT_BEGIN_A_FIELD {
                compare(format!("{first}\n{next}"), format!("{first};\n{next}"));
            }
            compare(
                format!("{first} assert false"),
                format!("{first}; assert false"),
            );
        }
        for first in FIELD_LINES_NOT_READ {
            for next in LINES_THAT_BEGIN_A_FIELD.iter().chain(QUANTIFIER_LINES) {
                compare(format!("{first}\n{next}"), format!("{first};\n{next}"));
            }
            compare(
                format!("{first} assert false"),
                format!("{first}; assert false"),
            );
        }
        assert!(
            failures.is_empty(),
            "{} blocks are not read as they are with a semicolon before the next field:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn an_assertion_cut_off_where_the_next_field_begins_is_e100_there() {
        for (cut_off, next) in [
            ("assert true or 1 +", "assert false"),
            ("assert true or 1 ==", "assert false"),
            ("assert true or not", "assert false"),
            ("assert 1 ==", "assert false"),
            ("for_all x in {1, 2}: true or 1 +", "assert false"),
            ("assert true or 1 +", "bounds persons 1"),
            ("assert true or 1 +", "assuming fair"),
            ("assert true or 1 +", "interpretations all_admissible"),
            ("assert true or 1 +", "judgments all_lawful"),
            ("assert true or 1 +", "choices all_lawful"),
            ("assert true or 1 +", FOR_EVERY),
        ] {
            for separator in ["\n    ", " "] {
                // A field word other than `assert` begins a field only when
                // it begins a line.
                if separator == " " && next != "assert false" {
                    continue;
                }
                let source = wrap_module(&format!("verify V {{\n{cut_off}{separator}{next}\n}}"));
                let parsed = parse_file(&source);
                let diagnostics: Vec<(DiagnosticCode, &str, Option<Span>)> = parsed
                    .diagnostics
                    .iter()
                    .map(|diagnostic| {
                        (
                            diagnostic.code,
                            diagnostic.message.as_str(),
                            diagnostic.primary_span,
                        )
                    })
                    .collect();
                // The diagnostic points at the place where the assertion was
                // cut off: the start of the next field.
                let at = source.find(next).expect("the next field") as u32;
                assert_eq!(
                    diagnostics,
                    [(
                        DiagnosticCode::E100,
                        "expected expression after operator",
                        Some(Span { start: at, end: at })
                    )],
                    "`{cut_off}` then `{next}`, separated by {separator:?}"
                );
            }
        }
    }

    #[test]
    fn a_quantifier_line_below_a_line_that_is_read_can_continue_it() {
        assert_eq!(
            verify_assertions("for_all x in {1, 2}:\n  exists y in {1, 2}: x == y"),
            [asserted("for_all x in {1, 2}: exists y in {1, 2}: x == y")]
        );
        assert_eq!(
            verify_assertions("assert true or 1 +\nfor_all x in {1, 2}: x == 3"),
            [asserted("true or 1 + for_all x in {1, 2}: x == 3")]
        );
        // When the line above is complete, the quantifier line is a field of
        // its own.
        assert_eq!(
            verify_assertions("assert true\nfor_all x in {1, 2}: x == 3"),
            [asserted("true"), asserted("for_all x in {1, 2}: x == 3")]
        );
        // An `assert` line is never the body of the quantifier above it.
        assert_eq!(
            verify_assertions("for_all x in {1, 2}:\nassert true"),
            [asserted("for_all x in {1, 2}:"), asserted("true")]
        );
    }

    #[test]
    fn a_field_word_after_a_complete_verify_expression_begins_the_next_field() {
        // A word before `:` is not an operand, so the expression has ended
        // and `assuming` begins a field, which is not read.
        assert_eq!(
            verify_assertions("assert true assuming: x"),
            [asserted("true")]
        );
        assert_eq!(
            verify_assertions("for_all x in {1, 2}: x < 3 assuming: y"),
            [asserted("for_all x in {1, 2}: x < 3")]
        );
        // Without the `:` the word is an operand: the one assertion is
        // `true` applied to it, not `true`.
        let applied = verify_assertions("assert true assuming x");
        assert_eq!(applied.len(), 1, "{applied:?}");
        assert!(applied[0].starts_with("Apply"), "{applied:?}");
    }

    /// The seven duration unit words.
    const DURATION_UNIT_WORDS: &[&str] = &[
        "days",
        "working_days",
        "counted_days",
        "calendar_days",
        "hours",
        "minutes",
        "seconds",
    ];

    #[test]
    fn text_after_the_expression_of_an_assert_joins_it_or_is_e100() {
        // The assertions of a verify block of these lines, and the messages
        // of its diagnostics.
        let read = |lines: &str| {
            let parsed = parse_file(&wrap_module(&format!("verify V {{\n{lines}\n}}")));
            let assertions: Vec<String> =
                first_decl(&parsed).assertions.iter().map(shown).collect();
            let messages: Vec<String> = parsed
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect();
            (assertions, messages)
        };
        // Text that begins with a word joins the `assert`: together they are
        // one assertion that cannot be decided, and there is no diagnostic.
        // A duration unit is a word like any other.
        let words = ["asert false", "x", "dayz", "working_days x"];
        for text in words.iter().chain(DURATION_UNIT_WORDS) {
            let lines = format!("assert 1 == 1\n{text}");
            assert_eq!(
                read(&lines),
                (vec![undecided(&lines)], Vec::new()),
                "{lines:?}"
            );
        }
        assert_eq!(
            read("assert true every x"),
            (vec![undecided("assert true every x")], Vec::new())
        );
        // Text that begins with anything else is rejected.
        for lines in [
            "assert true\n1 == 2",
            "assert true\n\"s\"",
            "assert 1 == 1\n-1 == 2",
            "assert true )",
        ] {
            assert_eq!(
                read(lines),
                (
                    vec![undecided(lines)],
                    vec!["unexpected tokens after verification assertion".to_owned()]
                ),
                "{lines:?}"
            );
        }
    }

    #[test]
    fn a_line_verify_does_not_decide_takes_with_it_what_continues_it() {
        // The first line ends in `:`, so the `assert` line below continues
        // it.
        assert_eq!(
            verify_assertions(
                "for every operative Filed(c):\n    assert not operative Formed(H)\nassert true"
            ),
            [
                undecided("for every operative Filed(c):\n    assert not operative Formed(H)"),
                asserted("true")
            ]
        );
        // `for` continues a statement.
        assert_eq!(
            verify_assertions(&format!("asert true\n{FOR_EVERY}")),
            [undecided(&format!("asert true\n{FOR_EVERY}"))]
        );
    }

    #[test]
    fn nothing_inside_the_brackets_of_a_verify_field_begins_a_field() {
        assert_eq!(
            verify_assertions("interpretations {\nassert false\n}\nassert true"),
            [asserted("true")]
        );
        assert_eq!(
            verify_assertions(&format!("assuming (fair\n{FOR_EVERY}\n)\nassert true")),
            [asserted("true")]
        );
        assert_eq!(
            verify_assertions("assert true or f(\nassert false\n)").len(),
            1
        );
    }

    #[test]
    fn an_assert_with_no_expression_is_an_assertion_that_cannot_be_decided() {
        assert_eq!(
            verify_assertions("assert\nassert true"),
            [undecided("assert"), asserted("true")]
        );
        assert_eq!(verify_assertions("assert"), [undecided("assert")]);
        assert_eq!(
            verify_assertions("assert; assert true"),
            [undecided("assert"), asserted("true")]
        );
        // `not` with nothing after it gives no expression either.
        assert_eq!(
            verify_assertions("assert not\nassert true"),
            [undecided("assert not"), asserted("true")]
        );
    }

    #[test]
    fn a_blank_or_comment_line_inside_a_condition_does_not_end_it() {
        // `between` stands between the first line of the guard and the line
        // that begins with `and`.
        let rule = |between: &str| {
            parse_body(&format!(
                "rule R : derive from \"cite\" {{\nwhen operative A\n{between}and operative B\nthen derive C\n}}"
            ))
        };
        let plain = rule("");
        assert!(
            matches!(
                first_decl(&plain).guard,
                Some(Expr::Binary { op: BinOp::And, .. })
            ),
            "{:?}",
            first_decl(&plain).guard
        );
        assert_eq!(first_decl(&plain).consequences.len(), 1);
        for between in [
            "// the second condition\n",
            "\n",
            "\n// the second condition\n\n",
        ] {
            assert_eq!(
                rule_statements(&rule(between)),
                rule_statements(&plain),
                "between the two lines: {between:?}"
            );
        }
        let body = |lines: &str| {
            let parsed = parse_body(&format!("query q() -> Bool {{\n{lines}\n}}"));
            first_query(&parsed).expr.clone().expect("a query body")
        };
        for (lines, one_line) in [
            (
                "require operative A\n// why\nand operative B\nreturn true",
                "require operative A and operative B\nreturn true",
            ),
            (
                "return operative A\n// why\nor operative B",
                "return operative A or operative B",
            ),
            ("return x\n// why\n.field", "return x.field"),
            (
                "return operative P\n// why\nin C",
                "return operative P in C",
            ),
        ] {
            assert_eq!(body(lines), body(one_line), "{lines:?}");
        }
    }

    /// Lines that begin with a token no statement begins with.
    const LINES_THAT_ONLY_CONTINUE: &[&str] = &[
        "and b", "or b", "in b", ".b", ", b", "-> b", "=> b", ": b", "= b", "| b", "< b",
    ];

    /// Lines that begin with a continuation a statement can begin with.
    const LINES_THAT_CAN_STAND_ALONE: &[&str] = &[
        "where b",
        "then b",
        "else b",
        "from b",
        "decides b",
        "occupied_by b",
        "establishes b",
        "using b",
        "for b",
        "rank b",
        "as b",
        "to b",
        "version b",
        "fuel b",
        "(b)",
        "{ b }",
        "[b]",
        "! b",
    ];

    #[test]
    fn below_a_blank_or_comment_line_only_a_token_no_statement_begins_with_continues() {
        // `asert a` is a line a verify block does not recognize. The parser
        // steps over it, and over what continues it, with the rule it has
        // for the end of every statement it does not read.
        let mut failures = Vec::new();
        let mut check = |lines: String, expected: Vec<String>| {
            let read = verify_assertions(&format!("{lines}\nassert true"));
            let expected = [expected, vec![asserted("true")]].concat();
            if read != expected {
                failures.push(format!(
                    "{lines:?}\n    read:     {read:?}\n    expected: {expected:?}"
                ));
            }
        };
        for between in ["\n", "// why\n", "\n// why\n\n"] {
            for line in LINES_THAT_ONLY_CONTINUE {
                let lines = format!("asert a\n{between}{line}");
                let joined = vec![undecided(&lines)];
                check(lines, joined);
            }
            for line in LINES_THAT_CAN_STAND_ALONE {
                check(
                    format!("asert a\n{between}{line}"),
                    vec![undecided("asert a"), undecided(line)],
                );
            }
        }
        // Directly below the line, each of them continues it.
        for line in LINES_THAT_ONLY_CONTINUE
            .iter()
            .chain(LINES_THAT_CAN_STAND_ALONE)
        {
            let lines = format!("asert a\n{line}");
            let joined = vec![undecided(&lines)];
            check(lines, joined);
        }
        assert!(
            failures.is_empty(),
            "{} lines are not joined or kept apart as they should be:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_blank_or_comment_line_does_not_end_an_expression_before_an_operator() {
        // The value a query returns, or its parse errors.
        let returned = |expression: &str| {
            let parsed = parse_file(&wrap_module(&format!(
                "query q() -> Bool {{\nreturn {expression}\n}}"
            )));
            if parsed.has_errors() {
                Err(format!("{:?}", parsed.diagnostics))
            } else {
                Ok(first_query(&parsed).expr.clone())
            }
        };
        let mut failures = Vec::new();
        for (left, right, one_line) in [
            ("a", "and b", "a and b"),
            ("a", "or b", "a or b"),
            ("a", "in b", "a in b"),
            ("a", ".b", "a.b"),
            ("a", "=> b", "a => b"),
            ("a", "< b", "a < b"),
            ("f(a", ", b)", "f(a, b)"),
            ("for_all x in d", ": x > 0", "for_all x in d: x > 0"),
        ] {
            for between in ["\n", "// why\n", "\n// why\n\n"] {
                let lines = format!("{left}\n{between}{right}");
                let read = returned(&lines);
                let expected = returned(one_line);
                if expected.is_err() || read != expected {
                    failures.push(format!(
                        "{lines:?}\n    read:     {read:?}\n    expected: {expected:?}"
                    ));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} expressions are cut short:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_blank_or_comment_line_still_ends_a_statement_before_a_line_that_can_stand_alone() {
        for between in ["\n", "// why\n"] {
            // A parenthesized expression is a statement of its own.
            let parsed = parse_body(&format!("calc f() -> Int {{\nx\n{between}(y)\n}}"));
            assert_eq!(
                first_fn(&parsed).expr,
                Some(Expr::Block(vec![
                    Expr::Ident("x".to_owned()),
                    Expr::Grouped(Box::new(Expr::Ident("y".to_owned()))),
                ])),
                "between the two lines: {between:?}"
            );
            // Two clauses of a judgment that the parser keeps as text.
            let parsed = parse_body(&format!(
                "judgment J(x: T) decides P(x) {{\nauthority A\noptions f(x)\n{between}for each y in ys:\nrequire operative R(y)\n}}"
            ));
            let clauses: Vec<&str> = first_decl(&parsed)
                .judgment
                .as_ref()
                .expect("a judgment")
                .unsupported_clauses
                .iter()
                .map(|clause| clause.trim())
                .collect();
            assert_eq!(
                clauses,
                ["options f(x)", "for each y in ys:\nrequire operative R(y)"],
                "between the two lines: {between:?}"
            );
            // The block of an `if` does not follow a blank or comment line.
            let parsed = parse_file(&wrap_module(&format!(
                "calc f(c: Int) -> Int {{\nif c > 0\n{between}{{ 1 }}\n}}"
            )));
            let messages: Vec<&str> = parsed
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect();
            assert_eq!(
                messages,
                ["expected `{` after the `if` condition"],
                "between the two lines: {between:?}"
            );
        }
    }

    #[test]
    fn the_next_part_of_a_form_is_read_across_a_blank_or_comment_line() {
        let returned = |expression: &str| {
            let parsed = parse_body(&format!("query q() -> Bool {{\nreturn {expression}\n}}"));
            first_query(&parsed).expr.clone().expect("a returned value")
        };
        for between in ["\n", "// why\n"] {
            for (first, next) in [
                ("if c > 0 { a }", "else { b }"),
                ("for_all x in {1, 2}", "{ p(x) }"),
                ("transaction", "{ a }"),
            ] {
                assert_eq!(
                    returned(&format!("{first}\n{between}{next}")),
                    returned(&format!("{first} {next}")),
                    "`{first}` then `{next}`, between them: {between:?}"
                );
            }
        }
    }

    #[test]
    fn a_clause_the_parser_keeps_as_text_goes_on_below_a_comment_line() {
        let parsed =
            parse_body("judgment J(x: T) decides P(x) {\nauthority A\noptions a\n// why\nand b\n}");
        let judgment = first_decl(&parsed).judgment.as_ref().expect("a judgment");
        let clauses: Vec<&str> = judgment
            .unsupported_clauses
            .iter()
            .map(|clause| clause.trim())
            .collect();
        assert_eq!(clauses, ["options a\n// why\nand b"]);
    }

    fn ident(name: &str) -> Expr {
        Expr::Ident(name.to_owned())
    }

    fn call(callee: &str, args: Vec<Expr>) -> Expr {
        Expr::Call {
            callee: callee.to_owned(),
            args,
        }
    }

    fn binary(op: BinOp, left: Expr, right: Expr) -> Expr {
        Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// `Rec { a: 1 }`: the name `Rec` applied to a block that holds the field
    /// `a`.
    fn rec() -> Expr {
        Expr::Apply {
            callee: Box::new(ident("Rec")),
            args: vec![Expr::Block(vec![Expr::NamedArg {
                name: "a".to_owned(),
                value: Box::new(Expr::Int(1)),
            }])],
        }
    }

    /// `if cond { 1 } else { 2 }`.
    fn one_or_two(cond: Expr) -> Expr {
        Expr::If {
            cond: Box::new(cond),
            then: Box::new(Expr::Int(1)),
            else_: Some(Box::new(Expr::Int(2))),
        }
    }

    /// `word x in domain { x > 0 }`.
    fn positive(word: &str, domain: Expr) -> Expr {
        Expr::Apply {
            callee: Box::new(ident(word)),
            args: vec![
                ident("x"),
                domain,
                binary(BinOp::Gt, ident("x"), Expr::Int(0)),
            ],
        }
    }

    fn messages(parsed: &Parse) -> Vec<&str> {
        parsed
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect()
    }

    /// The keyword and the name of each item of the module.
    fn item_names(parsed: &Parse) -> Vec<(&str, Option<&str>)> {
        parsed
            .module()
            .expect("a module")
            .items
            .iter()
            .map(|item| {
                let name = item.decl().and_then(|decl| decl.name.as_deref());
                (item.keyword(), name)
            })
            .collect()
    }

    /// The diagnostics of a `calc` whose body holds these lines, and the body.
    fn calc_body(lines: &str) -> (Vec<String>, Option<Expr>) {
        let parsed = parse_file(&wrap_module(&format!("calc f() -> Int {{\n{lines}\n}}")));
        let diagnostics = messages(&parsed).into_iter().map(str::to_owned).collect();
        (diagnostics, first_decl(&parsed).expr.clone())
    }

    /// Fails with every row whose lines, as the body of a `calc`, are not
    /// read without a diagnostic as the tree beside them.
    fn assert_calc_bodies(rows: &[(&str, Expr)]) {
        let mut failures = Vec::new();
        for (lines, tree) in rows {
            let read = calc_body(lines);
            let expected = (Vec::new(), Some(tree.clone()));
            if read != expected {
                failures.push(format!(
                    "{lines:?}\n    read:     {read:?}\n    expected: {expected:?}"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} bodies are not read as written:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_rule_keeps_its_own_body_whatever_its_basis_is() {
        let headers = [
            ("from Basis {", ident("Basis")),
            ("from Basis\n{", ident("Basis")),
            (
                "from Basis.Sub {",
                Expr::Field {
                    base: Box::new(ident("Basis")),
                    name: "Sub".to_owned(),
                },
            ),
            ("from Basis(x) {", call("Basis", vec![ident("x")])),
            ("from \"cite\" {", Expr::String("cite".to_owned())),
            // In the parentheses of a basis a name takes its block as it
            // does anywhere else.
            ("from Basis(Rec { a: 1 }) {", call("Basis", vec![rec()])),
        ];
        let guard = Expr::Apply {
            callee: Box::new(ident("operative")),
            args: vec![call("P", Vec::new())],
        };
        let consequence = ("derive".to_owned(), call("Q", Vec::new()));
        let mut failures = Vec::new();
        for (header, basis) in headers {
            let parsed = parse_file(&wrap_module(&format!(
                "rule R : derive {header}\nwhen operative P()\nthen derive Q()\n}}\nquery after() -> Int {{ return 1 }}"
            )));
            let rule = first_decl(&parsed);
            let read = (
                messages(&parsed),
                rule.source_basis.clone(),
                rule.guard.clone(),
                consequence_pairs(&rule.consequences),
                item_names(&parsed),
            );
            let expected = (
                Vec::new(),
                Some(basis),
                Some(guard.clone()),
                vec![consequence.clone()],
                vec![("rule", Some("R")), ("query", Some("after"))],
            );
            if read != expected {
                failures.push(format!(
                    "{header:?}\n    read:     {read:?}\n    expected: {expected:?}"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} rule headers lose the rule's body or the declaration after it:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn an_if_condition_that_ends_in_a_name_keeps_its_branch() {
        assert_calc_bodies(&[
            (
                "if a < b { 1 } else { 2 }",
                one_or_two(binary(BinOp::Lt, ident("a"), ident("b"))),
            ),
            ("if flag { 1 } else { 2 }", one_or_two(ident("flag"))),
            ("if flag\n{ 1 }\nelse { 2 }", one_or_two(ident("flag"))),
            (
                "if not flag { 1 } else { 2 }",
                one_or_two(Expr::Apply {
                    callee: Box::new(ident("not")),
                    args: vec![ident("flag")],
                }),
            ),
            (
                "if a.b == c { 1 } else { 2 }",
                one_or_two(binary(
                    BinOp::Eq,
                    Expr::Field {
                        base: Box::new(ident("a")),
                        name: "b".to_owned(),
                    },
                    ident("c"),
                )),
            ),
            (
                "if f(x) and flag { 1 } else { 2 }",
                one_or_two(binary(
                    BinOp::And,
                    call("f", vec![ident("x")]),
                    ident("flag"),
                )),
            ),
        ]);
    }

    #[test]
    fn an_if_whose_condition_is_a_name_is_complete_at_the_end_of_its_line() {
        let conditional = Expr::If {
            cond: Box::new(ident("flag")),
            then: Box::new(Expr::Int(1)),
            else_: None,
        };
        assert_calc_bodies(&[
            ("if flag { 1 }", conditional.clone()),
            (
                "if flag { 1 }\n2",
                Expr::Block(vec![conditional, Expr::Int(2)]),
            ),
        ]);
    }

    #[test]
    fn between_the_delimiters_of_a_condition_a_name_takes_its_block() {
        // Parentheses, the arguments of a call, brackets and braces. After
        // each closing delimiter the name `r` is the condition's last
        // operand again and leaves the block to the `if`.
        assert_calc_bodies(&[
            (
                "if (Rec { a: 1 }) == r { 1 } else { 2 }",
                one_or_two(binary(
                    BinOp::Eq,
                    Expr::Grouped(Box::new(rec())),
                    ident("r"),
                )),
            ),
            (
                "if total(Rec { a: 1 }) > 0 { 1 } else { 2 }",
                one_or_two(binary(BinOp::Gt, call("total", vec![rec()]), Expr::Int(0))),
            ),
            (
                "if total(Rec { a: 1 }) == r { 1 } else { 2 }",
                one_or_two(binary(BinOp::Eq, call("total", vec![rec()]), ident("r"))),
            ),
            (
                "if [Rec { a: 1 }, b] == r { 1 } else { 2 }",
                one_or_two(binary(
                    BinOp::Eq,
                    Expr::Apply {
                        callee: Box::new(ident("interval")),
                        args: vec![rec(), ident("b")],
                    },
                    ident("r"),
                )),
            ),
            (
                "if {Rec { a: 1 }} == r { 1 } else { 2 }",
                one_or_two(binary(BinOp::Eq, Expr::Set(vec![rec()]), ident("r"))),
            ),
            // The braces of a quantifier's body and of a `transaction`.
            (
                "if for_all x in Dom { Rec { a: 1 } == x } { 1 } else { 2 }",
                one_or_two(Expr::Apply {
                    callee: Box::new(ident("for_all")),
                    args: vec![
                        ident("x"),
                        ident("Dom"),
                        binary(BinOp::Eq, rec(), ident("x")),
                    ],
                }),
            ),
            (
                "if transaction { Rec { a: 1 } } { 1 } else { 2 }",
                one_or_two(Expr::Apply {
                    callee: Box::new(ident("transaction")),
                    args: vec![rec()],
                }),
            ),
        ]);
    }

    #[test]
    fn a_quantifier_domain_that_ends_in_a_name_leaves_the_block_to_the_quantifier() {
        assert_calc_bodies(&[
            (
                "for_all x in Dom { x > 0 }",
                positive("for_all", ident("Dom")),
            ),
            (
                "exists x in Dom { x > 0 }",
                positive("exists", ident("Dom")),
            ),
            (
                "for_all x in Dom\n{ x > 0 }",
                positive("for_all", ident("Dom")),
            ),
            ("for_all x in Dom: x > 0", positive("for_all", ident("Dom"))),
            (
                "for_all x in a.b { x > 0 }",
                positive(
                    "for_all",
                    Expr::Field {
                        base: Box::new(ident("a")),
                        name: "b".to_owned(),
                    },
                ),
            ),
            (
                "for_all x in members(Rec { a: 1 }) { x > 0 }",
                positive("for_all", call("members", vec![rec()])),
            ),
        ]);
    }

    #[test]
    fn a_judgment_header_that_ends_in_a_name_keeps_the_body() {
        let headers = [
            ("decides Liable {", ident("Liable"), None),
            ("decides Liable\n{", ident("Liable"), None),
            (
                "decides Liable from Court {",
                ident("Liable"),
                Some(ident("Court")),
            ),
            (
                "decides Liable\nfrom Court\n{",
                ident("Liable"),
                Some(ident("Court")),
            ),
            (
                "decides Liable(person) from Court {",
                call("Liable", vec![ident("person")]),
                Some(ident("Court")),
            ),
            // In the parentheses of a header expression a name takes its
            // block as it does anywhere else.
            (
                "decides Liable(Rec { a: 1 }) from records(Rec { a: 1 }) {",
                call("Liable", vec![rec()]),
                Some(call("records", vec![rec()])),
            ),
        ];
        let mut failures = Vec::new();
        for (header, decides, source) in headers {
            let parsed = parse_file(&wrap_module(&format!(
                "judgment J(person: NaturalPerson) {header}\nauthority Court\nwhere person == person\n}}\nquery after() -> Int {{ return 1 }}"
            )));
            let decl = first_decl(&parsed);
            let judgment = decl.judgment.as_ref().expect("a judgment");
            let read = (
                messages(&parsed),
                judgment.decides.clone(),
                decl.source_basis.clone(),
                judgment.authority.clone(),
                judgment.constraints.clone(),
                item_names(&parsed),
            );
            let expected = (
                Vec::new(),
                decides,
                source,
                Some(JudgmentAuthorityAst::Actor(ident("Court"))),
                vec![binary(BinOp::Eq, ident("person"), ident("person"))],
                vec![("judgment", Some("J")), ("query", Some("after"))],
            );
            if read != expected {
                failures.push(format!(
                    "{header:?}\n    read:     {read:?}\n    expected: {expected:?}"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} judgment headers lose the body or the declaration after it:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn an_observation_header_that_ends_in_a_name_keeps_the_body() {
        let headers = [
            ("establishes Seen {", ident("Seen"), None),
            ("establishes Seen\n{", ident("Seen"), None),
            (
                "establishes Seen from Registry {",
                ident("Seen"),
                Some(ident("Registry")),
            ),
            (
                "establishes Seen\nfrom Registry\n{",
                ident("Seen"),
                Some(ident("Registry")),
            ),
            (
                "establishes Seen(record.subject) from Registry {",
                call(
                    "Seen",
                    vec![Expr::Field {
                        base: Box::new(ident("record")),
                        name: "subject".to_owned(),
                    }],
                ),
                Some(ident("Registry")),
            ),
        ];
        let field = |name: &str| Expr::Field {
            base: Box::new(ident("record")),
            name: name.to_owned(),
        };
        let mut failures = Vec::new();
        for (header, establishes, source) in headers {
            let parsed = parse_file(&wrap_module(&format!(
                "observation O(record: Certificate) {header}\nauthenticate record.signed == true\nrequire record.count > 0\n}}\nquery after() -> Int {{ return 1 }}"
            )));
            let decl = first_decl(&parsed);
            let observation = decl.observation.as_ref().expect("an observation");
            let read = (
                messages(&parsed),
                observation.establishes.clone(),
                decl.source_basis.clone(),
                observation.validators.clone(),
                item_names(&parsed),
            );
            let expected = (
                Vec::new(),
                establishes,
                source,
                vec![
                    binary(BinOp::Eq, field("signed"), Expr::Bool(true)),
                    binary(BinOp::Gt, field("count"), Expr::Int(0)),
                ],
                vec![("observation", Some("O")), ("query", Some("after"))],
            );
            if read != expected {
                failures.push(format!(
                    "{header:?}\n    read:     {read:?}\n    expected: {expected:?}"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} observation headers lose the body or the declaration after it:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_name_followed_by_a_brace_takes_the_block_where_no_construct_claims_it() {
        let positive_c = || binary(BinOp::Gt, ident("c"), Expr::Int(0));
        assert_calc_bodies(&[
            // A record constructor in a `return`, as a statement of its own,
            // as an argument, as each branch of an `if`, and in the body of
            // a quantifier.
            ("return Rec { a: 1 }", rec()),
            ("Rec { a: 1 }", rec()),
            ("g(Rec { a: 1 })", call("g", vec![rec()])),
            (
                "if c > 0 { Rec { a: 1 } } else { Rec { a: 1 } }",
                Expr::If {
                    cond: Box::new(positive_c()),
                    then: Box::new(rec()),
                    else_: Some(Box::new(rec())),
                },
            ),
            (
                "for_all x in {1, 2}: Rec { a: 1 } == x",
                Expr::Apply {
                    callee: Box::new(ident("for_all")),
                    args: vec![
                        ident("x"),
                        Expr::Set(vec![Expr::Int(1), Expr::Int(2)]),
                        binary(BinOp::Eq, rec(), ident("x")),
                    ],
                },
            ),
            // After the condition of an `if` among the arguments of a call,
            // the next argument is read in the ordinary way.
            (
                "g(if c > 0 { 1 } else { 2 }, Rec { a: 1 })",
                call("g", vec![one_or_two(positive_c()), rec()]),
            ),
        ]);
        // In the guard and the consequence of a rule, after its basis.
        for basis in ["\"cite\"", "Basis"] {
            let parsed = parse_file(&wrap_module(&format!(
                "rule R : derive from {basis} {{\nwhen Rec {{ a: 1 }} == r\nthen derive Q(Rec {{ a: 1 }})\n}}"
            )));
            let rule = first_decl(&parsed);
            assert_eq!(
                (
                    messages(&parsed),
                    rule.guard.clone(),
                    consequence_pairs(&rule.consequences)
                ),
                (
                    Vec::new(),
                    Some(binary(BinOp::Eq, rec(), ident("r"))),
                    vec![("derive".to_owned(), call("Q", vec![rec()]))]
                ),
                "from {basis}"
            );
        }
    }

    #[test]
    fn each_construct_gets_its_own_block_when_one_header_expression_holds_another() {
        let apply = |callee: &str, args: Vec<Expr>| Expr::Apply {
            callee: Box::new(ident(callee)),
            args,
        };
        let choice = |cond: &str, then: &str, otherwise: &str| Expr::If {
            cond: Box::new(ident(cond)),
            then: Box::new(ident(then)),
            else_: Some(Box::new(ident(otherwise))),
        };
        let p_of_x = || call("p", vec![ident("x")]);
        assert_calc_bodies(&[
            // The condition of an `if` begins with an `if`.
            (
                "if if a { b } else { c } == d { 1 } else { 2 }",
                one_or_two(binary(BinOp::Eq, choice("a", "b", "c"), ident("d"))),
            ),
            // The word `transaction` in a condition takes the first block as
            // its steps: it is a form with a block of its own, not a name.
            (
                "if transaction { a } { 1 } else { 2 }",
                one_or_two(apply("transaction", vec![ident("a")])),
            ),
            // A quantifier in a condition takes the first block as its body.
            (
                "if for_all x in Dom { p(x) } { 1 } else { 2 }",
                one_or_two(apply("for_all", vec![ident("x"), ident("Dom"), p_of_x()])),
            ),
            // The domain of a quantifier is an `if`.
            (
                "for_all x in if c { Dom } else { Other } { p(x) }",
                apply(
                    "for_all",
                    vec![ident("x"), choice("c", "Dom", "Other"), p_of_x()],
                ),
            ),
            // The condition of an `if` among the arguments of a call ends in
            // a name, and the argument after it is a record.
            (
                "g(if flag { 1 } else { 2 }, Rec { a: 1 })",
                call("g", vec![one_or_two(ident("flag")), rec()]),
            ),
        ]);
    }

    const IF_NEEDS_A_BRANCH: &str = "expected `{` after the `if` condition";
    const ELSE_NEEDS_A_BRANCH: &str = "expected `{` after `else`";

    #[test]
    fn an_if_or_an_else_with_no_branch_is_e100() {
        let positive_c = || Box::new(binary(BinOp::Gt, ident("c"), Expr::Int(0)));
        let no_branch = || Expr::Block(Vec::new());
        // `if c > 0 { 1 } else` with nothing as the branch of the `else`.
        let cut_off = || Expr::If {
            cond: positive_c(),
            then: Box::new(Expr::Int(1)),
            else_: Some(Box::new(no_branch())),
        };
        // The diagnostics and the body of a `calc` written on one line, so
        // that its closing brace follows the text directly.
        let on_one_line = |text: &str| {
            let parsed = parse_file(&wrap_module(&format!("calc f() -> Int {{ {text} }}")));
            let diagnostics: Vec<(DiagnosticCode, String)> = parsed
                .diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.code, diagnostic.message.clone()))
                .collect();
            (diagnostics, first_decl(&parsed).expr.clone())
        };
        assert_eq!(
            on_one_line("if c > 0"),
            (
                vec![(DiagnosticCode::E100, IF_NEEDS_A_BRANCH.to_owned())],
                Some(Expr::If {
                    cond: positive_c(),
                    then: Box::new(no_branch()),
                    else_: None,
                })
            )
        );
        assert_eq!(
            on_one_line("if c > 0 { 1 } else"),
            (
                vec![(DiagnosticCode::E100, ELSE_NEEDS_A_BRANCH.to_owned())],
                Some(cut_off())
            )
        );
        // After an `else` that ends its statement, the next line is a
        // statement of its own and not the branch.
        let mut failures = Vec::new();
        for (lines, below) in [
            ("if c > 0 { 1 } else", None),
            ("if c > 0 { 1 } else\n2", Some(Expr::Int(2))),
            ("if c > 0 { 1 } else;\n2", Some(Expr::Int(2))),
            (
                "if c > 0 { 1 } else\n\n{ 2 }",
                Some(Expr::Set(vec![Expr::Int(2)])),
            ),
            (
                "if c > 0 { 1 } else\n// why\n{ 2 }",
                Some(Expr::Set(vec![Expr::Int(2)])),
            ),
            (
                "if c > 0 { 1 } else\nif c > 1 { 1 } else { 2 }",
                Some(one_or_two(binary(BinOp::Gt, ident("c"), Expr::Int(1)))),
            ),
        ] {
            let read = calc_body(lines);
            let body = match below {
                Some(statement) => Expr::Block(vec![cut_off(), statement]),
                None => cut_off(),
            };
            let expected = (vec![ELSE_NEEDS_A_BRANCH.to_owned()], Some(body));
            if read != expected {
                failures.push(format!(
                    "{lines:?}\n    read:     {read:?}\n    expected: {expected:?}"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} bodies with an `else` that has no branch are not rejected as they should be:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_branch_without_braces_on_the_line_of_its_if_or_else_is_accepted() {
        let positive_c = || Box::new(binary(BinOp::Gt, ident("c"), Expr::Int(0)));
        let not = |name: &str| Expr::Apply {
            callee: Box::new(ident("not")),
            args: vec![ident(name)],
        };
        assert_calc_bodies(&[
            (
                "if c > 0 { 1 } else 2",
                one_or_two(binary(BinOp::Gt, ident("c"), Expr::Int(0))),
            ),
            (
                "if c > 0 not a else not b",
                Expr::If {
                    cond: positive_c(),
                    then: Box::new(not("a")),
                    else_: Some(Box::new(not("b"))),
                },
            ),
            // `{` continues a statement, so the block of an `else` can stand
            // on the next line, and so can an `else` below its `if`.
            (
                "if c > 0 { 1 } else\n{ 2 }",
                one_or_two(binary(BinOp::Gt, ident("c"), Expr::Int(0))),
            ),
            (
                "if c > 0 { 1 }\nelse { 2 }",
                one_or_two(binary(BinOp::Gt, ident("c"), Expr::Int(0))),
            ),
            (
                "if c > 0 { 1 } else if c > 1 { 1 } else { 2 }",
                Expr::If {
                    cond: positive_c(),
                    then: Box::new(Expr::Int(1)),
                    else_: Some(Box::new(one_or_two(binary(
                        BinOp::Gt,
                        ident("c"),
                        Expr::Int(1),
                    )))),
                },
            ),
        ]);
    }

    #[test]
    fn a_word_that_cannot_begin_an_expression_is_not_an_item_between_braces() {
        // `return`, `when` and `then` cannot begin an expression. Between the
        // braces of an expression such a word is not kept as a name: the
        // closing brace is expected where the word stands.
        for (body, word) in [
            ("if c > 0 { return 1 } else { return 2 }", "return"),
            ("if c > 0 { when 1 } else { 2 }", "when"),
            ("pick({ then })", "then"),
        ] {
            let source = wrap_module(&format!("calc f(c: Int) -> Int {{\n{body}\n}}"));
            let parsed = parse_file(&source);
            let first = parsed.diagnostics.first().map(|diagnostic| {
                (
                    diagnostic.code,
                    diagnostic.message.as_str(),
                    diagnostic.primary_span,
                )
            });
            let start = source.find(word).expect("the word") as u32;
            let end = start + word.len() as u32;
            assert_eq!(
                first,
                Some((
                    DiagnosticCode::E100,
                    "expected `}`",
                    Some(Span { start, end })
                )),
                "{body:?}"
            );
        }
    }

    const RULE_NEEDS_A_BODY: &str = "expected `{` to begin the rule body";
    const VERIFY_NEEDS_A_BLOCK: &str = "expected `{` to begin the verify block";
    const FUNCTION_NEEDS_A_BODY: &str = "expected `{` to begin the function body";
    const NEEDS_A_NAME: &str = "expected a name";
    const NEEDS_A_TYPE: &str = "expected a type";
    const NEEDS_A_RULE_KIND: &str = "expected rule kind";
    const NEEDS_A_NAME_PART: &str = "expected identifier";
    const TYPE_NEEDS_AN_EQUALS_SIGN: &str = "expected `=` in type declaration";
    const STOPS_SHORT: &str = "expected the rest of the declaration";
    const NEEDS_A_BLOCK: &str = "expected `{`";
    const NEEDS_ESTABLISHES: &str = "expected `establishes`";
    const NEEDS_DECIDES: &str = "expected `decides`";
    const NEEDS_ONE_BINDER: &str = "observation requires exactly one typed record binder";

    /// Declarations cut short: the line, the name the parser gives the
    /// declaration, and its diagnostics. The first rows end after a header,
    /// the next ones hold a keyword and nothing else, and the ones after
    /// those end in `:`, `->`, `=` or `.`, after which the line rule reads
    /// on. Then come type declarations with no definition, the four kinds
    /// whose body is a block of clauses or fields, and declarations with no
    /// name.
    const TRUNCATED_HEADERS: &[(&str, Option<&str>, &[&str])] = &[
        ("proposition P", Some("P"), &[]),
        ("entity E", Some("E"), &[]),
        ("office O", Some("O"), &[]),
        ("duty D", Some("D"), &[]),
        ("power W", Some("W"), &[]),
        ("scenario S", Some("S"), &[]),
        ("clause C", Some("C"), &[]),
        ("decision D2", Some("D2"), &[]),
        ("legal_act L", Some("L"), &[]),
        ("effect F", Some("F"), &[]),
        ("interpretation_family I", Some("I"), &[]),
        ("conflict_doctrine X", Some("X"), &[]),
        ("source S2", Some("S2"), &[]),
        (
            "rule R : derive from \"c\"",
            Some("R"),
            &[RULE_NEEDS_A_BODY],
        ),
        ("rule R : derive", Some("R"), &[RULE_NEEDS_A_BODY]),
        ("rule R", Some("R"), &[RULE_NEEDS_A_BODY]),
        ("verify V", Some("V"), &[VERIFY_NEEDS_A_BLOCK]),
        ("query Q() -> Bool", Some("Q"), &[]),
        ("query Q()", Some("Q"), &[]),
        ("calc f(a: Int) -> Int", Some("f"), &[FUNCTION_NEEDS_A_BODY]),
        ("fn g(a: Int) -> Int", Some("g"), &[FUNCTION_NEEDS_A_BODY]),
        ("for_all x", Some("x"), &[]),
        ("exists y", Some("y"), &[]),
        // A header keyword with no value in its statement.
        (
            "jurisdiction",
            None,
            &["expected a value after `jurisdiction`"],
        ),
        (
            "effective_at",
            None,
            &["expected a value after `effective_at`"],
        ),
        (
            "recorded_at",
            None,
            &["expected a value after `recorded_at`"],
        ),
        (
            "outside_scope",
            None,
            &["expected a value after `outside_scope`"],
        ),
        (
            "source_snapshot",
            None,
            &["expected a value after `source_snapshot`"],
        ),
        (
            "source_manifest",
            None,
            &["expected a value after `source_manifest`"],
        ),
        ("import M version \"1\"", Some("M"), &[]),
        ("import M", Some("M"), &[]),
        ("type T = Int", Some("T"), &[]),
        ("nomination A for B rank 1", Some("A"), &[]),
        // A keyword that reads a name, with nothing after it on its line.
        ("proposition", None, &[NEEDS_A_NAME]),
        ("entity", None, &[NEEDS_A_NAME]),
        ("office", None, &[NEEDS_A_NAME]),
        ("duty", None, &[NEEDS_A_NAME]),
        ("power", None, &[NEEDS_A_NAME]),
        ("scenario", None, &[NEEDS_A_NAME]),
        ("clause", None, &[NEEDS_A_NAME]),
        ("decision", None, &[NEEDS_A_NAME]),
        ("legal_act", None, &[NEEDS_A_NAME]),
        ("effect", None, &[NEEDS_A_NAME]),
        ("interpretation_family", None, &[NEEDS_A_NAME]),
        ("conflict_doctrine", None, &[NEEDS_A_NAME]),
        ("for_all", None, &[NEEDS_A_NAME]),
        ("exists", None, &[NEEDS_A_NAME]),
        ("nomination", None, &[NEEDS_A_NAME]),
        ("type", None, &[NEEDS_A_NAME]),
        ("record_type", None, &[NEEDS_A_NAME]),
        ("evidence_type", None, &[NEEDS_A_NAME]),
        ("observation", None, &[NEEDS_A_NAME]),
        ("judgment", None, &[NEEDS_A_NAME]),
        ("source", None, &["expected source name"]),
        ("import", None, &["expected import name"]),
        ("rule", None, &["expected rule name"]),
        ("verify", None, &["expected verification name"]),
        ("query", None, &["expected query name"]),
        ("query automatic", None, &["expected query name"]),
        ("calc", None, &["expected function name"]),
        ("fn", None, &["expected function name"]),
        // A header that ends in `:`.
        ("entity E :", Some("E"), &[NEEDS_A_TYPE]),
        ("rule R :", Some("R"), &[NEEDS_A_RULE_KIND]),
        ("rule R(x: T) :", Some("R"), &[NEEDS_A_RULE_KIND]),
        ("proposition P :", Some("P"), &[STOPS_SHORT]),
        ("office O :", Some("O"), &[STOPS_SHORT]),
        ("duty D :", Some("D"), &[STOPS_SHORT]),
        ("power W :", Some("W"), &[STOPS_SHORT]),
        ("scenario S :", Some("S"), &[STOPS_SHORT]),
        ("clause C :", Some("C"), &[STOPS_SHORT]),
        ("decision D2 :", Some("D2"), &[STOPS_SHORT]),
        ("legal_act L :", Some("L"), &[STOPS_SHORT]),
        ("effect F :", Some("F"), &[STOPS_SHORT]),
        ("interpretation_family I :", Some("I"), &[STOPS_SHORT]),
        ("conflict_doctrine X :", Some("X"), &[STOPS_SHORT]),
        ("source S2 :", Some("S2"), &[STOPS_SHORT]),
        ("verify V :", Some("V"), &[VERIFY_NEEDS_A_BLOCK]),
        ("query Q() :", Some("Q"), &[STOPS_SHORT]),
        ("calc f(a: Int) :", Some("f"), &[FUNCTION_NEEDS_A_BODY]),
        ("fn g(a: Int) :", Some("g"), &[FUNCTION_NEEDS_A_BODY]),
        ("for_all x in D :", Some("x"), &[STOPS_SHORT]),
        ("exists y in D :", Some("y"), &[STOPS_SHORT]),
        ("jurisdiction Test :", None, &[STOPS_SHORT]),
        ("effective_at 2026-01-01 :", None, &[STOPS_SHORT]),
        ("recorded_at 2026-01-01T00:00:00Z :", None, &[STOPS_SHORT]),
        ("outside_scope { everything_else } :", None, &[STOPS_SHORT]),
        ("import M :", Some("M"), &[STOPS_SHORT]),
        ("nomination A for B rank 1 :", Some("A"), &[STOPS_SHORT]),
        // A header that ends in `->`.
        ("decision D2(x: Int) ->", Some("D2"), &[NEEDS_A_TYPE]),
        ("query Q() ->", Some("Q"), &[NEEDS_A_TYPE]),
        ("query automatic Q() ->", Some("Q"), &[NEEDS_A_TYPE]),
        ("calc f(a: Int) ->", Some("f"), &[NEEDS_A_TYPE]),
        ("fn g(a: Int) ->", Some("g"), &[NEEDS_A_TYPE]),
        ("proposition P(x: Int) ->", Some("P"), &[STOPS_SHORT]),
        ("entity E ->", Some("E"), &[STOPS_SHORT]),
        ("office O ->", Some("O"), &[STOPS_SHORT]),
        ("duty D ->", Some("D"), &[STOPS_SHORT]),
        ("power W ->", Some("W"), &[STOPS_SHORT]),
        ("scenario S ->", Some("S"), &[STOPS_SHORT]),
        ("clause C ->", Some("C"), &[STOPS_SHORT]),
        ("legal_act L ->", Some("L"), &[STOPS_SHORT]),
        ("effect F ->", Some("F"), &[STOPS_SHORT]),
        ("interpretation_family I ->", Some("I"), &[STOPS_SHORT]),
        ("conflict_doctrine X ->", Some("X"), &[STOPS_SHORT]),
        ("source S2 ->", Some("S2"), &[STOPS_SHORT]),
        ("rule R ->", Some("R"), &[RULE_NEEDS_A_BODY]),
        ("verify V ->", Some("V"), &[VERIFY_NEEDS_A_BLOCK]),
        ("for_all x in D ->", Some("x"), &[STOPS_SHORT]),
        ("exists y in D ->", Some("y"), &[STOPS_SHORT]),
        ("jurisdiction Test ->", None, &[STOPS_SHORT]),
        ("effective_at 2026-01-01 ->", None, &[STOPS_SHORT]),
        ("recorded_at 2026-01-01T00:00:00Z ->", None, &[STOPS_SHORT]),
        ("outside_scope { everything_else } ->", None, &[STOPS_SHORT]),
        ("import M ->", Some("M"), &[STOPS_SHORT]),
        ("nomination A for B rank 1 ->", Some("A"), &[STOPS_SHORT]),
        // A header that ends in `=`, and a sum type that ends in `|`.
        ("type T =", Some("T"), &[NEEDS_A_TYPE]),
        ("type T = A |", Some("T"), &[NEEDS_A_TYPE]),
        ("power W =", Some("W"), &[STOPS_SHORT]),
        ("jurisdiction Test =", None, &[STOPS_SHORT]),
        // A header that ends in the `.` of a qualified name.
        ("entity E .", Some("E"), &[NEEDS_A_NAME_PART]),
        ("proposition P .", Some("P"), &[NEEDS_A_NAME_PART]),
        ("power W .", Some("W"), &[NEEDS_A_NAME_PART]),
        ("import M .", Some("M"), &[NEEDS_A_NAME_PART]),
        ("source S2 .", Some("S2"), &[NEEDS_A_NAME_PART]),
        ("entity E : A .", Some("E"), &[NEEDS_A_NAME_PART]),
        ("type T = A .", Some("T"), &[NEEDS_A_NAME_PART]),
        ("query Q() -> A .", Some("Q"), &[NEEDS_A_NAME_PART]),
        (
            "calc f(a: Int) -> A .",
            Some("f"),
            &[NEEDS_A_NAME_PART, FUNCTION_NEEDS_A_BODY],
        ),
        ("rule R .", Some("R"), &[RULE_NEEDS_A_BODY]),
        ("jurisdiction Test .", None, &[STOPS_SHORT]),
        // A type declaration with no `=`, or with no type after it, and a
        // bracket that the line leaves open.
        ("type T", Some("T"), &[TYPE_NEEDS_AN_EQUALS_SIGN]),
        ("type T {", Some("T"), &[TYPE_NEEDS_AN_EQUALS_SIGN]),
        ("type T (", Some("T"), &[TYPE_NEEDS_AN_EQUALS_SIGN]),
        ("type T { a: Int }", Some("T"), &[TYPE_NEEDS_AN_EQUALS_SIGN]),
        ("type T :", Some("T"), &[TYPE_NEEDS_AN_EQUALS_SIGN]),
        ("type T ->", Some("T"), &[TYPE_NEEDS_AN_EQUALS_SIGN]),
        ("type T = {", Some("T"), &[NEEDS_A_TYPE]),
        ("type T = 5", Some("T"), &[NEEDS_A_TYPE]),
        // A record type or an evidence type with no block of fields.
        ("record_type R", Some("R"), &[NEEDS_A_BLOCK]),
        ("record_type R :", Some("R"), &[NEEDS_A_BLOCK]),
        ("record_type R ->", Some("R"), &[NEEDS_A_BLOCK]),
        ("record_type R =", Some("R"), &[NEEDS_A_BLOCK]),
        ("record_type R(x: Int) :", Some("R"), &[NEEDS_A_BLOCK]),
        ("evidence_type V", Some("V"), &[NEEDS_A_BLOCK]),
        ("evidence_type V :", Some("V"), &[NEEDS_A_BLOCK]),
        ("evidence_type V ->", Some("V"), &[NEEDS_A_BLOCK]),
        ("evidence_type V =", Some("V"), &[NEEDS_A_BLOCK]),
        // An observation whose header stops before `establishes`, before
        // its proposition or its source, or before its block.
        (
            "observation O",
            Some("O"),
            &[NEEDS_ONE_BINDER, NEEDS_ESTABLISHES],
        ),
        ("observation O(r: Int)", Some("O"), &[NEEDS_ESTABLISHES]),
        ("observation O(r: Int) :", Some("O"), &[NEEDS_ESTABLISHES]),
        ("observation O(r: Int) ->", Some("O"), &[NEEDS_ESTABLISHES]),
        ("observation O(r: Int) =", Some("O"), &[NEEDS_ESTABLISHES]),
        (
            "observation O :",
            Some("O"),
            &[NEEDS_ONE_BINDER, NEEDS_ESTABLISHES],
        ),
        (
            "observation O(r: Int) establishes",
            Some("O"),
            &["expected observation proposition"],
        ),
        (
            "observation O(r: Int) establishes P(r)",
            Some("O"),
            &[NEEDS_A_BLOCK],
        ),
        (
            "observation O(r: Int) establishes P(r) from",
            Some("O"),
            &["expected observation source"],
        ),
        (
            "observation O(r: Int) establishes P(r) :",
            Some("O"),
            &[NEEDS_A_BLOCK],
        ),
        // A judgment whose header stops before `decides`, before its
        // proposition or its source, or before its block.
        ("judgment J", Some("J"), &[NEEDS_DECIDES]),
        ("judgment J(x: Int)", Some("J"), &[NEEDS_DECIDES]),
        ("judgment J(x: Int) :", Some("J"), &[NEEDS_DECIDES]),
        ("judgment J(x: Int) ->", Some("J"), &[NEEDS_DECIDES]),
        ("judgment J(x: Int) =", Some("J"), &[NEEDS_DECIDES]),
        ("judgment J :", Some("J"), &[NEEDS_DECIDES]),
        (
            "judgment J(x: Int) decides",
            Some("J"),
            &["expected the proposition decided by the judgment"],
        ),
        (
            "judgment J(x: Int) decides P(x)",
            Some("J"),
            &[NEEDS_A_BLOCK],
        ),
        (
            "judgment J(x: Int) decides P(x) from",
            Some("J"),
            &["expected judgment source"],
        ),
        (
            "judgment J(x: Int) decides P(x) ->",
            Some("J"),
            &[NEEDS_A_BLOCK],
        ),
        // A declaration with no name: what stands after the keyword on its
        // line is not one.
        ("clause { when true }", None, &[NEEDS_A_NAME]),
        ("decision (x: Int) -> Int { }", None, &[NEEDS_A_NAME]),
        ("duty { bearer A }", None, &[NEEDS_A_NAME]),
        ("effect { }", None, &[NEEDS_A_NAME]),
        ("entity : NaturalPerson", None, &[NEEDS_A_NAME]),
        // A unit word such as `days` is not a name.
        ("entity days : NaturalPerson", None, &[NEEDS_A_NAME]),
        ("interpretation_family { }", None, &[NEEDS_A_NAME]),
        ("legal_act { }", None, &[NEEDS_A_NAME]),
        ("nomination \"x\" for B rank 1", None, &[NEEDS_A_NAME]),
        ("office (x: Int) occupied_by T { }", None, &[NEEDS_A_NAME]),
        ("power { holder A }", None, &[NEEDS_A_NAME]),
        ("proposition (x: Int)", None, &[NEEDS_A_NAME]),
        ("scenario { }", None, &[NEEDS_A_NAME]),
        ("conflict_doctrine { }", None, &[NEEDS_A_NAME]),
        ("for_all 5 in D : true", None, &[NEEDS_A_NAME]),
        ("exists (y) in D : true", None, &[NEEDS_A_NAME]),
    ];

    #[test]
    fn a_declaration_with_a_truncated_header_does_not_take_the_declaration_after_it() {
        let mut failures = Vec::new();
        for &(line, name, diagnostics) in TRUNCATED_HEADERS {
            // The declaration after it stands on the next line, below a
            // comment line, or below a blank line.
            for between in ["", "// { not a body }\n", "\n"] {
                let parsed = parse_file(&wrap_module(&format!(
                    "{line}\n{between}proposition Witness()"
                )));
                let keyword = line.split(' ').next().unwrap_or(line);
                // The text the parser keeps for the first item is its own
                // line with its line end, and nothing of the lines below it.
                // For a header keyword it is what follows the keyword on its
                // line. A declaration whose header the parser steps over
                // ends with the `}` of the block on its line: its text ends
                // with that brace. A `type` line with no definition is
                // stepped over to its line end.
                let ends_with_its_block = line.ends_with('}') && keyword != "type";
                let own_line = if ends_with_its_block {
                    line.to_owned()
                } else {
                    format!("{line}\n")
                };
                let (first_text, own_text) =
                    match parsed.module().and_then(|module| module.items.first()) {
                        Some(Item::Header(header)) => (
                            header.value.as_str(),
                            line[keyword.len()..].trim().to_owned(),
                        ),
                        Some(item) => (
                            item.decl().map_or("", |decl| decl.source.as_str()),
                            own_line,
                        ),
                        None => ("", own_line),
                    };
                let witness_text = parsed
                    .module()
                    .and_then(|module| module.items.get(1))
                    .and_then(Item::decl)
                    .map_or("", |decl| decl.source.trim());
                let read = (
                    messages(&parsed),
                    item_names(&parsed),
                    first_text,
                    witness_text,
                );
                let expected = (
                    diagnostics.to_vec(),
                    vec![(keyword, name), ("proposition", Some("Witness"))],
                    own_text.as_str(),
                    "proposition Witness()",
                );
                if read != expected {
                    failures.push(format!(
                        "`{line}` then {between:?}\n    read:     {read:?}\n    expected: {expected:?}"
                    ));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} truncated headers take the declaration after them or are not diagnosed as they should be:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn the_text_of_a_declaration_the_parser_steps_over_ends_with_its_statement() {
        // `{` continues a statement, so a block on the next line belongs to
        // the declaration.
        let parsed = parse_body("power W\n{\nholder A\n}\nproposition Witness()");
        assert_eq!(
            item_names(&parsed),
            [("power", Some("W")), ("proposition", Some("Witness"))]
        );
        assert_eq!(first_decl(&parsed).source, "power W\n{\nholder A\n}");
        // Below a blank line or a comment line it does not continue the
        // statement, so the declaration has ended and the block is not its
        // body.
        for between in ["\n", "// why\n"] {
            let parsed = parse_file(&wrap_module(&format!(
                "power W\n{between}{{\nholder A\n}}\nproposition Witness()"
            )));
            assert_eq!(
                messages(&parsed),
                ["expected declaration"],
                "between the two lines: {between:?}"
            );
            assert_eq!(
                item_names(&parsed),
                [("power", Some("W")), ("proposition", Some("Witness"))],
                "between the two lines: {between:?}"
            );
            // Its text ends with its line and holds neither the blank line
            // nor the comment.
            assert_eq!(
                first_decl(&parsed).source,
                "power W\n",
                "between the two lines: {between:?}"
            );
        }
        // Where the parser expects a block it reads on across such lines.
        for between in ["\n", "// why\n"] {
            let parsed = parse_body(&format!(
                "rule R : derive\n{between}{{\nwhen true\nthen derive Q()\n}}\nproposition Witness()"
            ));
            assert_eq!(
                rule_statements(&parsed),
                [
                    "when Bool(true)".to_owned(),
                    format!("then derive {:?}", call("Q", Vec::new())),
                ],
                "between the two lines: {between:?}"
            );
        }
    }

    #[test]
    fn a_rule_without_a_body_is_e100_and_keeps_its_header() {
        let parsed = parse_file(&wrap_module(
            "rule R(x: T) : derive from \"cite\"\nproposition Witness()",
        ));
        let diagnostics: Vec<(DiagnosticCode, &str)> = parsed
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
            .collect();
        assert_eq!(diagnostics, [(DiagnosticCode::E100, RULE_NEEDS_A_BODY)]);
        let rule = first_decl(&parsed);
        assert_eq!(rule.name.as_deref(), Some("R"));
        assert_eq!(rule.params, [("x".to_owned(), "T".to_owned())]);
        assert_eq!(rule.rule_kind.as_deref(), Some("derive"));
        assert_eq!(rule.source_basis, Some(Expr::String("cite".to_owned())));
        assert_eq!((&rule.guard, rule.consequences.len()), (&None, 0));
        // Text between the header and a block is stepped over together with
        // the block, which is not read as the body.
        let parsed = parse_file(&wrap_module(
            "rule R : derive extra {\nwhen true\nthen derive P()\n}\nproposition Witness()",
        ));
        assert_eq!(messages(&parsed), [RULE_NEEDS_A_BODY]);
        assert_eq!(
            item_names(&parsed),
            [("rule", Some("R")), ("proposition", Some("Witness"))]
        );
        let rule = first_decl(&parsed);
        assert_eq!((&rule.guard, rule.consequences.len()), (&None, 0));
    }

    #[test]
    fn text_between_a_verify_name_and_its_block_is_e100() {
        let parsed = parse_file(&wrap_module(
            "verify Checked extra { assert false }\nproposition Witness()",
        ));
        let diagnostics: Vec<(DiagnosticCode, &str)> = parsed
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
            .collect();
        assert_eq!(diagnostics, [(DiagnosticCode::E100, VERIFY_NEEDS_A_BLOCK)]);
        assert_eq!(
            item_names(&parsed),
            [
                ("verify", Some("Checked")),
                ("proposition", Some("Witness"))
            ]
        );
        // The block is stepped over with the stray word, so no assertion is
        // read from it.
        assert!(first_decl(&parsed).assertions.is_empty());
    }

    #[test]
    fn a_function_without_a_body_is_e100_and_keeps_its_header() {
        for keyword in ["calc", "fn"] {
            // The header ends with its type, with a `fuel` clause, with an
            // effect row or with its parameters.
            for header in [
                "f(a: Int) -> Int",
                "f(a: Int) -> Int fuel = 100",
                "f(a: Int) -> Int ! {Observe}",
                "f(a: Int)",
            ] {
                let parsed = parse_file(&wrap_module(&format!(
                    "{keyword} {header}\nproposition Witness()"
                )));
                let diagnostics: Vec<(DiagnosticCode, &str)> = parsed
                    .diagnostics
                    .iter()
                    .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
                    .collect();
                assert_eq!(
                    diagnostics,
                    [(DiagnosticCode::E100, FUNCTION_NEEDS_A_BODY)],
                    "{keyword} {header}"
                );
                assert_eq!(
                    item_names(&parsed),
                    [(keyword, Some("f")), ("proposition", Some("Witness"))],
                    "{keyword} {header}"
                );
                let function = first_decl(&parsed);
                assert_eq!(function.params, [("a".to_owned(), "Int".to_owned())]);
                assert_eq!(function.expr, None, "{keyword} {header}");
            }
            // Text between the header and a block is stepped over together
            // with the block, which is not read as the body.
            let parsed = parse_file(&wrap_module(&format!(
                "{keyword} f(a: Int) -> Int extra {{ return 7 }}\nproposition Witness()"
            )));
            assert_eq!(messages(&parsed), [FUNCTION_NEEDS_A_BODY], "{keyword}");
            assert_eq!(
                item_names(&parsed),
                [(keyword, Some("f")), ("proposition", Some("Witness"))],
                "{keyword}"
            );
            assert_eq!(first_decl(&parsed).expr, None, "{keyword}");
        }
    }

    #[test]
    fn a_header_that_stops_short_is_reported_at_its_last_token() {
        // The line, the diagnostic and the token of the line it points at.
        let rows = [
            ("rule R : derive", RULE_NEEDS_A_BODY, "derive"),
            ("rule R : derive from \"c\"", RULE_NEEDS_A_BODY, "\"c\""),
            ("verify V", VERIFY_NEEDS_A_BLOCK, "V"),
            ("calc f(a: Int) -> Int", FUNCTION_NEEDS_A_BODY, "Int"),
            ("proposition", NEEDS_A_NAME, "proposition"),
            ("rule", "expected rule name", "rule"),
            ("query automatic", "expected query name", "automatic"),
            ("entity E :", NEEDS_A_TYPE, ":"),
            ("decision D2(x: Int) ->", NEEDS_A_TYPE, "->"),
            ("type T =", NEEDS_A_TYPE, "="),
            ("rule R :", NEEDS_A_RULE_KIND, ":"),
            ("power W =", STOPS_SHORT, "="),
            ("jurisdiction Test :", STOPS_SHORT, ":"),
            (
                "jurisdiction",
                "expected a value after `jurisdiction`",
                "jurisdiction",
            ),
            (
                "outside_scope",
                "expected a value after `outside_scope`",
                "outside_scope",
            ),
            ("record_type R", NEEDS_A_BLOCK, "R"),
            ("evidence_type V :", NEEDS_A_BLOCK, ":"),
            ("observation O(r: Int)", NEEDS_ESTABLISHES, ")"),
            (
                "observation O(r: Int) establishes",
                "expected observation proposition",
                "establishes",
            ),
            ("observation O(r: Int) establishes P(r)", NEEDS_A_BLOCK, ")"),
            ("judgment J(x: Int) ->", NEEDS_DECIDES, "->"),
            (
                "judgment J(x: Int) decides P(x) from",
                "expected judgment source",
                "from",
            ),
            // A declaration with no name: the diagnostic points at what
            // stands where the name would.
            ("power { holder A }", NEEDS_A_NAME, "{"),
            ("entity : NaturalPerson", NEEDS_A_NAME, ":"),
            // Text that stands where the block would begin is part of the
            // statement, so the diagnostic points at it.
            ("rule R : derive extra { }", RULE_NEEDS_A_BODY, "extra"),
            ("verify V extra { }", VERIFY_NEEDS_A_BLOCK, "extra"),
            ("calc f() -> Int extra { }", FUNCTION_NEEDS_A_BODY, "extra"),
            ("record_type R extra { a: Int }", NEEDS_A_BLOCK, "extra"),
        ];
        let mut failures = Vec::new();
        for (line, message, token) in rows {
            // The declaration after it stands on the next line or below a
            // blank line.
            for between in ["", "\n"] {
                let module = wrap_module(&format!("{line}\n{between}proposition Witness()"));
                let parsed = parse_file(&module);
                let line_start = module.find(line).expect("the line");
                let token_start = line_start + line.rfind(token).expect("the token");
                let pointed_at: Vec<(&str, Option<(usize, usize)>)> = parsed
                    .diagnostics
                    .iter()
                    .map(|diagnostic| {
                        let span = diagnostic
                            .primary_span
                            .map(|span| (span.start as usize, span.end as usize));
                        (diagnostic.message.as_str(), span)
                    })
                    .collect();
                let expected = vec![(message, Some((token_start, token_start + token.len())))];
                if pointed_at != expected {
                    failures.push(format!(
                        "`{line}` then {between:?}\n    read:     {pointed_at:?}\n    expected: {expected:?}"
                    ));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} diagnostics do not point at the header they are about:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_keyword_alone_before_the_end_of_the_module_is_e100() {
        // Nothing follows the keyword: not on its line, and not in the
        // module.
        for (keyword, message) in [
            ("proposition", NEEDS_A_NAME),
            ("entity", NEEDS_A_NAME),
            ("rule", "expected rule name"),
            ("query", "expected query name"),
            ("jurisdiction", "expected a value after `jurisdiction`"),
            ("effective_at", "expected a value after `effective_at`"),
            ("recorded_at", "expected a value after `recorded_at`"),
            ("outside_scope", "expected a value after `outside_scope`"),
        ] {
            for end in ["\n}\n", " }\n", ";\n}\n"] {
                let parsed = parse_file(&format!(
                    "module X version \"0.1.0\" {{\nproposition Witness()\n{keyword}{end}"
                ));
                assert_eq!(messages(&parsed), [message], "{keyword} then {end:?}");
                assert_eq!(
                    item_names(&parsed),
                    [("proposition", Some("Witness")), (keyword, None)],
                    "{keyword} then {end:?}"
                );
            }
        }
    }

    #[test]
    fn a_type_with_an_open_brace_and_no_definition_is_e100_and_the_parse_ends() {
        // The parser did not stop on this module: with no `=` it read the
        // `{` as the fields of a variant, and the loop over the fields
        // consumed nothing at the `(` of the line below.
        let parsed = parse_file(
            "module Probe.Shape version \"0.1.0\" {\ntype N {\nproposition Witness()\n}\n",
        );
        let diagnostics: Vec<(DiagnosticCode, &str)> = parsed
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
            .collect();
        assert_eq!(
            diagnostics,
            [(DiagnosticCode::E100, TYPE_NEEDS_AN_EQUALS_SIGN)]
        );
        assert_eq!(
            item_names(&parsed),
            [("type", Some("N")), ("proposition", Some("Witness"))]
        );
    }

    #[test]
    fn a_loop_over_fields_moves_on_at_a_token_that_begins_no_field() {
        // Each loop here reads a name, a `:` and a type or a value. At a
        // token that is none of those it consumed nothing, so the parse did
        // not end. The token is now reported and stepped over.
        // Beside each module: whether the declaration after it is kept. An
        // opening bracket that nothing closes takes the rest of the module.
        for (body, kept) in [
            // The fields of a variant.
            ("type T = A { ) }\nproposition Witness()", true),
            ("type T = A { ( }\nproposition Witness()", false),
            (
                "type T = A { ) : Int, b: Int }\nproposition Witness()",
                true,
            ),
            (
                "type T = A {\n\"s\"\nb: Int\n}\nproposition Witness()",
                true,
            ),
            ("type T = A {\nproposition Witness()", false),
            // The fields of a judgment's request.
            (
                "judgment J(x: T) decides P(x) {\nauthority A\notherwise suspend NeedJudgment { ) }\n}\nproposition Witness()",
                true,
            ),
            (
                "judgment J(x: T) decides P(x) {\nauthority A\notherwise suspend NeedJudgment {\nissue: P(x)\n-> stray\nprotocol: Determine\n}\n}\nproposition Witness()",
                true,
            ),
            (
                "judgment J(x: T) decides P(x) {\nauthority A\notherwise suspend NeedJudgment {\n(\nproposition Witness()",
                false,
            ),
        ] {
            let parsed = parse_file(&wrap_module(body));
            assert!(
                messages(&parsed).contains(&"expected identifier"),
                "{body:?}: {:?}",
                messages(&parsed)
            );
            let witness = item_names(&parsed).contains(&("proposition", Some("Witness")));
            assert_eq!(witness, kept, "{body:?}");
        }
    }

    /// Every keyword that begins a declaration.
    const DECLARATION_KEYWORDS: &[&str] = &[
        "jurisdiction",
        "source_snapshot",
        "source_manifest",
        "effective_at",
        "recorded_at",
        "outside_scope",
        "source",
        "import",
        "entity",
        "type",
        "record_type",
        "evidence_type",
        "office",
        "proposition",
        "observation",
        "nomination",
        "fn",
        "calc",
        "effect",
        "for_all",
        "exists",
        "rule",
        "power",
        "duty",
        "judgment",
        "decision",
        "legal_act",
        "clause",
        "interpretation_family",
        "conflict_doctrine",
        "query",
        "scenario",
        "verify",
    ];

    #[test]
    fn every_header_that_stops_early_is_parsed_to_the_end() {
        // This test exists to catch a parse that does not end: it passes
        // when every parse returns. One that does not end hangs the test and
        // allocates until the test process is stopped, so run it under a
        // memory limit after a change to a loop of the parser.
        //
        // Each header is a keyword in one of five forms, followed by one of
        // these tokens on its line or on a line of its own, above another
        // declaration.
        let stems = [
            "{k} N",
            "{k} N(x: Int)",
            "{k} N : T",
            "{k} N(x: Int) -> T",
            "{k}",
        ];
        let tails = [
            ":",
            "->",
            "=",
            ",",
            ".",
            "+",
            "-",
            "*",
            "/",
            "|",
            "<",
            ">",
            "!",
            "=>",
            "==",
            "<=",
            "(",
            "[",
            "{",
            "not",
            "effective",
            "operative",
            "and",
            "or",
            "in",
            "from",
            "for",
            "rank",
            "version",
            "using",
            "as",
            "to",
            "where",
            "then",
            "else",
            "fuel",
            "decides",
            "occupied_by",
            "establishes",
        ];
        let mut parses = 0;
        let mut silent = Vec::new();
        for keyword in DECLARATION_KEYWORDS {
            assert!(is_declaration_keyword(keyword), "{keyword}");
            for stem in stems {
                let header = stem.replace("{k}", keyword);
                for tail in tails {
                    for line in [format!("{header} {tail}"), format!("{header}\n{tail}")] {
                        let parsed =
                            parse_file(&wrap_module(&format!("{line}\nproposition Witness()")));
                        parses += 1;
                        // A header the parser accepts without a diagnostic
                        // has not taken the declaration below it.
                        let kept = item_names(&parsed).contains(&("proposition", Some("Witness")));
                        if !parsed.has_errors() && !kept {
                            silent.push(line);
                        }
                    }
                }
            }
        }
        assert_eq!(parses, 33 * 5 * 39 * 2);
        assert!(
            silent.is_empty(),
            "{} headers take the declaration below them without a diagnostic: {silent:?}",
            silent.len()
        );
    }

    #[test]
    fn type_arguments_that_are_not_closed_are_e100_and_take_no_declaration() {
        // After `<` the parser reads on to the `>`. The declaration below
        // is not a type argument.
        for (header, keyword, diagnostics) in [
            ("entity E : A <", "entity", vec!["expected `>`"]),
            ("entity E : A <\nB,", "entity", vec!["expected `>`"]),
            (
                "decision D(x: Int) -> A <",
                "decision",
                vec!["expected `>`"],
            ),
            ("query Q() -> A<B, C", "query", vec!["expected `>`"]),
            ("type T = A <", "type", vec!["expected `>`"]),
            (
                "calc f(a: Int) -> A <",
                "calc",
                vec!["expected `>`", FUNCTION_NEEDS_A_BODY],
            ),
        ] {
            let parsed = parse_file(&wrap_module(&format!("{header}\nproposition Witness()")));
            assert_eq!(messages(&parsed), diagnostics, "{header:?}");
            let items = item_names(&parsed);
            assert_eq!(
                (items.first().map(|item| item.0), items.get(1)),
                (Some(keyword), Some(&("proposition", Some("Witness")))),
                "{header:?}"
            );
        }
        // Arguments that are closed on a later line are read as before.
        let parsed = parse_body("entity E : Map<\nK,\nV\n>\nproposition Witness()");
        assert_eq!(
            item_names(&parsed),
            [("entity", Some("E")), ("proposition", Some("Witness"))]
        );
        assert_eq!(
            first_decl(&parsed).result_type.as_deref(),
            Some("Map<\nK,\nV\n>")
        );
    }

    #[test]
    fn a_type_or_a_rule_kind_on_the_line_below_is_read_when_it_begins_no_declaration() {
        // The line rule reads on after `:`, `->` and `=`, and a line that
        // begins with `->` or `{` continues the header above it.
        let parsed = parse_body(
            "entity Pat :\nNaturalPerson\ndecision D(x: Int) ->\nBool\nrule R :\nderive {\nwhen true\nthen derive P()\n}\nquery q()\n-> Int\n{ return 7 }\ntype T =\nInt",
        );
        assert_eq!(
            item_names(&parsed),
            [
                ("entity", Some("Pat")),
                ("decision", Some("D")),
                ("rule", Some("R")),
                ("query", Some("q")),
                ("type", Some("T")),
            ]
        );
        let module = parsed.module().expect("a module");
        let decls: Vec<&Decl> = module.items.iter().filter_map(Item::decl).collect();
        assert_eq!(decls[0].result_type.as_deref(), Some("NaturalPerson"));
        assert_eq!(decls[1].result_type.as_deref(), Some("Bool"));
        assert_eq!(decls[2].rule_kind.as_deref(), Some("derive"));
        assert_eq!(decls[3].result_type.as_deref(), Some("Int"));
        assert_eq!(decls[3].expr, Some(Expr::Int(7)));
    }

    #[test]
    fn a_then_or_an_otherwise_with_no_consequence_is_e100() {
        for word in ["then", "otherwise"] {
            let message = format!("expected a consequence after `{word}`");
            // The word stands alone on its line, or what follows it is not
            // a consequence. Either way the statement below it is kept.
            for line in [word.to_owned(), format!("{word} Q()")] {
                let module = wrap_module(&format!(
                    "rule R : derive {{\n{line}\nwhen true\nthen derive Q()\n}}"
                ));
                let parsed = parse_file(&module);
                assert_eq!(messages(&parsed), [message.as_str()], "`{line}`");
                let rule = first_decl(&parsed);
                assert_eq!(rule.guard, Some(Expr::Bool(true)), "below `{line}`");
                assert_eq!(rule.consequences.len(), 1, "below `{line}`");
                assert!(rule.fallback.is_empty(), "below `{line}`");
                // The diagnostic points at the word when it stands alone,
                // and at what follows it otherwise.
                let span = parsed.diagnostics[0].primary_span.expect("a span");
                let line_start = module.find(&format!("{line}\nwhen")).expect("the line");
                let expected = if line == word {
                    (line_start, line_start + word.len())
                } else {
                    (line_start + word.len() + 1, line_start + word.len() + 2)
                };
                assert_eq!(
                    (span.start as usize, span.end as usize),
                    expected,
                    "`{line}`"
                );
            }
        }
        // A consequence on the line below the word belongs to it.
        let parsed = parse_body("rule R : derive {\nthen\nderive Q()\notherwise\nderive P()\n}");
        let rule = first_decl(&parsed);
        assert_eq!((rule.consequences.len(), rule.fallback.len()), (1, 1));
        // A line that begins with `then` continues the line above it. Below
        // a word that stands alone it is what is left of that statement, so
        // it is stepped over with it.
        for word in ["then", "otherwise"] {
            let parsed = parse_file(&wrap_module(&format!(
                "rule R : derive {{\n{word}\nthen derive Q()\nwhen true\n}}"
            )));
            assert_eq!(
                messages(&parsed),
                [format!("expected a consequence after `{word}`")],
                "`{word}`"
            );
            let rule = first_decl(&parsed);
            assert_eq!(rule.guard, Some(Expr::Bool(true)), "below `{word}`");
            assert!(rule.consequences.is_empty(), "below `{word}`");
        }
    }

    #[test]
    fn text_in_a_rule_that_is_no_rule_statement_is_e100_and_only_it_is_stepped_over() {
        // A line stands before, between and after the statements of the
        // rule. `whne` is `when` misspelled: without the diagnostic the rule
        // would lose its guard. In the last two rows the text follows a
        // consequence on its line.
        for lines in [
            "nonsense here\nwhen true\nthen derive Q()",
            "then derive Q()\nnonsense here\nwhen true",
            "when true\nthen derive Q()\nnonsense here",
            "when true\nthen derive Q()\nnonsense { here }\n",
            "whne false\nwhen true\nthen derive Q()",
            "then derive Q()\n) stray\nwhen true",
            "then derive Q() using Protocol\nwhen true",
            "then derive Q() where x > 0\nwhen true",
        ] {
            let parsed = parse_file(&wrap_module(&format!("rule R : derive {{\n{lines}\n}}")));
            assert_eq!(
                messages(&parsed),
                ["expected a rule statement"],
                "{lines:?}"
            );
            assert_eq!(
                rule_statements(&parsed),
                [
                    "when Bool(true)".to_owned(),
                    format!("then derive {:?}", call("Q", Vec::new())),
                ],
                "{lines:?}"
            );
        }
        // The diagnostic points at the first token of the line.
        let module = wrap_module("rule R : derive {\nwhen true\nwhne false\nthen derive Q()\n}");
        let parsed = parse_file(&module);
        let span = parsed.diagnostics[0].primary_span.expect("a span");
        assert_eq!(&module[span.start as usize..span.end as usize], "whne");
        let module = wrap_module("rule R : derive {\nthen derive Q() using Protocol\n}");
        let parsed = parse_file(&module);
        let span = parsed.diagnostics[0].primary_span.expect("a span");
        assert_eq!(&module[span.start as usize..span.end as usize], "using");
        // A line that begins with `then` continues the line above it. Below
        // a line the rule does not recognize it is what is left of that
        // statement, so it is stepped over with it.
        let parsed = parse_file(&wrap_module(
            "rule R : derive {\nwhne false\nthen derive Q()\nwhen true\n}",
        ));
        assert_eq!(messages(&parsed), ["expected a rule statement"]);
        assert_eq!(rule_statements(&parsed), ["when Bool(true)"]);
    }

    #[test]
    fn a_valid_time_or_record_time_line_of_a_rule_is_read_and_nothing_of_it_is_kept() {
        let plain = parse_body("rule R : derive {\nwhen true\nthen establish Q()\n}");
        // The four lines of the corpus that say when a consequence holds,
        // and both words on one line.
        for lines in [
            "valid_time [effective_at, +inf)\nrecord_time [max_record_time(guard_trace), +inf)",
            "valid_time [2026-08-14, +inf)\nrecord_time [2026-08-14T00:00:00-04:00, +inf)",
            "valid_time [2026-08-14, 2027-01-01]",
            "valid_time [a, b) record_time [c, d)",
            "valid_time\n[a, b)",
        ] {
            // Below the consequence, on its line, and above the guard.
            for body in [
                format!("when true\nthen establish Q()\n{lines}"),
                format!("when true\nthen establish Q() {lines}"),
                format!("{lines}\nwhen true\nthen establish Q()"),
            ] {
                let parsed = parse_file(&wrap_module(&format!("rule R : derive {{\n{body}\n}}")));
                assert_eq!(messages(&parsed), Vec::<&str>::new(), "{body:?}");
                let (rule, expected) = (first_decl(&parsed), first_decl(&plain));
                assert_eq!(rule.guard, expected.guard, "{body:?}");
                assert_eq!(rule.consequences, expected.consequences, "{body:?}");
                assert_eq!(rule.fallback, expected.fallback, "{body:?}");
                assert_eq!(rule.require, expected.require, "{body:?}");
            }
        }
    }

    #[test]
    fn a_valid_time_or_record_time_line_with_no_interval_is_e100() {
        for word in ["valid_time", "record_time"] {
            let message = format!("expected an interval after `{word}`");
            // The word stands alone, or what follows it is not an interval.
            for line in [
                word.to_owned(),
                format!("{word} 2026-08-14"),
                format!("{word} (a, b)"),
            ] {
                let parsed = parse_file(&wrap_module(&format!(
                    "rule R : derive {{\n{line}\nwhen true\nthen derive Q()\n}}"
                )));
                assert_eq!(messages(&parsed), [message.as_str()], "`{line}`");
                assert_eq!(
                    rule_statements(&parsed),
                    [
                        "when Bool(true)".to_owned(),
                        format!("then derive {:?}", call("Q", Vec::new())),
                    ],
                    "`{line}`"
                );
            }
        }
        // An interval that does not close is `E100` as it is in an
        // expression.
        let parsed = parse_file(&wrap_module(
            "rule R : derive {\nvalid_time [a\nwhen true\nthen derive Q()\n}",
        ));
        assert!(
            messages(&parsed).contains(&"expected `,`"),
            "{:?}",
            parsed.diagnostics
        );
    }

    #[test]
    fn a_statement_goal_evaluate_cannot_read_is_e100_and_only_it_is_stepped_over() {
        let unreadable = "expected evaluation statement";
        let after_return = "evaluation statement follows `return`";
        // The lines of the goal, the diagnostics, and the statements kept.
        let required_false = format!("{:?}", Expr::Require(Box::new(Expr::Bool(false))));
        let rows: [(&str, &[&str], Vec<String>); 5] = [
            // `return` with no expression: a token of the statement is
            // consumed, and the statement has ended.
            (
                "true\nreturn\nrequire false",
                &[unreadable, after_return],
                vec!["Bool(true)".to_owned(), required_false.clone()],
            ),
            // `not` with no operand goes on to the next line by the line
            // rule, so that line is what is left of the statement.
            (
                "true\nnot\nrequire false",
                &[unreadable],
                vec!["Bool(true)".to_owned()],
            ),
            // A token no statement begins with: nothing is consumed, and
            // the line is stepped over.
            (
                "true\n) stray\nrequire false",
                &[unreadable],
                vec!["Bool(true)".to_owned(), required_false.clone()],
            ),
            // The statement that cannot be read is the last one.
            ("true\nreturn", &[unreadable], vec!["Bool(true)".to_owned()]),
            (
                "require false\n) stray",
                &[unreadable],
                vec![required_false.clone()],
            ),
        ];
        let mut failures = Vec::new();
        for (lines, diagnostics, statements) in rows {
            let parsed = parse_file(&wrap_module(&format!(
                "query q() -> Bool {{\ngoal Evaluate {{\n{lines}\n}}\n}}"
            )));
            let read = (messages(&parsed), goal_statements(&parsed));
            let expected = (diagnostics.to_vec(), statements);
            if read != expected {
                failures.push(format!(
                    "{lines:?}\n    read:     {read:?}\n    expected: {expected:?}"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} goals are not read as they should be:\n{}",
            failures.len(),
            failures.join("\n")
        );
        // The diagnostic points at the first token of the statement.
        let module =
            wrap_module("query q() -> Bool {\ngoal Evaluate {\ntrue\nreturn\nrequire false\n}\n}");
        let parsed = parse_file(&module);
        let span = parsed.diagnostics[0].primary_span.expect("a span");
        assert_eq!(
            (span.start as usize, span.end as usize),
            (
                module.find("return").expect("the word"),
                module.find("return").expect("the word") + 6
            )
        );
    }

    #[test]
    fn using_at_the_end_of_a_line_is_e100_and_does_not_take_the_statement_below_it() {
        let message = "expected a protocol name after `using`";
        let required = |name: &str| format!("{:?}", Expr::Require(Box::new(ident(name))));
        // Every place a `require` statement can stand: its name, the lines
        // before and after the statements, and how to list them.
        type Statements = fn(&Parse) -> Vec<String>;
        let places: [(&str, &str, &str, Statements); 4] = [
            ("a query body", "query q() -> Bool {", "}", body_statements),
            (
                "goal Evaluate",
                "query q() -> Bool {\ngoal Evaluate {",
                "}\n}",
                goal_statements,
            ),
            ("a calc body", "calc f() -> Bool {", "}", body_statements),
            (
                "a transaction",
                "query q() -> Bool {\ntransaction {",
                "}\n}",
                transaction_steps,
            ),
        ];
        for (place, open, close, statements) in places {
            // `using` ends its line, or what follows it is not a name.
            for line in ["require a using", "require a\nusing", "require a using 5"] {
                let parsed =
                    parse_file(&wrap_module(&format!("{open}\n{line}\nrequire b\n{close}")));
                assert_eq!(messages(&parsed), [message], "{place}: {line:?}");
                let read = statements(&parsed);
                assert_eq!(read.first(), Some(&required("a")), "{place}: {line:?}");
                assert_eq!(read.last(), Some(&required("b")), "{place}: {line:?}");
            }
            // A protocol name after `using`, on its line or with `using` on
            // the line below the condition, is read as before.
            for line in ["require a using Determine", "require a\nusing Determine"] {
                let parsed =
                    parse_file(&wrap_module(&format!("{open}\n{line}\nrequire b\n{close}")));
                assert_eq!(messages(&parsed), Vec::<&str>::new(), "{place}: {line:?}");
                assert_eq!(
                    statements(&parsed),
                    [required("a"), required("b")],
                    "{place}: {line:?}"
                );
            }
        }
        // The diagnostic points at `using` when the line ends there.
        let module = wrap_module("query q() -> Bool {\nrequire a using\nrequire b\n}");
        let parsed = parse_file(&module);
        let span = parsed.diagnostics[0].primary_span.expect("a span");
        assert_eq!(&module[span.start as usize..span.end as usize], "using");
    }

    #[test]
    fn a_source_field_alone_on_its_line_does_not_take_the_field_below_it() {
        let parsed = parse_body("source S {\ndigest\nkind statute\n}");
        let fields: Vec<(&str, &str)> = first_decl(&parsed)
            .fields
            .iter()
            .map(|(field, value)| (field.as_str(), value.as_str()))
            .collect();
        assert_eq!(fields, [("digest", ""), ("kind", "statute")]);
        // A field the block does not know is rejected, and it does not take
        // the field below it either.
        let parsed = parse_file(&wrap_module("source S {\nnonsense\nkind statute\n}"));
        assert_eq!(messages(&parsed), ["unknown SourceField `nonsense`"]);
        let fields: Vec<(&str, &str)> = first_decl(&parsed)
            .fields
            .iter()
            .map(|(field, value)| (field.as_str(), value.as_str()))
            .collect();
        assert_eq!(fields, [("kind", "statute")]);
    }

    #[test]
    fn a_bounds_word_alone_on_its_line_does_not_take_the_assertion_below_it() {
        let parsed = parse_body("verify V {\nbounds\nassert true\n}");
        let block = first_decl(&parsed);
        assert_eq!(block.assertions, [Expr::Bool(true)]);
        assert_eq!(
            block.fields.get("bounds").map(String::as_str),
            Some("bounds")
        );
    }

    #[test]
    fn a_judgment_clause_word_alone_on_its_line_does_not_take_the_clause_below_it() {
        let mut failures = Vec::new();
        for word in ["record", "otherwise", "on", "review"] {
            // The clause below it stands on the next line or below a comment
            // line. The text kept for the word holds neither.
            for between in ["", "// note\n"] {
                let parsed = parse_file(&wrap_module(&format!(
                    "judgment J(x: T) decides P(x) {{\nauthority A\n{word}\n{between}establish when a == b\n}}"
                )));
                let judgment = first_decl(&parsed).judgment.as_ref().expect("a judgment");
                let kept_as_text: Vec<&str> = judgment
                    .unsupported_clauses
                    .iter()
                    .map(|clause| clause.trim())
                    .collect();
                let read = (
                    messages(&parsed),
                    kept_as_text,
                    judgment.establish_when.clone(),
                );
                let expected = (
                    Vec::new(),
                    vec![word],
                    Some(binary(BinOp::Eq, ident("a"), ident("b"))),
                );
                if read != expected {
                    failures.push(format!(
                        "`{word}` then {between:?}\n    read:     {read:?}\n    expected: {expected:?}"
                    ));
                }
            }
        }
        // `establish when` and `reject when` end with their line as well: a
        // `determination` line below them is a clause of its own.
        for words in ["establish when", "reject when"] {
            let parsed = parse_file(&wrap_module(&format!(
                "judgment J(x: T) decides P(x) {{\nauthority A\n{words}\ndetermination d\nstandard S\n}}"
            )));
            let judgment = first_decl(&parsed).judgment.as_ref().expect("a judgment");
            let kept_as_text: Vec<&str> = judgment
                .unsupported_clauses
                .iter()
                .map(|clause| clause.trim())
                .collect();
            let read = (messages(&parsed), kept_as_text, judgment.standard.clone());
            let expected = (Vec::new(), vec![words, "determination d"], Some(ident("S")));
            if read != expected {
                failures.push(format!(
                    "`{words}`\n    read:     {read:?}\n    expected: {expected:?}"
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} clause words take the clause below them:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_comment_at_the_end_of_a_line_is_not_part_of_the_text_before_it() {
        let parsed = parse_file(&wrap_module(
            "jurisdiction Test // where\neffective_at 2026-01-01 // when\nentity Pat : NaturalPerson // the tenant\ncalc f(a: Int) -> Int // doubles\n{ return a }\nsource S {\nkind statute // which kind\n}\njudgment J(x: T) decides P(x) {\nauthority A\nrecord // kept as text\n}",
        ));
        assert_eq!(messages(&parsed), Vec::<&str>::new());
        let module = parsed.module().expect("a module");
        let header_values: Vec<&str> = module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Header(header) => Some(header.value.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(header_values, ["Test", "2026-01-01"]);
        let decl = |name: &str| {
            module
                .items
                .iter()
                .filter_map(Item::decl)
                .find(|decl| decl.name.as_deref() == Some(name))
                .unwrap_or_else(|| panic!("declaration {name}"))
        };
        assert_eq!(decl("Pat").source, "entity Pat : NaturalPerson");
        assert_eq!(decl("Pat").signature.as_deref(), Some(": NaturalPerson"));
        assert_eq!(decl("f").result_type.as_deref(), Some("Int"));
        assert_eq!(
            decl("S").fields.get("kind").map(String::as_str),
            Some("statute")
        );
        let judgment = decl("J").judgment.as_ref().expect("a judgment");
        assert_eq!(judgment.unsupported_clauses, ["record"]);
    }

    #[test]
    fn a_value_after_a_header_string_is_e100_and_ends_with_its_line() {
        let parsed = parse_file(&wrap_module(
            "source_snapshot \"2026\" extra\nentity Bryan : NaturalPerson",
        ));
        assert_eq!(messages(&parsed), ["unexpected value after header string"]);
        assert_eq!(
            item_names(&parsed),
            [("source_snapshot", None), ("entity", Some("Bryan"))]
        );
    }

    #[test]
    fn a_header_keyword_reads_no_value_from_the_line_below_it() {
        let value_of = |parsed: &Parse| match parsed.module().expect("a module").items.first() {
            Some(Item::Header(header)) => header.value.clone(),
            other => panic!("{other:?}"),
        };
        // The statement of the keyword ends with it, so what stands on a
        // line below is not its value: directly below it, below a blank
        // line or below a comment line. That holds for each of the six
        // header keywords, the two that take a string among them: a string
        // does not continue a line. The keyword is rejected, and the line
        // below is read on its own: a word as a declaration that is not
        // known, anything else as no declaration.
        for (keyword, below, stray) in [
            ("jurisdiction", "Test", "unknown declaration `Test`"),
            ("source_snapshot", "\"p\"", "expected declaration"),
            ("source_manifest", "\"p\"", "expected declaration"),
            ("effective_at", "2026-09-17", "expected declaration"),
            (
                "recorded_at",
                "2026-09-17T12:00:00Z",
                "expected declaration",
            ),
            (
                "outside_scope",
                "everything_else",
                "unknown declaration `everything_else`",
            ),
        ] {
            let alone = format!("expected a value after `{keyword}`");
            for gap in ["\n", "\n\n", "\n// the value\n"] {
                let parsed = parse_file(&wrap_module(&format!(
                    "{keyword}{gap}{below}\nproposition Witness()"
                )));
                assert_eq!(
                    messages(&parsed),
                    [alone.as_str(), stray],
                    "{keyword}{gap:?}"
                );
                assert_eq!(
                    item_names(&parsed),
                    [(keyword, None), ("proposition", Some("Witness"))],
                    "{keyword}{gap:?}"
                );
                assert_eq!(value_of(&parsed), "", "{keyword}{gap:?}");
            }
        }
        // A `{` continues the line above it, so a block on the next line is
        // the value. Below a blank line or a comment line it continues
        // nothing, and the keyword has no value.
        let parsed = parse_body("outside_scope\n{ everything_else }\nproposition Witness()");
        assert_eq!(value_of(&parsed), "{ everything_else }");
        assert_eq!(
            item_names(&parsed),
            [("outside_scope", None), ("proposition", Some("Witness"))]
        );
        for gap in ["\n\n", "\n// the value\n"] {
            let parsed = parse_file(&wrap_module(&format!(
                "outside_scope{gap}{{ everything_else }}\nproposition Witness()"
            )));
            assert_eq!(
                messages(&parsed),
                [
                    "expected a value after `outside_scope`",
                    "expected declaration"
                ],
                "{gap:?}"
            );
            assert_eq!(value_of(&parsed), "", "{gap:?}");
        }
        for keyword in ["source_snapshot", "source_manifest"] {
            // A string in the statement of the keyword is its value, as
            // before.
            let parsed = parse_body(&format!("{keyword} \"p\"\nproposition Witness()"));
            assert_eq!(value_of(&parsed), "\"p\"", "{keyword}");
            // Text in the statement that is no string is reported as before,
            // at the place where the string should stand.
            let parsed = parse_file(&wrap_module(&format!(
                "{keyword} Test\nproposition Witness()"
            )));
            assert_eq!(
                messages(&parsed),
                [
                    "expected a JSON string",
                    "unexpected value after header string"
                ],
                "{keyword}"
            );
        }
    }

    #[test]
    fn a_statement_a_block_cannot_read_is_e100_wherever_it_stands_in_the_block() {
        // The four blocks of statements: the lines that open and close one,
        // its diagnostic, and how to list the statements it keeps.
        type Statements = fn(&Parse) -> Vec<String>;
        let blocks: [(&str, &str, &str, &str, Statements); 4] = [
            (
                "a query body",
                "query q() -> Bool {",
                "}",
                "expected query statement",
                body_statements,
            ),
            (
                "goal Evaluate",
                "query q() -> Bool {\ngoal Evaluate {",
                "}\n}",
                "expected evaluation statement",
                goal_statements,
            ),
            (
                "a calc body",
                "calc f() -> Bool {",
                "}",
                "expected function statement",
                body_statements,
            ),
            (
                "a transaction",
                "query q() -> Bool {\ntransaction {",
                "}\n}",
                "expected transaction step",
                transaction_steps,
            ),
        ];
        // The lines of the block, the token of them that the diagnostic
        // points at, and the statements the block keeps.
        let truth = vec!["Bool(true)".to_owned()];
        let rows: [(&str, &str, Vec<String>); 7] = [
            // The statement that cannot be read is the last one of the
            // block. `return` with no expression: a token of the statement
            // is consumed, and the statement has ended.
            ("true\nreturn", "return", truth.clone()),
            // The same with the `;` that ends it.
            ("true\nreturn;", "return", truth.clone()),
            // `not` with no operand.
            ("true\nnot", "not", truth.clone()),
            // A token no statement begins with: nothing is consumed.
            ("true\n) stray", ")", truth.clone()),
            // It is the only statement of the block.
            ("return", "return", Vec::new()),
            // A statement stands below it: only what is left of the
            // statement that cannot be read is stepped over.
            (") stray\ntrue", ")", truth.clone()),
            ("not;\ntrue", "not", truth.clone()),
        ];
        let mut failures = Vec::new();
        for (block, open, close, message, statements) in blocks {
            for (lines, token, kept) in &rows {
                let module = wrap_module(&format!("{open}\n{lines}\n{close}"));
                let parsed = parse_file(&module);
                let lines_start = module.find(open).expect("the block") + open.len() + 1;
                let token_start = lines_start + lines.rfind(token).expect("the token");
                let pointed_at: Vec<(&str, Option<(usize, usize)>)> = parsed
                    .diagnostics
                    .iter()
                    .map(|diagnostic| {
                        let span = diagnostic
                            .primary_span
                            .map(|span| (span.start as usize, span.end as usize));
                        (diagnostic.message.as_str(), span)
                    })
                    .collect();
                let read = (pointed_at, statements(&parsed));
                let expected = (
                    vec![(message, Some((token_start, token_start + token.len())))],
                    kept.clone(),
                );
                if read != expected {
                    failures.push(format!(
                        "{block}: {lines:?}\n    read:     {read:?}\n    expected: {expected:?}"
                    ));
                }
            }
            // A `;` that was read has ended the statement, so the statement
            // after it on the line is not stepped over with it.
            let parsed = parse_file(&wrap_module(&format!("{open}\nreturn; true\n{close}")));
            if messages(&parsed).first() != Some(&message) || statements(&parsed) != truth {
                failures.push(format!(
                    "{block}: \"return; true\"\n    read:     {:?} {:?}",
                    messages(&parsed),
                    statements(&parsed)
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "{} blocks do not report a statement they cannot read, or step over more than it:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }

    #[test]
    fn a_quantifier_on_the_line_below_a_quantifier_header_is_its_body() {
        // The header of a module-level quantifier ends in `:`, and the line
        // below it begins with `for_all` or `exists`. Each of the two words
        // begins a declaration and also an expression. Below such a header
        // the line is the body of the quantifier.
        for (lines, keyword) in [
            (
                "for_all x in {1, 2} :\nexists y in {1, 2} : x == y",
                "for_all",
            ),
            (
                "exists x in {1, 2} :\nfor_all y in {1, 2} : x == y",
                "exists",
            ),
            // The body has a body on a third line.
            (
                "for_all x in A :\nfor_all y in B :\nexists z in C : p(x, y, z)",
                "for_all",
            ),
            // A blank line and a comment line stand between the two.
            (
                "for_all x in A :\n\n// why\nexists y in B : p(x, y)",
                "for_all",
            ),
            // The same on one line.
            (
                "for_all x in {1, 2} : exists y in {1, 2} : x == y",
                "for_all",
            ),
        ] {
            let parsed = parse_file(&wrap_module(&format!("{lines}\nproposition Witness()")));
            assert_eq!(messages(&parsed), Vec::<&str>::new(), "{lines:?}");
            assert_eq!(
                item_names(&parsed),
                [(keyword, Some("x")), ("proposition", Some("Witness"))],
                "{lines:?}"
            );
            assert_eq!(
                first_decl(&parsed).source,
                format!("{lines}\n"),
                "{lines:?}"
            );
        }
        // Any other declaration keyword on the line below begins a
        // declaration of its own, also below the header of a quantifier. And
        // below any other header, so does `for_all` or `exists`.
        for (lines, diagnostics, items) in [
            (
                "for_all x in {1, 2} :\nproposition Witness()",
                vec![STOPS_SHORT],
                vec![("for_all", Some("x")), ("proposition", Some("Witness"))],
            ),
            (
                "for_all x in {1, 2} :\nrule R : derive {\nthen derive Q()\n}",
                vec![STOPS_SHORT],
                vec![("for_all", Some("x")), ("rule", Some("R"))],
            ),
            (
                "entity E :\nfor_all y in {1, 2} : y == y",
                vec![NEEDS_A_TYPE],
                vec![("entity", Some("E")), ("for_all", Some("y"))],
            ),
            (
                "power W =\nexists y in {1, 2} : y == y",
                vec![STOPS_SHORT],
                vec![("power", Some("W")), ("exists", Some("y"))],
            ),
        ] {
            let parsed = parse_file(&wrap_module(lines));
            assert_eq!(messages(&parsed), diagnostics, "{lines:?}");
            assert_eq!(item_names(&parsed), items, "{lines:?}");
        }
    }

    #[test]
    fn a_name_on_the_line_below_a_header_that_is_a_declaration_keyword_begins_a_declaration() {
        // A type, a first variant, a part of a name or a type argument
        // stands on the line below the token it follows, and its name is a
        // word that begins a declaration. The line is read as that
        // declaration, and the header above it has stopped short.
        for (lines, diagnostics, items) in [
            (
                "entity Pat :\noffice",
                vec![NEEDS_A_TYPE, NEEDS_A_NAME],
                vec![("entity", Some("Pat")), ("office", None)],
            ),
            (
                "type Kind =\nrule | duty",
                vec![NEEDS_A_TYPE, "expected rule name", RULE_NEEDS_A_BODY],
                vec![("type", Some("Kind")), ("rule", None)],
            ),
            (
                "entity Pat : Lib.\noffice",
                vec![NEEDS_A_NAME_PART, NEEDS_A_NAME],
                vec![("entity", Some("Pat")), ("office", None)],
            ),
            (
                "entity Pat : Map<\nsource,\nInt>",
                vec!["expected `>`", "expected source name"],
                vec![("entity", Some("Pat")), ("source", None)],
            ),
        ] {
            let parsed = parse_file(&wrap_module(lines));
            assert_eq!(messages(&parsed), diagnostics, "{lines:?}");
            assert_eq!(item_names(&parsed), items, "{lines:?}");
        }
    }

    #[test]
    fn a_when_a_require_or_a_source_with_nothing_after_it_is_e100() {
        let consequence = format!("then derive {:?}", call("Q", Vec::new()));
        for (word, message) in [
            ("when", "expected a condition after `when`"),
            ("require", "expected a condition after `require`"),
            ("source", "expected a source after `source`"),
        ] {
            // The word stands alone above a statement, has a comment or a
            // `;` where its expression would stand, or is on the last line
            // of the rule. Beside each: whether the word begins the lines.
            for (lines, first) in [
                (format!("{word}\nrequire true\nthen derive Q()"), true),
                (
                    format!("{word} // operative A()\nrequire true\nthen derive Q()"),
                    true,
                ),
                (format!("{word} ;\nrequire true\nthen derive Q()"), true),
                (format!("require true\nthen derive Q()\n{word}"), false),
                (
                    format!("require true\nthen derive Q()\n{word} // operative A()"),
                    false,
                ),
            ] {
                let module = wrap_module(&format!("rule R : derive {{\n{lines}\n}}"));
                let parsed = parse_file(&module);
                assert_eq!(messages(&parsed), [message], "{lines:?}");
                // The diagnostic points at the word.
                let span = parsed.diagnostics[0].primary_span.expect("a span");
                let in_lines = if first {
                    0
                } else {
                    lines.rfind(word).expect("the word")
                };
                let word_start = module.find(&lines).expect("the lines") + in_lines;
                assert_eq!(
                    (span.start as usize, span.end as usize),
                    (word_start, word_start + word.len()),
                    "{lines:?}"
                );
                // The statements around it are kept, and nothing is read
                // for the word.
                let rule = first_decl(&parsed);
                assert_eq!(rule.guard, None, "{lines:?}");
                assert_eq!(rule.require, Some(Expr::Bool(true)), "{lines:?}");
                assert_eq!(rule.source_basis, None, "{lines:?}");
                assert_eq!(
                    rule_statements(&parsed),
                    std::slice::from_ref(&consequence),
                    "{lines:?}"
                );
            }
            // A line that begins with `then` continues the line above it.
            // Below the word it is what is left of the statement, so it is
            // stepped over with it.
            let parsed = parse_file(&wrap_module(&format!(
                "rule R : derive {{\n{word}\nthen derive Q()\n}}"
            )));
            assert_eq!(messages(&parsed), [message], "{word} above then");
            assert!(first_decl(&parsed).consequences.is_empty(), "{word}");
        }
        // With an expression each of the three is read as before.
        let parsed = parse_body(
            "rule R : derive {\nwhen true\nrequire true\nsource \"cite\"\nthen derive Q()\n}",
        );
        let rule = first_decl(&parsed);
        assert_eq!(rule.guard, Some(Expr::Bool(true)));
        assert_eq!(rule.require, Some(Expr::Bool(true)));
        assert_eq!(rule.source_basis, Some(Expr::String("cite".to_owned())));
    }

    #[test]
    fn an_interval_of_a_rule_that_lacks_a_bound_is_e100() {
        for word in ["valid_time", "record_time"] {
            let message = format!("expected two bounds in the interval after `{word}`");
            for interval in ["[, +inf)", "[2026-08-14, )", "[, ]", "[ , )"] {
                let module = wrap_module(&format!(
                    "rule R : derive {{\nwhen true\nthen derive Q()\n{word} {interval}\n}}"
                ));
                let parsed = parse_file(&module);
                assert_eq!(messages(&parsed), [message.as_str()], "{word} {interval}");
                // The diagnostic points at the `[` of the interval.
                let span = parsed.diagnostics[0].primary_span.expect("a span");
                let open = module.find('[').expect("the interval");
                assert_eq!(
                    (span.start as usize, span.end as usize),
                    (open, open + 1),
                    "{word} {interval}"
                );
                assert_eq!(
                    rule_statements(&parsed),
                    [
                        "when Bool(true)".to_owned(),
                        format!("then derive {:?}", call("Q", Vec::new())),
                    ],
                    "{word} {interval}"
                );
            }
        }
        // Where an interval is an expression, one with a missing bound is
        // read as it was before.
        let parsed = parse_body("query q() -> Bool { return [, +inf) }");
        assert!(first_decl(&parsed).expr.is_some());
    }

    #[test]
    fn type_arguments_of_a_call_nested_deeper_than_the_limit_are_e100() {
        // `f<A<A<...>>>(1)` with `depth` levels of type arguments.
        let call_with = |depth: usize| {
            format!(
                "query q() -> Int {{ return f<{}A{}(1) }}\nproposition Witness()",
                "A<".repeat(depth - 1),
                ">".repeat(depth)
            )
        };
        // At the limit the arguments are read.
        let parsed = parse_file(&wrap_module(&call_with(128)));
        assert_eq!(messages(&parsed), Vec::<&str>::new());
        // One level deeper, and far deeper, the module is rejected and the
        // declaration after the query is read. With no limit the parser
        // called itself once for each level, and a deep nesting used the
        // stack up.
        for depth in [129, 5_000] {
            let parsed = parse_file(&wrap_module(&call_with(depth)));
            assert_eq!(
                messages(&parsed),
                ["type nesting exceeds the limit of 128"],
                "{depth} levels"
            );
            assert_eq!(
                item_names(&parsed),
                [("query", Some("q")), ("proposition", Some("Witness"))],
                "{depth} levels"
            );
        }
    }

    #[test]
    fn a_chain_of_else_if_longer_than_the_limit_is_e100() {
        // `if c { 1 } else if c { 1 } ... else { 2 }` with `links` times
        // `else if`.
        let chain = |links: usize| {
            format!(
                "calc f(c: Bool) -> Int {{ if c {{ 1 }}{} else {{ 2 }} }}\nproposition Witness()",
                " else if c { 1 }".repeat(links)
            )
        };
        // The first `if` is one level, each link is one more, and so is the
        // condition of the last link: 126 links are 128 levels.
        let parsed = parse_file(&wrap_module(&chain(126)));
        assert_eq!(messages(&parsed), Vec::<&str>::new());
        // Each `else if` nests in the `if` before it. A chain deeper than
        // the limit on expressions is rejected, and the declaration after
        // it is read. With no count of the links the parser called itself
        // once for each, and a long chain used the stack up.
        for links in [127, 200, 5_000] {
            let parsed = parse_file(&wrap_module(&chain(links)));
            assert!(
                messages(&parsed).contains(&"expression nesting exceeds the limit of 128"),
                "{links} links: {:?}",
                messages(&parsed)
            );
            assert_eq!(
                item_names(&parsed),
                [("calc", Some("f")), ("proposition", Some("Witness"))],
                "{links} links"
            );
        }
    }

    #[test]
    fn a_long_run_of_blank_lines_is_parsed_like_one_blank_line_and_in_time() {
        // Two declarations with blank lines between them.
        fn tree(blank_lines: usize) -> (Vec<String>, Vec<String>) {
            let parsed = parse_file(&format!(
                "module X version \"0.1.0\" {{\nproposition A(x: Int)\n{}rule R : derive {{\nwhen true\nthen derive A(1)\n}}\n}}\n",
                "\n".repeat(blank_lines)
            ));
            let diagnostics = parsed
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect();
            // The declarations as the parser read them, without the places
            // they stand at.
            let declarations = parsed
                .module()
                .expect("a module")
                .items
                .iter()
                .filter_map(Item::decl)
                .map(|decl| {
                    let mut decl = decl.clone();
                    decl.span = Span { start: 0, end: 0 };
                    format!("{decl:?}")
                })
                .collect();
            (diagnostics, declarations)
        }
        // The parse of 100,000 blank lines runs on a thread, and the test
        // fails when it has not returned after 20 seconds. The bound is
        // there to catch a return of the scan that walked the whole run of
        // blank lines for each of its newlines. That scan took time that
        // grows with the square of the run: minutes for this one.
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(tree(100_000));
        });
        let long = receiver
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("the parse of 100,000 blank lines returns within 20 seconds");
        let short = tree(1);
        assert_eq!(short.0, Vec::<String>::new());
        assert_eq!(short.1.len(), 2);
        assert_eq!(long, short);
    }

    /// The three places that hold a list of expressions separated by
    /// commas: the arguments of a call, the elements of a set literal and
    /// the steps of a transaction block.
    const LIST_PLACES: [&str; 3] = ["call", "set", "transaction"];
    const TOO_DEEP: &str = "type nesting exceeds the limit of 128";
    const TREE_TOO_LARGE: &str = "expression tree exceeds depth or node budget";

    /// A `return` of `count` times `x`, `operator`, `y`, separated by commas,
    /// in one of `LIST_PLACES`. `last` is one more element after them.
    fn comparison_list(place: &str, operator: &str, count: usize, last: Option<&str>) -> String {
        let mut items = vec![format!("x {operator} y"); count];
        items.extend(last.map(str::to_owned));
        let items = items.join(", ");
        match place {
            "call" => format!("return g({items})"),
            "set" => format!("return {{{items}}}"),
            "transaction" => format!("return transaction {{ {items} }}"),
            other => panic!("{other}"),
        }
    }

    /// The tree of `comparison_list` when each element is read as the
    /// comparison `op` of `x` and `y`.
    fn comparison_tree(place: &str, op: BinOp, count: usize, last: Option<Expr>) -> Expr {
        let mut items = vec![binary(op, ident("x"), ident("y")); count];
        items.extend(last);
        match place {
            "call" => call("g", items),
            "set" => Expr::Set(items),
            "transaction" => Expr::Apply {
                callee: Box::new(ident("transaction")),
                args: items,
            },
            other => panic!("{other}"),
        }
    }

    #[test]
    fn a_long_list_of_comparisons_is_read_as_comparisons() {
        // In `g(x < y, x < y)` the scan for type arguments that begins at the
        // first `<` goes on over each `x <` after a comma, as if arguments
        // nested there. No `>` closes them, so the list holds comparisons,
        // however long it is. A scan that gave up at 128 levels rejected a
        // list of 129.
        for place in LIST_PLACES {
            for count in [129, 300] {
                let read = calc_body(&comparison_list(place, "<", count, None));
                let tree = comparison_tree(place, BinOp::Lt, count, None);
                assert!(
                    read == (Vec::new(), Some(tree)),
                    "{place}, {count} comparisons: {:?}",
                    read.0
                );
            }
            // 5,000 comparisons are more than the tree of one expression may
            // hold. They get the diagnostics of the same list written with
            // `<=`, which no scan for type arguments reads, and none of them
            // names type nesting.
            let read = calc_body(&comparison_list(place, "<", 5_000, None)).0;
            let control = calc_body(&comparison_list(place, "<=", 5_000, None)).0;
            assert!(
                control.iter().any(|message| message == TREE_TOO_LARGE),
                "{place}: {control:?}"
            );
            assert_eq!(read, control, "{place}, 5,000 comparisons");
        }
    }

    #[test]
    fn type_arguments_after_a_list_of_comparisons_are_read_on_their_own() {
        for (place, after_too_deep) in [
            // After type arguments that are too deep the rest of the
            // statement is stepped over, and in a call that takes the `)`
            // of the call with it.
            ("call", vec!["expected `)`"]),
            ("set", Vec::new()),
            ("transaction", Vec::new()),
        ] {
            // A call with type arguments is the last element, after 200
            // comparisons. Its arguments close and `(` follows them, so they
            // are type arguments, and the comparisons before it stay
            // comparisons.
            let read = calc_body(&comparison_list(place, "<", 200, Some("h<A>(1)")));
            let last = call("h<A>", vec![Expr::Int(1)]);
            let tree = comparison_tree(place, BinOp::Lt, 200, Some(last));
            assert!(read == (Vec::new(), Some(tree)), "{place}: {:?}", read.0);
            // The same call with type arguments nested 129 deep. They close
            // too, so they are type arguments that are deeper than the
            // limit: one diagnostic, at that call and at none of the
            // comparisons.
            let deep = format!("h<{}A{}(1)", "A<".repeat(128), ">".repeat(129));
            let read = calc_body(&comparison_list(place, "<", 200, Some(&deep))).0;
            let mut expected = vec![TOO_DEEP];
            expected.extend(after_too_deep);
            assert_eq!(read, expected, "{place}");
        }
        // One comparison before type arguments nested as deep as the limit
        // allows does not make them too deep.
        let deepest = format!("h<{}A{}(1)", "A<".repeat(127), ">".repeat(128));
        let read = calc_body(&comparison_list("call", "<", 1, Some(&deepest))).0;
        assert_eq!(read, Vec::<String>::new());
    }

    #[test]
    fn a_list_of_comparisons_as_long_as_the_budgets_allow_is_read_in_time() {
        // A set literal of `count` comparisons written with no white space,
        // which is four tokens for each element, and a declaration below it.
        fn module(count: usize) -> String {
            let list = vec!["x<y"; count].join(",");
            format!(
                "module X version \"0.1.0\" {{\ncalc f() -> Int {{ return {{{list}}} }}\nproposition Witness()\n}}\n"
            )
        }
        fn read(count: usize) -> (Vec<String>, Vec<(String, Option<String>)>) {
            let parsed = parse_file(&module(count));
            let diagnostics = messages(&parsed).into_iter().map(str::to_owned).collect();
            let items = item_names(&parsed)
                .into_iter()
                .map(|(keyword, name)| (keyword.to_owned(), name.map(str::to_owned)))
                .collect();
            (diagnostics, items)
        }
        let both = vec![
            ("calc".to_owned(), Some("f".to_owned())),
            ("proposition".to_owned(), Some("Witness".to_owned())),
        ];

        // The tree of one expression holds 10,000 nodes: the set and 3,333
        // comparisons of three nodes each. That list is read with no
        // diagnostic.
        let parsed = parse_file(&module(3_333));
        assert_eq!(messages(&parsed), Vec::<&str>::new());
        assert!(
            first_decl(&parsed).expr == Some(comparison_tree("set", BinOp::Lt, 3_333, None)),
            "the tree of 3,333 comparisons"
        );
        // One comparison more is too many for the tree, and is rejected for
        // that. The declaration below is read.
        let too_large = read(3_334);
        assert!(
            too_large.0.iter().any(|message| message == TREE_TOO_LARGE),
            "{:?}",
            too_large.0
        );
        assert!(!too_large.0.iter().any(|message| message == TOO_DEEP));
        assert_eq!(too_large.1, both);

        // The longest list the token budget holds: one element more, and the
        // source is refused before it is parsed.
        let wrapper = lex_bounded(&module(0), MAX_SOURCE_TOKENS)
            .expect("the module with an empty set")
            .len();
        let longest = (MAX_SOURCE_TOKENS - wrapper + 1) / 4;
        assert!(longest > 65_000, "{longest}");
        assert!(lex_bounded(&module(longest), MAX_SOURCE_TOKENS).is_ok());
        assert!(lex_bounded(&module(longest + 1), MAX_SOURCE_TOKENS).is_err());
        // Its parse runs on a thread, and the test fails when it has not
        // returned after 20 seconds. The bound is there to catch a return of
        // the scan that began again at each `<` of the list and ran to the
        // end of it. That scan called itself once for each comparison, which
        // used the stack up for a list of 65,000, and it took time that
        // grows with the square of the length of the list: more than two
        // minutes for a list of 32,000.
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(read(longest));
        });
        let long = receiver
            .recv_timeout(std::time::Duration::from_secs(20))
            .expect("the parse of the longest list returns within 20 seconds");
        // It is read as the list of 3,334 is: too large for the tree, with
        // no diagnostic that names type nesting, and the declaration below
        // it is read.
        assert_eq!(long, too_large);
    }
}

/// The line rule asks about the tokens around a newline, and `LineIndex`
/// answers from lists computed once. These tests hold the questions as they
/// are defined, by a walk over the tokens, and compare the two for every
/// newline, every cursor position and every view of the tokens.
#[cfg(test)]
mod line_index_tests {
    use super::*;

    fn significant(kind: TokenKind) -> bool {
        !Parser::is_trivia(kind) && kind != TokenKind::Newline
    }

    /// `newline_ends_statement` by its definition.
    fn ends_statement(p: &Parser<'_>, index: usize) -> bool {
        let before = p.tokens[..index]
            .iter()
            .rev()
            .find(|token| significant(token.kind));
        let needs_operand = before.is_some_and(|token| {
            !Parser::can_end_statement(token.kind)
                || token.kind == TokenKind::Ident && Parser::is_prefix_word(p.text(*token))
        });
        !(line_continues(p, index) || needs_operand)
    }

    /// `next_line_continues` by its definition.
    fn line_continues(p: &Parser<'_>, index: usize) -> bool {
        let mut after = p.tokens[index + 1..]
            .iter()
            .filter(|token| !Parser::is_trivia(token.kind));
        match after.next() {
            Some(first) if first.kind == TokenKind::Newline => after
                .find(|token| token.kind != TokenKind::Newline)
                .is_some_and(|token| p.only_continues(*token)),
            Some(first) => p.is_continuation(*first),
            None => false,
        }
    }

    /// `passed_statement_end` by its definition.
    fn passed_end(p: &Parser<'_>) -> bool {
        p.tokens[..p.idx]
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_, token)| !significant(token.kind))
            .any(|(index, token)| token.kind == TokenKind::Newline && ends_statement(p, index))
    }

    fn place(token: Option<Token>) -> Option<(u32, u32)> {
        token.map(|token| (token.start, token.end))
    }

    /// Every answer the parser takes from the index, for the cursor at
    /// `idx`, beside the same answer found by walking the tokens.
    fn compare_at(p: &mut Parser<'_>, idx: usize, source: &str, failures: &mut Vec<String>) {
        p.idx = idx;
        let visible = p.tokens.len();
        let last = p.tokens[..idx]
            .iter()
            .rev()
            .find(|token| significant(token.kind))
            .copied();
        let line_end = p.tokens[..idx]
            .iter()
            .rev()
            .take_while(|token| !significant(token.kind))
            .any(|token| token.kind == TokenKind::Newline);
        let ahead: Vec<Token> = p.tokens[idx..]
            .iter()
            .filter(|token| significant(token.kind))
            .copied()
            .collect();
        let nth = |n: usize| {
            place(Some(
                ahead.get(n).copied().unwrap_or_else(|| p.end_of_input()),
            ))
        };
        let checks = [
            (
                "passed_statement_end",
                p.passed_statement_end() == passed_end(p),
            ),
            ("last_read", place(p.last_read()) == place(last)),
            ("line_end_passed", p.line_end_passed() == line_end),
            (
                "peek_significant",
                place(Some(p.peek_significant())) == nth(0),
            ),
            ("peek_nth(1)", place(Some(p.peek_nth(1))) == nth(1)),
            ("peek_nth(3)", place(Some(p.peek_nth(3))) == nth(3)),
        ];
        for (question, same) in checks {
            if !same {
                failures.push(format!(
                    "{question} at token {idx} of {visible} visible in {source:?}"
                ));
            }
        }
        if idx < visible && p.tokens[idx].kind == TokenKind::Newline {
            let declaration = p.tokens[idx + 1..]
                .iter()
                .find(|token| significant(token.kind))
                .is_some_and(|token| {
                    token.kind == TokenKind::Ident && is_declaration_keyword(p.text(*token))
                });
            let checks = [
                (
                    "newline_ends_statement",
                    p.newline_ends_statement(idx) == ends_statement(p, idx),
                ),
                (
                    "next_line_continues",
                    p.next_line_continues(idx) == line_continues(p, idx),
                ),
                (
                    "declaration_begins_after",
                    p.declaration_begins_after(idx) == declaration,
                ),
            ];
            for (question, same) in checks {
                if !same {
                    failures.push(format!(
                        "{question} for the newline at token {idx} of {visible} visible in {source:?}"
                    ));
                }
            }
        }
    }

    #[test]
    fn the_index_answers_as_a_walk_over_the_tokens_does() {
        // What a line can end with, what can stand between two lines, and
        // what the next line can begin with: tokens that end a statement and
        // tokens that do not, a prefix word, blank lines and comment lines,
        // continuations that only continue and ones that also begin a line,
        // a declaration keyword, and the end of a block.
        let endings = ["a", "f(x)", "a +", "a :", "not", "a,", "{", "x }", "a ; "];
        let gaps = [
            "\n",
            "\n\n",
            "\n \n\n",
            " // c\n",
            "\n// c\n",
            "\n\n// c\n\n  ",
            "\n// c",
        ];
        let beginnings = [
            "b",
            "and b",
            "then b",
            ". b",
            "( b )",
            "{ b }",
            "proposition P()",
            "}",
            "",
        ];
        let mut failures = Vec::new();
        let mut sources = 0;
        for ending in endings {
            for gap in gaps {
                for beginning in beginnings {
                    let source = format!("q {ending}{gap}{beginning}{gap}c");
                    let tokens = lex_bounded(&source, MAX_SOURCE_TOKENS).expect("tokens");
                    // `with_tokens_until` hides the tokens from an index on.
                    // The index is made for all of them and has to answer
                    // for each such view.
                    for visible in 0..=tokens.len() {
                        let mut p = Parser {
                            source: &source,
                            tokens: &tokens[..visible],
                            lines: LineIndex::new(&tokens),
                            idx: 0,
                            diagnostics: Vec::new(),
                            builder: GreenNodeBuilder::new(),
                            node_depth: 0,
                            token_cursor: 0,
                            expression_depth: 0,
                            judgment_body: false,
                            brace_opens_enclosing_block: false,
                            observation_body: false,
                            angle_scans: HashMap::new(),
                        };
                        for idx in 0..=visible {
                            compare_at(&mut p, idx, &source, &mut failures);
                        }
                    }
                    sources += 1;
                }
            }
        }
        assert_eq!(sources, 9 * 7 * 9);
        assert!(
            failures.is_empty(),
            "{} answers of the index differ from a walk over the tokens:\n{}",
            failures.len(),
            failures
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

/// `call_generics_ahead` decides whether a `<` after a name begins the type
/// arguments of a call. It answers from a scan that calls itself for no
/// nesting and keeps what it found for each `<` it passed. These tests hold
/// the question as it is defined, by a function that calls itself, and
/// compare the two for every `<`, in each order of asking and under views of
/// the tokens that end at different places.
#[cfg(test)]
mod type_argument_scan_tests {
    use super::*;

    fn parser<'a>(source: &'a str, tokens: &'a [Token], visible: usize) -> Parser<'a> {
        Parser {
            source,
            tokens: &tokens[..visible],
            lines: LineIndex::new(tokens),
            idx: 0,
            diagnostics: Vec::new(),
            builder: GreenNodeBuilder::new(),
            node_depth: 0,
            token_cursor: 0,
            expression_depth: 0,
            judgment_body: false,
            brace_opens_enclosing_block: false,
            observation_body: false,
            angle_scans: HashMap::new(),
        }
    }

    /// The tokens that are no white space, comment or newline, each with its
    /// index.
    fn significant(tokens: &[Token]) -> Vec<(usize, TokenKind)> {
        tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| !Parser::is_trivia(token.kind) && token.kind != TokenKind::Newline)
            .map(|(index, token)| (index, token.kind))
            .collect()
    }

    fn kind_at(tokens: &[(usize, TokenKind)], at: usize) -> Option<TokenKind> {
        tokens.get(at).map(|(_, kind)| *kind)
    }

    fn is_name(kind: Option<TokenKind>) -> bool {
        matches!(kind, Some(TokenKind::Ident | TokenKind::DurationUnit))
    }

    /// Type arguments by their definition: `<`, types separated by commas,
    /// `>`. `at` is the place of the `<` among the significant tokens. The
    /// answer is the place after the `>` and the depth of the nesting, or
    /// nothing when the arguments do not close.
    fn arguments(tokens: &[(usize, TokenKind)], at: usize) -> Option<(usize, usize)> {
        let mut at = at + 1;
        let mut deepest = 0;
        loop {
            let (after, depth) = one_type(tokens, at)?;
            deepest = deepest.max(depth);
            match kind_at(tokens, after) {
                Some(TokenKind::Comma) => at = after + 1,
                Some(TokenKind::Gt) => return Some((after + 1, deepest + 1)),
                _ => return None,
            }
        }
    }

    /// A type by its definition: a name, parts after `.`, and type arguments
    /// of its own when a `<` follows. The answer is the place after it and
    /// the depth of its arguments, 0 when it has none.
    fn one_type(tokens: &[(usize, TokenKind)], at: usize) -> Option<(usize, usize)> {
        if !is_name(kind_at(tokens, at)) {
            return None;
        }
        let mut at = at + 1;
        while kind_at(tokens, at) == Some(TokenKind::Dot) {
            if !is_name(kind_at(tokens, at + 1)) {
                return None;
            }
            at += 2;
        }
        if kind_at(tokens, at) == Some(TokenKind::Lt) {
            return arguments(tokens, at);
        }
        Some((at, 0))
    }

    /// What `call_generics_ahead` must answer at the `<` in place `at` of
    /// the significant tokens `list`, and the index the cursor must stand at
    /// after the arguments are stepped over, when they are read.
    fn defined(
        tokens: &[Token],
        list: &[(usize, TokenKind)],
        at: usize,
    ) -> (TypeArgs, Option<usize>) {
        let Some((after, depth)) = arguments(list, at) else {
            return (TypeArgs::Absent, None);
        };
        if !matches!(
            kind_at(list, after),
            Some(TokenKind::LParen | TokenKind::LBrace)
        ) {
            return (TypeArgs::Absent, None);
        }
        if depth > MAX_EXPRESSION_DEPTH {
            return (TypeArgs::TooDeep, None);
        }
        // The cursor stands after the `>` and the white space and comments
        // that follow it.
        let mut cursor = list[after - 1].0 + 1;
        while tokens
            .get(cursor)
            .is_some_and(|token| Parser::is_trivia(token.kind))
        {
            cursor += 1;
        }
        (TypeArgs::Read, Some(cursor))
    }

    /// The views a source of `count` tokens is compared under, each a number
    /// of tokens that are visible. A short source is compared under every
    /// view. A long one is compared whole, under each view that ends within
    /// its last eight tokens, where its arguments lose their end, and under
    /// the views that end at its quarter and at its half.
    fn views(count: usize) -> Vec<usize> {
        if count <= 64 {
            return (0..=count).collect();
        }
        let mut views = vec![count / 4, count / 2];
        views.extend(count - 8..=count);
        views
    }

    /// Fails the comparison for every `<` of `source` that
    /// `call_generics_ahead` does not answer as defined: asked from the
    /// first to the last, from the last to the first, and under each view of
    /// the tokens in turn on one parser.
    fn compare(source: &str, failures: &mut Vec<String>) {
        let tokens = lex_bounded(source, MAX_SOURCE_TOKENS).expect("tokens");
        let views = views(tokens.len());
        let mut check = |p: &mut Parser<'_>,
                         list: &[(usize, TokenKind)],
                         at: usize,
                         order: &str| {
            let visible = p.tokens.len();
            let index = list[at].0;
            let expected = defined(&tokens[..visible], list, at);
            p.idx = index;
            let answered = p.call_generics_ahead();
            let mut stands_at = None;
            if p.idx != index {
                failures.push(format!(
                    "{order}: the cursor moved, at {index} of {source:?}"
                ));
            } else if answered == TypeArgs::Read {
                let _ = p.consume_generic_args_text();
                stands_at = Some(p.idx);
            }
            if (answered, stands_at) != expected {
                failures.push(format!(
                    "{order}, {visible} tokens visible, `<` at {index} of {source:?}: {:?}, defined {expected:?}",
                    (answered, stands_at)
                ));
            }
        };
        // The places of each `<` among the significant tokens.
        let angles = |list: &[(usize, TokenKind)]| -> Vec<usize> {
            (0..list.len())
                .filter(|at| list[*at].1 == TokenKind::Lt)
                .collect()
        };
        // One parser for all the views, from the widest to the narrowest and
        // back: what was kept under one view must not answer under another.
        let mut shared = parser(source, &tokens, tokens.len());
        for &visible in views.iter().rev().chain(&views) {
            let list = significant(&tokens[..visible]);
            shared.tokens = &tokens[..visible];
            for at in angles(&list) {
                check(&mut shared, &list, at, "views in turn");
            }
        }
        for &visible in &views {
            let list = significant(&tokens[..visible]);
            let mut forward = parser(source, &tokens, visible);
            for at in angles(&list) {
                check(&mut forward, &list, at, "first to last");
            }
            let mut backward = parser(source, &tokens, visible);
            for at in angles(&list).into_iter().rev() {
                check(&mut backward, &list, at, "last to first");
            }
        }
    }

    #[test]
    fn the_scan_answers_as_the_definition_of_type_arguments_does() {
        let mut failures = Vec::new();
        // Written cases: lists of comparisons, type arguments, the two
        // mixed, parts after `.`, unit words as names, line ends and
        // comments between the tokens, and arguments that stop short.
        for source in [
            "g(x < y, x < y, x < y)",
            "{x < y, x < y}",
            "f<A>(1)",
            "f<A, B>(1)",
            "f<A<B>, C<D<E>>>(1)",
            "f<A> {1}",
            "f<A> + 1",
            "f<A>",
            "f<A",
            "f<A,>(1)",
            "f<>(1)",
            "f<A.B.C<D.E>>(1)",
            "f<A.>(1)",
            "f<A.<B>>(1)",
            "f<days, hours<minutes>>(1)",
            "g(x < y, h<A>(1), x < y)",
            "g(x < y, h<A<B>>(1))",
            "g(x < h<A>(1), y)",
            "a < b<c>(1), d",
            "a < b, c > (1)",
            "a < b, c > d",
            "f<A\n>(1)",
            "f<A // why\n, B>\n(1)",
            "f\n<A>(1)",
            "x < y\nassert f<A>(1)",
            "a < b < c > > (1)",
            "a < b < c > , d > (1)",
            "a << b >> (1)",
            "f<A>> (1)",
        ] {
            compare(source, &mut failures);
        }
        // Type arguments at the limit and past it, alone and behind a
        // comparison, and a list of comparisons longer than the limit.
        for depth in [127, 128, 129, 130] {
            let nested = format!("{}A{}", "A<".repeat(depth - 1), ">".repeat(depth));
            compare(&format!("f<{nested}(1)"), &mut failures);
            compare(&format!("g(x < y, h<{nested}(1))"), &mut failures);
            compare(&format!("f<{nested} + 1"), &mut failures);
        }
        compare(
            &format!("g({})", vec!["x < y"; 140].join(", ")),
            &mut failures,
        );
        // Every sequence of up to six of these tokens.
        let alphabet = ["a", "<", ">", ",", ".", "(", "{", "1"];
        let mut sequences = 0;
        for length in 1..=6 {
            for code in 0..alphabet.len().pow(length) {
                let mut code = code;
                let mut parts = Vec::new();
                for _ in 0..length {
                    parts.push(alphabet[code % alphabet.len()]);
                    code /= alphabet.len();
                }
                // Without a `<` after a name there is nothing to ask.
                if !parts.windows(2).any(|pair| pair == ["a", "<"]) {
                    continue;
                }
                compare(&parts.join(" "), &mut failures);
                sequences += 1;
            }
        }
        assert_eq!(sequences, 22_329);
        assert!(
            failures.is_empty(),
            "{} answers differ from the definition:\n{}",
            failures.len(),
            failures
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
