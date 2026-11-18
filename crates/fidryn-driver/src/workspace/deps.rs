//! The dependencies of a curated workspace.
//!
//! Each entry of a curated manifest's `dependencies` names a generated
//! workspace by its directory, relative to the manifest's, and pins it to a
//! release and to the BLAKE3 of the bytes of its lock. [`open`] takes the
//! entries in their order: an entry whose path is absolute, or whose name
//! an entry before it has, is refused (`E730`); then the directory must hold
//! a generated workspace with a lock (`E740`), the digest of the lock and
//! its release must be those of the entry (`E740`), and its prefix must
//! neither be, contain nor lie within the prefix of the workspace or of a
//! dependency before it that passed these checks (`E730`). An entry that
//! fails one check is not checked further. The module files of a dependency
//! that passes are read by the rules of any workspace, and go through phase
//! one through the cache of the depending workspace.
//! [`settle`] then checks each one by itself, as `fidryn check` of its
//! directory does: when that finds an error, one `E740` says how many, and
//! none of them is reported; its warnings are not reported either.
//!
//! Every diagnostic about a dependency names the manifest of the depending
//! workspace, and none has a span. A dependency that fails gives phase two
//! no interface, only its prefix when the manifest of its directory was
//! read; the manifest of an entry refused for its path or its name is not
//! read. Nothing is written inside a dependency's directory: a check whose
//! cache's directory is that directory, lies inside it or holds it keeps no
//! cache ([`cache_shared`]).

use super::cache::Cache;
use super::check::{Cached, Files, PhaseOne, lock_disagrees, read_files, settle as settle_files};
use super::discover::{JSON_LIMIT, Unread, read_regular};
use super::index::{DependencyInterfaces, own_checks};
use super::{LOCK_FILE, MANIFEST_FILE};
use fidryn_core::{Diagnostic, DiagnosticCode, Severity};
use fidryn_provision::workspace::{Dependency, Lock, Manifest, WorkspaceKind};
use fidryn_provision::{Address, Hash32};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// A dependency that passed the checks of its entry, its manifest and its
/// lock, with its files read.
struct Opened {
    entry: Dependency,
    manifest: Manifest,
    lock: Lock,
    files: Files,
}

/// What [`open`] gave of one entry of the manifest.
enum Opening {
    Opened(Box<Opened>),
    /// The entry failed; this is the prefix of the manifest of its
    /// directory when that was read, and `None` when it was not.
    Failed(Option<Address>),
}

/// The dependencies of a workspace, opened, and the diagnostics about them
/// so far.
pub(super) struct Dependencies {
    /// One for each entry of the manifest, in their order.
    entries: Vec<Opening>,
    diagnostics: Vec<Diagnostic>,
}

impl Dependencies {
    /// Phase one of every module file of every dependency opened, with what
    /// the cache gave of it.
    pub(super) fn phase_one(&self) -> impl Iterator<Item = &(PhaseOne, Cached)> {
        self.entries.iter().flat_map(|entry| match entry {
            Opening::Opened(opened) => opened.files.read.as_slice(),
            Opening::Failed(_) => &[],
        })
    }
}

/// A diagnostic about a dependency, which names the manifest.
fn about(code: DiagnosticCode, message: String) -> Diagnostic {
    Diagnostic::new(code, message).with_source(MANIFEST_FILE)
}

