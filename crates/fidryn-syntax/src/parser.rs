//! Recursive-descent parser with automatic semicolon insertion and a Pratt expression parser.

use crate::ast::{BinOp, ConsequenceAst, Decl, Expr, GoalAst, Header, HeaderKind, Item, Module};
use crate::lexer::{Token, TokenKind, lex};
use crate::syntax::{FidrynLanguage, SyntaxKind};
use fidryn_core::Span;
use fidryn_core::{Diagnostic, DiagnosticCode};
use rowan::GreenNodeBuilder;
use std::collections::BTreeMap;

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
    let tokens = lex(source);
    let mut diagnostics = Vec::new();
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
    }
    let mut p = Parser {
        source,
        tokens: &tokens,
        idx: 0,
        diagnostics,
        builder: GreenNodeBuilder::new(),
        node_depth: 0,
        token_cursor: 0,
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

struct Parser<'a> {
    source: &'a str,
    tokens: &'a [Token],
    idx: usize,
    diagnostics: Vec<Diagnostic>,
    builder: GreenNodeBuilder<'static>,
    node_depth: usize,
    token_cursor: usize,
}

#[derive(Clone, Copy)]
enum InfixOp {
    Bin(BinOp),
    Word(&'static str),
}

impl<'a> Parser<'a> {
    fn peek_raw(&self) -> Token {
        self.tokens.get(self.idx).copied().unwrap_or(Token {
            kind: TokenKind::Eof,
            start: self.source.len() as u32,
            end: self.source.len() as u32,
        })
    }

    fn is_trivia(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Whitespace | TokenKind::Comment | TokenKind::DocComment
        )
    }

    fn peek(&mut self) -> Token {
        self.apply_asi();
        let t = self.peek_raw();
        if t.kind == TokenKind::Newline {
            self.idx += 1;
            return self.peek();
        }
        t
    }

    fn apply_asi(&mut self) {
        loop {
            while Self::is_trivia(self.peek_raw().kind) {
                self.idx += 1;
            }
            if self.peek_raw().kind != TokenKind::Newline {
                return;
            }
            let after = self.lookahead_non_trivia(1);
            if self.is_continuation(after) {
                self.idx += 1;
                continue;
            }
            return;
        }
    }

    fn peek_significant(&self) -> Token {
        let mut i = self.idx;
        while let Some(t) = self.tokens.get(i) {
            if Self::is_trivia(t.kind) || t.kind == TokenKind::Newline {
                i += 1;
                continue;
            }
            return *t;
        }
        Token {
            kind: TokenKind::Eof,
            start: self.source.len() as u32,
            end: self.source.len() as u32,
        }
    }

    fn peek_nth(&self, n: usize) -> Token {
        let mut i = self.idx;
        let mut seen = 0usize;
        while let Some(t) = self.tokens.get(i) {
            if Self::is_trivia(t.kind) || t.kind == TokenKind::Newline {
                i += 1;
                continue;
            }
            if seen == n {
                return *t;
            }
            seen += 1;
            i += 1;
        }
        Token {
            kind: TokenKind::Eof,
            start: self.source.len() as u32,
            end: self.source.len() as u32,
        }
    }

