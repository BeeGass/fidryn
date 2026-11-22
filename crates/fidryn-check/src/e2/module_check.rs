//! The checks of one module over its lowered model.

use super::{Declared, DefinitionSpans, Lowered, Spans, Value};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::hash::{
    hash_container, hash_note, layer_hashes_of_container, layer_hashes_of_unit, tree_hashes,
};
use fidryn_provision::{
    Address, Block, ContainerModule, CuratedModule, Definition, DefinitionKind, Footnote, Hash32,
    Item, LayerHashes, Module, Note, PartRole, Provision, QuotedItem, Run, Text, UnitModule,
};
use std::collections::{BTreeMap, BTreeSet};

/// The checks of one module, in the order of the text they are about:
///
/// - the top provision of a unit has the module's address, every child
///   provision's address is below its parent's, every note's address is
///   below the module's, and no address is declared twice (`E700`);
/// - every `hash` and every value of `layers` is the hash computed from what
///   the module writes; the footnotes are numbered from 1, without a gap,
///   in the order they are written, and every footnote marker names a
///   footnote of the module; and every definition names a provision of the
///   unit whose text holds each term and the source words, whose text or the
///   chapeau of one of whose ancestors holds the reach words when they are
///   stated and the condition when there is one, and, when its kind is
///   `borrowed`, says what it borrows from in exactly one of `borrowed_from`
///   and `borrowed_words`, which a definition of any other kind does not
///   have (`E710`). Those texts are read without their footnote markers and
///   without their superscript runs made only of digits;
/// - no name of a curated module is declared twice, its citations and its
///   bindings sharing one set of names (`E200`).
pub fn check_module(lowered: &Lowered) -> Vec<Diagnostic> {
    let mut check = Check {
        spans: &lowered.spans,
        diagnostics: Vec::new(),
    };
    match &lowered.module {
        Module::Unit(unit) => check.unit(unit),
        Module::Container(container) => check.container(container),
        Module::Curated(curated) => check.curated(curated),
    }
    let mut diagnostics = check.diagnostics;
    diagnostics
        .sort_by_key(|diagnostic| diagnostic.primary_span.map(|span| (span.start, span.end)));
    diagnostics
}

/// A provision of a unit and its parent.
struct Placed<'a> {
    provision: &'a Provision,
    parent: Option<&'a Provision>,
}

/// Every provision of the tree below `root`, in document order.
fn in_document_order(root: &Provision) -> Vec<Placed<'_>> {
    fn walk<'a>(
        provision: &'a Provision,
        parent: Option<&'a Provision>,
        out: &mut Vec<Placed<'a>>,
    ) {
        out.push(Placed { provision, parent });
        for item in &provision.items {
            if let Item::Child(child) = item {
                walk(child, Some(provision), out);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, None, &mut out);
    out
}

/// Whether `address` lies below `other`, and is not `other` itself.
fn below(address: &Address, other: &Address) -> bool {
    address != other && address.is_within(other)
}

/// The word a module writes for `kind`.
fn kind_word(kind: DefinitionKind) -> &'static str {
    match kind {
        DefinitionKind::Means => "means",
        DefinitionKind::Includes => "includes",
        DefinitionKind::DoesNotInclude => "does_not_include",
        DefinitionKind::Borrowed => "borrowed",
    }
}

/// Whether `run` is a superscript run whose words are only the digits `0` to
/// `9`, at least one: a footnote number marked as a superscript.
fn superscript_number(run: &Run) -> bool {
    match run {
        Run::Sup(words) => !words.is_empty() && words.bytes().all(|byte| byte.is_ascii_digit()),
        _ => false,
    }
}

/// The words a run gives to the text the checks of a definition read: none
/// for a footnote marker and for a superscript number.
fn read_words(run: &Run) -> &str {
    match run {
        Run::Mark { .. } => "",
        run if superscript_number(run) => "",
        Run::Words(words) | Run::Ref { words, .. } | Run::Sup(words) | Run::Sub(words) => words,
    }
}

/// The text of `text` that the checks of a definition read: its runs in
/// order, without its footnote markers and without its superscript runs
/// made only of the digits `0` to `9`. With each such run one space goes
/// too: the space just before it, when just after it there is a space or
/// the end of the text; or, when it begins the text, the space just after
/// it. A digit written in any other run is text.
pub(super) fn definition_text(text: &Text) -> String {
    let mut out = String::new();
    // A number began the text, and the space just after it goes.
    let mut space_after = false;
    for (index, run) in text.runs.iter().enumerate() {
        if superscript_number(run) {
            if out.is_empty() {
                space_after = true;
            } else if out.ends_with(' ') {
                let after = text.runs[index + 1..]
                    .iter()
                    .map(read_words)
                    .find(|words| !words.is_empty());
                if after.is_none_or(|words| words.starts_with(' ')) {
                    out.pop();
                }
            }
            continue;
        }
        let mut words = read_words(run);
        if space_after && !words.is_empty() {
            words = words.strip_prefix(' ').unwrap_or(words);
            space_after = false;
        }
        out.push_str(words);
    }
    out
}

