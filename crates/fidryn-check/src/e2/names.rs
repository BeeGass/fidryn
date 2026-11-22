//! The names of a curated module, resolved across the modules of a
//! workspace from their interfaces alone.
//!
//! A curated module has one set of names: the names of its citations and of
//! its bindings, the alias of each import written `import "..." as alias`,
//! and each name that an import brings into scope with
//! `import "..." { ... }`. A citation gives a name to an address. `binds`
//! names an address or a citation: without an alias, a citation of the
//! module itself or one that an import brings into scope; as `alias.name`,
//! a citation of the module imported under that alias. Only an imported
//! module's own citations are visible through it, and nothing it imports
//! itself, so two modules may import each other.

use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::interface::{BindingRanges, ByteRange, Declared};
use fidryn_provision::{Address, BindTarget, Binding, Interface};
use std::collections::{BTreeMap, BTreeSet};

/// A binding of a curated module and the address it binds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedBinding {
    /// The binding as the module declares it, with where it and its parts
    /// stand in the module's source.
    pub binding: Declared<Binding, BindingRanges>,
    /// The address after `binds`, or the target of the citation named there.
    pub target: Address,
}

/// The bindings of the curated module `module` that resolve, each with the
/// address it binds, in the order of the module's bindings; and the
/// diagnostics of its names, sorted by position, none of which names a file.
/// `imported` gives the interface of the module that an import names, or
/// `None` when there is none; an import of a module that is not there is not
/// reported here, and the check of a workspace reports it (`E720`).
///
/// - The names of its citations and bindings, the aliases of its imports
///   and the names its imports bring into scope are one set: a name that
///   enters it again is `E200` where it does, with the place where it
///   entered first as a related span. Two citations or bindings of one name
///   are left to [`check_module`](super::check_module), which reports them.
/// - Each name that an import brings into scope is a citation of the module
///   it imports (`E200` at the name).
/// - A binding binds the address after `binds`, or the target of the
///   citation named there, which is: for a name without an alias, a
///   citation of the module or one that an import brings into scope; for
///   `alias.name`, a citation of the module imported under that alias
///   (`E200` at the name, or at the alias when no import has it).
///
/// A binding whose name cannot be resolved for a reason reported elsewhere,
/// a name that enters the set twice, a name brought into scope that its
/// module does not declare, or an import of a module that is not there, is
/// left out and not reported again.
pub fn resolve<'a>(
    module: &Interface,
    imported: &dyn Fn(&Address) -> Option<&'a Interface>,
) -> (Vec<ResolvedBinding>, Vec<Diagnostic>) {
    let imports: Vec<Option<&'a Interface>> = module
        .imports
        .iter()
        .map(|import| imported(&import.item.module))
        .collect();
    let mut names = Names {
        module,
        imports,
        set: set_of(module),
        diagnostics: Vec::new(),
    };
    names.declared_once();
    names.brought_into_scope();
    let resolved = module
        .bindings
        .iter()
        .filter_map(|binding| {
            names.target(binding).map(|target| ResolvedBinding {
                binding: binding.clone(),
                target,
            })
        })
        .collect();
    let mut diagnostics = names.diagnostics;
    diagnostics
        .sort_by_key(|diagnostic| diagnostic.primary_span.map(|span| (span.start, span.end)));
    (resolved, diagnostics)
}

/// What puts a name into the set of names of a module.
#[derive(Clone, Copy, Debug)]
enum Source {
    /// The citation at this place of the module's citations.
    Citation(usize),
    /// A binding of the module.
    Binding,
    /// The alias of the import at this place of the module's imports.
    Alias(usize),
    /// A name in the braces of the import at this place.
    Brought(usize),
}

impl Source {
    /// Whether the module declares the name itself, as a citation or a
    /// binding.
    fn local(self) -> bool {
        matches!(self, Source::Citation(_) | Source::Binding)
    }
}

/// A name of the set of names of a module, where it stands, and what puts
/// it there.
struct Name<'m> {
    text: &'m str,
    range: ByteRange,
    source: Source,
}

/// The set of names of `module`, in the order of its source.
fn set_of(module: &Interface) -> Vec<Name<'_>> {
    let mut set = Vec::new();
    for (at, import) in module.imports.iter().enumerate() {
        if let (Some(alias), Some(range)) = (&import.item.alias, import.range.alias) {
            set.push(Name {
                text: alias,
                range,
                source: Source::Alias(at),
            });
        }
        for (name, range) in import.item.names.iter().zip(&import.range.names) {
            set.push(Name {
                text: name,
                range: *range,
                source: Source::Brought(at),
            });
        }
    }
    for (at, citation) in module.citations.iter().enumerate() {
        set.push(Name {
            text: &citation.item.name,
            range: citation.range.name,
            source: Source::Citation(at),
        });
    }
    for binding in &module.bindings {
        set.push(Name {
            text: &binding.item.name,
            range: binding.range.name,
            source: Source::Binding,
        });
    }
    set.sort_by_key(|name| name.range.start);
    set
}

/// The target of the citation named `name` of `module`, the module's own and
/// not one it imports.
fn citation<'a>(module: &'a Interface, name: &str) -> Option<&'a Address> {
    module
        .citations
        .iter()
        .find(|citation| citation.item.name == name)
        .map(|citation| &citation.item.target)
}

fn span(range: ByteRange) -> Span {
    Span {
        start: range.start,
        end: range.end,
    }
}

/// The resolution of the names of one module.
struct Names<'m, 'a> {
    module: &'m Interface,
    /// The interface of the module that each import names, when there is
    /// one.
    imports: Vec<Option<&'a Interface>>,
    set: Vec<Name<'m>>,
    diagnostics: Vec<Diagnostic>,
}

