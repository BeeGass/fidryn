//! Retained dependency notices from the locked Cargo graph.

use crate::workspace::{cargo_bin, workspace_root};
use anyhow::{Context, Result, bail, ensure};
use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;

const DIRECTORY: &str = "THIRD_PARTY_LICENSES";
const INDEX: &str = "THIRD_PARTY_NOTICES.md";
const PROVENANCE: &str = "provenance.json";
const MAX_METADATA_BYTES: usize = 16 * 1024 * 1024;
const MAX_PACKAGES: usize = 4096;
const MAX_ENTRIES: usize = 500_000;
const MAX_DEPTH: usize = 32;
const MAX_DOCUMENTS: usize = 16_384;
const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;

struct CrateInput {
    name: String,
    version: String,
    source: Option<String>,
    license: Option<String>,
    license_file: Option<PathBuf>,
    root: PathBuf,
}

struct Document {
    path: String,
    bytes: Vec<u8>,
}

struct Notices {
    input: CrateInput,
    origin: Value,
    documents: Vec<Document>,
}

#[derive(Default)]
struct Limits {
    entries: usize,
    documents: usize,
    bytes: usize,
}

impl Limits {
    fn entry(&mut self) -> Result<()> {
        self.entries += 1;
        ensure!(
            self.entries <= MAX_ENTRIES,
            "dependency notice inventory exceeds {MAX_ENTRIES} entries"
        );
        Ok(())
    }

    fn document(&mut self, bytes: usize) -> Result<()> {
        self.documents += 1;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .context("dependency notice size overflow")?;
        ensure!(
            self.documents <= MAX_DOCUMENTS,
            "dependency notices exceed {MAX_DOCUMENTS} documents"
        );
        ensure!(
            self.bytes <= MAX_TOTAL_BYTES,
            "dependency notices exceed the 64 MiB byte limit"
        );
        Ok(())
    }
}

/// Retain notice documents for the locked, all-features workspace graph.
/// This inventory includes development and other-target dependencies; it is
/// not a claim about exact executable linkage or a legal assessment.
pub fn copy_third_party_notices(payload: &Path) -> Result<()> {
    let root = workspace_root();
    let metadata = locked_metadata(&root)?;
    let workspace = metadata.workspace_members.iter().collect::<BTreeSet<_>>();
    let resolved = metadata
        .resolve
        .as_ref()
        .context("Cargo metadata omitted the resolved dependency graph")?;
    let packages = metadata
        .packages
        .iter()
        .map(|package| (&package.id, package))
        .collect::<BTreeMap<_, _>>();
    let mut inputs = Vec::new();
    for node in &resolved.nodes {
        if workspace.contains(&node.id) {
            continue;
        }
        let package = packages
            .get(&node.id)
            .context("resolved dependency is absent from Cargo package metadata")?;
        inputs.push(CrateInput {
            name: package.name.to_string(),
            version: package.version.to_string(),
            source: package.source.as_ref().map(ToString::to_string),
            license: package.license.clone(),
            license_file: package
                .license_file
                .as_ref()
                .map(|path| path.as_std_path().to_path_buf()),
            root: package
                .manifest_path
                .parent()
                .context("dependency manifest has no parent directory")?
                .as_std_path()
                .to_path_buf(),
        });
    }
    copy_inputs(payload, inputs, &root.join("LICENSES/crates"))
}

fn locked_metadata(root: &Path) -> Result<Metadata> {
    let mut metadata = MetadataCommand::new();
    metadata
        .cargo_path(cargo_bin())
        .current_dir(root)
        .manifest_path(root.join("Cargo.toml"))
        .features(CargoOpt::AllFeatures)
        .other_options(vec!["--locked".into(), "--offline".into()]);
    let mut command = metadata.cargo_command();
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = command
        .spawn()
        .context("run locked offline Cargo dependency metadata")?;
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .context("Cargo metadata has no output")?
        .take((MAX_METADATA_BYTES + 1) as u64)
        .read_to_end(&mut bytes);
    if read.is_err() || bytes.len() > MAX_METADATA_BYTES {
        let _ = child.kill();
        let _ = child.wait();
        read.context("read Cargo dependency metadata")?;
        bail!("Cargo dependency metadata exceeds the 16 MiB byte limit");
    }
    ensure!(
        child.wait()?.success(),
        "locked offline Cargo dependency metadata failed"
    );
    serde_json::from_slice(&bytes).context("invalid Cargo dependency metadata")
}

