//! Phase two of the check of a workspace: the checks across its modules, and
//! the outcome of every reference, from the interfaces of the modules alone.

use super::MANIFEST_FILE;
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::interface::ByteRange;
use fidryn_provision::layout::module_path;
use fidryn_provision::workspace::{Manifest, WorkspaceKind};
use fidryn_provision::{Address, AddressIndex, Interface, Layer, Outcome};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The layers of the model, in their order.
pub(crate) const LAYERS: [Layer; 3] = [Layer::Operative, Layer::Enacted, Layer::Editorial];

/// A reference whose outcome is not `exact`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved {
    /// The address the reference stands in: of its provision or note, or
    /// of its module for a credit, a footnote and the number or heading of
    /// a container.
    pub from: Address,
    pub layer: Layer,
    /// The target as the source writes it.
    pub target: String,
    pub outcome: Outcome,
}

/// What phase two gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PhaseTwo {
    /// Every diagnostic, each naming the file of its module by the path of
    /// the module's address, sorted by that path and then by position.
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
}

/// Counts of every layer and every outcome, all zero.
pub(crate) fn no_references() -> BTreeMap<Layer, BTreeMap<String, u64>> {
    LAYERS
        .into_iter()
        .map(|layer| {
            let outcomes = Outcome::NAMES
                .into_iter()
                .map(|name| (name.to_owned(), 0))
                .collect();
            (layer, outcomes)
        })
        .collect()
}

/// Phase two of the check of a workspace whose manifest is `manifest`, run
/// by the toolchain of version `toolchain`, as a function of the interfaces
/// of its modules, `interfaces`, and of those of its dependencies,
/// `dependencies`:
///
/// - the manifest's `toolchain` is `toolchain` (`W760`); a generated
///   workspace has a `release` and no dependencies, and a curated workspace
///   no `release` (`E730`);
/// - the `within` of a unit or a container names a container of the
///   workspace that lists it as a member, and every member of a container
///   is a module whose `within` is that container (`E720`);
/// - the outcome of every reference of the workspace, against an index of
///   the addresses of `interfaces` and `dependencies` below the manifest's
///   prefix.
///
/// Each module is taken to stand at the path of its address, which names
/// its file in a diagnostic. [`check_workspace`](super::check_workspace)
/// runs the same, with the interfaces of the modules that stand at their
/// paths, on several threads, and also names the module files that gave
/// none, so that nothing that names one of them is reported again.
pub fn check_interfaces(
    interfaces: &[Interface],
    dependencies: &[Interface],
    manifest: &Manifest,
    toolchain: &str,
) -> PhaseTwo {
    across(
        interfaces,
        dependencies,
        manifest,
        toolchain,
        &BTreeSet::new(),
        1,
    )
}

/// [`check_interfaces`], with `unread` the paths of the module files that
/// gave no interface, on `jobs` threads.
pub(crate) fn across(
    interfaces: &[Interface],
    dependencies: &[Interface],
    manifest: &Manifest,
    toolchain: &str,
    unread: &BTreeSet<String>,
    jobs: usize,
) -> PhaseTwo {
    let mut diagnostics = of_manifest(manifest, toolchain);
    diagnostics.extend(hierarchy(interfaces, unread));
    diagnostics.sort_by(|a, b| order(a).cmp(&order(b)));
    let index = AddressIndex::build(
        interfaces.iter().chain(dependencies),
        std::slice::from_ref(&manifest.prefix),
    );
    let counted = outcomes(interfaces, &index, jobs);
    let mut references = no_references();
    for (layer, counts) in LAYERS.into_iter().zip(counted.references) {
        let names = references.get_mut(&layer).expect("every layer is counted");
        for (name, count) in Outcome::NAMES.into_iter().zip(counts) {
            *names.get_mut(name).expect("every outcome is counted") = count;
        }
    }
    PhaseTwo {
        diagnostics,
        references,
        outside_by_body: counted.outside_by_body,
        unresolved: counted.unresolved,
    }
}

