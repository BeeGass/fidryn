//! Fidryn command-line toolchain.

pub mod ui;

use clap::{Parser, Subcommand};
use fidryn_adapt::{DryRun, FilingAdapter, MassachusettsCorporations};
use fidryn_check::check;
use fidryn_core::{
    AdmissibleCompletions, CaseRecord, CoreModule, Diagnostic, DiagnosticCode, Instant, QueryName,
    RunContext, SourceManifest, TimeError, TraceId, Value, canonical_json,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use fidryn_hir::elaborate;
use fidryn_render::{module_vars, render};
use fidryn_syntax::{format_module, parse_file};
use fidryn_trace::{explain, explain_value, render_outcome};
use fidryn_verify::{explore_query, verify_property};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "fidryn", version, about = "Fidryn reference interpreter")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Fmt {
        path: PathBuf,
    },
    Check {
        path: PathBuf,
    },
    /// Evaluate a query against a case record. Never chooses a completion.
    Run {
        path: PathBuf,
        #[arg(long)]
        query: String,
        #[arg(long)]
        case: PathBuf,
        /// ISO 8601 / RFC 3339 valid time (`Z` or a numeric offset)
        #[arg(long = "valid-at")]
        valid_at: String,
        /// ISO 8601 / RFC 3339 record time (`Z` or a numeric offset)
        #[arg(long = "known-at")]
        known_at: String,
        /// Set a case fact (`provision=...` writes `case.facts["provision"]`).
        #[arg(long = "arg", value_name = "KEY=VALUE")]
        args: Vec<String>,
    },
    /// Explore a query under explicit finite bounds.
    Explore {
        path: PathBuf,
        #[arg(long)]
        query: String,
        #[arg(long)]
        case: PathBuf,
        #[arg(long)]
        bounds: Option<PathBuf>,
        /// ISO 8601 / RFC 3339 valid time (`Z` or a numeric offset)
        #[arg(long = "valid-at")]
        valid_at: String,
        /// ISO 8601 / RFC 3339 record time (`Z` or a numeric offset)
        #[arg(long = "known-at")]
        known_at: String,
    },
    Explain {
        trace_id: String,
        #[arg(long, default_value = "text")]
        format: String,
    },
    Verify {
        path: PathBuf,
        #[arg(long)]
        property: String,
    },
    Diff {
        old_snapshot: PathBuf,
        new_snapshot: PathBuf,
        #[arg(long)]
        query: String,
    },
    Render {
        path: PathBuf,
        #[arg(long)]
        template: PathBuf,
    },
    /// Submit a filing packet. Live HTTP requires `--live` and
    /// `FIDRYN_ALLOW_LIVE_FILING=1`. Transmission is not a `Filed` fact.
    File {
        packet: PathBuf,
        #[arg(long, default_value = "dry-run")]
        adapter: String,
        #[arg(long)]
        live: bool,
        #[arg(long)]
        endpoint: Option<String>,
    },
    /// Localhost mill on 127.0.0.1. Does not live-file.
    Ui {
        /// Port on 127.0.0.1 (default 8751).
        #[arg(long)]
        port: Option<u16>,
        /// Do not open a browser.
        #[arg(long)]
        no_open: bool,
    },
}

/// Dispatch a parsed CLI invocation.
pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Command::Fmt { path } => cmd_fmt(&path),
        Command::Check { path } => cmd_check(&path),
        Command::Run {
            path,
            query,
            case,
            valid_at,
            known_at,
            args,
        } => cmd_run(&path, &query, &case, &valid_at, &known_at, &args),
        Command::Explore {
            path,
            query,
            case,
            bounds,
            valid_at,
            known_at,
        } => cmd_explore(
            &path,
            &query,
            &case,
            bounds.as_deref(),
            &valid_at,
            &known_at,
        ),
        Command::Explain { trace_id, format } => cmd_explain(&trace_id, &format),
        Command::Verify { path, property } => cmd_verify(&path, &property),
        Command::Diff {
            old_snapshot,
            new_snapshot,
            query,
        } => cmd_diff(&old_snapshot, &new_snapshot, &query),
        Command::Render { path, template } => cmd_render(&path, &template),
        Command::File {
            packet,
            adapter,
            live,
            endpoint,
        } => cmd_file(&packet, &adapter, live, endpoint.as_deref()),
        Command::Ui { port, no_open } => cmd_ui(port, no_open),
    }
}

