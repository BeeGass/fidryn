//! The check of a workspace.
//!
//! The check reads each module file by itself: its bytes, which a generated
//! workspace's lock records; its edition; and, for a module of edition 2,
//! the grammar, lowering and the checks of one module. Of each module it
//! lowers it keeps a [`Summary`]. The checks across modules then work from
//! the manifest and the summaries alone, and read no module again.

use super::discover::walk;
use super::{LOCK_FILE, MANIFEST_FILE};
use fidryn_check::e2::{Lowered, check_module, lower};
use fidryn_core::{Diagnostic, DiagnosticCode, Severity, Span};
use fidryn_provision::layout::module_path;
use fidryn_provision::workspace::{Lock, Manifest, WorkspaceKind};
use fidryn_provision::{Address, Hash32, Module};
use fidryn_syntax::e2::{E2_MAX_SOURCE_BYTES, parse};
use fidryn_syntax::edition::{Edition, edition_of};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// How [`check_workspace`] runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckOptions {
    /// The number of worker threads. The check reads the modules one at a
    /// time, on the thread that calls it, and does not read this.
    pub jobs: usize,
    /// The directory of the cache. The check keeps no cache and does not read
    /// this.
    pub cache_dir: Option<PathBuf>,
    /// The version of the running toolchain, which the manifest's
    /// `toolchain` is compared with.
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
/// nothing else of it is read. Otherwise every `.fr` file outside the
/// directories whose name begins with a dot is read and checked by itself,
/// and then the checks across the modules run:
///
/// - the manifest and the lock are well formed, a generated workspace has a
///   release, a lock and no dependencies, and a curated workspace neither a
///   release nor a lock; every module stands at the path of its address,
///   below the prefix; in a generated workspace every module file is listed
///   in the lock with the BLAKE3 of its bytes, and every listed file is
///   present; no file is of the first edition, and every entry is a file or
///   a directory (`E730`);
/// - the `within` of a unit or a container names a container of the
///   workspace that lists it as a member, and every member of a container is
///   a module whose `within` is that container (`E720`);
/// - the manifest's `toolchain` is the running one (`W760`).
pub fn check_workspace(root: &Path, options: &CheckOptions) -> WorkspaceReport {
    let mut diagnostics = Vec::new();
    let mut modules = 0;
    if let Some(manifest) = read_manifest(root, &mut diagnostics) {
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
        modules = found.modules.len();
        let mut summaries = Vec::new();
        let mut reported = BTreeSet::new();
        for rel in &found.modules {
            let checked = check_file(root, rel, lock.as_ref());
            diagnostics.extend(checked.diagnostics);
            match checked.summary {
                Some(summary) => summaries.push(summary),
                None => {
                    reported.insert(rel.clone());
                }
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
                    reported.insert(rel.clone());
                }
            }
        }
        diagnostics.extend(across(&manifest, &options.toolchain, &summaries, &reported));
    }
    diagnostics.sort_by(|a, b| order(a).cmp(&order(b)));
    WorkspaceReport {
        modules,
        from_cache: 0,
        diagnostics,
    }
}

/// The key the diagnostics of a report are sorted by: the file, the
/// position, then the code and the message.
fn order(diagnostic: &Diagnostic) -> (Option<&str>, Option<(u32, u32)>, &'static str, &str) {
    (
        diagnostic.source_file.as_deref(),
        diagnostic.primary_span.map(|span| (span.start, span.end)),
        diagnostic.code.as_str(),
        &diagnostic.message,
    )
}

/// A diagnostic about the file `rel` as a whole, which has no span.
fn of_file(code: DiagnosticCode, rel: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, message).with_source(rel)
}

