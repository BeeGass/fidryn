//! Verified source bundles: per-artifact byte checks, not digest-shaped strings.
//!
//! This type never reads filesystem paths. Observed bytes are supplied by the
//! caller, so pasted mill source cannot follow server paths.

use crate::case::{ManifestArtifact, SourceManifest};
use crate::ids::hex_encode;
use crate::outcome::TrustProfile;
use serde::{Deserialize, Serialize};

/// How one manifest artifact was classified for verification.
///
/// `fixture` is not byte-verified. `hex` is a 32- or 64-digit digest.
/// `none` is any other digest string.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactVerificationMethod {
    Fixture,
    Hex,
    #[default]
    None,
}

/// One pinned artifact inside a [`VerifiedSourceBundle`].
///
/// `ok` is true for `"fixture"` without bytes, and for hex only when
/// observed bytes were supplied and matched the expected digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BundleArtifact {
    pub path: String,
    pub expected_digest: String,
    pub observed_digest: Option<String>,
    pub method: ArtifactVerificationMethod,
    pub ok: bool,
}

/// Blake3 identity of pinned artifact bytes and digests.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceBundleId([u8; 32]);

impl SourceBundleId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn hex(&self) -> String {
        hex_encode(&self.0)
    }
}

impl std::fmt::Debug for SourceBundleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SourceBundleId({})", self.hex())
    }
}

impl std::fmt::Display for SourceBundleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.hex())
    }
}

/// Immutable record of which source artifacts were actually checked.
///
/// [`Self::trust_summary`] is [`TrustProfile::ByteVerified`] only when every
/// hex artifact was checked and matched. Any fixture artifact yields
/// [`TrustProfile::Fixture`]. Otherwise the bundle is unauthenticated.
/// Identity is a hash of pinned observed digests, not of digest-looking
/// strings alone.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedSourceBundle {
    pub snapshot: String,
    pub jurisdiction: String,
    pub artifacts: Vec<BundleArtifact>,
}

impl VerifiedSourceBundle {
    /// Bundle for `manifest` with no observed bytes. Never reads paths.
    pub fn from_manifest(manifest: &SourceManifest) -> Self {
        Self::from_observed(manifest, |_| None)
    }

    /// Pin `manifest` artifacts using caller-supplied bytes for each path.
    ///
    /// The closure is the only source of bytes. Returning `None` records an
    /// unread hex artifact (`ok = false`). This method does not open paths.
    pub fn from_observed<F>(manifest: &SourceManifest, mut observed: F) -> Self
    where
        F: FnMut(&str) -> Option<Vec<u8>>,
    {
        let artifacts = manifest
            .artifacts
            .iter()
            .map(|artifact| BundleArtifact::pin(artifact, observed(&artifact.path).as_deref()))
            .collect();
        Self {
            snapshot: manifest.snapshot.clone(),
            jurisdiction: manifest.jurisdiction.clone(),
            artifacts,
        }
    }

    /// Blake3 of pinned snapshot labels, paths, expected digests, observed
    /// digests (or an unread sentinel), method, and per-artifact `ok`.
    pub fn identity(&self) -> SourceBundleId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"fidryn.source-bundle/v0.1");
        hasher.update(&[0xff]);
        hasher.update(self.snapshot.as_bytes());
        hasher.update(&[0xff]);
        hasher.update(self.jurisdiction.as_bytes());
        hasher.update(&[0xff]);
        for artifact in &self.artifacts {
            hasher.update(artifact.path.as_bytes());
            hasher.update(&[0xff]);
            hasher.update(artifact.expected_digest.as_bytes());
            hasher.update(&[0xff]);
            match &artifact.observed_digest {
                Some(digest) => hasher.update(digest.as_bytes()),
                None => hasher.update(b"unread"),
            };
            hasher.update(&[0xff]);
            hasher.update(method_tag(artifact.method));
            hasher.update(&[0xff]);
            hasher.update(&[u8::from(artifact.ok)]);
            hasher.update(&[0xff]);
        }
        SourceBundleId(*hasher.finalize().as_bytes())
    }

    /// Aggregate trust from recorded per-artifact checks.
    ///
    /// Does not re-parse digest-looking strings as authentication. Fixture
    /// wins over hex. ByteVerified requires at least one hex artifact and
    /// every hex artifact `ok`.
    pub fn trust_summary(&self) -> TrustProfile {
        let mut any_fixture = false;
        let mut hex_count = 0usize;
        let mut hex_ok = 0usize;
        for artifact in &self.artifacts {
            match artifact.method {
                ArtifactVerificationMethod::Fixture => any_fixture = true,
                ArtifactVerificationMethod::Hex => {
                    hex_count += 1;
                    if artifact.ok {
                        hex_ok += 1;
                    }
                }
                ArtifactVerificationMethod::None => {}
            }
        }
        if any_fixture {
            TrustProfile::Fixture
        } else if hex_count > 0 && hex_ok == hex_count {
            TrustProfile::ByteVerified
        } else {
            TrustProfile::Unauthenticated
        }
    }
}

