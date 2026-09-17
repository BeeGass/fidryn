//! Honest evaluator results. Determinate is never a guess.

use crate::case::{Assumption, CaseRecord};
use crate::effects::OpenOperation;
use crate::ids::{CompletionProofId, ModuleId, QueryName, SourceSnapshotId, TraceId};
use crate::ir::CoreConflictDoctrine;
use crate::patterns::PropPattern;
use crate::time::Instant;
use crate::value::Value;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// One claimed world: bindings for a case clone and the query answer there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchClaim {
    pub bindings: BTreeMap<String, Value>,
    pub answer: Value,
}

/// Exhaustive-search witness for a covering certificate.
///
/// A hash of open issues is not covering. Completeness requires a nonempty
/// examined space (`examined == total && examined > 0`) that was not cut
/// short (`incomplete == false`). [`Self::is_complete`] does not inspect
/// `branches`. [`CheckedCertificate::verified_covering`] requires one unique
/// world per `total` with matching answers. FiniteReplay meaning is checked
/// by kernel replay, not by this shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageWitness {
    pub examined: usize,
    pub total: usize,
    pub incomplete: bool,
    pub answer: Value,
    #[serde(default)]
    pub branches: Vec<BranchClaim>,
}

impl CoverageWitness {
    /// Complete shape with empty `branches`.
    ///
    /// Shape-only: [`CheckedCertificate::verified_structural`], not FiniteReplay.
    pub fn complete(examined: usize, answer: Value) -> Self {
        Self {
            examined,
            total: examined,
            incomplete: false,
            answer,
            branches: Vec::new(),
        }
    }

    pub fn is_complete(&self) -> bool {
        !self.incomplete && self.examined == self.total && self.examined > 0
    }
}

/// How an artifact or evaluation was authenticated.
///
/// `fixture` is not byte-verified. Missing digest, or a digest without
/// bytes, is [`Unauthenticated`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TrustProfile {
    Fixture,
    ByteVerified,
    PolicyAccepted,
    #[default]
    Unauthenticated,
}

/// How a certificate's coverage claim was established.
///
/// `None` is a claims digest ([`BoundClaims`]). `Structural` is shape-only
/// ([`StructurallyCheckedCoverage`]). `FiniteReplay` is finite coverage
/// verification by replay ([`ReplayVerifiedCoverage`]). Only FiniteReplay
/// is covering. This is not independent Lean / SMT proof checking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CoverageMethod {
    #[default]
    None,
    Structural,
    FiniteReplay,
}

/// Whether evaluation committed operative state or applied a scenario overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionMode {
    #[default]
    Operative,
    Scenario,
}

/// Evaluator result plus unresolved issues, coverage, and trust.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationReport<T = Value> {
    pub outcome: Outcome<T>,
    pub unresolved: BTreeSet<OpenRequest>,
    pub coverage: Option<CoverageWitness>,
    pub trust: TrustProfile,
    pub provenance_root: TraceId,
    #[serde(default)]
    pub execution_mode: ExecutionMode,
    #[serde(default)]
    pub assumptions: Vec<Assumption>,
    #[serde(default)]
    pub verification_method: CoverageMethod,
}

impl<T> EvaluationReport<T> {
    /// Wrap an outcome. Coverage is unset; trust is unauthenticated.
    ///
    /// Unresolved issues are the suspended requests or contingent pivots.
    /// Mode is operative; assumptions and verification method are empty/none.
    pub fn from_outcome(outcome: Outcome<T>) -> Self {
        let unresolved = match &outcome {
            Outcome::Suspended { requests, .. } => requests.clone(),
            Outcome::Contingent { pivots, .. } => pivots.clone(),
            _ => BTreeSet::new(),
        };
        let provenance_root = outcome.trace();
        Self {
            outcome,
            unresolved,
            coverage: None,
            trust: TrustProfile::Unauthenticated,
            provenance_root,
            execution_mode: ExecutionMode::Operative,
            assumptions: Vec::new(),
            verification_method: CoverageMethod::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OpenRequest {
    NeedEvidence {
        issue: PropPattern,
        schema: String,
    },
    NeedJudgment {
        issue: crate::value::PropTerm,
        protocol: String,
    },
    NeedChoice {
        protocol: String,
        options: Vec<String>,
    },
    NeedInterpretation {
        source: String,
        family: String,
    },
    NeedApplicableLaw {
        issue: String,
        candidates: Vec<String>,
    },
    NeedConflict {
        graph: Vec<String>,
        doctrines: Vec<String>,
    },
    NeedCustom {
        effect: String,
        payload: String,
    },
}

/// A convergence certificate bound to a claims digest, optionally covering.
///
/// Strength is type-distinct: [`BoundClaims`] (`None`),
/// [`StructurallyCheckedCoverage`] (`Structural`),
/// [`ReplayVerifiedCoverage`] (`FiniteReplay`).
/// [`verified`](Self::verified) is a claims-digest binder, not a covering
/// proof checker. Matching claims do not discharge open constraints.
/// [`verified_structural`](Self::verified_structural) binds a
/// shape-complete witness and is not covering. Ignoring open issues
/// requires [`verified_covering`](Self::verified_covering) with unique
/// nonempty `branches` (`examined == total == branches.len()`). Empty
/// [`CoverageWitness::complete`] is not FiniteReplay. A raw
/// [`CompletionProofId`] is not a certificate.
///
/// ```compile_fail
/// use fidryn_core::{CheckedCertificate, CompletionProofId};
/// let id = CompletionProofId::of(b"P11");
/// let _ = CheckedCertificate {
///     id,
///     claims_digest: [0u8; 16],
/// };
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CheckedCertificate {
    id: CompletionProofId,
    claims_digest: [u8; 16],
    method: CoverageMethod,
}

#[derive(Serialize)]
struct CertificateClaims<'a> {
    program: ModuleId,
    snapshot: SourceSnapshotId,
    case: &'a CaseRecord,
    query: &'a str,
    valid: Instant,
    known: Instant,
    constraints: &'a BTreeSet<OpenRequest>,
    answer: &'a Value,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    args: &'a BTreeMap<String, Value>,
}

#[derive(Serialize)]
struct CoveringClaims<'a> {
    #[serde(flatten)]
    claims: CertificateClaims<'a>,
    tag: &'static str,
    examined: usize,
    total: usize,
    incomplete: bool,
    witness_answer: &'a Value,
}

/// Claims-digest evidence. [`CoverageMethod::None`]. Not covering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BoundClaims {
    certificate: CheckedCertificate,
}