/// Parse an ISO 8601 / RFC 3339 timestamp.
///
/// `Instant::parse` uses `time::Rfc3339`. Both a `Z` suffix and a numeric
/// offset such as `+00:00` or `-04:00` are accepted. If one spelling is
/// rejected, the `Z` / `+00:00` pair is tried before failing.
pub fn parse_instant(text: &str) -> Result<Instant, TimeError> {
    match Instant::parse(text) {
        Ok(instant) => Ok(instant),
        Err(err) => {
            if let Some(prefix) = text.strip_suffix('Z') {
                Instant::parse(&format!("{prefix}+00:00")).map_err(|_| err)
            } else if let Some(prefix) = text.strip_suffix("+00:00") {
                Instant::parse(&format!("{prefix}Z")).map_err(|_| err)
            } else {
                Err(err)
            }
        }
    }
}

/// Apply `--arg key=value` bindings to case facts. `provision=...` sets
/// `case.facts["provision"]`. This does not select an interpretation or
/// otherwise choose a completion.
pub fn apply_run_args(case: &mut CaseRecord, args: &[String]) {
    for a in args {
        if let Some((k, v)) = a.split_once('=') {
            case.facts.insert(k.to_owned(), Value::String(v.to_owned()));
        }
    }
}

/// Parse, elaborate, and check a module from source text.
pub fn compile_source(src: &str, manifest: &SourceManifest) -> Result<CoreModule, Vec<Diagnostic>> {
    let parsed = parse_file(src);
    if parsed.has_errors() {
        return Err(parsed.diagnostics);
    }
    let hir = elaborate(&parsed, manifest)?;
    check(&hir, manifest)
}

/// Compile a `.fidryn` path, loading `sources/manifest.json` when present.
pub fn compile_module(path: &Path) -> Result<(CoreModule, SourceManifest), Vec<Diagnostic>> {
    let src = fs::read_to_string(path).map_err(|e| {
        vec![Diagnostic::new(
            DiagnosticCode::E100,
            format!("cannot read {}: {e}", path.display()),
        )]
    })?;
    let manifest = load_manifest(path);
    let module = compile_source(&src, &manifest)?;
    Ok((module, manifest))
}

fn emit_diagnostics(diagnostics: &[Diagnostic]) {
    for d in diagnostics {
        eprintln!("{d}");
    }
}

fn compile_or_exit(path: &Path) -> Result<(CoreModule, SourceManifest), ExitCode> {
    compile_module(path).map_err(|ds| {
        emit_diagnostics(&ds);
        ExitCode::from(1)
    })
}

fn load_manifest(module_path: &Path) -> SourceManifest {
    let candidate = module_path
        .parent()
        .unwrap_or(Path::new("."))
        .join("sources")
        .join("manifest.json");
    if let Ok(text) = fs::read_to_string(candidate) {
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        SourceManifest::default()
    }
}

fn load_case(path: &Path) -> Result<CaseRecord, ExitCode> {
    let text = fs::read_to_string(path).map_err(|e| {
        eprintln!("cannot read {}: {e}", path.display());
        ExitCode::from(1)
    })?;
    serde_json::from_str(&text).map_err(|e| {
        eprintln!("invalid case record: {e}");
        ExitCode::from(1)
    })
}

fn instant_or_exit(flag: &str, text: &str) -> Result<Instant, ExitCode> {
    parse_instant(text).map_err(|err| {
        eprintln!(
            "{flag}: {err}. Use ISO 8601 / RFC 3339, for example 2033-01-01T00:00:00Z or 2033-01-01T00:00:00+00:00."
        );
        ExitCode::from(1)
    })
}

fn cmd_fmt(path: &Path) -> ExitCode {
    let src = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {e}", path.display());
            return ExitCode::from(1);
        }
    };
    match format_module(&src) {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(d) => {
            eprintln!("{d}");
            ExitCode::from(1)
        }
    }
}

fn cmd_check(path: &Path) -> ExitCode {
    match compile_or_exit(path) {
        Ok(_) => {
            println!("ok");
            ExitCode::SUCCESS
        }
        Err(code) => code,
    }
}

fn cmd_run(
    path: &Path,
    query: &str,
    case_path: &Path,
    valid_at: &str,
    known_at: &str,
    args: &[String],
) -> ExitCode {
    let Ok((module, _)) = compile_or_exit(path) else {
        return ExitCode::from(1);
    };
    let Ok(mut case) = load_case(case_path) else {
        return ExitCode::from(1);
    };
    apply_run_args(&mut case, args);
    let Ok(valid) = instant_or_exit("--valid-at", valid_at) else {
        return ExitCode::from(1);
    };
    let Ok(known) = instant_or_exit("--known-at", known_at) else {
        return ExitCode::from(1);
    };
    let ctx = RunContext::new(valid, known);
    // Occupancy and completions come only from the case record.
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
    };
    let outcome = evaluate(
        &module,
        &QueryName::from(query),
        &Default::default(),
        &state,
        &ctx,
        &mut handler,
        &case,
    );
    println!(
        "{}",
        render_outcome(
            &module,
            &QueryName::from(query),
            valid,
            known,
            &case,
            &outcome
        )
    );
    ExitCode::SUCCESS
}

