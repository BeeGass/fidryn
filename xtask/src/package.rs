//! Native release archives with source provenance and an extracted-binary check.

use crate::workspace::{cargo_command, load_metadata, workspace_root};
use anyhow::{Context, Result, bail, ensure};
use clap::Args;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};

const ROOT_MARKER: &str = ".fidryn-packages.json";
const RELEASE_MARKER: &str = ".fidryn-package.json";
const MANIFEST: &str = "PACKAGE-MANIFEST.json";
const SMOKE_SOURCE: &str = "module Fidryn.PackageSmoke version \"0.1.0\" {\n    query smoke() -> Bool { return true }\n}\n";
const SMOKE_CASE: &str = "{\"schema\":\"fidryn.case-record/v0.1\",\"admissibleCompletions\":{}}\n";

#[derive(Debug, Args)]
pub struct PackageArgs {
    /// Parent directory for owned version/host directories (default: target/packages).
    #[arg(long, value_name = "DIR")]
    pub output_dir: Option<PathBuf>,
    /// Package an explicitly marked dirty development tree.
    #[arg(long)]
    pub allow_dirty: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArchiveKind {
    TarGz,
    Zip,
}

impl ArchiveKind {
    fn for_host(host: &str) -> Self {
        if host.contains("windows") {
            Self::Zip
        } else {
            Self::TarGz
        }
    }
    fn extension(self) -> &'static str {
        match self {
            Self::TarGz => "tar.gz",
            Self::Zip => "zip",
        }
    }
    fn create_flag(self) -> &'static str {
        match self {
            Self::TarGz => "-czf",
            Self::Zip => "-caf",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Provenance {
    commit: String,
    dirty: bool,
    status: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FileDigest {
    size: u64,
    sha256: String,
    blake3: String,
}

pub fn run(args: PackageArgs) -> Result<()> {
    let root = workspace_root()
        .canonicalize()
        .context("canonical workspace root")?;
    let metadata = load_metadata()?;
    let version = metadata
        .workspace_packages()
        .into_iter()
        .find(|package| package.name == "fidryn-cli")
        .context("fidryn-cli is not a workspace member")?
        .version
        .to_string();
    let host = rustc_host()?;
    validate_component(&version)?;
    validate_component(&host)?;
    let output = absolute_output(
        &root,
        args.output_dir.as_deref().unwrap_or(
            metadata
                .target_directory
                .as_std_path()
                .join("packages")
                .as_path(),
        ),
    )?;
    validate_output_location(&root, &output)?;
    let before = provenance(&root, &output)?;
    ensure!(
        args.allow_dirty || !before.dirty,
        "refusing to package a dirty source tree; commit the changes or explicitly use --allow-dirty for a development package"
    );
    let stem = format!("fidryn-{version}-{host}");
    let release = prepare_output(&root, &output, &stem)?;

    let mut build = cargo_command();
    build.args([
        "build",
        "-p",
        "fidryn-cli",
        "--bin",
        "fidryn",
        "--release",
        "--locked",
        "--offline",
        "--target",
        &host,
    ]);
    checked_status(&mut build)?;
    let binary = metadata
        .target_directory
        .as_std_path()
        .join(&host)
        .join("release")
        .join(format!("fidryn{}", std::env::consts::EXE_SUFFIX));
    regular_file(&binary)?;

    let stage = release.join("stage");
    let payload = stage.join(&stem);
    fs::create_dir_all(&payload)?;
    let binary_name = binary.file_name().context("binary has no filename")?;
    copy_file(&binary, &payload.join(binary_name))?;
    copy_assets(&root, &payload)?;
    fs::create_dir_all(payload.join("examples"))?;
    fs::write(payload.join("examples/package-smoke.fr"), SMOKE_SOURCE)?;
    fs::write(payload.join("examples/package-smoke.case.json"), SMOKE_CASE)?;
    let binary_relative = binary_name.to_str().context("binary name must be UTF-8")?;
    let manifest = build_manifest(&payload, &version, &host, &before, binary_relative)?;
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    fs::write(payload.join(MANIFEST), &manifest_bytes)?;
    fs::write(
        release.join(format!("{stem}.manifest.json")),
        &manifest_bytes,
    )?;

    let kind = ArchiveKind::for_host(&host);
    let tar = tar_program(kind)?;
    let archive = release.join(format!("{stem}.{}", kind.extension()));
    create_archive(&tar, kind, &archive, &stage, &stem)?;
    let extraction = release.join("verification");
    extract_archive(&tar, &archive, &extraction, &stem)?;
    let extracted = extraction.join(&stem);
    verify_manifest(&extracted, &manifest)?;
    verify_binary(
        &extracted,
        binary_relative,
        &version,
        &extraction.join("traces"),
    )?;
    let after = provenance(&root, &output)?;
    ensure!(
        before == after,
        "source provenance changed while packaging; discard this attempt and rerun from a stable tree"
    );
    let digest = digest_file(&archive)?;
    let filename = archive
        .file_name()
        .and_then(OsStr::to_str)
        .context("archive name must be UTF-8")?;
    fs::write(
        release.join(format!("{filename}.sha256")),
        format!("{}  {filename}\n", digest.sha256),
    )?;
    fs::write(
        release.join(format!("{filename}.blake3")),
        format!("{}  {filename}\n", digest.blake3),
    )?;
    fs::remove_dir_all(&stage)?;
    fs::remove_dir_all(&extraction)?;
    for name in asset_names(&stem, kind) {
        let source = release.join(&name);
        let destination = output.join(&name);
        if fs::symlink_metadata(&destination).is_ok() {
            regular_file(&destination)?;
            fs::remove_file(&destination)?;
        }
        fs::rename(&source, &destination)
            .with_context(|| format!("publish verified package asset {name}"))?;
    }
    println!(
        "Verified native package: {}",
        output.join(filename).display()
    );
    println!(
        "Source: {}{}",
        before.commit,
        if before.dirty {
            " (dirty development tree)"
        } else {
            ""
        }
    );
    println!("SHA256: {}", digest.sha256);
    println!("BLAKE3: {}", digest.blake3);
    Ok(())
}

fn absolute_output(root: &Path, path: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    ensure!(
        !path
            .components()
            .any(|part| matches!(part, Component::ParentDir)),
        "output directory must not contain parent traversal"
    );
    let mut existing = path.as_path();
    let mut missing = Vec::new();
    while !existing.exists() {
        if let Ok(metadata) = fs::symlink_metadata(existing) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "output directory is a dangling symlink"
            );
        }
        missing.push(
            existing
                .file_name()
                .context("output directory has no existing ancestor")?
                .to_owned(),
        );
        existing = existing
            .parent()
            .context("output directory has no existing ancestor")?;
    }
    ensure_no_symlink_components(existing)?;
    let mut resolved = existing.canonicalize()?;
    for component in missing.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn validate_output_location(root: &Path, output: &Path) -> Result<()> {
    ensure!(
        output != root && !root.starts_with(output),
        "output directory may not be the workspace or one of its ancestors"
    );
    ensure!(
        !output
            .components()
            .any(|part| matches!(part, Component::Normal(name) if name == ".git")),
        "output directory may not be inside .git"
    );
    if let Ok(relative) = output.strip_prefix(root) {
        let mut tracked = Command::new("git");
        tracked
            .current_dir(root)
            .args(["ls-files", "-z", "--"])
            .arg(relative);
        ensure!(
            checked_output(&mut tracked)?.stdout.is_empty(),
            "output directory contains tracked source files"
        );
    }
    Ok(())
}

fn provenance(root: &Path, output: &Path) -> Result<Provenance> {
    let mut head = Command::new("git");
    head.current_dir(root)
        .args(["rev-parse", "--verify", "HEAD"]);
    let commit = String::from_utf8(checked_output(&mut head)?.stdout)?
        .trim()
        .to_owned();
    ensure!(
        commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "git did not return a full commit identity"
    );
    let mut status = Command::new("git");
    status.current_dir(root).args([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
        ".",
    ]);
    if let Ok(relative) = output.strip_prefix(root) {
        status.arg(format!(":(exclude,literal){}", portable_path(relative)?));
    }
    let status = checked_output(&mut status)?.stdout;
    Ok(Provenance {
        commit,
        dirty: !status.is_empty(),
        status,
    })
}

fn prepare_output(root: &Path, output: &Path, stem: &str) -> Result<PathBuf> {
    validate_component(stem)?;
    ensure_no_symlink_components(output)?;
    let root_owner = json!({"schema":"fidryn.package-output/v0.1","workspace":root.to_str().context("workspace path must be UTF-8")?,"output":output.to_str().context("output path must be UTF-8")?});
    if output.exists() {
        ensure!(
            output.is_dir(),
            "package output exists and is not a directory"
        );
        if output.join(ROOT_MARKER).exists() {
            verify_owner(&output.join(ROOT_MARKER), &root_owner)?;
        } else {
            ensure!(
                fs::read_dir(output)?.next().is_none(),
                "refusing to take ownership of a nonempty package output directory"
            );
            write_new(&output.join(ROOT_MARKER), &serde_json::to_vec(&root_owner)?)?;
        }
    } else {
        fs::create_dir_all(output)?;
        write_new(&output.join(ROOT_MARKER), &serde_json::to_vec(&root_owner)?)?;
    }
    let release = output.join(format!(".{stem}.work"));
    let owner = json!({"schema":"fidryn.package-directory/v0.1","workspace":root.to_str(),"output":output.to_str(),"package":stem});
    if fs::symlink_metadata(&release).is_err() {
        for name in asset_names(stem, ArchiveKind::for_host(stem)) {
            ensure!(
                fs::symlink_metadata(output.join(name)).is_err(),
                "refusing to replace package assets without their existing ownership marker"
            );
        }
    }
    if fs::symlink_metadata(&release).is_ok() {
        ensure!(
            fs::symlink_metadata(&release)?.is_dir(),
            "refusing to replace a non-directory package path"
        );
        verify_owner(&release.join(RELEASE_MARKER), &owner)?;
        ensure_tree_regular(&release)?;
        fs::remove_dir_all(&release)
            .context("remove previous owned version/host package directory")?;
    }
    fs::create_dir(&release)?;
    write_new(&release.join(RELEASE_MARKER), &serde_json::to_vec(&owner)?)?;
    Ok(release)
}

fn verify_owner(path: &Path, expected: &Value) -> Result<()> {
    regular_file(path)?;
    let metadata = fs::metadata(path)?;
    ensure!(
        metadata.len() <= 16_384,
        "package ownership marker is too large"
    );
    let actual: Value = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        &actual == expected,
        "refusing to replace a directory with a missing or mismatched package ownership marker: {}",
        path.display()
    );
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    File::options()
        .create_new(true)
        .write(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}

fn ensure_no_symlink_components(path: &Path) -> Result<()> {
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            ensure!(
                !metadata.file_type().is_symlink(),
                "symlink paths are not allowed in package output: {}",
                current.display()
            );
        }
    }
    Ok(())
}