fn copy_inputs(payload: &Path, mut inputs: Vec<CrateInput>, fallback: &Path) -> Result<()> {
    ensure!(
        inputs.len() <= MAX_PACKAGES,
        "dependency notice graph exceeds {MAX_PACKAGES} packages"
    );
    ensure!(
        fs::symlink_metadata(payload)?.file_type().is_dir(),
        "dependency notice destination must be a real directory"
    );
    for path in [payload.join(DIRECTORY), payload.join(INDEX)] {
        ensure!(
            fs::symlink_metadata(&path)
                .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
            "refusing to replace existing dependency notice output: {}",
            path.display()
        );
    }
    inputs.sort_by(|left, right| {
        (&left.name, &left.version, &left.source).cmp(&(&right.name, &right.version, &right.source))
    });
    let mut stems = BTreeSet::new();
    let mut limits = Limits::default();
    let mut notices = Vec::new();
    for input in inputs {
        let stem = crate_directory(&input)?;
        ensure!(
            stems.insert(stem),
            "distinct dependencies have the same name-version notice directory"
        );
        notices.push(collect_notices(input, fallback, &mut limits)?);
    }
    let index = render_index(&notices)?;
    ensure!(
        index.len() <= MAX_METADATA_BYTES,
        "dependency notice index exceeds the 16 MiB byte limit"
    );
    fs::create_dir(payload.join(DIRECTORY))?;
    for notice in notices {
        let directory = payload
            .join(DIRECTORY)
            .join(crate_directory(&notice.input)?);
        fs::create_dir(&directory)?;
        for document in notice.documents {
            let path = directory.join(relative_path(&document.path)?);
            fs::create_dir_all(path.parent().context("notice output has no parent")?)?;
            write_new(&path, &document.bytes)?;
        }
    }
    write_new(&payload.join(INDEX), index.as_bytes())
}

fn crate_directory(input: &CrateInput) -> Result<String> {
    for value in [&input.name, &input.version] {
        ensure!(
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-+".contains(&byte))
                && value != "."
                && value != "..",
            "invalid dependency name/version for notice output"
        );
    }
    Ok(format!("{}-{}", input.name, input.version))
}