/// Merge bounds JSON into `case.admissible_completions` when it parses as
/// `AdmissibleCompletions` or `{interpretations, evidence, choices}`.
pub fn merge_bounds_json(case: &mut CaseRecord, bounds: &serde_json::Value) -> Result<(), String> {
    let candidate = bounds
        .get("admissibleCompletions")
        .or_else(|| bounds.get("admissible_completions"))
        .unwrap_or(bounds);
    let ac: AdmissibleCompletions = serde_json::from_value(candidate.clone()).map_err(|err| {
        format!(
            "bounds JSON must be AdmissibleCompletions or {{interpretations, evidence, choices}}: {err}"
        )
    })?;
    case.admissible_completions
        .interpretations
        .extend(ac.interpretations);
    case.admissible_completions.evidence.extend(ac.evidence);
    case.admissible_completions.choices.extend(ac.choices);
    Ok(())
}

fn cmd_explore(
    path: &Path,
    query: &str,
    case_path: &Path,
    bounds: Option<&Path>,
    valid_at: &str,
    known_at: &str,
) -> ExitCode {
    let Ok((module, _)) = compile_or_exit(path) else {
        return ExitCode::from(1);
    };
    let Ok(mut case) = load_case(case_path) else {
        return ExitCode::from(1);
    };
    if let Some(bounds_path) = bounds {
        let text = match fs::read_to_string(bounds_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("cannot read {}: {e}", bounds_path.display());
                return ExitCode::from(1);
            }
        };
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("invalid bounds JSON: {e}");
                return ExitCode::from(1);
            }
        };
        if let Err(e) = merge_bounds_json(&mut case, &value) {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    }
    let Ok(valid) = instant_or_exit("--valid-at", valid_at) else {
        return ExitCode::from(1);
    };
    let Ok(known) = instant_or_exit("--known-at", known_at) else {
        return ExitCode::from(1);
    };
    let ctx = RunContext::new(valid, known);
    let outcome = explore_query(&module, &QueryName::from(query), &case, &ctx);
    println!(
        "{}",
        render_outcome(
            &module,
            &QueryName::from(query),
            valid,
            known,
            &case,
            &outcome
        )
    );
    ExitCode::SUCCESS
}

