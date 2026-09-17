//! Trusted checks for covering evaluation. Proof generation lives in
//! `fidryn-verify` / `fidryn-solve`; this crate only accepts or rejects
//! a covering claim.

use fidryn_core::{
    CaseRecord, CheckedCertificate, CompletionProofId, CoverageWitness, ModuleId, OpenRequest,
    QueryName, RunContext, SourceSnapshotId, Value,
};
use std::collections::BTreeSet;

/// Accept a covering witness and bind a certificate to the claimed answer.
///
/// The witness must be complete (`examined == total`, `examined > 0`,
/// `incomplete == false`) and its answer must equal `answer`. This does
/// not invent worlds; the caller supplies the examined space. A claims
/// digest is not covering proof.
#[allow(clippy::too_many_arguments)]
pub fn accept_covering(
    id: CompletionProofId,
    program: ModuleId,
    snapshot: SourceSnapshotId,
    case: &CaseRecord,
    query: &QueryName,
    ctx: &RunContext,
    constraints: &BTreeSet<OpenRequest>,
    answer: &Value,
    witness: CoverageWitness,
) -> Result<CheckedCertificate, String> {
    witness_is_admissible(&witness, answer)?;
    CheckedCertificate::verified_covering(
        id,
        program,
        snapshot,
        case,
        query,
        ctx.valid_time,
        ctx.record_time,
        constraints,
        answer,
        witness,
    )
}

/// Reject a claims-digest certificate as covering evidence for ignored issues.
pub fn reject_digest_as_covering(cert: &CheckedCertificate) -> bool {
    !cert.is_covering()
}

/// Admit only a complete finite-search witness whose answer matches `answer`.
///
/// Fails when the witness is incomplete, examined is not equal to total,
/// examined is zero, or the witnessed answer differs from the claim.
pub fn witness_is_admissible(witness: &CoverageWitness, answer: &Value) -> Result<(), String> {
    if witness.incomplete {
        return Err("coverage witness is incomplete".into());
    }
    if witness.examined == 0 {
        return Err("coverage witness examined no worlds".into());
    }
    if witness.examined != witness.total {
        return Err(format!(
            "coverage witness examined {} of {} worlds",
            witness.examined, witness.total
        ));
    }
    if &witness.answer != answer {
        return Err("coverage witness answer does not match claimed answer".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::Instant;

    fn complete_witness(answer: Value) -> CoverageWitness {
        CoverageWitness {
            examined: 3,
            total: 3,
            incomplete: false,
            answer,
        }
    }

    fn at() -> Instant {
        Instant::parse("2033-01-01T00:00:00Z").expect("instant")
    }

    fn claims_digest_certificate(answer: &Value) -> CheckedCertificate {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = at();
        let constraints = BTreeSet::new();
        let id = CheckedCertificate::claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            answer,
        )
        .expect("claims id");
        CheckedCertificate::verified(
            id,
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            answer,
        )
        .expect("verified digest")
    }

    #[test]
    fn test_witness_is_admissible_with_complete_matching_answer_returns_ok() {
        let answer = Value::Int(7);
        let witness = complete_witness(answer.clone());
        assert_eq!(witness_is_admissible(&witness, &answer), Ok(()));
    }

    #[test]
    fn test_witness_is_admissible_with_incomplete_witness_returns_err() {
        let answer = Value::Int(7);
        let witness = CoverageWitness {
            examined: 3,
            total: 3,
            incomplete: true,
            answer: answer.clone(),
        };
        let err = witness_is_admissible(&witness, &answer).expect_err("incomplete");
        assert!(err.contains("incomplete"), "{err}");
    }

    #[test]
    fn test_witness_is_admissible_with_examined_zero_returns_err() {
        let answer = Value::Int(7);
        let witness = CoverageWitness {
            examined: 0,
            total: 0,
            incomplete: false,
            answer: answer.clone(),
        };
        let err = witness_is_admissible(&witness, &answer).expect_err("examined 0");
        assert!(
            err.contains("no worlds") || err.contains("examined"),
            "{err}"
        );
    }

    #[test]
    fn test_witness_is_admissible_with_examined_not_equal_total_returns_err() {
        let answer = Value::Int(7);
        let witness = CoverageWitness {
            examined: 2,
            total: 3,
            incomplete: false,
            answer: answer.clone(),
        };
        let err = witness_is_admissible(&witness, &answer).expect_err("examined != total");
        assert!(err.contains("examined"), "{err}");
    }

    #[test]
    fn test_witness_is_admissible_with_answer_mismatch_returns_err() {
        let claimed = Value::Int(7);
        let witness = complete_witness(Value::Int(1));
        let err = witness_is_admissible(&witness, &claimed).expect_err("mismatch");
        assert!(err.contains("answer"), "{err}");
    }

    #[test]
    fn test_reject_digest_as_covering_with_claims_digest_returns_true() {
        let answer = Value::Bool(true);
        let cert = claims_digest_certificate(&answer);
        assert!(reject_digest_as_covering(&cert));
    }

    #[test]
    fn test_accept_covering_with_complete_witness_returns_covering_certificate() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = at();
        let ctx = RunContext::new(t, t);
        let constraints = BTreeSet::new();
        let answer = Value::Int(7);
        let witness = complete_witness(answer.clone());
        let id = CheckedCertificate::covering_claims_id(
            program,
            snapshot,
            &case,
            &query,
            ctx.valid_time,
            ctx.record_time,
            &constraints,
            &answer,
            &witness,
        )
        .expect("covering claims id");
        let cert = accept_covering(
            id,
            program,
            snapshot,
            &case,
            &query,
            &ctx,
            &constraints,
            &answer,
            witness,
        )
        .expect("accept covering");
        assert!(cert.is_covering());
        assert!(!reject_digest_as_covering(&cert));
    }

    #[test]
    fn test_accept_covering_with_digest_id_returns_err() {
        let program = ModuleId::of(b"m");
        let snapshot = SourceSnapshotId::of(b"s");
        let case = CaseRecord::default();
        let query = QueryName::from("q");
        let t = at();
        let ctx = RunContext::new(t, t);
        let constraints = BTreeSet::new();
        let answer = Value::Int(7);
        let witness = complete_witness(answer.clone());
        let digest_id = CheckedCertificate::claims_id(
            program,
            snapshot,
            &case,
            &query,
            t,
            t,
            &constraints,
            &answer,
        )
        .expect("claims id");
        let err = accept_covering(
            digest_id,
            program,
            snapshot,
            &case,
            &query,
            &ctx,
            &constraints,
            &answer,
            witness,
        )
        .expect_err("digest is not covering");
        assert!(
            err.contains("does not match") || err.contains("covering") || err.contains("digest"),
            "{err}"
        );
    }
}
