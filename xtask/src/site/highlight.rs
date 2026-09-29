//! Build-time highlighting for code frames: `.fr` through the real lexer,
//! JSON, and shell. Output is escaped HTML with `tk-*` spans; no span
//! crosses a line break, so stripping the spans and unescaping gives the
//! input back exactly.

use super::html::esc;
use anyhow::{Context, Result};
use fidryn_syntax::{TokenKind, lex};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// The language of a code frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Fr,
    Json,
    Shell,
    Plain,
}

impl Lang {
    /// Caption shown in the frame header when no other caption is given.
    pub fn label(self) -> &'static str {
        match self {
            Lang::Fr => ".fr",
            Lang::Json => "json",
            Lang::Shell => "shell",
            Lang::Plain => "text",
        }
    }

    /// Value of the frame's `data-lang` attribute.
    pub fn class(self) -> &'static str {
        match self {
            Lang::Fr => "fr",
            Lang::Json => "json",
            Lang::Shell => "shell",
            Lang::Plain => "text",
        }
    }
}

/// `.fr` keywords: every quoted terminal in `grammar.ebnf` made only of
/// `[a-z_]` and longer than one character. `true` and `false` are not quoted
/// in the grammar; they highlight as literals.
#[derive(Clone, Debug)]
pub struct Keywords(BTreeSet<String>);

impl Keywords {
    pub fn from_grammar(grammar: &str) -> Self {
        let mut words = BTreeSet::new();
        for line in grammar.lines() {
            let mut rest = line;
            while let Some(open) = rest.find('"') {
                let after = &rest[open + 1..];
                let Some(close) = after.find('"') else {
                    break;
                };
                let word = &after[..close];
                if word.len() > 1 && word.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
                    words.insert(word.to_owned());
                }
                rest = &after[close + 1..];
            }
        }
        Self(words)
    }

    /// The keywords of `root/grammar.ebnf`.
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join("grammar.ebnf");
        let grammar =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        Ok(Self::from_grammar(&grammar))
    }

    pub fn contains(&self, word: &str) -> bool {
        self.0.contains(word)
    }

    /// Every keyword, sorted. The site itself only asks `contains`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(String::as_str)
    }
}

/// The language of a code block. An explicit info string wins; an unlabeled
/// block is JSON when it opens with `{` or `[`, shell when its first word is
/// a known command or a `$` prompt, `.fr` when its first word is a keyword,
/// and plain text otherwise.
pub fn sniff(info: &str, code: &str, kw: &Keywords) -> Lang {
    let tag = info
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    match tag.as_str() {
        "fr" | "fidryn" => return Lang::Fr,
        "json" => return Lang::Json,
        "sh" | "bash" | "shell" | "console" | "zsh" => return Lang::Shell,
        "" => {}
        _ => return Lang::Plain,
    }
    let text = code.trim_start();
    if text.starts_with(['{', '[']) {
        return Lang::Json;
    }
    let first = text.split_whitespace().next().unwrap_or("");
    if matches!(
        first,
        "$" | "cargo" | "fidryn" | "cat" | "rustup" | "git" | "curl"
    ) {
        return Lang::Shell;
    }
    let ident = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .map_or(text, |end| &text[..end]);
    if kw.contains(ident) {
        Lang::Fr
    } else {
        Lang::Plain
    }
}

/// `code` as escaped HTML with `<span class="tk-…">` around recognized tokens.
pub fn highlight(lang: Lang, code: &str, kw: &Keywords) -> String {
    lines(lang, code, kw).join("\n")
}

/// A framed code block. The caption defaults to the language label; one
/// final newline is dropped so the frame does not end in a blank line.
pub fn code_frame(lang: Lang, code: &str, kw: &Keywords, caption: Option<&str>) -> String {
    let code = code.strip_suffix('\n').unwrap_or(code);
    format!(
        "<figure class=\"code\" data-lang=\"{}\"><figcaption><span>{}</span>{COPY}</figcaption><pre><code>{}</code></pre></figure>",
        lang.class(),
        esc(caption.unwrap_or(lang.label())),
        highlight(lang, code, kw)
    )
}

/// A framed excerpt with real line numbers. Each segment is (first line
/// number, text); a gap line marks the lines skipped between segments.
pub fn numbered_frame(
    lang: Lang,
    segments: &[(usize, String)],
    kw: &Keywords,
    caption: &str,
) -> String {
    let mut body = String::new();
    let mut next: Option<usize> = None;
    for (first, text) in segments {
        if next.is_some_and(|n| n != *first) {
            body.push_str("<span class=\"line gap\" data-n=\"\">&hellip;</span>\n");
        }
        let text = text.strip_suffix('\n').unwrap_or(text);
        let mut n = *first;
        for line in lines(lang, text, kw) {
            body.push_str(&format!(
                "<span class=\"line\" data-n=\"{n}\">{line}</span>\n"
            ));
            n += 1;
        }
        next = Some(n);
    }
    format!(
        "<figure class=\"code\" data-lang=\"{}\"><figcaption><span>{}</span>{COPY}</figcaption><pre class=\"lines\"><code>{body}</code></pre></figure>",
        lang.class(),
        esc(caption)
    )
}

