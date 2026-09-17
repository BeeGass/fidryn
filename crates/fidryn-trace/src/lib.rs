//! Proof-relevant traces and canonical outcome JSON.
//!
//! [`OutcomeDocument`] is the single envelope for CLI `run`/`explore` and
//! the mill HTTP API. Field names match `schemas/outcome-v0.1.json`.

use fidryn_core::{
    AdmissibleCompletions, CaseRecord, CoreModule, Instant, ModelBoundary, Outcome, QueryName,
    TraceId, Value, canonical_json,
};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

/// Canonical outcome envelope (`fidryn.outcome/v0.1`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeDocument {
    pub schema: String,
    pub module: String,
    pub source_snapshot: String,
    pub query: String,
    pub as_of: AsOf,
    pub model_boundary: ModelBoundary,
    pub outcome: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsOf {
    pub valid_time: String,
    pub record_time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceNode {
    pub id: String,
    pub kind: TraceKind,
    pub detail: String,
    /// Incoming DAG edges. Empty for roots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parents: Vec<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TraceKind {
    SourceText,
    StrictRuleApplication,
    DefeasibleRuleApplication,
    EvidenceSubmission,
    ClosureRecord,
    Presumption,
    Determination,
    DiscretionaryDecision,
    InterpretationSelection,
    ConflictDoctrine,
    Assumption,
    ConstitutiveEffect,
    SolverLemma,
}

/// Build the schema-matching outcome envelope.
///
/// `sourceSnapshot` is the module snapshot identity, not the case's module
/// name. `modelBoundary.outsideScope` is the stable unique union of module
/// and case exclusions. Admissible completions come from the case when
/// nonempty; otherwise the module default is empty.
pub fn outcome_document(
    module: &CoreModule,
    query: &QueryName,
    valid: Instant,
    known: Instant,
    case: &CaseRecord,
    outcome: &Outcome<Value>,
) -> OutcomeDocument {
    OutcomeDocument {
        schema: "fidryn.outcome/v0.1".into(),
        module: format!("{}@{}", module.name, module.version),
        source_snapshot: module.snapshot.to_string(),
        query: query.as_str().to_owned(),
        as_of: AsOf {
            valid_time: valid.to_rfc3339(),
            record_time: known.to_rfc3339(),
        },
        model_boundary: merged_model_boundary(module, case),
        outcome: envelope_outcome(outcome),
    }
}

pub fn render_outcome(
    module: &CoreModule,
    query: &QueryName,
    valid: Instant,
    known: Instant,
    case: &CaseRecord,
    outcome: &Outcome<Value>,
) -> String {
    canonical_json(&outcome_document(
        module, query, valid, known, case, outcome,
    ))
    .expect("canonical json")
}

/// Proof-relevant DAG for `explain`.
///
/// `nodes` is always present. It is empty only when no persisted run
/// artifact was loaded: the CLI hashed a `TraceId` from the argument
/// and did not read a `TRACE_ID.json` file. This crate does not invent
/// a constant-hash eval chain in that case.
///
/// When an outcome document is wrapped, `provenance_root` is the source
/// snapshot (or the outcome `trace` if no snapshot is present).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceDocument {
    pub trace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_root: Option<String>,
    /// Empty only when no persisted DAG was loaded for this id.
    #[serde(default)]
    pub nodes: Vec<TraceNode>,
}

/// Render a trace DAG as `text`, `json`, or `dot`.
///
/// When no artifact exists, `nodes` is an empty array. That emptiness
/// is documented here and is not an omitted field.
pub fn explain(trace: TraceId, format: &str) -> String {
    format_trace(
        &TraceDocument {
            trace: trace.to_string(),
            provenance_root: None,
            nodes: Vec::new(),
        },
        format,
    )
}

/// Render a JSON trace artifact (a [`TraceDocument`] or an outcome document).
pub fn explain_value(value: &serde_json::Value, format: &str) -> String {
    format_trace(&trace_document_from_value(value), format)
}

/// Wrap an evaluator outcome as a small DAG.
///
/// The evaluator does not yet pass per-step nodes. The wrap uses the
/// outcome's [`TraceId`] (hex) and an optional provenance root (module
/// snapshot identity). It does not emit a shared constant hash.
pub fn wrap_outcome(outcome: &Outcome<Value>, provenance_root: Option<&str>) -> TraceDocument {
    wrap_outcome_json(&envelope_outcome(outcome), provenance_root)
}

fn merged_model_boundary(module: &CoreModule, case: &CaseRecord) -> ModelBoundary {
    ModelBoundary {
        outside_scope: union_unique(&module.outside_scope, &case.outside_scope),
        admissible_completions: if completions_are_empty(&case.admissible_completions) {
            AdmissibleCompletions::default()
        } else {
            case.admissible_completions.clone()
        },
    }
}

