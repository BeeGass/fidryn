//! The change set between two generated workspaces of one prefix.
//!
//! [`change_set`] reads each workspace as the check of a workspace reads
//! it, through phase one and the cache, and stops at the first that has an
//! error. It then compares the two from their interfaces: a module whose
//! layer hashes, and for a unit the addresses of its provisions, are equal
//! on both sides is not read again. Notes, containers and modules on one
//! side only come from the interfaces alone. Only a unit whose operative or
//! editorial hash or provisions differ, a container whose editorial hash
//! differs, and a unit on one side whose top provision could have moved, of
//! a kind and status that a provision on the other side has, are opened
//! and compared ([`fidryn_provision::changes`]). The bindings of curated
//! workspaces are then judged against the newer side.

use super::check::{CheckOptions, checked, lower_file, modules_of, read_manifest, read_module};
use super::{LOCK_FILE, MANIFEST_FILE};
use fidryn_check::e2::names::resolve;
use fidryn_core::{Diagnostic, DiagnosticCode, Severity};
use fidryn_provision::changes::{
    ChangeSet, NoteAt, OneSided, Release, StaleBinding, diff_containers, diff_units, note_changes,
    pair_moved, unnumbered_hash,
};
use fidryn_provision::interface::Shape;
use fidryn_provision::layout::module_path;
use fidryn_provision::workspace::{Manifest, WorkspaceKind};
use fidryn_provision::{Address, AddressIndex, Hash32, Interface, Layer, Module, staleness};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The change set from the generated workspace at `old` to the one at
/// `new`, of one prefix, and the bindings of the curated workspaces at
/// `bindings` that are stale against `new`.
///
/// Each of the two is checked as [`check_workspace`](super::check_workspace)
/// checks it, with `options`; `Err` holds the diagnostics of the first that
/// has an error, each naming its file below the directory given, and no
/// change set is made. A relative `options.cache_dir` is taken inside the
/// directory of each workspace read, so that each keeps its own cache; an
/// absolute one is the cache of all of them. `Err` holds one `E730` when a
/// directory holds no workspace, when either of the two is a curated
/// workspace or a workspace of `bindings` a generated one, and when the
/// two have different prefixes.
///
/// A binding of a workspace of `bindings` is judged against `new` by the
/// rule of [`staleness`], whatever release that workspace is pinned to and
/// whether or not its pin is satisfied: its modules are read by phase one
/// alone, and an error there ends the comparison as above. A binding is
/// left out when its name does not resolve, or when it binds an address
/// below no prefix but `new`'s. A directory that `bindings` names twice,
/// once its path is resolved, is read once.
///
/// Every list is sorted ([`ChangeSet::sort`]), and the result is the same
/// for every number of `options.jobs`. The manifest and the lock of each
/// workspace are read once.
pub fn change_set(
    old: &Path,
    new: &Path,
    bindings: &[PathBuf],
    options: &CheckOptions,
) -> Result<ChangeSet, Vec<Diagnostic>> {
    compare(old, new, bindings, options).map(|compared| compared.set)
}

/// What [`compare`] gives: the change set, and what the checks of the
/// workspaces it read say of their cache, which the change set does not
/// depend on.
pub struct Compared {
    pub set: ChangeSet,
    /// Why the cache was not used, as
    /// [`WorkspaceReport::cache_unused`](super::WorkspaceReport::cache_unused)
    /// says it, of the first workspace read whose check says so.
    pub cache_unused: Option<String>,
    /// Why the cache could not be written, as
    /// [`WorkspaceReport::cache_failure`](super::WorkspaceReport::cache_failure)
    /// says it, of the first workspace read whose check says so.
    pub cache_failure: Option<String>,
    /// How many module files the comparison opened to compare their
    /// modules, which the tests read.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) opened: usize,
}