impl Names<'_, '_> {
    fn error(&mut self, range: ByteRange, message: String) {
        self.diagnostics
            .push(Diagnostic::new(DiagnosticCode::E200, message).with_span(span(range)));
    }

    /// How `source` puts a name into the set, as a sentence says it.
    fn described(&self, source: Source) -> String {
        match source {
            Source::Citation(_) => "as a citation".to_owned(),
            Source::Binding => "as a binding".to_owned(),
            Source::Alias(at) => format!(
                "as the alias of the import of `{}`",
                self.module.imports[at].item.module
            ),
            Source::Brought(at) => format!(
                "as a name brought into scope from `{}`",
                self.module.imports[at].item.module
            ),
        }
    }

    /// Reports each name of the set where it enters the set again, unless
    /// the module declares it again after declaring it itself, which the
    /// check of one module reports.
    fn declared_once(&mut self) {
        let mut first: BTreeMap<&str, (ByteRange, Source)> = BTreeMap::new();
        let mut local: BTreeSet<&str> = BTreeSet::new();
        let mut twice = Vec::new();
        for name in &self.set {
            match first.get(name.text) {
                None => {
                    first.insert(name.text, (name.range, name.source));
                }
                Some(&(earlier, source)) => {
                    if !(name.source.local() && local.contains(name.text)) {
                        twice.push((name.text, name.range, name.source, earlier, source));
                    }
                }
            }
            if name.source.local() {
                local.insert(name.text);
            }
        }
        for (text, range, later, earlier, before) in twice {
            let message = format!(
                "the name `{text}` is declared twice: {}, and before that {}",
                self.described(later),
                self.described(before)
            );
            let mut diagnostic =
                Diagnostic::new(DiagnosticCode::E200, message).with_span(span(range));
            diagnostic.related_spans.push(span(earlier));
            self.diagnostics.push(diagnostic);
        }
    }

    /// Reports each name that an import brings into scope and that the
    /// module it imports does not cite.
    fn brought_into_scope(&mut self) {
        let module = self.module;
        for (import, found) in module.imports.iter().zip(self.imports.clone()) {
            let Some(found) = found else {
                continue;
            };
            for (name, range) in import.item.names.iter().zip(&import.range.names) {
                if citation(found, name).is_none() {
                    self.error(
                        *range,
                        format!(
                            "the import brings `{name}` into scope from `{}`, which declares no \
                             citation `{name}`",
                            import.item.module
                        ),
                    );
                }
            }
        }
    }

    /// The address that `binding` binds, when it resolves.
    fn target(&mut self, binding: &Declared<Binding, BindingRanges>) -> Option<Address> {
        match &binding.item.binds {
            BindTarget::Address(address) => Some(address.clone()),
            BindTarget::Name { module: None, name } => self.unqualified(name, binding.range.target),
            BindTarget::Name {
                module: Some(alias),
                name,
            } => self.qualified(alias, name, &binding.range),
        }
    }

    /// The target of the citation that `binds name` names, the name standing
    /// at `range`.
    fn unqualified(&mut self, name: &str, range: ByteRange) -> Option<Address> {
        let found: Vec<Source> = self
            .set
            .iter()
            .filter(|each| each.text == name)
            .map(|each| each.source)
            .take(2)
            .collect();
        let source = match found.as_slice() {
            [one] => *one,
            // A name that enters the set twice is reported where it does.
            [_, _, ..] => return None,
            [] => {
                self.error(
                    range,
                    format!(
                        "`binds` names `{name}`, which is no citation of this module and no name \
                         that an import brings into scope"
                    ),
                );
                return None;
            }
        };
        match source {
            Source::Citation(at) => Some(self.module.citations[at].item.target.clone()),
            // A name that the imported module does not cite is reported at
            // the import, and so is a module that is not there.
            Source::Brought(at) => self.imports[at]
                .and_then(|found| citation(found, name))
                .cloned(),
            Source::Alias(_) => {
                self.error(
                    range,
                    format!(
                        "`binds` names `{name}`, which is the alias of an import and not a \
                         citation; a citation of the module it imports is `{name}.` followed by \
                         its name"
                    ),
                );
                None
            }
            Source::Binding => {
                self.error(
                    range,
                    format!("`binds` names `{name}`, which is a binding and not a citation"),
                );
                None
            }
        }
    }

    /// The target of the citation that `binds alias.name` names, its parts
    /// standing at `ranges`.
    fn qualified(&mut self, alias: &str, name: &str, ranges: &BindingRanges) -> Option<Address> {
        let module = self.module;
        let mut aliased = module
            .imports
            .iter()
            .enumerate()
            .filter(|(_, import)| import.item.alias.as_deref() == Some(alias));
        let (at, import) = match (aliased.next(), aliased.next()) {
            // An alias of two imports is reported where it is declared again.
            (Some(_), Some(_)) => return None,
            (Some(one), None) => one,
            (None, _) => {
                self.error(
                    ranges.qualifier.unwrap_or(ranges.target),
                    format!(
                        "`binds` names `{alias}.{name}`, and `{alias}` is the alias of no import \
                         of this module"
                    ),
                );
                return None;
            }
        };
        // A module that is not there is reported at its import.
        let found = self.imports[at]?;
        if let Some(target) = citation(found, name) {
            return Some(target.clone());
        }
        let mut message = format!(
            "`binds` names `{alias}.{name}`, and the module `{}`, imported as `{alias}`, declares \
             no citation `{name}`",
            import.item.module
        );
        let imports_it = found
            .imports
            .iter()
            .any(|import| import.item.names.iter().any(|each| each == name));
        if imports_it {
            message.push_str(&format!(
                "; it imports `{name}`, and what a module imports is not visible through it"
            ));
        }
        self.error(ranges.target, message);
        None
    }
}