/// The key the diagnostics of a report are sorted by: the file, the
/// position, then the code and the message.
pub(crate) fn order(
    diagnostic: &Diagnostic,
) -> (Option<&str>, Option<(u32, u32)>, &'static str, &str) {
    (
        diagnostic.source_file.as_deref(),
        diagnostic.primary_span.map(|span| (span.start, span.end)),
        diagnostic.code.as_str(),
        &diagnostic.message,
    )
}

/// `range` as the span of a diagnostic.
pub(crate) fn span(range: ByteRange) -> Span {
    Span {
        start: range.start,
        end: range.end,
    }
}

/// The checks of the manifest by itself.
fn of_manifest(manifest: &Manifest, toolchain: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut report = |code: DiagnosticCode, message: String| {
        out.push(Diagnostic::new(code, message).with_source(MANIFEST_FILE));
    };
    if manifest.toolchain != toolchain {
        report(
            DiagnosticCode::W760,
            format!(
                "the workspace was last checked with the toolchain `{}`, and this is `{toolchain}`",
                manifest.toolchain
            ),
        );
    }
    match (manifest.kind, &manifest.release) {
        (WorkspaceKind::Generated, None) => report(
            DiagnosticCode::E730,
            "a generated workspace has a `release` in its manifest, and this one has none"
                .to_owned(),
        ),
        (WorkspaceKind::Curated, Some(release)) => report(
            DiagnosticCode::E730,
            format!(
                "a curated workspace has no `release` in its manifest, and this one has \
                 `{release}`"
            ),
        ),
        _ => {}
    }
    if manifest.kind == WorkspaceKind::Generated && !manifest.dependencies.is_empty() {
        report(
            DiagnosticCode::E730,
            format!(
                "a generated workspace has no dependencies, and this one has {}",
                manifest.dependencies.len()
            ),
        );
    }
    out
}

/// The modules of a workspace by their addresses and by their aliases; of
/// two with one address or one alias, the first is kept.
struct Modules<'a> {
    by_address: BTreeMap<&'a Address, &'a Interface>,
    by_alias: BTreeMap<&'a Address, &'a Interface>,
    /// The paths of the module files that gave no interface, which are
    /// already reported.
    unread: &'a BTreeSet<String>,
}