/// A diagnostic about the text at `span` of the file `rel`.
fn at(code: DiagnosticCode, rel: &str, span: Span, message: String) -> Diagnostic {
    Diagnostic::new(code, message)
        .with_span(span)
        .with_source(rel)
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

/// What the check of one module file gives: its diagnostics, and its
/// summary when it lowers.
struct Checked {
    diagnostics: Vec<Diagnostic>,
    summary: Option<Summary>,
}

/// The checks of the module file `rel` by itself: its bytes against the lock
/// of a generated workspace, its edition, and the checks of one module.
fn check_file(root: &Path, rel: &str, lock: Option<&Lock>) -> Checked {
    let (digest, head) = match read_module(&root.join(rel)) {
        Ok(read) => read,
        Err(error) => {
            return Checked {
                diagnostics: vec![of_file(
                    DiagnosticCode::E730,
                    rel,
                    format!("this file cannot be read: {error}"),
                )],
                summary: None,
            };
        }
    };
    let mut diagnostics = Vec::new();
    if let Some(lock) = lock {
        match lock.modules.get(rel) {
            None => diagnostics.push(of_file(
                DiagnosticCode::E730,
                rel,
                format!("this module file is not listed in the lock, `{LOCK_FILE}`"),
            )),
            Some(recorded) if *recorded != digest => diagnostics.push(of_file(
                DiagnosticCode::E730,
                rel,
                format!(
                    "this file has changed since the lock was written: the BLAKE3 of its bytes \
                     is {digest}, and the lock records {recorded}"
                ),
            )),
            Some(_) => {}
        }
    }
    let summary = match edition_of(utf8_prefix(&head)) {
        Err(diagnostic) => {
            diagnostics.push(diagnostic.with_source(rel));
            None
        }
        Ok(Edition::One) => {
            diagnostics.push(of_file(
                DiagnosticCode::E730,
                rel,
                "this file is of the first edition, and every `.fr` file of a workspace is a \
                 module of edition 2",
            ));
            None
        }
        Ok(Edition::Two) => match source_text(head) {
            Err(error) => {
                diagnostics.push(of_file(
                    DiagnosticCode::E100,
                    rel,
                    format!("this file is not UTF-8: {error}"),
                ));
                None
            }
            Ok(source) => match lower(&parse(&source)) {
                Err(found) => {
                    diagnostics.extend(found.into_iter().map(|found| found.with_source(rel)));
                    None
                }
                Ok(lowered) => {
                    let found = check_module(&lowered);
                    diagnostics.extend(found.into_iter().map(|found| found.with_source(rel)));
                    Some(Summary::of(rel, &lowered))
                }
            },
        },
    };
    Checked {
        diagnostics,
        summary,
    }
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

/// The shape of a module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Unit,
    Container,
    Curated,
}

impl Shape {
    fn described(self) -> &'static str {
        match self {
            Shape::Unit => "a unit",
            Shape::Container => "a container",
            Shape::Curated => "a curated module",
        }
    }
}

/// What the checks across modules read of one module: the path of its
/// file, its address, its shape, the container it is within, its members
/// and its aliases, with the spans they are reported at.
#[derive(Clone, Debug)]
struct Summary {
    path: String,
    address: Address,
    address_span: Span,
    shape: Shape,
    within: Option<(Address, Span)>,
    members: Vec<(Address, Span)>,
    aliases: Vec<Address>,
}

impl Summary {
    fn of(path: &str, lowered: &Lowered) -> Summary {
        let (address, shape, within, members, aliases): (_, _, _, &[Address], &[Address]) =
            match &lowered.module {
                Module::Unit(unit) => (
                    &unit.address,
                    Shape::Unit,
                    unit.within.as_ref(),
                    &[],
                    &unit.aliases,
                ),
                Module::Container(container) => (
                    &container.address,
                    Shape::Container,
                    container.within.as_ref(),
                    &container.members,
                    &container.aliases,
                ),
                Module::Curated(curated) => (&curated.address, Shape::Curated, None, &[], &[]),
            };
        Summary {
            path: path.to_owned(),
            address: address.clone(),
            address_span: lowered.address_span(),
            shape,
            within: within.cloned().zip(lowered.within_span()),
            members: members
                .iter()
                .cloned()
                .zip(lowered.member_spans().iter().copied())
                .collect(),
            aliases: aliases.to_vec(),
        }
    }

    /// Whether the module stands at the path of its address.
    fn placed(&self) -> bool {
        module_path(&self.address) == self.path
    }
}

/// The modules of a workspace by their addresses and by their aliases. Of
/// two modules with one address, the one that stands at its path is kept,
/// else the first by path.
struct Modules<'a> {
    by_address: BTreeMap<&'a Address, &'a Summary>,
    by_alias: BTreeMap<&'a Address, &'a Summary>,
    /// The paths of the module files that take no part in these checks,
    /// already reported: those that did not lower and those that are
    /// missing.
    reported: &'a BTreeSet<String>,
}