impl BundleArtifact {
    fn pin(artifact: &ManifestArtifact, bytes: Option<&[u8]>) -> Self {
        let expected_digest = artifact.digest.trim().to_string();
        let method = method_of(&expected_digest);
        let observed_digest = bytes.map(|b| hex_encode(blake3::hash(b).as_bytes()));
        let ok = match method {
            ArtifactVerificationMethod::Fixture => true,
            ArtifactVerificationMethod::Hex => bytes
                .map(|b| digest_matches(&expected_digest, b))
                .unwrap_or(false),
            ArtifactVerificationMethod::None => false,
        };
        Self {
            path: artifact.path.clone(),
            expected_digest,
            observed_digest,
            method,
            ok,
        }
    }
}

fn method_of(digest: &str) -> ArtifactVerificationMethod {
    if digest.eq_ignore_ascii_case("fixture") {
        ArtifactVerificationMethod::Fixture
    } else if is_plausible_hex_digest(digest) {
        ArtifactVerificationMethod::Hex
    } else {
        ArtifactVerificationMethod::None
    }
}

fn method_tag(method: ArtifactVerificationMethod) -> &'static [u8] {
    match method {
        ArtifactVerificationMethod::Fixture => b"fixture",
        ArtifactVerificationMethod::Hex => b"hex",
        ArtifactVerificationMethod::None => b"none",
    }
}

fn is_plausible_hex_digest(digest: &str) -> bool {
    matches!(digest.len(), 32 | 64) && digest.bytes().all(|b| b.is_ascii_hexdigit())
}

