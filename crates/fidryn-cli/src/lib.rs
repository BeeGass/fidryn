//! Fidryn command-line toolchain.

pub mod ui;

use clap::{Parser, Subcommand};
use fidryn_adapt::{DryRun, FilingAdapter, MassachusettsCorporations};
use fidryn_check::check;
use fidryn_core::{
    AdmissibleCompletions, CaseRecord, CoreDecl, CoreModule, Diagnostic, DiagnosticCode,
    EngineError, EvaluationReport, ExecutionMode, Instant, Outcome, QueryName, RunContext,
    SourceManifest, TimeError, TraceId, Value, canonical_json,
};
use fidryn_hir::elaborate;
use fidryn_render::{module_vars, render};
use fidryn_syntax::ast::{HeaderKind, Item};
use fidryn_syntax::{format_module, parse_file};
pub(crate) use fidryn_trace::render_report;
use fidryn_trace::{REPORT_QUALIFICATION_FIELDS, explain, explain_value};
use fidryn_verify::{explore_query, verify_property};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

thread_local! {
    static DRIVER: RefCell<fidryn_driver::Driver> = RefCell::new(fidryn_driver::Driver::new());
}

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
        /// Write a case fact (`KEY=VALUE` sets `case.facts[KEY]`). Missing `=` is an error.
        #[arg(long = "arg", value_name = "KEY=VALUE")]
        args: Vec<String>,
        /// Evaluate with case assumptions as a scenario overlay.
        #[arg(long)]
        scenario: bool,
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
            scenario,
        } => cmd_run(&path, &query, &case, &valid_at, &known_at, &args, scenario),
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

/// Apply `--arg KEY=VALUE` bindings to case facts. `provision=...` sets
/// `case.facts["provision"]`. This does not select an interpretation or
/// otherwise choose a completion. A binding without `=` is an error.
pub fn apply_run_args(case: &mut CaseRecord, args: &[String]) -> Result<(), String> {
    for a in args {
        let Some((k, v)) = a.split_once('=') else {
            return Err(format!(
                "--arg `{a}` must be KEY=VALUE (writes case.facts[KEY])"
            ));
        };
        if k.is_empty() {
            return Err("--arg KEY=VALUE requires a nonempty KEY".into());
        }
        case.facts.insert(k.to_owned(), Value::String(v.to_owned()));
    }
    Ok(())
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

/// Compile a `.fr` path, loading the declared `source_manifest` or a
/// `sources/` fallback. A declared path that is missing or malformed is a
/// diagnostic. Modules that omit a manifest get an empty default.
pub fn compile_module(path: &Path) -> Result<(CoreModule, SourceManifest), Vec<Diagnostic>> {
    DRIVER.with(|driver| driver.borrow_mut().check_path(path))
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

/// Engine failure from `evaluate` / `explore_query` when those APIs return
/// `Result<Outcome, EngineError>`. Unknown queries and invalid input are
/// never rewritten as `Outcome::Inconsistent` here.
///
/// `kind` is the `EngineError` variant name. HTTP status is derived from
/// the variant (`Internal` -> 500, every other variant -> 400), not from
/// `Display` / `Debug` text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EngineFailure {
    pub kind: &'static str,
    pub message: String,
}

impl fmt::Display for EngineFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl EngineFailure {
    pub(crate) fn from_err(err: EngineError) -> Self {
        Self::from(err)
    }

    pub(crate) fn is_internal(&self) -> bool {
        self.kind == "Internal"
    }
}

impl From<EngineError> for EngineFailure {
    fn from(err: EngineError) -> Self {
        let kind = match &err {
            EngineError::UnknownQuery(_) => "UnknownQuery",
            EngineError::Unsupported(_) => "Unsupported",
            EngineError::FuelExhausted { .. } => "FuelExhausted",
            EngineError::InvalidInput(_) => "InvalidInput",
            EngineError::Internal(_) => "Internal",
        };
        Self {
            kind,
            message: err.to_string(),
        }
    }
}

/// Accept both `Outcome` and `Result<Outcome, EngineError>` (core or eval).
pub(crate) trait IntoEvalOutcome {
    fn into_eval_outcome(self) -> Result<Outcome<Value>, EngineFailure>;
}

impl IntoEvalOutcome for Outcome<Value> {
    fn into_eval_outcome(self) -> Result<Outcome<Value>, EngineFailure> {
        Ok(self)
    }
}

impl IntoEvalOutcome for Result<Outcome<Value>, EngineError> {
    fn into_eval_outcome(self) -> Result<Outcome<Value>, EngineFailure> {
        self.map_err(EngineFailure::from_err)
    }
}

/// Load the source manifest for `module_path`.
///
/// Resolution:
/// 1. `source_manifest "..."` on the module, relative to the module directory
/// 2. else `sources/manifest.json` next to the module
/// 3. else the unique `*.manifest.json` in `sources/` (trust fixture layout)
///
/// A **declared** path that is missing or malformed is a diagnostic, not
/// [`SourceManifest::default`]. `"digest": "fixture"` is a fixture profile
/// label, not an authenticated hash.
pub fn load_manifest(module_path: &Path, src: &str) -> Result<SourceManifest, Vec<Diagnostic>> {
    let dir = module_path.parent().unwrap_or(Path::new("."));
    if let Some(declared) = declared_manifest_path(src) {
        if declared.is_empty() {
            return Err(vec![Diagnostic::new(
                DiagnosticCode::E540,
                "declared source_manifest path is empty",
            )]);
        }
        let path = resolve_manifest_path(dir, &declared);
        return read_manifest(&path, true);
    }

    let default_path = dir.join("sources").join("manifest.json");
    if default_path.is_file() {
        return read_manifest(&default_path, false);
    }

    let sources_dir = dir.join("sources");
    if let Some(unique) = unique_glob_manifest(&sources_dir) {
        return read_manifest(&unique, false);
    }

    Ok(SourceManifest::default())
}

fn resolve_manifest_path(module_dir: &Path, declared: &str) -> PathBuf {
    let path = Path::new(declared);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        module_dir.join(path)
    }
}

