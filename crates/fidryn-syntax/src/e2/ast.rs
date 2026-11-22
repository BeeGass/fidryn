//! The syntax tree of an edition-2 module.
//!
//! One type stands for each production of the grammar. Every node carries the
//! span of the text it was read from, every string is decoded and keeps the
//! span of its token (quotation marks included), and every integer is a
//! `u32`. The tree holds what the source writes and judges nothing: whether a
//! module is a unit, a container or curated, whether an address is well
//! formed, and whether a hash matches are decided when the tree is lowered.

use fidryn_core::Span;

/// A string literal, decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Str {
    pub value: String,
    pub span: Span,
}

/// A word in the place of an identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ident {
    pub text: String,
    pub span: Span,
}

/// An integer literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Int {
    pub value: u32,
    pub span: Span,
}

/// One of a closed set of words, and where it was written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

/// `module String { Decl }`, after the marker `edition 2`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub address: Str,
    pub decls: Vec<Decl>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decl {
    Alias(Alias),
    Within(Within),
    Source(Source),
    PositiveLaw(PositiveLaw),
    Provision(Provision),
    Container(Container),
    Credit(Credit),
    Note(Note),
    Footnote(Footnote),
    Definition(Definition),
    Layers(Layers),
    Import(Import),
    Citation(Citation),
    Binding(Binding),
}

impl Decl {
    pub fn span(&self) -> Span {
        match self {
            Self::Alias(decl) => decl.span,
            Self::Within(decl) => decl.span,
            Self::Source(decl) => decl.span,
            Self::PositiveLaw(decl) => decl.span,
            Self::Provision(decl) => decl.span,
            Self::Container(decl) => decl.span,
            Self::Credit(decl) => decl.span,
            Self::Note(decl) => decl.span,
            Self::Footnote(decl) => decl.span,
            Self::Definition(decl) => decl.span,
            Self::Layers(decl) => decl.span,
            Self::Import(decl) => decl.span,
            Self::Citation(decl) => decl.span,
            Self::Binding(decl) => decl.span,
        }
    }
}

/// `alias String`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Alias {
    pub address: Str,
    pub span: Span,
}

/// `within String`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Within {
    pub address: Str,
    pub span: Span,
}

/// `source String`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub name: Str,
    pub span: Span,
}

/// `positive_law true` or `positive_law false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PositiveLaw {
    pub value: Spanned<bool>,
    pub span: Span,
}

/// `provision String Ident { [Num] [heading Text] [status Ident] { Part | Provision } hash String }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provision {
    pub address: Str,
    pub kind: Ident,
    pub num: Option<Num>,
    pub heading: Option<Text>,
    pub status: Option<Ident>,
    pub items: Vec<ProvisionItem>,
    pub hash: Str,
    pub span: Span,
}

/// A text part or a child provision, in the order the source writes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProvisionItem {
    Part(Part),
    Provision(Provision),
}

impl ProvisionItem {
    pub fn span(&self) -> Span {
        match self {
            Self::Part(part) => part.span,
            Self::Provision(provision) => provision.span,
        }
    }
}

/// `num String Text`: the machine form and the form printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Num {
    pub value: Str,
    pub shown: Text,
    pub span: Span,
}

/// `Role ( Text | { Block { Block } } )`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub role: Spanned<Role>,
    pub body: Body,
    pub span: Span,
}

/// `chapeau`, `content`, `continuation` or `proviso`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Chapeau,
    Content,
    Continuation,
    Proviso,
}

/// What a part or a cell holds: a text in the short form, or blocks between
/// braces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Text(Text),
    Blocks { blocks: Vec<Block>, span: Span },
}

impl Body {
    pub fn span(&self) -> Span {
        match self {
            Self::Text(text) => text.span,
            Self::Blocks { span, .. } => *span,
        }
    }
}

/// `p Text`, `subheading Text`, a table, quoted content or `image String`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Para { text: Text, span: Span },
    Subheading { text: Text, span: Span },
    Table(Table),
    Quoted(Quoted),
    Image { source: Str, span: Span },
}

impl Block {
    pub fn span(&self) -> Span {
        match self {
            Self::Para { span, .. } | Self::Subheading { span, .. } | Self::Image { span, .. } => {
                *span
            }
            Self::Table(table) => table.span,
            Self::Quoted(quoted) => quoted.span,
        }
    }
}

/// `Run { Run }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub runs: Vec<Run>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    /// `String`.
    Words(Str),
    /// `[ String -> String ]`: the linked words and the target.
    Ref { words: Str, target: Str, span: Span },
    /// `sup String`.
    Sup { words: Str, span: Span },
    /// `sub String`.
    Sub { words: Str, span: Span },
    /// `mark String Int`: the marker shown and the ordinal of its footnote.
    Mark {
        shown: Str,
        footnote: Int,
        span: Span,
    },
}

