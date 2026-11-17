//! Addresses of modules, provisions and notes.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// The name of a module, a provision or a note: `/` followed by segments
/// separated by `/`.
///
/// A segment is one or more ASCII letters, ASCII digits, `-`, `.`, `~` and
/// the en dash U+2013, and is never `.` or `..`. Two addresses are equal when
/// their texts are equal byte for byte, and they are ordered by the bytes of
/// their text.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(String);

/// Why a text is no address. `index` counts the segments from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressError {
    /// The text is empty.
    Empty,
    /// The text does not begin with `/`.
    NoLeadingSlash,
    /// The segment at `index` is empty.
    EmptySegment { index: usize },
    /// The segment at `index` is `.` or `..`.
    DotSegment { index: usize },
    /// The segment at `index` holds `character`, which no segment holds.
    Character { index: usize, character: char },
}

/// Whether `character` may stand in a segment.
fn is_segment_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '~' | '\u{2013}')
}

/// Checks `segment` as the segment at `index` of an address.
pub(crate) fn check_segment(segment: &str, index: usize) -> Result<(), AddressError> {
    if segment.is_empty() {
        return Err(AddressError::EmptySegment { index });
    }
    if segment == "." || segment == ".." {
        return Err(AddressError::DotSegment { index });
    }
    match segment.chars().find(|&c| !is_segment_character(c)) {
        Some(character) => Err(AddressError::Character { index, character }),
        None => Ok(()),
    }
}

/// Checks `text` as an address.
pub(crate) fn check(text: &str) -> Result<(), AddressError> {
    if text.is_empty() {
        return Err(AddressError::Empty);
    }
    let Some(segments) = text.strip_prefix('/') else {
        return Err(AddressError::NoLeadingSlash);
    };
    segments
        .split('/')
        .enumerate()
        .try_for_each(|(index, segment)| check_segment(segment, index))
}

impl Address {
    /// The address written as `text`.
    pub fn parse(text: &str) -> Result<Address, AddressError> {
        check(text)?;
        Ok(Address(text.to_owned()))
    }

    /// The address written as `text`, which the caller has checked segment by
    /// segment.
    pub(crate) fn from_checked(text: String) -> Address {
        debug_assert_eq!(check(&text), Ok(()), "{text}");
        Address(text)
    }

    /// The text of the address.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The segments of the address, in order.
    pub fn segments(&self) -> std::str::Split<'_, char> {
        self.0[1..].split('/')
    }

    /// The address without its last segment; `None` for an address of one
    /// segment.
    pub fn parent(&self) -> Option<Address> {
        match self.0.rfind('/') {
            Some(end) if end > 0 => Some(Address(self.0[..end].to_owned())),
            _ => None,
        }
    }

    /// The address with `segment` added as its last segment. `segment` is one
    /// segment: a `/` in it is a character no segment holds.
    pub fn child(&self, segment: &str) -> Result<Address, AddressError> {
        check_segment(segment, self.segments().count())?;
        Ok(Address(format!("{}/{segment}", self.0)))
    }

    /// Whether this address equals `other` or lies below it at a segment
    /// boundary.
    pub fn is_within(&self, other: &Address) -> bool {
        self.0
            .strip_prefix(other.0.as_str())
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Display for AddressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Empty => f.write_str("the address is empty"),
            Self::NoLeadingSlash => f.write_str("the address does not begin with `/`"),
            Self::EmptySegment { index } => {
                write!(
                    f,
                    "segment {index} of the address, counted from 0, is empty"
                )
            }
            Self::DotSegment { index } => write!(
                f,
                "segment {index} of the address, counted from 0, is `.` or `..`"
            ),
            Self::Character { index, character } => write!(
                f,
                "segment {index} of the address, counted from 0, holds {character:?} \
                 (U+{:04X}); a segment holds only ASCII letters, ASCII digits, `-`, `.`, `~` \
                 and the en dash",
                u32::from(character)
            ),
        }
    }
}

impl std::error::Error for AddressError {}

