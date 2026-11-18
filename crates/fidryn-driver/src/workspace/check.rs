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
//! reference and every citation its outcome from the interfaces alone
//! ([`check_interfaces`](super::check_interfaces)); it reads no module
//! again. The module files of the dependencies of a curated workspace go
//! through both phases in the same way ([`deps`](super::deps)).

use super::cache::Cache;
use super::deps;
use super::discover::walk;
use super::index::{
    Unresolved, UnresolvedCitation, across, no_citations, no_references, order, span,
};
use super::{LOCK_FILE, MANIFEST_FILE};
use fidryn_check::e2::{Lowered, check_module, lower};
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
    /// The identity of the running executable, which the command gives as
    /// the BLAKE3 of its file. It is part of the key of every entry of the
    /// cache, so that an entry that another build of the same version wrote
    /// is never read; the check uses it for nothing else.
    pub executable: String,
}

/// What [`check_workspace`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceReport {
    /// The number of module files read: every `.fr` file of the workspace.
    /// The module files of its dependencies are counted nowhere.
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
    /// The number of citations of the curated modules with each outcome, by
    /// the outcome's name; every name is present.
    pub citations: BTreeMap<String, u64>,
    /// Every citation whose outcome is not `exact`, sorted by the address of
    /// its module, then by its target and its name.
    pub unresolved_citations: Vec<UnresolvedCitation>,
    /// Why the cache could not be written, the first time an entry could
    /// not be; `None` when every entry was, and when there is no cache. The
    /// rest of the report does not depend on it, and neither the summary
    /// line nor the check report holds it.
    pub cache_failure: Option<String>,
    /// Why the cache of `CheckOptions::cache_dir` was not used at all: its
    /// directory is the directory of a dependency, lies inside it or holds
    /// it, and nothing is written inside a dependency. The check then reads
    /// and writes no entry, and the rest of the report is that of a check
    /// without a cache; neither the summary line nor the check report holds
    /// this.
    pub cache_unused: Option<String>,
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
/// - every import of a curated module names a module of the workspace or
///   of a dependency (`E720`), and its names resolve (`E200`);
/// - no two modules declare one address (`E700`);
/// - the manifest's `toolchain` is the running one (`W760`);
/// - each dependency of a curated workspace has a relative path and a name
///   of its own, and a prefix that neither is, contains nor lies within
///   that of the workspace or of a dependency before it (`E730`); it is a
///   generated workspace with a lock, of the digest and the release that
///   the manifest records, and has no error of its own (`E740`), as its
///   own check finds, which reads its module files through the cache of
///   this one and writes nothing into its directory. Each of these
///   diagnostics names the manifest.
///
/// It also gives every reference of the workspace, and every citation of
/// its curated modules, its outcome, from the modules of the workspace and
/// of its dependencies. Phase one, and the outcomes in phase two, run on
/// `options.jobs` threads, and the report is the same for every number of
/// them. With a cache, `options.cache_dir`, the result of phase one of a
/// file whose bytes were checked before is read from it, and the report is
/// the same as without one, but for the count of the modules read from it.
/// A cache whose directory is the directory of a dependency the manifest
/// names, lies inside it or holds it is not used at all, since nothing is
/// written inside a dependency: the report is that of a check without a
/// cache, and says why in [`WorkspaceReport::cache_unused`].
pub fn check_workspace(root: &Path, options: &CheckOptions) -> WorkspaceReport {
    checked(root, options).report
}

/// What [`checked`] gives: the report of [`check_workspace`], and the
/// interfaces of the modules that stand at the paths of their addresses,
/// which phase two read, in the order of their paths.
pub(super) struct Checked {
    pub(super) report: WorkspaceReport,
    pub(super) interfaces: Vec<Interface>,
}

