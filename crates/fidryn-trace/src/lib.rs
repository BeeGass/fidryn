//! Proof-relevant traces and canonical outcome JSON.

use fidryn_core::{
    CaseRecord, CoreModule, Instant, ModelBoundary, Outcome, QueryName, TraceId, Value,
    canonical_json,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutcomeDocument {
    pub schema: String,
    pub module: String,
    #[serde(rename = "sourceSnapshot")]
    pub source_snapshot: String,
    pub query: String,
    #[serde(rename = "asOf")]
    pub as_of: AsOf,
    #[serde(rename = "modelBoundary")]
    pub model_boundary: ModelBoundary,
    pub outcome: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AsOf {
    #[serde(rename = "validTime")]
    pub valid_time: String,
    #[serde(rename = "recordTime")]
    pub record_time: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceNode {
    pub id: String,
    pub kind: TraceKind,
    pub detail: String,
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

pub fn render_outcome(
    module: &CoreModule,
    query: &QueryName,
    valid: Instant,
    known: Instant,
    case: &CaseRecord,
    outcome: &Outcome<Value>,
) -> String {
    let mut boundary = case.model_boundary();
    if boundary.outside_scope.is_empty() {
        boundary.outside_scope = module.outside_scope.clone();
    }
    let doc = OutcomeDocument {
        schema: "fidryn.outcome/v0.1".into(),
        module: format!("{}@{}", module.name, module.version),
        source_snapshot: case.module.clone().unwrap_or_else(|| module.name.clone()),
        query: query.as_str().to_owned(),
        as_of: AsOf {
            valid_time: valid.to_rfc3339(),
            record_time: known.to_rfc3339(),
        },
        model_boundary: boundary,
        outcome: serde_json::to_value(outcome).expect("outcome serializes"),
    };
    canonical_json(&doc).expect("canonical json")
}

/// Proof-relevant DAG for `explain`.
///
/// `nodes` is always present. It is empty only when no persisted run
/// artifact was loaded: the CLI hashed a `TraceId` from the argument
/// and did not read a `TRACE_ID.json` file.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TraceDocument {
    pub trace: String,
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
            nodes: Vec::new(),
        },
        format,
    )
}

/// Render a JSON trace artifact (a [`TraceDocument`] or an outcome document).
pub fn explain_value(value: &serde_json::Value, format: &str) -> String {
    format_trace(&trace_document_from_value(value), format)
}

fn trace_document_from_value(value: &serde_json::Value) -> TraceDocument {
    if let Ok(doc) = serde_json::from_value::<TraceDocument>(value.clone()) {
        return doc;
    }
    let trace = value
        .get("trace")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown")
        .to_owned();
    if let Some(nodes_value) = value.get("nodes") {
        let nodes = serde_json::from_value(nodes_value.clone()).unwrap_or_default();
        return TraceDocument { trace, nodes };
    }
    let mut nodes = Vec::new();
    if let Some(outcome) = value.get("outcome") {
        nodes.push(TraceNode {
            id: trace.clone(),
            kind: TraceKind::Determination,
            detail: canonical_json(outcome).unwrap_or_default(),
        });
    }
    TraceDocument { trace, nodes }
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
    for node in &doc.nodes {
        out.push_str(&format!("{} {:?}: {}\n", node.id, node.kind, node.detail));
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
            outside_scope: vec!["tax".into()],
            declarations: vec![],
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
    }

    #[test]
    fn module_outside_scope_appears_when_case_leaves_it_empty() {
        let module = sample_module();
        let case = CaseRecord::default();
        assert!(case.outside_scope.is_empty());
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let json = render_outcome(
            &module,
            &QueryName::from("acting_trustee"),
            t,
            t,
            &case,
            &sample_outcome(),
        );
        assert!(json.contains("modelBoundary"));
        assert!(json.contains(r#""outsideScope":["tax"]"#));
        let doc: OutcomeDocument = serde_json::from_str(&json).unwrap();
        assert_eq!(doc.model_boundary.outside_scope, vec!["tax".to_string()]);
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
}