impl BoundClaims {
    pub fn certificate(self) -> CheckedCertificate {
        self.certificate
    }
}

impl TryFrom<CheckedCertificate> for BoundClaims {
    type Error = String;

    fn try_from(certificate: CheckedCertificate) -> Result<Self, Self::Error> {
        if certificate.method() != CoverageMethod::None {
            return Err("BoundClaims requires CoverageMethod::None".into());
        }
        Ok(Self { certificate })
    }
}

impl From<BoundClaims> for CheckedCertificate {
    fn from(bound: BoundClaims) -> Self {
        bound.certificate
    }
}

/// Shape-complete coverage evidence. [`CoverageMethod::Structural`]. Not covering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StructurallyCheckedCoverage {
    certificate: CheckedCertificate,
}

impl StructurallyCheckedCoverage {
    pub fn certificate(self) -> CheckedCertificate {
        self.certificate
    }
}

impl TryFrom<CheckedCertificate> for StructurallyCheckedCoverage {
    type Error = String;

    fn try_from(certificate: CheckedCertificate) -> Result<Self, Self::Error> {
        if certificate.method() != CoverageMethod::Structural {
            return Err("StructurallyCheckedCoverage requires CoverageMethod::Structural".into());
        }
        Ok(Self { certificate })
    }
}

impl From<StructurallyCheckedCoverage> for CheckedCertificate {
    fn from(coverage: StructurallyCheckedCoverage) -> Self {
        coverage.certificate
    }
}

/// Finite-replay coverage evidence. [`CoverageMethod::FiniteReplay`]. Covering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ReplayVerifiedCoverage {
    certificate: CheckedCertificate,
}

impl ReplayVerifiedCoverage {
    pub fn certificate(self) -> CheckedCertificate {
        self.certificate
    }
}

impl TryFrom<CheckedCertificate> for ReplayVerifiedCoverage {
    type Error = String;

    fn try_from(certificate: CheckedCertificate) -> Result<Self, Self::Error> {
        if !certificate.is_covering() {
            return Err(
                "ReplayVerifiedCoverage requires CoverageMethod::FiniteReplay (is_covering)".into(),
            );
        }
        Ok(Self { certificate })
    }
}

impl From<ReplayVerifiedCoverage> for CheckedCertificate {
    fn from(coverage: ReplayVerifiedCoverage) -> Self {
        coverage.certificate
    }
}

fn reject_raw_certificate_id<'de, D, T>(deserializer: D, ctor: &'static str) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
{
    let _ = Option::<serde_json::Value>::deserialize(deserializer)?;
    Err(D::Error::custom(format!(
        "convergence certificates cannot be rebuilt from a raw id; use {ctor}"
    )))
}

impl<'de> Deserialize<'de> for BoundClaims {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        reject_raw_certificate_id(deserializer, "CheckedCertificate::verified")
    }
}

impl<'de> Deserialize<'de> for StructurallyCheckedCoverage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        reject_raw_certificate_id(deserializer, "CheckedCertificate::verified_structural")
    }
}

impl<'de> Deserialize<'de> for ReplayVerifiedCoverage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        reject_raw_certificate_id(deserializer, "CheckedCertificate::verified_covering")
    }
}

impl CheckedCertificate {
    fn claims_payload(claims: &CertificateClaims<'_>) -> Result<Vec<u8>, String> {
        crate::canonical_to_vec(claims).map_err(|e| e.to_string())
    }

