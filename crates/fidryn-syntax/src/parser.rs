//! Recursive-descent parser with automatic semicolon insertion.

use crate::ast::{Decl, Header, HeaderKind, Item, Module};
use crate::lexer::{Token, TokenKind, lex};
use fidryn_core::Span;
use fidryn_core::{Diagnostic, DiagnosticCode};

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

pub struct Parse {
    pub source: String,
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
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
        &self.source[token.start as usize..token.end as usize]
    }
}

pub fn parse_file(source: &str) -> Parse {
    let tokens = lex(source);
    let mut p = Parser {
        source,
        tokens: &tokens,
        idx: 0,
        diagnostics: Vec::new(),
    };
    let module = p.parse_module();
    let diagnostics = p.diagnostics;
    Parse {
        source: source.to_owned(),
        tokens,
        diagnostics,
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
        &self.source[token.start as usize..token.end as usize]
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

    fn parse_module(&mut self) -> Option<Module> {
        if !self.eat_ident("module") {
            let t = self.peek();
            self.error(t, "expected `module`");
            return None;
        }
        let name = self.parse_qname();
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
                self.recover_declaration();
            }
        }
        let end = self.expect_kind(TokenKind::RBrace, "expected `}`");
        Some(Module {
            span: Span {
                start: 0,
                end: end.end,
            },
            name,
            version,
            items,
        })
    }

    fn parse_qname(&mut self) -> String {
        let mut name = String::new();
        loop {
            let t = self.expect_kind(TokenKind::Ident, "expected identifier");
            if !name.is_empty() {
                name.push('.');
            }
            name.push_str(self.text(t));
            if self.peek().kind == TokenKind::Dot {
                self.bump();
            } else {
                break;
            }
        }
        name
    }

    fn parse_item(&mut self) -> Option<Item> {
        let t = self.peek();
        if t.kind != TokenKind::Ident {
            self.error(t, "expected declaration");
            return None;
        }
        let kw = self.text(t);
        match kw {
            "jurisdiction" => Some(Item::Header(self.parse_header(HeaderKind::Jurisdiction))),
            "source_snapshot" => Some(Item::Header(self.parse_header(HeaderKind::SourceSnapshot))),
            "source_manifest" => Some(Item::Header(self.parse_header(HeaderKind::SourceManifest))),
            "effective_at" => Some(Item::Header(self.parse_header(HeaderKind::EffectiveAt))),
            "recorded_at" => Some(Item::Header(self.parse_header(HeaderKind::RecordedAt))),
            "outside_scope" => Some(Item::Header(self.parse_header(HeaderKind::OutsideScope))),
            "source" => Some(Item::Source(self.parse_decl("source"))),
            "import" => Some(Item::Import(self.parse_decl("import"))),
            "entity" => Some(Item::Entity(self.parse_decl("entity"))),
            "type" => Some(Item::Type(self.parse_decl("type"))),
            "record_type" | "evidence_type" => {
                let kw = kw.to_owned();
                Some(Item::RecordType(self.parse_decl(&kw)))
            }
            "office" => Some(Item::Office(self.parse_decl("office"))),
            "proposition" => Some(Item::Proposition(self.parse_decl("proposition"))),
            "observation" => Some(Item::Observation(self.parse_decl("observation"))),
            "nomination" => Some(Item::Nomination(self.parse_decl("nomination"))),
            "fn" | "calc" => {
                let kw = kw.to_owned();
                Some(Item::Function(self.parse_decl(&kw)))
            }
            "effect" => Some(Item::Effect(self.parse_decl("effect"))),
            "for_all" | "exists" => {
                let kw = kw.to_owned();
                Some(Item::Verify(self.parse_decl(&kw)))
            }
            "rule" => Some(Item::Rule(self.parse_decl("rule"))),
            "power" => Some(Item::Power(self.parse_decl("power"))),
            "duty" => Some(Item::Duty(self.parse_decl("duty"))),
            "judgment" => Some(Item::Judgment(self.parse_decl("judgment"))),
            "decision" => Some(Item::Decision(self.parse_decl("decision"))),
            "legal_act" => Some(Item::LegalAct(self.parse_decl("legal_act"))),
            "clause" => Some(Item::Clause(self.parse_decl("clause"))),
            "interpretation_family" => Some(Item::Interpretation(
                self.parse_decl("interpretation_family"),
            )),
            "conflict_doctrine" => {
                Some(Item::ConflictDoctrine(self.parse_decl("conflict_doctrine")))
            }
            "query" => Some(Item::Query(self.parse_decl("query"))),
            "scenario" => Some(Item::Scenario(self.parse_decl("scenario"))),
            "verify" => Some(Item::Verify(self.parse_decl("verify"))),
            _ => {
                self.error(t, &format!("unknown declaration `{kw}`"));
                None
            }
        }
    }

    fn parse_header(&mut self, kind: HeaderKind) -> Header {
        let start = self.bump();
        let value_start = self.peek().start;
        self.skip_balanced_until_end();
        let end = self.last_end(start.end);
        Header {
            span: Span {
                start: start.start,
                end,
            },
            kind,
            value: self.source[value_start as usize..end as usize]
                .trim()
                .to_owned(),
        }
    }

    fn parse_decl(&mut self, keyword: &str) -> Decl {
        let start = self.bump();
        if keyword == "query" && self.at_ident("automatic") {
            self.bump();
        }
        let mut name = None;
        let t = self.peek();
        if t.kind == TokenKind::Ident {
            name = Some(self.parse_qname());
        }
        let sig_start = self.peek().start;
        self.skip_balanced_until_end();
        let end = self.last_end(start.end);
        Decl {
            span: Span {
                start: start.start,
                end,
            },
            keyword: keyword.to_owned(),
            name,
            signature: Some(
                self.source[sig_start as usize..end as usize]
                    .trim()
                    .to_owned(),
            ),
            source: self.source[start.start as usize..end as usize].to_owned(),
        }
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
                        // Effect rows (`! {Observe}`) and variant types (`| Next`)
                        // continue the same declaration after a closed brace group.
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
                        // Half-open interval: [start, +inf)
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
        // Consume the unknown introducer so the next declaration keyword is
        // left for parse_item.
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
        let formatted = format_module(&src).unwrap_or_else(|d| panic!("{rel} format: {d}"));
        let again = format_module(&formatted).unwrap_or_else(|d| panic!("{rel} reformat: {d}"));
        assert_eq!(formatted, again, "{rel}: format should be stable");
    }

    #[test]
    fn parses_example_modules_without_e100() {
        parse_example_or_skip("../../examples/trust/bryan-revocable-trust.fidryn");
        parse_example_or_skip("../../examples/prenup/ava-noah.fidryn");
        parse_example_or_skip("../../examples/foia/foia-request.fidryn");
        parse_example_or_skip("../../examples/massachusetts-llc/harbor-robotics.fidryn");
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
}
