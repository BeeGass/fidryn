//! Incremental compile and evaluate driver.
//!
//! Check is memoized by blake3 of source bytes together with the manifest
//! snapshot and artifact digests. Evaluate is memoized by canonical module
//! JSON (executable content, not only [`fidryn_core::ModuleId`]), query name,
//! query arguments, canonical case JSON, and bitemporal times. Memo tables
//! are explicit [`HashMap`]s; the `salsa` crate is not used.

use fidryn_check::check;
use fidryn_core::{
    CaseRecord, CoreModule, Diagnostic, DiagnosticCode, EngineError, Outcome, QueryName,
    RunContext, SourceManifest, Value, canonical_json,
};
use fidryn_eval::evaluate;
use fidryn_handlers::CaseFile;
use fidryn_hir::elaborate;
use fidryn_syntax::ast::{HeaderKind, Item};
use fidryn_syntax::parse_file;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Incremental parse/check/evaluate session.
pub struct Driver {
    check_cache: HashMap<CheckKey, Result<CoreModule, Vec<Diagnostic>>>,
    run_cache: HashMap<RunKey, Result<Outcome<Value>, EngineError>>,
    hits: u64,
    misses: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct CheckKey([u8; 32]);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RunKey([u8; 32]);

impl Driver {
    /// Empty memo tables.
    pub fn new() -> Self {
        Self {
            check_cache: HashMap::new(),
            run_cache: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    /// Combined check and run cache hits.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Combined check and run cache misses.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Parse, elaborate, and check `source` against `manifest`.
    ///
    /// The memo key is blake3(source) together with the snapshot string and
    /// artifact digests. Comment-only edits change the source bytes and miss.
    pub fn check_source(
        &mut self,
        source: &str,
        manifest: &SourceManifest,
    ) -> Result<CoreModule, Vec<Diagnostic>> {
        let key = check_key(source, manifest);
        if let Some(cached) = self.check_cache.get(&key) {
            self.hits += 1;
            return cached.clone();
        }
        self.misses += 1;
        let result = compile_source(source, manifest);
        self.check_cache.insert(key, result.clone());
        result
    }

    /// Compile a `.fr` path, loading the declared `source_manifest` or a
    /// `sources/` fallback. A declared path that is missing or malformed is a
    /// diagnostic. Modules that omit a manifest get an empty default.
    pub fn check_path(
        &mut self,
        path: &Path,
    ) -> Result<(CoreModule, SourceManifest), Vec<Diagnostic>> {
        let src = fs::read_to_string(path).map_err(|e| {
            vec![Diagnostic::new(
                DiagnosticCode::E100,
                format!("cannot read {}: {e}", path.display()),
            )]
        })?;
        let manifest = load_manifest(path, &src)?;
        let module = self.check_source(&src, &manifest)?;
        Ok((module, manifest))
    }

    /// Compile a `.fr` path, discarding the loaded manifest.
    pub fn check(&mut self, path: &Path) -> Result<CoreModule, Vec<Diagnostic>> {
        self.check_path(path).map(|(module, _)| module)
    }

    /// Evaluate `query` against `case`. Hits return a clone of the stored
    /// outcome. The memo key includes executable module content and query
    /// arguments. Fuel exhaustion is not stored.
    pub fn run(
        &mut self,
        module: &CoreModule,
        query: &str,
        case: &CaseRecord,
        ctx: &RunContext,
    ) -> Result<Outcome<Value>, EngineError> {
        self.run_with_args(module, query, case, ctx, &BTreeMap::new())
    }

    pub fn run_with_args(
        &mut self,
        module: &CoreModule,
        query: &str,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
    ) -> Result<Outcome<Value>, EngineError> {
        let key = run_key(module, query, args, case, ctx)?;
        if let Some(cached) = self.run_cache.get(&key) {
            self.hits += 1;
            return cached.clone();
        }
        self.misses += 1;
        let result = evaluate_run(module, query, args, case, ctx);
        if !matches!(result, Err(EngineError::FuelExhausted { .. })) {
            self.run_cache.insert(key, result.clone());
        }
        result
    }
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}

fn compile_source(source: &str, manifest: &SourceManifest) -> Result<CoreModule, Vec<Diagnostic>> {
    let parsed = parse_file(source);
    if parsed.has_errors() {
        return Err(parsed.diagnostics);
    }
    let hir = elaborate(&parsed, manifest)?;
    check(&hir, manifest)
}

fn evaluate_run(
    module: &CoreModule,
    query: &str,
    args: &BTreeMap<String, Value>,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<Outcome<Value>, EngineError> {
    let state = case.into_state();
    let mut handler = CaseFile {
        record: case.clone(),
        known_at: Some(ctx.record_time),
    };
    evaluate(
        module,
        &QueryName::from(query),
        args,
        &state,
        ctx,
        &mut handler,
        case,
    )
}

fn check_key(source: &str, manifest: &SourceManifest) -> CheckKey {
    let mut hasher = blake3::Hasher::new();
    hasher.update(source.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(manifest.snapshot.as_bytes());
    hasher.update(&[0xff]);
    for artifact in &manifest.artifacts {
        hasher.update(artifact.digest.as_bytes());
        hasher.update(&[0xff]);
        hasher.update(artifact.path.as_bytes());
        hasher.update(&[0xff]);
    }
    CheckKey(*hasher.finalize().as_bytes())
}

fn run_key(
    module: &CoreModule,
    query: &str,
    args: &BTreeMap<String, Value>,
    case: &CaseRecord,
    ctx: &RunContext,
) -> Result<RunKey, EngineError> {
    let module_json =
        canonical_json(module).map_err(|err| EngineError::Internal(err.to_string()))?;
    let case_json = canonical_json(case).map_err(|err| EngineError::Internal(err.to_string()))?;
    let args_json = canonical_json(args).map_err(|err| EngineError::Internal(err.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(module_json.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(query.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(args_json.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(case_json.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(ctx.valid_time.to_rfc3339().as_bytes());
    hasher.update(&[0xff]);
    hasher.update(ctx.record_time.to_rfc3339().as_bytes());
    Ok(RunKey(*hasher.finalize().as_bytes()))
}

/// Load the source manifest for `module_path`.
///
/// Resolution:
/// 1. `source_manifest "..."` on the module, relative to the module directory
/// 2. else `sources/manifest.json` next to the module
/// 3. else the unique `*.manifest.json` in `sources/` (trust fixture layout)
///
/// A **declared** path that is missing or malformed is a diagnostic, not
/// [`SourceManifest::default`].
fn load_manifest(module_path: &Path, src: &str) -> Result<SourceManifest, Vec<Diagnostic>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use fidryn_core::{Instant, QueryPlan, Term};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn bool_query(literal: &str) -> String {
        format!(
            r#"
module Examples.T version "0.1.0" {{
    query q() -> Bool {{
        goal Evaluate {{ {literal} }}
    }}
}}
"#
        )
    }

    fn at() -> Instant {
        Instant::parse("2033-01-01T00:00:00Z").expect("timestamp")
    }

    fn ctx() -> RunContext {
        RunContext::new(at(), at())
    }

    fn query_plan(module: &CoreModule) -> &QueryPlan {
        &module.query("q").expect("query q").plan
    }

    fn temp_module_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "fidryn-driver-{}-{}-{tag}",
            std::process::id(),
            nanos
        ));
        fs::create_dir_all(dir.join("sources")).expect("temp sources");
        dir
    }

    #[test]
    fn check_source_identical_string_is_hit() {
        let src = bool_query("true");
        let manifest = SourceManifest::default();
        let mut driver = Driver::new();
        let first = driver.check_source(&src, &manifest).expect("compile");
        assert_eq!(driver.misses(), 1);
        assert_eq!(driver.hits(), 0);
        let second = driver.check_source(&src, &manifest).expect("compile");
        assert_eq!(driver.hits(), 1);
        assert_eq!(driver.misses(), 1);
        assert_eq!(first.id, second.id);
        assert_eq!(query_plan(&first), query_plan(&second));
    }

    #[test]
    fn query_true_to_false_is_miss_with_different_plan() {
        let yes_src = bool_query("true");
        let no_src = bool_query("false");
        let manifest = SourceManifest::default();
        let mut driver = Driver::new();
        let yes = driver.check_source(&yes_src, &manifest).expect("true");
        assert_eq!(driver.misses(), 1);
        let no = driver.check_source(&no_src, &manifest).expect("false");
        assert_eq!(driver.misses(), 2);
        assert_eq!(driver.hits(), 0);
        assert_ne!(query_plan(&yes), query_plan(&no));
        match (query_plan(&yes), query_plan(&no)) {
            (QueryPlan::Evaluate(Term::Bool(true)), QueryPlan::Evaluate(Term::Bool(false))) => {}
            other => panic!("expected Evaluate(true) vs Evaluate(false), got {other:?}"),
        }
    }

    #[test]
    fn run_identical_inputs_hit() {
        let src = bool_query("true");
        let mut driver = Driver::new();
        let module = driver
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        let case = CaseRecord::default();
        let first = driver.run(&module, "q", &case, &ctx()).expect("run");
        let hits_after_miss = driver.hits();
        let misses_after_first_run = driver.misses();
        let second = driver.run(&module, "q", &case, &ctx()).expect("run");
        assert_eq!(driver.hits(), hits_after_miss + 1);
        assert_eq!(driver.misses(), misses_after_first_run);
        assert_eq!(first, second);
        match first {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("expected determinate true, got {other:?}"),
        }
    }

    #[test]
    fn module_body_edit_invalidates_execution_cache() {
        let yes_src = r#"
module Regression version "0.1.0" {
    query q() -> Bool {
        return true
    }
}
"#;
        let no_src = r#"
module Regression version "0.1.0" {
    query q() -> Bool {
        return false
    }
}
"#;
        let mut driver = Driver::new();
        let manifest = SourceManifest::default();
        let yes = driver.check_source(yes_src, &manifest).expect("true");
        let no = driver.check_source(no_src, &manifest).expect("false");
        let case = CaseRecord::default();
        let first = driver.run(&yes, "q", &case, &ctx()).expect("run true");
        let misses = driver.misses();
        let second = driver.run(&no, "q", &case, &ctx()).expect("run false");
        assert_eq!(driver.misses(), misses + 1, "body edit must miss run cache");
        match first {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("expected determinate true, got {other:?}"),
        }
        match second {
            Outcome::Determinate {
                value: Value::Bool(false),
                ..
            } => {}
            other => panic!("expected determinate false, got {other:?}"),
        }
    }

    #[test]
    fn different_query_arguments_miss_the_run_cache() {
        let src = r#"
module Examples.T version "0.1.0" {
    calc echo(x: Int) -> Int { x }
    query q(x: Int) -> Int { return echo(x) }
}
"#;
        let mut driver = Driver::new();
        let module = driver
            .check_source(src, &SourceManifest::default())
            .expect("compile");
        let case = CaseRecord::default();
        let mut a = BTreeMap::new();
        a.insert("x".into(), Value::Int(1));
        let mut b = BTreeMap::new();
        b.insert("x".into(), Value::Int(2));
        let first = driver
            .run_with_args(&module, "q", &case, &ctx(), &a)
            .expect("run a");
        let misses = driver.misses();
        let second = driver
            .run_with_args(&module, "q", &case, &ctx(), &b)
            .expect("run b");
        assert_eq!(driver.misses(), misses + 1, "argument B must miss");
        assert_ne!(first, second);
    }

    #[test]
    fn check_path_identical_bytes_is_hit() {
        let dir = temp_module_dir("path");
        let path = dir.join("m.fr");
        fs::write(&path, bool_query("true")).expect("write module");
        let mut driver = Driver::new();
        let (first, _) = driver.check_path(&path).expect("check");
        assert_eq!(driver.misses(), 1);
        let (second, _) = driver.check_path(&path).expect("check");
        assert_eq!(driver.hits(), 1);
        assert_eq!(first.id, second.id);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn comment_only_edit_misses_check_cache() {
        let manifest = SourceManifest::default();
        let mut driver = Driver::new();
        driver
            .check_source(&bool_query("true"), &manifest)
            .expect("compile");
        let commented = format!("{}\n// trailing comment\n", bool_query("true"));
        driver.check_source(&commented, &manifest).expect("compile");
        assert_eq!(driver.misses(), 2);
        assert_eq!(driver.hits(), 0);
    }

    #[test]
    fn changed_import_digest_misses_check_cache() {
        let src = bool_query("true");
        let artifact = |digest: &str| fidryn_core::ManifestArtifact {
            path: "Other.Law".into(),
            digest: digest.into(),
            kind: "fixture".into(),
            effective: String::new(),
            weight: fidryn_core::SourceWeight::Explanatory,
        };
        let mut first = SourceManifest::default();
        first.artifacts.push(artifact("aaa"));
        let mut second = SourceManifest::default();
        second.artifacts.push(artifact("bbb"));
        let mut driver = Driver::new();
        driver.check_source(&src, &first).expect("compile a");
        assert_eq!(driver.misses(), 1);
        driver.check_source(&src, &second).expect("compile b");
        assert_eq!(driver.misses(), 2);
        assert_eq!(driver.hits(), 0);
        driver.check_source(&src, &first).expect("compile a again");
        assert_eq!(driver.hits(), 1);
    }

    #[test]
    fn same_driver_repeated_check_reports_hit() {
        let src = bool_query("true");
        let manifest = SourceManifest::default();
        let mut driver = Driver::new();
        driver.check_source(&src, &manifest).expect("cold");
        assert_eq!(driver.misses(), 1);
        assert_eq!(driver.hits(), 0);
        driver.check_source(&src, &manifest).expect("repeat");
        assert_eq!(driver.misses(), 1);
        assert_eq!(driver.hits(), 1);
        let edited = bool_query("false");
        driver.check_source(&edited, &manifest).expect("edit");
        assert_eq!(driver.misses(), 2);
        assert_eq!(driver.hits(), 1);
    }
}
