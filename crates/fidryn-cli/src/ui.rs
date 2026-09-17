//! Local mill: localhost-only web UI. Never live-files.

use crate::{
    EngineFailure, compile_source, explore_report, merge_bounds_json, parse_instant, render_report,
};
use axum::Router;
use axum::extract::Json;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use fidryn_core::{
    CaseRecord, CoreModule, Diagnostic, EvaluationReport, Instant, QueryName, RunContext,
    SourceManifest, TrustProfile,
};
use fidryn_driver::Driver;
use fidryn_render::{module_vars, render};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr};

const INDEX: &str = include_str!("../../../web/index.html");
const DEFAULT_PORT: u16 = 8751;

/// Axum router used by `fidryn ui` and the HTTP tests.
pub fn router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/check", post(check))
        .route("/api/run", post(run))
        .route("/api/explore", post(explore))
        .route("/api/render", post(render_api))
}

/// Bind `127.0.0.1` and serve the mill. Never listens on other interfaces.
pub async fn serve(preferred: u16, no_open: bool) -> anyhow::Result<()> {
    let listener = bind_loopback(preferred).await?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{port}");
    eprintln!("fidryn mill on {url}");
    eprintln!("127.0.0.1 only. live filing is disabled.");
    if !no_open {
        eprintln!("browser auto-open is not linked; open the URL locally");
    }
    axum::serve(listener, router())
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}

/// Bind a TCP listener on loopback only.
pub async fn bind_loopback(port: u16) -> std::io::Result<tokio::net::TcpListener> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => Ok(listener),
        Err(err) if err.kind() == ErrorKind::AddrInUse => Err(std::io::Error::new(
            ErrorKind::AddrInUse,
            format!("127.0.0.1:{port} is already in use. Stop that process or pass --port."),
        )),
        Err(err) => Err(err),
    }
}

pub fn default_port() -> u16 {
    DEFAULT_PORT
}

async fn index() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        Html(INDEX),
    )
}

async fn health() -> &'static str {
    "ok"
}

#[derive(Debug, Deserialize)]
struct CheckRequest {
    source: String,
}

#[derive(Debug, Serialize)]
struct CheckResponse {
    ok: bool,
    diagnostics: Vec<Diagnostic>,
}

/// Compile pasted mill source in memory only.
///
/// The paste is a standalone in-memory program: no external artifacts are
/// claimed as verified. `source_root` is not set. Missing hex artifacts are
/// E200 only on path compile (`Driver::check_path`), not by reading server
/// paths from pasted source. Never call `check_path` or `fs::read` of
/// artifact paths from the paste.
fn mill_compile(source: &str) -> Result<CoreModule, Vec<Diagnostic>> {
    compile_source(source, &SourceManifest::default())
}

async fn check(Json(req): Json<CheckRequest>) -> (StatusCode, Json<CheckResponse>) {
    match mill_compile(&req.source) {
        Ok(_) => (
            StatusCode::OK,
            Json(CheckResponse {
                ok: true,
                diagnostics: Vec::new(),
            }),
        ),
        Err(diagnostics) => (
            StatusCode::OK,
            Json(CheckResponse {
                ok: false,
                diagnostics,
            }),
        ),
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EvalRequest {
    source: String,
    query: String,
    #[serde(default)]
    case: serde_json::Value,
    #[serde(default)]
    bounds: Option<serde_json::Value>,
    valid_at: String,
    known_at: String,
}

type JsonResponse = (StatusCode, Json<serde_json::Value>);

#[derive(Debug, Deserialize)]
struct RenderRequest {
    source: String,
    template: String,
}

#[derive(Debug, Serialize)]
struct RenderResponse {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn parse_case(value: serde_json::Value) -> Result<CaseRecord, String> {
    match value {
        serde_json::Value::Null => Ok(CaseRecord::default()),
        serde_json::Value::Object(map) if map.is_empty() => Ok(CaseRecord::default()),
        serde_json::Value::Object(map) => {
            let mut base = serde_json::to_value(CaseRecord::default())
                .map_err(|e| format!("invalid case: {e}"))?;
            let Some(obj) = base.as_object_mut() else {
                return Err("invalid case: default record is not an object".into());
            };
            for (k, v) in map {
                obj.insert(k, v);
            }
            serde_json::from_value(base).map_err(|e| format!("invalid case: {e}"))
        }
        other => serde_json::from_value(other).map_err(|e| format!("invalid case: {e}")),
    }
}

fn mill_err(
    status: StatusCode,
    error: impl Into<String>,
    diagnostics: Vec<Diagnostic>,
) -> JsonResponse {
    (
        status,
        Json(serde_json::json!({
            "ok": false,
            "error": error.into(),
            "diagnostics": diagnostics,
        })),
    )
}

fn mill_engine_err(err: &EngineFailure) -> JsonResponse {
    let status = if err.is_internal() {
        StatusCode::INTERNAL_SERVER_ERROR
    } else {
        StatusCode::BAD_REQUEST
    };
    (
        status,
        Json(serde_json::json!({
            "kind": "engineError",
            "error": err.kind,
            "message": err.message,
            "ok": false,
        })),
    )
}

/// Mill success transport: `{ "ok": true, "report": <evaluation-report> }`.
///
/// `ok` is not a field of `fidryn.evaluation-report/v0.1`. Pasted compile
/// is `sourceTrust: unauthenticated` and is never `byteVerified`.
fn mill_report_doc(
    module: &CoreModule,
    query: &QueryName,
    valid: Instant,
    known: Instant,
    case: &CaseRecord,
    report: &EvaluationReport,
) -> JsonResponse {
    let text = render_report(module, query, valid, known, case, report);
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(report_json) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "ok": true,
                "report": report_json,
            })),
        ),
        Err(err) => mill_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            err.to_string(),
            Vec::new(),
        ),
    }
}

