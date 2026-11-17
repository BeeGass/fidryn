//! The check of a workspace, in two phases.
//!
//! Phase one reads each module file by itself, on several threads: the
//! BLAKE3 of its bytes, its edition and, for a module of edition 2, the
//! grammar, lowering, the checks of one module and the module's
//! [`Interface`]. Its result depends on the file's bytes alone, so it is
//! kept in the [cache](super::cache) under a key of those bytes, and taken
//! from there when the bytes come again. Phase two compares each file's
//! digest with the lock of a generated workspace, checks where each module
//! stands, and then runs the checks across the modules and gives every
//! reference its outcome from the interfaces alone
//! ([`check_interfaces`](super::check_interfaces)); it reads no module
//! again.

use super::cache::Cache;
use super::discover::walk;
use super::index::{Unresolved, across, no_references, order, span};
use super::{LOCK_FILE, MANIFEST_FILE};
use fidryn_check::e2::{check_module, lower};
use fidryn_core::{Diagnostic, DiagnosticCode, Severity};
use fidryn_provision::layout::module_path;
use fidryn_provision::workspace::{Lock, Manifest, WorkspaceKind};
use fidryn_provision::{Address, Hash32, Interface, Layer, interface_of};
use fidryn_syntax::e2::{E2_MAX_SOURCE_BYTES, parse};
use fidryn_syntax::edition::{Edition, edition_of};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// How [`check_workspace`] runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckOptions {
    /// The number of worker threads of each phase, at least 1; 0 is taken as
    /// 1. The result is the same for every number.
    pub jobs: usize,
    /// The directory of the cache, which the check reads the result of
    /// phase one from and writes it to; `None` for no cache, when every
    /// module file is read and checked and nothing is written.
    pub cache_dir: Option<PathBuf>,
    /// The version of the running toolchain, which the manifest's
    /// `toolchain` is compared with and which is part of the key of every
    /// entry of the cache.
    pub toolchain: String,
}

/// What [`check_workspace`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceReport {
    /// The number of module files read: every `.fr` file of the workspace.
    pub modules: usize,
    /// How many of them were read from the cache.
    pub from_cache: usize,
    /// Every diagnostic, each naming its file by its path relative to the
    /// workspace's directory, sorted by that path and then by position; a
    /// diagnostic about a file as a whole has no span and comes first.
    pub diagnostics: Vec<Diagnostic>,
    /// The number of references of each layer with each outcome, by the
    /// outcome's name; every layer and every name is present.
    pub references: BTreeMap<Layer, BTreeMap<String, u64>>,
    /// The number of references of outcome `outside` by the body of law
    /// they name.
    pub outside_by_body: BTreeMap<String, u64>,
    /// Every reference whose outcome is not `exact`, sorted by the address
    /// it stands in, then by its target.
    pub unresolved: Vec<Unresolved>,
    /// Why the cache could not be written, the first time an entry could
    /// not be; `None` when every entry was, and when there is no cache. The
    /// rest of the report does not depend on it, and neither the summary
    /// line nor the check report holds it.
    pub cache_failure: Option<String>,
}

impl WorkspaceReport {
    /// The number of diagnostics that are errors.
    pub fn errors(&self) -> usize {
        self.count(Severity::Error)
    }

    /// The number of diagnostics that are warnings.
    pub fn warnings(&self) -> usize {
        self.count(Severity::Warning)
    }

    /// The summary line: `modules: N; from cache: C; errors: E; warnings: W`.
    pub fn summary(&self) -> String {
        format!(
            "modules: {}; from cache: {}; errors: {}; warnings: {}",
            self.modules,
            self.from_cache,
            self.errors(),
            self.warnings()
        )
    }

    fn count(&self, severity: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.severity() == severity)
            .count()
    }
}