fn collect_notices(mut input: CrateInput, fallback: &Path, limits: &mut Limits) -> Result<Notices> {
    ensure!(
        fs::symlink_metadata(&input.root)?.file_type().is_dir(),
        "dependency source must be a real directory: {}",
        input.root.display()
    );
    let root = input.root.canonicalize()?;
    let mut paths = BTreeSet::new();
    walk(&root, &root, 0, limits, &mut |path| {
        if is_notice(path) {
            paths.insert(portable(path)?);
        }
        Ok(())
    })?;
    if let Some(path) = &input.license_file {
        let path = if path.is_absolute() {
            path.clone()
        } else {
            root.join(path)
        };
        let mut component_path = PathBuf::new();
        for component in path.components() {
            component_path.push(component);
            if !matches!(component, Component::Normal(_)) {
                continue;
            }
            ensure!(
                !fs::symlink_metadata(&component_path)?
                    .file_type()
                    .is_symlink(),
                "symlink in declared dependency license_file: {}",
                component_path.display()
            );
        }
        let path = path.canonicalize()?;
        ensure!(
            path.starts_with(&root),
            "declared dependency license_file escapes its crate directory"
        );
        let relative = path.strip_prefix(&root)?;
        confined_file(&root, relative)?;
        paths.insert(portable(relative)?);
        input.license_file = Some(relative.to_path_buf());
    }
    if paths.is_empty() {
        return collect_fallback(input, fallback, limits);
    }
    let documents = paths
        .into_iter()
        .map(|path| {
            let bytes = read_document(&root, &relative_path(&path)?, limits)?;
            Ok(Document { path, bytes })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Notices {
        input,
        origin: json!({"kind":"resolved_cargo_package"}),
        documents,
    })
}

fn walk(
    root: &Path,
    directory: &Path,
    depth: usize,
    limits: &mut Limits,
    visit: &mut impl FnMut(&Path) -> Result<()>,
) -> Result<()> {
    ensure!(
        depth <= MAX_DEPTH,
        "dependency notice directory depth exceeds {MAX_DEPTH}"
    );
    for entry in
        fs::read_dir(directory).with_context(|| format!("inventory {}", directory.display()))?
    {
        let path = entry?.path();
        limits.entry()?;
        let kind = fs::symlink_metadata(&path)?.file_type();
        ensure!(
            !kind.is_symlink(),
            "symlinks are not allowed in dependency notice inputs: {}",
            path.display()
        );
        if kind.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| matches!(name, ".git" | ".hg" | ".svn" | "target"))
            {
                continue;
            }
            walk(root, &path, depth + 1, limits, visit)?;
        } else {
            ensure!(
                kind.is_file(),
                "dependency notice inputs contain a special file: {}",
                path.display()
            );
            visit(path.strip_prefix(root)?)?;
        }
    }
    Ok(())
}

fn is_notice(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let name = name.to_ascii_uppercase();
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "rs" | "c"
                    | "h"
                    | "cpp"
                    | "cc"
                    | "py"
                    | "js"
                    | "ts"
                    | "wasm"
                    | "png"
                    | "jpg"
                    | "woff"
                    | "woff2"
                    | "ttf"
                    | "a"
                    | "so"
                    | "dll"
                    | "exe"
            )
        })
    {
        return false;
    }
    let prefixed = [
        "LICENSE",
        "LICENCE",
        "COPYING",
        "COPYRIGHT",
        "NOTICE",
        "UNLICENSE",
        "AUTHORS",
    ]
    .iter()
    .any(|prefix| name.starts_with(prefix));
    let in_license_directory = path.parent().is_some_and(|parent| parent.components().any(|component| {
        matches!(component, Component::Normal(name) if name.to_str().is_some_and(|name| matches!(name.to_ascii_lowercase().as_str(), "licenses" | "licences" | "notices")))
    }));
    prefixed || in_license_directory
}

fn confined_file(root: &Path, relative: &Path) -> Result<PathBuf> {
    relative_path(&portable(relative)?)?;
    let mut path = root.to_path_buf();
    for component in relative.components() {
        path.push(component);
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "symlink in dependency notice path: {}",
            path.display()
        );
    }
    ensure!(
        fs::symlink_metadata(&path)?.file_type().is_file(),
        "dependency notice must be a regular file: {}",
        path.display()
    );
    ensure!(
        path.canonicalize()?.starts_with(root),
        "dependency notice escapes its crate directory"
    );
    Ok(path)
}

fn read_document(root: &Path, relative: &Path, limits: &mut Limits) -> Result<Vec<u8>> {
    let path = confined_file(root, relative)?;
    let file = File::open(&path)?;
    ensure!(
        file.metadata()?.len() <= MAX_DOCUMENT_BYTES as u64,
        "dependency notice exceeds the 2 MiB document limit: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_DOCUMENT_BYTES && !bytes.contains(&0),
        "dependency notice is empty, binary or exceeds its byte limit: {}",
        path.display()
    );
    limits.document(bytes.len())?;
    Ok(bytes)
}