impl<'a> Modules<'a> {
    fn new(interfaces: &'a [Interface], unread: &'a BTreeSet<String>) -> Modules<'a> {
        let mut by_address = BTreeMap::new();
        let mut by_alias = BTreeMap::new();
        for interface in interfaces {
            by_address.entry(&interface.address).or_insert(interface);
            for alias in &interface.aliases {
                by_alias.entry(&alias.item).or_insert(interface);
            }
        }
        Modules {
            by_address,
            by_alias,
            unread,
        }
    }

    /// The module with the address `address`. `Err` holds why there is
    /// none, or `None` when the file of that address is already reported.
    fn get(&self, address: &Address) -> Result<&'a Interface, Option<String>> {
        if let Some(found) = self.by_address.get(address) {
            return Ok(found);
        }
        if self.unread.contains(&module_path(address)) {
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

/// The checks of `within` and of the members of containers.
fn hierarchy(interfaces: &[Interface], unread: &BTreeSet<String>) -> Vec<Diagnostic> {
    let modules = Modules::new(interfaces, unread);
    let mut out = Vec::new();
    for interface in interfaces {
        let file = module_path(&interface.address);
        let mut report = |range: ByteRange, message: String| {
            out.push(
                Diagnostic::new(DiagnosticCode::E720, message)
                    .with_span(span(range))
                    .with_source(file.as_str()),
            );
        };
        if let Some(within) = &interface.within {
            let named = &within.item;
            let problem = match modules.get(named) {
                Err(why) => why.map(|why| format!("`within` names `{named}`, which {why}")),
                Ok(container) => match &container.container {
                    None => Some(format!(
                        "`within` names `{named}`, which is {}, not a container",
                        container.shape.described()
                    )),
                    Some(entry)
                        if !entry
                            .members
                            .iter()
                            .any(|member| member.item == interface.address) =>
                    {
                        Some(format!(
                            "`within` names the container `{named}`, which does not list `{}` \
                             as a member",
                            interface.address
                        ))
                    }
                    Some(_) => None,
                },
            };
            if let Some(message) = problem {
                report(within.range, message);
            }
        }
        let members = interface
            .container
            .iter()
            .flat_map(|container| &container.members);
        for member in members {
            let named = &member.item;
            let problem = match modules.get(named) {
                Err(why) => why.map(|why| format!("the member `{named}` {why}")),
                Ok(module) => match &module.within {
                    None => Some(format!(
                        "the member `{named}` has no `within`, and a member is within the \
                         container that lists it"
                    )),
                    Some(within) if within.item != interface.address => Some(format!(
                        "the member `{named}` is within `{}`, not within this container",
                        within.item
                    )),
                    Some(_) => None,
                },
            };
            if let Some(message) = problem {
                report(member.range, message);
            }
        }
    }
    out
}

/// The outcomes of the references of some modules, counted.
#[derive(Default)]
struct Counted {
    /// By layer, in the order of [`LAYERS`], and by outcome, in the order of
    /// [`Outcome::NAMES`].
    references: [[u64; 8]; 3],
    outside_by_body: BTreeMap<String, u64>,
    unresolved: Vec<Unresolved>,
}

impl Counted {
    fn add(&mut self, other: Counted) {
        for (mine, theirs) in self.references.iter_mut().zip(other.references) {
            for (count, more) in mine.iter_mut().zip(theirs) {
                *count += more;
            }
        }
        for (body, count) in other.outside_by_body {
            *self.outside_by_body.entry(body).or_default() += count;
        }
        self.unresolved.extend(other.unresolved);
    }

    fn of(&mut self, interface: &Interface, index: &AddressIndex) {
        for reference in &interface.references {
            let outcome = index.outcome(&reference.target);
            let layer = LAYERS
                .iter()
                .position(|layer| *layer == reference.layer)
                .expect("a reference has one of the three layers");
            let name = Outcome::NAMES
                .iter()
                .position(|name| *name == outcome.name())
                .expect("every outcome has a name");
            self.references[layer][name] += 1;
            if let Outcome::Outside(body) = &outcome {
                *self.outside_by_body.entry(body.clone()).or_default() += 1;
            }
            if outcome != Outcome::Exact {
                self.unresolved.push(Unresolved {
                    from: interface.place_address(reference.from),
                    layer: reference.layer,
                    target: reference.target.to_string(),
                    outcome,
                });
            }
        }
    }
}

/// The outcome of every reference of `interfaces`, on `jobs` threads. The
/// result is the same for any number of them: the counts are sums, and the
/// unresolved references are sorted.
fn outcomes(interfaces: &[Interface], index: &AddressIndex, jobs: usize) -> Counted {
    let next = AtomicUsize::new(0);
    let work = || {
        let mut counted = Counted::default();
        loop {
            let at = next.fetch_add(1, Ordering::Relaxed);
            let Some(interface) = interfaces.get(at) else {
                break;
            };
            counted.of(interface, index);
        }
        counted
    };
    let workers = jobs.clamp(1, interfaces.len().max(1));
    let mut counted = if workers == 1 {
        work()
    } else {
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers).map(|_| scope.spawn(work)).collect();
            let mut all = Counted::default();
            for handle in handles {
                all.add(handle.join().expect("a worker of the check panicked"));
            }
            all
        })
    };
    counted.unresolved.sort_by(|a, b| {
        (&a.from, &a.target, a.layer, &a.outcome).cmp(&(&b.from, &b.target, b.layer, &b.outcome))
    });
    counted
}
