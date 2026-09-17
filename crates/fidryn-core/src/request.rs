//! Explicit evaluation request: program, query, case snapshot, clocks, mode.

use crate::case::CaseRecord;
use crate::ids::{ModuleId, ProgramDigest, QueryName, hex_encode};
use crate::ir::CoreModule;
use crate::outcome::ExecutionMode;
use crate::time::{Instant, RunContext};
use crate::value::Value;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use std::fmt;

/// Name-derived module id plus executable content fingerprint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramIdentity {
    pub module_id: ModuleId,
    pub fingerprint: ProgramDigest,
}

impl ProgramIdentity {
    pub fn of(module: &CoreModule) -> Result<Self, String> {
        Ok(Self {
            module_id: module.id,
            fingerprint: module.program_digest()?,
        })
    }
}

/// Blake3 of canonical case JSON.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CaseSnapshotIdentity([u8; 32]);

impl CaseSnapshotIdentity {
    pub fn of(case: &CaseRecord) -> Result<Self, String> {
        let json = crate::canonical_json(case).map_err(|e| e.to_string())?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"fidryn.case-snapshot/v0.1");
        hasher.update(&[0xff]);
        hasher.update(json.as_bytes());
        Ok(Self(*hasher.finalize().as_bytes()))
    }

    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn hex(&self) -> String {
        hex_encode(&self.0)
    }
}

impl Serialize for CaseSnapshotIdentity {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for CaseSnapshotIdentity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        ProgramDigest::from_hex(&text)
            .map(|digest| Self(*digest.as_bytes()))
            .map_err(D::Error::custom)
    }
}

impl fmt::Debug for CaseSnapshotIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CaseSnapshotIdentity({})", self.hex())
    }
}

impl fmt::Display for CaseSnapshotIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

/// Blake3 identity of an [`ExecutionRequest`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExecutionRequestId([u8; 32]);

impl ExecutionRequestId {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn hex(&self) -> String {
        hex_encode(&self.0)
    }
}

impl fmt::Debug for ExecutionRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ExecutionRequestId({})", self.hex())
    }
}

impl fmt::Display for ExecutionRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

/// One evaluation invocation: program, query, case snapshot, clocks, mode.
///
/// Optional `budget` is part of identity even when the evaluator does not
/// yet consume it. Warm and cold execution of the same request must agree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionRequest {
    pub program: ProgramIdentity,
    pub query: QueryName,
    pub args: BTreeMap<String, Value>,
    pub case: CaseSnapshotIdentity,
    pub valid_at: Instant,
    pub known_at: Instant,
    pub mode: ExecutionMode,
    pub budget: Option<u32>,
}

impl ExecutionRequest {
    /// Bind a compiled module, query arguments, case record, and clocks.
    pub fn from_eval(
        module: &CoreModule,
        query: &str,
        args: &BTreeMap<String, Value>,
        case: &CaseRecord,
        ctx: &RunContext,
        mode: ExecutionMode,
    ) -> Result<Self, String> {
        Ok(Self {
            program: ProgramIdentity::of(module)?,
            query: QueryName::from(query),
            args: args.clone(),
            case: CaseSnapshotIdentity::of(case)?,
            valid_at: ctx.valid_time,
            known_at: ctx.record_time,
            mode,
            budget: None,
        })
    }

    pub fn with_budget(mut self, budget: Option<u32>) -> Self {
        self.budget = budget;
        self
    }

