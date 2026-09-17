//! Lexer, lossless CST, parser, and formatter. See `grammar.ebnf`.

use fidryn_core::Diagnostic;

pub mod ast;
pub mod lexer;
pub mod parser;

pub use lexer::{Token, TokenKind, lex};
pub use parser::{Parse, parse_file};

pub fn format_module(source: &str) -> Result<String, Diagnostic> {
    parser::format_module(source)
}
