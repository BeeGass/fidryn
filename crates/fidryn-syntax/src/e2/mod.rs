//! Edition 2: modules that hold provisions of law as data.
//!
//! A file whose first two tokens are `edition 2` (see [`crate::edition`]) is
//! read by [`parse`] into the typed syntax of [`ast`]. The tokens are those of
//! [`crate::lexer`]; words such as `module` and `content` are identifier
//! tokens compared by their text, and line ends are not significant. The
//! parser enforces the grammar: the closed set of fields of each block, their
//! order, the required ones, and the limits below. It stops at the first
//! error.

use fidryn_core::Diagnostic;

pub mod ast;
mod parser;
#[cfg(test)]
mod tests;

pub use parser::parse;

/// The largest source the parser reads, in bytes: 64 MiB.
pub const E2_MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;

/// The most tokens the parser reads, white space, line ends and comments
/// included.
pub const E2_MAX_TOKENS: usize = 16_777_216;

/// The deepest nesting of blocks the parser reads. The braces of the module
/// itself are not counted: a provision at the top of a module stands at depth
/// one.
pub const MAX_NESTING: usize = 64;

/// The result of [`parse`]. The parser stops at the first error, so
/// `diagnostics` holds none or one, and `module` is `Some` exactly when it
/// holds none.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parse {
    pub module: Option<ast::Module>,
    pub diagnostics: Vec<Diagnostic>,
}