/// The text a definition is read in (see [`definition_text`]) of each
/// paragraph and subheading of the text parts of `provision` whose role
/// `wanted` accepts, those in its tables and quoted content included, and
/// nothing of its children.
fn texts_of(provision: &Provision, wanted: impl Fn(PartRole) -> bool) -> Vec<String> {
    fn blocks(list: &[Block], out: &mut Vec<String>) {
        for block in list {
            match block {
                Block::Para(text) | Block::Subheading(text) => out.push(definition_text(text)),
                Block::Table(table) => {
                    for cell in table.rows.iter().flat_map(|row| &row.cells) {
                        blocks(&cell.blocks, out);
                    }
                }
                Block::Quoted(quoted) => items(&quoted.items, out),
                Block::Image { .. } => {}
            }
        }
    }
    fn items(list: &[QuotedItem], out: &mut Vec<String>) {
        for item in list {
            match item {
                QuotedItem::Block(block) => blocks(std::slice::from_ref(block), out),
                QuotedItem::Part { blocks: inner, .. } => blocks(inner, out),
                QuotedItem::Level(level) => items(&level.items, out),
            }
        }
    }
    let mut out = Vec::new();
    for item in &provision.items {
        if let Item::Part {
            role,
            blocks: inner,
        } = item
            && wanted(*role)
        {
            blocks(inner, &mut out);
        }
    }
    out
}

struct Check<'a> {
    spans: &'a Spans,
    diagnostics: Vec<Diagnostic>,
}

