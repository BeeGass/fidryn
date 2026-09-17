//! Tokenization with trivia. ASI is applied in the parser, not here.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Ident,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub start: u32,
    pub end: u32,
}

pub fn lex(source: &str) -> Vec<Token> {
    Lexer::new(source).collect()
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    done: bool,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
            done: false,
        }
    }

    fn bump(&mut self) -> Option<u8> {
        let b = *self.bytes.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn starts_with(&self, s: &str) -> bool {
        self.src[self.pos..].starts_with(s)
    }

    fn emit(&self, kind: TokenKind, start: usize) -> Token {
        Token {
            kind,
            start: start as u32,
            end: self.pos as u32,
        }
    }

    fn starts_with_inf_token(&self) -> bool {
        if !self.starts_with("inf") {
            return false;
        }
        !self
            .bytes
            .get(self.pos + 3)
            .is_some_and(|b| is_ident_continue(*b))
    }
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

impl Iterator for Lexer<'_> {
    type Item = Token;

    fn next(&mut self) -> Option<Token> {
        if self.done {
            return None;
        }
        if self.pos >= self.bytes.len() {
            self.done = true;
            return Some(Token {
                kind: TokenKind::Eof,
                start: self.pos as u32,
                end: self.pos as u32,
            });
        }
        let start = self.pos;
        let c = self.bump()?;
        let tok = match c {
            b' ' | b'\t' | b'\r' => {
                while matches!(self.peek(), Some(b' ' | b'\t' | b'\r')) {
                    self.bump();
                }
                self.emit(TokenKind::Whitespace, start)
            }
            b'\n' => self.emit(TokenKind::Newline, start),
            b'{' => self.emit(TokenKind::LBrace, start),
            b'}' => self.emit(TokenKind::RBrace, start),
            b'(' => self.emit(TokenKind::LParen, start),
            b')' => self.emit(TokenKind::RParen, start),
            b'[' => self.emit(TokenKind::LBracket, start),
            b']' => self.emit(TokenKind::RBracket, start),
            b',' => self.emit(TokenKind::Comma, start),
            b'.' => {
                if self.peek() == Some(b'.') {
                    self.bump();
                    self.emit(TokenKind::Range, start)
                } else {
                    self.emit(TokenKind::Dot, start)
                }
            }
            b':' => self.emit(TokenKind::Colon, start),
            b';' => self.emit(TokenKind::Semicolon, start),
            b'!' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    self.emit(TokenKind::Ne, start)
                } else {
                    self.emit(TokenKind::Bang, start)
                }
            }
            b'=' => match self.peek() {
                Some(b'=') => {
                    self.bump();
                    self.emit(TokenKind::EqEq, start)
                }
                Some(b'>') => {
                    self.bump();
                    self.emit(TokenKind::FatArrow, start)
                }
                _ => self.emit(TokenKind::Eq, start),
            },
            b'<' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    self.emit(TokenKind::Le, start)
                } else {
                    self.emit(TokenKind::Lt, start)
                }
            }
            b'>' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    self.emit(TokenKind::Ge, start)
                } else {
                    self.emit(TokenKind::Gt, start)
                }
            }
            b'+' => {
                if self.starts_with_inf_token() {
                    self.pos += 3;
                    self.emit(TokenKind::PlusInf, start)
                } else {
                    self.emit(TokenKind::Plus, start)
                }
            }
            b'-' => {
                if self.peek() == Some(b'>') {
                    self.bump();
                    self.emit(TokenKind::Arrow, start)
                } else if self.starts_with_inf_token() {
                    self.pos += 3;
                    self.emit(TokenKind::MinusInf, start)
                } else {
                    self.emit(TokenKind::Minus, start)
                }
            }
            b'*' => self.emit(TokenKind::Star, start),
            b'/' => {
                if self.peek() == Some(b'/') {
                    self.bump();
                    let doc = self.peek() == Some(b'/');
                    while !matches!(self.peek(), None | Some(b'\n')) {
                        self.bump();
                    }
                    self.emit(
                        if doc {
                            TokenKind::DocComment
                        } else {
                            TokenKind::Comment
                        },
                        start,
                    )
                } else {
                    self.emit(TokenKind::Slash, start)
                }
            }
            b'|' => self.emit(TokenKind::Pipe, start),
            b'"' => {
                while let Some(ch) = self.bump() {
                    if ch == b'\\' {
                        self.bump();
                    } else if ch == b'"' {
                        break;
                    }
                }
                self.emit(TokenKind::String, start)
            }
            b'0'..=b'9' => self.lex_number(start),
            b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                while matches!(
                    self.peek(),
                    Some(b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
                ) {
                    self.bump();
                }
                let text = &self.src[start..self.pos];
                let kind = match text {
                    "days" | "working_days" | "counted_days" | "calendar_days" | "hours"
                    | "minutes" | "seconds" => TokenKind::DurationUnit,
                    _ => TokenKind::Ident,
                };
                self.emit(kind, start)
            }
            _ => self.emit(TokenKind::Error, start),
        };
        Some(tok)
    }
}

impl Lexer<'_> {
    fn lex_number(&mut self, start: usize) -> Token {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.bump();
        }
        if self.peek() == Some(b'-')
            && self.pos + 1 < self.bytes.len()
            && self.bytes[self.pos + 1].is_ascii_digit()
        {
            let rest = &self.src[start..];
            let n = datetime_len(rest);
            if n > 0 {
                self.pos = start + n;
                let kind = if rest[..n].contains('T') {
                    TokenKind::DateTime
                } else {
                    TokenKind::Date
                };
                return self.emit(kind, start);
            }
        }
        if self.peek() == Some(b'.')
            && self.pos + 1 < self.bytes.len()
            && self.bytes[self.pos + 1].is_ascii_digit()
        {
            self.bump();
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.bump();
            }
            return self.emit(TokenKind::Decimal, start);
        }
        self.emit(TokenKind::Int, start)
    }
}

