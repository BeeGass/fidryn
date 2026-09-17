//! Incremental compile and evaluate driver.
//!
//! Check and run are salsa tracked functions over interned input structs:
//! source bytes, manifest snapshot, artifact observations, `source_root`
//! display path, query, arguments, case, bitemporal times, and execution
//! mode. A [`VerifiedSourceBundle`] is stored with the compiled module;
//! trust is that bundle's summary, not a reconstruction from digest-looking
//! strings. Fuel exhaustion is not retained as a stable memo. Pasted mill
//! source uses `source_root = None` and never follows artifact paths or
//! `packages/`.

use fidryn_check::check_with_sources;
use fidryn_core::{
    CaseRecord, CoreModule, Diagnostic, DiagnosticCode, EngineError, EvaluationReport,
    ExecutionMode, ExecutionRequest, Outcome, PackageLock, QueryName, RunContext, SourceManifest,
    TrustProfile, Value, VerifiedSourceBundle, canonical_json, is_safe_package_name,
    package_name_from_import, packages_root,
};
use fidryn_eval::{evaluate, evaluate_scenario, report_from_scenario};
use fidryn_handlers::CaseFile;
use fidryn_hir::elaborate;
use fidryn_syntax::ast::{HeaderKind, Item};
use fidryn_syntax::parse_file;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Interned run-key payload hashed by canonical JSON, compared structurally.
#[derive(Clone, PartialEq, Eq)]
struct CanonicalKey<T>(T);

macro_rules! hash_canonical {
    ($t:ty) => {
        impl Hash for CanonicalKey<$t> {
            fn hash<H: Hasher>(&self, state: &mut H) {
                match canonical_json(&self.0) {
                    Ok(json) => json.hash(state),
                    Err(_) => 0u8.hash(state),
                }
            }
        }
    };
}

hash_canonical!(CoreModule);
hash_canonical!(BTreeMap<String, Value>);
hash_canonical!(CaseRecord);

#[salsa::db]
trait DriverJar: salsa::Database {}

#[salsa::db]
#[derive(Clone)]
struct DriverDb {
    storage: salsa::Storage<Self>,
}

#[salsa::db]
impl salsa::Database for DriverDb {}

#[salsa::db]
impl DriverJar for DriverDb {}

/// Observed artifact bytes interned as part of the check key.
///
/// `Unread` is mill / `check_source` (`source_root = None`) and must not
/// follow filesystem paths. `Missing` is a path compile whose file was absent.
#[derive(Clone, PartialEq, Eq, Hash)]
enum ArtifactObservation {
    Unread,
    Missing,
    Bytes(Vec<u8>),
}

#[salsa::interned]
struct CheckInput<'db> {
    #[returns(deref)]
    source: String,
    #[returns(deref)]
    snapshot: String,
    #[returns(as_deref)]
    source_root: Option<String>,
    #[returns(clone)]
    artifacts: Vec<(String, String, ArtifactObservation)>,
    #[returns(deref)]
    manifest_json: String,
}

#[derive(Clone, PartialEq, Eq)]
struct CheckMemo {
    result: Result<CoreModule, Vec<Diagnostic>>,
    bundle: Option<VerifiedSourceBundle>,
}

#[salsa::interned]
struct RunInput<'db> {
    #[returns(clone)]
    module: CanonicalKey<CoreModule>,
    #[returns(deref)]
    query: String,
    #[returns(clone)]
    args: CanonicalKey<BTreeMap<String, Value>>,
    #[returns(clone)]
    case: CanonicalKey<CaseRecord>,
    #[returns(deref)]
    valid_time: String,
    #[returns(deref)]
    record_time: String,
    #[returns(deref)]
    request_identity: String,
    #[returns(copy)]
    scenario: bool,
    #[returns(copy)]
    fuel_attempt: u64,
}

#[salsa::tracked(returns(clone))]
fn compile_checked<'db>(db: &'db dyn DriverJar, input: CheckInput<'db>) -> CheckMemo {
    let source = input.source(db);
    let artifacts = input.artifacts(db);
    let snapshot = input.snapshot(db);
    let manifest = match serde_json::from_str::<SourceManifest>(input.manifest_json(db)) {
        Ok(manifest) => manifest,
        Err(err) => {
            return CheckMemo {
                result: Err(vec![Diagnostic::new(
                    DiagnosticCode::E100,
                    format!("cannot decode source_manifest: {err}"),
                )]),
                bundle: None,
            };
        }
    };
    if snapshot != manifest.snapshot {
        return CheckMemo {
            result: Err(vec![Diagnostic::new(
                DiagnosticCode::E100,
                "interned snapshot does not match source_manifest",
            )]),
            bundle: None,
        };
    }
    let root_buf = input.source_root(db).map(PathBuf::from);
    // Unread observations are mill / check_source: never follow a filesystem root.
    let unread = artifacts
        .iter()
        .any(|(_, _, obs)| matches!(obs, ArtifactObservation::Unread));
    let source_root = if unread { None } else { root_buf.as_deref() };
    let result = compile_with_sources(source, &manifest, source_root);
    let bundle = result.is_ok().then(|| pin_bundle(&manifest, source_root));
    CheckMemo { result, bundle }
}