/// The check of the workspace whose directory is `root`, as
/// [`check_workspace`] runs it, with the interfaces it read.
pub(super) fn checked(root: &Path, options: &CheckOptions) -> Checked {
    let mut diagnostics = Vec::new();
    let Some(manifest) = read_manifest(root, &mut diagnostics) else {
        let report = WorkspaceReport {
            modules: 0,
            from_cache: 0,
            diagnostics,
            references: no_references(),
            outside_by_body: BTreeMap::new(),
            unresolved: Vec::new(),
            citations: no_citations(),
            unresolved_citations: Vec::new(),
            cache_failure: None,
            cache_unused: None,
        };
        return Checked {
            report,
            interfaces: Vec::new(),
        };
    };
    let lock = read_lock(root, &manifest, &mut diagnostics);
    let jobs = options.jobs.max(1);

    // Phase one: each module file of the workspace and of its dependencies
    // by itself, or its result from the cache; then the entries of the
    // files the cache did not hold. A cache that shares a directory with a
    // dependency is not used.
    let (cache, cache_unused) = cache_of(root, &manifest, options);
    let files = read_files(root, jobs, cache.as_ref());
    let opened = deps::open(root, &manifest, jobs, cache.as_ref());
    let modules = files.modules.len();
    let from_cache = files
        .read
        .iter()
        .filter(|(_, cached)| matches!(cached, Cached::Read))
        .count();
    if let Some(cache) = &cache {
        write_missed(cache, files.read.iter().chain(opened.phase_one()), jobs);
    }

    // Phase two: the files against the lock, where each module stands, each
    // dependency by itself, and then the interfaces alone.
    let (interfaces, unread) = settle(files, lock.as_ref(), &manifest.prefix, &mut diagnostics);
    let dependencies = deps::settle(opened, &options.toolchain, &mut diagnostics);
    let phase_two = across(
        &interfaces,
        &dependencies,
        &manifest,
        &options.toolchain,
        &unread,
        jobs,
    );
    diagnostics.extend(phase_two.diagnostics);
    diagnostics.sort_by(|a, b| order(a).cmp(&order(b)));
    let report = WorkspaceReport {
        modules,
        from_cache,
        diagnostics,
        references: phase_two.references,
        outside_by_body: phase_two.outside_by_body,
        unresolved: phase_two.unresolved,
        citations: phase_two.citations,
        unresolved_citations: phase_two.unresolved_citations,
        cache_failure: cache.and_then(Cache::failure),
        cache_unused,
    };
    Checked { report, interfaces }
}

/// The cache of the check of the workspace at `root`, whose manifest is
/// `manifest`, that `options` names, and why it is not used when its
/// directory shares a directory with a dependency.
fn cache_of(
    root: &Path,
    manifest: &Manifest,
    options: &CheckOptions,
) -> (Option<Cache>, Option<String>) {
    let cache_unused = options
        .cache_dir
        .as_deref()
        .and_then(|dir| deps::cache_shared(root, manifest, dir));
    let cache = options
        .cache_dir
        .as_deref()
        .filter(|_| cache_unused.is_none())
        .map(|dir| Cache::new(dir, &options.toolchain, &options.executable));
    (cache, cache_unused)
}

/// Phase one alone of the workspace at `root`, whose manifest is
/// `manifest`, through the cache of `options` as [`check_workspace`] keeps
/// it, and then where each module stands: the interfaces of the modules
/// that stand at the paths of their addresses, in the order of their paths,
/// and the diagnostics of the files, each naming its file by its path
/// relative to `root`, sorted. Nothing of a lock, of the manifest, of a
/// dependency or across the modules is checked.
pub(super) fn modules_of(
    root: &Path,
    manifest: &Manifest,
    options: &CheckOptions,
) -> (Vec<Interface>, Vec<Diagnostic>) {
    let (cache, _) = cache_of(root, manifest, options);
    let jobs = options.jobs.max(1);
    let files = read_files(root, jobs, cache.as_ref());
    if let Some(cache) = &cache {
        write_missed(cache, &files.read, jobs);
    }
    let mut diagnostics = Vec::new();
    let (interfaces, _) = settle(files, None, &manifest.prefix, &mut diagnostics);
    diagnostics.sort_by(|a, b| order(a).cmp(&order(b)));
    (interfaces, diagnostics)
}

/// A diagnostic about the file `rel` as a whole, which has no span.
pub(super) fn of_file(code: DiagnosticCode, rel: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message).with_source(rel)
}