    /// Blake3 of program identity, query, args, case snapshot, clocks, mode,
    /// and budget.
    pub fn identity(&self) -> ExecutionRequestId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"fidryn.execution-request/v0.1");
        hasher.update(&[0xff]);
        hasher.update(self.program.module_id.as_bytes());
        hasher.update(&[0xff]);
        hasher.update(self.program.fingerprint.as_bytes());
        hasher.update(&[0xff]);
        hasher.update(self.query.as_str().as_bytes());
        hasher.update(&[0xff]);
        for (key, value) in &self.args {
            hasher.update(key.as_bytes());
            hasher.update(&[0xff]);
            match crate::canonical_json(value) {
                Ok(json) => hasher.update(json.as_bytes()),
                Err(_) => hasher.update(b"unencodable"),
            };
            hasher.update(&[0xff]);
        }
        hasher.update(self.case.as_bytes());
        hasher.update(&[0xff]);
        hasher.update(self.valid_at.to_rfc3339().as_bytes());
        hasher.update(&[0xff]);
        hasher.update(self.known_at.to_rfc3339().as_bytes());
        hasher.update(&[0xff]);
        hasher.update(match self.mode {
            ExecutionMode::Operative => b"operative",
            ExecutionMode::Scenario => b"scenario",
        });
        hasher.update(&[0xff]);
        match self.budget {
            Some(n) => hasher.update(&n.to_le_bytes()),
            None => hasher.update(b"unbounded"),
        };
        ExecutionRequestId(*hasher.finalize().as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{JurisdictionId, ModuleId, SourceManifestId, SourceSnapshotId};
    use crate::ir::CoreModule;

    fn empty_module(name: &str, version: &str) -> CoreModule {
        CoreModule {
            id: ModuleId::of(name.as_bytes()),
            name: name.into(),
            version: version.into(),
            snapshot: SourceSnapshotId::of(b"s"),
            manifest: SourceManifestId::of(b"m"),
            jurisdiction: JurisdictionId::of(b"j"),
            outside_scope: Vec::new(),
            declarations: Vec::new(),
            nominations: Vec::new(),
            queries: Vec::new(),
            verifications: Vec::new(),
            assertions: Vec::new(),
        }
    }

    fn at() -> Instant {
        Instant::parse("2033-01-01T00:00:00Z").expect("timestamp")
    }

    fn ctx() -> RunContext {
        RunContext::new(at(), at())
    }

    fn request(
        module: &CoreModule,
        query: &str,
        args: &BTreeMap<String, Value>,
        case: &CaseRecord,
        mode: ExecutionMode,
    ) -> ExecutionRequest {
        ExecutionRequest::from_eval(module, query, args, case, &ctx(), mode).expect("request")
    }

    #[test]
    fn program_identity_uses_module_id_and_content_fingerprint() {
        let a = empty_module("Trust", "0.1.0");
        let b = empty_module("Other", "0.1.0");
        let id_a = ProgramIdentity::of(&a).expect("a");
        let id_b = ProgramIdentity::of(&b).expect("b");
        assert_eq!(id_a.module_id, a.id);
        assert_eq!(id_a.fingerprint, a.program_digest().expect("fp"));
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn case_snapshot_identity_is_canonical_json_hash() {
        let mut left = CaseRecord::default();
        left.facts.insert("n".into(), Value::Int(1));
        let mut right = CaseRecord::default();
        right.facts.insert("n".into(), Value::Int(2));
        assert_ne!(
            CaseSnapshotIdentity::of(&left).unwrap(),
            CaseSnapshotIdentity::of(&right).unwrap()
        );
        assert_eq!(
            CaseSnapshotIdentity::of(&left).unwrap(),
            CaseSnapshotIdentity::of(&left).unwrap()
        );
    }

    #[test]
    fn identity_changes_with_query_args_case_mode_and_budget() {
        let module = empty_module("Trust", "0.1.0");
        let case = CaseRecord::default();
        let empty = BTreeMap::new();
        let base = request(&module, "q", &empty, &case, ExecutionMode::Operative);
        assert_eq!(
            base.identity(),
            request(&module, "q", &empty, &case, ExecutionMode::Operative).identity()
        );
        assert_ne!(
            base.identity(),
            request(&module, "other", &empty, &case, ExecutionMode::Operative).identity()
        );
        let mut args = BTreeMap::new();
        args.insert("x".into(), Value::Int(1));
        assert_ne!(
            base.identity(),
            request(&module, "q", &args, &case, ExecutionMode::Operative).identity()
        );
        let mut other_case = CaseRecord::default();
        other_case.facts.insert("k".into(), Value::Bool(true));
        assert_ne!(
            base.identity(),
            request(&module, "q", &empty, &other_case, ExecutionMode::Operative).identity()
        );
        assert_ne!(
            base.identity(),
            request(&module, "q", &empty, &case, ExecutionMode::Scenario).identity()
        );
        assert_ne!(
            base.identity(),
            base.clone().with_budget(Some(64)).identity()
        );
        assert_eq!(base.budget, None);
        assert_eq!(base.valid_at, ctx().valid_time);
        assert_eq!(base.known_at, ctx().record_time);
    }

    #[test]
    fn same_name_different_body_changes_program_identity() {
        let mut a = empty_module("Trust", "0.1.0");
        a.version = "0.1.0".into();
        let mut b = empty_module("Trust", "0.1.0");
        b.version = "0.2.0".into();
        assert_eq!(a.id, b.id);
        assert_ne!(
            ProgramIdentity::of(&a).unwrap().fingerprint,
            ProgramIdentity::of(&b).unwrap().fingerprint
        );
        let case = CaseRecord::default();
        let empty = BTreeMap::new();
        assert_ne!(
            request(&a, "q", &empty, &case, ExecutionMode::Operative).identity(),
            request(&b, "q", &empty, &case, ExecutionMode::Operative).identity()
        );
    }
}
