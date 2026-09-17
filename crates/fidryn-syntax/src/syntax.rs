//! Rowan CST kinds and language glue.
//!
//! Token kinds match [`crate::lexer::TokenKind`]. Composite nodes are
//! `MODULE`, `ITEM`, `DECL`, `HEADER`, `EXPR`, `WHITESPACE`, `COMMENT`,
//! and `ERROR`. Trivia is kept as leaves so `syntax().text()` equals source.

use crate::lexer::TokenKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(non_camel_case_types)]
#[repr(u16)]
pub enum SyntaxKind {
    Ident = 0,
    String,
    Int,
    Decimal,
    Date,
    DateTime,
    DurationUnit,
    PlusInf,
    MinusInf,
    Comment,
    DocComment,
    Whitespace,
    Newline,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Bang,
    Arrow,
    FatArrow,
    Eq,
    EqEq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Plus,
    Minus,
    Star,
    Slash,
    Pipe,
    Range,
    Eof,
    Error,
    MODULE,
    ITEM,
    DECL,
    HEADER,
    EXPR,
    WHITESPACE,
    COMMENT,
    ERROR,
}

const KINDS: &[SyntaxKind] = &[
    SyntaxKind::Ident,
    SyntaxKind::String,
    SyntaxKind::Int,
    SyntaxKind::Decimal,
    SyntaxKind::Date,
    SyntaxKind::DateTime,
    SyntaxKind::DurationUnit,
    SyntaxKind::PlusInf,
    SyntaxKind::MinusInf,
    SyntaxKind::Comment,
    SyntaxKind::DocComment,
    SyntaxKind::Whitespace,
    SyntaxKind::Newline,
    SyntaxKind::LBrace,
    SyntaxKind::RBrace,
    SyntaxKind::LParen,
    SyntaxKind::RParen,
    SyntaxKind::LBracket,
    SyntaxKind::RBracket,
    SyntaxKind::Comma,
    SyntaxKind::Dot,
    SyntaxKind::Colon,
    SyntaxKind::Semicolon,
    SyntaxKind::Bang,
    SyntaxKind::Arrow,
    SyntaxKind::FatArrow,
    SyntaxKind::Eq,
    SyntaxKind::EqEq,
    SyntaxKind::Ne,
    SyntaxKind::Lt,
    SyntaxKind::Le,
    SyntaxKind::Gt,
    SyntaxKind::Ge,
    SyntaxKind::Plus,
    SyntaxKind::Minus,
    SyntaxKind::Star,
    SyntaxKind::Slash,
    SyntaxKind::Pipe,
    SyntaxKind::Range,
    SyntaxKind::Eof,
    SyntaxKind::Error,
    SyntaxKind::MODULE,
    SyntaxKind::ITEM,
    SyntaxKind::DECL,
    SyntaxKind::HEADER,
    SyntaxKind::EXPR,
    SyntaxKind::WHITESPACE,
    SyntaxKind::COMMENT,
    SyntaxKind::ERROR,
];

impl SyntaxKind {
    pub fn from_u16(n: u16) -> Self {
        KINDS.get(n as usize).copied().unwrap_or(SyntaxKind::ERROR)
    }
}

impl From<SyntaxKind> for rowan::SyntaxKind {
    fn from(kind: SyntaxKind) -> Self {
        Self(kind as u16)
    }
}

