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
//! about.

use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::{Hash32, Module};
use fidryn_syntax::e2::parse;
use fidryn_syntax::edition::edition_of;

mod lower;
mod module_check;
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
}

/// Where the parts of a lowered module stand in its source.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Spans {
    /// The module's address, after the word `module`.
    module: Span,
    /// Each provision of a unit, in document order.
    provisions: Vec<Declared>,
    /// The container of a container module.
    container: Option<Declared>,
    /// Each note, in the order of the module's notes.
    notes: Vec<Declared>,
    /// The values of `layers`: operative, enacted and editorial.
    layers: Option<[Value; 3]>,
    /// Each footnote marker: the footnote it names, and its span.
    marks: Vec<(u32, Span)>,
    /// Each definition, in the order of the module's definitions.
    definitions: Vec<DefinitionSpans>,
    imports: Vec<Span>,
    citations: Vec<Span>,
    /// The name of each citation.
    citation_names: Vec<Span>,
    bindings: Vec<Span>,
    /// The name of each binding.
    binding_names: Vec<Span>,
}

impl Spans {
    fn new(module: Span) -> Spans {
        Spans {
            module,
            provisions: Vec::new(),
            container: None,
            notes: Vec::new(),
            layers: None,
            marks: Vec::new(),
            definitions: Vec::new(),
            imports: Vec::new(),
            citations: Vec::new(),
            citation_names: Vec::new(),
            bindings: Vec::new(),
            binding_names: Vec::new(),
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