impl Serialize for Address {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Address {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        check(&text).map_err(serde::de::Error::custom)?;
        Ok(Address(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    fn address(text: &str) -> Address {
        Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error:?}"))
    }

    #[test]
    fn parse_accepts_the_forms_the_publisher_uses() {
        for text in [
            "/zz/demo/t29/s2611/2/B/ii",
            "/zz/demo/t42/s1395i\u{2013}3",
            "/zz/demo/t28a/courtRules/Civil/rule71.1",
            "/zz/demo/t2/s44...46",
            "/zz/demo/rule1001",
            "/zz/demo/t28/s1932~2",
            "/zz/demo/t5a/~compiledAct.1/~section.2",
            "/zz/demo/a-b",
        ] {
            let parsed = address(text);
            assert_eq!(parsed.as_str(), text);
            assert_eq!(parsed.to_string(), text);
        }
        assert_eq!(
            address("/zz/demo/t42/s1395i\u{2013}3")
                .segments()
                .collect::<Vec<_>>(),
            ["zz", "demo", "t42", "s1395i\u{2013}3"]
        );
        assert_eq!(address("/zz").segments().collect::<Vec<_>>(), ["zz"]);
    }

    #[test]
    fn parse_rejects_what_is_no_address() {
        for (text, error) in [
            ("", AddressError::Empty),
            ("zz/demo", AddressError::NoLeadingSlash),
            ("/", AddressError::EmptySegment { index: 0 }),
            ("/zz//demo", AddressError::EmptySegment { index: 1 }),
            ("/zz/demo/", AddressError::EmptySegment { index: 2 }),
            ("/zz/./a", AddressError::DotSegment { index: 1 }),
            ("/zz/../a", AddressError::DotSegment { index: 1 }),
            (
                "/zz/a b",
                AddressError::Character {
                    index: 1,
                    character: ' ',
                },
            ),
            (
                "/zz/a_b",
                AddressError::Character {
                    index: 1,
                    character: '_',
                },
            ),
            (
                "/zz/a%20b",
                AddressError::Character {
                    index: 1,
                    character: '%',
                },
            ),
        ] {
            assert_eq!(Address::parse(text), Err(error), "{text:?}");
        }
    }

    #[test]
    fn addresses_that_differ_only_by_case_are_distinct() {
        let lower = address("/zz/demo/t1/s8/a");
        let upper = address("/zz/demo/t1/s8/A");
        assert_ne!(lower, upper);
        assert_eq!(upper.cmp(&lower), Ordering::Less);
        assert_eq!(
            upper.cmp(&lower),
            upper.as_str().as_bytes().cmp(lower.as_str().as_bytes())
        );
    }

    #[test]
    fn parent_child_and_is_within() {
        let s8 = address("/zz/demo/t1/s8");
        let a = address("/zz/demo/t1/s8/a");
        assert_eq!(a.parent(), Some(s8.clone()));
        assert_eq!(address("/zz/demo").parent(), Some(address("/zz")));
        assert_eq!(address("/zz").parent(), None);

        assert_eq!(s8.child("a"), Ok(a.clone()));
        assert_eq!(s8.child(""), Err(AddressError::EmptySegment { index: 4 }));
        assert_eq!(s8.child(".."), Err(AddressError::DotSegment { index: 4 }));
        assert_eq!(
            s8.child("a/b"),
            Err(AddressError::Character {
                index: 4,
                character: '/',
            })
        );

        assert!(a.is_within(&s8));
        assert!(a.is_within(&address("/zz")));
        assert!(s8.is_within(&s8));
        assert!(!s8.is_within(&a));
        assert!(!address("/zz/demo/t1/s80").is_within(&s8));
        assert!(!address("/zz/demo/t1/s8a").is_within(&s8));
    }

    #[test]
    fn an_address_is_written_as_its_text_and_checked_when_read() {
        let a = address("/zz/demo/t42/s1395i\u{2013}3");
        let json = serde_json::to_string(&a).expect("serialize");
        assert_eq!(json, "\"/zz/demo/t42/s1395i\u{2013}3\"");
        let read: Address = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(read, a);
        let error = serde_json::from_str::<Address>("\"/zz//demo\"")
            .expect_err("an empty segment is no address");
        assert!(
            error.to_string().contains("segment 1"),
            "unexpected message: {error}"
        );
        assert!(serde_json::from_str::<Address>("17").is_err());
    }
}
