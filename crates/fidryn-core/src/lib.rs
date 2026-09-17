//! Canonical Fidryn Core: types, IR, legal state, outcomes, and diagnostics.
//!
//! `Prop` is a first-class sort. It has no conversion to [`bool`].

pub mod canonical;
pub mod case;
pub mod diagnostics;
pub mod effects;
pub mod ids;
pub mod ir;
pub mod outcome;
pub mod patterns;
pub mod positions;
pub mod state;
pub mod time;
pub mod types;
pub mod value;

pub use canonical::{canonical_json, canonical_to_vec};
pub use case::{
    AdmissibleCompletions, CaseRecord, ClosureRecord, CompletionDomain, EvidenceItem,
    ManifestArtifact, ModelBoundary, SourceManifest, SourceWeight,
};
pub use diagnostics::{Diagnostic, DiagnosticCode, Severity, Span};
pub use effects::{
    EffectName, HaltReason, Handler, HandlerResult, OpenOperation, SuspensionReason,
};
pub use ids::{
    ClauseId, CompletionProofId, EffectId, JurisdictionId, ModuleId, NodeId, OriginId, QueryName,
    RuleId, SourceManifestId, SourceSnapshotId, TraceId,
};
pub use ir::{
    ClauseSelector, Consequence, CoreAssertion, CoreConflictDoctrine, CoreDecision, CoreDecl,
    CoreEffect, CoreEffectDecl, CoreEffectOp, CoreFact, CoreFunction, CoreModule, CoreObservation,
    CoreQuery, CoreRule, CoreSelection, CoreVerify, DecisionReturn, DeclaredDecisionResult, Guard,
    NodeMeta, QueryPlan, RuleKind, SelectionResult, VerificationBounds,
};
pub use outcome::{OpenRequest, Outcome};
pub use patterns::{
    ContextPattern, LegalEffectPattern, LegalStatusPattern, LegalSubjectPattern, PositionPattern,
    PropPattern, TermPattern,
};
pub use positions::{Position, PositionKind};
pub use state::{
    AuthorityLedger, DecisionLedger, InterpretationLedger, LegalState, LegalStatusLedger,
    Occupancy, PositionLedger, RecordLedger, SourceLedger, StatusMode, WorldLedger,
};
pub use time::{
    Bound, CalendarKind, FidrynDuration, Instant, Interval, RunContext, TemporalLens, TimeError,
};
pub use types::{PrimitiveType, Sort, Type, is_subtype};
pub use value::{PropTerm, Term, Value};

/// Compile-time reminder: a proposition is not a Boolean.
pub const PROP_IS_NOT_BOOL: &str = "Prop has no implicit conversion to Bool";
