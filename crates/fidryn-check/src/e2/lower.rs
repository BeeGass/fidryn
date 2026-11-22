//! Lowering: from the syntax tree of an edition-2 module to the model.
//!
//! Lowering reports an error and goes on with a stand-in value, so that one
//! pass reports every error it can find; a module with any error gives its
//! diagnostics and no model.

use super::{Declared, DefinitionSpans, Lowered, Spans, Value};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::normalize::{WHITE_SPACE, is_normalized, normalize};
use fidryn_provision::{
    Address, Basis, BindTarget, Binding, Block, Cell, Citation, ContainerModule, CuratedModule,
    Definition, DefinitionKind, Footnote, Hash32, Import, Item, Layer, Module, Note, Num, PartRole,
    Provision, Quoted, QuotedItem, QuotedLevel, Row, RowGroup, Run, Table, Text, UnitModule,
};
use fidryn_syntax::e2::{Parse, ast};

/// Lower a parsed module of edition 2 into the model.
///
/// The parse's own diagnostics are returned as they are. Otherwise lowering
/// judges what the grammar leaves open and the model needs:
///
/// - the shape of the module: a unit has one `provision` at the top, a
///   container module one `container`, a curated module neither; every
///   declaration belongs to its shape; `within`, `source`, `positive_law`,
///   `credit` and `layers` stand at most once, and a unit or a container has
///   a `source` and `layers` (`E100`);
/// - every address, and the address of a container, which is the module's
///   (`E700`);
/// - the form of every hash, the kind and the basis of every definition, and
///   that a container module, which holds no provision, holds no definition
///   (`E710`);
/// - the form of every text: no run without words, except that `td ""` is a
///   cell with no block and the shown form `""` of a number is the empty
///   text; the text of every block normalized; every topic one word (`E710`);
/// - no empty string where no value may be empty: a term, stated reach
///   words, `borrowed_from`, `source`, the release of a binding, and the
///   origin of quoted content (`E710`). The value of a number, the shown
///   form of a footnote and of a marker, the source of an image and the
///   target of a link may be empty.
pub fn lower(parse: &Parse) -> Result<Lowered, Vec<Diagnostic>> {
    let Some(module) = &parse.module else {
        return Err(parse.diagnostics.clone());
    };
    if !parse.diagnostics.is_empty() {
        return Err(parse.diagnostics.clone());
    }
    let mut lowering = Lowering {
        diagnostics: Vec::new(),
        spans: Spans::new(module.address.span),
    };
    let lowered = lowering.module(module);
    if lowering.diagnostics.is_empty() {
        Ok(Lowered {
            module: lowered,
            spans: lowering.spans,
        })
    } else {
        let mut diagnostics = lowering.diagnostics;
        diagnostics
            .sort_by_key(|diagnostic| diagnostic.primary_span.map(|span| (span.start, span.end)));
        Err(diagnostics)
    }
}

/// The address that stands in for one that is no address.
fn stand_in_address() -> Address {
    Address::parse("/stand-in").expect("a stand-in address")
}

/// What the top of a module makes it.
#[derive(Clone, Copy)]
enum Shape<'a> {
    Unit(&'a ast::Provision),
    Container(&'a ast::Container),
    Curated,
}

impl Shape<'_> {
    fn name(self) -> &'static str {
        match self {
            Shape::Unit(_) => "a unit",
            Shape::Container(_) => "a container module",
            Shape::Curated => "a curated module",
        }
    }
}

/// The declarations that stand at most once in a module.
const ONCE: [&str; 5] = ["within", "source", "positive_law", "credit", "layers"];