    fn covering_tag(method: CoverageMethod) -> Result<&'static str, String> {
        match method {
            CoverageMethod::Structural => Ok("structural"),
            CoverageMethod::FiniteReplay => Ok("finiteReplay"),
            CoverageMethod::None => Err("claims digest is not a covering method".into()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_claims(
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        args: &BTreeMap<String, Value>,
    ) -> Result<(Vec<u8>, CompletionProofId), String> {
        let claims = CertificateClaims {
            program,
            snapshot,
            case,
            query: query.as_str(),
            valid,
            known,
            constraints,
            answer,
            args,
        };
        let payload = Self::claims_payload(&claims)?;
        let id = CompletionProofId::of(&payload);
        Ok((payload, id))
    }

    /// Content hash of the certificate claims. Pass this as `id` to [`verified`].
    #[allow(clippy::too_many_arguments)]
    pub fn claims_id(
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
    ) -> Result<CompletionProofId, String> {
        Self::claims_id_with_args(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            &BTreeMap::new(),
        )
    }

    /// [`claims_id`] with query arguments bound into the digest.
    #[allow(clippy::too_many_arguments)]
    pub fn claims_id_with_args(
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        args: &BTreeMap<String, Value>,
    ) -> Result<CompletionProofId, String> {
        let (_payload, id) = Self::bind_claims(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            args,
        )?;
        Ok(id)
    }

    /// Bind an untrusted proof id to checked claims. The id must be the
    /// content hash of those claims.
    ///
    /// This is a claims-digest binder ([`BoundClaims`]), not a covering
    /// proof checker. Open `constraints` are hashed into the digest (the
    /// path used when constructing a certificate that names ignored issues)
    /// so they cannot be swapped silently; they are not discharged by a
    /// kernel proof. Assumptions live on [`CaseRecord`] and are hashed with
    /// the case. Query arguments default to empty; see [`verified_with_args`].
    #[allow(clippy::too_many_arguments)]
    pub fn verified(
        id: CompletionProofId,
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
    ) -> Result<Self, String> {
        Self::verified_with_args(
            id,
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            &BTreeMap::new(),
        )
    }

    /// [`verified`] with query arguments bound into the claims digest.
    #[allow(clippy::too_many_arguments)]
    pub fn verified_with_args(
        id: CompletionProofId,
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        args: &BTreeMap<String, Value>,
    ) -> Result<Self, String> {
        let (payload, expected) = Self::bind_claims(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            args,
        )?;
        if expected != id {
            return Err(format!(
                "completion proof id {} does not match claims {}",
                id.hex(),
                expected.hex()
            ));
        }
        Ok(Self {
            id,
            claims_digest: Self::digest_from_payload(&payload),
            method: CoverageMethod::None,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_covering(
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: &CoverageWitness,
        method: CoverageMethod,
        args: &BTreeMap<String, Value>,
    ) -> Result<(Vec<u8>, CompletionProofId), String> {
        let claims = CoveringClaims {
            claims: CertificateClaims {
                program,
                snapshot,
                case,
                query: query.as_str(),
                valid,
                known,
                constraints,
                answer,
                args,
            },
            tag: Self::covering_tag(method)?,
            examined: witness.examined,
            total: witness.total,
            incomplete: witness.incomplete,
            witness_answer: &witness.answer,
        };
        let payload = crate::canonical_to_vec(&claims).map_err(|e| e.to_string())?;
        let id = CompletionProofId::of(&payload);
        Ok((payload, id))
    }

    /// Content hash of FiniteReplay covering claims. Pass this as `id` to
    /// [`verified_covering`]. For Structural evidence use
    /// [`structural_claims_id`].
    #[allow(clippy::too_many_arguments)]
    pub fn covering_claims_id(
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: &CoverageWitness,
    ) -> Result<CompletionProofId, String> {
        Self::covering_claims_id_with_args(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            witness,
            &BTreeMap::new(),
        )
    }

    /// [`covering_claims_id`] with query arguments bound into the digest.
    #[allow(clippy::too_many_arguments)]
    pub fn covering_claims_id_with_args(
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: &CoverageWitness,
        args: &BTreeMap<String, Value>,
    ) -> Result<CompletionProofId, String> {
        let (_payload, id) = Self::bind_covering(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            witness,
            CoverageMethod::FiniteReplay,
            args,
        )?;
        Ok(id)
    }

    /// Content hash of Structural covering claims. Pass this as `id` to
    /// [`verified_structural`]. Distinct from [`covering_claims_id`].
    #[allow(clippy::too_many_arguments)]
    pub fn structural_claims_id(
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: &CoverageWitness,
    ) -> Result<CompletionProofId, String> {
        Self::structural_claims_id_with_args(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            witness,
            &BTreeMap::new(),
        )
    }

    /// [`structural_claims_id`] with query arguments bound into the digest.
    #[allow(clippy::too_many_arguments)]
    pub fn structural_claims_id_with_args(
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: &CoverageWitness,
        args: &BTreeMap<String, Value>,
    ) -> Result<CompletionProofId, String> {
        let (_payload, id) = Self::bind_covering(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            witness,
            CoverageMethod::Structural,
            args,
        )?;
        Ok(id)
    }

    /// Bind an untrusted proof id to checked claims plus a shape-complete
    /// witness. Completeness is `is_complete` and a matching answer. This
    /// is not covering: [`is_covering`] is false.
    #[allow(clippy::too_many_arguments)]
    pub fn verified_structural(
        id: CompletionProofId,
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: CoverageWitness,
    ) -> Result<Self, String> {
        Self::verified_structural_with_args(
            id,
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            witness,
            &BTreeMap::new(),
        )
    }

    /// [`verified_structural`] with query arguments bound into the claims.
    #[allow(clippy::too_many_arguments)]
    pub fn verified_structural_with_args(
        id: CompletionProofId,
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: CoverageWitness,
        args: &BTreeMap<String, Value>,
    ) -> Result<Self, String> {
        Self::require_complete_witness(&witness, answer)?;
        let (payload, expected) = Self::bind_covering(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            &witness,
            CoverageMethod::Structural,
            args,
        )?;
        if expected != id {
            return Err(format!(
                "completion proof id {} does not match covering claims {}",
                id.hex(),
                expected.hex()
            ));
        }
        Ok(Self {
            id,
            claims_digest: Self::digest_from_payload(&payload),
            method: CoverageMethod::Structural,
        })
    }

    /// Bind an untrusted proof id to checked claims plus a FiniteReplay
    /// witness. Empty `branches` (including [`CoverageWitness::complete`])
    /// is not covering. Method is FiniteReplay only after unique worlds
    /// with matching answers are present. A claims digest is not covering.
    #[allow(clippy::too_many_arguments)]
    pub fn verified_covering(
        id: CompletionProofId,
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: CoverageWitness,
    ) -> Result<Self, String> {
        Self::verified_covering_with_args(
            id,
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            witness,
            &BTreeMap::new(),
        )
    }

    /// [`verified_covering`] with query arguments bound into the claims.
    #[allow(clippy::too_many_arguments)]
    pub fn verified_covering_with_args(
        id: CompletionProofId,
        program: ModuleId,
        snapshot: SourceSnapshotId,
        case: &CaseRecord,
        query: &QueryName,
        valid: Instant,
        known: Instant,
        constraints: &BTreeSet<OpenRequest>,
        answer: &Value,
        witness: CoverageWitness,
        args: &BTreeMap<String, Value>,
    ) -> Result<Self, String> {
        Self::require_replay_witness(&witness, answer)?;
        let (payload, expected) = Self::bind_covering(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
            &witness,
            CoverageMethod::FiniteReplay,
            args,
        )?;
        if expected != id {
            return Err(format!(
                "completion proof id {} does not match covering claims {}",
                id.hex(),
                expected.hex()
            ));
        }
        Ok(Self {
            id,
            claims_digest: Self::digest_from_payload(&payload),
            method: CoverageMethod::FiniteReplay,
        })
    }

    fn require_complete_witness(witness: &CoverageWitness, answer: &Value) -> Result<(), String> {
        if !witness.is_complete() {
            return Err("coverage witness is incomplete".into());
        }
        if witness.answer != *answer {
            return Err("coverage witness answer does not match claimed answer".into());
        }
        Ok(())
    }

    fn require_replay_witness(witness: &CoverageWitness, answer: &Value) -> Result<(), String> {
        Self::require_complete_witness(witness, answer)?;
        if witness.branches.is_empty() {
            return Err("coverage witness has no branches".into());
        }
        if witness.branches.len() != witness.total || witness.branches.len() != witness.examined {
            return Err(format!(
                "coverage witness branches length {} does not match examined {} / total {}",
                witness.branches.len(),
                witness.examined,
                witness.total
            ));
        }
        if has_duplicate_bindings(&witness.branches) {
            return Err("duplicate worlds in coverage witness".into());
        }
        if witness
            .branches
            .iter()
            .any(|branch| branch.answer != *answer || branch.answer != witness.answer)
        {
            return Err("coverage witness branch answer does not match claimed answer".into());
        }
        Ok(())
    }

    fn digest_from_payload(payload: &[u8]) -> [u8; 16] {
        let hash = blake3::hash(payload);
        let mut claims_digest = [0u8; 16];
        claims_digest.copy_from_slice(&hash.as_bytes()[..16]);
        claims_digest
    }

    pub fn id(self) -> CompletionProofId {
        self.id
    }

    pub fn claims_digest(self) -> [u8; 16] {
        self.claims_digest
    }

    pub fn method(self) -> CoverageMethod {
        self.method
    }

    /// True only for [`CoverageMethod::FiniteReplay`].
    pub fn is_covering(self) -> bool {
        matches!(self.method, CoverageMethod::FiniteReplay)
    }
}

fn has_duplicate_bindings(branches: &[BranchClaim]) -> bool {
    branches.iter().enumerate().any(|(index, branch)| {
        branches[..index]
            .iter()
            .any(|prior| prior.bindings == branch.bindings)
    })
}

impl fmt::Debug for CheckedCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedCertificate")
            .field("id", &self.id)
            .field("method", &self.method)
            .field("covering", &self.is_covering())
            .finish_non_exhaustive()
    }
}

impl Serialize for CheckedCertificate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.id.hex())
    }
}