fn eval_request(req: EvalRequest, explore_mode: bool) -> JsonResponse {
    let module = match mill_compile(&req.source) {
        Ok(module) => module,
        Err(diagnostics) => {
            return mill_err(StatusCode::BAD_REQUEST, "check failed", diagnostics);
        }
    };
    let mut case = match parse_case(req.case) {
        Ok(case) => case,
        Err(err) => return mill_err(StatusCode::BAD_REQUEST, err, Vec::new()),
    };
    if explore_mode
        && let Some(bounds) = req.bounds
        && !bounds.is_null()
        && let Err(err) = merge_bounds_json(&mut case, &bounds)
    {
        return mill_err(StatusCode::BAD_REQUEST, err, Vec::new());
    }
    let valid = match parse_instant(&req.valid_at) {
        Ok(instant) => instant,
        Err(err) => {
            return mill_err(
                StatusCode::BAD_REQUEST,
                format!("validAt: {err}"),
                Vec::new(),
            );
        }
    };
    let known = match parse_instant(&req.known_at) {
        Ok(instant) => instant,
        Err(err) => {
            return mill_err(
                StatusCode::BAD_REQUEST,
                format!("knownAt: {err}"),
                Vec::new(),
            );
        }
    };
    let ctx = RunContext::new(valid, known);
    let query = QueryName::from(req.query.as_str());
    let mut report = if explore_mode {
        match explore_report(&module, &query, &case, &ctx) {
            Ok(report) => report,
            Err(err) => return mill_engine_err(&err),
        }
    } else {
        match Driver::new().run_report(&module, query.as_str(), &case, &ctx) {
            Ok(report) => report,
            Err(err) => return mill_engine_err(&EngineFailure::from_err(err)),
        }
    };
    report.trust = TrustProfile::Unauthenticated;
    mill_report_doc(&module, &query, valid, known, &case, &report)
}

async fn run(Json(req): Json<EvalRequest>) -> JsonResponse {
    eval_request(req, false)
}

async fn explore(Json(req): Json<EvalRequest>) -> JsonResponse {
    eval_request(req, true)
}