impl<'a> Modules<'a> {
    fn new(summaries: &'a [Summary], reported: &'a BTreeSet<String>) -> Modules<'a> {
        let mut by_address: BTreeMap<&Address, &Summary> = BTreeMap::new();
        let mut by_alias = BTreeMap::new();
        for summary in summaries {
            by_address
                .entry(&summary.address)
                .and_modify(|kept| {
                    if !kept.placed() && summary.placed() {
                        *kept = summary;
                    }
                })
                .or_insert(summary);
            for alias in &summary.aliases {
                by_alias.entry(alias).or_insert(summary);
            }
        }
        Modules {
            by_address,
            by_alias,
            reported,
        }
    }

    /// The module with the address `address`. `Err` holds why there is
    /// none, or `None` when the file of that address is already reported.
    fn get(&self, address: &Address) -> Result<&'a Summary, Option<String>> {
        if let Some(found) = self.by_address.get(address) {
            return Ok(found);
        }
        if self.reported.contains(&module_path(address)) {
            return Err(None);
        }
        Err(Some(match self.by_alias.get(address) {
            Some(module) => format!(
                "is an alias of the module `{}`, and a module is named by its address",
                module.address
            ),
            None => "is no module of the workspace".to_owned(),
        }))
    }
}

/// Whether `address` lies below `prefix` and is not `prefix` itself.
fn below(address: &Address, prefix: &Address) -> bool {
    address != prefix && address.is_within(prefix)
}

/// The checks across the modules of a workspace, from its manifest, the
/// running toolchain's version and the summary of each module alone.
/// `reported` holds the paths of the module files that have no summary,
/// whose own diagnostics say why.
fn across(
    manifest: &Manifest,
    toolchain: &str,
    summaries: &[Summary],
    reported: &BTreeSet<String>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut of_manifest = |code: DiagnosticCode, message: String| {
        out.push(of_file(code, MANIFEST_FILE, message));
    };
    if manifest.toolchain != toolchain {
        of_manifest(
            DiagnosticCode::W760,
            format!(
                "the workspace was last checked with the toolchain `{}`, and this is `{toolchain}`",
                manifest.toolchain
            ),
        );
    }
    match (manifest.kind, &manifest.release) {
        (WorkspaceKind::Generated, None) => of_manifest(
            DiagnosticCode::E730,
            "a generated workspace has a `release` in its manifest, and this one has none"
                .to_owned(),
        ),
        (WorkspaceKind::Curated, Some(release)) => of_manifest(
            DiagnosticCode::E730,
            format!(
                "a curated workspace has no `release` in its manifest, and this one has \
                 `{release}`"
            ),
        ),
        _ => {}
    }
    if manifest.kind == WorkspaceKind::Generated && !manifest.dependencies.is_empty() {
        of_manifest(
            DiagnosticCode::E730,
            format!(
                "a generated workspace has no dependencies, and this one has {}",
                manifest.dependencies.len()
            ),
        );
    }

    let modules = Modules::new(summaries, reported);
    for summary in summaries {
        let file = summary.path.as_str();
        if !below(&summary.address, &manifest.prefix) {
            out.push(at(
                DiagnosticCode::E730,
                file,
                summary.address_span,
                format!(
                    "the address `{}` is not below the prefix `{}` of the workspace",
                    summary.address, manifest.prefix
                ),
            ));
        }
        if !summary.placed() {
            out.push(at(
                DiagnosticCode::E730,
                file,
                summary.address_span,
                format!(
                    "the module `{}` stands at `{file}`, and the path of its address is `{}`",
                    summary.address,
                    module_path(&summary.address)
                ),
            ));
        }
        if let Some((within, span)) = &summary.within {
            let problem = match modules.get(within) {
                Err(why) => why.map(|why| format!("`within` names `{within}`, which {why}")),
                Ok(container) if container.shape != Shape::Container => Some(format!(
                    "`within` names `{within}`, which is {}, not a container",
                    container.shape.described()
                )),
                Ok(container)
                    if !container
                        .members
                        .iter()
                        .any(|(member, _)| *member == summary.address) =>
                {
                    Some(format!(
                        "`within` names the container `{within}`, which does not list `{}` as \
                         a member",
                        summary.address
                    ))
                }
                Ok(_) => None,
            };
            if let Some(message) = problem {
                out.push(at(DiagnosticCode::E720, file, *span, message));
            }
        }
        for (member, span) in &summary.members {
            let problem = match modules.get(member) {
                Err(why) => why.map(|why| format!("the member `{member}` {why}")),
                Ok(module) => match &module.within {
                    None => Some(format!(
                        "the member `{member}` has no `within`, and a member is within the \
                         container that lists it"
                    )),
                    Some((within, _)) if *within != summary.address => Some(format!(
                        "the member `{member}` is within `{within}`, not within this container"
                    )),
                    Some(_) => None,
                },
            };
            if let Some(message) = problem {
                out.push(at(DiagnosticCode::E720, file, *span, message));
            }
        }
    }
    out
}