fn regular_file(path: &Path) -> Result<()> {
    ensure!(
        fs::symlink_metadata(path)
            .with_context(|| format!("read {}", path.display()))?
            .file_type()
            .is_file(),
        "package inputs must be regular files: {}",
        path.display()
    );
    Ok(())
}

fn ensure_tree_regular(root: &Path) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let kind = fs::symlink_metadata(&path)?.file_type();
        ensure!(
            !kind.is_symlink(),
            "refusing to clean or package a symlink: {}",
            path.display()
        );
        if kind.is_dir() {
            ensure_tree_regular(&path)?;
        } else {
            ensure!(
                kind.is_file(),
                "special files are not allowed in a package directory"
            );
        }
    }
    Ok(())
}

fn copy_file(source: &Path, target: &Path) -> Result<()> {
    regular_file(source)?;
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(source, target)
        .with_context(|| format!("copy {} to {}", source.display(), target.display()))?;
    Ok(())
}

fn copy_assets(root: &Path, payload: &Path) -> Result<()> {
    let mut command = Command::new("git");
    command.current_dir(root).args([
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
        "--",
        "LICENSE",
        "README.md",
        "CHANGELOG.md",
        "schemas",
        "templates",
        "docs",
    ]);
    let output = checked_output(&mut command)?;
    let mut assets = BTreeSet::new();
    for name in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let name = std::str::from_utf8(name).context("source asset path must be UTF-8")?;
        validate_relative(name)?;
        if root.join(name).exists() {
            assets.insert(name.to_owned());
        }
    }
    for name in ["LICENSE", "README.md", "CHANGELOG.md"] {
        ensure!(
            assets.contains(name),
            "required package asset is missing, ignored or outside the source inventory: {name}"
        );
    }
    for name in ["schemas", "templates", "docs"] {
        ensure!(
            fs::symlink_metadata(root.join(name))?.file_type().is_dir(),
            "required package assets must be real directories: {name}"
        );
        ensure!(
            assets
                .iter()
                .any(|path| path.starts_with(&format!("{name}/"))),
            "required package asset directory is empty: {name}"
        );
    }
    for name in assets {
        copy_file(&root.join(&name), &payload.join(&name))?;
    }
    Ok(())
}

