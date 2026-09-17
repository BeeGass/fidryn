//! Content-hashed identifiers. Stable within a module's content hash.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! hashed_id {
    ($name:ident, $tag:literal) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 16]);

        impl $name {
            pub fn of(payload: &[u8]) -> Self {
                let mut hasher = blake3::Hasher::new();
                hasher.update($tag.as_bytes());
                hasher.update(&[0xff]);
                hasher.update(payload);
                let hash = *hasher.finalize().as_bytes();
                let mut id = [0u8; 16];
                id.copy_from_slice(&hash[..16]);
                Self(id)
            }

            pub fn from_bytes(bytes: [u8; 16]) -> Self {
                Self(bytes)
            }

            pub fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }

            pub fn hex(&self) -> String {
                hex_encode(&self.0)
            }

            pub fn from_hex(text: &str) -> Result<Self, String> {
                hex_decode(text).map(Self)
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.hex())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let text = String::deserialize(deserializer)?;
                Self::from_hex(&text).map_err(serde::de::Error::custom)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.hex())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.hex())
            }
        }
    };
}

hashed_id!(NodeId, "node");
hashed_id!(ModuleId, "module");
hashed_id!(ClauseId, "clause");
hashed_id!(RuleId, "rule");
hashed_id!(EffectId, "effect");
hashed_id!(TraceId, "trace");
hashed_id!(CompletionProofId, "proof");
hashed_id!(SourceSnapshotId, "snapshot");
hashed_id!(SourceManifestId, "manifest");
hashed_id!(JurisdictionId, "jurisdiction");

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QueryName(pub String);

impl QueryName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for QueryName {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OriginId {
    Direct(NodeId),
    Clause(ClauseId),
    Imported(NodeId),
}

pub fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

pub fn hex_decode(text: &str) -> Result<[u8; 16], String> {
    if text.len() != 32 {
        return Err(format!("expected 32 hex characters, got {}", text.len()));
    }
    let bytes = text.as_bytes();
    let mut out = [0u8; 16];
    for i in 0..16 {
        let hi = hex_nibble(bytes[i * 2])?;
        let lo = hex_nibble(bytes[i * 2 + 1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, String> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err("invalid hex character".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_is_deterministic() {
        let a = NodeId::of(b"TrusteeOf");
        let b = NodeId::of(b"TrusteeOf");
        assert_eq!(a, b);
        assert_ne!(a, NodeId::of(b"other"));
        assert_eq!(a.hex().len(), 32);
    }

    #[test]
    fn distinct_tags_do_not_collide() {
        assert_ne!(NodeId::of(b"x").hex(), RuleId::of(b"x").hex());
    }

    #[test]
    fn trace_id_serializes_as_hex_string() {
        let id = TraceId::of(b"t");
        let json = serde_json::to_value(id).unwrap();
        assert!(json.is_string(), "{json}");
        assert_eq!(json.as_str().unwrap(), id.hex());
        assert_eq!(json.as_str().unwrap().len(), 32);
        let back: TraceId = serde_json::from_value(json).unwrap();
        assert_eq!(back, id);
        assert!(!serde_json::to_value(id).unwrap().is_array());
    }

    #[test]
    fn hashed_ids_used_in_outcome_json_are_hex_strings() {
        for json in [
            serde_json::to_value(NodeId::of(b"n")).unwrap(),
            serde_json::to_value(CompletionProofId::of(b"p")).unwrap(),
            serde_json::to_value(TraceId::of(b"t")).unwrap(),
        ] {
            assert!(json.is_string(), "{json}");
            assert_eq!(json.as_str().unwrap().len(), 32);
        }
    }
}