const COPY: &str = "<button type=\"button\" class=\"copy\" data-copy hidden>Copy</button>";

/// Token classes, rendered as `tk-*` span classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tk {
    Kw,
    Ty,
    St,
    Nu,
    Co,
    Pu,
}

impl Tk {
    fn class(self) -> &'static str {
        match self {
            Tk::Kw => "tk-kw",
            Tk::Ty => "tk-ty",
            Tk::St => "tk-st",
            Tk::Nu => "tk-nu",
            Tk::Co => "tk-co",
            Tk::Pu => "tk-pu",
        }
    }
}

/// A slice of the input and its class; `None` is plain text.
type Piece<'a> = (Option<Tk>, &'a str);

/// Highlighted HTML for each line of `code`.
fn lines(lang: Lang, code: &str, kw: &Keywords) -> Vec<String> {
    let pieces = match lang {
        Lang::Fr => fr_pieces(code, kw),
        Lang::Json => json_pieces(code),
        Lang::Shell => shell_pieces(code),
        Lang::Plain => vec![(None, code)],
    };
    let mut lines = vec![String::new()];
    for (tk, text) in pieces {
        for (i, part) in text.split('\n').enumerate() {
            if i > 0 {
                lines.push(String::new());
            }
            if part.is_empty() {
                continue;
            }
            let line = lines.last_mut().expect("lines starts non-empty");
            match tk {
                Some(tk) => {
                    line.push_str("<span class=\"");
                    line.push_str(tk.class());
                    line.push_str("\">");
                    line.push_str(&esc(part));
                    line.push_str("</span>");
                }
                None => line.push_str(&esc(part)),
            }
        }
    }
    lines
}

/// Where a `module` or `import` name is in `A.B.C`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ModPath {
    Off,
    /// An identifier comes next.
    Name,
    /// A dot may continue the name.
    Dot,
}

/// `.fr` tokens from `fidryn_syntax::lex`. Bytes the lexer skips are kept
/// as plain text.
fn fr_pieces<'a>(code: &'a str, kw: &Keywords) -> Vec<Piece<'a>> {
    let mut pieces = Vec::new();
    let mut pos = 0;
    let mut prev = None;
    let mut path = ModPath::Off;
    for token in lex(code) {
        let (start, end) = (token.start as usize, token.end as usize);
        if start > pos {
            pieces.push((None, &code[pos..start]));
        }
        if token.kind == TokenKind::Eof {
            pos = pos.max(start);
            break;
        }
        let text = &code[start..end];
        let tk = match token.kind {
            TokenKind::Whitespace | TokenKind::Newline => None,
            TokenKind::Comment | TokenKind::DocComment => Some(Tk::Co),
            kind => {
                let tk = fr_class(kind, text, prev, &mut path, kw);
                prev = Some(kind);
                tk
            }
        };
        pieces.push((tk, text));
        pos = end;
    }
    if pos < code.len() {
        pieces.push((None, &code[pos..]));
    }
    pieces
}

