//! Phase two of the check of a workspace: the checks across its modules, the
//! staleness of every binding, and the outcome of every reference and of
//! every citation, from the interfaces of the modules of the workspace and of
//! its dependencies alone.

use super::MANIFEST_FILE;
use fidryn_check::e2::names::{ResolvedBinding, resolve};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::interface::{ByteRange, Shape};
use fidryn_provision::layout::module_path;
use fidryn_provision::workspace::{Manifest, WorkspaceKind};
use fidryn_provision::{Address, AddressIndex, Interface, Layer, Outcome, StaleReason, staleness};
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

/// A citation of a curated module whose outcome is not `exact`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnresolvedCitation {
    /// The address of the module that declares the citation.
    pub from: Address,
    /// The citation's name.
    pub citation: String,
    pub target: Address,
    pub outcome: Outcome,
}

/// A workspace that the checked one depends on, as phase two knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DependencyInterfaces {
    /// A dependency that passed its checks: the name its entry gives it,
    /// the release of its lock, which a stale binding into it is reported
    /// with, its prefix, and the interfaces of its modules.
    Loaded {
        name: String,
        release: String,
        prefix: Address,
        interfaces: Vec<Interface>,
    },
    /// A dependency that failed its checks, with its prefix when the check
    /// learned it. Then nothing is known below that prefix but what lies
    /// below the prefix of the workspace or of a loaded dependency; and when
    /// the prefix is not known, nothing is known below no such prefix. A
    /// citation or a reference whose target lies where nothing is known is
    /// left out of the counts and of the unresolved lists, an import of a
    /// module there is not reported, and a binding there is not judged,
    /// since the diagnostic of the dependency says what is wrong.
    Failed { prefix: Option<Address> },
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
    /// The number of citations of the curated modules with each outcome, by
    /// the outcome's name; every name is present.
    pub citations: BTreeMap<String, u64>,
    /// Every citation whose outcome is not `exact`, sorted by the address of
    /// its module, then by its target and its name.
    pub unresolved_citations: Vec<UnresolvedCitation>,
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

/// Counts of every outcome, all zero.
pub(crate) fn no_citations() -> BTreeMap<String, u64> {
    Outcome::NAMES
        .into_iter()
        .map(|name| (name.to_owned(), 0))
        .collect()
}