fn cmd_verify(path: &Path, property: &str) -> ExitCode {
    let Ok((module, _)) = compile_or_exit(path) else {
        return ExitCode::from(1);
    };
    match verify_property(&module, property) {
        Ok(()) => {
            println!("verified {property}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_render(path: &Path, template: &Path) -> ExitCode {
    let Ok((module, _)) = compile_or_exit(path) else {
        return ExitCode::from(1);
    };
    let template_src = match fs::read_to_string(template) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {e}", template.display());
            return ExitCode::from(1);
        }
    };
    match render(&template_src, &module_vars(&module)) {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_file(packet: &Path, adapter: &str, live: bool, endpoint: Option<&str>) -> ExitCode {
    let text = match fs::read_to_string(packet) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {}: {e}", packet.display());
            return ExitCode::from(1);
        }
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("invalid packet JSON: {e}");
            return ExitCode::from(1);
        }
    };
    let result = match adapter {
        "ma-corporations" | "massachusetts" => {
            MassachusettsCorporations::new(endpoint.unwrap_or("https://www.sec.state.ma.us/filing"))
                .submit(&value, live)
        }
        _ => DryRun.submit(&value, live),
    };
    match result {
        Ok(out) => {
            println!("{}", serde_json::to_string(&out).unwrap_or_default());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_explain(trace_id: &str, format: &str) -> ExitCode {
    match load_explain_source(trace_id) {
        Ok(ExplainSource::File(value)) => {
            println!("{}", explain_value(&value, format));
            ExitCode::SUCCESS
        }
        Ok(ExplainSource::Hashed(id)) => {
            println!("{}", explain(id, format));
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

enum ExplainSource {
    File(serde_json::Value),
    Hashed(TraceId),
}

fn load_explain_source(trace_id: &str) -> Result<ExplainSource, String> {
    let as_path = Path::new(trace_id);
    let candidate = if as_path.exists() {
        Some(as_path.to_path_buf())
    } else {
        let with_json = PathBuf::from(format!("{trace_id}.json"));
        if with_json.exists() {
            Some(with_json)
        } else {
            None
        }
    };
    match candidate {
        Some(path) => {
            let text = fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let value = serde_json::from_str(&text)
                .map_err(|e| format!("invalid trace JSON {}: {e}", path.display()))?;
            Ok(ExplainSource::File(value))
        }
        None => Ok(ExplainSource::Hashed(TraceId::of(trace_id.as_bytes()))),
    }
}

fn cmd_diff(old_snapshot: &Path, new_snapshot: &Path, query: &str) -> ExitCode {
    let Ok(old) = load_snapshot(old_snapshot, query) else {
        return ExitCode::from(1);
    };
    let Ok(new) = load_snapshot(new_snapshot, query) else {
        return ExitCode::from(1);
    };
    let diff = SnapshotDiff::from_maps(&old, &new);
    match canonical_json(&diff) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("cannot encode diff: {e}");
            ExitCode::from(1)
        }
    }
}

fn cmd_ui(port: Option<u16>, no_open: bool) -> ExitCode {
    let preferred = port.unwrap_or(ui::default_port());
    match tokio::runtime::Runtime::new() {
        Ok(rt) => match rt.block_on(ui::serve(preferred, no_open)) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                eprintln!("{err}");
                ExitCode::from(1)
            }
        },
        Err(err) => {
            eprintln!("tokio runtime: {err}");
            ExitCode::from(1)
        }
    }
}

/// Canonical `{added, removed, changed}` of query and module names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SnapshotDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

impl SnapshotDiff {
    pub fn from_maps(old: &BTreeMap<String, String>, new: &BTreeMap<String, String>) -> Self {
        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut changed = Vec::new();
        for (name, fp) in old {
            match new.get(name) {
                None => removed.push(name.clone()),
                Some(other) if other != fp => changed.push(name.clone()),
                Some(_) => {}
            }
        }
        for name in new.keys() {
            if !old.contains_key(name) {
                added.push(name.clone());
            }
        }
        added.sort();
        removed.sort();
        changed.sort();
        Self {
            added,
            removed,
            changed,
        }
    }
}

/// Names and fingerprints from a compiled module (module name plus queries).
pub fn snapshot_names_from_module(module: &CoreModule) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    let module_fp = canonical_json(&serde_json::json!({
        "name": module.name,
        "version": module.version,
        "outside_scope": module.outside_scope,
    }))
    .expect("canonical json");
    names.insert(module.name.clone(), module_fp);
    for query in &module.queries {
        names.insert(
            query.name.clone(),
            canonical_json(query).expect("canonical json"),
        );
    }
    names
}

/// Names and fingerprints from an outcome JSON (or a serialized module).
pub fn snapshot_names_from_json(
    value: &serde_json::Value,
    query: &str,
) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                merge_json_snapshot(&mut names, item, query);
            }
        }
        other => merge_json_snapshot(&mut names, other, query),
    }
    if names.is_empty() {
        names.insert(
            query.to_owned(),
            canonical_json(value).unwrap_or_else(|_| value.to_string()),
        );
    }
    names
}

fn merge_json_snapshot(
    names: &mut BTreeMap<String, String>,
    value: &serde_json::Value,
    query: &str,
) {
    let Some(obj) = value.as_object() else {
        names.insert(
            query.to_owned(),
            canonical_json(value).unwrap_or_else(|_| value.to_string()),
        );
        return;
    };
    if let Some(module) = obj.get("module").and_then(|m| m.as_str()) {
        let module_fp = canonical_json(&serde_json::json!({
            "module": module,
            "sourceSnapshot": obj.get("sourceSnapshot"),
        }))
        .unwrap_or_else(|_| module.to_owned());
        names.insert(module.to_owned(), module_fp);
    }
    if let Some(q) = obj.get("query").and_then(|q| q.as_str()) {
        let fp = obj
            .get("outcome")
            .map(|o| canonical_json(o).unwrap_or_else(|_| o.to_string()))
            .unwrap_or_else(|| canonical_json(value).unwrap_or_else(|_| value.to_string()));
        names.insert(q.to_owned(), fp);
        return;
    }
    if let Some(queries) = obj.get("queries") {
        if let Some(name) = obj.get("name").and_then(|n| n.as_str()) {
            names.insert(
                name.to_owned(),
                canonical_json(&serde_json::json!({
                    "name": name,
                    "version": obj.get("version"),
                }))
                .unwrap_or_else(|_| name.to_owned()),
            );
        }
        match queries {
            serde_json::Value::Object(map) => {
                for (k, v) in map {
                    names.insert(
                        k.clone(),
                        canonical_json(v).unwrap_or_else(|_| v.to_string()),
                    );
                }
            }
            serde_json::Value::Array(items) => {
                for q in items {
                    if let Some(name) = q.get("name").and_then(|n| n.as_str()) {
                        names.insert(
                            name.to_owned(),
                            canonical_json(q).unwrap_or_else(|_| q.to_string()),
                        );
                    } else if let Some(s) = q.as_str() {
                        names.insert(s.to_owned(), s.to_owned());
                    }
                }
            }
            _ => {}
        }
        return;
    }
    if obj.contains_key("kind") && obj.contains_key("trace") {
        names.insert(
            query.to_owned(),
            canonical_json(value).unwrap_or_else(|_| value.to_string()),
        );
    }
}

