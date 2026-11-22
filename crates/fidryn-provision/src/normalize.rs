//! The normalization of text.
//!
//! [`normalize`] puts text into Unicode Normalization Form C, replaces each
//! character of [`WHITE_SPACE`] with U+0020, replaces each run of U+0020 with
//! one, and removes U+0020 at the start and at the end. Nothing else changes.

use unicode_normalization::UnicodeNormalization;

/// The 25 characters with the Unicode property `White_Space`, in code point
/// order: U+0009 to U+000D, U+0020, U+0085, U+00A0, U+1680, U+2000 to
/// U+200A, U+2028, U+2029, U+202F, U+205F and U+3000.
pub const WHITE_SPACE: [char; 25] = [
    '\u{9}', '\u{A}', '\u{B}', '\u{C}', '\u{D}', '\u{20}', '\u{85}', '\u{A0}', '\u{1680}',
    '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}',
    '\u{2008}', '\u{2009}', '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}',
];

/// `raw`, normalized.
pub fn normalize(raw: &str) -> String {
    let mut text = String::with_capacity(raw.len());
    let mut space = false;
    for character in raw.nfc() {
        if WHITE_SPACE.contains(&character) {
            space = !text.is_empty();
        } else {
            if space {
                text.push(' ');
                space = false;
            }
            text.push(character);
        }
    }
    text
}

/// Whether `text` is normalized: `normalize(text) == text`.
pub fn is_normalized(text: &str) -> bool {
    normalize(text) == text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Xorshift;

    #[test]
    fn normalize_follows_the_table() {
        for (raw, normalized) in [
            ("  a  b ", "a b"),
            ("a\u{A0}b", "a b"),
            ("\u{A7}\u{202F}1", "\u{A7} 1"),
            ("Current\u{2000}through", "Current through"),
            ("a\tb\nc\r\nd", "a b c d"),
            ("e\u{301}", "\u{E9}"),
            ("1\u{A0}000", "1 000"),
            ("\u{3000}a\u{1680}", "a"),
            ("a\u{200B}b", "a\u{200B}b"),
            ("\u{FEFF}a", "\u{FEFF}a"),
            ("[sic]", "[sic]"),
            (
                "\u{201C}x\u{201D} \u{2014} y",
                "\u{201C}x\u{201D} \u{2014} y",
            ),
        ] {
            assert_eq!(normalize(raw), normalized, "{raw:?}");
            assert!(is_normalized(normalized), "{normalized:?}");
            assert_eq!(is_normalized(raw), raw == normalized, "{raw:?}");
        }
        assert_eq!(normalize(""), "");
        assert_eq!(normalize(" \u{2028}\u{2029} "), "");
        assert!(is_normalized(""));
    }

    #[test]
    fn the_white_space_list_is_the_unicode_property() {
        let mut count = 0;
        for code in 0..=u32::from(char::MAX) {
            let Some(c) = char::from_u32(code) else {
                continue;
            };
            assert_eq!(WHITE_SPACE.contains(&c), c.is_whitespace(), "U+{code:04X}");
            if c.is_whitespace() {
                count += 1;
            }
        }
        assert_eq!(count, WHITE_SPACE.len());
        assert!(WHITE_SPACE.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn normalize_is_idempotent() {
        const LETTERS: &[char] = &[
            'a', 'e', 'o', 's', 'u', 'z', 'A', 'E', 'O', 'Z', '\u{C5}', '\u{E9}', '\u{1A1}',
            '\u{212B}',
        ];
        const MARKS: &[char] = &[
            '\u{300}', '\u{301}', '\u{308}', '\u{31B}', '\u{323}', '\u{327}', '\u{345}',
        ];
        let mut rng = Xorshift::new(0x5EED_0003);
        for _ in 0..2_000 {
            let length = rng.below(13);
            let raw: String = (0..length)
                .map(|_| match rng.below(4) {
                    0 => *rng.pick(LETTERS),
                    1 => *rng.pick(&WHITE_SPACE),
                    2 => *rng.pick(MARKS),
                    _ => {
                        if rng.below(2) == 0 {
                            '\u{200B}'
                        } else {
                            *rng.pick(LETTERS)
                        }
                    }
                })
                .collect();
            let once = normalize(&raw);
            assert_eq!(normalize(&once), once, "{raw:?}");
            assert!(is_normalized(&once), "{raw:?}");
            assert!(unicode_normalization::is_nfc(&once), "{raw:?}");
        }
    }
}