/// The check of the workspace whose directory is `root`.
///
/// A directory with no readable manifest is reported with `E730` and
/// nothing else of it is read. Otherwise every `.fr` file is read and
/// checked by itself, and then the checks across the modules run. No entry
/// whose name begins with a dot is read, nor anything below one:
///
/// - the manifest and the lock are well formed, a generated workspace has a
///   release, a lock and no dependencies, and a curated workspace neither a
///   release nor a lock; every module stands at the path of its address,
///   below the prefix; in a generated workspace every module file is listed
///   in the lock with the BLAKE3 of its bytes, and every listed file is
///   present; no file is of the first edition, and every entry is a file or
///   a directory, none a symbolic link (`E730`);
/// - the `within` of a unit or a container names a container of the
///   workspace that lists it as a member, and every member of a container is
///   a module whose `within` is that container (`E720`);
/// - the manifest's `toolchain` is the running one (`W760`).
///
/// It also gives every reference of the workspace its outcome. Phase one,
/// and the outcomes of the references in phase two, run on `options.jobs`
/// threads, and the report is the same for every number of them. With a
/// cache, `options.cache_dir`, the result of phase one of a file whose bytes
/// were checked before is read from it, and the report is the same as
/// without one, but for the count of the modules read from it.
pub fn check_workspace(root: &Path, options: &CheckOptions) -> WorkspaceReport {
    let mut diagnostics = Vec::new();
    let Some(manifest) = read_manifest(root, &mut diagnostics) else {
        return WorkspaceReport {
            modules: 0,
            from_cache: 0,
            diagnostics,
            references: no_references(),
            outside_by_body: BTreeMap::new(),
            unresolved: Vec::new(),
            cache_failure: None,
        };
    };
    let lock = read_lock(root, &manifest, &mut diagnostics);
    let found = walk(root);
    for (rel, why) in &found.unreadable {
        diagnostics.push(of_file(
            DiagnosticCode::E730,
            rel,
            format!("this entry cannot be read: {why}"),
        ));
    }
    for rel in &found.special {
        diagnostics.push(of_file(
            DiagnosticCode::E730,
            rel,
            "this entry is neither a file nor a directory; a workspace check follows no \
             symbolic link",
        ));
    }
    let jobs = options.jobs.max(1);

    // Phase one: each module file by itself, or its result from the cache.
    let cache = options
        .cache_dir
        .as_deref()
        .map(|dir| Cache::new(dir, &options.toolchain));
    let read = phase_one(root, &found.modules, jobs, cache.as_ref());
    let from_cache = read.iter().filter(|(_, cached)| *cached).count();

    // Phase two: the files against the lock, where each module stands, and
    // then the interfaces alone.
    let mut interfaces = Vec::new();
    let mut unread = BTreeSet::new();
    for (rel, (module, _)) in found.modules.iter().zip(read) {
        diagnostics.extend(
            module
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.with_source(rel.as_str())),
        );
        if let (Some(lock), Some(digest)) = (&lock, module.digest) {
            diagnostics.extend(against_lock(rel, digest, lock));
        }
        let Some(interface) = module.interface else {
            unread.insert(rel.clone());
            continue;
        };
        diagnostics.extend(stands(rel, &interface, &manifest.prefix));
        let path = module_path(&interface.address);
        if path == *rel {
            interfaces.push(interface);
        } else {
            unread.insert(path);
        }
    }
    if let Some(lock) = &lock {
        for rel in lock.modules.keys() {
            if found.modules.binary_search(rel).is_err() {
                diagnostics.push(of_file(
                    DiagnosticCode::E730,
                    rel,
                    "the lock lists this module file, and it is missing",
                ));
                unread.insert(rel.clone());
            }
        }
    }
    let phase_two = across(
        &interfaces,
        &[],
        &manifest,
        &options.toolchain,
        &unread,
        jobs,
    );
    diagnostics.extend(phase_two.diagnostics);
    diagnostics.sort_by(|a, b| order(a).cmp(&order(b)));
    WorkspaceReport {
        modules: found.modules.len(),
        from_cache,
        diagnostics,
        references: phase_two.references,
        outside_by_body: phase_two.outside_by_body,
        unresolved: phase_two.unresolved,
        cache_failure: cache.and_then(Cache::failure),
    }
}

/// A diagnostic about the file `rel` as a whole, which has no span.
fn of_file(code: DiagnosticCode, rel: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message).with_source(rel)
}

/// The manifest of the workspace at `root`; `None`, with the reason in
/// `diagnostics`, when it is missing or cannot be read.
fn read_manifest(root: &Path, diagnostics: &mut Vec<Diagnostic>) -> Option<Manifest> {
    let text = match fs::read_to_string(root.join(MANIFEST_FILE)) {
        Ok(text) => text,
        Err(error) => {
            let message = if error.kind() == io::ErrorKind::NotFound {
                format!("no `{MANIFEST_FILE}` was found in the directory, so it is no workspace")
            } else {
                format!("this file cannot be read: {error}")
            };
            diagnostics.push(of_file(DiagnosticCode::E730, MANIFEST_FILE, message));
            return None;
        }
    };
    Manifest::from_json(&text)
        .map_err(|error| {
            diagnostics.push(of_file(
                DiagnosticCode::E730,
                MANIFEST_FILE,
                format!("this file is no manifest of a workspace: {error}"),
            ));
        })
        .ok()
}