/// Opens each dependency of `manifest`, the manifest of the workspace at
/// `root`, when it is a curated workspace: a generated workspace has none
/// to open, and is reported for any it names. The module files of each
/// dependency that passes the checks of its entry, its manifest and its
/// lock are read, on `jobs` threads and through `cache`, which nothing is
/// written to here.
pub(super) fn open(
    root: &Path,
    manifest: &Manifest,
    jobs: usize,
    cache: Option<&Cache>,
) -> Dependencies {
    let mut entries = Vec::new();
    let mut diagnostics = Vec::new();
    if manifest.kind != WorkspaceKind::Curated {
        return Dependencies {
            entries,
            diagnostics,
        };
    }
    // The path of the first entry of each name.
    let mut names: BTreeMap<&str, &str> = BTreeMap::new();
    // The prefix of the workspace, and of each dependency opened so far,
    // with whose it is.
    let mut prefixes = vec![(manifest.prefix.clone(), "this workspace".to_owned())];
    for entry in &manifest.dependencies {
        // An entry refused for its path still has its name.
        let first = names.get(entry.name.as_str()).copied();
        if first.is_none() {
            names.insert(&entry.name, &entry.path);
        }
        if is_absolute(&entry.path) {
            diagnostics.push(about(
                DiagnosticCode::E730,
                format!(
                    "the dependency `{}` has the path `{}`, which is absolute, and the path of \
                     a dependency is relative to the directory of the manifest",
                    entry.name, entry.path
                ),
            ));
            entries.push(Opening::Failed(None));
            continue;
        }
        if let Some(first) = first {
            diagnostics.push(about(
                DiagnosticCode::E730,
                format!(
                    "the dependency `{}` is listed twice, at `{first}` and at `{}`, and each \
                     dependency has a name of its own",
                    entry.name, entry.path
                ),
            ));
            entries.push(Opening::Failed(None));
            continue;
        }
        let dir = root.join(&entry.path);
        let (found, lock, digest) = match generated(&dir, entry) {
            Ok(found) => found,
            Err((message, prefix)) => {
                diagnostics.push(about(DiagnosticCode::E740, message));
                entries.push(Opening::Failed(prefix));
                continue;
            }
        };
        if digest != entry.lock_digest || lock.release != entry.release {
            diagnostics.push(about(
                DiagnosticCode::E740,
                format!(
                    "the dependency `{}` at `{}` is pinned to the release `{}` and the lock \
                     digest `{}`, and its lock is of the release `{}` and has the digest \
                     `{digest}`",
                    entry.name, entry.path, entry.release, entry.lock_digest, lock.release
                ),
            ));
            entries.push(Opening::Failed(Some(found.prefix)));
            continue;
        }
        if let Some(relation) = overlap(&found.prefix, &prefixes) {
            diagnostics.push(about(
                DiagnosticCode::E730,
                format!(
                    "the dependency `{}` has the prefix `{}`, which {relation}; no prefix of a \
                     workspace and of its dependencies is, contains or lies within another",
                    entry.name, found.prefix
                ),
            ));
            entries.push(Opening::Failed(Some(found.prefix)));
            continue;
        }
        prefixes.push((
            found.prefix.clone(),
            format!("the dependency `{}`", entry.name),
        ));
        let files = read_files(&dir, jobs, cache);
        entries.push(Opening::Opened(Box::new(Opened {
            entry: entry.clone(),
            manifest: found,
            lock,
            files,
        })));
    }
    Dependencies {
        entries,
        diagnostics,
    }
}

/// Whether `path`, the path of a dependency, is absolute: it begins with
/// `/`, or is absolute on this host.
fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || Path::new(path).is_absolute()
}

/// How `prefix` overlaps the first of `prefixes` it overlaps, each with
/// whose it is, as the end of a sentence about it; `None` when it overlaps
/// none.
fn overlap(prefix: &Address, prefixes: &[(Address, String)]) -> Option<String> {
    prefixes.iter().find_map(|(other, whose)| {
        if prefix == other {
            Some(format!("is also the prefix of {whose}"))
        } else if other.is_within(prefix) {
            Some(format!("contains the prefix `{other}` of {whose}"))
        } else if prefix.is_within(other) {
            Some(format!("lies within the prefix `{other}` of {whose}"))
        } else {
            None
        }
    })
}

/// What a dependency failed of, as the message of its `E740`, and the
/// prefix of its manifest when that could be read.
type Refusal = (String, Option<Address>);

/// The manifest and the lock of the generated workspace in `dir`, which
/// `entry` names, and the BLAKE3 of the bytes of the lock; or why there is
/// none.
fn generated(dir: &Path, entry: &Dependency) -> Result<(Manifest, Lock, Hash32), Refusal> {
    let named = format!(
        "the dependency `{}` names the directory `{}`",
        entry.name, entry.path
    );
    match fs::metadata(dir) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => return Err((format!("{named}, and there is no directory there"), None)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Err((format!("{named}, and there is no directory there"), None));
        }
        Err(error) => return Err((format!("{named}, which cannot be read: {error}"), None)),
    }
    let manifest = match read_regular(&dir.join(MANIFEST_FILE), JSON_LIMIT) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|error| error.to_string())
            .and_then(|text| Manifest::from_json(&text))
            .map_err(|error| {
                (
                    format!(
                        "{named}, whose `{MANIFEST_FILE}` is no manifest of a workspace: {error}"
                    ),
                    None,
                )
            })?,
        Err(Unread::Missing) => {
            return Err((
                format!("{named}, which holds no `{MANIFEST_FILE}` and so is no workspace"),
                None,
            ));
        }
        Err(unread) => {
            return Err((
                format!("{named}, whose `{MANIFEST_FILE}` {}", unread.clause()),
                None,
            ));
        }
    };
    let prefix = Some(manifest.prefix.clone());
    if manifest.kind != WorkspaceKind::Generated {
        return Err((
            format!(
                "{named}, which holds a curated workspace, and a dependency is a generated \
                 workspace"
            ),
            prefix,
        ));
    }
    let bytes = match read_regular(&dir.join(LOCK_FILE), JSON_LIMIT) {
        Ok(bytes) => bytes,
        Err(Unread::Missing) => {
            return Err((
                format!("{named}, which holds a generated workspace without a lock, `{LOCK_FILE}`"),
                prefix,
            ));
        }
        Err(unread) => {
            return Err((
                format!("{named}, whose `{LOCK_FILE}` {}", unread.clause()),
                prefix,
            ));
        }
    };
    let digest = Hash32(*blake3::hash(&bytes).as_bytes());
    let lock = String::from_utf8(bytes)
        .map_err(|error| error.to_string())
        .and_then(|text| Lock::from_json(&text))
        .map_err(|error| {
            (
                format!("{named}, whose `{LOCK_FILE}` is no lock: {error}"),
                prefix,
            )
        })?;
    Ok((manifest, lock, digest))
}

