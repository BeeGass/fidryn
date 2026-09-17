//! Package identity. Path compile may resolve `source_root/packages`.
//! In-memory compile never follows package paths.

use crate::case::{ManifestArtifact, SourceWeight};
use crate::ids::PackageId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Schema id for [`PackageLock`] JSON.
pub const PACKAGE_LOCK_SCHEMA: &str = "fidryn.package-lock/v0.1";

/// Directory name under `source_root` that holds packages for path compile.
pub const PACKAGES_DIR: &str = "packages";

/// Locked package identity: name, version, and blake3 digest of the module bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageLock {
    #[serde(default = "package_lock_schema")]
    pub schema: String,
    pub name: String,
    pub version: String,
    pub digest: String,
}

fn package_lock_schema() -> String {
    PACKAGE_LOCK_SCHEMA.to_owned()
}

impl Default for PackageLock {
    fn default() -> Self {
        Self {
            schema: PACKAGE_LOCK_SCHEMA.into(),
            name: String::new(),
            version: String::new(),
            digest: String::new(),
        }
    }
}

impl PackageLock {
    pub fn id(&self) -> PackageId {
        let mut payload =
            Vec::with_capacity(self.name.len() + self.version.len() + self.digest.len() + 2);
        payload.extend_from_slice(self.name.as_bytes());
        payload.push(0xff);
        payload.extend_from_slice(self.version.as_bytes());
        payload.push(0xff);
        payload.extend_from_slice(self.digest.as_bytes());
        PackageId::of(&payload)
    }

    /// Artifact row used when path compile injects this lock into a source manifest.
    pub fn module_artifact(&self, relative_path: impl Into<String>) -> ManifestArtifact {
        ManifestArtifact {
            path: relative_path.into(),
            digest: self.digest.clone(),
            kind: "package_module".into(),
            effective: self.version.clone(),
            weight: SourceWeight::Explanatory,
        }
    }
}

/// `source_root/packages`. Callers must not use this for mill paste / `check_source`.
pub fn packages_root(source_root: &Path) -> PathBuf {
    source_root.join(PACKAGES_DIR)
}

/// First QName segment of an import, lowercased (`Std.Core` -> `std`).
pub fn package_name_from_import(import_name: &str) -> String {
    let base = import_name.split('<').next().unwrap_or(import_name).trim();
    let first = base.split('.').next().unwrap_or(base).trim();
    first.to_ascii_lowercase()
}

/// Package directory names are a single path segment; never `..` or separators.
pub fn is_safe_package_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// True when `artifact.path` is under `packages/`.
pub fn artifact_path_is_package(path: &str) -> bool {
    let p = normalize_rel_path(path);
    p == PACKAGES_DIR || p.starts_with("packages/")
}

/// Package directory segment of a `packages/<name>/...` artifact path.
pub fn package_dir_from_artifact_path(path: &str) -> Option<String> {
    let p = normalize_rel_path(path);
    let rest = p.strip_prefix("packages/")?;
    let dir = rest.split('/').next()?.trim();
    if !is_safe_package_name(dir) {
        None
    } else {
        Some(dir.to_ascii_lowercase())
    }
}

/// `packages/std/core.fr` satisfies `import Std.Core` when keys match.
pub fn package_path_matches_import(path: &str, import_name: &str) -> bool {
    let import_key = normalize_key(import_name);
    if import_key.is_empty() {
        return false;
    }
    if let Some(key) = package_module_key(path)
        && normalize_key(&key) == import_key
    {
        return true;
    }
    if let Some(dir) = package_dir_from_artifact_path(path) {
        return normalize_key(&dir) == import_key;
    }
    false
}

fn package_module_key(path: &str) -> Option<String> {
    let p = normalize_rel_path(path);
    let rest = p.strip_prefix("packages/")?;
    let mut segs: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() {
        return None;
    }
    let file = segs.pop()?;
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(file);
    if segs.is_empty() {
        return Some(stem.to_ascii_lowercase());
    }
    let mut key = segs.join(".");
    key.push('.');
    key.push_str(stem);
    Some(key)
}

fn normalize_rel_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn normalize_key(s: &str) -> String {
    let base = s.split('<').next().unwrap_or(s);
    base.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn package_name_from_import_uses_first_segment() {
        assert_eq!(package_name_from_import("Std.Core"), "std");
        assert_eq!(package_name_from_import("Std"), "std");
        assert_eq!(package_name_from_import("Id<NaturalPerson>"), "id");
        assert_eq!(package_name_from_import("MA.TrustLaw.Fixture"), "ma");
    }

    #[test]
    fn packages_std_core_matches_std_core_import() {
        assert!(package_path_matches_import(
            "packages/std/core.fr",
            "Std.Core"
        ));
        assert!(package_path_matches_import(
            "packages/logic/true.fr",
            "Logic.True"
        ));
        assert!(package_path_matches_import(
            "packages\\std\\core.fr",
            "Std.Core"
        ));
        assert!(!package_path_matches_import(
            "packages/std/core.fr",
            "Other.Law"
        ));
        assert!(!package_path_matches_import("Other.Law", "Std.Core"));
        assert!(artifact_path_is_package("packages/std/core.fr"));
        assert!(!artifact_path_is_package("sources/unrelated.txt"));
    }

    #[test]
    fn package_lock_id_is_stable_and_includes_digest() {
        let a = PackageLock {
            schema: PACKAGE_LOCK_SCHEMA.into(),
            name: "std".into(),
            version: "0.1.0".into(),
            digest: "aa".into(),
        };
        let b = a.clone();
        let mut c = a.clone();
        c.digest = "bb".into();
        assert_eq!(a.id(), b.id());
        assert_ne!(a.id(), c.id());
        assert_eq!(a.id().hex().len(), 32);
    }

    #[test]
    fn package_lock_json_round_trip() {
        let lock = PackageLock {
            schema: PACKAGE_LOCK_SCHEMA.into(),
            name: "std".into(),
            version: "0.1.0".into(),
            digest: "ab".repeat(32),
        };
        let json = serde_json::to_value(&lock).unwrap();
        assert_eq!(json["schema"], PACKAGE_LOCK_SCHEMA);
        assert_eq!(json["name"], "std");
        assert_eq!(json["version"], "0.1.0");
        let back: PackageLock = serde_json::from_value(json).unwrap();
        assert_eq!(back, lock);
    }

    #[test]
    fn packages_root_is_source_root_join_packages() {
        assert_eq!(
            packages_root(Path::new("/mod")),
            Path::new("/mod").join("packages")
        );
        assert!(is_safe_package_name("std"));
        assert!(!is_safe_package_name(".."));
        assert!(!is_safe_package_name("std/core"));
    }
}
