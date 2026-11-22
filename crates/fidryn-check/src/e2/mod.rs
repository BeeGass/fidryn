//! Edition 2: lowering a module into the model of `fidryn_provision`, and
//! the checks of one module.
//!
//! [`lower`] turns the syntax tree that `fidryn_syntax::e2::parse` gives into
//! a [`fidryn_provision::Module`]. It judges what the parser leaves open and
//! the model needs: the shape of the module and the declarations that belong
//! to it (`E100`), every address (`E700`), the form of every hash and the
//! closed lists of a definition (`E710`), and the form of every text: no run
//! without words, normalized text, a topic of one word and no empty origin
//! of quoted content (`E710`). [`check_module`] then checks what holds
//! between the parts of the model: where each address stands (`E700`), the
//! hashes, the footnote markers and the definitions (`E710`), and the names
//! of a curated module (`E200`). [`check_source`] goes the whole way from
//! the text of a file. Every diagnostic carries the span of the text it is
//! about. [`names::resolve`] resolves the names of a curated module across
//! the modules of a workspace, from their interfaces alone.

use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::interface::{BindingRanges, ByteRange, CitationRanges, ImportRanges};
use fidryn_provision::{DeclSpans, Hash32, Module};
use fidryn_syntax::e2::parse;
use fidryn_syntax::edition::edition_of;

mod lower;
mod module_check;
pub mod names;
#[cfg(test)]
mod tests;

pub use lower::lower;
pub use module_check::check_module;

/// A module lowered into the model, with where its parts stand in its
/// source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lowered {
    pub module: Module,
    spans: Spans,
}

impl Lowered {
    /// The span of the module's address, after the word `module`.
    pub fn address_span(&self) -> Span {
        self.spans.module
    }

    /// The span of each `import` declaration, in the order of the module's
    /// imports.
    pub fn import_spans(&self) -> &[Span] {
        &self.spans.imports
    }

    /// The span of each `citation` declaration, in the order of the module's
    /// citations.
    pub fn citation_spans(&self) -> &[Span] {
        &self.spans.citations
    }

    /// The span of each `binding` declaration, in the order of the module's
    /// bindings.
    pub fn binding_spans(&self) -> &[Span] {
        &self.spans.bindings
    }

    /// The span of the address after `within`, when the module has one.
    pub fn within_span(&self) -> Option<Span> {
        self.spans.within
    }

    /// The span of the address after each `alias`, in the order of the
    /// module's aliases.
    pub fn alias_spans(&self) -> &[Span] {
        &self.spans.aliases
    }

    /// The span of the address after each `member` of a container, in the
    /// order of its members.
    pub fn member_spans(&self) -> &[Span] {
        &self.spans.members
    }

    /// The span of the address after the `region` of each definition, in the
    /// order of the module's definitions; `None` for one that has none.
    pub fn region_spans(&self) -> &[Option<Span>] {
        &self.spans.regions
    }

    /// The span of the address after the `see_note` of each definition, in
    /// the order of the module's definitions; `None` for one that has none.
    pub fn see_note_spans(&self) -> &[Option<Span>] {
        &self.spans.see_notes
    }

    /// The span of the address or the name after the `binds` of each
    /// binding, `alias.name` whole, in the order of the module's bindings.
    pub fn binds_spans(&self) -> &[Span] {
        &self.spans.binds
    }

    /// The span of the `provision_hash` value of each binding, in the order
    /// of the module's bindings.
    pub fn provision_hash_spans(&self) -> &[Span] {
        &self.spans.provision_hashes
    }

    /// The span of the `notes_hash` value of each binding, in the order of
    /// the module's bindings.
    pub fn notes_hash_spans(&self) -> &[Span] {
        &self.spans.notes_hashes
    }

    /// The byte ranges that the interface of the module records: of its
    /// address, of the address after `within`, after each `alias` and after
    /// each `member`, and of each `import`, `citation` and `binding`
    /// declaration with its parts, each list in declaration order.
    pub fn decl_spans(&self) -> DeclSpans {
        let spans = &self.spans;
        let ranges = |list: &[Span]| list.iter().copied().map(range).collect();
        let imports = spans
            .imports
            .iter()
            .zip(&spans.import_modules)
            .zip(&spans.import_aliases)
            .zip(&spans.import_names)
            .map(|(((whole, module), alias), names)| ImportRanges {
                whole: range(*whole),
                module: range(*module),
                alias: alias.map(range),
                names: ranges(names),
            })
            .collect();
        let citations = spans
            .citations
            .iter()
            .zip(&spans.citation_names)
            .map(|(whole, name)| CitationRanges {
                whole: range(*whole),
                name: range(*name),
            })
            .collect();
        let bindings = spans
            .bindings
            .iter()
            .zip(&spans.binding_names)
            .zip(&spans.qualifiers)
            .zip(&spans.bind_targets)
            .map(|(((whole, name), qualifier), target)| BindingRanges {
                whole: range(*whole),
                name: range(*name),
                qualifier: qualifier.map(range),
                target: range(*target),
            })
            .collect();
        DeclSpans {
            address: range(spans.module),
            within: spans.within.map(range),
            aliases: ranges(&spans.aliases),
            members: ranges(&spans.members),
            imports,
            citations,
            bindings,
        }
    }
}

