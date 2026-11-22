//! Lexer, lossless CST, parser, and formatter. See `grammar.ebnf`.

use fidryn_core::Diagnostic;

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod syntax;

pub use lexer::{Token, TokenKind, lex};
pub use parser::{Parse, parse_file};
pub use syntax::{FidrynLanguage, SyntaxKind, SyntaxNode};

/// Decode the JSON string spelling used by source literals.
pub fn decode_string_literal(source: &str) -> Result<String, serde_json::Error> {
    serde_json::from_str(source)
}

pub fn format_module(source: &str) -> Result<String, Diagnostic> {
    parser::format_module(source)
}
