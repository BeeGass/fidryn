//! Surface AST. Trivia is retained on [`crate::Parse`] tokens; this tree keeps spans.

use fidryn_core::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub span: Span,
    pub name: String,
    pub version: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    Header(Header),
    Source(Decl),
    Import(Decl),
    Entity(Decl),
    Type(Decl),
    RecordType(Decl),
    Office(Decl),
    Proposition(Decl),
    Observation(Decl),
    Nomination(Decl),
    Function(Decl),
    Effect(Decl),
    Rule(Decl),
    Power(Decl),
    Duty(Decl),
    Judgment(Decl),
    Decision(Decl),
    LegalAct(Decl),
    Clause(Decl),
    Interpretation(Decl),
    ConflictDoctrine(Decl),
    Query(Decl),
    Scenario(Decl),
    Verify(Decl),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub span: Span,
    pub kind: HeaderKind,
    pub value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderKind {
    Jurisdiction,
    SourceSnapshot,
    SourceManifest,
    EffectiveAt,
    RecordedAt,
    OutsideScope,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl {
    pub span: Span,
    pub keyword: String,
    pub name: Option<String>,
    pub signature: Option<String>,
    pub source: String,
}

impl Item {
    pub fn span(&self) -> Span {
        match self {
            Item::Header(h) => h.span,
            Item::Source(d)
            | Item::Import(d)
            | Item::Entity(d)
            | Item::Type(d)
            | Item::RecordType(d)
            | Item::Office(d)
            | Item::Proposition(d)
            | Item::Observation(d)
            | Item::Nomination(d)
            | Item::Function(d)
            | Item::Effect(d)
            | Item::Rule(d)
            | Item::Power(d)
            | Item::Duty(d)
            | Item::Judgment(d)
            | Item::Decision(d)
            | Item::LegalAct(d)
            | Item::Clause(d)
            | Item::Interpretation(d)
            | Item::ConflictDoctrine(d)
            | Item::Query(d)
            | Item::Scenario(d)
            | Item::Verify(d) => d.span,
        }
    }

    pub fn keyword(&self) -> &str {
        match self {
            Item::Header(h) => match h.kind {
                HeaderKind::Jurisdiction => "jurisdiction",
                HeaderKind::SourceSnapshot => "source_snapshot",
                HeaderKind::SourceManifest => "source_manifest",
                HeaderKind::EffectiveAt => "effective_at",
                HeaderKind::RecordedAt => "recorded_at",
                HeaderKind::OutsideScope => "outside_scope",
            },
            Item::Source(_) => "source",
            Item::Import(_) => "import",
            Item::Entity(_) => "entity",
            Item::Type(_) => "type",
            Item::RecordType(d) => d.keyword.as_str(),
            Item::Office(_) => "office",
            Item::Proposition(_) => "proposition",
            Item::Observation(_) => "observation",
            Item::Nomination(_) => "nomination",
            Item::Function(d) => d.keyword.as_str(),
            Item::Effect(_) => "effect",
            Item::Rule(_) => "rule",
            Item::Power(_) => "power",
            Item::Duty(_) => "duty",
            Item::Judgment(_) => "judgment",
            Item::Decision(_) => "decision",
            Item::LegalAct(_) => "legal_act",
            Item::Clause(_) => "clause",
            Item::Interpretation(_) => "interpretation_family",
            Item::ConflictDoctrine(_) => "conflict_doctrine",
            Item::Query(_) => "query",
            Item::Scenario(_) => "scenario",
            Item::Verify(d) => d.keyword.as_str(),
        }
    }
}