fn is_digits(bytes: &[u8], start: usize, n: usize) -> bool {
    bytes.len() >= start + n && bytes[start..start + n].iter().all(u8::is_ascii_digit)
}

fn datetime_len(s: &str) -> usize {
    let bytes = s.as_bytes();
    if bytes.len() < 10 {
        return 0;
    }
    if !(is_digits(bytes, 0, 4)
        && bytes[4] == b'-'
        && is_digits(bytes, 5, 2)
        && bytes[7] == b'-'
        && is_digits(bytes, 8, 2))
    {
        return 0;
    }
    // DateTime ::= Date "T" TimeOfDay Offset?
    // TimeOfDay is 8 bytes (HH:MM:SS), so the instant without offset is 19 bytes.
    if bytes.len() >= 19
        && bytes[10] == b'T'
        && is_digits(bytes, 11, 2)
        && bytes[13] == b':'
        && is_digits(bytes, 14, 2)
        && bytes[16] == b':'
        && is_digits(bytes, 17, 2)
    {
        let mut i = 19;
        if i < bytes.len() && bytes[i] == b'Z' {
            i += 1;
        } else if bytes.len() >= i + 6
            && (bytes[i] == b'+' || bytes[i] == b'-')
            && is_digits(bytes, i + 1, 2)
            && bytes[i + 3] == b':'
            && is_digits(bytes, i + 4, 2)
        {
            i += 6;
        }
        return i;
    }
    10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_module_header() {
        let tokens: Vec<_> = lex("module Examples.Trust version \"0.1.0\" { }")
            .into_iter()
            .filter(|t| !matches!(t.kind, TokenKind::Whitespace | TokenKind::Eof))
            .map(|t| t.kind)
            .collect();
        assert_eq!(
            tokens,
            vec![
                TokenKind::Ident,
                TokenKind::Ident,
                TokenKind::Dot,
                TokenKind::Ident,
                TokenKind::Ident,
                TokenKind::String,
                TokenKind::LBrace,
                TokenKind::RBrace,
            ]
        );
    }

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src)
            .into_iter()
            .filter(|t| {
                !matches!(
                    t.kind,
                    TokenKind::Whitespace | TokenKind::Newline | TokenKind::Eof
                )
            })
            .map(|t| t.kind)
            .collect()
    }

    fn text_of(src: &str, token: Token) -> &str {
        &src[token.start as usize..token.end as usize]
    }

    #[test]
    fn lexes_datetime_with_numeric_offsets_and_z() {
        for src in [
            "2026-08-23T12:00:00-04:00",
            "2026-08-23T12:00:00+00:00",
            "2026-08-23T12:00:00Z",
            "2026-08-23T12:00:00",
        ] {
            let tokens = lex(src);
            assert_eq!(tokens[0].kind, TokenKind::DateTime, "{src}");
            assert_eq!(text_of(src, tokens[0]), src, "{src}");
            assert_eq!(tokens[1].kind, TokenKind::Eof, "{src}");
        }
    }

    #[test]
    fn lexes_date() {
        let src = "2026-08-23";
        let tokens = lex(src);
        assert_eq!(tokens[0].kind, TokenKind::Date);
        assert_eq!(text_of(src, tokens[0]), src);
    }

    #[test]
    fn lexes_inf_duration_range_bang_qname_and_comments() {
        assert_eq!(
            kinds("+inf -inf"),
            vec![TokenKind::PlusInf, TokenKind::MinusInf]
        );
        assert_eq!(
            kinds("+information"),
            vec![TokenKind::Plus, TokenKind::Ident]
        );
        assert_eq!(
            kinds("90 days 2 working_days"),
            vec![
                TokenKind::Int,
                TokenKind::DurationUnit,
                TokenKind::Int,
                TokenKind::DurationUnit,
            ]
        );
        assert_eq!(
            kinds("0..1"),
            vec![TokenKind::Int, TokenKind::Range, TokenKind::Int]
        );
        assert_eq!(
            kinds("! {Observe, Determine}"),
            vec![
                TokenKind::Bang,
                TokenKind::LBrace,
                TokenKind::Ident,
                TokenKind::Comma,
                TokenKind::Ident,
                TokenKind::RBrace,
            ]
        );
        assert_eq!(
            kinds("Examples.Trust.Fixture"),
            vec![
                TokenKind::Ident,
                TokenKind::Dot,
                TokenKind::Ident,
                TokenKind::Dot,
                TokenKind::Ident,
            ]
        );
        assert_eq!(kinds("// line"), vec![TokenKind::Comment]);
        assert_eq!(kinds("/// docs"), vec![TokenKind::DocComment]);
    }

    #[test]
    fn tokens_are_lossless() {
        let src = "module A.B version \"0.1.0\" { // c\n  n: 0..1 // r\n  t: 2026-08-23T12:00:00-04:00\n  d: 90 days\n  e: [x, +inf)\n  ! {Observe}\n}\n";
        let tokens = lex(src);
        let mut pos = 0u32;
        let mut reconstructed = String::new();
        for t in &tokens {
            if t.kind == TokenKind::Eof {
                assert_eq!(t.start as usize, src.len());
                continue;
            }
            assert_eq!(t.start, pos, "gap or overlap at {}", t.start);
            reconstructed.push_str(text_of(src, *t));
            pos = t.end;
        }
        assert_eq!(reconstructed, src);
    }
}
