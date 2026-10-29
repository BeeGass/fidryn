//! Portable review inputs and a freshly checked evaluation.

use fidryn_core::{CaseRecord, RunContext, canonical_json};
use fidryn_driver::Driver;
use fidryn_syntax::ast::{Decl, HeaderKind, Item};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path};

const MAX_CASE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ARCHIVE_INPUT_BYTES: usize = 64 * 1024 * 1024;

/// Inputs to a fresh review export. Templates are retained as reviewer
/// materials and never interpreted as automatic validity instructions.
pub struct ReviewRequest<'a> {
    pub path: &'a Path,
    pub query: &'a str,
    pub case_path: &'a Path,
    pub context: &'a RunContext,
    pub scenario: bool,
    pub source_root: Option<&'a Path>,
    pub template: Option<&'a Path>,
    pub budget: Option<u32>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReviewFile {
    path: String,
    roles: Vec<String>,
    availability: &'static str,
    encoding: &'static str,
    byte_length: Option<usize>,
    blake3: Option<String>,
    bytes_hex: Option<String>,
}

/// Build an in-memory JSON archive from admitted source observations and
/// the exact case bytes evaluated in this call. No archive is extracted.
pub fn build_review(request: &ReviewRequest<'_>) -> Result<String, String> {
    let source_root = request.source_root.unwrap_or_else(|| {
        request
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    });
    let source_root = source_root
        .canonicalize()
        .map_err(|error| format!("cannot resolve review source root: {error}"))?;
    let module_parent = request
        .path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let module_path = module_parent
        .canonicalize()
        .map_err(|error| format!("cannot resolve review module directory: {error}"))?
        .join(
            request
                .path
                .file_name()
                .ok_or("review module path has no filename")?,
        );
    let module_relative = module_path
        .strip_prefix(&source_root)
        .map_err(|_| "review module is outside its admitted source root")?;
    let module_relative = portable_path(module_relative)?;
    let program_path = format!("program/{module_relative}");
    let case_path = "case/case-record.json";
    let mut driver = Driver::new();
    let (artifact, _) = driver
        .check_path_artifact_with_root(request.path, &source_root)
        .map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        })?;
    let case_bytes = read_bounded(request.case_path, MAX_CASE_BYTES)?;
    let case: CaseRecord = serde_json::from_slice(&case_bytes)
        .map_err(|error| format!("invalid review case: {error}"))?;
    let checked = driver
        .run_artifact_checked_with_budget(
            &artifact,
            request.query,
            &case,
            request.context,
            request.scenario,
            request.budget,
        )
        .map_err(|error| error.to_string())?;
    let execution = checked
        .report()
        .request
        .as_ref()
        .ok_or("checked review has no invocation")?;
    let rendered = fidryn_trace::render_checked_report(
        artifact.module(),
        &execution.query,
        request.context.valid_time,
        request.context.record_time,
        &case,
        &checked,
    )?;
    if rendered.len() > MAX_ARCHIVE_INPUT_BYTES {
        return Err("review report exceeds the 64 MiB serialization budget".into());
    }
    let steps = driver
        .derivation_of(execution)
        .ok_or("review evaluation did not retain its derivation")?;
    let traced = crate::attach_derivation(&rendered, steps)?;
    if traced.len() > MAX_ARCHIVE_INPUT_BYTES {
        return Err("review trace exceeds the 64 MiB serialization budget".into());
    }
    let report: Value = serde_json::from_str(&traced).map_err(|error| error.to_string())?;
    let mut files = BTreeMap::new();
    let mut total_bytes = 0usize;
    insert_file(
        &mut files,
        &mut total_bytes,
        program_path.clone(),
        "root_source",
        Some(artifact.source().as_bytes()),
    )?;
    insert_file(
        &mut files,
        &mut total_bytes,
        case_path.into(),
        "case_record",
        Some(&case_bytes),
    )?;
    for (path, bytes) in artifact.source_inputs() {
        let path = format!("program/{}", portable_path(portable_reference(path)?)?);
        insert_file(
            &mut files,
            &mut total_bytes,
            path,
            "captured_source_input",
            bytes.as_deref(),
        )?;
    }
    let parsed = fidryn_syntax::parse_file(artifact.source());
    let syntax = parsed
        .module()
        .ok_or("admitted review source has no module syntax")?;
    let module_dir = Path::new(&module_relative)
        .parent()
        .unwrap_or(Path::new(""));
    let declared_manifest = syntax.items.iter().find_map(|item| match item {
        Item::Header(header) if header.kind == HeaderKind::SourceManifest => {
            Some(header.value.as_str())
        }
        _ => None,
    });
    let manifest_relative = if let Some(raw) = declared_manifest {
        let path = if raw.trim().starts_with('"') {
            fidryn_syntax::decode_string_literal(raw.trim())
                .map_err(|error| format!("cannot preserve manifest reference: {error}"))?
        } else {
            raw.trim().to_owned()
        };
        portable_path(&module_dir.join(portable_reference(&path)?))?
    } else {
        portable_path(&module_dir.join("sources/manifest.json"))?
    };
    let manifest_path = format!("program/{manifest_relative}");
    let normalized_manifest =
        canonical_json(artifact.manifest()).map_err(|error| error.to_string())?;
    insert_file(
        &mut files,
        &mut total_bytes,
        manifest_path.clone(),
        "normalized_manifest",
        Some(normalized_manifest.as_bytes()),
    )?;
    let mut source_links = Vec::new();
    collect_source_links(&mut source_links, &program_path, artifact.source())?;
    for (path, bytes) in artifact.source_inputs() {
        if path.ends_with(".fr")
            && let Some(bytes) = bytes
            && let Ok(source) = std::str::from_utf8(bytes)
        {
            collect_source_links(
                &mut source_links,
                &format!("program/{}", portable_path(portable_reference(path)?)?),
                source,
            )?;
        }
    }
    let template_path = if let Some(path) = request.template {
        let bytes = read_bounded(path, MAX_CASE_BYTES)?;
        let archive_path = "review/template";
        insert_file(
            &mut files,
            &mut total_bytes,
            archive_path.into(),
            "reviewer_template",
            Some(&bytes),
        )?;
        Some(archive_path)
    } else {
        None
    };
    let mut command = vec![
        "fidryn".to_owned(),
        "run".into(),
        program_path.clone(),
        "--query".into(),
        request.query.into(),
        "--case".into(),
        case_path.into(),
        "--valid-at".into(),
        request.context.valid_time.to_rfc3339(),
        "--known-at".into(),
        request.context.record_time.to_rfc3339(),
        "--source-root".into(),
        "program".into(),
    ];
    if request.scenario {
        command.push("--scenario".into());
    }
    if let Some(budget) = request.budget {
        command.extend(["--budget".into(), budget.to_string()]);
    }
    let engine = crate::engine::executable_identity()?;
    let mut archive = json!({
        "schema":"fidryn.review-bundle/v0.1",
        "digestAlgorithm":"blake3",
        "identityConvention":"BLAKE3 of canonical JSON excluding archiveDigest",
        "invocation":execution,
        "invocationId":execution.identity().hex(),
        "engineArtifact":engine,
        "sourceBundle":{"identity":artifact.sources().identity().hex(),"observations":artifact.sources()},
        "files":files.into_values().collect::<Vec<_>>(),
        "normalizedManifest":{"path":manifest_path,"representation":"normalized semantic manifest; original formatting was not retained by compilation","value":artifact.manifest()},
        "sourceLinks":source_links,
        "report":report,
        "qualification":{
            "automaticChecks":"The sources compiled and the case JSON passed input validation. The reported outcome, including any legal refusal, was replayed against this invocation; actual evaluation derivations are retained.",
            "sourceTrust":checked.report().trust,
            "verificationMethod":checked.report().verification_method,
            "reviewStatus":"not_assessed",
            "reviewerTemplate":template_path,
            "legalAssessment":"Reviewer assessment is separate from interpreter execution and source authentication.",
            "modelBoundary":"The report retains declared exclusions, admissible completions, unresolved issues, scenario hypotheses and source trust. Unsupported semantics and resource exhaustion do not establish automatic validity."
        },
        "rerun":{
            "modulePath":program_path,"casePath":case_path,"sourceRoot":"program",
            "engineVersion":format!("fidryn/{};semantics=0.2",env!("CARGO_PKG_VERSION")),
            "engineArtifact":engine,
            "arguments":command,
            "instructions":"Restore present files at their relative archive paths, keep missing observations absent, and run the argument vector from the restored archive root. Compare the available executable fingerprint with engineArtifact; the archive identifies that binary but does not distribute it."
        }
    });
    let body = canonical_json(&archive).map_err(|error| error.to_string())?;
    archive["archiveDigest"] = Value::String(blake3::hash(body.as_bytes()).to_hex().to_string());
    canonical_json(&archive).map_err(|error| error.to_string())
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > limit {
        return Err(format!(
            "review input {} exceeds {limit} bytes",
            path.display()
        ));
    }
    Ok(bytes)
}