fn deserialize_certificate<'de, D>(deserializer: D) -> Result<Option<CheckedCertificate>, D::Error>
where
    D: Deserializer<'de>,
{
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(_) => Err(D::Error::custom(
            "convergence certificates cannot be rebuilt from a raw id; use CheckedCertificate::verified",
        )),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Outcome<T = Value> {
    Determinate {
        value: T,
        trace: TraceId,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_certificate"
        )]
        convergence_certificate: Option<CheckedCertificate>,
        ignored_open_issues: BTreeSet<OpenRequest>,
    },
    Contingent {
        alternatives: BTreeMap<String, T>,
        pivots: BTreeSet<OpenRequest>,
        trace: TraceId,
    },
    Suspended {
        requests: BTreeSet<OpenRequest>,
        trace: TraceId,
    },
    NormConflict {
        doctrines: Vec<CoreConflictDoctrine>,
        trace: TraceId,
    },
    OutsideCompetence {
        request: OpenRequest,
        reason: String,
        trace: TraceId,
    },
    Inconsistent {
        core: Vec<String>,
        trace: TraceId,
    },
}

impl<T> Outcome<T> {
    /// Construct a determinate result. Nonempty ignored issues require a
    /// covering certificate. A claims-digest binder is not covering.
    pub fn determinate(
        value: T,
        trace: TraceId,
        certificate: Option<CheckedCertificate>,
        ignored: BTreeSet<OpenRequest>,
    ) -> Result<Self, String> {
        if !ignored.is_empty() {
            match certificate {
                None => {
                    return Err(
                        "ignored_open_issues requires a checked convergence certificate".into(),
                    );
                }
                Some(cert) if !cert.is_covering() => {
                    return Err("ignored_open_issues requires a covering certificate".into());
                }
                Some(_) => {}
            }
        }
        Ok(Self::Determinate {
            value,
            trace,
            convergence_certificate: certificate,
            ignored_open_issues: ignored,
        })
    }