fn asset_names(stem: &str, kind: ArchiveKind) -> Vec<String> {
    let archive = format!("{stem}.{}", kind.extension());
    vec![
        format!("{stem}.manifest.json"),
        archive.clone(),
        format!("{archive}.sha256"),
        format!("{archive}.blake3"),
    ]
}

fn file_inventory(root: &Path) -> Result<Vec<PathBuf>> {
    fn walk(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            let kind = fs::symlink_metadata(&path)?.file_type();
            if kind.is_dir() {
                walk(root, &path, output)?;
            } else {
                ensure!(
                    kind.is_file(),
                    "only regular files may be packaged: {}",
                    path.display()
                );
                let relative = path.strip_prefix(root)?.to_path_buf();
                portable_path(&relative)?;
                output.push(relative);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

fn build_manifest(
    payload: &Path,
    version: &str,
    host: &str,
    provenance: &Provenance,
    binary: &str,
) -> Result<Value> {
    let mut files = Vec::new();
    let mut included = BTreeSet::from([MANIFEST.to_owned()]);
    for path in file_inventory(payload)? {
        let name = portable_path(&path)?;
        let digest = digest_file(&payload.join(&path))?;
        included.insert(name.clone());
        files.push(
            json!({"path":name,"size":digest.size,"sha256":digest.sha256,"blake3":digest.blake3}),
        );
    }
    let executable = files
        .iter()
        .find(|file| file["path"] == binary)
        .context("package manifest is missing its binary")?
        .clone();
    Ok(
        json!({"schema":"fidryn.package/v0.1","version":version,"host":host,"sourceCommit":provenance.commit,"dirty":provenance.dirty,"binary":executable,"manifestPath":MANIFEST,"includedFiles":included,"files":files}),
    )
}

fn digest_file(path: &Path) -> Result<FileDigest> {
    regular_file(path)?;
    let mut file = File::open(path)?;
    let mut sha256 = Sha256::new();
    let mut blake3 = blake3::Hasher::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 65_536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .context("package input size overflow")?;
        sha256.update(&buffer[..count]);
        blake3.update(&buffer[..count]);
    }
    Ok(FileDigest {
        size,
        sha256: format!("{:x}", sha256.finalize()),
        blake3: blake3.finalize().to_hex().to_string(),
    })
}

fn verify_manifest(payload: &Path, manifest: &Value) -> Result<()> {
    let actual: Value = serde_json::from_slice(&fs::read(payload.join(MANIFEST))?)?;
    ensure!(
        &actual == manifest,
        "extracted package manifest differs from the staged manifest"
    );
    let expected: BTreeSet<_> = manifest["includedFiles"]
        .as_array()
        .context("manifest includedFiles is missing")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .context("invalid included filename")
        })
        .collect::<Result<_>>()?;
    let inventory: BTreeSet<_> = file_inventory(payload)?
        .iter()
        .map(|path| portable_path(path))
        .collect::<Result<_>>()?;
    ensure!(
        inventory == expected,
        "extracted archive file inventory differs from the manifest"
    );
    for file in manifest["files"]
        .as_array()
        .context("manifest files is missing")?
    {
        let name = file["path"]
            .as_str()
            .context("manifest file path is missing")?;
        validate_relative(name)?;
        let digest = digest_file(&payload.join(name))?;
        ensure!(
            file["size"].as_u64() == Some(digest.size)
                && file["sha256"] == digest.sha256
                && file["blake3"] == digest.blake3,
            "extracted package file does not match its digest: {name}"
        );
    }
    Ok(())
}