/// The dependencies as phase two takes them, in the order of the
/// manifest's entries, with each opened dependency checked by itself, as
/// `fidryn check` of its directory checks it with the toolchain of version
/// `toolchain`. Every diagnostic about them is added to `diagnostics`: one
/// that has an error of its own is one `E740`, which says how many, and is
/// given no interface.
pub(super) fn settle(
    dependencies: Dependencies,
    toolchain: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<DependencyInterfaces> {
    diagnostics.extend(dependencies.diagnostics);
    let mut out = Vec::new();
    for entry in dependencies.entries {
        let opened = match entry {
            Opening::Failed(prefix) => {
                out.push(DependencyInterfaces::Failed { prefix });
                continue;
            }
            Opening::Opened(opened) => *opened,
        };
        let Opened {
            entry,
            manifest,
            lock,
            files,
        } = opened;
        let mut own = lock_disagrees(&manifest, &lock);
        let (interfaces, unread) = settle_files(files, Some(&lock), &manifest.prefix, &mut own);
        own.extend(own_checks(&interfaces, &manifest, toolchain, &unread));
        let errors = own
            .iter()
            .filter(|diagnostic| diagnostic.code.severity() == Severity::Error)
            .count();
        if errors == 0 {
            out.push(DependencyInterfaces::Loaded {
                name: entry.name,
                release: lock.release,
                prefix: manifest.prefix,
                interfaces,
            });
            continue;
        }
        let counted = if errors == 1 {
            "1 error".to_owned()
        } else {
            format!("{errors} errors")
        };
        diagnostics.push(about(
            DiagnosticCode::E740,
            format!(
                "the dependency `{}` at `{}` has {counted} of its own, which `fidryn check` of \
                 its directory shows",
                entry.name, entry.path
            ),
        ));
        out.push(DependencyInterfaces::Failed {
            prefix: Some(manifest.prefix),
        });
    }
    out
}

/// Why the cache's directory `cache` must not be used in the check of the
/// workspace at `root`, whose manifest is `manifest`: it shares a directory
/// with one of the dependencies the manifest names, where nothing is
/// written. It does when it is the directory of the dependency, lies inside
/// it, or holds it, so that an entry written below it could land inside the
/// dependency. `None` when it shares a directory with none. The paths are
/// compared with every link and `..` resolved, as far as each exists
/// ([`resolved`]); the directory of every entry is compared, whatever its
/// checks find.
pub(super) fn cache_shared(root: &Path, manifest: &Manifest, cache: &Path) -> Option<String> {
    let (existing, at) = resolved(cache)?;
    manifest.dependencies.iter().find_map(|entry| {
        let (_, dir) = resolved(&root.join(&entry.path))?;
        let named = format!("the dependency `{}`, `{}`", entry.name, entry.path);
        let relation = if at == dir {
            format!("the directory is the directory of {named}")
        } else if at.starts_with(&dir) || existing.starts_with(&dir) {
            format!("the directory lies inside the directory of {named}")
        } else if dir.starts_with(&at) {
            format!("the directory of {named}, lies inside the directory")
        } else {
            return None;
        };
        Some(format!(
            "{}: {relation}, and nothing is written inside a dependency",
            cache.display()
        ))
    })
}

/// The longest part of `path` that exists, made canonical, and `path` with
/// that part so resolved and the rest of it taken as written, a `..` there
/// removing the segment before it; `None` when no part of it exists, which
/// cannot be of an absolute path. A directory made at `path` is made below
/// the first, so that both say where a write would be.
fn resolved(path: &Path) -> Option<(PathBuf, PathBuf)> {
    let absolute = std::path::absolute(path).ok()?;
    let (existing, rest) = absolute.ancestors().find_map(|part| {
        let existing = fs::canonicalize(part).ok()?;
        let rest = absolute.strip_prefix(part).ok()?;
        Some((existing, rest))
    })?;
    let mut at = existing.clone();
    for component in rest.components() {
        match component {
            Component::ParentDir => {
                at.pop();
            }
            Component::Normal(name) => at.push(name),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
        }
    }
    Some((existing, at))
}