/// The class of a token that is not whitespace or a comment.
fn fr_class(
    kind: TokenKind,
    text: &str,
    prev: Option<TokenKind>,
    path: &mut ModPath,
    kw: &Keywords,
) -> Option<Tk> {
    match (*path, kind) {
        (ModPath::Name, TokenKind::Ident) => {
            *path = ModPath::Dot;
            return Some(Tk::Ty);
        }
        (ModPath::Dot, TokenKind::Dot) => {
            *path = ModPath::Name;
            return Some(Tk::Pu);
        }
        _ => *path = ModPath::Off,
    }
    match kind {
        TokenKind::Ident if kw.contains(text) => {
            if matches!(text, "module" | "import") {
                *path = ModPath::Name;
            }
            Some(Tk::Kw)
        }
        TokenKind::Ident if matches!(text, "true" | "false") => Some(Tk::Nu),
        TokenKind::Ident
            if matches!(
                prev,
                Some(TokenKind::Arrow | TokenKind::Colon | TokenKind::Lt)
            ) && text.starts_with(|c: char| c.is_ascii_uppercase()) =>
        {
            Some(Tk::Ty)
        }
        TokenKind::Ident
        | TokenKind::Whitespace
        | TokenKind::Newline
        | TokenKind::Error
        | TokenKind::Eof => None,
        TokenKind::String => Some(Tk::St),
        TokenKind::Int
        | TokenKind::Decimal
        | TokenKind::Date
        | TokenKind::DateTime
        | TokenKind::PlusInf
        | TokenKind::MinusInf => Some(Tk::Nu),
        TokenKind::DurationUnit => Some(Tk::Kw),
        TokenKind::Comment | TokenKind::DocComment => Some(Tk::Co),
        TokenKind::LBrace
        | TokenKind::RBrace
        | TokenKind::LParen
        | TokenKind::RParen
        | TokenKind::LBracket
        | TokenKind::RBracket
        | TokenKind::Comma
        | TokenKind::Dot
        | TokenKind::Colon
        | TokenKind::Semicolon
        | TokenKind::Bang
        | TokenKind::Arrow
        | TokenKind::FatArrow
        | TokenKind::Eq
        | TokenKind::EqEq
        | TokenKind::Ne
        | TokenKind::Lt
        | TokenKind::Le
        | TokenKind::Gt
        | TokenKind::Ge
        | TokenKind::Plus
        | TokenKind::Minus
        | TokenKind::Star
        | TokenKind::Slash
        | TokenKind::Pipe
        | TokenKind::Range => Some(Tk::Pu),
    }
}

/// JSON tokens. Anything that is not JSON stays plain, so a block that only
/// looks like JSON still renders.
fn json_pieces(code: &str) -> Vec<Piece<'_>> {
    let bytes = code.as_bytes();
    let mut pieces = Vec::new();
    let mut plain = 0;
    let mut i = 0;
    while i < bytes.len() {
        let token = match bytes[i] {
            b'"' => {
                let end = quoted_end(bytes, i, true);
                let next = bytes[end..].iter().find(|b| !b.is_ascii_whitespace());
                Some((end, if next == Some(&b':') { Tk::Ty } else { Tk::St }))
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' => Some((i + 1, Tk::Pu)),
            b'-' | b'0'..=b'9' => number_end(bytes, i).map(|end| (end, Tk::Nu)),
            b't' | b'f' | b'n' => literal_end(bytes, i).map(|end| (end, Tk::Nu)),
            _ => None,
        };
        match token {
            Some((end, tk)) => {
                if plain < i {
                    pieces.push((None, &code[plain..i]));
                }
                pieces.push((Some(tk), &code[i..end]));
                i = end;
                plain = end;
            }
            None => i += 1,
        }
    }
    if plain < bytes.len() {
        pieces.push((None, &code[plain..]));
    }
    pieces
}