fn rustc_host() -> Result<String> {
    let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command.current_dir(workspace_root()).arg("-vV");
    let output = checked_output(&mut command)?;
    let version = String::from_utf8(output.stdout)?;
    version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .context("rustc -vV did not report a host target")
}

fn tar_program(kind: ArchiveKind) -> Result<PathBuf> {
    let program = if cfg!(windows) {
        std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .map(|root| root.join("System32/tar.exe"))
            .filter(|path| path.is_file())
            .unwrap_or_else(|| "tar".into())
    } else {
        "tar".into()
    };
    let mut command = Command::new(&program);
    command.arg("--version");
    let version = checked_output(&mut command)?;
    if kind == ArchiveKind::Zip {
        ensure!(
            String::from_utf8_lossy(&version.stdout).contains("bsdtar"),
            "Windows ZIP packaging requires system bsdtar"
        );
    }
    Ok(program)
}

fn create_archive(
    tar: &Path,
    kind: ArchiveKind,
    archive: &Path,
    stage: &Path,
    stem: &str,
) -> Result<()> {
    let mut command = Command::new(tar);
    command
        .arg(kind.create_flag())
        .arg(archive)
        .arg("-C")
        .arg(stage)
        .arg("--")
        .arg(stem);
    checked_status(&mut command)
}

