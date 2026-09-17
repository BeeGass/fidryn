//! Canonical Fidryn Core: types, IR, legal state, outcomes, and diagnostics.
//!
//! `Prop` is a first-class sort. It has no conversion to [`bool`].

pub mod bundle;
pub mod canonical;
pub mod case;
pub mod diagnostics;
pub mod duty;
pub mod effects;
pub mod engine;
pub mod ids;
pub mod ir;
pub mod outcome;
pub mod package;
pub mod patterns;
pub mod positions;
pub mod request;
pub mod state;
pub mod time;
pub mod types;
pub mod value;
pub mod view;

pub use bundle::{
    ArtifactVerificationMethod, BundleArtifact, SourceBundleId, VerifiedSourceBundle,
};
pub use canonical::{CANONICAL_SCHEMA, canonical_json, canonical_to_vec};
pub use case::{
    AdmissibleCompletions, Assumption, CaseRecord, ClosureRecord, CompletionDomain, EvidenceItem,
    LedgerEvent, ManifestArtifact, ModelBoundary, SourceManifest, SourceWeight,
};
pub use diagnostics::{Diagnostic, DiagnosticCode, Severity, Span};
pub use duty::{DutyState, DutyStatus};
pub use effects::{
    EffectName, HaltReason, Handler, HandlerResult, OpenOperation, SuspensionReason,
};
pub use engine::EngineError;
pub use ids::{
    ClauseId, CompletionProofId, EffectId, JurisdictionId, ModuleId, NodeId, OriginId, PackageId,
    ProgramDigest, QueryName, RuleId, SourceManifestId, SourceSnapshotId, TraceId,
};
pub use ir::{
    ClauseSelector, Consequence, CoreAssertion, CoreConflictDoctrine, CoreDecision, CoreDecl,
    CoreEffect, CoreEffectDecl, CoreEffectOp, CoreFact, CoreFunction, CoreModule, CoreNomination,
    CoreObservation, CoreQuery, CoreRule, CoreSelection, CoreVerify, DecisionReturn,
    DeclaredDecisionResult, Guard, NodeMeta, QueryPlan, RuleKind, SelectionResult,
    VerificationBounds,
};
pub use outcome::{
    BoundClaims, BranchClaim, CheckedCertificate, CoverageMethod, CoverageWitness,
    EvaluationReport, ExecutionMode, OpenRequest, Outcome, ReplayIssuance, ReplayVerifiedCoverage,
    StructurallyCheckedCoverage, TrustProfile, canonical_branch_root,
};
pub use package::{
    PACKAGE_LOCK_SCHEMA, PACKAGES_DIR, PackageLock, artifact_path_is_package, is_safe_package_name,
    package_dir_from_artifact_path, package_name_from_import, package_path_matches_import,
    packages_root,
};
pub use patterns::{
    ContextPattern, LegalEffectPattern, LegalStatusPattern, LegalSubjectPattern, PositionPattern,
    PropPattern, TermPattern,
};
pub use positions::{Position, PositionKind};
pub use request::{CaseSnapshotIdentity, ExecutionRequest, ExecutionRequestId, ProgramIdentity};
pub use state::{
    AuthorityGrant, AuthorityLedger, DecisionLedger, InterpretationLedger, LegalState,
    LegalStatusLedger, Occupancy, PositionLedger, RecordLedger, SourceLedger, StatusMode,
    WorldLedger,
};
pub use time::{
    Bound, CalendarKind, FidrynDuration, Instant, Interval, RunContext, TemporalLens, TimeError,
};
pub use types::{PrimitiveType, Sort, Type, is_subtype};
pub use value::{BinOp, PropTerm, Term, Value};
pub use view::FrozenCaseView;

/// Compile-time reminder: a proposition is not a Boolean.
pub const PROP_IS_NOT_BOOL: &str = "Prop has no implicit conversion to Bool";