/// Phase two of the check of a workspace whose manifest is `manifest`, run
/// by the toolchain of version `toolchain`, as a function of the interfaces
/// of its modules, `interfaces`, and of what is known of its dependencies,
/// `dependencies`:
///
/// - the manifest's `toolchain` is `toolchain` (`W760`); a generated
///   workspace has a `release` and no dependencies, and a curated workspace
///   no `release` (`E730`);
/// - the `within` of a unit or a container names a container of the
///   workspace that lists it as a member, and every member of a container
///   is a module whose `within` is that container (`E720`);
/// - every import of a curated module names a module of `interfaces` or of
///   a dependency (`E720`), and the names of the curated module resolve
///   ([`resolve`]; `E200`);
/// - no binding that resolves is stale (`W750`, a warning, at its name):
///   what it binds lies below the prefix of a dependency that passed its
///   checks, and the two hashes it records are those of the provision there
///   and of its unit ([`staleness`]); what lies below no such prefix is
///   "provision removed or moved";
/// - no two modules of the workspace declare one address, as the address of
///   a module, a provision or a note: each module after the first by path
///   that declares it is reported (`E700`), at its address when the address
///   is its own, and otherwise without a span;
/// - the outcome of every reference of the workspace and of every citation
///   of its curated modules, against an index of the addresses of
///   `interfaces` and of the dependencies, below the manifest's prefix and
///   theirs.
///
/// What is not known is what lies below no prefix of the workspace or of a
/// loaded dependency and, when every dependency that failed has a known
/// prefix, below the prefix of one of them: a reference or a citation whose
/// target is an address that lies there is left out of the counts and of
/// the unresolved lists, an import of a module there that is not found is
/// not reported, and a binding of an address there is not judged. A target
/// that is no address is `malformed` wherever it would lie.
///
/// Each module is taken to stand at the path of its address, which names
/// its file in a diagnostic. The result is the same for any order of
/// `interfaces`, of `dependencies` and of the interfaces of each, in which
/// no two modules declare one address of a module, a provision or a note,
/// or one alias, since a reference to such an address or alias reaches the
/// interface that comes first, and in which no two of the prefixes of the
/// workspace and of its dependencies overlap.
/// [`check_workspace`](super::check_workspace) runs the same, with the
/// interfaces of the modules that stand at their paths and those of the
/// dependencies it loaded, on several threads, and also names the module
/// files that gave none, so that nothing that names one of them is reported
/// again.
pub fn check_interfaces(
    interfaces: &[Interface],
    dependencies: &[DependencyInterfaces],
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
    dependencies: &[DependencyInterfaces],
    manifest: &Manifest,
    toolchain: &str,
    unread: &BTreeSet<String>,
    jobs: usize,
) -> PhaseTwo {
    let mut loaded: Vec<&Interface> = Vec::new();
    let mut known = vec![manifest.prefix.clone()];
    let mut pinned = Vec::new();
    let mut failed = Vec::new();
    let mut unplaced = false;
    for dependency in dependencies {
        match dependency {
            DependencyInterfaces::Loaded {
                name,
                release,
                prefix,
                interfaces,
            } => {
                loaded.extend(interfaces);
                known.push(prefix.clone());
                pinned.push(Pinned {
                    name,
                    release,
                    prefix,
                });
            }
            DependencyInterfaces::Failed {
                prefix: Some(prefix),
            } => failed.push(prefix),
            DependencyInterfaces::Failed { prefix: None } => unplaced = true,
        }
    }
    let unknown = Unknown {
        known: &known,
        failed,
        unplaced,
    };
    let index = AddressIndex::build(interfaces.iter().chain(loaded.iter().copied()), &known);
    let passed = Loaded {
        interfaces: &loaded,
        pinned: &pinned,
    };
    let mut diagnostics = checks(
        interfaces, &passed, manifest, toolchain, unread, &index, &unknown,
    );
    diagnostics.sort_by(|a, b| order(a).cmp(&order(b)));
    let counted = outcomes(interfaces, &index, &unknown, jobs);
    let mut references = no_references();
    for (layer, counts) in LAYERS.into_iter().zip(counted.references) {
        let names = references.get_mut(&layer).expect("every layer is counted");
        for (name, count) in Outcome::NAMES.into_iter().zip(counts) {
            *names.get_mut(name).expect("every outcome is counted") = count;
        }
    }
    let citations = Outcome::NAMES
        .into_iter()
        .zip(counted.citations)
        .map(|(name, count)| (name.to_owned(), count))
        .collect();
    PhaseTwo {
        diagnostics,
        references,
        outside_by_body: counted.outside_by_body,
        unresolved: counted.unresolved,
        citations,
        unresolved_citations: counted.unresolved_citations,
    }
}

/// The diagnostics of phase two of a workspace that has no dependencies,
/// such as a generated workspace: those of [`check_interfaces`], without
/// the outcomes. The check of a dependency runs this on its modules, so
/// that it finds what `fidryn check` of the dependency's directory finds.
pub(crate) fn own_checks(
    interfaces: &[Interface],
    manifest: &Manifest,
    toolchain: &str,
    unread: &BTreeSet<String>,
) -> Vec<Diagnostic> {
    let index = AddressIndex::build(interfaces, std::slice::from_ref(&manifest.prefix));
    let unknown = Unknown {
        known: &[],
        failed: Vec::new(),
        unplaced: false,
    };
    let none = Loaded {
        interfaces: &[],
        pinned: &[],
    };
    checks(
        interfaces, &none, manifest, toolchain, unread, &index, &unknown,
    )
}