fn extract_archive(tar: &Path, archive: &Path, output: &Path, stem: &str) -> Result<()> {
    let mut listing = Command::new(tar);
    listing.arg("-tf").arg(archive);
    let listing = String::from_utf8(checked_output(&mut listing)?.stdout)?;
    ensure!(!listing.is_empty(), "package archive is empty");
    for entry in listing.lines() {
        let entry = entry.trim_end_matches('/');
        validate_relative(entry)?;
        ensure!(
            entry == stem || entry.starts_with(&format!("{stem}/")),
            "archive entry escapes its package directory: {entry}"
        );
    }
    fs::create_dir(output)?;
    let mut command = Command::new(tar);
    command.arg("-xf").arg(archive).arg("-C").arg(output);
    checked_status(&mut command)
}

fn verify_binary(
    payload: &Path,
    binary: &str,
    version: &str,
    trace_directory: &Path,
) -> Result<()> {
    let executable = payload.join(binary);
    let mut command = Command::new(&executable);
    command.current_dir(payload).arg("--version");
    let output = checked_output(&mut command)?;
    validate_version(&String::from_utf8(output.stdout)?, version)?;
    let mut command = Command::new(&executable);
    command
        .current_dir(payload)
        .arg("check")
        .arg("examples/package-smoke.fr");
    checked_status(&mut command)?;
    let mut command = Command::new(&executable);
    command
        .current_dir(payload)
        .args([
            "run",
            "examples/package-smoke.fr",
            "--query",
            "smoke",
            "--case",
            "examples/package-smoke.case.json",
            "--valid-at",
            "2026-01-01T00:00:00Z",
            "--known-at",
            "2026-01-01T00:00:00Z",
        ])
        .env("FIDRYN_TRACE_DIR", trace_directory);
    let report = checked_output(&mut command)?;
    let report: Value = serde_json::from_slice(&report.stdout)
        .context("packaged CLI did not emit a JSON smoke report")?;
    validate_smoke_report(&report)
}