/// The lock of a generated workspace, when it has one that can be read. A
/// generated workspace without one, and a curated workspace with one, are
/// reported in `diagnostics`.
fn read_lock(root: &Path, manifest: &Manifest, diagnostics: &mut Vec<Diagnostic>) -> Option<Lock> {
    let text = match fs::read_to_string(root.join(LOCK_FILE)) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    };
    let mut report = |message: String| {
        diagnostics.push(of_file(DiagnosticCode::E730, LOCK_FILE, message));
    };
    match (manifest.kind, text) {
        (WorkspaceKind::Curated, Ok(None)) => None,
        (WorkspaceKind::Curated, _) => {
            report(format!(
                "a curated workspace has no lock, and this one has `{LOCK_FILE}`"
            ));
            None
        }
        (WorkspaceKind::Generated, Ok(None)) => {
            report(format!(
                "a generated workspace has a lock, `{LOCK_FILE}`, and this one has none"
            ));
            None
        }
        (WorkspaceKind::Generated, Err(error)) => {
            report(format!("this file cannot be read: {error}"));
            None
        }
        (WorkspaceKind::Generated, Ok(Some(text))) => match Lock::from_json(&text) {
            Err(error) => {
                report(format!("this file is no lock of a workspace: {error}"));
                None
            }
            Ok(lock) => {
                if lock.prefix != manifest.prefix {
                    report(format!(
                        "the lock is of the prefix `{}`, and the manifest of `{}`",
                        lock.prefix, manifest.prefix
                    ));
                }
                if let Some(release) = &manifest.release
                    && *release != lock.release
                {
                    report(format!(
                        "the lock is of the release `{}`, and the manifest of `{release}`",
                        lock.release
                    ));
                }
                Some(lock)
            }
        },
    }
}

/// What phase one gives of one module file, and what an entry of the cache
/// holds. It depends on the file's bytes alone: no diagnostic names the file
/// yet, and nothing of the path, the lock, the manifest or another module is
/// in it.
pub(super) struct PhaseOne {
    /// The BLAKE3 of the file's bytes; `None` when it cannot be read.
    pub(super) digest: Option<Hash32>,
    pub(super) diagnostics: Vec<Diagnostic>,
    /// The module's interface, when it lowers.
    pub(super) interface: Option<Interface>,
}

/// Phase one of every file of `modules`, relative to `root`, on `jobs`
/// threads, in the order of `modules`, each with whether it was read from
/// `cache`.
fn phase_one(
    root: &Path,
    modules: &[String],
    jobs: usize,
    cache: Option<&Cache>,
) -> Vec<(PhaseOne, bool)> {
    let next = AtomicUsize::new(0);
    let work = || {
        let mut done = Vec::new();
        loop {
            let at = next.fetch_add(1, Ordering::Relaxed);
            let Some(rel) = modules.get(at) else {
                break;
            };
            done.push((at, read_one(&root.join(rel), cache)));
        }
        done
    };
    let workers = jobs.clamp(1, modules.len().max(1));
    let mut done = if workers == 1 {
        work()
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers).map(|_| scope.spawn(work)).collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("a worker of the check panicked"))
                .collect()
        })
    };
    done.sort_by_key(|(at, _)| *at);
    done.into_iter().map(|(_, read)| read).collect()
}

/// Phase one of the module file at `path`, and whether it was read from
/// `cache`: the entry of the file's bytes when the cache holds one that is
/// valid, and otherwise [`check_file`], whose result is then written to the
/// cache. A file larger than the largest source is not kept in the cache,
/// since only its first bytes are held.
fn read_one(path: &Path, cache: Option<&Cache>) -> (PhaseOne, bool) {
    let (digest, head) = match read_module(path) {
        Ok(read) => read,
        Err(error) => {
            let unread = PhaseOne {
                digest: None,
                diagnostics: vec![Diagnostic::new(
                    DiagnosticCode::E730,
                    format!("this file cannot be read: {error}"),
                )],
                interface: None,
            };
            return (unread, false);
        }
    };
    let keyed = cache
        .filter(|_| head.len() <= E2_MAX_SOURCE_BYTES)
        .map(|cache| (cache, cache.key(&head)));
    if let Some((cache, key)) = &keyed
        && let Some(kept) = cache.read(key, &digest)
    {
        return (kept, true);
    }
    let checked = check_file(digest, head);
    if let Some((cache, key)) = keyed {
        cache.write(&key, &checked);
    }
    (checked, false)
}