fn word(decl: &ast::Decl) -> &'static str {
    match decl {
        ast::Decl::Alias(_) => "alias",
        ast::Decl::Within(_) => "within",
        ast::Decl::Source(_) => "source",
        ast::Decl::PositiveLaw(_) => "positive_law",
        ast::Decl::Provision(_) => "provision",
        ast::Decl::Container(_) => "container",
        ast::Decl::Credit(_) => "credit",
        ast::Decl::Note(_) => "note",
        ast::Decl::Footnote(_) => "footnote",
        ast::Decl::Definition(_) => "definition",
        ast::Decl::Layers(_) => "layers",
        ast::Decl::Import(_) => "import",
        ast::Decl::Citation(_) => "citation",
        ast::Decl::Binding(_) => "binding",
    }
}

/// Whether `text` is written as exactly one empty string.
fn is_empty_string(text: &ast::Text) -> bool {
    matches!(text.runs.as_slice(), [ast::Run::Words(words)] if words.value.is_empty())
}

fn role(role: ast::Role) -> PartRole {
    match role {
        ast::Role::Chapeau => PartRole::Chapeau,
        ast::Role::Content => PartRole::Content,
        ast::Role::Continuation => PartRole::Continuation,
        ast::Role::Proviso => PartRole::Proviso,
    }
}

/// What a unit and a container module hold besides their top.
#[derive(Default)]
struct Matter {
    aliases: Vec<Address>,
    within: Option<Address>,
    source: Option<String>,
    positive_law: Option<bool>,
    credit: Option<Text>,
    notes: Vec<Note>,
    footnotes: Vec<Footnote>,
    definitions: Vec<Definition>,
}

struct Lowering {
    diagnostics: Vec<Diagnostic>,
    spans: Spans,
}

impl Lowering {
    fn error(&mut self, code: DiagnosticCode, span: Span, message: impl Into<String>) {
        self.diagnostics
            .push(Diagnostic::new(code, message).with_span(span));
    }

    fn module(&mut self, module: &ast::Module) -> Module {
        let address = self.address(&module.address);
        let shape = self.shape(module);
        let address_or_stand_in = address.clone().unwrap_or_else(stand_in_address);
        match shape {
            Shape::Curated => Module::Curated(self.curated(address_or_stand_in, &module.decls)),
            Shape::Unit(top) => {
                let root = self.provision(top);
                let matter = self.matter(&module.decls, shape);
                Module::Unit(UnitModule {
                    address: address_or_stand_in,
                    aliases: matter.aliases,
                    within: matter.within,
                    source: matter.source.unwrap_or_default(),
                    positive_law: matter.positive_law,
                    root,
                    credit: matter.credit,
                    notes: matter.notes,
                    footnotes: matter.footnotes,
                    definitions: matter.definitions,
                })
            }
            Shape::Container(top) => {
                let declared = self.address(&top.address);
                if let (Some(module_address), Some(declared)) = (&address, &declared)
                    && module_address != declared
                {
                    self.error(
                        DiagnosticCode::E700,
                        top.address.span,
                        format!(
                            "the container's address `{declared}` is not the module's address \
                             `{module_address}`"
                        ),
                    );
                }
                let hash = self.hash(&top.hash);
                self.spans.container = Some(Declared {
                    address: top.address.span,
                    hash,
                });
                let num = top.num.as_ref().map(|num| self.num(num));
                let heading = top.heading.as_ref().map(|heading| self.text(heading));
                let members = top
                    .members
                    .iter()
                    .map(|member| {
                        self.spans.members.push(member.span);
                        self.address_or_stand_in(member)
                    })
                    .collect();
                let matter = self.matter(&module.decls, shape);
                Module::Container(ContainerModule {
                    address: address_or_stand_in,
                    aliases: matter.aliases,
                    within: matter.within,
                    source: matter.source.unwrap_or_default(),
                    positive_law: matter.positive_law,
                    kind: top.kind.text.clone(),
                    num,
                    heading,
                    status: top.status.as_ref().map(|status| status.text.clone()),
                    members,
                    credit: matter.credit,
                    notes: matter.notes,
                    footnotes: matter.footnotes,
                })
            }
        }
    }