fn validate_version(output: &str, version: &str) -> Result<()> {
    ensure!(
        output.trim() == format!("fidryn {version}"),
        "packaged CLI version mismatch: expected fidryn {version}, got {}",
        output.trim()
    );
    Ok(())
}

fn validate_smoke_report(report: &Value) -> Result<()> {
    let outcome = &report["outcomeDocument"]["outcome"];
    ensure!(
        report["schema"] == "fidryn.evaluation-report/v0.2"
            && outcome["kind"] == "determinate"
            && outcome["value"]["kind"] == "bool"
            && outcome["value"]["data"] == true,
        "packaged CLI did not produce the expected determinate Boolean smoke report"
    );
    Ok(())
}

fn validate_component(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-+".contains(&byte))
            && name != "."
            && name != "..",
        "invalid package version or host component: {name}"
    );
    Ok(())
}

fn portable_path(path: &Path) -> Result<String> {
    let name = path
        .components()
        .map(|component| match component {
            Component::Normal(name) => name.to_str().context("package filename must be UTF-8"),
            _ => bail!("package path must be relative without traversal"),
        })
        .collect::<Result<Vec<_>>>()?
        .join("/");
    validate_relative(&name)?;
    Ok(name)
}

fn validate_relative(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && !name.starts_with('/')
            && !name.contains(['\\', '\0', ':', '\n', '\r'])
            && name
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."),
        "invalid archive-relative path: {name:?}"
    );
    Ok(())
}

fn checked_status(command: &mut Command) -> Result<()> {
    eprintln!("+ {command:?}");
    ensure!(
        command
            .status()
            .with_context(|| format!("spawn {command:?}"))?
            .success(),
        "command failed: {command:?}"
    );
    Ok(())
}

