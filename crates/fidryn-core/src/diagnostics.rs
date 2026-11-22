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
    E700,
    E710,
    E720,
    E730,
    E740,
    W750,
    W760,
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
            Self::E700 => "E700",
            Self::E710 => "E710",
            Self::E720 => "E720",
            Self::E730 => "E730",
            Self::E740 => "E740",
            Self::W750 => "W750",
            Self::W760 => "W760",
        }
    }

    pub fn severity(self) -> Severity {
        match self {
            Self::W610 | Self::W620 | Self::W750 | Self::W760 => Severity::Warning,
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
            Self::E700 => "InvalidAddress",
            Self::E710 => "IntegrityMismatch",
            Self::E720 => "UnresolvedModule",
            Self::E730 => "WorkspaceViolation",
            Self::E740 => "PinMismatch",
            Self::W750 => "StaleBinding",
            Self::W760 => "ToolchainMismatch",
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
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
            source_file: None,
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

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source_file = Some(source.into());
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
        if let Some(source) = &self.source_file {
            write!(f, "{source}: ")?;
        }
        write!(f, "{} {}: {}", self.code, self.code.title(), self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::{DiagnosticCode, Severity};
    use std::fs;
    use std::path::Path;

    /// Every code of the enum. The `match` below is exhaustive, so a code
    /// added to the enum stops this module from compiling until it is named
    /// there; add it to the list in the same edit.
    fn every_code() -> Vec<DiagnosticCode> {
        use DiagnosticCode::*;
        let codes = vec![
            E100, E200, E210, E310, E320, E330, E340, E410, E420, E430, E431, E510, E511, E520,
            E530, E540, W610, W620, E700, E710, E720, E730, E740, W750, W760,
        ];
        for code in &codes {
            match code {
                E100 | E200 | E210 | E310 | E320 | E330 | E340 | E410 | E420 | E430 | E431
                | E510 | E511 | E520 | E530 | E540 | W610 | W620 | E700 | E710 | E720 | E730
                | E740 | W750 | W760 => {}
            }
        }
        codes
    }

    /// The lines of the first fenced block under `## Diagnostic codes`.
    fn code_list(guide: &str) -> Vec<String> {
        let mut lines = guide
            .lines()
            .skip_while(|line| line.trim_end() != "## Diagnostic codes");
        assert!(
            lines.next().is_some(),
            "the guide has no `## Diagnostic codes` section"
        );
        let mut lines = lines.skip_while(|line| !line.starts_with("```"));
        assert!(
            lines.next().is_some(),
            "the `## Diagnostic codes` section has no code block"
        );
        lines
            .take_while(|line| !line.starts_with("```"))
            .map(|line| line.trim_end().to_owned())
            .collect()
    }

    #[test]
    fn new_codes_have_their_names_titles_and_severities() {
        let expected = [
            (
                DiagnosticCode::E700,
                "E700",
                "InvalidAddress",
                Severity::Error,
            ),
            (
                DiagnosticCode::E710,
                "E710",
                "IntegrityMismatch",
                Severity::Error,
            ),
            (
                DiagnosticCode::E720,
                "E720",
                "UnresolvedModule",
                Severity::Error,
            ),
            (
                DiagnosticCode::E730,
                "E730",
                "WorkspaceViolation",
                Severity::Error,
            ),
            (DiagnosticCode::E740, "E740", "PinMismatch", Severity::Error),
            (
                DiagnosticCode::W750,
                "W750",
                "StaleBinding",
                Severity::Warning,
            ),
            (
                DiagnosticCode::W760,
                "W760",
                "ToolchainMismatch",
                Severity::Warning,
            ),
        ];
        for (code, name, title, severity) in expected {
            assert_eq!(code.as_str(), name);
            assert_eq!(code.title(), title, "{name}");
            assert_eq!(code.severity(), severity, "{name}");
        }
    }

    #[test]
    fn the_code_list_of_the_architecture_guide_is_the_enum() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ARCHITECTURE.md");
        let guide = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read docs/ARCHITECTURE.md: {error}"));
        let mut listed = code_list(&guide);
        listed.sort();
        let mut expected: Vec<String> = every_code()
            .into_iter()
            .map(|code| format!("{} {}", code.as_str(), code.title()))
            .collect();
        expected.sort();
        assert_eq!(
            listed, expected,
            "docs/ARCHITECTURE.md must list every diagnostic code once, with its title"
        );
    }
}