/// The manifest of the workspace at `root`; `None`, with the reason in
/// `diagnostics`, when it is missing or cannot be read.
pub(super) fn read_manifest(root: &Path, diagnostics: &mut Vec<Diagnostic>) -> Option<Manifest> {
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
pub(super) fn read_lock(
    root: &Path,
    manifest: &Manifest,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Lock> {
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
                diagnostics.extend(lock_disagrees(manifest, &lock));
                Some(lock)
            }
        },
    }
}

/// Where the lock `lock` of a generated workspace disagrees with its
/// manifest, `manifest`: in its prefix, and in its release.
pub(super) fn lock_disagrees(manifest: &Manifest, lock: &Lock) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if lock.prefix != manifest.prefix {
        out.push(of_file(
            DiagnosticCode::E730,
            LOCK_FILE,
            format!(
                "the lock is of the prefix `{}`, and the manifest of `{}`",
                lock.prefix, manifest.prefix
            ),
        ));
    }
    if let Some(release) = &manifest.release
        && *release != lock.release
    {
        out.push(of_file(
            DiagnosticCode::E730,
            LOCK_FILE,
            format!(
                "the lock is of the release `{}`, and the manifest of `{release}`",
                lock.release
            ),
        ));
    }
    out
}

/// The files of a workspace, read: every module file, with phase one of
/// it and what the cache gave of it, and a diagnostic for each entry that
/// is neither a file nor a directory, or that cannot be read.
pub(super) struct Files {
    /// The path of every module file, in the byte order of the paths.
    pub(super) modules: Vec<String>,
    /// Phase one of each file of `modules`, in their order.
    pub(super) read: Vec<(PhaseOne, Cached)>,
    pub(super) diagnostics: Vec<Diagnostic>,
}

/// The files of the workspace at `root`, by the rules of a workspace: no
/// entry whose name begins with a dot is read, and no link is followed.
/// Phase one runs on `jobs` threads, through `cache`, which nothing is
/// written to here.
pub(super) fn read_files(root: &Path, jobs: usize, cache: Option<&Cache>) -> Files {
    let found = walk(root);
    let mut diagnostics = Vec::new();
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
    let read = phase_one(root, &found.modules, jobs, cache);
    Files {
        modules: found.modules,
        read,
        diagnostics,
    }
}

/// The first part of phase two of the workspace of the prefix `prefix`
/// whose files are `files`, and which has the lock `lock` when it is a
/// generated workspace whose lock could be read: the diagnostics of the
/// files, each file against the lock, and where each module stands, added
/// to `diagnostics`. It gives the interfaces of the modules that stand at
/// the paths of their addresses, and the paths of the module files that
/// gave none.
pub(super) fn settle(
    files: Files,
    lock: Option<&Lock>,
    prefix: &Address,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<Interface>, BTreeSet<String>) {
    diagnostics.extend(files.diagnostics);
    let mut interfaces = Vec::new();
    let mut unread = BTreeSet::new();
    for (rel, (module, _)) in files.modules.iter().zip(files.read) {
        diagnostics.extend(
            module
                .diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.with_source(rel.as_str())),
        );
        if let (Some(lock), Some(digest)) = (lock, module.digest) {
            diagnostics.extend(against_lock(rel, digest, lock));
        }
        let Some(interface) = module.interface else {
            unread.insert(rel.clone());
            continue;
        };
        diagnostics.extend(stands(rel, &interface, prefix));
        let path = module_path(&interface.address);
        if path == *rel {
            interfaces.push(interface);
        } else {
            unread.insert(path);
        }
    }
    if let Some(lock) = lock {
        for rel in lock.modules.keys() {
            if files.modules.binary_search(rel).is_err() {
                diagnostics.push(of_file(
                    DiagnosticCode::E730,
                    rel,
                    "the lock lists this module file, and it is missing",
                ));
                unread.insert(rel.clone());
            }
        }
    }
    (interfaces, unread)
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

/// What the cache gave, or is to be given, of one file's phase one.
#[derive(Clone, Copy)]
pub(super) enum Cached {
    /// The result was read from the cache.
    Read,
    /// The cache had no valid entry of the key; the result is to be written
    /// under it once every file has been read.
    Missed(Hash32),
    /// There is no cache, or the file is not kept in it.
    Not,
}