fn insert_file(
    files: &mut BTreeMap<String, ReviewFile>,
    total: &mut usize,
    path: String,
    role: &str,
    bytes: Option<&[u8]>,
) -> Result<(), String> {
    let digest = bytes.map(|bytes| blake3::hash(bytes).to_hex().to_string());
    if let Some(previous) = files.get_mut(&path) {
        if previous.blake3 != digest || previous.byte_length != bytes.map(<[u8]>::len) {
            return Err(format!("portable review inputs conflict at `{path}`"));
        }
        previous.roles.push(role.into());
        return Ok(());
    }
    *total = total
        .checked_add(bytes.map_or(0, <[u8]>::len))
        .ok_or("review input size overflow")?;
    if *total > MAX_ARCHIVE_INPUT_BYTES {
        return Err("review inputs exceed the 64 MiB archive budget".into());
    }
    files.insert(
        path.clone(),
        ReviewFile {
            path,
            roles: vec![role.into()],
            availability: if bytes.is_some() {
                "present"
            } else {
                "missing"
            },
            encoding: "hex",
            byte_length: bytes.map(<[u8]>::len),
            blake3: digest,
            bytes_hex: bytes.map(hex),
        },
    );
    Ok(())
}

fn portable_reference(path: &str) -> Result<&Path, String> {
    if path.contains(['\\', ':']) {
        return Err("review input paths cannot contain platform-dependent separators".into());
    }
    Ok(Path::new(path))
}