/// A dependency that passed its checks, as the warning of a stale binding
/// into it names it.
struct Pinned<'a> {
    name: &'a str,
    /// The release of its lock.
    release: &'a str,
    prefix: &'a Address,
}

/// What phase two holds of the dependencies that passed their checks.
struct Loaded<'a> {
    /// The interfaces of their modules.
    interfaces: &'a [&'a Interface],
    /// Each of them, in the order in which phase two is given them; their
    /// prefixes do not overlap, so at most one holds a bound address.
    pinned: &'a [Pinned<'a>],
}

/// The checks of phase two that give diagnostics, with `dependencies` what
/// is held of the dependencies that were loaded, and `index` the index of
/// the addresses of `interfaces` and of theirs.
fn checks(
    interfaces: &[Interface],
    dependencies: &Loaded<'_>,
    manifest: &Manifest,
    toolchain: &str,
    unread: &BTreeSet<String>,
    index: &AddressIndex,
    unknown: &Unknown<'_>,
) -> Vec<Diagnostic> {
    let mut diagnostics = of_manifest(manifest, toolchain);
    diagnostics.extend(hierarchy(interfaces, unread));
    diagnostics.extend(curated(interfaces, dependencies, unread, index, unknown));
    diagnostics.extend(declared_twice(index, interfaces));
    diagnostics
}

/// Where nothing is known: below no prefix of the workspace or of a loaded
/// dependency, and either below the prefix of a dependency that failed or
/// anywhere at all when a dependency failed whose prefix is not known.
struct Unknown<'a> {
    /// The prefixes of the workspace and of the loaded dependencies.
    known: &'a [Address],
    /// The prefixes of the dependencies that failed, where they are known.
    failed: Vec<&'a Address>,
    /// Whether a dependency failed whose prefix is not known.
    unplaced: bool,
}

impl Unknown<'_> {
    /// Whether `target` lies where nothing is known.
    fn holds(&self, target: &str) -> bool {
        if self.failed.is_empty() && !self.unplaced {
            return false;
        }
        !self.known.iter().any(|prefix| lies_below(target, prefix))
            && (self.unplaced || self.failed.iter().any(|prefix| lies_below(target, prefix)))
    }
}

/// Whether `target` lies below `prefix`: is it, or continues it at a
/// segment boundary.
fn lies_below(target: &str, prefix: &Address) -> bool {
    target
        .strip_prefix(prefix.as_str())
        .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
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
                "the workspace records the toolchain `{}`, and this is `{toolchain}`",
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
    /// Why an address that is neither a module's nor an alias names
    /// nothing, as the end of a sentence about it.
    missing: &'static str,
}

impl<'a> Modules<'a> {
    fn new(
        interfaces: impl IntoIterator<Item = &'a Interface>,
        unread: &'a BTreeSet<String>,
        missing: &'static str,
    ) -> Modules<'a> {
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
            missing,
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
            None => self.missing.to_owned(),
        }))
    }
}