/// Phase one of every file of `modules`, relative to `root`, on `jobs`
/// threads, in the order of `modules`, each with what `cache` gave of it.
/// Nothing is written to the cache here.
fn phase_one(
    root: &Path,
    modules: &[String],
    jobs: usize,
    cache: Option<&Cache>,
) -> Vec<(PhaseOne, Cached)> {
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

/// Phase one of the module file at `path`, and what `cache` gave of it: the
/// entry of the file's bytes when the cache holds one that is valid, and
/// otherwise [`check_file`], whose result is to be written to the cache. A
/// file larger than the largest source is not kept in the cache, since only
/// its first bytes are held.
fn read_one(path: &Path, cache: Option<&Cache>) -> (PhaseOne, Cached) {
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
            return (unread, Cached::Not);
        }
    };
    let key = cache
        .filter(|_| head.len() <= E2_MAX_SOURCE_BYTES)
        .map(|cache| (cache, cache.key(&head)));
    match key {
        Some((cache, key)) => match cache.read(&key, &digest) {
            Some(kept) => (kept, Cached::Read),
            None => (check_file(digest, head), Cached::Missed(key)),
        },
        None => (check_file(digest, head), Cached::Not),
    }
}

/// Writes to `cache` the result of each file of `read` whose entry it
/// missed, once for each key, on `jobs` threads. It runs after every file,
/// of the workspace and of its dependencies, has been read, so that no read
/// of a check sees an entry that the check wrote, and the count of the
/// modules read from the cache is the same for any number of workers.
fn write_missed<'a>(
    cache: &Cache,
    read: impl IntoIterator<Item = &'a (PhaseOne, Cached)>,
    jobs: usize,
) {
    let mut keys = BTreeSet::new();
    let missed: Vec<(Hash32, &PhaseOne)> = read
        .into_iter()
        .filter_map(|(result, cached)| match cached {
            Cached::Missed(key) if keys.insert(*key) => Some((*key, result)),
            _ => None,
        })
        .collect();
    let next = AtomicUsize::new(0);
    let work = || {
        while let Some((key, result)) = missed.get(next.fetch_add(1, Ordering::Relaxed)) {
            cache.write(key, result);
        }
    };
    let workers = jobs.clamp(1, missed.len().max(1));
    if workers == 1 {
        work();
    } else {
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(work);
            }
        });
    }
}

/// Phase one of the module file whose bytes have the BLAKE3 `digest` and
/// begin with `head`: its edition, and the checks of one module.
fn check_file(digest: Hash32, head: Vec<u8>) -> PhaseOne {
    let mut diagnostics = Vec::new();
    let interface = match lower_file(head) {
        Err(found) => {
            diagnostics.extend(found);
            None
        }
        Ok(lowered) => {
            diagnostics.extend(check_module(&lowered));
            Some(interface_of(&lowered.module, &lowered.decl_spans()))
        }
    };
    PhaseOne {
        digest: Some(digest),
        diagnostics,
        interface,
    }
}

/// The module of the file whose bytes begin with `head`, lowered into the
/// model: its edition, its text, and the grammar and lowering of edition 2.
/// No diagnostic names the file.
pub(super) fn lower_file(head: Vec<u8>) -> Result<Lowered, Vec<Diagnostic>> {
    match edition_of(utf8_prefix(&head)) {
        Err(diagnostic) => Err(vec![diagnostic]),
        Ok(Edition::One) => Err(vec![Diagnostic::new(
            DiagnosticCode::E730,
            "this file is of the first edition, and every `.fr` file of a workspace is a \
             module of edition 2",
        )]),
        Ok(Edition::Two) => match source_text(head) {
            Err(error) => Err(vec![Diagnostic::new(
                DiagnosticCode::E100,
                format!("this file is not UTF-8: {error}"),
            )]),
            Ok(source) => lower(&parse(&source)),
        },
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
pub(super) fn read_module(path: &Path) -> io::Result<(Hash32, Vec<u8>)> {
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
