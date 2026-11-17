//! The file path of a module.
//!
//! A module with address `A` is the file `module_path(A)` under its
//! workspace's root:
//!
//! 1. Each segment of the address maps to one path segment, character by
//!    character: a lower-case ASCII letter, a digit or `~` is kept; an
//!    upper-case ASCII letter is written as `_` followed by its lower-case
//!    form; the en dash is written `-`; the ASCII hyphen is written `%2D`; a
//!    full stop is kept, except as the last character of a segment, where it
//!    is written `%2E`.
//! 2. If the mapped segment, or its part before its first full stop, compared
//!    without regard to case, is one of `con`, `prn`, `aux`, `nul`, `com1` to
//!    `com9` or `lpt1` to `lpt9`, its first character is written as `%`
//!    followed by the two upper-case hexadecimal digits of its code.
//! 3. The path is the mapped segments joined by `/`, followed by `.fr`.
//!
//! The mapping is one to one, also when paths are compared without regard to
//! case, and [`address_of_path`] is its inverse.

use crate::address::{self, Address};
use std::fmt;

/// Why a path is the file path of no module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// The path does not end in `.fr`.
    Extension,
    /// The path segment at `index`, counted from 0, is not one that
    /// [`module_path()`] writes.
    Segment { index: usize },
}

/// The names of rule 2, which some file systems reserve.
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// The file path of the module with address `address`, relative to its
/// workspace's root, with `/` between segments.
pub fn module_path(address: &Address) -> String {
    let mut path = String::with_capacity(address.as_str().len() + 8);
    for (index, segment) in address.segments().enumerate() {
        if index > 0 {
            path.push('/');
        }
        push_segment(&mut path, segment);
    }
    path.push_str(".fr");
    path
}

/// The address whose [`module_path()`] is `path`. Refuses every path that
/// [`module_path()`] does not write.
pub fn address_of_path(path: &str) -> Result<Address, LayoutError> {
    let stem = path.strip_suffix(".fr").ok_or(LayoutError::Extension)?;
    let mut text = String::with_capacity(stem.len() + 1);
    let mut written = String::new();
    for (index, mapped) in stem.split('/').enumerate() {
        let refused = LayoutError::Segment { index };
        let segment = read_segment(mapped).ok_or(refused)?;
        address::check_segment(&segment, index).map_err(|_| refused)?;
        written.clear();
        push_segment(&mut written, &segment);
        if written != mapped {
            return Err(refused);
        }
        text.push('/');
        text.push_str(&segment);
    }
    Ok(Address::from_checked(text))
}

/// Appends to `path` the path segment of the address segment `segment`, by
/// rules 1 and 2.
fn push_segment(path: &mut String, segment: &str) {
    let start = path.len();
    let mut characters = segment.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            'A'..='Z' => {
                path.push('_');
                path.push(character.to_ascii_lowercase());
            }
            '\u{2013}' => path.push('-'),
            '-' => path.push_str("%2D"),
            '.' if characters.peek().is_none() => path.push_str("%2E"),
            _ => path.push(character),
        }
    }
    let mapped = &path[start..];
    let stem = mapped.split_once('.').map_or(mapped, |(stem, _)| stem);
    if RESERVED.iter().any(|name| stem.eq_ignore_ascii_case(name)) {
        let first = path.as_bytes()[start];
        path.replace_range(start..=start, &format!("%{first:02X}"));
    }
}

/// The text that the path segment `mapped` spells, read back character by
/// character; `None` when `mapped` holds what no path segment holds. The
/// caller checks that [`push_segment`] writes `mapped` for the result.
fn read_segment(mapped: &str) -> Option<String> {
    let mut segment = String::with_capacity(mapped.len());
    let mut characters = mapped.chars();
    while let Some(character) = characters.next() {
        let read = match character {
            'a'..='z' | '0'..='9' | '~' | '.' => character,
            '_' => characters
                .next()
                .filter(char::is_ascii_lowercase)?
                .to_ascii_uppercase(),
            '-' => '\u{2013}',
            '%' => {
                let high = characters.next()?.to_digit(16)?;
                let low = characters.next()?.to_digit(16)?;
                char::from_u32(high * 16 + low)?
            }
            _ => return None,
        };
        segment.push(read);
    }
    Some(segment)
}

impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Extension => f.write_str("the path does not end in `.fr`"),
            Self::Segment { index } => write!(
                f,
                "segment {index} of the path, counted from 0, is not one that the file path of \
                 any address holds"
            ),
        }
    }
}

impl std::error::Error for LayoutError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Xorshift;
    use std::collections::BTreeMap;

    fn address(text: &str) -> Address {
        Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error:?}"))
    }

    /// Every segment form of the publisher's identifiers, and the forms an
    /// ingester adds.
    const FORMS: &[&str] = &[
        "/zz/demo/t29/s2611/2/B/ii",
        "/zz/demo/t42/s1395i\u{2013}3",
        "/zz/demo/t42/ch7/schIX\u{2013}A",
        "/zz/demo/t28a/courtRules/Civil/rule71.1",
        "/zz/demo/t2/s44...46",
        "/zz/demo/rule1001",
        "/zz/demo/ptI",
        "/zz/demo/t5/ptIII/sptE",
        "/zz/demo/t28/s1932~2",
        "/zz/demo/t5a/~compiledAct.1/~section.2",
        "/zz/demo/a-b",
        "/zz/demo/a.",
        "/zz/demo/con",
        "/zz/demo/con.1",
        "/zz/demo/lpt1",
    ];

    /// Letters of the reserved names, in both cases, and every other
    /// character a segment may hold.
    const ALPHABET: &[char] = &[
        'a', 'c', 'l', 'm', 'n', 'o', 'p', 'r', 't', 'u', 'x', 'z', 'A', 'C', 'L', 'M', 'N', 'O',
        'P', 'R', 'T', 'U', 'X', 'Z', '0', '1', '4', '9', '-', '.', '~', '\u{2013}',
    ];

    const RESERVED: &[&str] = &[
        "con", "prn", "aux", "nul", "com1", "com5", "com9", "lpt1", "lpt4", "lpt9",
    ];

    const SUFFIXES: &[&str] = &["", ".", ".1", "..", "~2", "x", "-", "\u{2013}a"];

    fn generated_segment(rng: &mut Xorshift) -> String {
        loop {
            let segment: String = if rng.below(4) == 0 {
                let name = rng.pick(RESERVED);
                let mut segment: String = name
                    .chars()
                    .map(|c| {
                        if rng.below(3) == 0 {
                            c.to_ascii_uppercase()
                        } else {
                            c
                        }
                    })
                    .collect();
                segment.push_str(rng.pick(SUFFIXES));
                segment
            } else {
                let length = 1 + rng.below(8);
                (0..length).map(|_| *rng.pick(ALPHABET)).collect()
            };
            if segment != "." && segment != ".." {
                return segment;
            }
        }
    }

    fn generated_addresses(seed: u64, count: usize) -> Vec<Address> {
        let mut rng = Xorshift::new(seed);
        (0..count)
            .map(|_| {
                let segments = 1 + rng.below(6);
                let text: String = (0..segments)
                    .map(|_| format!("/{}", generated_segment(&mut rng)))
                    .collect();
                address(&text)
            })
            .collect()
    }

    #[test]
    fn module_path_follows_the_examples() {
        for (text, path) in [
            ("/zz/demo/t29/s2611", "zz/demo/t29/s2611.fr"),
            ("/zz/demo/t42/s1395i\u{2013}3", "zz/demo/t42/s1395i-3.fr"),
            ("/zz/demo/a-b", "zz/demo/a%2Db.fr"),
            ("/zz/demo/t2/s44...46", "zz/demo/t2/s44...46.fr"),
            ("/zz/demo/a.", "zz/demo/a%2E.fr"),
            ("/zz/demo/t5/ptIII/sptE", "zz/demo/t5/pt_i_i_i/spt_e.fr"),
            (
                "/zz/demo/t28a/courtRules/Civil/rule6",
                "zz/demo/t28a/court_rules/_civil/rule6.fr",
            ),
            ("/zz/demo/t28/s1932~2", "zz/demo/t28/s1932~2.fr"),
            ("/zz/demo/con", "zz/demo/%63on.fr"),
            ("/zz/demo/con.1", "zz/demo/%63on.1.fr"),
            ("/zz/demo/lpt1", "zz/demo/%6Cpt1.fr"),
        ] {
            assert_eq!(module_path(&address(text)), path, "{text}");
        }
        assert_eq!(module_path(&address("/con/nul")), "%63on/%6Eul.fr");
        assert_eq!(module_path(&address("/zz/Con")), "zz/_con.fr");
        assert_eq!(module_path(&address("/zz/con~1")), "zz/con~1.fr");
    }

    #[test]
    fn paths_of_addresses_that_differ_only_by_case_are_distinct() {
        let upper = module_path(&address("/zz/demo/ch6A"));
        let lower = module_path(&address("/zz/demo/ch6a"));
        assert_ne!(upper.to_lowercase(), lower.to_lowercase());

        let swapped = |text: &str| -> String {
            text.chars()
                .map(|c| {
                    if c.is_ascii_uppercase() {
                        c.to_ascii_lowercase()
                    } else {
                        c.to_ascii_uppercase()
                    }
                })
                .collect()
        };
        let mut seen: BTreeMap<String, Address> = BTreeMap::new();
        let mut pairs = 0;
        for generated in generated_addresses(0x5EED_0031, 2_000) {
            let text = generated.as_str();
            for variant in [
                text.to_owned(),
                text.to_ascii_lowercase(),
                text.to_ascii_uppercase(),
                swapped(text),
            ] {
                let a = address(&variant);
                let folded = module_path(&a).to_lowercase();
                if let Some(other) = seen.insert(folded.clone(), a.clone()) {
                    assert_eq!(other, a, "{other} and {a} share the path {folded}");
                } else if a.as_str() != text {
                    pairs += 1;
                }
            }
        }
        assert!(
            pairs > 1_000,
            "only {pairs} addresses differ from another by case"
        );
    }

    #[test]
    fn layout_is_reversible_for_every_segment_form() {
        let generated = generated_addresses(0x5EED_0032, 2_000);
        let paths: Vec<String> = generated.iter().map(module_path).collect();
        for form in [
            "_", "-", "%2D", "%2E", "~", "%61", "%63", "%6C", "%6E", "%70",
        ] {
            assert!(
                paths.iter().any(|path| path.contains(form)),
                "no generated path holds {form}"
            );
        }
        let forms = FORMS.iter().map(|text| address(text));
        for a in forms.chain(generated) {
            let path = module_path(&a);
            assert_eq!(address_of_path(&path), Ok(a.clone()), "{a} at {path}");
        }

        for (path, error) in [
            ("zz/demo/CH1.fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/a_.fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/a%2d.fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/a%5F.fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/a.txt", LayoutError::Extension),
            ("zz\\demo\\a.fr", LayoutError::Segment { index: 0 }),
            ("zz/demo/con.fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/a%2Fb.fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/a%2Eb.fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/a..fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/%61b.fr", LayoutError::Segment { index: 2 }),
            ("zz/demo/a\u{2013}b.fr", LayoutError::Segment { index: 2 }),
            ("zz//a.fr", LayoutError::Segment { index: 1 }),
            ("/zz/a.fr", LayoutError::Segment { index: 0 }),
            ("./zz/a.fr", LayoutError::Segment { index: 0 }),
            ("zz/a/.fr", LayoutError::Segment { index: 2 }),
            ("", LayoutError::Extension),
        ] {
            assert_eq!(address_of_path(path), Err(error), "{path:?}");
        }
        assert_eq!(
            address_of_path("zz/demo/a-b.fr"),
            Ok(address("/zz/demo/a\u{2013}b"))
        );
    }
}