/// [`change_set`], with what the checks say of their cache.
pub fn compare(
    old: &Path,
    new: &Path,
    bindings: &[PathBuf],
    options: &CheckOptions,
) -> Result<Compared, Vec<Diagnostic>> {
    let old_manifest = manifest_of(old, WorkspaceKind::Generated)?;
    let new_manifest = manifest_of(new, WorkspaceKind::Generated)?;
    if old_manifest.prefix != new_manifest.prefix {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::E730,
            format!(
                "the workspace `{}` has the prefix `{}` and the workspace `{}` the prefix `{}`, \
                 and a change set is made between two workspaces of one prefix",
                old.display(),
                old_manifest.prefix,
                new.display(),
                new_manifest.prefix
            ),
        )]);
    }
    let old_side = side(old, &old_manifest, options)?;
    let new_side = side(new, &new_manifest, options)?;
    let mut cache = CacheNotes::default();
    cache.add(&old_side.cache);
    cache.add(&new_side.cache);
    let jobs = options.jobs.max(1);
    let opened = AtomicUsize::new(0);
    let mut set = ChangeSet::empty(
        old_manifest.prefix.clone(),
        old_side.release.clone(),
        new_side.release.clone(),
    );
    let modules = [Modules::of(&old_side), Modules::of(&new_side)];
    let [old_modules, new_modules] = &modules;

    // Notes, from the interfaces alone.
    let (old_notes, new_notes) = (
        notes_of(&old_side.interfaces),
        notes_of(&new_side.interfaces),
    );
    set.enacted_notes = note_changes(&old_notes, &new_notes, Layer::Enacted);
    set.editorial.notes = note_changes(&old_notes, &new_notes, Layer::Editorial);

    // Containers: those on one side, those whose hash or aliases differ,
    // and the credits and footnotes of those whose editorial hash differs.
    let mut containers_to_open = Vec::new();
    for (address, old) in &old_modules.containers {
        let Some(new) = new_modules.containers.get(address) else {
            set.containers.removed.push((*address).clone());
            continue;
        };
        let (before, after) = (layers(old), layers(new));
        if before.operative != after.operative || aliases(old) != aliases(new) {
            set.containers.changed.push((*address).clone());
        }
        if before.editorial != after.editorial {
            containers_to_open.push(*address);
        }
    }
    for address in new_modules.containers.keys() {
        if !old_modules.containers.contains_key(address) {
            set.containers.added.push((*address).clone());
        }
    }
    let read = each(&containers_to_open, jobs, |address| {
        let old = open(&old_side.dir, address, &opened)?;
        let new = open(&new_side.dir, address, &opened)?;
        match (old, new) {
            (Module::Container(old), Module::Container(new)) => Ok(diff_containers(&old, &new)),
            _ => Err(not_the_shape(address, Shape::Container)),
        }
    });
    for (address, diff) in containers_to_open.iter().zip(read) {
        let diff = diff?;
        if diff.credit_changed {
            set.editorial.credits_changed.push((*address).clone());
        }
        if diff.footnotes_changed {
            set.editorial.footnotes_changed.push((*address).clone());
        }
    }

    // Units on both sides whose hashes or provisions differ.
    let mut units_to_open = Vec::new();
    for (address, old) in &old_modules.units {
        if let Some(new) = new_modules.units.get(address) {
            let (before, after) = (layers(old), layers(new));
            if before.operative != after.operative
                || before.editorial != after.editorial
                || !old
                    .provisions
                    .iter()
                    .map(|p| &p.at)
                    .eq(new.provisions.iter().map(|p| &p.at))
            {
                units_to_open.push(*address);
            }
        }
    }
    let read = each(&units_to_open, jobs, |address| {
        let old = open(&old_side.dir, address, &opened)?;
        let new = open(&new_side.dir, address, &opened)?;
        match (old, new) {
            (Module::Unit(old), Module::Unit(new)) => Ok(diff_units(&old, &new)),
            _ => Err(not_the_shape(address, Shape::Unit)),
        }
    });
    let mut removed: Vec<Candidate> = Vec::new();
    let mut added: Vec<Candidate> = Vec::new();
    for (address, diff) in units_to_open.iter().zip(read) {
        let diff = diff?;
        set.operative.amended_text.extend(diff.amended_text);
        set.operative
            .amended_structure
            .extend(diff.amended_structure);
        removed.extend(diff.removed.into_iter().map(Candidate::found));
        added.extend(diff.added.into_iter().map(Candidate::found));
        if diff.matter.credit_changed {
            set.editorial.credits_changed.push((*address).clone());
        }
        if diff.matter.footnotes_changed {
            set.editorial.footnotes_changed.push((*address).clone());
        }
    }

    // Units on one side: each is its top provision, which is opened when a
    // provision on the other side has its kind and its status, so that the
    // two can be compared with their numbers left out.
    removed.extend(one_sided(old_modules, new_modules));
    added.extend(one_sided(new_modules, old_modules));
    let removed_kinds: BTreeSet<(String, Option<String>)> =
        removed.iter().map(Candidate::kind).collect();
    let added_kinds: BTreeSet<(String, Option<String>)> =
        added.iter().map(Candidate::kind).collect();
    for (candidates, others, dir) in [
        (&mut removed, &added_kinds, &old_side.dir),
        (&mut added, &removed_kinds, &new_side.dir),
    ] {
        let wanted: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| {
                candidate.unnumbered.is_none() && others.contains(&candidate.kind())
            })
            .map(|(place, _)| place)
            .collect();
        let read = each(&wanted, jobs, |place| {
            let address = &candidates[*place].address;
            match open(dir, address, &opened)? {
                Module::Unit(unit) => Ok(unnumbered_hash(&unit.root)),
                _ => Err(not_the_shape(address, Shape::Unit)),
            }
        });
        for (place, hash) in wanted.into_iter().zip(read) {
            candidates[place].unnumbered = Some(hash?);
        }
    }
    let paired = pair_moved(
        removed.into_iter().map(Candidate::entry).collect(),
        added.into_iter().map(Candidate::entry).collect(),
    );
    set.operative.removed = paired.removed;
    set.operative.added = paired.added;
    set.operative.moved = paired.moved;

    // The bindings of the curated workspaces, against the new side; a
    // directory named twice is read once.
    if !bindings.is_empty() {
        let prefix = &new_manifest.prefix;
        let index = AddressIndex::build(&new_side.interfaces, std::slice::from_ref(prefix));
        let mut read = BTreeSet::new();
        for dir in bindings {
            if !read.insert(fs::canonicalize(dir).unwrap_or_else(|_| dir.clone())) {
                continue;
            }
            let (stale, notes) = stale_in(dir, &new_side.interfaces, &index, prefix, options)?;
            set.stale_bindings.extend(stale);
            cache.add(&notes);
        }
    }

    set.sort();
    Ok(Compared {
        set,
        cache_unused: cache.unused,
        cache_failure: cache.failure,
        opened: opened.into_inner(),
    })
}