/// The checks of `within` and of the members of containers.
fn hierarchy(interfaces: &[Interface], unread: &BTreeSet<String>) -> Vec<Diagnostic> {
    let modules = Modules::new(interfaces, unread, "is no module of the workspace");
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

/// The checks of the imports, the names and the bindings of each curated
/// module of `interfaces`: an import names a module of `interfaces` or of
/// `dependencies` (`E720`, at its address), the module's names resolve
/// ([`resolve`]), and no binding that resolves is stale ([`stale`],
/// `W750`). An import of a module file that gave no interface, and one of a
/// module where nothing is known, are not reported again, and nothing is
/// resolved through them.
fn curated(
    interfaces: &[Interface],
    dependencies: &Loaded<'_>,
    unread: &BTreeSet<String>,
    index: &AddressIndex,
    unknown: &Unknown<'_>,
) -> Vec<Diagnostic> {
    let importable = Modules::new(
        interfaces
            .iter()
            .chain(dependencies.interfaces.iter().copied()),
        unread,
        "is no module of the workspace or of a dependency",
    );
    let imported = |address: &Address| importable.by_address.get(address).copied();
    let mut out = Vec::new();
    let modules = interfaces
        .iter()
        .filter(|interface| interface.shape == Shape::Curated);
    for interface in modules {
        let file = module_path(&interface.address);
        for import in &interface.imports {
            let named = &import.item.module;
            if let Err(Some(why)) = importable.get(named)
                && !unknown.holds(named.as_str())
            {
                out.push(
                    Diagnostic::new(
                        DiagnosticCode::E720,
                        format!("`import` names `{named}`, which {why}"),
                    )
                    .with_span(span(import.range.module))
                    .with_source(file.as_str()),
                );
            }
        }
        let (bindings, diagnostics) = resolve(interface, &imported);
        out.extend(
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.with_source(file.as_str())),
        );
        out.extend(bindings.iter().filter_map(|binding| {
            stale(binding, dependencies.pinned, index, unknown)
                .map(|diagnostic| diagnostic.with_source(file.as_str()))
        }));
    }
    out
}