fn is_fidryn_source(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("fidryn"))
}

fn load_snapshot(path: &Path, query: &str) -> Result<BTreeMap<String, String>, ExitCode> {
    if is_fidryn_source(path) {
        let (module, _) = compile_or_exit(path)?;
        return Ok(snapshot_names_from_module(&module));
    }
    let text = fs::read_to_string(path).map_err(|e| {
        eprintln!("cannot read {}: {e}", path.display());
        ExitCode::from(1)
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        eprintln!("invalid snapshot JSON {}: {e}", path.display());
        ExitCode::from(1)
    })?;
    Ok(snapshot_names_from_json(&value, query))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn snapshot_diff_reports_added_removed_changed() {
        let mut old = BTreeMap::new();
        old.insert("Examples.T".into(), "v1".into());
        old.insert("q1".into(), "a".into());
        old.insert("q2".into(), "b".into());
        let mut new = BTreeMap::new();
        new.insert("Examples.T".into(), "v2".into());
        new.insert("q1".into(), "a".into());
        new.insert("q3".into(), "c".into());
        let diff = SnapshotDiff::from_maps(&old, &new);
        assert_eq!(diff.added, vec!["q3".to_string()]);
        assert_eq!(diff.removed, vec!["q2".to_string()]);
        assert_eq!(diff.changed, vec!["Examples.T".to_string()]);
        let text = canonical_json(&diff).unwrap();
        assert_eq!(
            text,
            r#"{"added":["q3"],"changed":["Examples.T"],"removed":["q2"]}"#
        );
    }

    #[test]
    fn snapshot_from_outcome_json_uses_query_and_module() {
        let v = json!({
            "schema": "fidryn.outcome/v0.1",
            "module": "Examples.T@0.1.0",
            "query": "acting_trustee",
            "outcome": {"kind": "determinate", "trace": "aa"}
        });
        let names = snapshot_names_from_json(&v, "acting_trustee");
        assert!(names.contains_key("Examples.T@0.1.0"));
        assert!(names.contains_key("acting_trustee"));
        let other = json!({
            "schema": "fidryn.outcome/v0.1",
            "module": "Examples.T@0.1.0",
            "query": "acting_trustee",
            "outcome": {"kind": "suspended", "trace": "bb"}
        });
        let diff = SnapshotDiff::from_maps(
            &snapshot_names_from_json(&v, "acting_trustee"),
            &snapshot_names_from_json(&other, "acting_trustee"),
        );
        assert!(diff.changed.contains(&"acting_trustee".to_string()));
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
    }

    #[test]
    fn snapshot_from_module_includes_query_names() {
        let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = compile_source(src, &SourceManifest::default()).expect("compile");
        let names = snapshot_names_from_module(&module);
        assert!(names.contains_key("Examples.T"));
        assert!(names.contains_key("q"));
    }

    #[test]
    fn merge_bounds_json_into_admissible_completions() {
        let mut case = CaseRecord::default();
        let bounds = json!({
            "interpretations": {"SuccessorEligibility": ["I1", "I2"]},
            "evidence": {},
            "choices": {"k": ["a"]}
        });
        merge_bounds_json(&mut case, &bounds).unwrap();
        assert_eq!(
            case.admissible_completions
                .interpretations
                .get("SuccessorEligibility"),
            Some(&vec!["I1".to_string(), "I2".to_string()])
        );
        assert_eq!(
            case.admissible_completions.choices.get("k"),
            Some(&vec!["a".to_string()])
        );
        let nested = json!({
            "admissibleCompletions": {
                "interpretations": {"Other": ["X"]},
                "choices": {}
            }
        });
        merge_bounds_json(&mut case, &nested).unwrap();
        assert_eq!(
            case.admissible_completions.interpretations.get("Other"),
            Some(&vec!["X".to_string()])
        );
    }
}