#[salsa::tracked(returns(clone))]
fn evaluate_query<'db>(
    db: &'db dyn DriverJar,
    input: RunInput<'db>,
) -> Result<Outcome<Value>, EngineError> {
    let module = input.module(db).0;
    let args = input.args(db).0;
    let case = input.case(db).0;
    let valid_time = fidryn_core::Instant::parse(input.valid_time(db))
        .map_err(|err| EngineError::Internal(err.to_string()))?;
    let record_time = fidryn_core::Instant::parse(input.record_time(db))
        .map_err(|err| EngineError::Internal(err.to_string()))?;
    let ctx = RunContext::new(valid_time, record_time);
    let _fuel_attempt = input.fuel_attempt(db);
    if input.scenario(db) {
        evaluate_scenario_run(&module, input.query(db), &args, &case, &ctx)
    } else {
        evaluate_run(&module, input.query(db), &args, &case, &ctx)
    }
}

/// Incremental parse/check/evaluate session.
pub struct Driver {
    db: DriverDb,
    executes: Arc<AtomicU64>,
    source_bundles: HashMap<TrustKey, VerifiedSourceBundle>,
    fuel_attempts: HashMap<[u8; 32], u64>,
    hits: u64,
    misses: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct TrustKey([u8; 32]);

impl Driver {
    /// Empty salsa database and memo counters.
    pub fn new() -> Self {
        let executes = Arc::new(AtomicU64::new(0));
        let event_executes = Arc::clone(&executes);
        Self {
            db: DriverDb {
                storage: salsa::Storage::new(Some(Box::new(move |event| {
                    if matches!(event.kind, salsa::EventKind::WillExecute { .. }) {
                        event_executes.fetch_add(1, Ordering::Relaxed);
                    }
                }))),
            },
            executes,
            source_bundles: HashMap::new(),
            fuel_attempts: HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    fn with_memo<R>(&mut self, op: impl FnOnce(&DriverDb) -> R) -> R {
        let before = self.executes.load(Ordering::Relaxed);
        let result = op(&self.db);
        if self.executes.load(Ordering::Relaxed) > before {
            self.misses += 1;
        } else {
            self.hits += 1;
        }
        result
    }

    /// Combined check and run cache hits.
    pub fn hits(&self) -> u64 {
        self.hits
    }

    /// Combined check and run cache misses.
    pub fn misses(&self) -> u64 {
        self.misses
    }

    /// Parse, elaborate, and check in-memory `source` against `manifest`.
    ///
    /// Artifact files are not read (`source_root` is `None`). The salsa
    /// interned check input is source bytes, the manifest snapshot, unread
    /// artifact sentinels, and no `source_root`. Comment-only edits change
    /// the source bytes and miss. Pasted source never follows artifact
    /// paths or `packages/` on the filesystem.
    pub fn check_source(
        &mut self,
        source: &str,
        manifest: &SourceManifest,
    ) -> Result<CoreModule, Vec<Diagnostic>> {
        self.check_cached(source, manifest, None)
    }

    /// Compile a `.fr` path, loading the declared `source_manifest` or a
    /// `sources/` fallback. A declared path that is missing or malformed is a
    /// diagnostic. Modules that omit a manifest get an empty default.
    ///
    /// After parse/elaborate, checking uses
    /// [`fidryn_check::check_with_sources`] with `source_root = path.parent()`
    /// so hex import digests authenticate against artifact bytes. Imports such
    /// as `Std.Core` may be satisfied by `source_root/packages/<name>` when
    /// the package lock digest matches the module bytes. The interned check
    /// input includes source_root and artifact bytes; tamper or a different
    /// root misses.
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
        let mut manifest = load_manifest(path, &src)?;
        let source_root = path.parent().unwrap_or(Path::new("."));
        extend_manifest_with_packages(&mut manifest, source_root, &src)?;
        let module = self.check_cached(&src, &manifest, Some(source_root))?;
        Ok((module, manifest))
    }

    fn check_cached(
        &mut self,
        source: &str,
        manifest: &SourceManifest,
        source_root: Option<&Path>,
    ) -> Result<CoreModule, Vec<Diagnostic>> {
        let manifest_json = match serde_json::to_string(manifest) {
            Ok(json) => json,
            Err(err) => {
                self.misses += 1;
                return Err(vec![Diagnostic::new(
                    DiagnosticCode::E100,
                    format!("cannot encode source_manifest: {err}"),
                )]);
            }
        };
        let snapshot = manifest.snapshot.clone();
        let source_owned = source.to_owned();
        let root_display = source_root.map(|path| path.display().to_string());
        let artifacts = artifact_observations(manifest, source_root);
        let memo = self.with_memo(|db| {
            let input = CheckInput::new(
                db,
                source_owned.clone(),
                snapshot.clone(),
                root_display.clone(),
                artifacts.clone(),
                manifest_json.clone(),
            );
            compile_checked(db, input)
        });
        if let (Ok(module), Some(bundle)) = (&memo.result, memo.bundle) {
            self.source_bundles
                .entry(trust_key(module))
                .or_insert(bundle);
        }
        memo.result
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
        let request = execution_request(module, query, args, case, ctx, ExecutionMode::Operative)?;
        self.cached_eval(module, query, args, case, ctx, &request)
    }

    /// Evaluate `query` and wrap the outcome as [`EvaluationReport`].
    ///
    /// [`Self::run`] stays operative (assumptions ignored for committed
    /// status; the run cache stores those outcomes). Nonempty
    /// `case.assumptions` selects [`fidryn_eval::evaluate_scenario`] and
    /// `ExecutionMode::Scenario` so a case file cannot silently render as
    /// an unconditional operative result. The caller's `case.events` is
    /// not mutated.
    pub fn run_report(
        &mut self,
        module: &CoreModule,
        query: &str,
        case: &CaseRecord,
        ctx: &RunContext,
    ) -> Result<EvaluationReport<Value>, EngineError> {
        self.run_report_with_args(module, query, case, ctx, &BTreeMap::new())
    }

    pub fn run_report_with_args(
        &mut self,
        module: &CoreModule,
        query: &str,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
    ) -> Result<EvaluationReport<Value>, EngineError> {
        self.finish_run_report(module, query, case, ctx, args, false)
    }

    /// Like [`Self::run_report`], always as a scenario overlay.
    ///
    /// Used by CLI `--scenario`. Nonempty `case.assumptions` already take
    /// this path from [`Self::run_report`].
    pub fn run_report_scenario(
        &mut self,
        module: &CoreModule,
        query: &str,
        case: &CaseRecord,
        ctx: &RunContext,
    ) -> Result<EvaluationReport<Value>, EngineError> {
        self.run_report_scenario_with_args(module, query, case, ctx, &BTreeMap::new())
    }

    pub fn run_report_scenario_with_args(
        &mut self,
        module: &CoreModule,
        query: &str,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
    ) -> Result<EvaluationReport<Value>, EngineError> {
        self.finish_run_report(module, query, case, ctx, args, true)
    }

    fn finish_run_report(
        &mut self,
        module: &CoreModule,
        query: &str,
        case: &CaseRecord,
        ctx: &RunContext,
        args: &BTreeMap<String, Value>,
        scenario: bool,
    ) -> Result<EvaluationReport<Value>, EngineError> {
        let scenario = scenario || !case.assumptions.is_empty();
        let mode = if scenario {
            ExecutionMode::Scenario
        } else {
            ExecutionMode::Operative
        };
        let request = execution_request(module, query, args, case, ctx, mode)?;
        let outcome = self.cached_eval(module, query, args, case, ctx, &request)?;
        let mut report = if scenario {
            report_from_scenario(outcome, case.assumptions.clone())
        } else {
            EvaluationReport::from_outcome(outcome)
        };
        report.trust = self.source_trust_of(module);
        Ok(report)
    }

    fn cached_eval(
        &mut self,
        module: &CoreModule,
        query: &str,
        args: &BTreeMap<String, Value>,
        case: &CaseRecord,
        ctx: &RunContext,
        request: &ExecutionRequest,
    ) -> Result<Outcome<Value>, EngineError> {
        let scenario = request.mode == ExecutionMode::Scenario;
        let identity = run_identity(module, query, args, case, ctx, request)?;
        let fuel_attempt = self.fuel_attempts.get(&identity).copied().unwrap_or(0);
        let module_key = CanonicalKey(module.clone());
        let args_key = CanonicalKey(args.clone());
        let case_key = CanonicalKey(case.clone());
        let query_owned = query.to_owned();
        let valid_time = ctx.valid_time.to_rfc3339();
        let record_time = ctx.record_time.to_rfc3339();
        let request_identity = request.identity().hex();
        let result = self.with_memo(|db| {
            let input = RunInput::new(
                db,
                module_key.clone(),
                query_owned.clone(),
                args_key.clone(),
                case_key.clone(),
                valid_time.clone(),
                record_time.clone(),
                request_identity.clone(),
                scenario,
                fuel_attempt,
            );
            evaluate_query(db, input)
        });
        if matches!(result, Err(EngineError::FuelExhausted { .. })) {
            self.fuel_attempts
                .insert(identity, fuel_attempt.saturating_add(1));
        }
        result
    }

    /// Bundle recorded when this driver compiled `module`.
    pub fn source_bundle_of(&self, module: &CoreModule) -> Option<&VerifiedSourceBundle> {
        self.source_bundles.get(&trust_key(module))
    }

    /// Trust recorded when this driver compiled `module`.
    ///
    /// Prefers the stored [`VerifiedSourceBundle`] summary over reconstructing
    /// trust from digest-looking strings. In-memory compile is unauthenticated
    /// unless an artifact digest is `"fixture"`. Path compile is byte-verified
    /// only when every hex artifact (including injected package modules) was
    /// read and matched. Unknown modules are unauthenticated. `check_source`
    /// never reads `packages/`.
    pub fn source_trust_of(&self, module: &CoreModule) -> TrustProfile {
        self.source_bundle_of(module)
            .map(VerifiedSourceBundle::trust_summary)
            .unwrap_or(TrustProfile::Unauthenticated)
    }
}

impl Default for Driver {
    fn default() -> Self {
        Self::new()
    }
}

fn pin_bundle(manifest: &SourceManifest, source_root: Option<&Path>) -> VerifiedSourceBundle {
    match source_root {
        None => VerifiedSourceBundle::from_manifest(manifest),
        Some(root) => {
            VerifiedSourceBundle::from_observed(manifest, |path| read_artifact_bytes(root, path))
        }
    }
}

fn execution_request(
    module: &CoreModule,
    query: &str,
    args: &BTreeMap<String, Value>,
    case: &CaseRecord,
    ctx: &RunContext,
    mode: ExecutionMode,
) -> Result<ExecutionRequest, EngineError> {
    ExecutionRequest::from_eval(module, query, args, case, ctx, mode).map_err(EngineError::Internal)
}

fn trust_key(module: &CoreModule) -> TrustKey {
    let mut hasher = blake3::Hasher::new();
    hasher.update(module.id.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(module.snapshot.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(module.manifest.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(module.name.as_bytes());
    hasher.update(&[0xff]);
    hasher.update(module.version.as_bytes());
    hasher.update(&[0xff]);
    if let Ok(digest) = module.program_digest() {
        hasher.update(digest.as_bytes());
    } else if let Ok(fingerprint) = module.content_fingerprint() {
        hasher.update(&fingerprint);
    }
    TrustKey(*hasher.finalize().as_bytes())
}

fn compile_with_sources(
    source: &str,
    manifest: &SourceManifest,
    source_root: Option<&Path>,
) -> Result<CoreModule, Vec<Diagnostic>> {
    let parsed = parse_file(source);
    if parsed.has_errors() {
        return Err(parsed.diagnostics);
    }
    let hir = elaborate(&parsed, manifest)?;
    check_with_sources(&hir, manifest, source_root)
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

fn evaluate_scenario_run(
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
    evaluate_scenario(
        module,
        &QueryName::from(query),
        args,
        &state,
        ctx,
        &mut handler,
        case,
    )
}

fn artifact_observations(
    manifest: &SourceManifest,
    source_root: Option<&Path>,
) -> Vec<(String, String, ArtifactObservation)> {
    manifest
        .artifacts
        .iter()
        .map(|artifact| {
            let observation = match source_root {
                None => ArtifactObservation::Unread,
                Some(root) => match read_artifact_bytes(root, &artifact.path) {
                    Some(bytes) => ArtifactObservation::Bytes(bytes),
                    None => ArtifactObservation::Missing,
                },
            };
            (artifact.path.clone(), artifact.digest.clone(), observation)
        })
        .collect()
}

fn read_artifact_bytes(source_root: &Path, artifact_path: &str) -> Option<Vec<u8>> {
    let path = Path::new(artifact_path);
    if path.is_absolute() {
        fs::read(path).ok()
    } else {
        fs::read(source_root.join(path)).ok()
    }
}

fn run_identity(
    module: &CoreModule,
    query: &str,
    args: &BTreeMap<String, Value>,
    case: &CaseRecord,
    ctx: &RunContext,
    request: &ExecutionRequest,
) -> Result<[u8; 32], EngineError> {
    let module_json =
        canonical_json(module).map_err(|err| EngineError::Internal(err.to_string()))?;
    let case_json = canonical_json(case).map_err(|err| EngineError::Internal(err.to_string()))?;
    let args_json = canonical_json(args).map_err(|err| EngineError::Internal(err.to_string()))?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(module_json.as_bytes());
    hasher.update(&[0xff]);
    if let Ok(digest) = module.program_digest() {
        hasher.update(digest.as_bytes());
    } else if let Ok(fingerprint) = module.content_fingerprint() {
        hasher.update(&fingerprint);
    }
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
    hasher.update(&[0xff]);
    hasher.update(request.identity().as_bytes());
    Ok(*hasher.finalize().as_bytes())
}

/// Inject `source_root/packages/<name>` locks for imports in `src`.
///
/// Only [`Driver::check_path`] calls this. [`Driver::check_source`] must not:
/// mill paste has no `source_root` and must not follow server `packages/`.
fn extend_manifest_with_packages(
    manifest: &mut SourceManifest,
    source_root: &Path,
    src: &str,
) -> Result<(), Vec<Diagnostic>> {
    let packages_dir = packages_root(source_root);
    if !packages_dir.is_dir() {
        return Ok(());
    }
    let wanted = imported_package_names(src);
    if wanted.is_empty() {
        return Ok(());
    }
    let mut errors = Vec::new();
    for name in wanted {
        match load_package_artifact(source_root, &name) {
            Ok(Some(artifact)) => {
                if !manifest
                    .artifacts
                    .iter()
                    .any(|existing| existing.path == artifact.path)
                {
                    manifest.artifacts.push(artifact);
                }
            }
            Ok(None) => {}
            Err(ds) => errors.extend(ds),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn imported_package_names(src: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let parsed = parse_file(src);
    let Some(module) = parsed.module() else {
        return names;
    };
    for item in &module.items {
        if let Item::Import(decl) = item {
            let raw = decl.name.as_deref().unwrap_or("");
            let name = package_name_from_import(raw);
            if is_safe_package_name(&name) {
                names.insert(name);
            }
        }
    }
    names
}

fn load_package_artifact(
    source_root: &Path,
    name: &str,
) -> Result<Option<fidryn_core::ManifestArtifact>, Vec<Diagnostic>> {
    if !is_safe_package_name(name) {
        return Ok(None);
    }
    let dir = packages_root(source_root).join(name);
    if !dir.is_dir() {
        return Ok(None);
    }
    let lock_path = dir.join("manifest.json");
    let text = match fs::read_to_string(&lock_path) {
        Ok(text) => text,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(vec![Diagnostic::new(
                DiagnosticCode::E540,
                format!("package `{name}` is missing packages/{name}/manifest.json"),
            )]);
        }
        Err(err) => {
            return Err(vec![Diagnostic::new(
                DiagnosticCode::E540,
                format!("cannot read package `{name}` lock: {err}"),
            )]);
        }
    };
    let lock: PackageLock = serde_json::from_str(&text).map_err(|err| {
        vec![Diagnostic::new(
            DiagnosticCode::E100,
            format!("malformed package lock {}: {err}", lock_path.display()),
        )]
    })?;
    if !lock.name.is_empty() && !lock.name.eq_ignore_ascii_case(name) {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::E200,
            format!(
                "package directory `{name}` does not match lock name `{}`",
                lock.name
            ),
        )]);
    }
    let Some(module_path) = unique_package_module(&dir) else {
        return Err(vec![Diagnostic::new(
            DiagnosticCode::E200,
            format!("package `{name}` does not contain a unique .fr module"),
        )]);
    };
    let file_name = module_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            vec![Diagnostic::new(
                DiagnosticCode::E200,
                format!("package `{name}` module path is not utf-8"),
            )]
        })?;
    let rel = format!("packages/{name}/{file_name}");
    Ok(Some(lock.module_artifact(rel)))
}

fn unique_package_module(dir: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("fr") {
            found.push(path);
        }
    }
    if found.len() == 1 { found.pop() } else { None }
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
    use fidryn_core::{Assumption, ExecutionMode, Instant, QueryPlan, Term, TrustProfile};
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
        assert_ne!(first, second);
    }

