//! The edition marker: which parser reads a file.
//!
//! The first two significant tokens of an edition-2 file are the word
//! `edition` and the integer `2`; white space, line ends and comments before
//! them are skipped. A file whose first significant token is anything else is
//! a file of the first edition and carries no marker.

use crate::lexer::{Token, TokenKind, lex};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edition {
    One,
    Two,
}

/// The edition of `source`, read from its first two significant tokens.
///
/// `edition 2` is [`Edition::Two`]; the word `edition` followed by anything
/// else is `E100` at the token after it, or at the word itself when the file
/// ends there; any other file, the empty one included, is [`Edition::One`].
/// Only the white space and comments before the marker and the two tokens of
/// the marker are read, so the cost does not grow with the rest of the file.
pub fn edition_of(source: &str) -> Result<Edition, Diagnostic> {
    let Some(word) = token_at(source, skip_trivia(source, 0)) else {
        return Ok(Edition::One);
    };
    if word.kind != TokenKind::Ident || text(source, word) != "edition" {
        return Ok(Edition::One);
    }
    match token_at(source, skip_trivia(source, word.end as usize)) {
        None => Err(unknown_edition(None, span(word))),
        Some(next) if next.kind == TokenKind::Int && text(source, next) == "2" => Ok(Edition::Two),
        Some(next) => Err(unknown_edition(Some(text(source, next)), span(next))),
    }
}

/// The `E100` for a marker that names no edition this toolchain reads.
/// `next` is the text of the token after the word `edition`, `None` when the
/// file ends there.
pub(crate) fn unknown_edition(next: Option<&str>, span: Span) -> Diagnostic {
    let found = match next {
        Some(next) => format!("unknown edition `{next}`"),
        None => String::from("unknown edition at the end of the file"),
    };
    Diagnostic::new(
        DiagnosticCode::E100,
        format!(
            "{found}; this toolchain reads edition 2 after the word `edition`, \
             and files of the first edition carry no marker"
        ),
    )
    .with_span(span)
}

/// The first byte at or after `start` that begins no white space, line end or
/// comment. The classes are those of the lexer: a space, a tab or a carriage
/// return is white space, `\n` is a line end, and `//` begins a comment that
/// runs to the line end.
fn skip_trivia(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut pos = start;
    loop {
        match bytes.get(pos) {
            Some(b' ' | b'\t' | b'\r' | b'\n') => pos += 1,
            Some(b'/') if bytes.get(pos + 1) == Some(&b'/') => {
                while !matches!(bytes.get(pos), None | Some(b'\n')) {
                    pos += 1;
                }
            }
            _ => return pos,
        }
    }
}

/// The lexer reads at most this many bytes past the end of a token to decide
/// where the token ends (a date followed by a time and an offset).
const LOOKAHEAD: usize = 32;

/// The token the lexer reads at `start`, or `None` at the end of the source.
///
/// The lexer reads a token from the bytes at and after its start alone, so
/// lexing a window that holds the whole token and the bytes the lexer looks
/// at past it gives the token of the whole source. The window doubles until
/// it does, so a long token costs time and memory in proportion to its own
/// length only.
fn token_at(source: &str, start: usize) -> Option<Token> {
    if start >= source.len() {
        return None;
    }
    let mut width = 2 * LOOKAHEAD;
    loop {
        let mut end = source.len().min(start.saturating_add(width));
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        let first = lex(&source[start..end]).into_iter().next()?;
        if end == source.len() || first.end as usize + LOOKAHEAD <= end - start {
            return Some(Token {
                kind: first.kind,
                start: first.start + start as u32,
                end: first.end + start as u32,
            });
        }
        width = width.saturating_mul(2);
    }
}

fn text(source: &str, token: Token) -> &str {
    source
        .get(token.start as usize..token.end as usize)
        .unwrap_or("")
}

fn span(token: Token) -> Span {
    Span {
        start: token.start,
        end: token.end,
    }
}

#[cfg(test)]
mod tests {
    use super::{Edition, edition_of};
    use fidryn_core::{DiagnosticCode, Span};

    fn span_of(source: &str, needle: &str) -> Span {
        let start = source.rfind(needle).expect("needle") as u32;
        Span {
            start,
            end: start + needle.len() as u32,
        }
    }

    #[test]
    fn edition_of_follows_the_table() {
        for source in [
            "edition 2\nmodule \"/zz/demo/a\" {\n}\n",
            "edition 2",
            "// An invented module.\n\n\n   edition 2\nmodule \"/zz/demo/a\" {\n}\n",
            "/// A comment of the other kind.\r\n\tedition // between\n 2 module",
        ] {
            assert_eq!(edition_of(source), Ok(Edition::Two), "{source:?}");
        }
        for source in [
            "module Examples.Trust version \"0.1.0\" { }",
            "",
            "// A file of comments only.\n// edition 2\n\n",
            "\n \t\r\n",
            "editions 2",
            "edition_2",
            "\u{feff}edition 2",
            "\"edition\" 2",
        ] {
            assert_eq!(edition_of(source), Ok(Edition::One), "{source:?}");
        }
        for (source, next) in [
            ("edition 3\nmodule \"/zz/demo/a\" {\n}\n", "3"),
            ("edition \"2\"\nmodule \"/zz/demo/a\" {\n}\n", "\"2\""),
            ("edition 2.0\nmodule \"/zz/demo/a\" {\n}\n", "2.0"),
            ("edition 02", "02"),
            ("edition\n// a comment\nmodule", "module"),
            ("edition 2026-01-01T00:00:00Z", "2026-01-01T00:00:00Z"),
            ("edition \u{e9}", "\u{e9}"),
        ] {
            let error = edition_of(source).expect_err(source);
            assert_eq!(error.code, DiagnosticCode::E100, "{source:?}");
            assert!(
                error.message.starts_with("unknown edition"),
                "{source:?}: {}",
                error.message
            );
            assert_eq!(
                error.primary_span,
                Some(span_of(source, next)),
                "{source:?}"
            );
        }
        let error = edition_of("edition 3").expect_err("edition 3");
        assert_eq!(
            error.message,
            "unknown edition `3`; this toolchain reads edition 2 after the word `edition`, \
             and files of the first edition carry no marker"
        );
        for source in ["edition", "// comment\nedition  \n// comment"] {
            let error = edition_of(source).expect_err(source);
            assert_eq!(error.code, DiagnosticCode::E100, "{source:?}");
            assert!(
                error.message.starts_with("unknown edition"),
                "{source:?}: {}",
                error.message
            );
            assert_eq!(
                error.primary_span,
                Some(span_of(source, "edition")),
                "{source:?}"
            );
        }
        // A long token after the word is read whole, as the lexer reads it.
        let long = format!("\"{}\"", "x".repeat(10_000));
        let source = format!("edition {long} module");
        let error = edition_of(&source).expect_err("long string");
        assert_eq!(error.primary_span, Some(span_of(&source, &long)));
    }
}