/// What the check of a workspace says of its cache.
#[derive(Default)]
struct CacheNotes {
    unused: Option<String>,
    failure: Option<String>,
}

impl CacheNotes {
    /// Keeps of `other` what this does not hold yet.
    fn add(&mut self, other: &CacheNotes) {
        if self.unused.is_none() {
            self.unused.clone_from(&other.unused);
        }
        if self.failure.is_none() {
            self.failure.clone_from(&other.failure);
        }
    }
}

/// A workspace that the comparison reads: its directory, its release and
/// the digest of its lock, the interfaces of its modules, and what its
/// check says of its cache.
struct Side {
    dir: PathBuf,
    release: Release,
    interfaces: Vec<Interface>,
    cache: CacheNotes,
}

/// `options` for the workspace in `dir`: a relative directory of the cache
/// is taken inside `dir`.
fn in_workspace(dir: &Path, options: &CheckOptions) -> CheckOptions {
    CheckOptions {
        cache_dir: options.cache_dir.as_ref().map(|cache| {
            if cache.is_relative() {
                dir.join(cache)
            } else {
                cache.clone()
            }
        }),
        ..options.clone()
    }
}

/// `diagnostics`, each naming its file below `dir`.
fn below(dir: &Path, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|mut diagnostic| {
            diagnostic.source_file = diagnostic
                .source_file
                .map(|file| dir.join(file).display().to_string());
            diagnostic
        })
        .collect()
}

