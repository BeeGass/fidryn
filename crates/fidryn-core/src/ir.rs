//! Canonical Core IR. The parser's AST is not the semantics.

use crate::diagnostics::Span;
use crate::effects::EffectName;
use crate::ids::{
    ClauseId, EffectId, JurisdictionId, ModuleId, NodeId, OriginId, RuleId, SourceManifestId,
    SourceSnapshotId,
};
use crate::patterns::LegalEffectPattern;
use crate::positions::Position;
use crate::time::Interval;
use crate::types::Type;
use crate::value::{PropTerm, Term};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreModule {
    /// Assigned by the checker from the module name (`ModuleId::of`). Not a
    /// content hash; see [`Self::content_fingerprint`].
    pub id: ModuleId,
    pub name: String,
    pub version: String,
    pub snapshot: SourceSnapshotId,
    pub manifest: SourceManifestId,
    pub jurisdiction: crate::ids::JurisdictionId,
    pub outside_scope: Vec<String>,
    pub declarations: Vec<CoreDecl>,
    #[serde(default)]
    pub nominations: Vec<CoreNomination>,
    pub queries: Vec<CoreQuery>,
    pub verifications: Vec<CoreVerify>,
    pub assertions: Vec<CoreAssertion>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreNomination {
    pub candidate: String,
    pub office: String,
    pub rank: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum CoreDecl {
    Source(CoreSource),
    Entity(CoreEntity),
    RecordType(CoreRecordType),
    Office(CoreOffice),
    Proposition(CoreProposition),
    Observation(CoreObservation),
    Fact(CoreFact),
    Function(CoreFunction),
    EffectDecl(CoreEffectDecl),
    Rule(CoreRule),
    Position(Position),
    Power(CorePower),
    Duty(CoreDuty),
    Judgment(CoreJudgment),
    Decision(CoreDecision),
    LegalAct(CoreLegalAct),
    InterpretationFamily(CoreInterpretationFamily),
    ConflictDoctrine(CoreConflictDoctrine),
    Clause(CoreClause),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreSource {
    pub id: NodeId,
    pub name: String,
    pub kind: String,
    pub artifact: Option<String>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreEntity {
    pub id: NodeId,
    pub name: String,
    pub ty: Type,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreRecordType {
    pub id: NodeId,
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub is_evidence: bool,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreOffice {
    pub id: NodeId,
    pub name: String,
    pub occupant: Type,
    pub cardinality_min: u32,
    pub cardinality_max: Option<u32>,
    pub competence: Vec<String>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreProposition {
    pub id: NodeId,
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreObservation {
    pub id: NodeId,
    pub name: String,
    pub record_binder: String,
    pub validators: Vec<Guard>,
    pub establishes: PropTerm,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreFunction {
    pub id: NodeId,
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub result: Type,
    pub effects: BTreeSet<EffectName>,
    pub is_calc: bool,
    #[serde(default)]
    pub fuel: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Term>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreEffectDecl {
    pub id: NodeId,
    pub name: String,
    pub operations: Vec<CoreEffectOp>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreEffectOp {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub result: Type,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreFact {
    pub id: NodeId,
    pub relation: String,
    pub arguments: Vec<Term>,
    pub meta: NodeMeta,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleKind {
    Derive,
    Constitutive,
    Prescriptive,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreRule {
    pub id: NodeId,
    pub name: String,
    pub kind: RuleKind,
    pub binders: Vec<String>,
    pub selection: Option<CoreSelection>,
    pub guard: Guard,
    pub consequences: Vec<CoreEffect>,
    pub fallback: Option<Vec<CoreEffect>>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreSelection {
    pub binder: String,
    pub finite_domain: Term,
    pub predicate: Guard,
    pub ordering: Vec<String>,
    pub require_unique_keys: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SelectionResult<T> {
    Selected(T),
    NoCandidate,
    OpenSelection(Vec<crate::outcome::OpenRequest>),
    Ambiguous(Vec<T>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorePower {
    pub id: NodeId,
    pub name: String,
    pub holder: Term,
    pub subject: Term,
    pub effect: Term,
    pub active_while: Guard,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreDuty {
    pub id: NodeId,
    pub name: String,
    pub bearer: Term,
    pub claimant: Option<Term>,
    pub attaches: Guard,
    pub content: Vec<Term>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreJudgment {
    pub id: NodeId,
    pub name: String,
    pub decides: PropTerm,
    pub authority: Term,
    pub record_requires: Vec<(String, Type)>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreDecision {
    pub id: NodeId,
    pub name: String,
    pub binders: Vec<(String, Type)>,
    pub requirements: Vec<Guard>,
    pub option_space: Term,
    pub declared_result: Option<DecisionReturn>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionReturn {
    pub result_type: Type,
    pub expression: Term,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreLegalAct {
    pub id: NodeId,
    pub name: String,
    pub performer: Term,
    pub validity: Vec<Guard>,
    pub effects: Vec<CoreEffect>,
    pub physical: Vec<Term>,
    pub non_derivations: Vec<CoreAssertion>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreInterpretationFamily {
    pub id: NodeId,
    pub name: String,
    pub source: Term,
    pub alternatives: Vec<(String, Vec<(PropTerm, bool)>)>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreConflictDoctrine {
    pub id: NodeId,
    pub name: String,
    pub guard: Guard,
    pub defeats: Vec<ConflictTarget>,
    pub as_to: Option<LegalEffectPattern>,
    pub reason: String,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConflictTarget {
    Clause(ClauseId),
    Rule(RuleId),
    Effect(EffectId),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreClause {
    pub id: ClauseId,
    pub name: String,
    pub rules: Vec<NodeId>,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreEffect {
    pub id: EffectId,
    pub consequence: Consequence,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Consequence {
    Derive(PropTerm),
    RecordOccurrence(Term),
    Establish(PropTerm),
    Terminate(PropTerm),
    Suspend(PropTerm),
    CreatePosition(Position),
    Discharge(String),
    Activate(String),
    Defeat { target: ConflictTarget },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Guard {
    Satisfied,
    Operative(PropTerm, String),
    CompletedAct(String),
    EffectiveAct(String),
    Observed {
        schema: String,
        binder: String,
    },
    Derived(PropTerm),
    Compare {
        op: CompareOp,
        left: Term,
        right: Term,
    },
    And(Vec<Guard>),
    Or(Vec<Guard>),
    Not(Box<Guard>),
    Request(crate::outcome::OpenRequest),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreQuery {
    pub id: NodeId,
    pub name: String,
    pub binders: Vec<(String, Type)>,
    pub result_type: Type,
    pub effects: BTreeSet<EffectName>,
    pub automatic: bool,
    pub plan: QueryPlan,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum QueryPlan {
    Evaluate(Term),
    UniqueOccupant {
        office: Term,
    },
    EvaluateClause {
        clause: ClauseSelector,
        context: String,
        result: Term,
    },
    RunDecision {
        decision: String,
        arguments: Vec<Term>,
        result: DeclaredDecisionResult,
    },
    StatusOf {
        status: crate::patterns::LegalStatusPattern,
        when_present: Term,
        when_closed_absent: Term,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClauseSelector {
    Instantiated {
        clause: ClauseId,
        arguments: Vec<Term>,
    },
    Bound {
        binder: String,
        module: ModuleId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredDecisionResult {
    pub expected_type: Type,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreVerify {
    pub id: NodeId,
    pub name: String,
    pub bounds: VerificationBounds,
    pub formula: String,
    pub meta: NodeMeta,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationBounds {
    pub persons: u32,
    pub events: u32,
    pub time_points: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreAssertion {
    NonDerivation {
        antecedent: Guard,
        forbidden: LegalEffectPattern,
    },
    Invariant(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeMeta {
    pub span: Option<Span>,
    pub source: Option<String>,
    pub jurisdiction: JurisdictionId,
    pub valid_time: Interval,
    pub record_time: Interval,
    pub origin: OriginId,
}

impl CoreModule {
    pub fn query(&self, name: &str) -> Option<&CoreQuery> {
        self.queries.iter().find(|q| q.name == name)
    }

    /// Blake3 of canonical JSON over name, version, queries, and declarations.
    ///
    /// Distinct from [`Self::id`], which is name-based via [`ModuleId::of`].
    pub fn content_fingerprint(&self) -> Result<[u8; 32], String> {
        #[derive(Serialize)]
        struct Fingerprint<'a> {
            name: &'a str,
            version: &'a str,
            queries: &'a [CoreQuery],
            declarations: &'a [CoreDecl],
        }
        let bytes = crate::canonical_to_vec(&Fingerprint {
            name: &self.name,
            version: &self.version,
            queries: &self.queries,
            declarations: &self.declarations,
        })
        .map_err(|e| e.to_string())?;
        Ok(*blake3::hash(&bytes).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{OriginId, SourceManifestId};

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

    #[test]
    fn content_fingerprint_hashes_name_version_queries_declarations() {
        let a = empty_module("Trust", "0.1.0");
        let b = empty_module("Trust", "0.1.0");
        assert_eq!(
            a.content_fingerprint().unwrap(),
            b.content_fingerprint().unwrap()
        );
        assert_ne!(
            a.content_fingerprint().unwrap(),
            empty_module("Other", "0.1.0")
                .content_fingerprint()
                .unwrap()
        );
        assert_ne!(
            a.content_fingerprint().unwrap(),
            empty_module("Trust", "0.2.0")
                .content_fingerprint()
                .unwrap()
        );

        let mut with_decl = empty_module("Trust", "0.1.0");
        with_decl.declarations.push(CoreDecl::Fact(CoreFact {
            id: NodeId::of(b"f"),
            relation: "Holds".into(),
            arguments: Vec::new(),
            meta: NodeMeta {
                span: None,
                source: None,
                jurisdiction: JurisdictionId::of(b"j"),
                valid_time: Interval::always(),
                record_time: Interval::always(),
                origin: OriginId::Direct(NodeId::of(b"f")),
            },
        }));
        assert_ne!(
            a.content_fingerprint().unwrap(),
            with_decl.content_fingerprint().unwrap()
        );
        assert_eq!(a.id, empty_module("Trust", "0.2.0").id);
    }
}