impl Check<'_> {
    fn error(&mut self, code: DiagnosticCode, span: Span, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::new(code, message).with_span(span));
    }

    fn unit(&mut self, unit: &UnitModule) {
        let spans = self.spans;
        let provisions = in_document_order(&unit.root);
        if unit.root.address != unit.address
            && let Some(root) = spans.provisions.first()
        {
            self.error(
                DiagnosticCode::E700,
                root.address,
                format!(
                    "the top provision's address `{}` is not the module's address `{}`",
                    unit.root.address, unit.address
                ),
            );
        }
        for (placed, declared) in provisions.iter().zip(&spans.provisions) {
            if let Some(parent) = placed.parent
                && !below(&placed.provision.address, &parent.address)
            {
                self.error(
                    DiagnosticCode::E700,
                    declared.address,
                    format!(
                        "the address `{}` is not below the address `{}` of its parent provision",
                        placed.provision.address, parent.address
                    ),
                );
            }
        }
        self.notes_below(&unit.address, &unit.notes);
        let declared: Vec<(&Address, &Declared)> = provisions
            .iter()
            .map(|placed| &placed.provision.address)
            .zip(&spans.provisions)
            .chain(
                unit.notes
                    .iter()
                    .map(|note| &note.address)
                    .zip(&spans.notes),
            )
            .collect();
        self.declared_once(&declared);

        let hashes = tree_hashes(&unit.root);
        for ((placed, declared), computed) in provisions.iter().zip(&spans.provisions).zip(hashes) {
            self.hash(
                declared.hash,
                computed,
                &format!("provision `{}`", placed.provision.address),
            );
        }
        self.note_hashes(&unit.notes);
        self.layers(&layer_hashes_of_unit(unit));
        self.footnote_numbers(&unit.footnotes);
        self.marks(&unit.footnotes);
        self.definitions(unit, &provisions);
    }

    fn container(&mut self, container: &ContainerModule) {
        let spans = self.spans;
        self.notes_below(&container.address, &container.notes);
        let declared: Vec<(&Address, &Declared)> = container
            .notes
            .iter()
            .map(|note| &note.address)
            .zip(&spans.notes)
            .collect();
        self.declared_once(&declared);
        if let Some(declared) = &spans.container {
            self.hash(
                declared.hash,
                hash_container(container),
                &format!("container `{}`", container.address),
            );
        }
        self.note_hashes(&container.notes);
        self.layers(&layer_hashes_of_container(container));
        self.footnote_numbers(&container.footnotes);
        self.marks(&container.footnotes);
    }

    /// Citations and bindings share one set of names: each name declared
    /// earlier in the source is reported where it is declared again.
    fn curated(&mut self, curated: &CuratedModule) {
        let spans = self.spans;
        let mut names: Vec<(&str, Span)> = curated
            .citations
            .iter()
            .map(|citation| citation.name.as_str())
            .zip(spans.citation_names.iter().copied())
            .chain(
                curated
                    .bindings
                    .iter()
                    .map(|binding| binding.name.as_str())
                    .zip(spans.binding_names.iter().copied()),
            )
            .collect();
        names.sort_by_key(|(_, span)| span.start);
        self.named_once(&names);
    }

    /// Each footnote is numbered one more than the one written before it,
    /// the first 1.
    fn footnote_numbers(&mut self, footnotes: &[Footnote]) {
        let mut previous = 0_u64;
        for (footnote, span) in footnotes.iter().zip(&self.spans.footnotes) {
            let next = previous + 1;
            let ordinal = u64::from(footnote.ordinal);
            if ordinal != next {
                self.error(
                    DiagnosticCode::E710,
                    *span,
                    format!(
                        "this footnote is numbered {ordinal} where {next} is next; the footnotes \
                         are numbered from 1, without a gap, in the order they are written"
                    ),
                );
            }
            previous = ordinal;
        }
    }

    fn notes_below(&mut self, module: &Address, notes: &[Note]) {
        for (note, declared) in notes.iter().zip(&self.spans.notes) {
            if !below(&note.address, module) {
                self.error(
                    DiagnosticCode::E700,
                    declared.address,
                    format!(
                        "the note's address `{}` is not below the module's address `{module}`",
                        note.address
                    ),
                );
            }
        }
    }

    /// Reports each address of `declared` that stands earlier in the source.
    fn declared_once(&mut self, declared: &[(&Address, &Declared)]) {
        let mut in_order: Vec<(&Address, Span)> = declared
            .iter()
            .map(|(address, declared)| (*address, declared.address))
            .collect();
        in_order.sort_by_key(|(_, span)| span.start);
        let mut first: BTreeMap<&Address, Span> = BTreeMap::new();
        for (address, span) in in_order {
            match first.get(address) {
                Some(earlier) => {
                    let mut diagnostic = Diagnostic::new(
                        DiagnosticCode::E700,
                        format!("the address `{address}` is declared twice"),
                    )
                    .with_span(span);
                    diagnostic.related_spans.push(*earlier);
                    self.diagnostics.push(diagnostic);
                }
                None => {
                    first.insert(address, span);
                }
            }
        }
    }

    /// Reports each name of `names`, which are in the order of the source,
    /// that an earlier one declares.
    fn named_once(&mut self, names: &[(&str, Span)]) {
        let mut first: BTreeMap<&str, Span> = BTreeMap::new();
        for &(name, span) in names {
            match first.get(name) {
                Some(earlier) => {
                    let mut diagnostic = Diagnostic::new(
                        DiagnosticCode::E200,
                        format!("the name `{name}` is declared twice"),
                    )
                    .with_span(span);
                    diagnostic.related_spans.push(*earlier);
                    self.diagnostics.push(diagnostic);
                }
                None => {
                    first.insert(name, span);
                }
            }
        }
    }

    fn hash(&mut self, declared: Value, computed: Hash32, what: &str) {
        if declared.hash != computed {
            self.error(
                DiagnosticCode::E710,
                declared.span,
                format!(
                    "the `hash` of {what} does not match what the module writes; the hash \
                     computed from it is `{computed}`"
                ),
            );
        }
    }

    fn note_hashes(&mut self, notes: &[Note]) {
        for (note, declared) in notes.iter().zip(&self.spans.notes) {
            self.hash(
                declared.hash,
                hash_note(note),
                &format!("note `{}`", note.address),
            );
        }
    }

    fn layers(&mut self, computed: &LayerHashes) {
        let Some([operative, enacted, editorial]) = self.spans.layers else {
            return;
        };
        for (name, declared, computed) in [
            ("operative", operative, computed.operative),
            ("enacted", enacted, computed.enacted),
            ("editorial", editorial, computed.editorial),
        ] {
            if declared.hash != computed {
                self.error(
                    DiagnosticCode::E710,
                    declared.span,
                    format!(
                        "the `{name}` value of `layers` does not match what the module writes; \
                         the hash computed from it is `{computed}`"
                    ),
                );
            }
        }
    }

    fn marks(&mut self, footnotes: &[Footnote]) {
        let ordinals: BTreeSet<u32> = footnotes.iter().map(|footnote| footnote.ordinal).collect();
        for &(footnote, span) in &self.spans.marks {
            if !ordinals.contains(&footnote) {
                self.error(
                    DiagnosticCode::E710,
                    span,
                    format!(
                        "the footnote marker names footnote {footnote}, and the module has no \
                         footnote {footnote}"
                    ),
                );
            }
        }
    }

    fn definitions(&mut self, unit: &UnitModule, provisions: &[Placed<'_>]) {
        let mut by_address: BTreeMap<&Address, &Placed<'_>> = BTreeMap::new();
        for placed in provisions {
            by_address
                .entry(&placed.provision.address)
                .or_insert(placed);
        }
        for (definition, spans) in unit.definitions.iter().zip(&self.spans.definitions) {
            let at = &definition.at;
            let Some(placed) = by_address.get(at) else {
                self.error(
                    DiagnosticCode::E710,
                    spans.at,
                    format!("the definition names `{at}`, which is no provision of this module"),
                );
                continue;
            };
            let texts = texts_of(placed.provision, |_| true);
            let stands =
                |texts: &[String], words: &str| texts.iter().any(|text| text.contains(words));
            for (term, span) in definition.terms.iter().zip(&spans.terms) {
                if !stands(&texts, term) {
                    self.error(
                        DiagnosticCode::E710,
                        *span,
                        format!("the term `{term}` does not stand in the text of provision `{at}`"),
                    );
                }
            }
            if definition.words.is_some() || definition.condition.is_some() {
                // The chapeaus of the ancestors, the parent first. Each
                // ancestor is found by its address, which no other provision
                // of a module that passes the checks of its addresses has.
                let mut chapeaus = Vec::new();
                let mut ancestor = placed.parent;
                while let Some(provision) = ancestor {
                    chapeaus.extend(texts_of(provision, |role| role == PartRole::Chapeau));
                    ancestor = by_address
                        .get(&provision.address)
                        .and_then(|placed| placed.parent);
                }
                if let Some(words) = &definition.words
                    && !stands(&texts, words)
                    && !stands(&chapeaus, words)
                {
                    self.error(
                        DiagnosticCode::E710,
                        spans.words,
                        format!(
                            "the words `{words}` stand neither in the text of provision `{at}` \
                             nor in the chapeau of one of its ancestors"
                        ),
                    );
                }
                if let Some(condition) = &definition.condition
                    && !stands(&texts, condition)
                    && !stands(&chapeaus, condition)
                {
                    self.error(
                        DiagnosticCode::E710,
                        spans.condition.unwrap_or(spans.words),
                        format!(
                            "the condition `{condition}` stands neither in the text of provision \
                             `{at}` nor in the chapeau of one of its ancestors"
                        ),
                    );
                }
            }
            if let Some(source) = &definition.borrowed_words
                && !stands(&texts, source)
            {
                self.error(
                    DiagnosticCode::E710,
                    spans.borrowed_words.unwrap_or(spans.kind),
                    format!(
                        "the source words `{source}` do not stand in the text of provision `{at}`"
                    ),
                );
            }
            self.source_of(definition, spans);
        }
    }

    /// A definition of kind `borrowed` has exactly one of `borrowed_from`
    /// and `borrowed_words`, and one of any other kind has neither.
    fn source_of(&mut self, definition: &Definition, spans: &DefinitionSpans) {
        let linked = spans.borrowed_from.unwrap_or(spans.kind);
        let unlinked = spans.borrowed_words.unwrap_or(spans.kind);
        match (
            definition.kind,
            definition.borrowed_from.is_some(),
            definition.borrowed_words.is_some(),
        ) {
            (DefinitionKind::Borrowed, false, false) => self.error(
                DiagnosticCode::E710,
                spans.kind,
                "a definition of kind `borrowed` has neither `borrowed_from` nor `borrowed_words`",
            ),
            (DefinitionKind::Borrowed, true, true) => self.error(
                DiagnosticCode::E710,
                unlinked,
                "a definition of kind `borrowed` has both `borrowed_from` and `borrowed_words`",
            ),
            (DefinitionKind::Borrowed, _, _) => {}
            (kind, from, words) => {
                let kind = kind_word(kind);
                for (given, field, span) in [
                    (from, "borrowed_from", linked),
                    (words, "borrowed_words", unlinked),
                ] {
                    if given {
                        self.error(
                            DiagnosticCode::E710,
                            span,
                            format!(
                                "a definition of kind `{kind}` has `{field}`, which only a \
                                 definition of kind `borrowed` has"
                            ),
                        );
                    }
                }
            }
        }
    }
}