fn collect_fallback(input: CrateInput, fallback: &Path, limits: &mut Limits) -> Result<Notices> {
    let directory = fallback.join(crate_directory(&input)?);
    ensure!(
        fs::symlink_metadata(&directory).is_ok_and(|metadata| metadata.file_type().is_dir()),
        "dependency {} {} contains no notice documents and has no verified exact-version fallback",
        input.name,
        input.version
    );
    let root = directory.canonicalize()?;
    let provenance_bytes = read_document(&root, Path::new(PROVENANCE), limits)?;
    let provenance: Value = serde_json::from_slice(&provenance_bytes)
        .context("invalid dependency notice fallback provenance")?;
    ensure!(
        provenance["crate"] == input.name
            && provenance["version"] == input.version
            && provenance["cargoSource"].as_str() == input.source.as_deref(),
        "dependency notice fallback does not match its crate/version/Cargo source"
    );
    let repository = provenance["sourceRepository"]
        .as_str()
        .context("fallback sourceRepository is missing")?;
    let commit = provenance["sourceCommit"]
        .as_str()
        .context("fallback sourceCommit is missing")?;
    ensure!(
        repository.starts_with("https://")
            && matches!(commit.len(), 40 | 64)
            && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "dependency notice fallback must identify an HTTPS repository and full immutable source commit"
    );
    let records = provenance["files"]
        .as_array()
        .context("fallback notice file records are missing")?;
    ensure!(!records.is_empty(), "fallback notice inventory is empty");
    let mut expected = BTreeSet::from([PROVENANCE.to_owned()]);
    let mut documents = Vec::new();
    let mut has_notice = false;
    for record in records {
        let path = record["path"]
            .as_str()
            .context("fallback file path is missing")?;
        let relative = relative_path(path)?;
        has_notice |= is_notice(&relative);
        ensure!(
            is_notice(&relative)
                || matches!(
                    relative.file_name().and_then(|name| name.to_str()),
                    Some("README" | "README.md" | "README.txt")
                ),
            "fallback file is not a notice document: {path}"
        );
        ensure!(
            expected.insert(path.to_owned()),
            "duplicate fallback notice path: {path}"
        );
        let url = record["sourceUrl"]
            .as_str()
            .context("fallback notice source URL is missing")?;
        let expected_hash = record["sha256"]
            .as_str()
            .context("fallback notice hash is missing")?;
        ensure!(
            url.starts_with("https://")
                && url.contains(commit)
                && expected_hash.len() == 64
                && expected_hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "fallback notice must have a pinned source URL and SHA256 hash"
        );
        let bytes = read_document(&root, &relative, limits)?;
        ensure!(
            format!("{:x}", Sha256::digest(&bytes)) == expected_hash,
            "fallback notice hash does not match: {path}"
        );
        documents.push(Document {
            path: path.to_owned(),
            bytes,
        });
    }
    ensure!(
        has_notice,
        "fallback attribution alone does not supply a notice document"
    );
    let mut actual = BTreeSet::new();
    walk(&root, &root, 0, limits, &mut |path| {
        actual.insert(portable(path)?);
        Ok(())
    })?;
    ensure!(
        actual == expected,
        "fallback notice directory contains unverified or missing files"
    );
    documents.push(Document {
        path: PROVENANCE.into(),
        bytes: provenance_bytes,
    });
    documents.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Notices {
        input,
        origin: json!({"kind":"verified_exact_version_upstream_fallback","provenance":provenance}),
        documents,
    })
}

fn portable(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        let Component::Normal(value) = component else {
            bail!("dependency notice paths must be relative without traversal");
        };
        let value = value
            .to_str()
            .context("dependency notice path must be UTF-8")?;
        ensure!(
            !value.is_empty()
                && !value.contains(['\\', ':'])
                && !value.chars().any(char::is_control),
            "dependency notice path is not portable"
        );
        parts.push(value);
    }
    ensure!(!parts.is_empty(), "dependency notice path is empty");
    Ok(parts.join("/"))
}

fn relative_path(path: &str) -> Result<PathBuf> {
    ensure!(
        !path.is_empty()
            && !path.contains(['\\', ':'])
            && path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."),
        "dependency notice paths must be relative without traversal"
    );
    let result = PathBuf::from(path);
    portable(&result)?;
    Ok(result)
}