/// The manifest of the workspace in `dir`, which is of the kind `kind`.
fn manifest_of(dir: &Path, kind: WorkspaceKind) -> Result<Manifest, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let Some(manifest) = read_manifest(dir, &mut diagnostics) else {
        return Err(below(dir, diagnostics));
    };
    if manifest.kind == kind {
        return Ok(manifest);
    }
    let (is, wanted) = match manifest.kind {
        WorkspaceKind::Curated => (
            "a curated workspace",
            "a change set is made between two generated workspaces",
        ),
        WorkspaceKind::Generated => (
            "a generated workspace",
            "the bindings judged are those of a curated workspace",
        ),
    };
    let diagnostic = Diagnostic::new(
        DiagnosticCode::E730,
        format!("this is the manifest of {is}, and {wanted}"),
    )
    .with_source(MANIFEST_FILE);
    Err(below(dir, vec![diagnostic]))
}

/// The generated workspace in `dir`, whose manifest is `manifest`, checked
/// as `fidryn check` checks it; its diagnostics when it has an error.
fn side(dir: &Path, manifest: &Manifest, options: &CheckOptions) -> Result<Side, Vec<Diagnostic>> {
    let checked = checked(dir, manifest, &in_workspace(dir, options));
    if checked.report.errors() > 0 {
        return Err(below(dir, checked.report.diagnostics));
    }
    // A generated workspace that checks without an error has a lock.
    let Some((lock, lock_digest)) = checked.lock else {
        return Err(below(
            dir,
            vec![
                Diagnostic::new(
                    DiagnosticCode::E730,
                    "a generated workspace has a lock, and this one has none that can be read",
                )
                .with_source(LOCK_FILE),
            ],
        ));
    };
    Ok(Side {
        dir: dir.to_owned(),
        release: Release {
            release: lock.release,
            lock_digest,
        },
        interfaces: checked.interfaces,
        cache: CacheNotes {
            unused: checked.report.cache_unused,
            failure: checked.report.cache_failure,
        },
    })
}

/// The units and the containers of a workspace, by address.
struct Modules<'a> {
    units: BTreeMap<&'a Address, &'a Interface>,
    containers: BTreeMap<&'a Address, &'a Interface>,
}

impl<'a> Modules<'a> {
    fn of(side: &'a Side) -> Modules<'a> {
        let mut modules = Modules {
            units: BTreeMap::new(),
            containers: BTreeMap::new(),
        };
        for interface in &side.interfaces {
            let by_address = match interface.shape {
                Shape::Unit => &mut modules.units,
                Shape::Container => &mut modules.containers,
                Shape::Curated => continue,
            };
            by_address.insert(&interface.address, interface);
        }
        modules
    }
}

/// The layer hashes of a unit or a container.
fn layers(interface: &Interface) -> fidryn_provision::LayerHashes {
    interface
        .layers
        .expect("a unit and a container have layer hashes")
}

/// The aliases of a module, in order.
fn aliases(interface: &Interface) -> Vec<&Address> {
    interface.aliases.iter().map(|alias| &alias.item).collect()
}

/// The notes of `interfaces`, each with the module that holds it, its
/// address, its layer and its note hash, as [`note_changes`] compares them.
fn notes_of(interfaces: &[Interface]) -> Vec<NoteAt> {
    interfaces
        .iter()
        .flat_map(|interface| {
            interface.notes.iter().map(|note| NoteAt {
                module: interface.address.clone(),
                address: interface.address_at(&note.at),
                layer: note.layer,
                hash: note.hash,
            })
        })
        .collect()
}

/// A highest provision on one side only, with what it is paired by, which
/// is not known yet for the top provision of a unit on one side that was
/// not opened.
struct Candidate {
    address: Address,
    kind: String,
    status: Option<String>,
    unnumbered: Option<Hash32>,
}

impl Candidate {
    fn found(one: OneSided) -> Candidate {
        Candidate {
            address: one.address,
            kind: one.kind,
            status: one.status,
            unnumbered: Some(one.unnumbered),
        }
    }

    fn kind(&self) -> (String, Option<String>) {
        (self.kind.clone(), self.status.clone())
    }

