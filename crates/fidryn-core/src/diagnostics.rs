//! Compiler diagnostics. These are not legal outcomes.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DiagnosticCode {
    E100,
    E200,
    E210,
    E310,
    E320,
    E330,
    E340,
    E410,
    E420,
    E430,
    E431,
    E510,
    E511,
    E520,
    E530,
    E540,
    W610,
    W620,
}

impl DiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::E100 => "E100",
            Self::E200 => "E200",
            Self::E210 => "E210",
            Self::E310 => "E310",
            Self::E320 => "E320",
            Self::E330 => "E330",
            Self::E340 => "E340",
            Self::E410 => "E410",
            Self::E420 => "E420",
            Self::E430 => "E430",
            Self::E431 => "E431",
            Self::E510 => "E510",
            Self::E511 => "E511",
            Self::E520 => "E520",
            Self::E530 => "E530",
            Self::E540 => "E540",
            Self::W610 => "W610",
            Self::W620 => "W620",
        }
    }

    pub fn severity(self) -> Severity {
        match self {
            Self::W610 | Self::W620 => Severity::Warning,
            _ => Severity::Error,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::E100 => "ParseError",
            Self::E200 => "UnresolvedName",
            Self::E210 => "TypeMismatch",
            Self::E310 => "PropAsGuard",
            Self::E320 => "DirectInstitutionalMutation",
            Self::E330 => "MissingConstitutiveBasis",
            Self::E340 => "InvalidAuthorityKind",
            Self::E410 => "AmbiguousSelection",
            Self::E420 => "UnhandledEffect",
            Self::E430 => "MissingQueryGoal",
            Self::E431 => "InvalidClauseReference",
            Self::E510 => "UnjustifiedPriority",
            Self::E511 => "UnknownConflictTarget",
            Self::E520 => "InvalidBitemporalInterval",
            Self::E530 => "NegativeRecursion",
            Self::E540 => "MissingLegalSource",
            Self::W610 => "OpenUniverse",
            Self::W620 => "BoundedVerification",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub primary_span: Option<Span>,
    pub related_spans: Vec<Span>,
    pub suggestion: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Diagnostic {
    pub fn new(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            primary_span: None,
            related_spans: Vec::new(),
            suggestion: None,
        }
    }

    pub fn with_span(mut self, span: Span) -> Self {
        self.primary_span = Some(span);
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn prop_as_guard(span: Span, pretty: &str) -> Self {
        Self::new(
            DiagnosticCode::E310,
            format!("`{pretty}` has type Prop, but this rule requires a guard"),
        )
        .with_span(span)
        .with_suggestion(
            "Use one of:\n    operative(issue, context)\n    determined(issue, Determination#...)\n    assumed(issue, Scenario#...)\nNo implicit proposition-to-Boolean conversion exists.",
        )
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.code, self.code.title(), self.message)
    }
}