    #[test]
    fn function_body_edit_invalidates_execution_cache() {
        let one_src = r#"
module Regression version "0.1.0" {
    calc answer() -> Int { return 1 }
    query q() -> Int { return answer() }
}
"#;
        let two_src = r#"
module Regression version "0.1.0" {
    calc answer() -> Int { return 2 }
    query q() -> Int { return answer() }
}
"#;
        let mut driver = Driver::new();
        let manifest = SourceManifest::default();
        let one = driver.check_source(one_src, &manifest).expect("one");
        let two = driver.check_source(two_src, &manifest).expect("two");
        assert_ne!(
            one.content_fingerprint().expect("fp one"),
            two.content_fingerprint().expect("fp two"),
            "function body must change the content fingerprint"
        );
        let case = CaseRecord::default();
        let first = driver.run(&one, "q", &case, &ctx()).expect("run 1");
        let misses = driver.misses();
        let second = driver.run(&two, "q", &case, &ctx()).expect("run 2");
        assert_eq!(
            driver.misses(),
            misses + 1,
            "function body edit must miss run cache"
        );
        match first {
            Outcome::Determinate {
                value: Value::Int(1),
                ..
            } => {}
            other => panic!("expected determinate 1, got {other:?}"),
        }
        match second {
            Outcome::Determinate {
                value: Value::Int(2),
                ..
            } => {}
            other => panic!("expected determinate 2, got {other:?}"),
        }
        assert_ne!(first, second);
    }

