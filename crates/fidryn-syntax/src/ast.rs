//! Surface AST. Trivia is retained on [`crate::Parse`] tokens; this tree keeps spans.

use fidryn_core::Span;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub span: Span,
    pub name: String,
    pub version: String,
    pub type_params: Vec<String>,
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
    pub automatic: bool,
    pub result_type: Option<String>,
    pub params: Vec<(String, String)>,
    pub effects: Vec<String>,
    pub fuel: Option<u32>,
    pub expr: Option<Expr>,
    pub goal: Option<GoalAst>,
    pub rule_kind: Option<String>,
    pub guard: Option<Expr>,
    pub consequences: Vec<ConsequenceAst>,
    pub fallback: Vec<ConsequenceAst>,
    pub require: Option<Expr>,
    pub source_basis: Option<Expr>,
    pub fields: BTreeMap<String, String>,
    pub type_args: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    Bool(bool),
    Int(i64),
    Decimal(String),
    String(String),
    Ident(String),
    Apply {
        callee: Box<Expr>,
        args: Vec<Expr>,
    },
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Field {
        base: Box<Expr>,
        name: String,
    },
    Call {
        callee: String,
        args: Vec<Expr>,
    },
    If {
        cond: Box<Expr>,
        then: Box<Expr>,
        else_: Option<Box<Expr>>,
    },
    Money {
        currency: String,
        amount: String,
    },
    Duration {
        n: i64,
        unit: String,
    },
    Require(Box<Expr>),
    Block(Vec<Expr>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GoalAst {
    pub kind: String,
    pub expr: Option<Expr>,
    pub fields: BTreeMap<String, Expr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsequenceAst {
    pub verb: String,
    pub expr: Expr,
}

impl Decl {
    pub fn new(
        span: Span,
        keyword: impl Into<String>,
        name: Option<String>,
        signature: Option<String>,
        source: String,
    ) -> Self {
        Self {
            span,
            keyword: keyword.into(),
            name,
            signature,
            source,
            automatic: false,
            result_type: None,
            params: Vec::new(),
            effects: Vec::new(),
            fuel: None,
            expr: None,
            goal: None,
            rule_kind: None,
            guard: None,
            consequences: Vec::new(),
            fallback: Vec::new(),
            require: None,
            source_basis: None,
            fields: BTreeMap::new(),
            type_args: Vec::new(),
        }
    }
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