/// `span` as a range of bytes of the source.
fn range(span: Span) -> ByteRange {
    ByteRange {
        start: span.start,
        end: span.end,
    }
}

/// Where the parts of a lowered module stand in its source. A span of a
/// string covers its quotation marks.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Spans {
    /// The module's address, after the word `module`.
    module: Span,
    within: Option<Span>,
    aliases: Vec<Span>,
    /// Each provision of a unit, in document order.
    provisions: Vec<Declared>,
    /// The container of a container module.
    container: Option<Declared>,
    members: Vec<Span>,
    /// Each note, in the order of the module's notes.
    notes: Vec<Declared>,
    /// The ordinal of each footnote, in the order of the module's footnotes.
    footnotes: Vec<Span>,
    /// The values of `layers`: operative, enacted and editorial.
    layers: Option<[Value; 3]>,
    /// Each footnote marker: the footnote it names, and its span.
    marks: Vec<(u32, Span)>,
    /// Each definition, in the order of the module's definitions.
    definitions: Vec<DefinitionSpans>,
    regions: Vec<Option<Span>>,
    see_notes: Vec<Option<Span>>,
    imports: Vec<Span>,
    /// The address after each `import`.
    import_modules: Vec<Span>,
    /// The alias after the `as` of each import; `None` for one with braces.
    import_aliases: Vec<Option<Span>>,
    /// Each name in the braces of each import.
    import_names: Vec<Vec<Span>>,
    citations: Vec<Span>,
    /// The name of each citation.
    citation_names: Vec<Span>,
    bindings: Vec<Span>,
    /// The name of each binding.
    binding_names: Vec<Span>,
    /// What each `binds` names, `alias.name` whole.
    binds: Vec<Span>,
    /// The alias of each `binds alias.name`; `None` for a name without one
    /// and for an address.
    qualifiers: Vec<Option<Span>>,
    /// What each `binds` names: the address, or the name after any alias.
    bind_targets: Vec<Span>,
    provision_hashes: Vec<Span>,
    notes_hashes: Vec<Span>,
}

impl Spans {
    fn new(module: Span) -> Spans {
        Spans {
            module,
            within: None,
            aliases: Vec::new(),
            provisions: Vec::new(),
            container: None,
            members: Vec::new(),
            notes: Vec::new(),
            footnotes: Vec::new(),
            layers: None,
            marks: Vec::new(),
            definitions: Vec::new(),
            regions: Vec::new(),
            see_notes: Vec::new(),
            imports: Vec::new(),
            import_modules: Vec::new(),
            import_aliases: Vec::new(),
            import_names: Vec::new(),
            citations: Vec::new(),
            citation_names: Vec::new(),
            bindings: Vec::new(),
            binding_names: Vec::new(),
            binds: Vec::new(),
            qualifiers: Vec::new(),
            bind_targets: Vec::new(),
            provision_hashes: Vec::new(),
            notes_hashes: Vec::new(),
        }
    }
}

/// A provision, a container or a note: its address and the hash it
/// declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Declared {
    address: Span,
    hash: Value,
}

/// A hash as the source declares it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Value {
    hash: Hash32,
    span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DefinitionSpans {
    at: Span,
    terms: Vec<Span>,
    /// The value of `words`, a phrase or the word `unstated`.
    words: Span,
    kind: Span,
}

/// Read `source` as a module of edition 2 and check it as one module: the
/// edition marker, the grammar, lowering and [`check_module`].
pub fn check_source(source: &str) -> Result<Lowered, Vec<Diagnostic>> {
    edition_of(source).map_err(|diagnostic| vec![diagnostic])?;
    let lowered = lower(&parse(source))?;
    let diagnostics = check_module(&lowered);
    if diagnostics.is_empty() {
        Ok(lowered)
    } else {
        Err(diagnostics)
    }
}

/// The `E100` of a command that compiles a module for evaluation, given a
/// module of edition 2.
pub fn not_compiled_for_evaluation() -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::E100,
        "an edition 2 module holds provisions and is not compiled for evaluation; \
         `fidryn check` reads it",
    )
}
