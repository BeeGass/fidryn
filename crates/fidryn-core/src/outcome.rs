//! Honest evaluator results. Determinate is never a guess.

use crate::case::CaseRecord;
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

/// A convergence certificate bound to a claims digest.
///
/// [`verified`](Self::verified) is a claims-digest binder, not a covering
/// proof checker. Matching claims do not discharge open constraints. The
/// only constructor is [`CheckedCertificate::verified`]. A raw
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
}

impl CheckedCertificate {
    fn claims_payload(claims: &CertificateClaims<'_>) -> Result<Vec<u8>, String> {
        crate::canonical_to_vec(claims).map_err(|e| e.to_string())
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
        let (_payload, id) = Self::bind_claims(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
        )?;
        Ok(id)
    }

    /// Bind an untrusted proof id to checked claims. The id must be the
    /// content hash of those claims.
    ///
    /// This is a claims-digest binder, not a covering proof checker. Open
    /// `constraints` are hashed into the digest (the path used when
    /// constructing a certificate that names ignored issues) so they cannot
    /// be swapped silently; they are not discharged by a kernel proof.
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
        let (payload, expected) = Self::bind_claims(
            program,
            snapshot,
            case,
            query,
            valid,
            known,
            constraints,
            answer,
        )?;
        if expected != id {
            return Err(format!(
                "completion proof id {} does not match claims {}",
                id.hex(),
                expected.hex()
            ));
        }
        let hash = blake3::hash(&payload);
        let mut claims_digest = [0u8; 16];
        claims_digest.copy_from_slice(&hash.as_bytes()[..16]);
        Ok(Self { id, claims_digest })
    }

    pub fn id(self) -> CompletionProofId {
        self.id
    }

    pub fn claims_digest(self) -> [u8; 16] {
        self.claims_digest
    }
}

impl fmt::Debug for CheckedCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedCertificate")
            .field("id", &self.id)
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
    /// Construct a determinate result. Nonempty ignored issues require a certificate.
    pub fn determinate(
        value: T,
        trace: TraceId,
        certificate: Option<CheckedCertificate>,
        ignored: BTreeSet<OpenRequest>,
    ) -> Result<Self, String> {
        if !ignored.is_empty() && certificate.is_none() {
            return Err("ignored_open_issues requires a checked convergence certificate".into());
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
        let id = CheckedCertificate::claims_id(
            program, snapshot, &case, &query, t, t, &ignored, &answer,
        )
        .unwrap();
        let cert = CheckedCertificate::verified(
            id, program, snapshot, &case, &query, t, t, &ignored, &answer,
        )
        .unwrap();
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
    }
}