    fn lookahead_non_trivia(&self, mut skip_newlines: usize) -> Token {
        let mut i = self.idx;
        while let Some(t) = self.tokens.get(i) {
            if Self::is_trivia(t.kind) {
                i += 1;
                continue;
            }
            if t.kind == TokenKind::Newline {
                if skip_newlines == 0 {
                    return *t;
                }
                skip_newlines -= 1;
                i += 1;
                continue;
            }
            return *t;
        }
        Token {
            kind: TokenKind::Eof,
            start: self.source.len() as u32,
            end: self.source.len() as u32,
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

    fn eat_semi(&mut self) {
        if self.peek().kind == TokenKind::Semicolon {
            self.bump();
        }
    }

    fn at_stmt_break(&mut self) -> bool {
        self.apply_asi();
        matches!(
            self.peek_raw().kind,
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::Eof
        )
    }

    fn checkpoint(&self) -> (usize, usize) {
        (self.idx, self.diagnostics.len())
    }

    fn restore(&mut self, cp: (usize, usize)) {
        self.idx = cp.0;
        self.diagnostics.truncate(cp.1);
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
            if self.peek().kind == TokenKind::Dot {
                self.bump();
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
            "type" => Some(self.wrap_item(|p| Item::Type(p.parse_decl("type")))),
            "record_type" | "evidence_type" => {
                let kw = kw.to_owned();
                Some(self.wrap_item(|p| Item::RecordType(p.parse_decl(&kw))))
            }
            "office" => Some(self.wrap_item(|p| Item::Office(p.parse_decl("office")))),
            "proposition" => {
                Some(self.wrap_item(|p| Item::Proposition(p.parse_decl("proposition"))))
            }
            "observation" => {
                Some(self.wrap_item(|p| Item::Observation(p.parse_decl("observation"))))
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
            "judgment" => Some(self.wrap_item(|p| Item::Judgment(p.parse_decl("judgment")))),
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
            "verify" => Some(self.wrap_item(|p| Item::Verify(p.parse_decl("verify")))),
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
        self.skip_balanced_until_end();
        let end = self.last_end(start.end);
        self.finish_node();
        Header {
            span: Span {
                start: start.start,
                end,
            },
            kind,
            value: self.src_slice(value_start, end).trim().to_owned(),
        }
    }

    fn parse_decl(&mut self, keyword: &str) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        let mut name = None;
        let t = self.peek();
        if t.kind == TokenKind::Ident {
            name = Some(self.parse_qname());
        }
        let sig_start = self.peek().start;
        self.skip_balanced_until_end();
        let end = self.last_end(start.end);
        let decl = Decl::new(
            Span {
                start: start.start,
                end,
            },
            keyword,
            name,
            Some(self.src_slice(sig_start, end).trim().to_owned()),
            self.src_slice(start.start, end),
        );
        self.finish_node();
        decl
    }

    fn parse_import_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        let name = if self.at_name() || self.peek().kind == TokenKind::Error {
            Some(self.parse_qname())
        } else {
            self.error_here("expected import name");
            None
        };
        let type_args = self.parse_type_args_opt();
        let sig_start = self.peek().start;
        self.skip_balanced_until_end();
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

    fn last_end(&self, fallback: u32) -> u32 {
        if self.idx == 0 {
            fallback
        } else {
            self.tokens
                .get(self.idx.saturating_sub(1))
                .map(|t| t.end)
                .unwrap_or(fallback)
        }
    }

    fn parse_type_string(&mut self) -> String {
        let start = self.peek().start;
        if !self.at_name() {
            self.error_here("expected type");
            return String::new();
        }
        let _ = self.parse_qname();
        if self.peek().kind == TokenKind::Lt {
            let mut depth = 1u32;
            self.bump();
            while depth > 0 {
                match self.peek().kind {
                    TokenKind::Lt => {
                        depth += 1;
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
                        self.skip_balanced_until_end();
                    }
                    continue;
                }
                self.bump();
                let value_start = self.peek().start;
                self.skip_balanced_until_end();
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
            self.skip_balanced_until_end();
        }
        let mut decl = self.finish_decl(start, "source", name);
        decl.fields = fields;
        self.finish_node();
        decl
    }

    fn parse_query_decl(&mut self) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
        let automatic = self.eat_ident("automatic");
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
        if self.peek().kind == TokenKind::Arrow {
            self.bump();
            result_type = Some(self.parse_type_string());
        }
        let effects = self.parse_effect_row_opt();
        let mut expr = None;
        let mut goal = None;
        if self.peek().kind == TokenKind::LBrace {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.eat_ident("goal") {
                    goal = Some(self.parse_goal());
                } else if self.eat_ident("return") {
                    expr = self.parse_expr();
                    self.eat_semi();
                } else if self.eat_ident("require") {
                    self.parse_require_tail();
                } else if self.at_ident("for_all") || self.at_ident("exists") {
                    let _ = self.parse_expr();
                    self.eat_semi();
                } else if let Some(e) = self.parse_expr() {
                    if expr.is_none() {
                        expr = Some(e);
                    }
                    self.eat_semi();
                } else if !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                    self.skip_balanced_until_end();
                }
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
        } else {
            self.skip_balanced_until_end();
        }
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

    fn parse_require_tail(&mut self) {
        let _ = self.parse_expr();
        if self.eat_ident("using") && self.at_name() {
            let _ = self.parse_ident_name();
        }
        self.eat_semi();
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
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.eat_ident("return") {
                    goal.expr = self.parse_expr();
                    self.eat_semi();
                } else if self.eat_ident("require") {
                    self.parse_require_tail();
                } else if self.at_ident("for_all") || self.at_ident("exists") {
                    let _ = self.parse_expr();
                    self.eat_semi();
                } else if let Some(e) = self.parse_expr() {
                    if goal.expr.is_none() {
                        goal.expr = Some(e);
                    }
                    self.eat_semi();
                } else if !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                    self.skip_balanced_until_end();
                }
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
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

    fn parse_function_decl(&mut self, keyword: &str) -> Decl {
        self.start_node(SyntaxKind::DECL);
        let start = self.bump();
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
        if self.peek().kind == TokenKind::Arrow {
            self.bump();
            result_type = Some(self.parse_type_string());
        }
        let effects = self.parse_effect_row_opt();
        let mut fuel = if self.at_ident("fuel") {
            self.parse_fuel_clause()
        } else {
            None
        };
        let (expr, body_fuel) = self.parse_fn_block();
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
            self.skip_balanced_until_end();
            return (None, None);
        }
        self.bump();
        let mut stmts = Vec::new();
        let mut ret = None;
        let mut fuel = None;
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            if self.eat_ident("return") {
                ret = self.parse_expr();
                self.eat_semi();
            } else if self.at_ident("fuel") {
                fuel = self.parse_fuel_clause();
                self.eat_semi();
            } else if let Some(e) = self.parse_expr() {
                stmts.push(e);
                self.eat_semi();
            } else if self.at_ident("else") {
                // `else` is an expr stop-word, so Pratt will not start here.
                // skip_balanced would swallow the else-branch as leftover.
                self.take_fn_block_else(&mut stmts);
            } else if !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                self.skip_balanced_until_end();
            }
        }
        self.expect_kind(TokenKind::RBrace, "expected `}`");
        let expr = ret.or_else(|| {
            if stmts.len() == 1 {
                stmts.pop()
            } else if stmts.is_empty() {
                None
            } else {
                Some(Expr::Block(stmts))
            }
        });
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
        if self.peek().kind == TokenKind::Colon {
            self.bump();
            if self.at_name() {
                rule_kind = Some(self.parse_ident_name());
            } else {
                self.error_here("expected rule kind");
            }
        }
        if self.eat_ident("from") {
            let _ = self.parse_expr();
        }
        let mut guard = None;
        let mut consequences = Vec::new();
        if self.peek().kind == TokenKind::LBrace {
            self.bump();
            while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
                if self.eat_ident("when") {
                    if let Some(e) = self.parse_expr() {
                        guard = Some(match guard.take() {
                            Some(prev) => Expr::Binary {
                                op: BinOp::And,
                                left: Box::new(prev),
                                right: Box::new(e),
                            },
                            None => e,
                        });
                    }
                    self.eat_semi();
                } else if self.eat_ident("then") || self.eat_ident("otherwise") {
                    if self.at_consequence_op() {
                        let verb = self.parse_ident_name();
                        let expr = self.parse_expr().unwrap_or(Expr::Ident(String::new()));
                        if self.eat_ident("as_to") {
                            let _ = self.parse_expr();
                        }
                        consequences.push(ConsequenceAst { verb, expr });
                        self.eat_semi();
                        if !self.at_rule_stmt()
                            && !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof)
                        {
                            self.skip_balanced_until_end();
                        }
                    } else {
                        self.skip_balanced_until_end();
                    }
                } else if self.eat_ident("require") {
                    let _ = self.parse_expr();
                    self.eat_semi();
                } else {
                    self.skip_balanced_until_end();
                }
            }
            self.expect_kind(TokenKind::RBrace, "expected `}`");
        } else {
            self.skip_balanced_until_end();
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
        self.finish_node();
        decl
    }

    fn at_consequence_op(&mut self) -> bool {
        let t = self.peek();
        t.kind == TokenKind::Ident && CONSEQUENCE_OPS.contains(&self.text(t))
    }

    fn at_rule_stmt(&mut self) -> bool {
        let t = self.peek();
        t.kind == TokenKind::Ident
            && matches!(
                self.text(t),
                "when" | "then" | "otherwise" | "require" | "select" | "source"
            )
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

    fn parse_bp(&mut self, min_bp: u8) -> Option<Expr> {
        let mut lhs = self.parse_prefix()?;
        loop {
            if self.at_stmt_break() {
                break;
            }
            if let Some((lbp, rbp, op)) = self.infix_info() {
                if lbp < min_bp {
                    break;
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
                    continue;
                }
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
        if self.at_stmt_break() {
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
            TokenKind::Ident if Self::is_prefix_word(self.text(t)) => {
                let name = self.text(t).to_owned();
                self.bump();
                let rhs = self.parse_bp(PREFIX_RBP)?;
                Some(Expr::Apply {
                    callee: Box::new(Expr::Ident(name)),
                    args: vec![rhs],
                })
            }
            TokenKind::Ident if Self::is_expr_stop_word(self.text(t)) => None,
            TokenKind::Minus => {
                self.bump();
                let rhs = self.parse_bp(PREFIX_RBP)?;
                Some(match rhs {
                    Expr::Int(n) => Expr::Int(-n),
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
        let cond = self.parse_expr().unwrap_or(Expr::Ident(String::new()));
        let then = if self.peek().kind == TokenKind::LBrace {
            self.parse_brace_unwrapped()
        } else {
            self.parse_expr().unwrap_or(Expr::Block(Vec::new()))
        };
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
        if self.at_ident("if") {
            Some(self.parse_if_expr())
        } else if self.peek().kind == TokenKind::LBrace {
            Some(self.parse_brace_unwrapped())
        } else {
            Some(self.parse_expr().unwrap_or(Expr::Block(Vec::new())))
        }
    }

    fn parse_quantifier_expr(&mut self) -> Expr {
        let kind = self.parse_ident_name();
        let binder = self.parse_ident_name();
        if !self.eat_ident("in") {
            self.error_here("expected `in`");
        }
        let domain = self.parse_expr().unwrap_or(Expr::Ident(String::new()));
        let body = if self.peek().kind == TokenKind::Colon {
            self.bump();
            self.parse_expr().unwrap_or(Expr::Block(Vec::new()))
        } else if self.peek().kind == TokenKind::LBrace {
            self.parse_braced_expr()
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
                let s = trim_quotes(self.text(t)).to_owned();
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
                let inner = self.parse_expr();
                self.expect_kind(TokenKind::RParen, "expected `)`");
                inner
            }
            TokenKind::LBrace => Some(self.parse_braced_expr()),
            TokenKind::LBracket => Some(self.parse_interval_expr()),
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
        if self.looks_like_call_generics() {
            name.push_str(&self.consume_generic_args_text());
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
        if self.peek().kind == TokenKind::LBrace {
            let block = self.parse_braced_expr();
            return Expr::Apply {
                callee: Box::new(Expr::Ident(name)),
                args: vec![block],
            };
        }
        Expr::Ident(name)
    }

    fn parse_int_or_duration(&mut self) -> Expr {
        let t = self.bump();
        let n = self.text(t).parse::<i64>().unwrap_or(0);
        if self.peek().kind == TokenKind::DurationUnit {
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
        match self.parse_braced_expr() {
            Expr::Block(mut items) if items.len() == 1 => {
                items.pop().unwrap_or(Expr::Block(Vec::new()))
            }
            other => other,
        }
    }

    fn parse_braced_expr(&mut self) -> Expr {
        self.bump();
        let mut items = Vec::new();
        while !matches!(self.peek().kind, TokenKind::RBrace | TokenKind::Eof) {
            if matches!(self.peek().kind, TokenKind::Comma | TokenKind::Semicolon) {
                self.bump();
                continue;
            }
            if self.at_name() && matches!(self.peek_nth(1).kind, TokenKind::Colon | TokenKind::Eq) {
                let name = self.parse_ident_name();
                self.bump();
                let val = self.parse_expr().unwrap_or(Expr::Ident(String::new()));
                items.push(Expr::Apply {
                    callee: Box::new(Expr::Ident(name)),
                    args: vec![val],
                });
            } else if let Some(e) = self.parse_expr() {
                items.push(e);
            } else if self.at_name() {
                // Pratt returns None on expr stop-words; keep them as block items
                // so `from Name { when ... }` does not fail expecting `}`.
                let name = self.parse_ident_name();
                items.push(self.parse_postfix(Expr::Ident(name)));
            } else {
                break;
            }
            if matches!(self.peek().kind, TokenKind::Comma | TokenKind::Semicolon) {
                self.bump();
            }
        }
        self.expect_kind(TokenKind::RBrace, "expected `}`");
        Expr::Block(items)
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> Expr {
        loop {
            if self.at_stmt_break() {
                break;
            }
            match self.peek().kind {
                TokenKind::LParen => {
                    self.bump();
                    let args = self.parse_args_until_rparen();
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
            return Some(Expr::Apply {
                callee: Box::new(Expr::Ident(name)),
                args: vec![val],
            });
        }
        self.parse_expr()
    }

    fn looks_like_call_generics(&mut self) -> bool {
        if self.peek().kind != TokenKind::Lt {
            return false;
        }
        let cp = self.checkpoint();
        let ok = self.try_generic_args()
            && matches!(self.peek().kind, TokenKind::LParen | TokenKind::LBrace);
        self.restore(cp);
        ok
    }

    fn consume_generic_args_text(&mut self) -> String {
        let start = self.peek().start;
        let _ = self.try_generic_args();
        self.src_slice(start, self.last_end(start))
    }

    fn try_generic_args(&mut self) -> bool {
        if self.peek().kind != TokenKind::Lt {
            return false;
        }
        self.bump();
        loop {
            if !self.try_skip_type() {
                return false;
            }
            if self.peek().kind == TokenKind::Comma {
                self.bump();
                continue;
            }
            if self.peek().kind == TokenKind::Gt {
                self.bump();
                return true;
            }
            return false;
        }
    }

    fn try_skip_type(&mut self) -> bool {
        if !self.at_name() {
            return false;
        }
        self.bump();
        while self.peek().kind == TokenKind::Dot {
            self.bump();
            if !self.at_name() {
                return false;
            }
            self.bump();
        }
        if self.peek().kind == TokenKind::Lt {
            return self.try_generic_args();
        }
        true
    }

    fn skip_balanced_until_end(&mut self) {
        let mut depth_brace = 0u32;
        let mut depth_paren = 0u32;
        let mut depth_brack = 0u32;
        let mut last_sig = TokenKind::Eof;
        loop {
            let t = self.peek_raw();
            if Self::is_trivia(t.kind) {
                self.idx += 1;
                continue;
            }
            match t.kind {
                TokenKind::Eof => return,
                TokenKind::LBrace => {
                    depth_brace += 1;
                    last_sig = t.kind;
                    self.idx += 1;
                }
                TokenKind::RBrace => {
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 {
                        return;
                    }
                    depth_brace = depth_brace.saturating_sub(1);
                    last_sig = t.kind;
                    self.idx += 1;
                    if depth_brace == 0 && depth_paren == 0 && depth_brack == 0 {
                        if self.is_continuation(self.peek_significant()) {
                            continue;
                        }
                        return;
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
                        return;
                    }
                }
                TokenKind::Newline => {
                    let after = self.lookahead_non_trivia(1);
                    if depth_brace == 0
                        && depth_paren == 0
                        && depth_brack == 0
                        && Self::can_end_statement(last_sig)
                        && !self.is_continuation(after)
                    {
                        self.idx += 1;
                        return;
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
                            Expr::Apply { callee, args } => {
                                assert_eq!(callee.as_ref(), &Expr::Ident("fee".to_owned()));
                                assert_eq!(
                                    args,
                                    &vec![Expr::Call {
                                        callee: "current_fee".to_owned(),
                                        args: vec![Expr::Ident("X".to_owned())],
                                    }]
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
}