async fn render_api(Json(req): Json<RenderRequest>) -> (StatusCode, Json<RenderResponse>) {
    let module = match mill_compile(&req.source) {
        Ok(module) => module,
        Err(diagnostics) => {
            let error = diagnostics
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            return (
                StatusCode::OK,
                Json(RenderResponse {
                    ok: false,
                    text: None,
                    error: Some(error),
                }),
            );
        }
    };
    match render(&req.template, &module_vars(&module)) {
        Ok(text) => (
            StatusCode::OK,
            Json(RenderResponse {
                ok: true,
                text: Some(text),
                error: None,
            }),
        ),
        Err(err) => (
            StatusCode::OK,
            Json(RenderResponse {
                ok: false,
                text: None,
                error: Some(err.to_string()),
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use std::io::{Read, Write};
    use tower::ServiceExt;

    async fn body_text(response: axum::response::Response) -> String {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    async fn post_json(uri: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
        let response = router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(uri)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        (status, json)
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let response = router()
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(body_text(response).await, "ok");
    }

    #[tokio::test]
    async fn index_returns_html_mill() {
        let response = router()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let html = body_text(response).await;
        assert!(html.contains("fidryn mill"), "{html}");
        assert!(html.contains("localhost"));
        assert!(html.contains("127.0.0.1"));
        assert!(html.contains("Live filing is not available"));
        assert!(html.contains(">Run<"), "{html}");
        assert!(html.contains(">Explore<"), "{html}");
        assert!(html.contains(">Render<"), "{html}");
    }

    #[tokio::test]
    async fn check_accepts_a_valid_module() {
        let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let (status, json) = post_json("/api/check", serde_json::json!({ "source": src })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["ok"], true);
        assert_eq!(json["diagnostics"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn check_returns_diagnostics_for_bad_source() {
        let (status, json) = post_json(
            "/api/check",
            serde_json::json!({ "source": "this is not fidryn" }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["ok"], false);
        assert!(
            json["diagnostics"]
                .as_array()
                .is_some_and(|d| !d.is_empty()),
            "{json}"
        );
    }

    #[tokio::test]
    async fn mill_has_no_live_filing_route() {
        for uri in ["/api/file", "/api/filing", "/api/submit", "/api/live"] {
            let response = router()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(uri)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{uri}");
        }
    }

    const SAMPLE: &str = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;

    fn eval_body() -> serde_json::Value {
        serde_json::json!({
            "source": SAMPLE,
            "query": "q",
            "case": {},
            "validAt": "2033-01-01T00:00:00Z",
            "knownAt": "2033-01-01T00:00:00Z"
        })
    }

    fn mill_report(json: &serde_json::Value) -> &serde_json::Value {
        json.get("report")
            .unwrap_or_else(|| panic!("mill transport must wrap report: {json}"))
    }

    fn assert_report_envelope(json: &serde_json::Value, query: &str, mode: &str) {
        assert_eq!(json["ok"], true, "{json}");
        assert!(
            json.get("schema").is_none(),
            "ok is transport, not an evaluation-report field: {json}"
        );
        let report = mill_report(json);
        assert!(
            report.get("ok").is_none(),
            "report schema forbids mill ok: {report}"
        );
        assert_eq!(report["schema"], "fidryn.evaluation-report/v0.1", "{json}");
        assert_eq!(report["executionMode"], mode, "{json}");
        assert_eq!(report["sourceTrust"], "unauthenticated", "{json}");
        assert_ne!(report["sourceTrust"], "byteVerified", "{json}");
        assert!(report["assumptions"].is_array(), "{json}");
        assert!(report["verificationMethod"].is_string(), "{json}");
        let doc = &report["outcomeDocument"];
        assert_eq!(doc["schema"], "fidryn.outcome/v0.1", "{json}");
        assert!(doc["module"].is_string(), "{json}");
        assert!(doc["sourceSnapshot"].is_string(), "{json}");
        assert_eq!(doc["query"], query, "{json}");
        assert!(doc["asOf"]["validTime"].is_string(), "{json}");
        assert!(doc["asOf"]["recordTime"].is_string(), "{json}");
        assert!(doc["modelBoundary"].is_object(), "{json}");
        assert!(doc["modelBoundary"]["outsideScope"].is_array(), "{json}");
        assert!(
            doc["modelBoundary"]["admissibleCompletions"].is_object(),
            "{json}"
        );
        assert!(doc["outcome"].is_object(), "{json}");
        assert!(doc["outcome"]["kind"].is_string(), "{json}");
        assert!(doc["outcome"]["trace"].is_string(), "{json}");
    }

    fn assert_outcome_document(json: &serde_json::Value, query: &str) {
        assert_report_envelope(json, query, "operative");
    }

    #[tokio::test]
    async fn run_evaluates_a_query() {
        let (status, json) = post_json("/api/run", eval_body()).await;
        assert_eq!(status, StatusCode::OK);
        assert_outcome_document(&json, "q");
    }

    #[tokio::test]
    async fn pasted_run_with_assumptions_is_scenario() {
        let mut body = eval_body();
        body["case"] = serde_json::json!({
            "assumptions": [{"id": "hyp-1", "payload": true}]
        });
        let (status, json) = post_json("/api/run", body).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_report_envelope(&json, "q", "scenario");
        let report = mill_report(&json);
        assert_eq!(report["assumptions"][0]["id"], "hyp-1", "{json}");
        assert_eq!(report["assumptions"][0]["payload"], true, "{json}");
        assert_eq!(report["sourceTrust"], "unauthenticated", "{json}");
        assert_ne!(report["sourceTrust"], "byteVerified", "{json}");
        assert_eq!(report["schema"], "fidryn.evaluation-report/v0.1", "{json}");
        assert_eq!(
            report["outcomeDocument"]["schema"], "fidryn.outcome/v0.1",
            "{json}"
        );
    }

    #[tokio::test]
    async fn pasted_explore_with_assumptions_is_scenario() {
        let mut body = eval_body();
        body["case"] = serde_json::json!({
            "assumptions": [{"id": "hyp-explore", "payload": true}]
        });
        let (status, json) = post_json("/api/explore", body).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_report_envelope(&json, "q", "scenario");
        let report = mill_report(&json);
        assert_eq!(report["assumptions"][0]["id"], "hyp-explore", "{json}");
        assert_eq!(report["sourceTrust"], "unauthenticated", "{json}");
    }

    fn flag_src() -> &'static str {
        r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { flag }
    }
}
"#
    }

    fn duty_src() -> &'static str {
        r#"
module Examples.T version "0.1.0" {
    entity Payer : NaturalPerson
    entity Payee : NaturalPerson
    proposition InvoiceIssued(person: NaturalPerson)
    duty PayInvoice {
        bearer Payer
        claimant Payee
        attaches when operative InvoiceIssued(Payer)
        content USD(100.00)
        due 30 counted_days after invoice_date
    }
    query q() -> String {
        goal Evaluate { duty_status(PayInvoice) }
    }
}
"#
    }

    fn attached_duty_case(assumptions: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "facts": {
                "invoice_date": {"kind": "instant", "data": "2033-01-01T00:00:00Z"}
            },
            "determinations": [{
                "issue": "InvoiceIssued(Payer)",
                "protocol": "InvoiceIssued",
                "established": true,
                "decider": "test",
                "recorded_at": "2033-01-01T00:00:00Z"
            }],
            "assumptions": assumptions
        })
    }

    fn outcome_bool(json: &serde_json::Value) -> Option<bool> {
        mill_report(json)["outcomeDocument"]["outcome"]["value"]["data"].as_bool()
    }

    fn outcome_duty_status(json: &serde_json::Value) -> String {
        mill_report(json)["outcomeDocument"]["outcome"]["value"]["data"]["status"]["data"]
            .as_str()
            .unwrap_or("")
            .to_owned()
    }

    #[tokio::test]
    async fn pasted_run_assumption_overlay_changes_boolean_fact() {
        let mut operative_body = eval_body();
        operative_body["source"] = serde_json::json!(flag_src());
        let (op_status, op_json) = post_json("/api/run", operative_body).await;
        assert_eq!(op_status, StatusCode::BAD_REQUEST, "{op_json}");
        assert_eq!(op_json["ok"], false, "{op_json}");

        let mut scenario_body = eval_body();
        scenario_body["source"] = serde_json::json!(flag_src());
        scenario_body["case"] = serde_json::json!({
            "assumptions": [{"id": "hyp-flag", "payload": {"flag": true}}]
        });
        let (status, json) = post_json("/api/run", scenario_body).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_report_envelope(&json, "q", "scenario");
        assert_eq!(outcome_bool(&json), Some(true), "{json}");
    }

    #[tokio::test]
    async fn pasted_explore_assumption_overlay_changes_boolean_fact() {
        let mut operative_body = eval_body();
        operative_body["source"] = serde_json::json!(flag_src());
        let (op_status, op_json) = post_json("/api/explore", operative_body).await;
        assert_eq!(op_status, StatusCode::OK, "{op_json}");
        assert_outcome_document(&op_json, "q");
        assert_ne!(outcome_bool(&op_json), Some(true), "{op_json}");

        let mut scenario_body = eval_body();
        scenario_body["source"] = serde_json::json!(flag_src());
        scenario_body["case"] = serde_json::json!({
            "assumptions": [{"id": "hyp-flag", "payload": {"flag": true}}]
        });
        let (status, json) = post_json("/api/explore", scenario_body).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_report_envelope(&json, "q", "scenario");
        assert_eq!(outcome_bool(&json), Some(true), "{json}");
    }

    #[tokio::test]
    async fn pasted_run_assumption_overlay_changes_duty_status() {
        let mut operative_body = eval_body();
        operative_body["source"] = serde_json::json!(duty_src());
        operative_body["case"] = attached_duty_case(serde_json::json!([]));
        let (op_status, op_json) = post_json("/api/run", operative_body).await;
        assert_eq!(op_status, StatusCode::OK, "{op_json}");
        assert_outcome_document(&op_json, "q");
        let operative_status = outcome_duty_status(&op_json);
        assert!(
            !operative_status.eq_ignore_ascii_case("Performed"),
            "{op_json}"
        );

        let mut scenario_body = eval_body();
        scenario_body["source"] = serde_json::json!(duty_src());
        scenario_body["case"] = attached_duty_case(serde_json::json!([{
            "id": "hyp-performed",
            "payload": {"kind": "ctor", "data": {"name": "Performed", "fields": {}}}
        }]));
        let (status, json) = post_json("/api/run", scenario_body).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        assert_report_envelope(&json, "q", "scenario");
        let scenario_status = outcome_duty_status(&json);
        assert_ne!(
            operative_status, scenario_status,
            "assumption overlay must change duty_status: {op_json} vs {json}"
        );
        assert!(scenario_status.eq_ignore_ascii_case("Performed"), "{json}");
    }

    #[tokio::test]
    async fn pasted_compile_is_not_byte_verified() {
        let (status, json) = post_json("/api/run", eval_body()).await;
        assert_eq!(status, StatusCode::OK, "{json}");
        let report = mill_report(&json);
        assert_eq!(report["sourceTrust"], "unauthenticated", "{json}");
        assert_ne!(report["sourceTrust"], "byteVerified", "{json}");
        let hex_src = r#"
module Examples.T version "0.1.0" {
    source_manifest "/etc/passwd"
    import Other.Law version "1" { digest "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let mut body = eval_body();
        body["source"] = serde_json::json!(hex_src);
        let (status, json) = post_json("/api/run", body).await;
        if json["ok"] == true {
            let report = mill_report(&json);
            assert_eq!(report["sourceTrust"], "unauthenticated", "{json}");
            assert_ne!(report["sourceTrust"], "byteVerified", "{status} {json}");
        } else {
            assert_ne!(json["sourceTrust"], "byteVerified", "{status} {json}");
        }
    }

    #[tokio::test]
    async fn run_unknown_query_is_engine_error_not_inconsistent() {
        let mut body = eval_body();
        body["query"] = serde_json::json!("no_such_query");
        let (status, json) = post_json("/api/run", body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{json}");
        assert_eq!(json["kind"], "engineError", "{json}");
        assert_eq!(json["error"], "UnknownQuery", "{json}");
        assert_ne!(json["outcome"]["kind"], "inconsistent", "{json}");
    }

    #[tokio::test]
    async fn explore_evaluates_with_optional_bounds() {
        let mut body = eval_body();
        body["bounds"] = serde_json::json!({
            "interpretations": {"SuccessorEligibility": ["I1", "I2"]},
            "evidence": {},
            "choices": {}
        });
        let (status, json) = post_json("/api/explore", body).await;
        assert_eq!(status, StatusCode::OK);
        assert_outcome_document(&json, "q");
    }

    #[tokio::test]
    async fn render_interpolates_module_vars() {
        let (status, json) = post_json(
            "/api/render",
            serde_json::json!({
                "source": SAMPLE,
                "template": "{{module}}@{{version}}"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["ok"], true, "{json}");
        assert_eq!(json["text"], "Examples.T@0.1.0");
    }

    #[tokio::test]
    async fn render_missing_key_fails_closed() {
        let (status, json) = post_json(
            "/api/render",
            serde_json::json!({
                "source": SAMPLE,
                "template": "{{missing}}"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["ok"], false, "{json}");
        assert!(
            json["error"]
                .as_str()
                .is_some_and(|e| e.contains("missing")),
            "{json}"
        );
    }

    #[tokio::test]
    async fn mill_binds_loopback_only() {
        let listener = bind_loopback(0).await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert_eq!(addr.ip(), Ipv4Addr::LOCALHOST);
        assert!(!addr.ip().is_unspecified());
        drop(listener);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn mill_mock_router_accepts_on_127_0_0_1() {
        let listener = bind_loopback(0).await.unwrap();
        let addr = listener.local_addr().unwrap();
        assert_eq!(addr.ip(), Ipv4Addr::LOCALHOST);
        let mock = Router::new().route("/api/health", get(|| async { "ok" }));
        tokio::spawn(async move {
            axum::serve(listener, mock).await.unwrap();
        });
        let mut stream = None;
        for _ in 0..50 {
            if let Ok(s) = std::net::TcpStream::connect(addr) {
                stream = Some(s);
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let mut stream = stream.expect("connect to mock mill on 127.0.0.1");
        stream
            .write_all(b"GET /api/health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut buf = String::new();
        stream.read_to_string(&mut buf).unwrap();
        assert!(buf.contains("ok"), "{buf}");
    }
}