fn declared_manifest_path(src: &str) -> Option<String> {
    let parsed = parse_file(src);
    if let Some(module) = parsed.module() {
        for item in &module.items {
            if let Item::Header(header) = item
                && header.kind == HeaderKind::SourceManifest
            {
                let path = unquote_manifest_path(&header.value);
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }
    }
    scan_source_manifest_header(src)
}

fn scan_source_manifest_header(src: &str) -> Option<String> {
    for line in src.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("source_manifest") else {
            continue;
        };
        let path = unquote_manifest_path(rest);
        if !path.is_empty() {
            return Some(path);
        }
    }
    None
}

fn unquote_manifest_path(raw: &str) -> String {
    let raw = raw.trim().trim_end_matches(';').trim();
    if let Some(inner) = raw.strip_prefix('"') {
        return inner
            .split_once('"')
            .map(|(s, _)| s)
            .unwrap_or(inner)
            .trim()
            .to_owned();
    }
    if let Some(inner) = raw.strip_prefix('\'') {
        return inner
            .split_once('\'')
            .map(|(s, _)| s)
            .unwrap_or(inner)
            .trim()
            .to_owned();
    }
    raw.split_whitespace().next().unwrap_or("").to_owned()
}

fn unique_glob_manifest(sources_dir: &Path) -> Option<PathBuf> {
    if !sources_dir.is_dir() {
        return None;
    }
    let mut found = Vec::new();
    let entries = fs::read_dir(sources_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name()?.to_str()?;
        if name.ends_with(".manifest.json") {
            found.push(path);
        }
    }
    if found.len() == 1 { found.pop() } else { None }
}

fn read_manifest(path: &Path, declared: bool) -> Result<SourceManifest, Vec<Diagnostic>> {
    match fs::read_to_string(path) {
        Ok(text) => parse_manifest_json(&text, path),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            if declared {
                Err(vec![Diagnostic::new(
                    DiagnosticCode::E540,
                    format!("declared source_manifest `{}` is missing", path.display()),
                )])
            } else {
                Ok(SourceManifest::default())
            }
        }
        Err(err) => Err(vec![Diagnostic::new(
            DiagnosticCode::E540,
            format!("cannot read source_manifest {}: {err}", path.display()),
        )]),
    }
}