    fn entry(self) -> (Address, Option<Hash32>) {
        (self.address, self.unnumbered)
    }
}

/// The top provision of each unit of `side` that `other` does not hold.
fn one_sided(side: &Modules<'_>, other: &Modules<'_>) -> Vec<Candidate> {
    side.units
        .iter()
        .filter(|(address, _)| !other.units.contains_key(*address))
        .map(|(address, unit)| {
            let top = &unit.provisions[0];
            Candidate {
                address: (*address).clone(),
                kind: unit.word(top.kind).to_owned(),
                status: top.status.map(|status| unit.word(status).to_owned()),
                unnumbered: None,
            }
        })
        .collect()
}

/// The module at `address` of the workspace in `dir`, read and lowered
/// again, which is counted in `opened`.
fn open(dir: &Path, address: &Address, opened: &AtomicUsize) -> Result<Module, Vec<Diagnostic>> {
    opened.fetch_add(1, Ordering::Relaxed);
    let rel = module_path(address);
    let (_, head) = read_module(&dir.join(&rel)).map_err(|error| {
        below(
            dir,
            vec![
                Diagnostic::new(
                    DiagnosticCode::E730,
                    format!("this file cannot be read again: {error}"),
                )
                .with_source(rel.as_str()),
            ],
        )
    })?;
    let lowered = lower_file(head).map_err(|found| {
        below(
            dir,
            found
                .into_iter()
                .map(|diagnostic| diagnostic.with_source(rel.as_str()))
                .collect(),
        )
    })?;
    Ok(lowered.module)
}

/// The diagnostic of a module file at `address` that no longer holds a
/// module of the shape `shape` when it is read again.
fn not_the_shape(address: &Address, shape: Shape) -> Vec<Diagnostic> {
    vec![
        Diagnostic::new(
            DiagnosticCode::E730,
            format!(
                "this file no longer holds {} when it is read again",
                shape.described()
            ),
        )
        .with_source(module_path(address)),
    ]
}

/// `work` of each of `items`, on `jobs` threads, in the order of `items`.
fn each<T: Sync, R: Send>(items: &[T], jobs: usize, work: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let worker = || {
        let mut done = Vec::new();
        loop {
            let place = next.fetch_add(1, Ordering::Relaxed);
            let Some(item) = items.get(place) else {
                break;
            };
            done.push((place, work(item)));
        }
        done
    };
    let workers = jobs.clamp(1, items.len().max(1));
    let mut done: Vec<(usize, R)> = if workers == 1 {
        worker()
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers).map(|_| scope.spawn(worker)).collect();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("a worker of the comparison panicked"))
                .collect()
        })
    };
    done.sort_by_key(|(place, _)| *place);
    done.into_iter().map(|(_, result)| result).collect()
}

/// The stale bindings of the curated workspace in `dir` against the
/// workspace whose interfaces are `interfaces`, of the prefix `prefix`,
/// indexed in `index`, and what the reading of its modules says of the
/// cache.
fn stale_in(
    dir: &Path,
    interfaces: &[Interface],
    index: &AddressIndex,
    prefix: &Address,
    options: &CheckOptions,
) -> Result<(Vec<StaleBinding>, CacheNotes), Vec<Diagnostic>> {
    let manifest = manifest_of(dir, WorkspaceKind::Curated)?;
    let read = modules_of(dir, &manifest, &in_workspace(dir, options));
    if read
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.severity() == Severity::Error)
    {
        return Err(below(dir, read.diagnostics));
    }
    let notes = CacheNotes {
        unused: read.cache_unused,
        failure: read.cache_failure,
    };
    let curated = read.interfaces;
    let importable: BTreeMap<&Address, &Interface> = curated
        .iter()
        .chain(interfaces)
        .map(|interface| (&interface.address, interface))
        .rev()
        .collect();
    let imported = |address: &Address| importable.get(address).copied();
    let mut stale = Vec::new();
    for module in curated
        .iter()
        .filter(|module| module.shape == Shape::Curated)
    {
        let (bindings, _) = resolve(module, &imported);
        for binding in bindings {
            let target = &binding.target;
            if !target.is_within(prefix) {
                continue;
            }
            let declared = &binding.binding.item;
            if let Some(reason) =
                staleness(target, declared.provision_hash, declared.notes_hash, index)
            {
                stale.push(StaleBinding {
                    workspace: manifest.name.clone(),
                    module: module.address.clone(),
                    binding: declared.name.clone(),
                    address: target.clone(),
                    reason,
                });
            }
        }
    }
    Ok((stale, notes))
}