    pub fn is_determinate(&self) -> bool {
        matches!(self, Self::Determinate { .. })
    }

    pub fn trace(&self) -> TraceId {
        match self {
            Self::Determinate { trace, .. }
            | Self::Contingent { trace, .. }
            | Self::Suspended { trace, .. }
            | Self::NormConflict { trace, .. }
            | Self::OutsideCompetence { trace, .. }
            | Self::Inconsistent { trace, .. } => *trace,
        }
    }
}

impl OpenRequest {
    pub fn as_operation(&self) -> OpenOperation {
        match self {
            OpenRequest::NeedEvidence { .. } => OpenOperation::Observe,
            OpenRequest::NeedJudgment { .. } => OpenOperation::Determine,
            OpenRequest::NeedChoice { .. } => OpenOperation::Choose,
            OpenRequest::NeedInterpretation { .. } => OpenOperation::Interpret,
            OpenRequest::NeedApplicableLaw { .. } => OpenOperation::SelectApplicableLaw,
            OpenRequest::NeedConflict { .. } => OpenOperation::ResolveNormConflict,
            OpenRequest::NeedCustom { .. } => OpenOperation::Observe,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::NodeId;
    use crate::patterns::{ContextPattern, PropPattern};
    use crate::value::PropTerm;

    fn sample_request() -> OpenRequest {
        OpenRequest::NeedEvidence {
            issue: PropPattern::Ground(PropTerm::new("P", vec![])),
            schema: "S".into(),
        }
    }

    #[test]
    fn ignored_issues_require_certificate() {
        let trace = TraceId::of(b"t");
        let mut ignored = BTreeSet::new();
        ignored.insert(sample_request());
        let err = Outcome::<Value>::determinate(Value::Unit, trace, None, ignored).unwrap_err();
        assert!(err.contains("convergence"));
        let _ = NodeId::of(b"ctx");
        let _ = ContextPattern::CurrentContext;
    }

    fn complete_witness(answer: Value) -> CoverageWitness {
        CoverageWitness::complete(1, answer)
    }

    fn replay_witness(answer: Value) -> CoverageWitness {
        CoverageWitness {
            examined: 1,
            total: 1,
            incomplete: false,
            answer: answer.clone(),
            branches: vec![BranchClaim {
                bindings: BTreeMap::new(),
                answer,
            }],
        }
    }

    #[test]
    fn determinate_fields_are_camel_case() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mut ignored = BTreeSet::new();
        ignored.insert(sample_request());
        let answer = Value::Bool(true);
        let witness = replay_witness(answer.clone());
        let id = CheckedCertificate::covering_claims_id(
            program, snapshot, &case, &query, t, t, &ignored, &answer, &witness,
        )
        .unwrap();
        let cert = CheckedCertificate::verified_covering(
            id, program, snapshot, &case, &query, t, t, &ignored, &answer, witness,
        )
        .unwrap();
        assert!(cert.is_covering());
        let out = Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), ignored).unwrap();
        let v = serde_json::to_value(&out).unwrap();
        assert_eq!(v["kind"], "determinate");
        assert!(v["trace"].is_string(), "{v}");
        assert_eq!(v["trace"].as_str().unwrap().len(), 32);
        assert!(v["convergenceCertificate"].is_string(), "{v}");
        assert_eq!(v["convergenceCertificate"], id.hex());
        assert!(v["ignoredOpenIssues"].is_array(), "{v}");
        assert!(v.get("convergence_certificate").is_none());
        assert!(v.get("ignored_open_issues").is_none());
    }

    #[test]
    fn verified_structural_is_not_covering_for_ignored_issues() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mut ignored = BTreeSet::new();
        ignored.insert(sample_request());
        let answer = Value::Bool(true);
        let witness = complete_witness(answer.clone());
        let id = CheckedCertificate::structural_claims_id(
            program, snapshot, &case, &query, t, t, &ignored, &answer, &witness,
        )
        .unwrap();
        let cert = CheckedCertificate::verified_structural(
            id, program, snapshot, &case, &query, t, t, &ignored, &answer, witness,
        )
        .unwrap();
        assert_eq!(cert.method(), CoverageMethod::Structural);
        assert!(!cert.is_covering());
        let err = Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), ignored).unwrap_err();
        assert!(err.contains("covering"), "{err}");
    }

    #[test]
    fn verified_covering_with_ignored_issues_is_determinate() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mut ignored = BTreeSet::new();
        ignored.insert(sample_request());
        let answer = Value::Bool(true);
        let witness = replay_witness(answer.clone());
        let id = CheckedCertificate::covering_claims_id(
            program, snapshot, &case, &query, t, t, &ignored, &answer, &witness,
        )
        .unwrap();
        let cert = CheckedCertificate::verified_covering(
            id, program, snapshot, &case, &query, t, t, &ignored, &answer, witness,
        )
        .unwrap();
        assert_eq!(cert.method(), CoverageMethod::FiniteReplay);
        assert!(cert.is_covering());
        let out = Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), ignored).unwrap();
        assert!(out.is_determinate());
    }

    #[test]
    fn verified_digest_is_not_covering_for_ignored_issues() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mut ignored = BTreeSet::new();
        ignored.insert(sample_request());
        let answer = Value::Bool(true);
        let id = CheckedCertificate::claims_id(
            program, snapshot, &case, &query, t, t, &ignored, &answer,
        )
        .unwrap();
        let cert = CheckedCertificate::verified(
            id, program, snapshot, &case, &query, t, t, &ignored, &answer,
        )
        .unwrap();
        assert_eq!(cert.method(), CoverageMethod::None);
        assert!(!cert.is_covering());
        let err = Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), ignored).unwrap_err();
        assert!(err.contains("covering"), "{err}");
    }

    #[test]
    fn verified_covering_rejects_incomplete_or_mismatched_witness() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let constraints = BTreeSet::new();
        let answer = Value::Bool(true);
        let incomplete = CoverageWitness {
            examined: 0,
            total: 0,
            incomplete: false,
            answer: answer.clone(),
            branches: Vec::new(),
        };
        let err = CheckedCertificate::verified_covering(
            CompletionProofId::of(b"x"),
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            incomplete,
        )
        .unwrap_err();
        assert!(err.contains("incomplete"), "{err}");

        let mismatched = complete_witness(Value::Bool(false));
        let err = CheckedCertificate::verified_covering(
            CompletionProofId::of(b"x"),
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            mismatched,
        )
        .unwrap_err();
        assert!(err.contains("answer"), "{err}");

        let duplicates = CoverageWitness {
            examined: 2,
            total: 2,
            incomplete: false,
            answer: answer.clone(),
            branches: vec![
                BranchClaim {
                    bindings: BTreeMap::new(),
                    answer: answer.clone(),
                },
                BranchClaim {
                    bindings: BTreeMap::new(),
                    answer: answer.clone(),
                },
            ],
        };
        let err = CheckedCertificate::verified_covering(
            CompletionProofId::of(b"x"),
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            duplicates,
        )
        .unwrap_err();
        assert!(err.contains("duplicate"), "{err}");

        let short = CoverageWitness {
            examined: 2,
            total: 2,
            incomplete: false,
            answer: answer.clone(),
            branches: vec![BranchClaim {
                bindings: BTreeMap::new(),
                answer: answer.clone(),
            }],
        };
        let err = CheckedCertificate::verified_covering(
            CompletionProofId::of(b"x"),
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            short,
        )
        .unwrap_err();
        assert!(
            err.contains("branches") || err.contains("length") || err.contains("total"),
            "{err}"
        );

        let incomplete = CoverageWitness {
            examined: 0,
            total: 0,
            incomplete: false,
            answer: answer.clone(),
            branches: Vec::new(),
        };
        let err = CheckedCertificate::verified_structural(
            CompletionProofId::of(b"x"),
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            incomplete,
        )
        .unwrap_err();
        assert!(err.contains("incomplete"), "{err}");
        let mismatched = complete_witness(Value::Bool(false));
        let err = CheckedCertificate::verified_structural(
            CompletionProofId::of(b"x"),
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            mismatched,
        )
        .unwrap_err();
        assert!(err.contains("answer"), "{err}");
    }

    #[test]
    fn covering_id_differs_from_claims_digest() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let constraints = BTreeSet::new();
        let answer = Value::Unit;
        let witness = complete_witness(answer.clone());
        let digest = CheckedCertificate::claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
        )
        .unwrap();
        let covering = CheckedCertificate::covering_claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            &witness,
        )
        .unwrap();
        assert_ne!(digest, covering);
    }

    #[test]
    fn coverage_witness_zero_space_is_not_complete() {
        let empty = CoverageWitness {
            examined: 0,
            total: 0,
            incomplete: false,
            answer: Value::Unit,
            branches: Vec::new(),
        };
        assert!(!empty.is_complete());
        let cut_short = CoverageWitness {
            examined: 1,
            total: 2,
            incomplete: true,
            answer: Value::Unit,
            branches: Vec::new(),
        };
        assert!(!cut_short.is_complete());
        assert!(complete_witness(Value::Unit).is_complete());
    }

    #[test]
    fn coverage_witness_deserializes_without_branches() {
        let json = serde_json::json!({
            "examined": 1,
            "total": 1,
            "incomplete": false,
            "answer": {"kind": "unit"}
        });
        let witness: CoverageWitness = serde_json::from_value(json).unwrap();
        assert!(witness.branches.is_empty());
        assert!(witness.is_complete());
        assert_eq!(witness.answer, Value::Unit);
    }

    #[test]
    fn coverage_witness_is_complete_ignores_branches() {
        let mut witness = CoverageWitness::complete(1, Value::Unit);
        assert!(witness.branches.is_empty());
        assert!(witness.is_complete());
        witness.branches.push(BranchClaim {
            bindings: BTreeMap::new(),
            answer: Value::Unit,
        });
        assert!(witness.is_complete());
    }

    #[test]
    fn evaluation_report_from_outcome_copies_suspended_requests() {
        let mut requests = BTreeSet::new();
        requests.insert(sample_request());
        let outcome = Outcome::<Value>::Suspended {
            requests: requests.clone(),
            trace: TraceId::of(b"t"),
        };
        let report = EvaluationReport::from_outcome(outcome);
        assert_eq!(report.unresolved, requests);
        assert!(report.coverage.is_none());
        assert_eq!(report.trust, TrustProfile::Unauthenticated);
        assert_eq!(report.provenance_root, TraceId::of(b"t"));
        assert_eq!(report.execution_mode, ExecutionMode::Operative);
        assert!(report.assumptions.is_empty());
        assert_eq!(report.verification_method, CoverageMethod::None);
    }

    #[test]
    fn evaluation_report_deserializes_without_new_envelope_fields() {
        let json = serde_json::json!({
            "outcome": {
                "kind": "suspended",
                "requests": [],
                "trace": TraceId::of(b"t").hex()
            },
            "unresolved": [],
            "coverage": null,
            "trust": "unauthenticated",
            "provenanceRoot": TraceId::of(b"t").hex()
        });
        let report: EvaluationReport<Value> = serde_json::from_value(json).unwrap();
        assert_eq!(report.execution_mode, ExecutionMode::Operative);
        assert!(report.assumptions.is_empty());
        assert_eq!(report.verification_method, CoverageMethod::None);
    }

    #[test]
    fn trust_profile_serializes_camel_case() {
        assert_eq!(
            serde_json::to_value(TrustProfile::ByteVerified).unwrap(),
            "byteVerified"
        );
        assert_eq!(
            serde_json::to_value(TrustProfile::Unauthenticated).unwrap(),
            "unauthenticated"
        );
        assert_eq!(TrustProfile::default(), TrustProfile::Unauthenticated);
    }

    #[test]
    fn coverage_method_and_execution_mode_serialize_camel_case() {
        assert_eq!(serde_json::to_value(CoverageMethod::None).unwrap(), "none");
        assert_eq!(
            serde_json::to_value(CoverageMethod::Structural).unwrap(),
            "structural"
        );
        assert_eq!(
            serde_json::to_value(CoverageMethod::FiniteReplay).unwrap(),
            "finiteReplay"
        );
        assert_eq!(
            serde_json::to_value(ExecutionMode::Operative).unwrap(),
            "operative"
        );
        assert_eq!(
            serde_json::to_value(ExecutionMode::Scenario).unwrap(),
            "scenario"
        );
        assert_eq!(CoverageMethod::default(), CoverageMethod::None);
        assert_eq!(ExecutionMode::default(), ExecutionMode::Operative);
    }

    #[test]
    fn verified_empty_constraints_is_not_covering() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let constraints = BTreeSet::new();
        let answer = Value::Unit;
        let id = CheckedCertificate::claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
        )
        .unwrap();
        let cert = CheckedCertificate::verified(
            id,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
        )
        .unwrap();
        assert!(!cert.is_covering());
        let _ =
            Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), BTreeSet::new()).unwrap();
    }

    #[test]
    fn checked_certificate_rejects_raw_proof_id() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let fake = CompletionProofId::of(b"P11");
        let err = CheckedCertificate::verified(
            fake,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &BTreeSet::new(),
            &Value::Unit,
        )
        .unwrap_err();
        assert!(err.contains("does not match"), "{err}");
    }

    #[test]
    fn certificate_cannot_be_deserialized_from_raw_id() {
        let trace = TraceId::of(b"t");
        let json = serde_json::json!({
            "kind": "determinate",
            "value": {"kind": "unit"},
            "trace": trace.hex(),
            "convergenceCertificate": CompletionProofId::of(b"P11").hex(),
            "ignoredOpenIssues": []
        });
        let err = serde_json::from_value::<Outcome<Value>>(json).unwrap_err();
        assert!(
            err.to_string().contains("CheckedCertificate::verified"),
            "{err}"
        );
        let raw = serde_json::json!(CompletionProofId::of(b"P11").hex());
        let bound = serde_json::from_value::<BoundClaims>(raw.clone()).unwrap_err();
        assert!(
            bound.to_string().contains("CheckedCertificate::verified"),
            "{bound}"
        );
        let structural =
            serde_json::from_value::<StructurallyCheckedCoverage>(raw.clone()).unwrap_err();
        assert!(
            structural
                .to_string()
                .contains("CheckedCertificate::verified_structural"),
            "{structural}"
        );
        let replay = serde_json::from_value::<ReplayVerifiedCoverage>(raw).unwrap_err();
        assert!(
            replay
                .to_string()
                .contains("CheckedCertificate::verified_covering"),
            "{replay}"
        );
    }

    #[test]
    fn verified_covering_with_empty_branches_is_err() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let constraints = BTreeSet::new();
        let answer = Value::Bool(true);
        let witness = complete_witness(answer.clone());
        assert!(witness.branches.is_empty());
        let id = CheckedCertificate::covering_claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            &witness,
        )
        .unwrap();
        let err = CheckedCertificate::verified_covering(
            id,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            witness,
        )
        .unwrap_err();
        assert!(
            err.contains("branch") || err.contains("empty") || err.contains("no branches"),
            "{err}"
        );
    }

    #[test]
    fn structural_and_finite_replay_covering_claims_ids_differ() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let constraints = BTreeSet::new();
        let answer = Value::Bool(true);
        let witness = replay_witness(answer.clone());
        let structural = CheckedCertificate::structural_claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            &witness,
        )
        .unwrap();
        let replay = CheckedCertificate::covering_claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            &witness,
        )
        .unwrap();
        assert_ne!(structural, replay);
        let err = CheckedCertificate::verified_covering(
            structural,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            witness.clone(),
        )
        .unwrap_err();
        assert!(err.contains("does not match"), "{err}");
        let err = CheckedCertificate::verified_structural(
            replay,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            witness,
        )
        .unwrap_err();
        assert!(err.contains("does not match"), "{err}");
    }

    #[test]
    fn replay_verified_coverage_try_from_rejects_structural_and_none() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let constraints = BTreeSet::new();
        let answer = Value::Bool(true);
        let structural_witness = complete_witness(answer.clone());
        let structural_id = CheckedCertificate::structural_claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            &structural_witness,
        )
        .unwrap();
        let structural = CheckedCertificate::verified_structural(
            structural_id,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            structural_witness,
        )
        .unwrap();
        assert!(!structural.is_covering());
        ReplayVerifiedCoverage::try_from(structural).expect_err("structural is not covering");
        BoundClaims::try_from(structural).expect_err("structural is not BoundClaims");
        StructurallyCheckedCoverage::try_from(structural).expect("structural wraps");

        let none_id = CheckedCertificate::claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
        )
        .unwrap();
        let none = CheckedCertificate::verified(
            none_id,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
        )
        .unwrap();
        ReplayVerifiedCoverage::try_from(none).expect_err("digest is not covering");
        BoundClaims::try_from(none).expect("digest wraps BoundClaims");
        StructurallyCheckedCoverage::try_from(none).expect_err("digest is not structural");

        let replay_w = replay_witness(answer.clone());
        let replay_id = CheckedCertificate::covering_claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            &replay_w,
        )
        .unwrap();
        let covering = CheckedCertificate::verified_covering(
            replay_id,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            replay_w,
        )
        .unwrap();
        let wrapped = ReplayVerifiedCoverage::try_from(covering).expect("finite replay wraps");
        assert!(wrapped.certificate().is_covering());
        BoundClaims::try_from(covering).expect_err("finite replay is not BoundClaims");
        StructurallyCheckedCoverage::try_from(covering)
            .expect_err("finite replay is not structural");
    }

    #[test]
    fn outcome_determinate_rejects_structural_for_ignored_issues() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mut ignored = BTreeSet::new();
        ignored.insert(sample_request());
        let answer = Value::Bool(true);
        let witness = complete_witness(answer.clone());
        let id = CheckedCertificate::structural_claims_id(
            program, snapshot, &case, &query, t, t, &ignored, &answer, &witness,
        )
        .unwrap();
        let cert = CheckedCertificate::verified_structural(
            id, program, snapshot, &case, &query, t, t, &ignored, &answer, witness,
        )
        .unwrap();
        let err = Outcome::determinate(answer, TraceId::of(b"t"), Some(cert), ignored).unwrap_err();
        assert!(err.contains("covering"), "{err}");
    }

    #[test]
    fn query_args_are_bound_into_claims() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let constraints = BTreeSet::new();
        let answer = Value::Unit;
        let empty = CheckedCertificate::claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
        )
        .unwrap();
        let mut args = BTreeMap::new();
        args.insert("x".into(), Value::Int(1));
        let with_args = CheckedCertificate::claims_id_with_args(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            &args,
        )
        .unwrap();
        assert_ne!(empty, with_args);
        let cert = CheckedCertificate::verified_with_args(
            with_args,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
            &args,
        )
        .unwrap();
        assert_eq!(cert.method(), CoverageMethod::None);
        BoundClaims::try_from(cert).expect("args-bound digest is BoundClaims");
    }
}