fn union_unique(module_scope: &[String], case_scope: &[String]) -> Vec<String> {
    let mut seen = IndexSet::new();
    for item in module_scope.iter().chain(case_scope) {
        seen.insert(item.clone());
    }
    seen.into_iter().collect()
}

fn completions_are_empty(completions: &AdmissibleCompletions) -> bool {
    completions.interpretations.is_empty()
        && completions.evidence.is_empty()
        && completions.choices.is_empty()
}

fn envelope_outcome(outcome: &Outcome<Value>) -> serde_json::Value {
    let mut value = serde_json::to_value(outcome).expect("outcome serializes");
    let Some(obj) = value.as_object_mut() else {
        return value;
    };
    rename_field(obj, "convergence_certificate", "convergenceCertificate");
    rename_field(obj, "ignored_open_issues", "ignoredOpenIssues");
    obj.insert(
        "trace".into(),
        serde_json::Value::String(outcome.trace().to_string()),
    );
    if let Some(cert) = obj.get("convergenceCertificate").cloned()
        && let Some(hex) = json_id(Some(&cert))
    {
        obj.insert(
            "convergenceCertificate".into(),
            serde_json::Value::String(hex),
        );
    }
    value
}

fn rename_field(obj: &mut serde_json::Map<String, serde_json::Value>, from: &str, to: &str) {
    if let Some(value) = obj.remove(from) {
        obj.insert(to.to_owned(), value);
    }
}

fn trace_document_from_value(value: &serde_json::Value) -> TraceDocument {
    if is_trace_artifact(value)
        && let Ok(doc) = serde_json::from_value::<TraceDocument>(value.clone())
    {
        return doc;
    }
    if let Some(nodes_value) = value.get("nodes") {
        let trace = json_id(value.get("trace")).unwrap_or_else(|| "unknown".into());
        let nodes = serde_json::from_value(nodes_value.clone()).unwrap_or_default();
        return TraceDocument {
            trace,
            provenance_root: json_id(value.get("provenanceRoot")),
            nodes,
        };
    }
    if value.get("outcome").is_some() || value.get("kind").is_some() {
        let snapshot = json_id(value.get("sourceSnapshot"));
        let outcome = value.get("outcome").unwrap_or(value);
        return wrap_outcome_json(outcome, snapshot.as_deref());
    }
    TraceDocument {
        trace: json_id(value.get("trace")).unwrap_or_else(|| "unknown".into()),
        provenance_root: None,
        nodes: Vec::new(),
    }
}

fn is_trace_artifact(value: &serde_json::Value) -> bool {
    value.get("trace").is_some() && value.get("nodes").is_some()
}

fn wrap_outcome_json(outcome: &serde_json::Value, provenance_root: Option<&str>) -> TraceDocument {
    let trace = json_id(outcome.get("trace")).unwrap_or_else(|| "unknown".into());
    let root = provenance_root
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| trace.clone());
    let mut nodes = Vec::new();
    let mut determination_parents = Vec::new();
    if root != trace {
        nodes.push(TraceNode {
            id: root.clone(),
            kind: TraceKind::SourceText,
            detail: "source snapshot".into(),
            parents: Vec::new(),
        });
        determination_parents.push(root.clone());
    }
    nodes.push(TraceNode {
        id: trace.clone(),
        kind: TraceKind::Determination,
        detail: canonical_json(outcome).unwrap_or_default(),
        parents: determination_parents,
    });
    TraceDocument {
        trace,
        provenance_root: Some(root),
        nodes,
    }
}

fn json_id(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
        serde_json::Value::Array(items) if items.len() == 16 => {
            let mut bytes = [0u8; 16];
            for (i, item) in items.iter().enumerate() {
                let n = item.as_u64()?;
                bytes[i] = u8::try_from(n).ok()?;
            }
            Some(TraceId::from_bytes(bytes).to_string())
        }
        _ => None,
    }
}

fn format_trace(doc: &TraceDocument, format: &str) -> String {
    match format {
        "json" => canonical_json(doc).expect("canonical json"),
        "dot" => format_dot(doc),
        _ => format_text(doc),
    }
}

fn format_text(doc: &TraceDocument) -> String {
    if doc.nodes.is_empty() {
        // Empty nodes: no persisted DAG for this hashed id.
        return format!("trace {}", doc.trace);
    }
    let mut out = format!("trace {}\n", doc.trace);
    if let Some(root) = &doc.provenance_root {
        out.push_str(&format!("provenance_root {root}\n"));
    }
    for node in &doc.nodes {
        if node.parents.is_empty() {
            out.push_str(&format!("{} {:?}: {}\n", node.id, node.kind, node.detail));
        } else {
            out.push_str(&format!(
                "{} {:?}: {} parents={}\n",
                node.id,
                node.kind,
                node.detail,
                node.parents.join(",")
            ));
        }
    }
    out
}

