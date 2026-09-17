//! Content-hashed identifiers. Stable within a module's content hash.

use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! hashed_id {
    ($name:ident, $tag:literal) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
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
}