fn checked_output(command: &mut Command) -> Result<Output> {
    let output = command
        .output()
        .with_context(|| format!("spawn {command:?}"))?;
    ensure!(
        output.status.success(),
        "command failed: {command:?}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fidryn-package-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn provenance_fixture() -> Provenance {
        Provenance {
            commit: "1".repeat(40),
            dirty: false,
            status: Vec::new(),
        }
    }
    fn tiny_payload(root: &Path) -> PathBuf {
        let payload = root.join("fidryn-test-host");
        fs::create_dir_all(payload.join("schemas")).unwrap();
        fs::write(payload.join("fidryn"), b"binary fixture").unwrap();
        fs::write(payload.join("LICENSE"), b"license").unwrap();
        fs::write(payload.join("schemas/case.json"), b"{}").unwrap();
        payload
    }

    #[test]
    fn refuses_an_unowned_output_without_removing_its_files() {
        let fixture = Fixture::new();
        let output = fixture.0.join("output");
        fs::create_dir(&output).unwrap();
        fs::write(output.join("keep"), "not package output").unwrap();
        assert!(prepare_output(&fixture.0, &output, "fidryn-1-host").is_err());
        assert_eq!(
            fs::read_to_string(output.join("keep")).unwrap(),
            "not package output"
        );
    }

    #[test]
    fn repeat_replaces_only_the_matching_owned_version_directory() {
        let fixture = Fixture::new();
        let output = fixture.0.join("output");
        let first = prepare_output(&fixture.0, &output, "fidryn-1-host").unwrap();
        fs::write(first.join("stale-archive"), "old").unwrap();
        let other = prepare_output(&fixture.0, &output, "fidryn-2-host").unwrap();
        fs::write(other.join("keep"), "another version").unwrap();
        let repeat = prepare_output(&fixture.0, &output, "fidryn-1-host").unwrap();
        assert!(!repeat.join("stale-archive").exists());
        assert_eq!(
            fs::read_to_string(other.join("keep")).unwrap(),
            "another version"
        );
    }

    #[test]
    fn mismatched_release_marker_refuses_cleanup() {
        let fixture = Fixture::new();
        let output = fixture.0.join("output");
        let release = prepare_output(&fixture.0, &output, "fidryn-1-host").unwrap();
        fs::write(release.join(RELEASE_MARKER), b"{}").unwrap();
        fs::write(release.join("keep"), b"retained").unwrap();
        assert!(prepare_output(&fixture.0, &output, "fidryn-1-host").is_err());
        assert!(release.join("keep").exists());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_in_owned_output_are_not_cleaned_or_followed() {
        let fixture = Fixture::new();
        let output = fixture.0.join("output");
        let release = prepare_output(&fixture.0, &output, "fidryn-1-host").unwrap();
        let outside = fixture.0.join("outside");
        fs::write(&outside, b"keep").unwrap();
        std::os::unix::fs::symlink(&outside, release.join("link")).unwrap();
        assert!(prepare_output(&fixture.0, &output, "fidryn-1-host").is_err());
        assert_eq!(fs::read(outside).unwrap(), b"keep");
    }

    #[test]
    fn manifest_hashes_and_inventory_detect_tampering_and_extra_assets() {
        let fixture = Fixture::new();
        let payload = tiny_payload(&fixture.0);
        let manifest = build_manifest(
            &payload,
            "1.2.3",
            "test-host",
            &provenance_fixture(),
            "fidryn",
        )
        .unwrap();
        assert_eq!(manifest["version"], "1.2.3");
        assert_eq!(manifest["sourceCommit"], "1".repeat(40));
        assert_eq!(manifest["dirty"], false);
        assert_eq!(manifest["host"], "test-host");
        fs::write(
            payload.join(MANIFEST),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        verify_manifest(&payload, &manifest).unwrap();
        fs::write(payload.join("fidryn"), b"altered").unwrap();
        assert!(verify_manifest(&payload, &manifest).is_err());
        fs::write(payload.join("fidryn"), b"binary fixture").unwrap();
        fs::write(payload.join("stale"), b"not declared").unwrap();
        assert!(verify_manifest(&payload, &manifest).is_err());
    }

    #[test]
    fn standard_sha256_and_blake3_hash_the_file_bytes() {
        let fixture = Fixture::new();
        let path = fixture.0.join("abc");
        fs::write(&path, b"abc").unwrap();
        let digest = digest_file(&path).unwrap();
        assert_eq!(digest.size, 3);
        assert_eq!(
            digest.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(digest.blake3, blake3::hash(b"abc").to_hex().as_str());
    }

    #[test]
    fn archive_round_trip_retains_the_declared_tiny_inventory() {
        let fixture = Fixture::new();
        let payload = tiny_payload(&fixture.0);
        let stem = payload.file_name().unwrap().to_str().unwrap();
        let manifest = build_manifest(
            &payload,
            "1.2.3",
            "test-host",
            &provenance_fixture(),
            "fidryn",
        )
        .unwrap();
        fs::write(
            payload.join(MANIFEST),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let kind = if cfg!(windows) {
            ArchiveKind::Zip
        } else {
            ArchiveKind::TarGz
        };
        let tar = tar_program(kind).unwrap();
        let archive = fixture.0.join(format!("test.{}", kind.extension()));
        create_archive(&tar, kind, &archive, &fixture.0, stem).unwrap();
        let extraction = fixture.0.join("extracted");
        extract_archive(&tar, &archive, &extraction, stem).unwrap();
        verify_manifest(&extraction.join(stem), &manifest).unwrap();
    }

    #[test]
    fn rejects_nonportable_or_escaping_archive_paths() {
        for path in [
            "",
            "/absolute",
            "../parent",
            "root/../escape",
            "root\\escape",
            "C:/drive",
            "root/./file",
            "root//file",
        ] {
            assert!(validate_relative(path).is_err(), "{path}");
        }
        assert!(validate_relative("fidryn-1-host/docs/getting-started.md").is_ok());
        assert!(validate_component("1.2.3-rc.1+build").is_ok());
        assert!(validate_component("../escape").is_err());
    }

    #[test]
    fn native_formats_and_smoke_results_are_checked() {
        assert_eq!(
            ArchiveKind::for_host("x86_64-pc-windows-msvc"),
            ArchiveKind::Zip
        );
        assert_eq!(
            ArchiveKind::for_host("aarch64-apple-darwin"),
            ArchiveKind::TarGz
        );
        assert_eq!(
            ArchiveKind::for_host("x86_64-unknown-linux-gnu"),
            ArchiveKind::TarGz
        );
        assert!(validate_version("fidryn 1.2.3\n", "1.2.3").is_ok());
        assert!(validate_version("fidryn 1.2.2", "1.2.3").is_err());
        let mut report = json!({"schema":"fidryn.evaluation-report/v0.2","outcomeDocument":{"outcome":{"kind":"determinate","value":{"kind":"bool","data":true}}}});
        validate_smoke_report(&report).unwrap();
        report["outcomeDocument"]["outcome"]["value"]["data"] = json!(false);
        assert!(validate_smoke_report(&report).is_err());
    }

    #[test]
    fn assets_exclude_ignored_local_files_but_include_visible_development_files() {
        let fixture = Fixture::new();
        let mut init = Command::new("git");
        init.current_dir(&fixture.0).args(["init", "--quiet"]);
        checked_status(&mut init).unwrap();
        fs::write(fixture.0.join(".gitignore"), ".DS_Store\n__pycache__/\n").unwrap();
        for name in ["LICENSE", "README.md", "CHANGELOG.md"] {
            fs::write(fixture.0.join(name), name).unwrap();
        }
        for name in ["docs", "schemas", "templates"] {
            fs::create_dir(fixture.0.join(name)).unwrap();
            fs::write(fixture.0.join(name).join("visible.txt"), name).unwrap();
        }
        fs::write(fixture.0.join("docs/.DS_Store"), "local metadata").unwrap();
        fs::create_dir(fixture.0.join("docs/__pycache__")).unwrap();
        fs::write(fixture.0.join("docs/__pycache__/local.pyc"), "local cache").unwrap();
        let payload = fixture.0.join("payload");
        copy_assets(&fixture.0, &payload).unwrap();
        assert!(payload.join("docs/visible.txt").is_file());
        assert!(!payload.join("docs/.DS_Store").exists());
        assert!(!payload.join("docs/__pycache__").exists());
    }

    #[test]
    fn final_asset_names_are_unique_and_flat() {
        assert_eq!(
            asset_names("fidryn-1-host", ArchiveKind::TarGz),
            vec![
                "fidryn-1-host.manifest.json",
                "fidryn-1-host.tar.gz",
                "fidryn-1-host.tar.gz.sha256",
                "fidryn-1-host.tar.gz.blake3"
            ]
        );
        for name in asset_names("fidryn-1-x86_64-pc-windows-msvc", ArchiveKind::Zip) {
            assert!(!name.contains('/'));
            assert!(name.starts_with("fidryn-1-x86_64-pc-windows-msvc"));
        }
    }

    #[test]
    fn refuses_flat_assets_without_their_owned_version_directory() {
        let fixture = Fixture::new();
        let output = fixture.0.join("output");
        prepare_output(&fixture.0, &output, "fidryn-1-host").unwrap();
        fs::write(output.join("fidryn-2-host.tar.gz"), "unowned archive").unwrap();
        assert!(prepare_output(&fixture.0, &output, "fidryn-2-host").is_err());
        assert_eq!(
            fs::read_to_string(output.join("fidryn-2-host.tar.gz")).unwrap(),
            "unowned archive"
        );
    }
}