fn portable_path(path: &Path) -> Result<String, String> {
    let mut clean = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                let value = value.to_str().ok_or("review paths must be UTF-8")?;
                if value.contains(['\\', ':']) {
                    return Err(
                        "review input paths cannot contain platform-dependent separators".into(),
                    );
                }
                clean.push(value);
            }
            Component::CurDir => {}
            Component::ParentDir if clean.pop().is_some() => {}
            _ => {
                return Err(format!(
                    "source reference `{}` is not portable within the review root",
                    path.display()
                ));
            }
        }
    }
    let text = clean.join("/");
    if text.is_empty() {
        return Err("review input path is empty".into());
    }
    Ok(text)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to String succeeds");
    }
    encoded
}

fn collect_source_links(links: &mut Vec<Value>, path: &str, source: &str) -> Result<(), String> {
    let parsed = fidryn_syntax::parse_file(source);
    let Some(module) = parsed.module() else {
        return Ok(());
    };
    for item in &module.items {
        let Some(declaration) = declaration(item) else {
            continue;
        };
        let start = declaration.span.start as usize;
        let end = declaration.span.end as usize;
        let text = source
            .get(start..end)
            .ok_or("source declaration has an invalid byte span")?;
        let prefix = &source[..start];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        links.push(json!({"path":path,"kind":declaration.keyword,"name":declaration.name,"startByte":start,"endByte":end,"line":line,"column":column,"text":text,"sourceBasis":declaration.source_basis.as_ref().map(|basis| format!("{basis:?}"))}));
    }
    Ok(())
}

fn declaration(item: &Item) -> Option<&Decl> {
    match item {
        Item::Header(_) => None,
        Item::Source(declaration)
        | Item::Import(declaration)
        | Item::Entity(declaration)
        | Item::Type(declaration)
        | Item::RecordType(declaration)
        | Item::Office(declaration)
        | Item::Proposition(declaration)
        | Item::Observation(declaration)
        | Item::Nomination(declaration)
        | Item::Function(declaration)
        | Item::Effect(declaration)
        | Item::Rule(declaration)
        | Item::Power(declaration)
        | Item::Duty(declaration)
        | Item::Judgment(declaration)
        | Item::Decision(declaration)
        | Item::LegalAct(declaration)
        | Item::Clause(declaration)
        | Item::Interpretation(declaration)
        | Item::ConflictDoctrine(declaration)
        | Item::Query(declaration)
        | Item::Scenario(declaration)
        | Item::Verify(declaration) => Some(declaration),
    }
}