/// The warning (`W750`, at the binding's name) of `binding` when it is
/// stale: when what it binds lies below the prefix of a dependency of
/// `pinned` and [`staleness`] gives a reason against `index`, the index of
/// the workspace and of the dependencies that passed their checks; and when
/// it lies below no prefix of a dependency, "provision removed or moved".
/// A binding of an address where nothing is known, below the prefix of a
/// dependency that failed, is not judged.
fn stale(
    binding: &ResolvedBinding,
    pinned: &[Pinned<'_>],
    index: &AddressIndex,
    unknown: &Unknown<'_>,
) -> Option<Diagnostic> {
    let target = &binding.target;
    if unknown.holds(target.as_str()) {
        return None;
    }
    let declared = &binding.binding.item;
    let release = &declared.release;
    let named = format!("the binding `{}` of `{target}`", declared.name);
    let hashes = "the hashes that `fidryn text` prints for it with `--hashes`";
    let message = match pinned
        .iter()
        .find(|dependency| lies_below(target.as_str(), dependency.prefix))
    {
        None => format!(
            "{named} is stale ({}): the address lies below no dependency of the workspace, and \
             the binding was written against the release `{release}`; bind a provision of a \
             dependency, with {hashes}",
            StaleReason::Removed
        ),
        Some(dependency) => {
            let reason = staleness(target, declared.provision_hash, declared.notes_hash, index)?;
            let action = match reason {
                StaleReason::Removed => format!(
                    "find where the provision now stands, read it again, and bind its address \
                     with {hashes}"
                ),
                StaleReason::TextChanged => {
                    format!("read the provision again, and write into the binding {hashes}")
                }
                StaleReason::EnactedNotesChanged => format!(
                    "read the provision and the enacted notes of its unit again, and write into \
                     the binding {hashes}"
                ),
            };
            format!(
                "{named} is stale ({reason}) in the release `{}` of the dependency `{}`, and \
                 was written against the release `{release}`; {action}",
                dependency.release, dependency.name
            )
        }
    };
    Some(Diagnostic::new(DiagnosticCode::W750, message).with_span(span(binding.binding.range.name)))
}

/// Each address that more than one module of `interfaces` declares, the
/// index of `interfaces` and then of the dependencies being `index`: it is
/// reported in each module after the first by path that declares it, naming
/// both files (`E700`), at the module's address when it is the module's own,
/// and otherwise without a span, since an interface holds no range of the
/// address of a provision or a note.
fn declared_twice(index: &AddressIndex, interfaces: &[Interface]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for twice in index.declared_twice() {
        // A place past `interfaces` is a module of a dependency.
        let mut modules: Vec<(String, &Interface)> = twice
            .modules
            .iter()
            .filter_map(|place| interfaces.get(*place))
            .map(|interface| (module_path(&interface.address), interface))
            .collect();
        modules.sort_by(|a, b| a.0.cmp(&b.0));
        let Some(((first, _), later)) = modules.split_first() else {
            continue;
        };
        for (file, interface) in later {
            let mut diagnostic = Diagnostic::new(
                DiagnosticCode::E700,
                format!(
                    "the address `{}` is declared in `{first}` and again in `{file}`, and an \
                     address is declared by one module only",
                    twice.address
                ),
            )
            .with_source(file.as_str());
            if interface.address == twice.address {
                diagnostic = diagnostic.with_span(span(interface.address_range));
            }
            out.push(diagnostic);
        }
    }
    out
}

/// The outcomes of the references and of the citations of some modules,
/// counted.
#[derive(Default)]
struct Counted {
    /// By layer, in the order of [`LAYERS`], and by outcome, in the order of
    /// [`Outcome::NAMES`].
    references: [[u64; 8]; 3],
    outside_by_body: BTreeMap<String, u64>,
    unresolved: Vec<Unresolved>,
    /// By outcome, in the order of [`Outcome::NAMES`].
    citations: [u64; 8],
    unresolved_citations: Vec<UnresolvedCitation>,
}

/// The place of the name of `outcome` in [`Outcome::NAMES`].
fn place_of(outcome: &Outcome) -> usize {
    Outcome::NAMES
        .iter()
        .position(|name| *name == outcome.name())
        .expect("every outcome has a name")
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
        for (count, more) in self.citations.iter_mut().zip(other.citations) {
            *count += more;
        }
        self.unresolved_citations.extend(other.unresolved_citations);
    }

    /// Counts the references and the citations of `interface`, but for
    /// those whose target is an address that lies where nothing is known; a
    /// target that is no address is `malformed` wherever it would lie.
    fn of(&mut self, interface: &Interface, index: &AddressIndex, unknown: &Unknown<'_>) {
        for reference in &interface.references {
            let outcome = index.outcome(&reference.target);
            if outcome != Outcome::Malformed && unknown.holds(&reference.target) {
                continue;
            }
            let layer = LAYERS
                .iter()
                .position(|layer| *layer == reference.layer)
                .expect("a reference has one of the three layers");
            self.references[layer][place_of(&outcome)] += 1;
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
        for citation in &interface.citations {
            let target = &citation.item.target;
            let outcome = index.outcome(target.as_str());
            if outcome != Outcome::Malformed && unknown.holds(target.as_str()) {
                continue;
            }
            self.citations[place_of(&outcome)] += 1;
            if outcome != Outcome::Exact {
                self.unresolved_citations.push(UnresolvedCitation {
                    from: interface.address.clone(),
                    citation: citation.item.name.clone(),
                    target: target.clone(),
                    outcome,
                });
            }
        }
    }
}

/// The outcome of every reference and of every citation of `interfaces`,
/// on `jobs` threads. The result is the same for any number of them: the
/// counts are sums, and the unresolved references and citations are sorted.
fn outcomes(
    interfaces: &[Interface],
    index: &AddressIndex,
    unknown: &Unknown<'_>,
    jobs: usize,
) -> Counted {
    let next = AtomicUsize::new(0);
    let work = || {
        let mut counted = Counted::default();
        loop {
            let at = next.fetch_add(1, Ordering::Relaxed);
            let Some(interface) = interfaces.get(at) else {
                break;
            };
            counted.of(interface, index, unknown);
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
    counted.unresolved_citations.sort_by(|a, b| {
        (&a.from, &a.target, &a.citation, &a.outcome).cmp(&(
            &b.from,
            &b.target,
            &b.citation,
            &b.outcome,
        ))
    });
    counted
}