impl From<TokenKind> for SyntaxKind {
    fn from(kind: TokenKind) -> Self {
        match kind {
            TokenKind::Ident => SyntaxKind::Ident,
            TokenKind::String => SyntaxKind::String,
            TokenKind::Int => SyntaxKind::Int,
            TokenKind::Decimal => SyntaxKind::Decimal,
            TokenKind::Date => SyntaxKind::Date,
            TokenKind::DateTime => SyntaxKind::DateTime,
            TokenKind::DurationUnit => SyntaxKind::DurationUnit,
            TokenKind::PlusInf => SyntaxKind::PlusInf,
            TokenKind::MinusInf => SyntaxKind::MinusInf,
            TokenKind::Comment => SyntaxKind::Comment,
            TokenKind::DocComment => SyntaxKind::DocComment,
            TokenKind::Whitespace => SyntaxKind::Whitespace,
            TokenKind::Newline => SyntaxKind::Newline,
            TokenKind::LBrace => SyntaxKind::LBrace,
            TokenKind::RBrace => SyntaxKind::RBrace,
            TokenKind::LParen => SyntaxKind::LParen,
            TokenKind::RParen => SyntaxKind::RParen,
            TokenKind::LBracket => SyntaxKind::LBracket,
            TokenKind::RBracket => SyntaxKind::RBracket,
            TokenKind::Comma => SyntaxKind::Comma,
            TokenKind::Dot => SyntaxKind::Dot,
            TokenKind::Colon => SyntaxKind::Colon,
            TokenKind::Semicolon => SyntaxKind::Semicolon,
            TokenKind::Bang => SyntaxKind::Bang,
            TokenKind::Arrow => SyntaxKind::Arrow,
            TokenKind::FatArrow => SyntaxKind::FatArrow,
            TokenKind::Eq => SyntaxKind::Eq,
            TokenKind::EqEq => SyntaxKind::EqEq,
            TokenKind::Ne => SyntaxKind::Ne,
            TokenKind::Lt => SyntaxKind::Lt,
            TokenKind::Le => SyntaxKind::Le,
            TokenKind::Gt => SyntaxKind::Gt,
            TokenKind::Ge => SyntaxKind::Ge,
            TokenKind::Plus => SyntaxKind::Plus,
            TokenKind::Minus => SyntaxKind::Minus,
            TokenKind::Star => SyntaxKind::Star,
            TokenKind::Slash => SyntaxKind::Slash,
            TokenKind::Pipe => SyntaxKind::Pipe,
            TokenKind::Range => SyntaxKind::Range,
            TokenKind::Eof => SyntaxKind::Eof,
            TokenKind::Error => SyntaxKind::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FidrynLanguage {}

impl rowan::Language for FidrynLanguage {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> Self::Kind {
        SyntaxKind::from_u16(raw.0)
    }

    fn kind_to_raw(kind: Self::Kind) -> rowan::SyntaxKind {
        kind.into()
    }
}

pub type SyntaxNode = rowan::SyntaxNode<FidrynLanguage>;
pub type SyntaxToken = rowan::SyntaxToken<FidrynLanguage>;
pub type SyntaxElement = rowan::SyntaxElement<FidrynLanguage>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::TokenKind;
    use rowan::Language;

    #[test]
    fn token_kinds_map_onto_syntax_kinds() {
        let tokens = [
            TokenKind::Ident,
            TokenKind::String,
            TokenKind::Int,
            TokenKind::Decimal,
            TokenKind::Date,
            TokenKind::DateTime,
            TokenKind::DurationUnit,
            TokenKind::PlusInf,
            TokenKind::MinusInf,
            TokenKind::Comment,
            TokenKind::DocComment,
            TokenKind::Whitespace,
            TokenKind::Newline,
            TokenKind::LBrace,
            TokenKind::RBrace,
            TokenKind::LParen,
            TokenKind::RParen,
            TokenKind::LBracket,
            TokenKind::RBracket,
            TokenKind::Comma,
            TokenKind::Dot,
            TokenKind::Colon,
            TokenKind::Semicolon,
            TokenKind::Bang,
            TokenKind::Arrow,
            TokenKind::FatArrow,
            TokenKind::Eq,
            TokenKind::EqEq,
            TokenKind::Ne,
            TokenKind::Lt,
            TokenKind::Le,
            TokenKind::Gt,
            TokenKind::Ge,
            TokenKind::Plus,
            TokenKind::Minus,
            TokenKind::Star,
            TokenKind::Slash,
            TokenKind::Pipe,
            TokenKind::Range,
            TokenKind::Eof,
            TokenKind::Error,
        ];
        for kind in tokens {
            assert_eq!(format!("{:?}", SyntaxKind::from(kind)), format!("{kind:?}"));
        }
    }

    #[test]
    fn language_round_trips_composite_kinds() {
        for kind in [
            SyntaxKind::MODULE,
            SyntaxKind::ITEM,
            SyntaxKind::DECL,
            SyntaxKind::HEADER,
            SyntaxKind::EXPR,
            SyntaxKind::WHITESPACE,
            SyntaxKind::COMMENT,
            SyntaxKind::ERROR,
        ] {
            let raw = FidrynLanguage::kind_to_raw(kind);
            assert_eq!(FidrynLanguage::kind_from_raw(raw), kind);
        }
    }
}