impl Run {
    pub fn span(&self) -> Span {
        match self {
            Self::Words(words) => words.span,
            Self::Ref { span, .. }
            | Self::Sup { span, .. }
            | Self::Sub { span, .. }
            | Self::Mark { span, .. } => *span,
        }
    }
}

/// `table { Row }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub rows: Vec<Row>,
    pub span: Span,
}

/// `( head | body | foot ) { Cell }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub group: Spanned<RowGroup>,
    pub cells: Vec<Cell>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowGroup {
    Head,
    Body,
    Foot,
}

/// `( td | th ) [ span Int Int ] ( Text | { { Block } } )`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    pub kind: Spanned<CellKind>,
    pub spanning: Option<Spanning>,
    pub body: Body,
    pub span: Span,
}

/// `td` or `th`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellKind {
    Data,
    Header,
}

/// `span Int Int`: the columns and the rows a cell covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spanning {
    pub colspan: Int,
    pub rowspan: Int,
    pub span: Span,
}

/// `quoted [ String ] { QItem }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quoted {
    pub origin: Option<Str>,
    pub items: Vec<QItem>,
    pub span: Span,
}

/// A block, a part, or a level of quoted content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QItem {
    Block(Block),
    Part(Part),
    Level(Level),
}

impl QItem {
    pub fn span(&self) -> Span {
        match self {
            Self::Block(block) => block.span(),
            Self::Part(part) => part.span,
            Self::Level(level) => level.span,
        }
    }
}

/// `level Ident { [Num] [heading Text] { QItem } }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    pub kind: Ident,
    pub num: Option<Num>,
    pub heading: Option<Text>,
    pub items: Vec<QItem>,
    pub span: Span,
}

/// `container String Ident { [Num] [heading Text] [status Ident] { member String } hash String }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Container {
    pub address: Str,
    pub kind: Ident,
    pub num: Option<Num>,
    pub heading: Option<Text>,
    pub status: Option<Ident>,
    pub members: Vec<Str>,
    pub hash: Str,
    pub span: Span,
}

/// `credit Text`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credit {
    pub text: Text,
    pub span: Span,
}

/// `note String ( enacted | editorial ) { group Ident { topic String } [heading Text] { Block } hash String }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub address: Str,
    pub layer: Spanned<NoteLayer>,
    pub group: Ident,
    pub topics: Vec<Str>,
    pub heading: Option<Text>,
    pub blocks: Vec<Block>,
    pub hash: Str,
    pub span: Span,
}

/// `enacted` or `editorial`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteLayer {
    Enacted,
    Editorial,
}

/// `footnote Int String { Block }`: the ordinal, the marker shown, the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Footnote {
    pub ordinal: Int,
    pub shown: Str,
    pub blocks: Vec<Block>,
    pub span: Span,
}

/// `definition String { term String { term String } kind Ident words ( String | unstated )
/// [condition String] [region String] basis Ident [borrowed_from String | borrowed_words String]
/// [see_note String] }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Definition {
    pub at: Str,
    pub terms: Vec<Str>,
    pub kind: Ident,
    pub words: Words,
    pub condition: Option<Str>,
    pub region: Option<Str>,
    pub basis: Ident,
    pub borrowed_from: Option<Str>,
    pub borrowed_words: Option<Str>,
    pub see_note: Option<Str>,
    pub span: Span,
}

/// The value of `words`: a phrase, or the word `unstated`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Words {
    Stated(Str),
    Unstated(Span),
}

/// `layers { operative String enacted String editorial String }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layers {
    pub operative: Str,
    pub enacted: Str,
    pub editorial: Str,
    pub span: Span,
}

/// `import String ( as Ident | { Ident { , Ident } } )`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub module: Str,
    pub form: ImportForm,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportForm {
    /// `as Ident`.
    As(Ident),
    /// `{ Ident { , Ident } }`.
    Names(Vec<Ident>),
}

/// `citation Ident -> String`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Citation {
    pub name: Ident,
    pub target: Str,
    pub span: Span,
}

/// `binding Ident { binds ( String | Ident [ . Ident ] ) release String provision_hash String
/// notes_hash String pending }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub name: Ident,
    pub binds: BindTarget,
    pub release: Str,
    pub provision_hash: Str,
    pub notes_hash: Str,
    /// The span of the word `pending`.
    pub pending: Span,
    pub span: Span,
}

/// What `binds` names: an address, or a name with the alias of an import
/// before it when it is written `alias.name`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindTarget {
    Address(Str),
    Name {
        qualifier: Option<Ident>,
        name: Ident,
        span: Span,
    },
}

impl BindTarget {
    pub fn span(&self) -> Span {
        match self {
            Self::Address(address) => address.span,
            Self::Name { span, .. } => *span,
        }
    }
}