fn format_dot(doc: &TraceDocument) -> String {
    let graph = sanitize_dot_id(&doc.trace);
    if doc.nodes.is_empty() {
        return format!("digraph {graph} {{ }}");
    }
    let mut out = format!("digraph {graph} {{\n");
    for node in &doc.nodes {
        let label = format!("{:?}: {}", node.kind, node.detail)
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let id = sanitize_dot_id(&node.id);
        out.push_str(&format!("  {id} [label=\"{label}\"];\n"));
    }
    for node in &doc.nodes {
        let id = sanitize_dot_id(&node.id);
        for parent in &node.parents {
            let parent_id = sanitize_dot_id(parent);
            out.push_str(&format!("  {parent_id} -> {id};\n"));
        }
    }
    out.push('}');
    out
}

fn sanitize_dot_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "trace".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::TraceId;
    use std::collections::BTreeSet;

    fn sample_module() -> CoreModule {
        CoreModule {
            id: fidryn_core::ModuleId::of(b"m"),
            name: "Examples.T".into(),
            version: "0.1.0".into(),
            snapshot: fidryn_core::SourceSnapshotId::of(b"s"),
            manifest: fidryn_core::SourceManifestId::of(b"m"),
            jurisdiction: fidryn_core::JurisdictionId::of(b"j"),
            outside_scope: vec!["tax".into(), "creditor_priority".into()],
            declarations: vec![],
            nominations: vec![],
            queries: vec![],
            verifications: vec![],
            assertions: vec![],
        }
    }

    fn sample_outcome() -> Outcome<Value> {
        Outcome::Determinate {
            value: Value::Entity("Bryan".into()),
            trace: TraceId::of(b"T10"),
            convergence_certificate: None,
            ignored_open_issues: BTreeSet::new(),
        }
    }

    fn render_sample(module: &CoreModule, case: &CaseRecord) -> String {
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        render_outcome(
            module,
            &QueryName::from("acting_trustee"),
            t,
            t,
            case,
            &sample_outcome(),
        )
    }

    #[test]
    fn outcome_json_is_canonical() {
        let module = sample_module();
        let case = CaseRecord::default();
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let out = sample_outcome();
        let a = render_outcome(
            &module,
            &QueryName::from("acting_trustee"),
            t,
            t,
            &case,
            &out,
        );
        let b = render_outcome(
            &module,
            &QueryName::from("acting_trustee"),
            t,
            t,
            &case,
            &out,
        );
        assert_eq!(a, b);
        assert_eq!(a.as_bytes(), b.as_bytes());
        assert!(a.starts_with('{'));
        assert!(a.contains("modelBoundary"));
        assert!(!a.contains('\n'));
        assert!(!a.contains('\t'));
        assert!(!a.contains(": "));
        assert!(!a.contains(", "));
        assert!(!a.contains(' '));
        assert!(
            a.contains(&format!("\"trace\":\"{}\"", out.trace().hex())),
            "{a}"
        );
        assert!(!a.contains("\"trace\":["));
        assert!(a.contains("ignoredOpenIssues"));
    }

    #[test]
    fn source_snapshot_uses_module_snapshot_identity() {
        let module = sample_module();
        let mut case = CaseRecord::default();
        case.module = Some("Examples.T".into());
        let json = render_sample(&module, &case);
        let doc: OutcomeDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.source_snapshot, module.snapshot.hex());
        assert_eq!(doc.source_snapshot, module.snapshot.to_string());
        assert_ne!(doc.source_snapshot, module.name);
        assert_ne!(doc.source_snapshot, case.module.clone().unwrap());
        assert_eq!(doc.module, "Examples.T@0.1.0");
    }

    #[test]
    fn module_outside_scope_appears_when_case_leaves_it_empty() {
        let module = sample_module();
        let empty = CaseRecord::default();
        assert!(empty.outside_scope.is_empty());
        let empty_json = render_sample(&module, &empty);
        assert!(empty_json.contains("modelBoundary"));
        assert!(empty_json.contains(r#""outsideScope":["tax","creditor_priority"]"#));
        let empty_doc: OutcomeDocument = serde_json::from_str(&empty_json).unwrap();
        assert_eq!(
            empty_doc.model_boundary.outside_scope,
            vec!["tax".to_string(), "creditor_priority".to_string()]
        );

        let mut case = CaseRecord::default();
        case.outside_scope = vec!["tax".into()];
        let json = render_sample(&module, &case);
        assert!(
            json.contains("creditor_priority"),
            "module exclusions must survive case listing a subset: {json}"
        );
        let doc: OutcomeDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(
            doc.model_boundary.outside_scope,
            vec!["tax".to_string(), "creditor_priority".to_string()]
        );
    }

    #[test]
    fn case_admissible_completions_win_when_nonempty() {
        let module = sample_module();
        let mut case = CaseRecord::default();
        case.admissible_completions.interpretations.insert(
            "SuccessorEligibility".into(),
            vec!["I1".into(), "I2".into()],
        );
        let json = render_sample(&module, &case);
        assert!(json.contains("SuccessorEligibility"));
        assert!(json.contains("admissibleCompletions"));
        let doc: OutcomeDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(
            doc.model_boundary
                .admissible_completions
                .interpretations
                .get("SuccessorEligibility")
                .map(Vec::as_slice),
            Some(["I1".to_string(), "I2".to_string()].as_slice())
        );
    }

    #[test]
    fn explain_json_includes_nodes_array() {
        let t = TraceId::of(b"abc");
        let json = explain(t, "json");
        assert!(json.contains("\"nodes\":[]"), "{json}");
        assert!(json.contains(&t.to_string()), "{json}");
        assert!(!json.contains(' '));
        let doc: TraceDocument = serde_json::from_str(&json).unwrap();
        assert!(
            doc.nodes.is_empty(),
            "empty nodes means no persisted DAG was loaded"
        );
        assert!(doc.provenance_root.is_none());
        assert_eq!(explain(t, "dot"), format!("digraph {} {{ }}", t.hex()));
        assert_eq!(explain(t, "text"), format!("trace {}", t.hex()));
    }

    #[test]
    fn explain_value_uses_persisted_nodes() {
        let value = serde_json::json!({
            "trace": "aa",
            "nodes": [{
                "id": "n0",
                "kind": "SourceText",
                "detail": "Instrument.clause 4.4"
            }]
        });
        let json = explain_value(&value, "json");
        assert!(json.contains("\"nodes\":["));
        assert!(json.contains("SourceText"));
        let text = explain_value(&value, "text");
        assert!(text.contains("n0"));
        assert!(text.contains("Instrument.clause 4.4"));
        let dot = explain_value(&value, "dot");
        assert!(dot.contains("digraph aa"));
        assert!(dot.contains("SourceText"));
    }

    #[test]
    fn wrapping_outcome_includes_provenance_root() {
        let module = sample_module();
        let case = CaseRecord::default();
        let json = render_sample(&module, &case);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        let explained = explain_value(&value, "json");
        let doc: TraceDocument = serde_json::from_str(&explained).unwrap();
        assert_eq!(
            doc.provenance_root.as_deref(),
            Some(module.snapshot.hex().as_str())
        );
        assert_eq!(doc.trace, TraceId::of(b"T10").hex());
        assert!(
            doc.nodes
                .iter()
                .any(|n| n.id == module.snapshot.hex() && matches!(n.kind, TraceKind::SourceText))
        );
        let determination = doc
            .nodes
            .iter()
            .find(|n| matches!(n.kind, TraceKind::Determination))
            .expect("determination node");
        assert_eq!(determination.id, TraceId::of(b"T10").hex());
        assert_eq!(determination.parents, vec![module.snapshot.hex()]);
        let dot = explain_value(&value, "dot");
        assert!(
            dot.contains(&format!(
                "{} -> {}",
                sanitize_dot_id(&module.snapshot.hex()),
                sanitize_dot_id(&TraceId::of(b"T10").hex())
            )),
            "{dot}"
        );
    }

    #[test]
    fn wrap_outcome_uses_trace_hex_not_constant_hash() {
        let outcome = sample_outcome();
        let doc = wrap_outcome(&outcome, Some("snap"));
        assert_eq!(doc.trace, TraceId::of(b"T10").hex());
        assert_eq!(doc.provenance_root.as_deref(), Some("snap"));
        assert_ne!(doc.trace, TraceId::of(b"eval").hex());
        assert_eq!(doc.nodes.len(), 2);
        assert_eq!(doc.nodes[1].parents, vec!["snap".to_string()]);
    }

    #[test]
    fn parent_edges_render_in_dot() {
        let value = serde_json::json!({
            "trace": "aa",
            "nodes": [
                {"id": "n0", "kind": "SourceText", "detail": "src"},
                {"id": "n1", "kind": "Determination", "detail": "out", "parents": ["n0"]}
            ]
        });
        let dot = explain_value(&value, "dot");
        assert!(dot.contains("n0 -> n1"), "{dot}");
        let text = explain_value(&value, "text");
        assert!(text.contains("parents=n0"), "{text}");
    }
}