fn parse_manifest_json(text: &str, path: &Path) -> Result<SourceManifest, Vec<Diagnostic>> {
    serde_json::from_str::<SourceManifest>(text).map_err(|err| {
        vec![Diagnostic::new(
            DiagnosticCode::E100,
            format!("malformed source_manifest {}: {err}", path.display()),
        )]
    })
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

/// Copy nonempty `case.assumptions` onto the evaluation-report envelope.
///
/// Explore evaluates under the scenario overlay when assumptions are
/// nonempty; this only labels the report. Nonempty assumptions are
/// `executionMode: scenario`.
pub(crate) fn apply_scenario_envelope(report: &mut EvaluationReport, case: &CaseRecord) {
    if !case.assumptions.is_empty() {
        report.execution_mode = ExecutionMode::Scenario;
        report.assumptions = case.assumptions.clone();
    }
}

/// Explore `query` and wrap it as an evaluation report.
///
/// Assignment evaluation uses `evaluate_scenario` when `case.assumptions`
/// is nonempty, so the overlay can change the answer. The envelope is
/// then labeled scenario.
pub(crate) fn explore_report(
    module: &CoreModule,
    query: &QueryName,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<EvaluationReport, EngineFailure> {
    let outcome = explore_query(module, query, case, ctx).into_eval_outcome()?;
    let mut report = EvaluationReport::from_outcome(outcome);
    apply_scenario_envelope(&mut report, case);
    Ok(report)
}

fn cmd_run(
    path: &Path,
    query: &str,
    case_path: &Path,
    valid_at: &str,
    known_at: &str,
    args: &[String],
    scenario: bool,
) -> ExitCode {
    let Ok((module, _)) = compile_or_exit(path) else {
        return ExitCode::from(1);
    };
    let Ok(mut case) = load_case(case_path) else {
        return ExitCode::from(1);
    };
    if let Err(err) = apply_run_args(&mut case, args) {
        eprintln!("{err}");
        return ExitCode::from(1);
    }
    let Ok(valid) = instant_or_exit("--valid-at", valid_at) else {
        return ExitCode::from(1);
    };
    let Ok(known) = instant_or_exit("--known-at", known_at) else {
        return ExitCode::from(1);
    };
    let ctx = RunContext::new(valid, known);
    let report = match DRIVER.with(|driver| {
        let mut driver = driver.borrow_mut();
        if scenario {
            driver.run_report_scenario(&module, query, &case, &ctx)
        } else {
            driver.run_report(&module, query, &case, &ctx)
        }
    }) {
        Ok(report) => report,
        Err(err) => {
            eprintln!("{}", EngineFailure::from_err(err));
            return ExitCode::from(1);
        }
    };
    println!(
        "{}",
        render_report(
            &module,
            &QueryName::from(query),
            valid,
            known,
            &case,
            &report
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
    let mut report = match explore_report(&module, &QueryName::from(query), &case, &ctx) {
        Ok(report) => report,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::from(1);
        }
    };
    report.trust = DRIVER.with(|driver| driver.borrow().source_trust_of(&module));
    println!(
        "{}",
        render_report(
            &module,
            &QueryName::from(query),
            valid,
            known,
            &case,
            &report
        )
    );
    ExitCode::SUCCESS
}

fn cmd_verify(path: &Path, property: &str) -> ExitCode {
    let Ok((module, _)) = compile_or_exit(path) else {
        return ExitCode::from(1);
    };
    let verdict = verify_property(&module, property);
    if verdict.is_proved() {
        println!("{verdict}");
        ExitCode::SUCCESS
    } else {
        eprintln!("{verdict}");
        ExitCode::from(1)
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
    let Ok(old) = load_diff_input(old_snapshot, query) else {
        return ExitCode::from(1);
    };
    let Ok(new) = load_diff_input(new_snapshot, query) else {
        return ExitCode::from(1);
    };
    let diff = NamedSnapshotDiff {
        result: SnapshotDiff::from_maps(&old.result, &new.result),
        assurance: SnapshotDiff::from_maps(&old.assurance, &new.assurance),
    };
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

/// Canonical `{added, removed, changed}` of named fingerprints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SnapshotDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

/// Named report comparison: semantic result vs assurance/provenance.
///
/// `result` is the outcomeDocument (or compiled module/query bodies).
/// `assurance` is executionMode, sourceTrust, assumptions, and
/// verificationMethod. Changing only qualifications must not be silent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NamedSnapshotDiff {
    pub result: SnapshotDiff,
    pub assurance: SnapshotDiff,
}

struct DiffInput {
    result: BTreeMap<String, String>,
    assurance: BTreeMap<String, String>,
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

/// Names and fingerprints from a compiled module.
///
/// The module fingerprint includes rules, functions, duties, and nominations.
/// Query fingerprints serialize the whole [`fidryn_core::CoreQuery`], including
/// `plan` / body. A function-body-only change (`f(){true}` vs `f(){false}`
/// with the same `return f()` query) must change the snapshot.
pub fn snapshot_names_from_module(module: &CoreModule) -> BTreeMap<String, String> {
    let mut names = BTreeMap::new();
    let mut functions = Vec::new();
    let mut duties = Vec::new();
    let mut rules = Vec::new();
    for decl in &module.declarations {
        match decl {
            CoreDecl::Function(function) => functions.push(function),
            CoreDecl::Duty(duty) => duties.push(duty),
            CoreDecl::Rule(rule) => rules.push(rule),
            _ => {}
        }
    }
    let module_fp = canonical_json(&serde_json::json!({
        "name": module.name,
        "version": module.version,
        "outside_scope": module.outside_scope,
        "rules": rules,
        "functions": functions,
        "duties": duties,
        "nominations": module.nominations,
    }))
    .expect("canonical json");
    names.insert(module.name.clone(), module_fp);
    for function in &functions {
        names
            .entry(function.name.clone())
            .or_insert_with(|| canonical_json(function).expect("canonical json"));
    }
    for duty in &duties {
        names
            .entry(duty.name.clone())
            .or_insert_with(|| canonical_json(duty).expect("canonical json"));
    }
    for rule in &rules {
        names
            .entry(rule.name.clone())
            .or_insert_with(|| canonical_json(rule).expect("canonical json"));
    }
    for nomination in &module.nominations {
        let key = format!("nomination:{}:{}", nomination.office, nomination.candidate);
        names
            .entry(key)
            .or_insert_with(|| canonical_json(nomination).expect("canonical json"));
    }
    for query in &module.queries {
        names.insert(
            query.name.clone(),
            canonical_json(query).expect("canonical json"),
        );
    }
    names
}

/// Names and fingerprints from an outcome JSON (or a serialized module).
///
/// This is the semantic **result** map: mill `{ok, report}` unwraps to the
/// report, and `outcomeDocument` is compared without envelope
/// qualifications. Use [`assurance_snapshot_names`] / [`assurance_diff`]
/// for mode, trust, assumptions, and verificationMethod.
pub fn snapshot_names_from_json(
    value: &serde_json::Value,
    query: &str,
) -> BTreeMap<String, String> {
    result_snapshot_names(value, query)
}

/// Semantic-result fingerprints (outcome / compiled query bodies).
pub fn result_snapshot_names(value: &serde_json::Value, query: &str) -> BTreeMap<String, String> {
    let value = unwrap_mill_report(value);
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

/// Assurance/provenance fingerprints from an evaluation-report envelope.
///
/// Keys are [`REPORT_QUALIFICATION_FIELDS`]. Outcome-only JSON yields an
/// empty map. Mill transport unwraps `report` first.
pub fn assurance_snapshot_names(value: &serde_json::Value) -> BTreeMap<String, String> {
    let value = unwrap_mill_report(value);
    let mut names = BTreeMap::new();
    let Some(obj) = value.as_object() else {
        return names;
    };
    if !is_evaluation_report_object(obj) {
        return names;
    }
    for key in REPORT_QUALIFICATION_FIELDS {
        if let Some(field) = obj.get(*key) {
            names.insert(
                (*key).to_owned(),
                canonical_json(field).unwrap_or_else(|_| field.to_string()),
            );
        }
    }
    names
}

/// Semantic-result diff of two snapshots (outcomeDocument / module bodies).
pub fn result_diff(old: &serde_json::Value, new: &serde_json::Value, query: &str) -> SnapshotDiff {
    SnapshotDiff::from_maps(
        &result_snapshot_names(old, query),
        &result_snapshot_names(new, query),
    )
}

/// Assurance/provenance diff (mode, trust, assumptions, verificationMethod).
pub fn assurance_diff(old: &serde_json::Value, new: &serde_json::Value) -> SnapshotDiff {
    SnapshotDiff::from_maps(
        &assurance_snapshot_names(old),
        &assurance_snapshot_names(new),
    )
}

/// Combined named operations used by `fidryn diff`.
pub fn named_diff(
    old: &serde_json::Value,
    new: &serde_json::Value,
    query: &str,
) -> NamedSnapshotDiff {
    NamedSnapshotDiff {
        result: result_diff(old, new, query),
        assurance: assurance_diff(old, new),
    }
}

fn unwrap_mill_report(value: &serde_json::Value) -> &serde_json::Value {
    match value.as_object() {
        Some(obj) => match obj.get("report") {
            Some(report) if is_evaluation_report_value(report) => report,
            _ => value,
        },
        None => value,
    }
}

fn is_evaluation_report_value(value: &serde_json::Value) -> bool {
    value.as_object().is_some_and(is_evaluation_report_object)
}

fn is_evaluation_report_object(obj: &serde_json::Map<String, serde_json::Value>) -> bool {
    obj.get("schema").and_then(|s| s.as_str()) == Some("fidryn.evaluation-report/v0.1")
        || obj.contains_key("outcomeDocument")
        || obj.contains_key("executionMode")
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
    if let Some(report) = obj.get("report")
        && is_evaluation_report_value(report)
    {
        merge_json_snapshot(names, report, query);
        return;
    }
    if let Some(nested) = obj.get("outcomeDocument") {
        merge_json_snapshot(names, nested, query);
        return;
    }
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
        .is_some_and(|e| e.eq_ignore_ascii_case("fr"))
}

fn load_diff_input(path: &Path, query: &str) -> Result<DiffInput, ExitCode> {
    if is_fidryn_source(path) {
        let (module, _) = compile_or_exit(path)?;
        return Ok(DiffInput {
            result: snapshot_names_from_module(&module),
            assurance: BTreeMap::new(),
        });
    }
    let text = fs::read_to_string(path).map_err(|e| {
        eprintln!("cannot read {}: {e}", path.display());
        ExitCode::from(1)
    })?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
        eprintln!("invalid snapshot JSON {}: {e}", path.display());
        ExitCode::from(1)
    })?;
    Ok(DiffInput {
        result: result_snapshot_names(&value, query),
        assurance: assurance_snapshot_names(&value),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::{
        Assumption, CoreNomination, CoreRule, ExecutionMode, Guard, NodeId, QueryPlan, RuleKind,
        Term,
    };
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn workspace_file(rel: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
    }

    fn temp_module_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "fidryn-cli-manifest-{}-{}-{tag}",
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(dir.join("sources")).expect("temp sources");
        dir
    }

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
    fn snapshot_from_evaluation_report_json_uses_outcome_document() {
        let v = json!({
            "schema": "fidryn.evaluation-report/v0.1",
            "executionMode": "operative",
            "sourceTrust": "unauthenticated",
            "outcomeDocument": {
                "schema": "fidryn.outcome/v0.1",
                "module": "Examples.T@0.1.0",
                "query": "acting_trustee",
                "outcome": {"kind": "determinate", "trace": "aa"}
            }
        });
        let names = snapshot_names_from_json(&v, "acting_trustee");
        assert!(names.contains_key("Examples.T@0.1.0"));
        assert!(names.contains_key("acting_trustee"));
        let other = json!({
            "schema": "fidryn.evaluation-report/v0.1",
            "outcomeDocument": {
                "schema": "fidryn.outcome/v0.1",
                "module": "Examples.T@0.1.0",
                "query": "acting_trustee",
                "outcome": {"kind": "suspended", "trace": "bb"}
            }
        });
        let diff = SnapshotDiff::from_maps(
            &snapshot_names_from_json(&v, "acting_trustee"),
            &snapshot_names_from_json(&other, "acting_trustee"),
        );
        assert!(diff.changed.contains(&"acting_trustee".to_string()));
    }

    #[test]
    fn render_report_uses_evaluation_report_schema() {
        let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = compile_source(src, &SourceManifest::default()).expect("compile");
        let case = CaseRecord::default();
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let report = DRIVER
            .with(|driver| driver.borrow_mut().run_report(&module, "q", &case, &ctx))
            .expect("report");
        let text = render_report(&module, &QueryName::from("q"), t, t, &case, &report);
        let json: serde_json::Value = serde_json::from_str(&text).expect("report json");
        assert_eq!(json["schema"], "fidryn.evaluation-report/v0.1", "{json}");
        assert_eq!(
            json["outcomeDocument"]["schema"], "fidryn.outcome/v0.1",
            "{json}"
        );
        assert_eq!(json["executionMode"], "operative", "{json}");
        assert_eq!(json["sourceTrust"], "unauthenticated", "{json}");
        assert_eq!(json["verificationMethod"], "none", "{json}");
        assert!(json["assumptions"].as_array().unwrap().is_empty(), "{json}");
        assert!(json["coverage"].is_null(), "{json}");
        assert_eq!(json["outcomeDocument"]["query"], "q", "{json}");
        assert!(
            json["outcomeDocument"]["outcome"]["kind"].is_string(),
            "{json}"
        );
        assert!(!text.contains(' '));
    }

    #[test]
    fn run_parses_scenario_flag() {
        let with_flag = Cli::try_parse_from([
            "fidryn",
            "run",
            "mod.fr",
            "--query",
            "q",
            "--case",
            "case.json",
            "--valid-at",
            "2033-01-01T00:00:00Z",
            "--known-at",
            "2033-01-01T00:00:00Z",
            "--scenario",
        ])
        .expect("parse --scenario");
        match with_flag.command {
            Command::Run { scenario, .. } => assert!(scenario),
            other => panic!("expected run, got {other:?}"),
        }
        let without = Cli::try_parse_from([
            "fidryn",
            "run",
            "mod.fr",
            "--query",
            "q",
            "--case",
            "case.json",
            "--valid-at",
            "2033-01-01T00:00:00Z",
            "--known-at",
            "2033-01-01T00:00:00Z",
        ])
        .expect("parse run");
        match without.command {
            Command::Run { scenario, .. } => assert!(!scenario),
            other => panic!("expected run, got {other:?}"),
        }
    }

    #[test]
    fn run_report_with_assumptions_renders_scenario_envelope() {
        let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = compile_source(src, &SourceManifest::default()).expect("compile");
        let mut case = CaseRecord::default();
        case.assumptions.push(Assumption {
            id: "hyp-1".into(),
            payload: Value::Bool(true),
        });
        let events_before = case.events.clone();
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let report = DRIVER
            .with(|driver| driver.borrow_mut().run_report(&module, "q", &case, &ctx))
            .expect("report");
        assert_eq!(report.execution_mode, ExecutionMode::Scenario);
        assert_eq!(report.assumptions, case.assumptions);
        assert_eq!(case.events, events_before);
        let text = render_report(&module, &QueryName::from("q"), t, t, &case, &report);
        let json: serde_json::Value = serde_json::from_str(&text).expect("report json");
        assert_eq!(json["schema"], "fidryn.evaluation-report/v0.1", "{json}");
        assert_eq!(
            json["outcomeDocument"]["schema"], "fidryn.outcome/v0.1",
            "{json}"
        );
        assert_eq!(json["executionMode"], "scenario", "{json}");
        assert_eq!(json["assumptions"][0]["id"], "hyp-1", "{json}");
        assert_eq!(json["assumptions"][0]["payload"], true, "{json}");
    }

    #[test]
    fn explore_report_copies_case_assumptions() {
        let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let module = compile_source(src, &SourceManifest::default()).expect("compile");
        let mut case = CaseRecord::default();
        case.assumptions.push(Assumption {
            id: "hyp-explore".into(),
            payload: Value::Bool(true),
        });
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let report = explore_report(&module, &QueryName::from("q"), &case, &ctx).expect("explore");
        assert_eq!(report.execution_mode, ExecutionMode::Scenario);
        assert_eq!(report.assumptions.len(), 1);
        assert_eq!(report.assumptions[0].id, "hyp-explore");
        let text = render_report(&module, &QueryName::from("q"), t, t, &case, &report);
        let json: serde_json::Value = serde_json::from_str(&text).expect("report json");
        assert_eq!(json["schema"], "fidryn.evaluation-report/v0.1", "{json}");
        assert_eq!(json["executionMode"], "scenario", "{json}");
        assert_eq!(json["assumptions"][0]["id"], "hyp-explore", "{json}");
        assert_eq!(
            json["outcomeDocument"]["schema"], "fidryn.outcome/v0.1",
            "{json}"
        );
    }

    #[test]
    fn run_report_assumption_overlay_changes_boolean_fact() {
        let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { flag }
    }
}
"#;
        let module = compile_source(src, &SourceManifest::default()).expect("compile");
        let mut case = CaseRecord::default();
        let mut facts = BTreeMap::new();
        facts.insert("flag".into(), Value::Bool(true));
        case.assumptions.push(Assumption {
            id: "hyp-flag".into(),
            payload: Value::Map(facts),
        });
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let ctx = RunContext::new(t, t);
        let operative = DRIVER.with(|driver| driver.borrow_mut().run(&module, "q", &case, &ctx));
        assert!(
            operative.is_err(),
            "operative evaluate must ignore the flag overlay: {operative:?}"
        );
        let report = DRIVER
            .with(|driver| driver.borrow_mut().run_report(&module, "q", &case, &ctx))
            .expect("run_report scenario");
        assert_eq!(report.execution_mode, ExecutionMode::Scenario);
        match &report.outcome {
            Outcome::Determinate { value, .. } => {
                assert_eq!(value, &Value::Bool(true), "{value:?}");
            }
            other => panic!("scenario overlay must determine flag: {other:?}"),
        }
    }

    #[test]
    fn run_report_assumption_overlay_changes_duty_status() {
        let src = r#"
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
"#;
        let module = compile_source(src, &SourceManifest::default()).expect("compile");
        let t = Instant::parse("2033-01-01T00:00:00Z").unwrap();
        let mut case = CaseRecord::default();
        case.facts.insert("invoice_date".into(), Value::Instant(t));
        case.determinations
            .push(fidryn_core::case::CaseDetermination {
                issue: "InvoiceIssued(Payer)".into(),
                protocol: "InvoiceIssued".into(),
                established: true,
                decider: "test".into(),
                recorded_at: Some(t),
            });
        case.assumptions.push(Assumption {
            id: "hyp-performed".into(),
            payload: Value::Ctor {
                name: "Performed".into(),
                fields: BTreeMap::new(),
            },
        });
        let ctx = RunContext::new(t, t);
        let operative = DRIVER
            .with(|driver| driver.borrow_mut().run(&module, "q", &case, &ctx))
            .expect("operative");
        let scenario = DRIVER
            .with(|driver| driver.borrow_mut().run_report(&module, "q", &case, &ctx))
            .expect("run_report scenario");
        assert_eq!(scenario.execution_mode, ExecutionMode::Scenario);
        let operative_status = duty_status_label(&operative);
        let scenario_status = duty_status_label(&scenario.outcome);
        assert_ne!(
            operative_status, scenario_status,
            "assumption overlay must change duty_status: operative={operative_status} scenario={scenario_status}"
        );
        assert!(
            !operative_status.eq_ignore_ascii_case("Performed"),
            "{operative_status}"
        );
        assert!(
            scenario_status.eq_ignore_ascii_case("Performed"),
            "{scenario_status}"
        );
    }

    fn duty_status_label(outcome: &Outcome<Value>) -> String {
        match outcome {
            Outcome::Determinate { value, .. } => match value {
                Value::Map(fields) | Value::Ctor { fields, .. } => match fields.get("status") {
                    Some(Value::String(s) | Value::Entity(s)) => s.clone(),
                    Some(Value::Ctor { name, .. }) => name.clone(),
                    _ => value.display_label(),
                },
                other => other.display_label(),
            },
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn result_diff_ignores_qualifications_assurance_diff_does_not() {
        let outcome = json!({
            "schema": "fidryn.outcome/v0.1",
            "module": "Examples.T@0.1.0",
            "query": "acting_trustee",
            "outcome": {"kind": "determinate", "trace": "aa"}
        });
        let operative = json!({
            "schema": "fidryn.evaluation-report/v0.1",
            "executionMode": "operative",
            "sourceTrust": "unauthenticated",
            "verificationMethod": "none",
            "assumptions": [],
            "outcomeDocument": outcome
        });
        let mut scenario = operative.clone();
        scenario["executionMode"] = json!("scenario");
        scenario["assumptions"] = json!([{"id": "hyp-1", "payload": true}]);
        scenario["sourceTrust"] = json!("fixture");
        let result = result_diff(&operative, &scenario, "acting_trustee");
        assert!(
            result.changed.is_empty() && result.added.is_empty() && result.removed.is_empty(),
            "same outcomeDocument must be result-equal: {result:?}"
        );
        let assurance = assurance_diff(&operative, &scenario);
        assert!(
            assurance.changed.contains(&"executionMode".to_string()),
            "{assurance:?}"
        );
        assert!(
            assurance.changed.contains(&"assumptions".to_string()),
            "{assurance:?}"
        );
        assert!(
            assurance.changed.contains(&"sourceTrust".to_string()),
            "{assurance:?}"
        );
        let mill = json!({ "ok": true, "report": scenario });
        let mill_diff = named_diff(&operative, &mill, "acting_trustee");
        assert!(mill_diff.result.changed.is_empty(), "{mill_diff:?}");
        assert!(
            mill_diff
                .assurance
                .changed
                .contains(&"assumptions".to_string()),
            "{mill_diff:?}"
        );
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
    fn evaluate_true_vs_false_changes_query_fingerprint() {
        let src_true = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let src_false = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { false }
    }
}
"#;
        let module_true =
            compile_source(src_true, &SourceManifest::default()).expect("compile true");
        let module_false =
            compile_source(src_false, &SourceManifest::default()).expect("compile false");
        match (&module_true.queries[0].plan, &module_false.queries[0].plan) {
            (QueryPlan::Evaluate(Term::Bool(true)), QueryPlan::Evaluate(Term::Bool(false))) => {}
            other => panic!("compilation must preserve Evaluate true vs false, got {other:?}"),
        }
        let a = snapshot_names_from_module(&module_true);
        let b = snapshot_names_from_module(&module_false);
        assert_ne!(
            a.get("q"),
            b.get("q"),
            "query fingerprint must include the Evaluate plan/body"
        );
    }

    #[test]
    fn rule_body_change_changes_module_snapshot() {
        let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let mut module = compile_source(src, &SourceManifest::default()).expect("compile");
        let meta = module.queries[0].meta.clone();
        module.declarations.push(CoreDecl::Rule(CoreRule {
            id: NodeId::of(b"R"),
            name: "R".into(),
            kind: RuleKind::Derive,
            binders: Vec::new(),
            selection: None,
            guard: Guard::Satisfied,
            consequences: Vec::new(),
            fallback: None,
            meta: meta.clone(),
        }));
        let before = snapshot_names_from_module(&module);
        if let Some(CoreDecl::Rule(rule)) = module.declarations.last_mut() {
            rule.guard = Guard::Not(Box::new(Guard::Satisfied));
        }
        let after = snapshot_names_from_module(&module);
        assert_ne!(
            before.get("Examples.T"),
            after.get("Examples.T"),
            "module snapshot must include rule bodies"
        );
        assert_ne!(before.get("R"), after.get("R"));
    }

    #[test]
    fn function_body_change_changes_snapshot_with_same_query() {
        let src_true = r#"
module Examples.T version "0.1.0" {
    fn f() -> Bool { true }
    query q() -> Bool { return f() }
}
"#;
        let src_false = r#"
module Examples.T version "0.1.0" {
    fn f() -> Bool { false }
    query q() -> Bool { return f() }
}
"#;
        let module_true =
            compile_source(src_true, &SourceManifest::default()).expect("compile true");
        let module_false =
            compile_source(src_false, &SourceManifest::default()).expect("compile false");
        let a = snapshot_names_from_module(&module_true);
        let b = snapshot_names_from_module(&module_false);
        assert_ne!(
            a, b,
            "f(){{true}} vs f(){{false}} with the same return f() query must differ"
        );
        assert_ne!(
            a.get("f"),
            b.get("f"),
            "function fingerprint must include the body"
        );
        assert_ne!(
            a.get("Examples.T"),
            b.get("Examples.T"),
            "module snapshot must include function bodies"
        );
    }

    #[test]
    fn duty_and_nomination_changes_change_module_snapshot() {
        let src = r#"
module Examples.T version "0.1.0" {
    entity Payer : NaturalPerson
    entity Payee : NaturalPerson
    duty PayInvoice {
        bearer Payer
        claimant Payee
        content USD(100.00)
    }
    query q() -> Bool { goal Evaluate { true } }
}
"#;
        let mut module = compile_source(src, &SourceManifest::default()).expect("compile");
        let before = snapshot_names_from_module(&module);
        assert!(before.contains_key("PayInvoice"), "{before:?}");
        if let Some(CoreDecl::Duty(duty)) = module
            .declarations
            .iter_mut()
            .find(|decl| matches!(decl, CoreDecl::Duty(_)))
        {
            duty.content.clear();
        }
        let after_duty = snapshot_names_from_module(&module);
        assert_ne!(
            before.get("PayInvoice"),
            after_duty.get("PayInvoice"),
            "duty fingerprint must include content"
        );
        assert_ne!(before.get("Examples.T"), after_duty.get("Examples.T"));
        module.nominations.push(CoreNomination {
            candidate: "Alice".into(),
            office: "Trustee".into(),
            rank: 1,
        });
        let after_nom = snapshot_names_from_module(&module);
        assert_ne!(
            after_duty.get("Examples.T"),
            after_nom.get("Examples.T"),
            "module snapshot must include nominations"
        );
        assert!(after_nom.contains_key("nomination:Trustee:Alice"));
    }

    #[test]
    fn apply_run_args_writes_facts_and_rejects_missing_equals() {
        let mut case = CaseRecord::default();
        apply_run_args(&mut case, &["provision=ChildSupportWaiver".into()]).expect("arg");
        assert_eq!(
            case.facts.get("provision"),
            Some(&Value::String("ChildSupportWaiver".into()))
        );
        let err = apply_run_args(&mut case, &["not-a-binding".into()]).expect_err("missing =");
        assert!(err.contains("KEY=VALUE"), "{err}");
        let err = apply_run_args(&mut case, &["=value".into()]).expect_err("empty key");
        assert!(err.contains("nonempty KEY"), "{err}");
    }

    #[test]
    fn load_manifest_trust_header_matches_file() {
        let path = workspace_file("examples/trust/bryan-revocable-trust.fr");
        let src = fs::read_to_string(&path).expect("read trust");
        let loaded = load_manifest(&path, &src).expect("load declared manifest");
        let expected: SourceManifest = serde_json::from_str(
            &fs::read_to_string(workspace_file(
                "examples/trust/sources/ma-trust-fixture.manifest.json",
            ))
            .expect("read fixture manifest"),
        )
        .expect("parse fixture manifest");
        assert_eq!(loaded, expected);
        assert_eq!(loaded.snapshot, "2026-08-23-ma-trust-fixture");
        assert_eq!(loaded.artifacts[0].digest, "fixture");
    }

    #[test]
    fn load_manifest_undeclared_states_corpus_is_empty_default() {
        let path = workspace_file("examples/states/alabama/final-wages.fr");
        let src = fs::read_to_string(&path).expect("read alabama");
        let loaded = load_manifest(&path, &src).expect("empty default");
        assert!(loaded.snapshot.is_empty(), "{loaded:?}");
        assert!(loaded.artifacts.is_empty(), "{loaded:?}");
    }

    #[test]
    fn load_manifest_declared_missing_is_diagnostic() {
        let dir = temp_module_dir("missing");
        let src = r#"
module Examples.T version "0.1.0" {
    source_manifest "sources/missing.manifest.json"
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let path = dir.join("mod.fr");
        fs::write(&path, src).unwrap();
        let err = load_manifest(&path, src).expect_err("declared missing");
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E540),
            "{err:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_manifest_declared_malformed_is_diagnostic() {
        let dir = temp_module_dir("malformed");
        let src = r#"
module Examples.T version "0.1.0" {
    source_manifest "sources/broken.manifest.json"
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let path = dir.join("mod.fr");
        fs::write(&path, src).unwrap();
        fs::write(dir.join("sources/broken.manifest.json"), "{").unwrap();
        let err = load_manifest(&path, src).expect_err("declared malformed");
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E100),
            "{err:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_manifest_unique_glob_when_undeclared() {
        let dir = temp_module_dir("glob");
        let src = r#"
module Examples.T version "0.1.0" {
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let path = dir.join("mod.fr");
        fs::write(&path, src).unwrap();
        fs::write(
            dir.join("sources/ma-trust-fixture.manifest.json"),
            r#"{
  "schema": "fidryn.source-manifest/v0.1",
  "snapshot": "glob-snap",
  "jurisdiction": "X",
  "artifacts": []
}"#,
        )
        .unwrap();
        let loaded = load_manifest(&path, src).expect("unique glob");
        assert_eq!(loaded.snapshot, "glob-snap");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_manifest_declared_does_not_silent_default() {
        let dir = temp_module_dir("no-default");
        let src = r#"
module Examples.T version "0.1.0" {
    source_manifest "sources/declared.manifest.json"
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let path = dir.join("mod.fr");
        fs::write(&path, src).unwrap();
        fs::write(
            dir.join("sources/manifest.json"),
            r#"{
  "schema": "fidryn.source-manifest/v0.1",
  "snapshot": "should-not-load",
  "jurisdiction": "X",
  "artifacts": []
}"#,
        )
        .unwrap();
        let err = load_manifest(&path, src).expect_err("declared wins over default");
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E540),
            "{err:?}"
        );
        let _ = fs::remove_dir_all(&dir);
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

    #[test]
    fn engine_failure_kinds_match_engine_error_variants() {
        let cases = [
            (EngineError::UnknownQuery("q".into()), "UnknownQuery", false),
            (EngineError::Unsupported("op".into()), "Unsupported", false),
            (
                EngineError::FuelExhausted { remaining: 0 },
                "FuelExhausted",
                false,
            ),
            (EngineError::InvalidInput("x".into()), "InvalidInput", false),
            (EngineError::Internal("boom".into()), "Internal", true),
        ];
        for (err, kind, internal) in cases {
            let fail = EngineFailure::from_err(err.clone());
            assert_eq!(fail.kind, kind, "{err:?}");
            assert_eq!(fail.is_internal(), internal, "{err:?}");
            assert_eq!(fail.message, err.to_string());
            assert_eq!(format!("{fail}"), format!("{kind}: {}", err));
        }
    }

    #[test]
    fn engine_failure_kind_is_not_parsed_from_message() {
        let unknown = EngineFailure::from_err(EngineError::UnknownQuery("Internal".into()));
        assert_eq!(unknown.kind, "UnknownQuery");
        assert!(!unknown.is_internal());

        let internal = EngineFailure::from_err(EngineError::Internal("UnknownQuery".into()));
        assert_eq!(internal.kind, "Internal");
        assert!(internal.is_internal());
    }
}