fn digest_matches(expected: &str, bytes: &[u8]) -> bool {
    let hash = *blake3::hash(bytes).as_bytes();
    let full = hex_encode(&hash);
    match expected.len() {
        64 => expected.eq_ignore_ascii_case(&full),
        32 => expected.eq_ignore_ascii_case(&full[..32]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::case::SourceWeight;

    fn artifact(path: &str, digest: &str) -> ManifestArtifact {
        ManifestArtifact {
            path: path.into(),
            digest: digest.into(),
            kind: "text".into(),
            effective: "2026-01-01".into(),
            weight: SourceWeight::Explanatory,
        }
    }

    fn manifest(artifacts: Vec<ManifestArtifact>) -> SourceManifest {
        SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: "snap".into(),
            jurisdiction: "Test".into(),
            artifacts,
        }
    }

    #[test]
    fn empty_bundle_is_unauthenticated() {
        let bundle = VerifiedSourceBundle::from_manifest(&SourceManifest::default());
        assert!(bundle.artifacts.is_empty());
        assert_eq!(bundle.trust_summary(), TrustProfile::Unauthenticated);
        assert_ne!(bundle.trust_summary(), TrustProfile::ByteVerified);
    }

    #[test]
    fn fixture_without_bytes_is_fixture_never_byte_verified() {
        let bundle =
            VerifiedSourceBundle::from_manifest(&manifest(vec![artifact("Other.Law", "fixture")]));
        assert_eq!(
            bundle.artifacts[0].method,
            ArtifactVerificationMethod::Fixture
        );
        assert!(bundle.artifacts[0].ok);
        assert!(bundle.artifacts[0].observed_digest.is_none());
        assert_eq!(bundle.trust_summary(), TrustProfile::Fixture);
        assert_ne!(bundle.trust_summary(), TrustProfile::ByteVerified);
    }

    #[test]
    fn hex_without_bytes_is_not_byte_verified() {
        let digest = hex_encode(blake3::hash(b"fidryn-source-bytes").as_bytes());
        let bundle =
            VerifiedSourceBundle::from_manifest(&manifest(vec![artifact("Other.Law", &digest)]));
        assert_eq!(bundle.artifacts[0].method, ArtifactVerificationMethod::Hex);
        assert!(!bundle.artifacts[0].ok);
        assert!(bundle.artifacts[0].observed_digest.is_none());
        assert_eq!(bundle.trust_summary(), TrustProfile::Unauthenticated);
        assert_ne!(bundle.trust_summary(), TrustProfile::ByteVerified);
    }

    #[test]
    fn matching_blake3_hex_is_byte_verified() {
        let bytes = b"fidryn-source-bytes";
        let digest = hex_encode(blake3::hash(bytes).as_bytes());
        let bundle = VerifiedSourceBundle::from_observed(
            &manifest(vec![artifact("Other.Law", &digest)]),
            |path| {
                assert_eq!(path, "Other.Law");
                Some(bytes.to_vec())
            },
        );
        assert!(bundle.artifacts[0].ok);
        assert_eq!(
            bundle.artifacts[0].observed_digest.as_deref(),
            Some(digest.as_str())
        );
        assert_eq!(bundle.trust_summary(), TrustProfile::ByteVerified);
    }

    #[test]
    fn matching_truncated_blake3_hex_is_byte_verified() {
        let bytes = b"fidryn-source-bytes";
        let digest = hex_encode(&blake3::hash(bytes).as_bytes()[..16]);
        assert_eq!(digest.len(), 32);
        let bundle = VerifiedSourceBundle::from_observed(
            &manifest(vec![artifact("Other.Law", &digest)]),
            |_| Some(bytes.to_vec()),
        );
        assert!(bundle.artifacts[0].ok);
        assert_eq!(bundle.trust_summary(), TrustProfile::ByteVerified);
    }

    #[test]
    fn mismatched_blake3_hex_is_not_ok() {
        let digest = hex_encode(blake3::hash(b"expected").as_bytes());
        let bundle = VerifiedSourceBundle::from_observed(
            &manifest(vec![artifact("Other.Law", &digest)]),
            |_| Some(b"tampered-bytes".to_vec()),
        );
        assert!(!bundle.artifacts[0].ok);
        assert_eq!(bundle.trust_summary(), TrustProfile::Unauthenticated);
    }

    #[test]
    fn mixed_checked_and_unread_hex_is_unauthenticated() {
        let bytes = b"fidryn-source-bytes";
        let good = hex_encode(blake3::hash(bytes).as_bytes());
        let dangling = hex_encode(blake3::hash(b"missing").as_bytes());
        let bundle = VerifiedSourceBundle::from_observed(
            &manifest(vec![
                artifact("Other.Law", &good),
                artifact("never-created.txt", &dangling),
            ]),
            |path| {
                if path == "Other.Law" {
                    Some(bytes.to_vec())
                } else {
                    None
                }
            },
        );
        assert!(bundle.artifacts[0].ok);
        assert!(!bundle.artifacts[1].ok);
        assert_eq!(bundle.trust_summary(), TrustProfile::Unauthenticated);
    }

    #[test]
    fn any_fixture_wins_over_matching_hex() {
        let bytes = b"fidryn-source-bytes";
        let digest = hex_encode(blake3::hash(bytes).as_bytes());
        let bundle = VerifiedSourceBundle::from_observed(
            &manifest(vec![
                artifact("fixture.txt", "fixture"),
                artifact("Other.Law", &digest),
            ]),
            |path| {
                if path == "Other.Law" {
                    Some(bytes.to_vec())
                } else {
                    None
                }
            },
        );
        assert_eq!(bundle.trust_summary(), TrustProfile::Fixture);
        assert_ne!(bundle.trust_summary(), TrustProfile::ByteVerified);
    }

    #[test]
    fn identity_hashes_pinned_bytes_not_only_expected_digest() {
        let expected = hex_encode(blake3::hash(b"expected").as_bytes());
        let a = VerifiedSourceBundle::from_observed(
            &manifest(vec![artifact("Other.Law", &expected)]),
            |_| Some(b"bytes-a".to_vec()),
        );
        let b = VerifiedSourceBundle::from_observed(
            &manifest(vec![artifact("Other.Law", &expected)]),
            |_| Some(b"bytes-b".to_vec()),
        );
        let unread =
            VerifiedSourceBundle::from_manifest(&manifest(vec![artifact("Other.Law", &expected)]));
        assert_ne!(a.identity(), b.identity());
        assert_ne!(a.identity(), unread.identity());
        assert_eq!(
            a.identity(),
            VerifiedSourceBundle::from_observed(
                &manifest(vec![artifact("Other.Law", &expected)]),
                |_| Some(b"bytes-a".to_vec()),
            )
            .identity()
        );
    }

    #[test]
    fn from_manifest_does_not_consult_observed_bytes() {
        let digest = hex_encode(blake3::hash(b"secret").as_bytes());
        let bundle = VerifiedSourceBundle::from_manifest(&manifest(vec![artifact(
            "/this/path/must/not/be/read",
            &digest,
        )]));
        assert!(bundle.artifacts[0].observed_digest.is_none());
        assert!(!bundle.artifacts[0].ok);
        assert_eq!(bundle.trust_summary(), TrustProfile::Unauthenticated);
    }

    #[test]
    fn non_hex_digest_is_method_none() {
        let bundle =
            VerifiedSourceBundle::from_manifest(&manifest(vec![artifact("Other.Law", "abc")]));
        assert_eq!(bundle.artifacts[0].method, ArtifactVerificationMethod::None);
        assert!(!bundle.artifacts[0].ok);
        assert_eq!(bundle.trust_summary(), TrustProfile::Unauthenticated);
    }
}