    /// The shape of `module`, after reporting every declaration that does
    /// not fit it, every declaration given twice, and every one a unit or a
    /// container lacks.
    fn shape<'a>(&mut self, module: &'a ast::Module) -> Shape<'a> {
        let mut shape = Shape::Curated;
        for decl in &module.decls {
            let message = match (shape, decl) {
                (Shape::Curated, ast::Decl::Provision(provision)) => {
                    shape = Shape::Unit(provision);
                    continue;
                }
                (Shape::Curated, ast::Decl::Container(container)) => {
                    shape = Shape::Container(container);
                    continue;
                }
                (Shape::Unit(_), ast::Decl::Provision(_)) => {
                    "a unit has one `provision` at the top, and this is a second one"
                }
                (Shape::Container(_), ast::Decl::Container(_)) => {
                    "a container module has one `container`, and this is a second one"
                }
                (_, ast::Decl::Provision(_) | ast::Decl::Container(_)) => {
                    "a module has one `provision` or one `container` at the top, not both"
                }
                _ => continue,
            };
            self.error(DiagnosticCode::E100, decl.span(), message);
        }
        let mut seen: Vec<&'static str> = Vec::new();
        for decl in &module.decls {
            let word = word(decl);
            let curated_only = matches!(
                decl,
                ast::Decl::Import(_) | ast::Decl::Citation(_) | ast::Decl::Binding(_)
            );
            if matches!(decl, ast::Decl::Provision(_) | ast::Decl::Container(_)) {
                continue;
            }
            let curated = matches!(shape, Shape::Curated);
            if curated_only && !curated {
                self.error(
                    DiagnosticCode::E100,
                    decl.span(),
                    format!(
                        "`{word}` belongs to curated modules, and this module is {}",
                        shape.name()
                    ),
                );
            } else if !curated_only && curated {
                self.error(
                    DiagnosticCode::E100,
                    decl.span(),
                    format!(
                        "`{word}` belongs to units and containers, and this module is curated: \
                         it has no `provision` and no `container` at the top"
                    ),
                );
            } else if ONCE.contains(&word) {
                if seen.contains(&word) {
                    self.error(
                        DiagnosticCode::E100,
                        decl.span(),
                        format!("declaration `{word}` is given twice"),
                    );
                } else {
                    seen.push(word);
                }
            }
        }
        if !matches!(shape, Shape::Curated) {
            let close = Span {
                start: module.span.end.saturating_sub(1),
                end: module.span.end,
            };
            for word in ["source", "layers"] {
                if !seen.contains(&word) {
                    self.error(
                        DiagnosticCode::E100,
                        close,
                        format!("missing declaration `{word}` in {}", shape.name()),
                    );
                }
            }
        }
        shape
    }

    /// The declarations of a unit or a container module other than its top.
    /// Those the shape reported are passed over.
    fn matter(&mut self, decls: &[ast::Decl], shape: Shape<'_>) -> Matter {
        let mut matter = Matter::default();
        let mut layers = false;
        for decl in decls {
            match decl {
                ast::Decl::Alias(alias) => {
                    let address = self.address_or_stand_in(&alias.address);
                    matter.aliases.push(address);
                    self.spans.aliases.push(alias.address.span);
                }
                ast::Decl::Within(within) if matter.within.is_none() => {
                    matter.within = Some(self.address_or_stand_in(&within.address));
                    self.spans.within = Some(within.address.span);
                }
                ast::Decl::Source(source) if matter.source.is_none() => {
                    self.not_empty(&source.name, "the source");
                    matter.source = Some(source.name.value.clone());
                }
                ast::Decl::PositiveLaw(positive_law) if matter.positive_law.is_none() => {
                    matter.positive_law = Some(positive_law.value.value);
                }
                ast::Decl::Credit(credit) if matter.credit.is_none() => {
                    matter.credit = Some(self.text(&credit.text));
                }
                ast::Decl::Note(note) => {
                    let note = self.note(note);
                    matter.notes.push(note);
                }
                ast::Decl::Footnote(footnote) => {
                    self.spans.footnotes.push(footnote.ordinal.span);
                    let footnote = Footnote {
                        ordinal: footnote.ordinal.value,
                        shown: footnote.shown.value.clone(),
                        blocks: self.blocks(&footnote.blocks),
                    };
                    matter.footnotes.push(footnote);
                }
                ast::Decl::Definition(definition) => match shape {
                    Shape::Unit(_) => {
                        let definition = self.definition(definition);
                        matter.definitions.push(definition);
                    }
                    _ => self.error(
                        DiagnosticCode::E710,
                        definition.at.span,
                        format!(
                            "a container module holds no provision, so the provision `{}` that \
                             this definition names is not in this module",
                            definition.at.value
                        ),
                    ),
                },
                ast::Decl::Layers(values) if !layers => {
                    layers = true;
                    let operative = self.hash(&values.operative);
                    let enacted = self.hash(&values.enacted);
                    let editorial = self.hash(&values.editorial);
                    self.spans.layers = Some([operative, enacted, editorial]);
                }
                _ => {}
            }
        }
        matter
    }

    fn curated(&mut self, address: Address, decls: &[ast::Decl]) -> CuratedModule {
        let mut curated = CuratedModule {
            address,
            imports: Vec::new(),
            citations: Vec::new(),
            bindings: Vec::new(),
        };
        for decl in decls {
            match decl {
                ast::Decl::Import(import) => {
                    let module = self.address_or_stand_in(&import.module);
                    let (alias, names): (Option<&ast::Ident>, &[ast::Ident]) = match &import.form {
                        ast::ImportForm::As(alias) => (Some(alias), &[]),
                        ast::ImportForm::Names(names) => (None, names),
                    };
                    curated.imports.push(Import {
                        module,
                        alias: alias.map(|alias| alias.text.clone()),
                        names: names.iter().map(|name| name.text.clone()).collect(),
                    });
                    self.spans.imports.push(import.span);
                    self.spans.import_modules.push(import.module.span);
                    self.spans
                        .import_aliases
                        .push(alias.map(|alias| alias.span));
                    self.spans
                        .import_names
                        .push(names.iter().map(|name| name.span).collect());
                }
                ast::Decl::Citation(citation) => {
                    let target = self.address_or_stand_in(&citation.target);
                    curated.citations.push(Citation {
                        name: citation.name.text.clone(),
                        target,
                    });
                    self.spans.citations.push(citation.span);
                    self.spans.citation_names.push(citation.name.span);
                }
                ast::Decl::Binding(binding) => {
                    let (binds, qualifier, target) = match &binding.binds {
                        ast::BindTarget::Address(address) => (
                            BindTarget::Address(self.address_or_stand_in(address)),
                            None,
                            address.span,
                        ),
                        ast::BindTarget::Name {
                            qualifier, name, ..
                        } => (
                            BindTarget::Name {
                                module: qualifier.as_ref().map(|qualifier| qualifier.text.clone()),
                                name: name.text.clone(),
                            },
                            qualifier.as_ref().map(|qualifier| qualifier.span),
                            name.span,
                        ),
                    };
                    self.spans.qualifiers.push(qualifier);
                    self.spans.bind_targets.push(target);
                    self.not_empty(&binding.release, "the release of a binding");
                    let provision_hash = self.hash(&binding.provision_hash).hash;
                    let notes_hash = self.hash(&binding.notes_hash).hash;
                    curated.bindings.push(Binding {
                        name: binding.name.text.clone(),
                        binds,
                        release: binding.release.value.clone(),
                        provision_hash,
                        notes_hash,
                    });
                    self.spans.bindings.push(binding.span);
                    self.spans.binding_names.push(binding.name.span);
                    self.spans.binds.push(binding.binds.span());
                    self.spans
                        .provision_hashes
                        .push(binding.provision_hash.span);
                    self.spans.notes_hashes.push(binding.notes_hash.span);
                }
                _ => {}
            }
        }
        curated
    }

    /// The address `value` writes, or `None` after an `E700`.
    fn address(&mut self, value: &ast::Str) -> Option<Address> {
        match Address::parse(&value.value) {
            Ok(address) => Some(address),
            Err(error) => {
                self.error(
                    DiagnosticCode::E700,
                    value.span,
                    format!("`{}` is no address: {error}", value.value),
                );
                None
            }
        }
    }

    fn address_or_stand_in(&mut self, value: &ast::Str) -> Address {
        self.address(value).unwrap_or_else(stand_in_address)
    }

    /// The hash `value` writes; after an `E710`, a stand-in.
    fn hash(&mut self, value: &ast::Str) -> Value {
        let hash = match value.value.parse::<Hash32>() {
            Ok(hash) => hash,
            Err(error) => {
                self.error(
                    DiagnosticCode::E710,
                    value.span,
                    format!("`{}` is no hash: {error}", value.value),
                );
                Hash32([0; 32])
            }
        };
        Value {
            hash,
            span: value.span,
        }
    }

    /// `provision` and the tree below it. The spans of each provision are
    /// recorded in document order: a provision before its children.
    fn provision(&mut self, provision: &ast::Provision) -> Provision {
        let address = self.address_or_stand_in(&provision.address);
        let hash = self.hash(&provision.hash);
        self.spans.provisions.push(Declared {
            address: provision.address.span,
            hash,
        });
        let num = provision.num.as_ref().map(|num| self.num(num));
        let heading = provision.heading.as_ref().map(|heading| self.text(heading));
        let items = provision
            .items
            .iter()
            .map(|item| match item {
                ast::ProvisionItem::Part(part) => Item::Part {
                    role: role(part.role.value),
                    blocks: self.part_blocks(&part.body),
                },
                ast::ProvisionItem::Provision(child) => Item::Child(self.provision(child)),
            })
            .collect();
        Provision {
            address,
            kind: provision.kind.text.clone(),
            num,
            heading,
            status: provision.status.as_ref().map(|status| status.text.clone()),
            items,
        }
    }

    /// A number. Its shown form `""` is the empty text.
    fn num(&mut self, num: &ast::Num) -> Num {
        let shown = if is_empty_string(&num.shown) {
            Text::default()
        } else {
            self.text(&num.shown)
        };
        Num {
            value: num.value.value.clone(),
            shown,
        }
    }

    /// The blocks of a part: the short form is one paragraph.
    fn part_blocks(&mut self, body: &ast::Body) -> Vec<Block> {
        match body {
            ast::Body::Text(text) => vec![Block::Para(self.text(text))],
            ast::Body::Blocks { blocks, .. } => self.blocks(blocks),
        }
    }

    fn blocks(&mut self, blocks: &[ast::Block]) -> Vec<Block> {
        blocks.iter().map(|block| self.block(block)).collect()
    }

    fn block(&mut self, block: &ast::Block) -> Block {
        match block {
            ast::Block::Para { text, .. } => Block::Para(self.text(text)),
            ast::Block::Subheading { text, .. } => Block::Subheading(self.text(text)),
            ast::Block::Table(table) => Block::Table(Table {
                rows: table.rows.iter().map(|row| self.row(row)).collect(),
            }),
            ast::Block::Quoted(quoted) => Block::Quoted(self.quoted(quoted)),
            ast::Block::Image { source, .. } => Block::Image {
                source: source.value.clone(),
            },
        }
    }

    fn row(&mut self, row: &ast::Row) -> Row {
        let group = match row.group.value {
            ast::RowGroup::Head => RowGroup::Head,
            ast::RowGroup::Body => RowGroup::Body,
            ast::RowGroup::Foot => RowGroup::Foot,
        };
        let cells = row
            .cells
            .iter()
            .map(|cell| {
                let blocks = match &cell.body {
                    ast::Body::Text(text) if is_empty_string(text) => Vec::new(),
                    body => self.part_blocks(body),
                };
                Cell {
                    header: cell.kind.value == ast::CellKind::Header,
                    colspan: cell.spanning.map_or(1, |spanning| spanning.colspan.value),
                    rowspan: cell.spanning.map_or(1, |spanning| spanning.rowspan.value),
                    blocks,
                }
            })
            .collect();
        Row { group, cells }
    }

    fn quoted(&mut self, quoted: &ast::Quoted) -> Quoted {
        if let Some(origin) = &quoted.origin
            && origin.value.is_empty()
        {
            self.error(
                DiagnosticCode::E710,
                origin.span,
                "the origin of quoted content is an empty string; quoted content with no \
                 origin names none",
            );
        }
        Quoted {
            origin: quoted.origin.as_ref().map(|origin| origin.value.clone()),
            items: self.quoted_items(&quoted.items),
        }
    }

    fn quoted_items(&mut self, items: &[ast::QItem]) -> Vec<QuotedItem> {
        items
            .iter()
            .map(|item| match item {
                ast::QItem::Block(block) => QuotedItem::Block(self.block(block)),
                ast::QItem::Part(part) => QuotedItem::Part {
                    role: role(part.role.value),
                    blocks: self.part_blocks(&part.body),
                },
                ast::QItem::Level(level) => QuotedItem::Level(QuotedLevel {
                    kind: level.kind.text.clone(),
                    num: level.num.as_ref().map(|num| self.num(num)),
                    heading: level.heading.as_ref().map(|heading| self.text(heading)),
                    items: self.quoted_items(&level.items),
                }),
            })
            .collect()
    }

    fn note(&mut self, note: &ast::Note) -> Note {
        let address = self.address_or_stand_in(&note.address);
        let hash = self.hash(&note.hash);
        self.spans.notes.push(Declared {
            address: note.address.span,
            hash,
        });
        for topic in &note.topics {
            if topic.value.is_empty() || topic.value.contains(WHITE_SPACE) {
                self.error(DiagnosticCode::E710, topic.span, "a topic is one word");
            }
        }
        Note {
            address,
            layer: match note.layer.value {
                ast::NoteLayer::Enacted => Layer::Enacted,
                ast::NoteLayer::Editorial => Layer::Editorial,
            },
            group: note.group.text.clone(),
            topics: note
                .topics
                .iter()
                .map(|topic| topic.value.clone())
                .collect(),
            heading: note.heading.as_ref().map(|heading| self.text(heading)),
            blocks: self.blocks(&note.blocks),
        }
    }

    fn definition(&mut self, definition: &ast::Definition) -> Definition {
        let at = self.address_or_stand_in(&definition.at);
        let kind = match definition.kind.text.as_str() {
            "means" => DefinitionKind::Means,
            "includes" => DefinitionKind::Includes,
            "does_not_include" => DefinitionKind::DoesNotInclude,
            "borrowed" => DefinitionKind::Borrowed,
            other => {
                self.error(
                    DiagnosticCode::E710,
                    definition.kind.span,
                    format!(
                        "unknown definition kind `{other}`; the kind of a definition is \
                         `means`, `includes`, `does_not_include` or `borrowed`"
                    ),
                );
                DefinitionKind::Means
            }
        };
        let basis = match definition.basis.text.as_str() {
            "enacted" => Basis::Enacted,
            "restated" => Basis::Restated,
            "unresolved" => Basis::Unresolved,
            other => {
                self.error(
                    DiagnosticCode::E710,
                    definition.basis.span,
                    format!(
                        "unknown basis `{other}`; the basis of a definition is `enacted`, \
                         `restated` or `unresolved`"
                    ),
                );
                Basis::Unresolved
            }
        };
        for term in &definition.terms {
            self.not_empty(term, "a term");
        }
        let (words, words_span) = match &definition.words {
            ast::Words::Stated(words) => {
                if words.value.is_empty() {
                    self.error(
                        DiagnosticCode::E710,
                        words.span,
                        "the reach words are an empty string; a definition whose text states \
                         none has `words unstated`",
                    );
                }
                (Some(words.value.clone()), words.span)
            }
            ast::Words::Unstated(span) => (None, *span),
        };
        if let Some(borrowed_from) = &definition.borrowed_from {
            self.not_empty(borrowed_from, "`borrowed_from`");
        }
        let region = definition
            .region
            .as_ref()
            .map(|region| self.address_or_stand_in(region));
        let see_note = definition
            .see_note
            .as_ref()
            .map(|see_note| self.address_or_stand_in(see_note));
        self.spans.definitions.push(DefinitionSpans {
            at: definition.at.span,
            terms: definition.terms.iter().map(|term| term.span).collect(),
            words: words_span,
            kind: definition.kind.span,
        });
        self.spans
            .regions
            .push(definition.region.as_ref().map(|region| region.span));
        self.spans
            .see_notes
            .push(definition.see_note.as_ref().map(|see_note| see_note.span));
        Definition {
            at,
            terms: definition
                .terms
                .iter()
                .map(|term| term.value.clone())
                .collect(),
            kind,
            words,
            region,
            basis,
            borrowed_from: definition
                .borrowed_from
                .as_ref()
                .map(|borrowed_from| borrowed_from.value.clone()),
            see_note,
        }
    }

    /// A text. Each run but a footnote marker has words, and the text of the
    /// block is normalized.
    fn text(&mut self, text: &ast::Text) -> Text {
        let mut runs = Vec::with_capacity(text.runs.len());
        for run in &text.runs {
            let lowered = match run {
                ast::Run::Words(words) => {
                    self.words(words);
                    Run::Words(words.value.clone())
                }
                ast::Run::Ref { words, target, .. } => {
                    self.words(words);
                    Run::Ref {
                        words: words.value.clone(),
                        target: target.value.clone(),
                    }
                }
                ast::Run::Sup { words, .. } => {
                    self.words(words);
                    Run::Sup(words.value.clone())
                }
                ast::Run::Sub { words, .. } => {
                    self.words(words);
                    Run::Sub(words.value.clone())
                }
                ast::Run::Mark {
                    shown,
                    footnote,
                    span,
                } => {
                    self.spans.marks.push((footnote.value, *span));
                    Run::Mark {
                        shown: shown.value.clone(),
                        footnote: footnote.value,
                    }
                }
            };
            runs.push(lowered);
        }
        let lowered = Text::from_runs(runs);
        let plain = lowered.plain();
        if !is_normalized(&plain) {
            let span = run_at(text, first_difference(&plain, &normalize(&plain)));
            self.error(DiagnosticCode::E710, span, "text is not normalized");
        }
        lowered
    }

    fn words(&mut self, words: &ast::Str) {
        if words.value.is_empty() {
            self.error(DiagnosticCode::E710, words.span, "a run has no words");
        }
    }

    /// An `E710` when `value`, which names `what`, is the empty string.
    fn not_empty(&mut self, value: &ast::Str, what: &str) {
        if value.value.is_empty() {
            self.error(
                DiagnosticCode::E710,
                value.span,
                format!("{what} is an empty string"),
            );
        }
    }
}

/// The byte offset into `plain` of the first character that `normalized`
/// does not have in its place.
fn first_difference(plain: &str, normalized: &str) -> usize {
    plain
        .char_indices()
        .zip(normalized.chars())
        .find(|((_, found), expected)| found != expected)
        .map_or(normalized.len().min(plain.len()), |((offset, _), _)| offset)
}

/// The span of the run of `text` whose words hold the byte `offset` of the
/// text's plain text, or of the whole text when no run does.
fn run_at(text: &ast::Text, offset: usize) -> Span {
    let mut start = 0;
    for run in &text.runs {
        let words = match run {
            ast::Run::Words(words)
            | ast::Run::Ref { words, .. }
            | ast::Run::Sup { words, .. }
            | ast::Run::Sub { words, .. } => words.value.len(),
            ast::Run::Mark { .. } => 0,
        };
        if (start..start + words).contains(&offset) {
            return run.span();
        }
        start += words;
    }
    text.span
}