/// End of the quoted run opening at `start`: just past the closing quote,
/// or at the end of the line when it is unterminated.
fn quoted_end(bytes: &[u8], start: usize, escapes: bool) -> usize {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => return i,
            b'\\' if escapes && bytes.get(i + 1).is_some_and(|b| *b != b'\n') => i += 2,
            b if b == quote => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// End of a JSON number starting at `start`, if one starts there.
fn number_end(bytes: &[u8], start: usize) -> Option<usize> {
    if start > 0 && is_word_byte(bytes[start - 1]) {
        return None;
    }
    let digits = |from: usize| {
        from + bytes[from..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count()
    };
    let mut i = start + usize::from(bytes[start] == b'-');
    let int_end = digits(i);
    if int_end == i {
        return None;
    }
    i = int_end;
    if bytes.get(i) == Some(&b'.') && digits(i + 1) > i + 1 {
        i = digits(i + 1);
    }
    if matches!(bytes.get(i), Some(b'e' | b'E')) {
        let sign = usize::from(matches!(bytes.get(i + 1), Some(b'+' | b'-')));
        let exp_end = digits(i + 1 + sign);
        if exp_end > i + 1 + sign {
            i = exp_end;
        }
    }
    Some(i)
}

/// End of `true`, `false`, or `null` starting at `start`, as a whole word.
fn literal_end(bytes: &[u8], start: usize) -> Option<usize> {
    if start > 0 && is_word_byte(bytes[start - 1]) {
        return None;
    }
    ["true", "false", "null"].iter().find_map(|lit| {
        let end = start + lit.len();
        let whole = bytes[start..].starts_with(lit.as_bytes())
            && !bytes.get(end).copied().is_some_and(is_word_byte);
        whole.then_some(end)
    })
}

/// Shell tokens, line by line. Command words come at the start of a line,
/// after `|`, `&&`, or `;`, and after a leading `$ ` prompt; a line that
/// continues the previous one (trailing `\`) does not start a command.
fn shell_pieces(code: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut heredoc: Option<&str> = None;
    let mut command = true;
    let mut continued = false;
    for (n, line) in code.split('\n').enumerate() {
        if n > 0 {
            pieces.push((None, "\n"));
        }
        if let Some(terminator) = heredoc {
            if line.trim() == terminator {
                heredoc = None;
                pieces.push((Some(Tk::Pu), line));
            } else if !line.is_empty() {
                pieces.push((Some(Tk::St), line));
            }
            continue;
        }
        if !continued {
            command = true;
        }
        let mut shell = ShellLine {
            pieces: &mut pieces,
            command,
            terminator: None,
        };
        continued = shell.scan(line, !continued);
        command = shell.command;
        heredoc = shell.terminator;
    }
    pieces
}

/// Scanner state for one line of shell.
struct ShellLine<'a, 'p> {
    pieces: &'p mut Vec<Piece<'a>>,
    /// The next word is a command word.
    command: bool,
    /// A heredoc announced on this line; its body starts on the next line.
    terminator: Option<&'a str>,
}

impl<'a> ShellLine<'a, '_> {
    /// Scan `line`; true when it ends with a continuation backslash.
    fn scan(&mut self, line: &'a str, fresh: bool) -> bool {
        let (body, continues) = match line.strip_suffix('\\') {
            Some(body) => (body, true),
            None => (line, false),
        };
        let bytes = body.as_bytes();
        let mut i = run(bytes, 0, |b| matches!(b, b' ' | b'\t'));
        if i > 0 {
            self.pieces.push((None, &body[..i]));
        }
        if fresh && body[i..].starts_with("$ ") {
            self.pieces.push((Some(Tk::Pu), &body[i..i + 1]));
            i += 1;
        }
        while i < bytes.len() {
            let (end, tk) = match bytes[i] {
                b if b.is_ascii_whitespace() => (run(bytes, i, |b| b.is_ascii_whitespace()), None),
                b'#' if i == 0 || bytes[i - 1].is_ascii_whitespace() => (bytes.len(), Some(Tk::Co)),
                b'\'' | b'"' => {
                    self.command = false;
                    (quoted_end(bytes, i, bytes[i] == b'"'), Some(Tk::St))
                }
                b'|' | b'&' | b';' => {
                    self.command = true;
                    (
                        run(bytes, i, |b| matches!(b, b'|' | b'&' | b';')),
                        Some(Tk::Pu),
                    )
                }
                b'<' | b'>' => match heredoc_start(body, i) {
                    Some((end, terminator)) => {
                        self.terminator = Some(terminator);
                        (end, Some(Tk::Pu))
                    }
                    None => (run(bytes, i, |b| matches!(b, b'<' | b'>')), Some(Tk::Pu)),
                },
                _ => {
                    let end = run(bytes, i, |b| {
                        !b.is_ascii_whitespace()
                            && !matches!(b, b'\'' | b'"' | b'|' | b'&' | b';' | b'<' | b'>')
                    });
                    (end, self.word(&body[i..end]))
                }
            };
            self.pieces.push((tk, &body[i..end]));
            i = end;
        }
        if continues {
            self.pieces.push((Some(Tk::Pu), &line[line.len() - 1..]));
        }
        continues
    }

    /// A bare word: the command word, a flag, or plain text.
    fn word(&mut self, word: &str) -> Option<Tk> {
        if self.command {
            if is_assignment(word) {
                return None;
            }
            self.command = false;
            Some(Tk::Kw)
        } else if word.starts_with('-') {
            Some(Tk::Ty)
        } else {
            None
        }
    }
}

/// End of the run of bytes from `start` that satisfy `pred`.
fn run(bytes: &[u8], start: usize, pred: impl Fn(u8) -> bool) -> usize {
    start + bytes[start..].iter().take_while(|b| pred(**b)).count()
}

/// `NAME=value`, which may precede a command word.
fn is_assignment(word: &str) -> bool {
    word.split_once('=').is_some_and(|(name, _)| {
        name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && name.bytes().all(is_word_byte)
    })
}

/// `<<EOF`, `<<-EOF`, `<<'EOF'`, or `<<"EOF"` at `i`: its end and terminator.
fn heredoc_start(line: &str, i: usize) -> Option<(usize, &str)> {
    let rest = line[i..].strip_prefix("<<")?;
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    let quote = rest.bytes().next().filter(|b| matches!(b, b'\'' | b'"'));
    let word = &rest[usize::from(quote.is_some())..];
    let len = word.bytes().take_while(|b| is_word_byte(*b)).count();
    if len == 0 {
        return None;
    }
    let mut end = line.len() - word.len() + len;
    if let Some(quote) = quote {
        if word.as_bytes().get(len) != Some(&quote) {
            return None;
        }
        end += 1;
    }
    Some((end, &word[..len]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::workspace_root;

    fn keywords() -> Keywords {
        Keywords::load(&workspace_root()).expect("read grammar.ebnf")
    }

    fn unescape(text: &str) -> String {
        text.replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&")
    }

    /// Every `tk-*` span of `html` as (class, unescaped text), in order.
    fn spans(html: &str) -> Vec<(String, String)> {
        let mut found = Vec::new();
        let mut rest = html;
        while let Some(at) = rest.find("<span class=\"tk-") {
            let open = &rest[at + "<span class=\"".len()..];
            let quote = open.find('"').expect("closing quote");
            let text = &open[quote + "\">".len()..];
            let close = text.find("</span>").expect("closing span");
            found.push((open[..quote].to_owned(), unescape(&text[..close])));
            rest = &text[close + "</span>".len()..];
        }
        found
    }

    /// The class of the first span whose text is exactly `text`.
    fn class_of(html: &str, text: &str) -> Option<String> {
        spans(html)
            .into_iter()
            .find(|(_, t)| t == text)
            .map(|(class, _)| class)
    }

    /// `html` without its `tk-*` spans, unescaped: the highlighted input.
    fn plain(html: &str) -> String {
        let mut out = String::new();
        let mut rest = html;
        while let Some(at) = rest.find("<span class=\"tk-") {
            out.push_str(&rest[..at]);
            let text = &rest[at + rest[at..].find('>').expect("tag end") + 1..];
            let close = text.find("</span>").expect("closing span");
            out.push_str(&text[..close]);
            rest = &text[close + "</span>".len()..];
        }
        out.push_str(rest);
        unescape(&out)
    }

    fn assert_classes(html: &str, expected: &[(&str, &str)]) {
        for (text, class) in expected {
            assert_eq!(
                class_of(html, text).as_deref(),
                Some(*class),
                "{text:?} in {html}"
            );
        }
    }

    const LANGS: [Lang; 4] = [Lang::Fr, Lang::Json, Lang::Shell, Lang::Plain];

    #[test]
    fn keywords_are_the_grammars_lowercase_quoted_terminals() {
        let kw = keywords();
        for word in [
            "module",
            "version",
            "query",
            "require",
            "return",
            "outside_scope",
            "interpretation_family",
            "days",
            "working_days",
            "fn",
            "as",
            "in",
        ] {
            assert!(kw.contains(word), "{word} should be a keyword");
        }
        for word in [
            "true",
            "false",
            "if",
            "_",
            "UniqueOccupant",
            "FiniteSet",
            "Evaluate",
        ] {
            assert!(!kw.contains(word), "{word} should not be a keyword");
        }
        let words: Vec<&str> = kw.iter().collect();
        assert!(
            words.windows(2).all(|pair| pair[0] < pair[1]),
            "sorted and unique"
        );
        assert!(
            words
                .iter()
                .all(|w| w.len() > 1 && w.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'))
        );
    }

    #[test]
    fn from_grammar_reads_closed_quotes_on_each_line() {
        let kw = Keywords::from_grammar(
            "Rule ::= \"rule\" Ident \":\" Kind\nKind ::= \"derive\" | \"x\" | \"Evaluate\" | \"a_b\" | \"..\"\nOpen ::= \"unclosed\n",
        );
        assert_eq!(kw.iter().collect::<Vec<_>>(), ["a_b", "derive", "rule"]);
    }

    #[test]
    fn load_names_the_missing_grammar() {
        let err = Keywords::load(Path::new("/nonexistent-fidryn-root")).unwrap_err();
        assert!(format!("{err:#}").contains("grammar.ebnf"), "{err:#}");
    }

    #[test]
    fn sniff_prefers_an_explicit_info_string() {
        let kw = keywords();
        for (info, lang) in [
            ("fr", Lang::Fr),
            ("fidryn", Lang::Fr),
            ("json", Lang::Json),
            ("JSON", Lang::Json),
            ("sh", Lang::Shell),
            ("bash", Lang::Shell),
            ("shell", Lang::Shell),
            ("console", Lang::Shell),
            ("zsh", Lang::Shell),
            ("rust", Lang::Plain),
            ("text", Lang::Plain),
            ("json title=x", Lang::Json),
        ] {
            assert_eq!(sniff(info, "cargo test", &kw), lang, "{info}");
        }
    }

    #[test]
    fn sniff_recognizes_unlabeled_blocks() {
        let kw = keywords();
        for (code, lang) in [
            ("{\"schema\": \"x\"}", Lang::Json),
            ("  [1, 2]", Lang::Json),
            ("cargo test --workspace --offline", Lang::Shell),
            ("fidryn run x.fr \\\n    --query q", Lang::Shell),
            ("$ fidryn ui", Lang::Shell),
            ("cat > /tmp/case.json <<'EOF'", Lang::Shell),
            ("rustup show", Lang::Shell),
            ("git status", Lang::Shell),
            ("curl -s http://127.0.0.1:8751/api/health", Lang::Shell),
            ("module Programs.RequireGate version \"0.1.0\" {", Lang::Fr),
            ("query q() -> Int { require true; return 7 }", Lang::Fr),
            ("interpretation_family DeadlineMeaning {", Lang::Fr),
            ("fidryn-syntax          lossless CST", Lang::Plain),
            ("syntax → hir → check", Lang::Plain),
            ("if amount <= 11925.00 {", Lang::Plain),
            ("$HOME/bin", Lang::Plain),
            ("", Lang::Plain),
        ] {
            assert_eq!(sniff("", code, &kw), lang, "{code:?}");
        }
    }

    #[test]
    fn fr_token_classes() {
        let kw = keywords();
        let src = "module Programs.RequireGate version \"0.1.0\" {\n  // gate\n  import MA.TrustLaw version \"2026-08-23\"\n  query q() -> Int { require true; return 7 }\n  deadline: Date = 2026-09-17 + 30 days\n  at 2026-09-17T12:00:00Z\n  window: Interval<Instant> = [-inf, +inf)\n  amount: Money<USD> = 1.50\n  holder = Alice\n}\n";
        let html = highlight(Lang::Fr, src, &kw);
        assert_classes(
            &html,
            &[
                ("module", "tk-kw"),
                ("Programs", "tk-ty"),
                (".", "tk-pu"),
                ("RequireGate", "tk-ty"),
                ("version", "tk-kw"),
                ("\"0.1.0\"", "tk-st"),
                ("{", "tk-pu"),
                ("// gate", "tk-co"),
                ("import", "tk-kw"),
                ("MA", "tk-ty"),
                ("TrustLaw", "tk-ty"),
                ("query", "tk-kw"),
                ("->", "tk-pu"),
                ("Int", "tk-ty"),
                ("require", "tk-kw"),
                ("true", "tk-nu"),
                (";", "tk-pu"),
                ("return", "tk-kw"),
                ("7", "tk-nu"),
                ("Date", "tk-ty"),
                ("2026-09-17", "tk-nu"),
                ("30", "tk-nu"),
                ("days", "tk-kw"),
                ("2026-09-17T12:00:00Z", "tk-nu"),
                ("Interval", "tk-ty"),
                ("Instant", "tk-ty"),
                ("-inf", "tk-nu"),
                ("+inf", "tk-nu"),
                ("Money", "tk-ty"),
                ("USD", "tk-ty"),
                ("1.50", "tk-nu"),
            ],
        );
        assert_eq!(class_of(&html, "q"), None, "a query name is plain");
        assert_eq!(class_of(&html, "deadline"), None);
        assert_eq!(
            class_of(&html, "Alice"),
            None,
            "capitalized but not after -> : <"
        );
        assert_eq!(plain(&html), src);
    }

    #[test]
    fn json_token_classes() {
        let kw = keywords();
        let src = "{\n  \"kind\": \"entity\",\n  \"data\" : \"Bob\",\n  \"n\": -1.5e3,\n  \"ok\": true,\n  \"none\": null,\n  \"list\": [1, false]\n}";
        let html = highlight(Lang::Json, src, &kw);
        assert_classes(
            &html,
            &[
                ("{", "tk-pu"),
                ("\"kind\"", "tk-ty"),
                (":", "tk-pu"),
                ("\"entity\"", "tk-st"),
                (",", "tk-pu"),
                ("\"data\"", "tk-ty"),
                ("\"Bob\"", "tk-st"),
                ("-1.5e3", "tk-nu"),
                ("true", "tk-nu"),
                ("null", "tk-nu"),
                ("[", "tk-pu"),
                ("1", "tk-nu"),
                ("false", "tk-nu"),
                ("}", "tk-pu"),
            ],
        );
        assert_eq!(plain(&html), src);
    }

    #[test]
    fn json_leaves_non_json_plain() {
        let kw = keywords();
        let html = highlight(Lang::Json, "{\"diagnostics\": [...], \"v\": nullable}", &kw);
        assert_eq!(class_of(&html, "..."), None);
        assert_eq!(
            class_of(&html, "null"),
            None,
            "`null` inside a word is not a literal"
        );
    }

    #[test]
    fn shell_token_classes() {
        let kw = keywords();
        let src = "$ cargo run -p fidryn-cli -- check x.fr # check it\nFIDRYN_X=1 fidryn file a.json \\\n    --live \"quoted arg\" | cat && git status; curl 'u'\ncat > /tmp/case.json <<'EOF'\n{\"schema\":\"x\"}\nEOF\n";
        let html = highlight(Lang::Shell, src, &kw);
        assert_classes(
            &html,
            &[
                ("$", "tk-pu"),
                ("cargo", "tk-kw"),
                ("-p", "tk-ty"),
                ("--", "tk-ty"),
                ("# check it", "tk-co"),
                ("fidryn", "tk-kw"),
                ("\\", "tk-pu"),
                ("--live", "tk-ty"),
                ("\"quoted arg\"", "tk-st"),
                ("|", "tk-pu"),
                ("cat", "tk-kw"),
                ("&&", "tk-pu"),
                ("git", "tk-kw"),
                (";", "tk-pu"),
                ("curl", "tk-kw"),
                ("'u'", "tk-st"),
                (">", "tk-pu"),
                ("<<'EOF'", "tk-pu"),
                ("{\"schema\":\"x\"}", "tk-st"),
                ("EOF", "tk-pu"),
            ],
        );
        for word in [
            "run",
            "fidryn-cli",
            "check",
            "x.fr",
            "FIDRYN_X=1",
            "file",
            "status",
        ] {
            assert_eq!(class_of(&html, word), None, "{word} is an argument");
        }
        assert_eq!(plain(&html), src);
    }

    #[test]
    fn shell_command_position_follows_continuations_and_heredocs() {
        let kw = keywords();
        let html = highlight(
            Lang::Shell,
            "cargo run \\\n  test\ncargo build && \\\n  cargo test",
            &kw,
        );
        let kinds: Vec<(String, String)> = spans(&html);
        assert_eq!(
            kinds,
            [
                ("tk-kw", "cargo"),
                ("tk-pu", "\\"),
                ("tk-kw", "cargo"),
                ("tk-pu", "&&"),
                ("tk-pu", "\\"),
                ("tk-kw", "cargo"),
            ]
            .map(|(c, t)| (c.to_owned(), t.to_owned()))
        );
        let html = highlight(Lang::Shell, "cat <<EOF\nhello\nEOF\necho done", &kw);
        assert_classes(
            &html,
            &[
                ("<<EOF", "tk-pu"),
                ("hello", "tk-st"),
                ("EOF", "tk-pu"),
                ("echo", "tk-kw"),
            ],
        );
    }

    #[test]
    fn highlighting_escapes_markup_and_quotes() {
        let kw = keywords();
        assert_eq!(
            highlight(Lang::Plain, "<a href=\"x\">&'</a>", &kw),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;"
        );
        assert_eq!(
            highlight(Lang::Fr, "\"<b> & 'c'\"", &kw),
            "<span class=\"tk-st\">&quot;&lt;b&gt; &amp; &#39;c&#39;&quot;</span>"
        );
        assert_eq!(
            highlight(Lang::Json, "{\"<k>\": \"a&b\"}", &kw),
            "<span class=\"tk-pu\">{</span><span class=\"tk-ty\">&quot;&lt;k&gt;&quot;</span><span class=\"tk-pu\">:</span> <span class=\"tk-st\">&quot;a&amp;b&quot;</span><span class=\"tk-pu\">}</span>"
        );
        assert_eq!(
            highlight(Lang::Shell, "echo '<x>' > out", &kw),
            "<span class=\"tk-kw\">echo</span> <span class=\"tk-st\">&#39;&lt;x&gt;&#39;</span> <span class=\"tk-pu\">&gt;</span> out"
        );
    }

    #[test]
    fn spans_never_cross_lines() {
        let kw = keywords();
        assert_eq!(
            highlight(Lang::Fr, "x = \"a\nb\"", &kw),
            "x <span class=\"tk-pu\">=</span> <span class=\"tk-st\">&quot;a</span>\n<span class=\"tk-st\">b&quot;</span>"
        );
    }

    #[test]
    fn code_frame_markup() {
        let kw = keywords();
        assert_eq!(
            code_frame(Lang::Json, "{\"a\": 1}\n", &kw, None),
            concat!(
                "<figure class=\"code\" data-lang=\"json\"><figcaption><span>json</span>",
                "<button type=\"button\" class=\"copy\" data-copy hidden>Copy</button></figcaption>",
                "<pre><code><span class=\"tk-pu\">{</span><span class=\"tk-ty\">&quot;a&quot;</span>",
                "<span class=\"tk-pu\">:</span> <span class=\"tk-nu\">1</span><span class=\"tk-pu\">}</span>",
                "</code></pre></figure>"
            )
        );
    }

    #[test]
    fn code_frame_drops_one_final_newline_and_escapes_the_caption() {
        let kw = keywords();
        let frame = code_frame(Lang::Plain, "a\n\n", &kw, Some("cases/<b>.json"));
        assert!(
            frame.contains("<span>cases/&lt;b&gt;.json</span>"),
            "{frame}"
        );
        assert!(
            frame.ends_with("<pre><code>a\n</code></pre></figure>"),
            "{frame}"
        );
        for (lang, label, class) in [
            (Lang::Fr, ".fr", "fr"),
            (Lang::Json, "json", "json"),
            (Lang::Shell, "shell", "shell"),
            (Lang::Plain, "text", "text"),
        ] {
            let frame = code_frame(lang, "", &kw, None);
            let head = format!("data-lang=\"{class}\"><figcaption><span>{label}</span>");
            assert!(frame.contains(&head), "{frame}");
        }
    }

    #[test]
    fn numbered_frame_markup_with_gap_lines() {
        let kw = keywords();
        let segments = [
            (3, "a\nb\n".to_owned()),
            (5, "c".to_owned()),
            (9, "d\n".to_owned()),
        ];
        assert_eq!(
            numbered_frame(Lang::Plain, &segments, &kw, "x.fr"),
            concat!(
                "<figure class=\"code\" data-lang=\"text\"><figcaption><span>x.fr</span>",
                "<button type=\"button\" class=\"copy\" data-copy hidden>Copy</button></figcaption>",
                "<pre class=\"lines\"><code>",
                "<span class=\"line\" data-n=\"3\">a</span>\n",
                "<span class=\"line\" data-n=\"4\">b</span>\n",
                "<span class=\"line\" data-n=\"5\">c</span>\n",
                "<span class=\"line gap\" data-n=\"\">&hellip;</span>\n",
                "<span class=\"line\" data-n=\"9\">d</span>\n",
                "</code></pre></figure>"
            )
        );
    }

    #[test]
    fn numbered_frame_highlights_inside_each_line() {
        let kw = keywords();
        let frame = numbered_frame(
            Lang::Fr,
            &[(12, "query q() -> Int {\n  return 7\n}\n".to_owned())],
            &kw,
            "tests/programs/require-gate.fr",
        );
        assert!(frame.starts_with("<figure class=\"code\" data-lang=\"fr\"><figcaption><span>tests/programs/require-gate.fr</span>"));
        assert!(frame.contains(
            "<span class=\"line\" data-n=\"13\">  <span class=\"tk-kw\">return</span> <span class=\"tk-nu\">7</span></span>\n"
        ));
        assert!(frame.contains(
            "<span class=\"line\" data-n=\"14\"><span class=\"tk-pu\">}</span></span>\n</code>"
        ));
    }

    /// Fenced blocks of a markdown file as (info, code), by line scanning.
    fn fenced_blocks(md: &str) -> Vec<(String, String)> {
        let mut blocks = Vec::new();
        let mut lines = md.lines();
        while let Some(line) = lines.next() {
            let Some(info) = line.trim_start().strip_prefix("```") else {
                continue;
            };
            let mut code = String::new();
            for body in lines.by_ref() {
                if body.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(body);
                code.push('\n');
            }
            blocks.push((info.trim().to_owned(), code));
        }
        blocks
    }

    #[test]
    fn every_fenced_block_in_docs_round_trips_in_every_language() {
        let kw = keywords();
        let mut count = 0;
        for entry in fs::read_dir(workspace_root().join("docs")).expect("read docs/") {
            let path = entry.expect("dir entry").path();
            if path.extension().is_none_or(|ext| ext != "md") {
                continue;
            }
            let md = fs::read_to_string(&path).expect("read guide");
            for (info, code) in fenced_blocks(&md) {
                for lang in LANGS {
                    let html = highlight(lang, &code, &kw);
                    assert_eq!(plain(&html), code, "{} as {lang:?}", path.display());
                }
                assert_eq!(
                    plain(&highlight(sniff(&info, &code, &kw), &code, &kw)),
                    code
                );
                count += 1;
            }
        }
        assert!(count >= 100, "found only {count} fenced blocks under docs/");
    }

    #[test]
    fn odd_input_round_trips_in_every_language() {
        let kw = keywords();
        for code in [
            "",
            "\n\n",
            "\"unterminated",
            "'",
            "§ é 日本 λx",
            "<<'EOF'\nbody",
            "tail \\",
            "{\"a\": [1, -2.5e3, true, null]}",
            "-",
            "#",
            "$ ",
            "x = \"a\\\"b\" // c\n  /// d",
            "2026-09-17T12:00:00Z +inf -inf 0..1",
            "a\u{c}b",
        ] {
            for lang in LANGS {
                assert_eq!(
                    plain(&highlight(lang, code, &kw)),
                    code,
                    "{lang:?} {code:?}"
                );
            }
        }
    }
}
