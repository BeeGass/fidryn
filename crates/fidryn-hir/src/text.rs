//! Two views of source text for the readers that walk it by character.
//!
//! The parser hands over the text of a declaration as it is written, with
//! its comments. A reader that searches that text for a word, counts its
//! brackets or splits it on a separator must find none of these in a
//! comment, and none inside a string literal.

use fidryn_syntax::lexer::{TokenKind, lex};
use std::borrow::Cow;

/// `text` with every comment replaced by spaces, one for each byte of the
/// comment.
///
/// Nothing else changes and the length stays the same, so a position in the
/// result is the same position in `text`. A reader that takes this text
/// reads a comment as the white space it stands for. The text is lexed as
/// the parser lexes it, so `//` inside a string literal begins no comment.
pub fn without_comments(text: &str) -> Cow<'_, str> {
    if !text.contains("//") {
        return Cow::Borrowed(text);
    }
    Cow::Owned(blanked(text, false))
}

/// `text` with every comment replaced by spaces, as `without_comments`
/// does, and with what stands between the quotes of each string literal
/// replaced by spaces as well.
///
/// The quotes stay, and so does a line end. What is left is every character
/// that can be structure: a reader that counts brackets, splits on a
/// separator or searches for a word walks this text, and cuts what it finds
/// out of the text it was given, at the same positions. A string literal
/// that is not closed is blank from its quote to its end.
pub fn structure_of(text: &str) -> Cow<'_, str> {
    if !text.contains("//") && !text.contains('"') {
        return Cow::Borrowed(text);
    }
    Cow::Owned(blanked(text, true))
}

/// `text` with its comments blank, and with the inside of its string
/// literals blank as well when `strings_too` is set.
fn blanked(text: &str, strings_too: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for token in lex(text) {
        let piece = &text[token.start as usize..token.end as usize];
        match token.kind {
            TokenKind::Comment | TokenKind::DocComment => blank(piece, &mut out),
            TokenKind::String if strings_too => {
                // The token begins with its quote. It ends with one unless
                // the text ended first.
                let inside = if string_is_closed(piece) {
                    1..piece.len() - 1
                } else {
                    1..piece.len()
                };
                out.push('"');
                blank(&piece[inside.clone()], &mut out);
                out.push_str(&piece[inside.end..]);
            }
            _ => out.push_str(piece),
        }
    }
    out
}

/// One space for each byte of `piece`. A line end stays a line end.
fn blank(piece: &str, out: &mut String) {
    out.extend(
        piece
            .bytes()
            .map(|byte| if byte == b'\n' { '\n' } else { ' ' }),
    );
}

/// True when the string token `piece` ends with the quote that closes it,
/// and not at the end of the text or with a quote behind a backslash.
fn string_is_closed(piece: &str) -> bool {
    let bytes = piece.as_bytes();
    let mut at = 1;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => at += 2,
            b'"' => return true,
            _ => at += 1,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_comment_is_replaced_by_as_many_spaces_and_nothing_else_moves() {
        let text = "a // b { \"\nc \"// not a comment\" /// d\n// e";
        let blank = without_comments(text);
        assert_eq!(
            blank,
            format!(
                "a {}\nc \"// not a comment\" {}\n{}",
                " ".repeat(8),
                " ".repeat(5),
                " ".repeat(4)
            )
        );
        assert_eq!(blank.len(), text.len());
        // A character of more than one byte becomes one space for each
        // byte, so that every position after it stays where it was.
        let text = "x // § 12\ny";
        let blank = without_comments(text);
        assert_eq!(blank, format!("x {}\ny", " ".repeat(8)));
        assert_eq!(blank.find('y'), text.find('y'));
        // A text with no comment is handed back as it is.
        for text in ["", "a / b", "a \"// b\" / c"] {
            assert_eq!(without_comments(text), text);
        }
    }

    #[test]
    fn the_structure_of_a_text_holds_no_comment_and_nothing_from_inside_a_string() {
        let text = "f(\"a) } , \\\" b\", c) // d )\n\"é\" e";
        let structure = structure_of(text);
        assert_eq!(
            structure,
            format!(
                "f(\"{}\", c) {}\n\"{}\" e",
                " ".repeat(11),
                " ".repeat(6),
                " ".repeat(2)
            )
        );
        assert_eq!(structure.len(), text.len());
        // A string that is not closed is blank to the end of the text,
        // also when the last thing in it is a quote behind a backslash.
        assert_eq!(structure_of("a \"b ) c"), format!("a \"{}", " ".repeat(5)));
        assert_eq!(structure_of("\"a\\\""), format!("\"{}", " ".repeat(3)));
        // A line end inside a string stays a line end.
        assert_eq!(structure_of("\"a\nb\" c"), "\" \n \" c");
        // A text with no comment and no string is handed back as it is.
        for text in ["", "f(a, b) / c"] {
            assert_eq!(structure_of(text), text);
        }
    }
}