    #[test]
    fn run_report_matches_cached_run_outcome() {
        let src = bool_query("true");
        let mut driver = Driver::new();
        let module = driver
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        let case = CaseRecord::default();
        let outcome = driver.run(&module, "q", &case, &ctx()).expect("run");
        let report = driver
            .run_report(&module, "q", &case, &ctx())
            .expect("report");
        assert_eq!(report.outcome, outcome);
        assert_eq!(report.trust, TrustProfile::Unauthenticated);
        assert_eq!(report.execution_mode, ExecutionMode::Operative);
        assert!(report.assumptions.is_empty());
        match report.outcome {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("expected determinate true, got {other:?}"),
        }
    }

    #[test]
    fn run_stays_operative_when_case_has_assumptions() {
        let src = bool_query("true");
        let mut driver = Driver::new();
        let module = driver
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        let mut case = CaseRecord::default();
        case.assumptions.push(Assumption {
            id: "hyp-1".into(),
            payload: Value::Bool(true),
        });
        let events_before = case.events.clone();
        let outcome = driver.run(&module, "q", &case, &ctx()).expect("run");
        assert_eq!(case.events, events_before);
        match outcome {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("operative run must stay determinate true, got {other:?}"),
        }
    }

    #[test]
    fn run_report_with_assumptions_is_scenario_and_preserves_events() {
        let src = bool_query("true");
        let mut driver = Driver::new();
        let module = driver
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        let mut case = CaseRecord::default();
        case.assumptions.push(Assumption {
            id: "hyp-1".into(),
            payload: Value::Bool(true),
        });
        let events_before = case.events.clone();
        let report = driver
            .run_report(&module, "q", &case, &ctx())
            .expect("report");
        assert_eq!(report.execution_mode, ExecutionMode::Scenario);
        assert_eq!(report.assumptions, case.assumptions);
        assert_eq!(report.trust, TrustProfile::Unauthenticated);
        assert_eq!(case.events, events_before);
        match report.outcome {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("expected determinate true, got {other:?}"),
        }
    }