fn render_index(notices: &[Notices]) -> Result<String> {
    let mut text = String::from(
        "# Third-party dependency notices\n\nThis inventory retains documents from all non-workspace packages in the locked, offline, all-features Cargo workspace graph. It is a superset that includes development and other-target dependencies; it does not assert exact executable linkage or provide legal approval. Upstream license metadata and documents retain their own terms. No dependency is relicensed by the project's current original-material terms.\n\nDocuments come from resolved Cargo package sources, except explicitly identified exact-version upstream fallbacks whose pinned provenance and SHA256 hashes were verified. This inventory does not cover fonts, other non-Cargo assets, or toolchain/runtime components; their separate notices remain applicable.\n\n",
    );
    for notice in notices {
        text.push_str(&format!(
            "## {} {}\n\n",
            notice.input.name, notice.input.version
        ));
        let directory = crate_directory(&notice.input)?;
        let files = notice.documents.iter().map(|document| json!({"path":format!("{DIRECTORY}/{directory}/{}", document.path),"byteLength":document.bytes.len()})).collect::<Vec<_>>();
        let license_file = notice
            .input
            .license_file
            .as_deref()
            .map(portable)
            .transpose()?;
        let metadata = json!({"name":notice.input.name,"version":notice.input.version,"source":notice.input.source,"license":notice.input.license,"declaredLicenseFile":license_file,"noticeOrigin":notice.origin,"files":files});
        text.push_str("```json\n");
        text.push_str(&serde_json::to_string_pretty(&metadata)?);
        text.push_str("\n```\n\n");
    }
    Ok(text)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    File::options()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
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
                "fidryn-notices-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
        fn write(&self, name: &str, bytes: &[u8]) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        fn input(&self) -> CrateInput {
            CrateInput {
                name: "notice-fixture".into(),
                version: "1.2.3".into(),
                source: Some("registry+https://example.test/index".into()),
                license: Some("MIT OR Apache-2.0".into()),
                license_file: None,
                root: self.0.join("crate"),
            }
        }
        fn prepare(&self) {
            fs::create_dir_all(self.0.join("crate")).unwrap();
            fs::create_dir(self.0.join("payload")).unwrap();
        }
        fn fallback(&self) {
            let bytes = b"upstream permission and copyright\r\n";
            self.write("fallback/notice-fixture-1.2.3/LICENSE-MIT", bytes);
            let commit = "a".repeat(40);
            let value = json!({"crate":"notice-fixture","version":"1.2.3","cargoSource":"registry+https://example.test/index","sourceRepository":"https://example.test/project","sourceCommit":commit,"files":[{"path":"LICENSE-MIT","sourceUrl":format!("https://example.test/project/{commit}/LICENSE-MIT"),"sha256":format!("{:x}",Sha256::digest(bytes))}]});
            self.write(
                "fallback/notice-fixture-1.2.3/provenance.json",
                &serde_json::to_vec(&value).unwrap(),
            );
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn copies_exact_recursive_notices_and_explicit_license_without_source_code() {
        let fixture = Fixture::new();
        fixture.prepare();
        fixture.write("crate/LICENSE-MIT", b"copyright\r\npermission\r\n");
        fixture.write("crate/vendor/NOTICES.md", b"vendor copyright");
        fixture.write("crate/licenses/Apache-2.0.txt", b"Apache terms");
        fixture.write("crate/terms/custom.txt", b"explicit custom license");
        fixture.write("crate/src/lib.rs", b"pub fn implementation() {}");
        fixture.write("crate/licenses/example.rs", b"pub fn not_a_notice() {}");
        let mut input = fixture.input();
        input.license_file = Some("terms/custom.txt".into());
        copy_inputs(
            &fixture.0.join("payload"),
            vec![input],
            &fixture.0.join("fallback"),
        )
        .unwrap();
        let output = fixture
            .0
            .join("payload/THIRD_PARTY_LICENSES/notice-fixture-1.2.3");
        assert_eq!(
            fs::read(output.join("LICENSE-MIT")).unwrap(),
            b"copyright\r\npermission\r\n"
        );
        assert!(output.join("vendor/NOTICES.md").is_file());
        assert!(output.join("licenses/Apache-2.0.txt").is_file());
        assert!(output.join("terms/custom.txt").is_file());
        assert!(!output.join("src/lib.rs").exists());
        assert!(!output.join("licenses/example.rs").exists());
        let index = fs::read_to_string(fixture.0.join("payload/THIRD_PARTY_NOTICES.md")).unwrap();
        assert!(index.contains("MIT OR Apache-2.0"));
        assert!(index.contains("superset"));
    }

    #[test]
    fn actual_authors_notice_precedes_a_fallback() {
        let fixture = Fixture::new();
        fixture.prepare();
        fixture.write("crate/AUTHORS", b"copyright and license grant");
        copy_inputs(
            &fixture.0.join("payload"),
            vec![fixture.input()],
            &fixture.0.join("missing"),
        )
        .unwrap();
        assert!(
            fixture
                .0
                .join("payload/THIRD_PARTY_LICENSES/notice-fixture-1.2.3/AUTHORS")
                .is_file()
        );
    }

    #[test]
    fn absolute_license_file_is_confined_and_indexed_without_native_cache_paths() {
        let fixture = Fixture::new();
        fixture.prepare();
        fixture.write("crate/terms/custom.txt", b"explicit license terms");
        let absolute = fixture.0.join("crate/terms/custom.txt");
        #[cfg(windows)]
        let absolute = absolute
            .to_str()
            .and_then(|path| path.strip_prefix(r"\\?\"))
            .map(PathBuf::from)
            .unwrap_or(absolute);
        let mut input = fixture.input();
        input.license_file = Some(absolute);
        copy_inputs(
            &fixture.0.join("payload"),
            vec![input],
            &fixture.0.join("missing"),
        )
        .unwrap();
        let index = fs::read_to_string(fixture.0.join("payload/THIRD_PARTY_NOTICES.md")).unwrap();
        assert!(index.contains("\"declaredLicenseFile\": \"terms/custom.txt\""));
        assert!(!index.contains("fidryn-notices-"));
    }

    #[test]
    fn missing_notices_refuse_without_creating_output() {
        let fixture = Fixture::new();
        fixture.prepare();
        assert!(
            copy_inputs(
                &fixture.0.join("payload"),
                vec![fixture.input()],
                &fixture.0.join("missing")
            )
            .is_err()
        );
        assert!(!fixture.0.join("payload/THIRD_PARTY_LICENSES").exists());
    }

    #[test]
    fn verified_fallback_preserves_documents_and_provenance() {
        let fixture = Fixture::new();
        fixture.prepare();
        fixture.fallback();
        copy_inputs(
            &fixture.0.join("payload"),
            vec![fixture.input()],
            &fixture.0.join("fallback"),
        )
        .unwrap();
        let output = fixture
            .0
            .join("payload/THIRD_PARTY_LICENSES/notice-fixture-1.2.3");
        assert_eq!(
            fs::read(output.join("LICENSE-MIT")).unwrap(),
            b"upstream permission and copyright\r\n"
        );
        assert!(output.join(PROVENANCE).is_file());
    }

    #[test]
    fn fallback_retains_explicit_verified_readme_attribution() {
        let fixture = Fixture::new();
        fixture.prepare();
        fixture.fallback();
        let path = fixture
            .0
            .join("fallback/notice-fixture-1.2.3/provenance.json");
        let mut provenance: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let attribution = b"Copyright belongs to the project contributors.\n";
        let commit = "a".repeat(40);
        provenance["files"].as_array_mut().unwrap().push(json!({
            "path":"README.md",
            "sourceUrl":format!("https://example.test/project/{commit}/README.md"),
            "sha256":format!("{:x}", Sha256::digest(attribution)),
        }));
        fs::write(&path, serde_json::to_vec(&provenance).unwrap()).unwrap();
        fixture.write("fallback/notice-fixture-1.2.3/README.md", attribution);
        copy_inputs(
            &fixture.0.join("payload"),
            vec![fixture.input()],
            &fixture.0.join("fallback"),
        )
        .unwrap();
        assert_eq!(
            fs::read(
                fixture
                    .0
                    .join("payload/THIRD_PARTY_LICENSES/notice-fixture-1.2.3/README.md")
            )
            .unwrap(),
            attribution,
        );
    }

    #[test]
    fn fallback_tampering_wrong_source_and_unverified_files_refuse() {
        let fixture = Fixture::new();
        fixture.prepare();
        fixture.fallback();
        fixture.write("fallback/notice-fixture-1.2.3/LICENSE-MIT", b"tampered");
        assert!(
            collect_notices(
                fixture.input(),
                &fixture.0.join("fallback"),
                &mut Limits::default()
            )
            .is_err()
        );
        fixture.fallback();
        let mut input = fixture.input();
        input.source = Some("git+https://other.test/project".into());
        assert!(
            collect_notices(input, &fixture.0.join("fallback"), &mut Limits::default()).is_err()
        );
        fixture.write("fallback/notice-fixture-1.2.3/NOTICE", b"not verified");
        assert!(
            collect_notices(
                fixture.input(),
                &fixture.0.join("fallback"),
                &mut Limits::default()
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_missing_or_escaping_license_refuses_even_with_another_notice() {
        let fixture = Fixture::new();
        fixture.prepare();
        fixture.write("crate/LICENSE", b"notice");
        fixture.write("outside", b"outside license");
        for path in ["missing.txt", "../outside"] {
            let mut input = fixture.input();
            input.license_file = Some(path.into());
            assert!(
                collect_notices(input, &fixture.0.join("fallback"), &mut Limits::default())
                    .is_err()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_notice_and_source_directory_refuse() {
        let fixture = Fixture::new();
        fixture.prepare();
        fixture.write("outside", b"outside license");
        std::os::unix::fs::symlink(fixture.0.join("outside"), fixture.0.join("crate/LICENSE"))
            .unwrap();
        assert!(
            collect_notices(
                fixture.input(),
                &fixture.0.join("fallback"),
                &mut Limits::default()
            )
            .is_err()
        );
        fs::remove_file(fixture.0.join("crate/LICENSE")).unwrap();
        std::os::unix::fs::symlink(&fixture.0, fixture.0.join("crate/linked")).unwrap();
        assert!(
            collect_notices(
                fixture.input(),
                &fixture.0.join("fallback"),
                &mut Limits::default()
            )
            .is_err()
        );
    }

    #[test]
    fn oversized_notice_inventory_budget_and_existing_outputs_refuse() {
        let fixture = Fixture::new();
        fixture.prepare();
        File::create(fixture.0.join("crate/LICENSE"))
            .unwrap()
            .set_len(MAX_DOCUMENT_BYTES as u64 + 1)
            .unwrap();
        assert!(
            collect_notices(
                fixture.input(),
                &fixture.0.join("fallback"),
                &mut Limits::default()
            )
            .is_err()
        );
        fixture.write("crate/LICENSE", b"license");
        let mut limits = Limits {
            entries: MAX_ENTRIES,
            ..Limits::default()
        };
        assert!(
            collect_notices(fixture.input(), &fixture.0.join("fallback"), &mut limits).is_err()
        );
        fixture.write("payload/THIRD_PARTY_NOTICES.md", b"do not overwrite");
        assert!(
            copy_inputs(
                &fixture.0.join("payload"),
                vec![fixture.input()],
                &fixture.0.join("fallback")
            )
            .is_err()
        );
        assert_eq!(
            fs::read(fixture.0.join("payload/THIRD_PARTY_NOTICES.md")).unwrap(),
            b"do not overwrite"
        );
    }
}