/// Phase one of the module file whose bytes have the BLAKE3 `digest` and
/// begin with `head`: its edition, and the checks of one module.
fn check_file(digest: Hash32, head: Vec<u8>) -> PhaseOne {
    let mut diagnostics = Vec::new();
    let interface = match edition_of(utf8_prefix(&head)) {
        Err(diagnostic) => {
            diagnostics.push(diagnostic);
            None
        }
        Ok(Edition::One) => {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::E730,
                "this file is of the first edition, and every `.fr` file of a workspace is a \
                 module of edition 2",
            ));
            None
        }
        Ok(Edition::Two) => match source_text(head) {
            Err(error) => {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::E100,
                    format!("this file is not UTF-8: {error}"),
                ));
                None
            }
            Ok(source) => match lower(&parse(&source)) {
                Err(found) => {
                    diagnostics.extend(found);
                    None
                }
                Ok(lowered) => {
                    diagnostics.extend(check_module(&lowered));
                    Some(interface_of(&lowered.module, &lowered.decl_spans()))
                }
            },
        },
    };
    PhaseOne {
        digest: Some(digest),
        diagnostics,
        interface,
    }
}

/// The comparison of the file `rel`, whose bytes have the BLAKE3 `digest`,
/// with the lock of a generated workspace.
fn against_lock(rel: &str, digest: Hash32, lock: &Lock) -> Option<Diagnostic> {
    match lock.modules.get(rel) {
        None => Some(of_file(
            DiagnosticCode::E730,
            rel,
            format!("this module file is not listed in the lock, `{LOCK_FILE}`"),
        )),
        Some(recorded) if *recorded != digest => Some(of_file(
            DiagnosticCode::E730,
            rel,
            format!(
                "this file has changed since the lock was written: the BLAKE3 of its bytes is \
                 {digest}, and the lock records {recorded}"
            ),
        )),
        Some(_) => None,
    }
}

/// Where the module of the file `rel` stands: below the prefix, and at the
/// path of its address.
fn stands(rel: &str, interface: &Interface, prefix: &Address) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut report = |message: String| {
        out.push(
            Diagnostic::new(DiagnosticCode::E730, message)
                .with_span(span(interface.address_range))
                .with_source(rel),
        );
    };
    let address = &interface.address;
    if address == prefix || !address.is_within(prefix) {
        report(format!(
            "the address `{address}` is not below the prefix `{prefix}` of the workspace"
        ));
    }
    let path = module_path(address);
    if path != rel {
        report(format!(
            "the module `{address}` stands at `{rel}`, and the path of its address is `{path}`"
        ));
    }
    out
}

/// The BLAKE3 of the bytes of the file at `path`, and its first bytes: all
/// of them, or one more than the largest source of edition 2, so that what
/// is held does not grow with a larger file.
fn read_module(path: &Path) -> io::Result<(Hash32, Vec<u8>)> {
    let mut file = fs::File::open(path)?;
    let mut head = Vec::new();
    file.by_ref()
        .take(E2_MAX_SOURCE_BYTES as u64 + 1)
        .read_to_end(&mut head)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(&head);
    if head.len() > E2_MAX_SOURCE_BYTES {
        io::copy(&mut file, &mut hasher)?;
    }
    Ok((Hash32(*hasher.finalize().as_bytes()), head))
}

/// The longest prefix of `bytes` that is UTF-8.
fn utf8_prefix(bytes: &[u8]) -> &str {
    match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => std::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap_or_default(),
    }
}

/// The text of a module file read as `head`. A file larger than the largest
/// source is read in part, and the parser refuses it for its size.
fn source_text(head: Vec<u8>) -> Result<String, std::string::FromUtf8Error> {
    if head.len() > E2_MAX_SOURCE_BYTES {
        Ok(String::from_utf8_lossy(&head).into_owned())
    } else {
        String::from_utf8(head)
    }
}