    #[test]
    fn run_report_scenario_flag_is_scenario_with_empty_assumptions() {
        let src = bool_query("true");
        let mut driver = Driver::new();
        let module = driver
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        let case = CaseRecord::default();
        let report = driver
            .run_report_scenario(&module, "q", &case, &ctx())
            .expect("report");
        assert_eq!(report.execution_mode, ExecutionMode::Scenario);
        assert!(report.assumptions.is_empty());
        assert_eq!(report.trust, TrustProfile::Unauthenticated);
    }

    #[test]
    fn check_source_trust_remains_unauthenticated() {
        let src = bool_query("true");
        let mut driver = Driver::new();
        let module = driver
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        assert_eq!(
            driver.source_trust_of(&module),
            TrustProfile::Unauthenticated
        );
    }

    #[test]
    fn run_report_fixture_digest_is_fixture_trust() {
        let src = r#"
module Examples.T version "0.1.0" {
    import Other.Law version "1" { digest "fixture" }
    query q() -> Bool {
        goal Evaluate { true }
    }
}
"#;
        let manifest = SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "Other.Law".into(),
                digest: "fixture".into(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        let mut driver = Driver::new();
        let module = driver.check_source(src, &manifest).expect("compile");
        let report = driver
            .run_report(&module, "q", &CaseRecord::default(), &ctx())
            .expect("report");
        assert_eq!(report.trust, TrustProfile::Fixture);
        assert_eq!(driver.source_trust_of(&module), TrustProfile::Fixture);
    }

    #[test]
    fn run_report_path_hex_digest_is_byte_verified() {
        let dir = temp_module_dir("report-bytes");
        let bytes = b"fidryn-source-bytes";
        let digest = blake3::hash(bytes).to_hex().to_string();
        fs::write(dir.join("Other.Law"), bytes).expect("write artifact");
        let manifest = SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "Other.Law".into(),
                digest: digest.clone(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        fs::write(
            dir.join("sources").join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");
        let src = format!(
            r#"
module Examples.ImpBytes version "0.1.0" {{
    import Other.Law version "1" {{ digest "{digest}" }}
    query ok() -> Bool {{
        goal Evaluate {{ true }}
    }}
}}
"#
        );
        let path = dir.join("m.fr");
        fs::write(&path, &src).expect("write module");
        let mut driver = Driver::new();
        let (module, _) = driver.check_path(&path).expect("check");
        let report = driver
            .run_report(&module, "ok", &CaseRecord::default(), &ctx())
            .expect("report");
        assert_eq!(report.trust, TrustProfile::ByteVerified);
        let memory = Driver::new()
            .check_source(&src, &manifest)
            .expect_err("in-memory hex is not byte-verified");
        assert!(
            memory.iter().any(|d| d.code == DiagnosticCode::E200),
            "{memory:?}"
        );
        let _ = fs::remove_dir_all(&dir);
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
    fn check_path_matching_blake3_authenticates_and_tamper_is_e200() {
        let dir = temp_module_dir("bytes");
        let bytes = b"fidryn-source-bytes";
        let digest = blake3::hash(bytes).to_hex().to_string();
        fs::write(dir.join("Other.Law"), bytes).expect("write artifact");
        let manifest = fidryn_core::SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "Other.Law".into(),
                digest: digest.clone(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        fs::write(
            dir.join("sources").join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");
        let src = format!(
            r#"
module Examples.ImpBytes version "0.1.0" {{
    import Other.Law version "1" {{ digest "{digest}" }}
    query ok() -> Bool {{
        goal Evaluate {{ true }}
    }}
}}
"#
        );
        let path = dir.join("m.fr");
        fs::write(&path, &src).expect("write module");

        Driver::new()
            .check_path(&path)
            .expect("matching blake3 hex authenticates");

        let memory_err = Driver::new()
            .check_source(&src, &manifest)
            .expect_err("in-memory check does not read artifact bytes");
        assert!(
            memory_err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{memory_err:?}"
        );

        fs::write(dir.join("Other.Law"), b"tampered-bytes").expect("tamper artifact");
        let err = Driver::new()
            .check_path(&path)
            .expect_err("tampered artifact must fail E200");
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_driver_artifact_tamper_misses_check_cache() {
        let dir = temp_module_dir("bytes-same-driver");
        let bytes = b"fidryn-source-bytes";
        let digest = blake3::hash(bytes).to_hex().to_string();
        fs::write(dir.join("Other.Law"), bytes).expect("write artifact");
        let manifest = fidryn_core::SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "Other.Law".into(),
                digest: digest.clone(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        fs::write(
            dir.join("sources").join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");
        let src = format!(
            r#"
module Examples.ImpBytes version "0.1.0" {{
    import Other.Law version "1" {{ digest "{digest}" }}
    query ok() -> Bool {{
        goal Evaluate {{ true }}
    }}
}}
"#
        );
        let path = dir.join("m.fr");
        fs::write(&path, &src).expect("write module");
        let mut driver = Driver::new();
        driver
            .check_path(&path)
            .expect("matching blake3 hex authenticates");
        assert_eq!(driver.misses(), 1);
        fs::write(dir.join("Other.Law"), b"tampered-bytes").expect("tamper artifact");
        let err = driver
            .check_path(&path)
            .expect_err("tampered bytes must miss the check cache");
        assert_eq!(driver.misses(), 2);
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_hex_manifest_entry_without_checked_bytes_is_not_byte_verified() {
        let dir = temp_module_dir("dangling-hex");
        let source = r#"module Review version "0.1.0" {
        source_manifest "sources/manifest.json"
        query q() -> Bool { return true }
    }"#;
        let path = dir.join("main.fr");
        fs::write(&path, source).expect("source");
        let digest = "ab".repeat(32);
        let manifest = serde_json::json!({
            "schema": "fidryn.source-manifest/v0.1",
            "snapshot": "review-snapshot",
            "jurisdiction": "Test",
            "artifacts": [{
                "path": "never-created.txt",
                "digest": digest,
                "kind": "text",
                "effective": "2033-01-01",
                "weight": "explanatory"
            }],
        });
        fs::write(
            dir.join("sources").join("manifest.json"),
            manifest.to_string(),
        )
        .expect("manifest");
        let mut driver = Driver::new();
        let (module, _) = driver
            .check_path(&path)
            .expect("a dangling hex artifact is not a digest-required import");
        assert_ne!(
            driver.source_trust_of(&module),
            TrustProfile::ByteVerified,
            "no bytes were available to authenticate this artifact"
        );
        assert_eq!(
            driver.source_trust_of(&module),
            TrustProfile::Unauthenticated
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_source_hex_manifest_without_bytes_is_not_byte_verified() {
        let src = bool_query("true");
        let manifest = SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: "review-snapshot".into(),
            jurisdiction: "Test".into(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "never-created.txt".into(),
                digest: "ab".repeat(32),
                kind: "text".into(),
                effective: "2033-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        let mut driver = Driver::new();
        let module = driver
            .check_source(&src, &manifest)
            .expect("in-memory compile does not require unread hex artifacts");
        assert_ne!(driver.source_trust_of(&module), TrustProfile::ByteVerified);
        assert_eq!(
            driver.source_trust_of(&module),
            TrustProfile::Unauthenticated
        );
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
    fn fidryn_driver_depends_on_salsa_and_repeated_check_source_hits() {
        let cargo_toml = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        assert!(
            cargo_toml.contains("salsa"),
            "fidryn-driver must depend on the salsa crate"
        );
        let src = bool_query("true");
        let manifest = SourceManifest::default();
        let mut driver = Driver::new();
        driver.check_source(&src, &manifest).expect("cold");
        assert_eq!(driver.misses(), 1);
        assert_eq!(driver.hits(), 0);
        driver.check_source(&src, &manifest).expect("warm");
        assert_eq!(driver.hits(), 1);
        assert_eq!(driver.misses(), 1);
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

    #[test]
    fn check_source_does_not_follow_artifact_paths() {
        let dir = temp_module_dir("mill-no-path");
        let bytes = b"secret-bytes-must-not-be-read";
        let digest = blake3::hash(bytes).to_hex().to_string();
        let artifact_path = dir.join("secret.txt");
        fs::write(&artifact_path, bytes).expect("write secret");
        let manifest = SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: "mill".into(),
            jurisdiction: "Test".into(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: artifact_path.to_string_lossy().into_owned(),
                digest,
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        let mut driver = Driver::new();
        let module = driver
            .check_source(&bool_query("true"), &manifest)
            .expect("in-memory compile does not require unread hex");
        assert_eq!(
            driver.source_trust_of(&module),
            TrustProfile::Unauthenticated
        );
        let bundle = driver.source_bundle_of(&module).expect("bundle");
        assert!(!bundle.artifacts[0].ok);
        assert!(bundle.artifacts[0].observed_digest.is_none());
        assert_ne!(bundle.trust_summary(), TrustProfile::ByteVerified);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_path_bundle_identity_is_byte_verified_when_hex_matches() {
        let dir = temp_module_dir("bundle-bytes");
        let bytes = b"fidryn-source-bytes";
        let digest = blake3::hash(bytes).to_hex().to_string();
        fs::write(dir.join("Other.Law"), bytes).expect("write artifact");
        let manifest = SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "Other.Law".into(),
                digest: digest.clone(),
                kind: "text".into(),
                effective: "2026-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        fs::write(
            dir.join("sources").join("manifest.json"),
            serde_json::to_string(&manifest).expect("manifest json"),
        )
        .expect("write manifest");
        let src = format!(
            r#"
module Examples.ImpBytes version "0.1.0" {{
    import Other.Law version "1" {{ digest "{digest}" }}
    query ok() -> Bool {{
        goal Evaluate {{ true }}
    }}
}}
"#
        );
        let path = dir.join("m.fr");
        fs::write(&path, &src).expect("write module");
        let mut driver = Driver::new();
        let (module, _) = driver.check_path(&path).expect("check");
        let bundle = driver.source_bundle_of(&module).expect("bundle");
        assert!(bundle.artifacts[0].ok);
        assert_eq!(
            bundle.artifacts[0].observed_digest.as_deref(),
            Some(digest.as_str())
        );
        assert_eq!(bundle.trust_summary(), TrustProfile::ByteVerified);
        assert_eq!(driver.source_trust_of(&module), TrustProfile::ByteVerified);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn dangling_hex_bundle_is_unauthenticated() {
        let src = bool_query("true");
        let manifest = SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: "review-snapshot".into(),
            jurisdiction: "Test".into(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "never-created.txt".into(),
                digest: "ab".repeat(32),
                kind: "text".into(),
                effective: "2033-01-01".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        let mut driver = Driver::new();
        let module = driver.check_source(&src, &manifest).expect("compile");
        let bundle = driver.source_bundle_of(&module).expect("bundle");
        assert!(!bundle.artifacts[0].ok);
        assert_eq!(bundle.trust_summary(), TrustProfile::Unauthenticated);
        assert_eq!(driver.source_trust_of(&module), bundle.trust_summary());
    }

    #[test]
    fn run_report_warm_and_cold_agree() {
        let src = bool_query("true");
        let case = CaseRecord::default();
        let mut cold = Driver::new();
        let module = cold
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        let cold_report = cold.run_report(&module, "q", &case, &ctx()).expect("cold");
        let mut warm = Driver::new();
        let warm_module = warm
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        let first = warm
            .run_report(&warm_module, "q", &case, &ctx())
            .expect("warm miss");
        let hits = warm.hits();
        let second = warm
            .run_report(&warm_module, "q", &case, &ctx())
            .expect("warm hit");
        assert_eq!(warm.hits(), hits + 1);
        assert_eq!(cold_report.outcome, first.outcome);
        assert_eq!(first, second);
        assert_eq!(first.execution_mode, ExecutionMode::Operative);
    }

    #[test]
    fn scenario_request_misses_operative_run_cache() {
        let src = bool_query("true");
        let mut driver = Driver::new();
        let module = driver
            .check_source(&src, &SourceManifest::default())
            .expect("compile");
        let case = CaseRecord::default();
        driver.run(&module, "q", &case, &ctx()).expect("operative");
        let misses = driver.misses();
        driver
            .run_report_scenario(&module, "q", &case, &ctx())
            .expect("scenario");
        assert_eq!(
            driver.misses(),
            misses + 1,
            "scenario ExecutionRequest identity must miss the operative slot"
        );
        driver
            .run_report_scenario(&module, "q", &case, &ctx())
            .expect("scenario hit");
        assert_eq!(driver.misses(), misses + 1);
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn std_core_import_src() -> &'static str {
        r#"
module Examples.UseStd version "0.1.0" {
    import Std.Core version "0.1.0"
    query ok() -> Bool {
        goal Evaluate { true }
    }
}
"#
    }

    fn std_core_call_src() -> &'static str {
        r#"
module Examples.UseStd version "0.1.0" {
    import Std.Core version "0.1.0"
    query q() -> Bool { return always_true() }
}
"#
    }

    fn has_core_function(module: &CoreModule, name: &str) -> bool {
        module.declarations.iter().any(
            |d| matches!(d, fidryn_core::CoreDecl::Function(function) if function.name == name),
        )
    }

    fn copy_workspace_packages(dest_root: &Path) {
        for name in ["logic", "std"] {
            let src = workspace_root().join("packages").join(name);
            let dest = dest_root.join("packages").join(name);
            fs::create_dir_all(&dest).expect("package dir");
            for entry in fs::read_dir(&src).expect("read package") {
                let entry = entry.expect("entry");
                let path = entry.path();
                if path.is_file() {
                    fs::copy(&path, dest.join(entry.file_name())).expect("copy package file");
                }
            }
        }
    }

    #[test]
    fn check_path_matching_package_digest_authenticates() {
        let dir = temp_module_dir("pkg-ok");
        copy_workspace_packages(&dir);
        let path = dir.join("m.fr");
        fs::write(&path, std_core_import_src()).expect("write module");
        let mut driver = Driver::new();
        let (module, manifest) = driver.check_path(&path).expect("matching package digest");
        assert!(
            manifest
                .artifacts
                .iter()
                .any(|a| a.path == "packages/std/core.fr"),
            "{manifest:?}"
        );
        assert_eq!(driver.source_trust_of(&module), TrustProfile::ByteVerified);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_path_mismatched_package_digest_is_e200() {
        let dir = temp_module_dir("pkg-bad");
        copy_workspace_packages(&dir);
        let lock_path = dir.join("packages").join("std").join("manifest.json");
        let mut lock: PackageLock =
            serde_json::from_str(&fs::read_to_string(&lock_path).expect("lock"))
                .expect("parse lock");
        lock.digest = blake3::hash(b"tampered-package-bytes").to_hex().to_string();
        fs::write(&lock_path, serde_json::to_string(&lock).expect("lock json"))
            .expect("write lock");
        let path = dir.join("m.fr");
        fs::write(&path, std_core_import_src()).expect("write module");
        let err = Driver::new()
            .check_path(&path)
            .expect_err("mismatched package digest is E200");
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_path_links_std_core_always_true() {
        let dir = temp_module_dir("pkg-link");
        copy_workspace_packages(&dir);
        let path = dir.join("m.fr");
        fs::write(&path, std_core_call_src()).expect("write module");
        let mut driver = Driver::new();
        let (module, _) = driver
            .check_path(&path)
            .expect("path compile links Std.Core");
        assert!(
            has_core_function(&module, "always_true"),
            "linked CoreModule must contain always_true: {module:?}"
        );
        let outcome = driver
            .run(&module, "q", &CaseRecord::default(), &ctx())
            .expect("run linked always_true");
        match outcome {
            Outcome::Determinate {
                value: Value::Bool(true),
                ..
            } => {}
            other => panic!("expected determinate true, got {other:?}"),
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_source_does_not_read_packages_from_filesystem() {
        let dir = temp_module_dir("pkg-trap");
        copy_workspace_packages(&dir);
        let src = std_core_call_src();
        let mut driver = Driver::new();
        let module = driver
            .check_source(src, &SourceManifest::default())
            .expect("digest-free import without a bundle is not path compile");
        assert_ne!(driver.source_trust_of(&module), TrustProfile::ByteVerified);
        assert_eq!(
            driver.source_trust_of(&module),
            TrustProfile::Unauthenticated
        );
        assert!(
            !has_core_function(&module, "always_true"),
            "check_source must not mill packages/ from cwd: {module:?}"
        );

        let bytes = fs::read(dir.join("packages").join("std").join("core.fr")).expect("bytes");
        let digest = blake3::hash(&bytes).to_hex().to_string();
        let bundle = SourceManifest {
            schema: "fidryn.source-manifest/v0.1".into(),
            snapshot: String::new(),
            jurisdiction: String::new(),
            artifacts: vec![fidryn_core::ManifestArtifact {
                path: "packages/std/core.fr".into(),
                digest,
                kind: "package_module".into(),
                effective: "0.1.0".into(),
                weight: fidryn_core::SourceWeight::Explanatory,
            }],
        };
        let err = Driver::new()
            .check_source(src, &bundle)
            .expect_err("in-memory compile without observed bytes is unresolved");
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn check_path_missing_nested_digest_is_e200() {
        let dir = temp_module_dir("pkg-nested-missing");
        copy_workspace_packages(&dir);
        let lock_path = dir.join("packages").join("logic").join("manifest.json");
        fs::remove_file(&lock_path).expect("drop nested lock");
        let path = dir.join("m.fr");
        fs::write(&path, std_core_call_src()).expect("write module");
        let err = Driver::new()
            .check_path(&path)
            .expect_err("missing nested digest is E200");
        assert!(
            err.iter().any(|d| d.code == DiagnosticCode::E200),
            "{err:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn repo_package_locks_match_module_bytes() {
        for (name, file) in [("std", "core.fr"), ("logic", "true.fr")] {
            let pkg = workspace_root().join("packages").join(name);
            let bytes = fs::read(pkg.join(file)).expect(file);
            let lock: PackageLock =
                serde_json::from_str(&fs::read_to_string(pkg.join("manifest.json")).expect("lock"))
                    .expect("parse lock");
            assert_eq!(lock.name, name);
            assert_eq!(lock.version, "0.1.0");
            assert_eq!(lock.digest, blake3::hash(&bytes).to_hex().to_string());
            assert_eq!(lock.schema, fidryn_core::PACKAGE_LOCK_SCHEMA);
        }
    }
}
