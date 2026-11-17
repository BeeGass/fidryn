//! Tests of the workspace check and of the writer. Every workspace here is
//! built in a temporary directory from invented models, in the invented
//! corpus `/zz/demo`, and so is every text.

use super::cache::Cache;
use super::discover::walk;
use super::{
    CheckOptions, GeneratedWriter, Unresolved, WorkspaceReport, WriteError, check_interfaces,
    check_workspace,
};
use fidryn_check::e2::lower;
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::layout::module_path;
use fidryn_provision::render::render;
use fidryn_provision::workspace::{
    AddressCounts, Artifact, Dependency, Generator, Lock, Manifest, Member, NotImported,
    RetrievalRecord, RetrievedArchive, WorkspaceKind,
};
use fidryn_provision::{
    Address, Block, ContainerModule, CuratedModule, Footnote, Hash32, Interface, Item, Layer,
    Module, Note, Num, Outcome, PartRole, Provision, Run, Text, UnitModule, interface_of,
};
use fidryn_syntax::e2::parse;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The version of the running toolchain, which the demo manifest records.
const TOOLCHAIN: &str = env!("CARGO_PKG_VERSION");

/// The identity of the running executable that the checks here give: any
/// string will do, since the driver only puts it into the keys of the cache.
const BUILD: &str = "an invented build";

const ARCHIVE_BLAKE3: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";
const ARCHIVE_SHA256: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";
const MEMBER_BLAKE3: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A directory of its own under the temporary directory, removed when
/// dropped unless `keep` is set.
struct Scratch {
    path: PathBuf,
    keep: bool,
}

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("fidryn-workspace-{}-{n}-{tag}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create the scratch directory");
        Scratch { path, keep: false }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.path.join(rel)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn address(text: &str) -> Address {
    Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn hash(text: &str) -> Hash32 {
    text.parse()
        .unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn plain(words: &str) -> Text {
    Text::from_runs(vec![Run::Words(words.to_owned())])
}

fn num(value: &str, shown: &str) -> Option<Num> {
    Some(Num {
        value: value.to_owned(),
        shown: plain(shown),
    })
}

/// A unit of one section, numbered `value`, that holds one paragraph.
fn unit(at: &str, within: Option<&str>, value: &str, heading: &str, words: &str) -> UnitModule {
    UnitModule {
        address: address(at),
        aliases: Vec::new(),
        within: within.map(address),
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        root: Provision {
            address: address(at),
            kind: "section".to_owned(),
            num: num(value, &format!("§ {value}.")),
            heading: Some(plain(heading)),
            status: None,
            items: vec![Item::Part {
                role: PartRole::Content,
                blocks: vec![Block::Para(plain(words))],
            }],
        },
        credit: None,
        notes: Vec::new(),
        footnotes: Vec::new(),
        definitions: Vec::new(),
    }
}

/// A container of kind `kind` that lists `members`.
fn container(
    at: &str,
    within: Option<&str>,
    kind: &str,
    shown: &str,
    heading: &str,
    members: &[&str],
) -> ContainerModule {
    ContainerModule {
        address: address(at),
        aliases: Vec::new(),
        within: within.map(address),
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        kind: kind.to_owned(),
        num: num("1", shown),
        heading: Some(plain(heading)),
        status: None,
        members: members.iter().map(|member| address(member)).collect(),
        credit: None,
        notes: Vec::new(),
        footnotes: Vec::new(),
    }
}

/// The models of the demo workspace: the title `/zz/demo/t1`, its chapter
/// `/zz/demo/t1/ch1`, and the three sections of the chapter, in that order.
fn demo_models() -> Vec<Module> {
    let chapter = "/zz/demo/t1/ch1";
    vec![
        Module::Container(container(
            "/zz/demo/t1",
            None,
            "title",
            "Title 1—",
            "Invented Things",
            &[chapter],
        )),
        Module::Container(container(
            chapter,
            Some("/zz/demo/t1"),
            "chapter",
            "Chapter 1—",
            "Widgets",
            &["/zz/demo/t1/s1", "/zz/demo/t1/s2", "/zz/demo/t1/s3"],
        )),
        Module::Unit(unit(
            "/zz/demo/t1/s1",
            Some(chapter),
            "1",
            "Definitions",
            "In this invented chapter, a widget is a small device.",
        )),
        Module::Unit(unit(
            "/zz/demo/t1/s2",
            Some(chapter),
            "2",
            "Sizes",
            "A widget is small or large.",
        )),
        Module::Unit(unit(
            "/zz/demo/t1/s3",
            Some(chapter),
            "3",
            "Colors",
            "A widget may be of any color.",
        )),
    ]
}

/// The container at `index` of `models`.
fn container_at(models: &mut [Module], index: usize) -> &mut ContainerModule {
    match &mut models[index] {
        Module::Container(container) => container,
        other => panic!("model {index} is no container: {other:?}"),
    }
}

/// The unit at `index` of `models`.
fn unit_at(models: &mut [Module], index: usize) -> &mut UnitModule {
    match &mut models[index] {
        Module::Unit(unit) => unit,
        other => panic!("model {index} is no unit: {other:?}"),
    }
}

fn demo_manifest() -> Manifest {
    Manifest {
        name: "demo".to_owned(),
        kind: WorkspaceKind::Generated,
        prefix: address("/zz/demo"),
        release: Some("42-7".to_owned()),
        toolchain: TOOLCHAIN.to_owned(),
        dependencies: Vec::new(),
    }
}

fn curated_manifest() -> Manifest {
    Manifest {
        name: "demo-curated".to_owned(),
        kind: WorkspaceKind::Curated,
        prefix: address("/zz/demo"),
        release: None,
        toolchain: TOOLCHAIN.to_owned(),
        dependencies: Vec::new(),
    }
}

/// The retrieval record of the invented archive the demo models come from.
fn retrieval() -> RetrievalRecord {
    RetrievalRecord {
        release: "42-7".to_owned(),
        archives: vec![RetrievedArchive {
            file: "demo01@42-7.zip".to_owned(),
            url: "https://example.invalid/releases/42/7/demo01@42-7.zip".to_owned(),
            retrieved_at: "2026-11-16T22:14:03Z".to_owned(),
            bytes: 42241,
            sha256: ARCHIVE_SHA256.to_owned(),
            blake3: hash(ARCHIVE_BLAKE3),
        }],
    }
}

/// The artifacts of the demo lock, copied from the retrieval record.
fn artifacts() -> Vec<Artifact> {
    retrieval()
        .archives
        .into_iter()
        .map(|archive| Artifact {
            file: archive.file,
            url: archive.url,
            retrieved_at: archive.retrieved_at,
            bytes: archive.bytes,
            blake3: archive.blake3,
            sha256: archive.sha256,
            members: vec![Member {
                name: "demo01.xml".to_owned(),
                bytes: 2048,
                blake3: hash(MEMBER_BLAKE3),
            }],
        })
        .collect()
}

fn generator() -> Generator {
    Generator {
        name: "demo-writer".to_owned(),
        version: TOOLCHAIN.to_owned(),
    }
}

fn create(out: &Path, manifest: Manifest) -> GeneratedWriter {
    GeneratedWriter::create(out, manifest)
        .unwrap_or_else(|error| panic!("create {}: {error}", out.display()))
}

/// Why `create` refuses `out`.
fn refused(out: &Path, manifest: Manifest) -> WriteError {
    match GeneratedWriter::create(out, manifest) {
        Ok(_) => panic!("create accepted {}", out.display()),
        Err(error) => error,
    }
}

fn finish(writer: GeneratedWriter) -> Lock {
    writer
        .finish(
            generator(),
            artifacts(),
            AddressCounts::default(),
            NotImported::default(),
        )
        .unwrap_or_else(|error| panic!("finish: {error}"))
}

/// Writes `models` into `out` with a writer of the demo manifest, from one
/// thread, and finishes the run.
fn write_workspace(out: &Path, models: &[Module]) -> Lock {
    let writer = create(out, demo_manifest());
    for model in models {
        writer
            .write(model)
            .unwrap_or_else(|error| panic!("write: {error}"));
    }
    finish(writer)
}

/// A curated workspace in `out`: its manifest, and each file of `files` at
/// its path.
fn curated_workspace(out: &Path, files: &[(&str, String)]) {
    fs::write(out.join("workspace.json"), curated_manifest().to_json()).expect("write");
    for (rel, text) in files {
        put(out, rel, text);
    }
}

/// Writes `text` to the file `rel` under `root`, and the directories above it.
fn put(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().expect("a file has a directory")).expect("create");
    fs::write(&path, text).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
}

fn read(root: &Path, rel: &str) -> String {
    let path = root.join(rel);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// An entry of a directory, as [`tree`] records it.
#[derive(Debug, PartialEq, Eq)]
enum Entry {
    Directory,
    File(Vec<u8>),
    /// A symbolic link, which is not followed, and its target.
    Link(PathBuf),
}

/// Every entry below `root`: a file with its bytes, a symbolic link with its
/// target, and a directory, whose entries follow it.
fn tree(root: &Path) -> BTreeMap<String, Entry> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, Entry>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
        for entry in entries {
            let entry = entry.expect("read an entry");
            let name = entry.file_name().into_string().expect("a UTF-8 name");
            let rel = format!("{prefix}{name}");
            let kind = entry.file_type().expect("the kind of an entry");
            if kind.is_symlink() {
                let target = fs::read_link(entry.path()).expect("read a link");
                out.insert(rel, Entry::Link(target));
            } else if kind.is_dir() {
                out.insert(rel.clone(), Entry::Directory);
                walk(&entry.path(), &format!("{rel}/"), out);
            } else {
                out.insert(
                    rel,
                    Entry::File(fs::read(entry.path()).expect("read a file")),
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, "", &mut out);
    out
}

fn options(toolchain: &str) -> CheckOptions {
    CheckOptions {
        jobs: 1,
        cache_dir: None,
        toolchain: toolchain.to_owned(),
        executable: BUILD.to_owned(),
    }
}

fn with_jobs(jobs: usize) -> CheckOptions {
    CheckOptions {
        jobs,
        ..options(TOOLCHAIN)
    }
}

fn check(root: &Path) -> WorkspaceReport {
    check_workspace(root, &options(TOOLCHAIN))
}

/// The one diagnostic of `report`.
fn only(report: &WorkspaceReport) -> &Diagnostic {
    match report.diagnostics.as_slice() {
        [diagnostic] => diagnostic,
        other => panic!("expected one diagnostic, found {other:#?}"),
    }
}

/// The span of the string `"quoted"` where it follows `before` in `text`,
/// its quotation marks included.
fn span_after(text: &str, before: &str, quoted: &str) -> Span {
    let needle = format!("{before}\"{quoted}\"");
    let at = text
        .find(&needle)
        .unwrap_or_else(|| panic!("{needle:?} is not in:\n{text}"))
        + before.len();
    let start = u32::try_from(at).expect("a small file");
    let end = start + u32::try_from(quoted.len() + 2).expect("a short string");
    Span { start, end }
}

fn assert_diagnostic(
    diagnostic: &Diagnostic,
    code: DiagnosticCode,
    file: &str,
    span: Option<Span>,
    words: &str,
) {
    assert_eq!(diagnostic.code, code, "{diagnostic:#?}");
    assert_eq!(
        diagnostic.source_file.as_deref(),
        Some(file),
        "{diagnostic:#?}"
    );
    assert_eq!(diagnostic.primary_span, span, "{diagnostic:#?}");
    assert!(
        diagnostic.message.contains(words),
        "{words:?} is not in {:?}",
        diagnostic.message
    );
}

#[test]
fn a_workspace_from_the_writer_checks_clean() {
    let scratch = Scratch::new("clean");
    let lock = write_workspace(scratch.path(), &demo_models());
    assert_eq!(
        lock.modules.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "zz/demo/t1.fr",
            "zz/demo/t1/ch1.fr",
            "zz/demo/t1/s1.fr",
            "zz/demo/t1/s2.fr",
            "zz/demo/t1/s3.fr"
        ]
    );
    for (rel, digest) in &lock.modules {
        let bytes = fs::read(scratch.join(rel)).expect("a written module");
        assert_eq!(*digest, Hash32(*blake3::hash(&bytes).as_bytes()), "{rel}");
    }
    assert_eq!(read(scratch.path(), "law.lock.json"), lock.to_json());
    assert_eq!(
        read(scratch.path(), "workspace.json"),
        demo_manifest().to_json()
    );
    let report = check(scratch.path());
    assert_eq!(report.diagnostics, []);
    assert_eq!(
        report.summary(),
        "modules: 5; from cache: 0; errors: 0; warnings: 0"
    );
}

/// The entries of `tree` that are a workspace's own: those with no segment
/// whose name begins with a dot, which the writer leaves alone, such as the
/// cache of a check.
fn own(tree: BTreeMap<String, Entry>) -> BTreeMap<String, Entry> {
    tree.into_iter()
        .filter(|(rel, _)| !rel.split('/').any(|segment| segment.starts_with('.')))
        .collect()
}

/// The demo workspace under `conformance/fixtures/workspace` is what the
/// writer writes from the demo models, beside the retrieval record of its
/// archive. The schemas of the three files are checked on it there. Entries
/// whose names begin with a dot, which the writer leaves alone, are not
/// compared: a check of the fixture with its default cache leaves one.
#[test]
fn the_committed_demo_workspace_is_what_the_writer_writes() {
    let mut scratch = Scratch::new("fixture");
    write_workspace(&scratch.join("demo"), &demo_models());
    put(
        scratch.path(),
        "archives/retrieval.json",
        &retrieval().to_json(),
    );
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/workspace");
    if !fixture.is_dir() || own(tree(scratch.path())) != own(tree(&fixture)) {
        scratch.keep = true;
        panic!(
            "conformance/fixtures/workspace is not what the writer writes from the demo models; \
             what it writes is in {}",
            scratch.path().display()
        );
    }
}

#[test]
fn writing_twice_gives_the_same_bytes() {
    let one = Scratch::new("one-thread");
    let eight = Scratch::new("eight-threads");
    let models = demo_models();
    let first = write_workspace(one.path(), &models);

    let writer = create(eight.path(), demo_manifest());
    std::thread::scope(|scope| {
        for worker in 0..8 {
            let writer = &writer;
            let models = &models;
            scope.spawn(move || {
                for model in models.iter().rev().skip(worker).step_by(8) {
                    writer
                        .write(model)
                        .unwrap_or_else(|error| panic!("write: {error}"));
                }
            });
        }
    });
    let second = finish(writer);
    assert_eq!(first, second);
    assert_eq!(tree(one.path()), tree(eight.path()));
}

#[test]
fn the_writer_removes_what_is_gone_and_refuses_a_foreign_directory() {
    let scratch = Scratch::new("rerun");
    write_workspace(scratch.path(), &demo_models());
    assert!(scratch.join("zz/demo/t1/s3.fr").is_file());

    // A second run without the third section, which its chapter no longer
    // lists.
    let mut models = demo_models();
    models.pop();
    container_at(&mut models, 1).members.pop();
    let lock = write_workspace(scratch.path(), &models);
    assert!(!scratch.join("zz/demo/t1/s3.fr").exists());
    assert!(!lock.modules.contains_key("zz/demo/t1/s3.fr"));
    assert_eq!(lock.modules.len(), 4);
    assert_eq!(
        Lock::from_json(&read(scratch.path(), "law.lock.json")),
        Ok(lock)
    );
    assert_eq!(check(scratch.path()).diagnostics, []);

    // A run keeps `coverage.json` at the top and every directory whose name
    // begins with a dot.
    put(scratch.path(), "coverage.json", "{}\n");
    put(scratch.path(), ".cache/v1/entry.json", "{}\n");
    put(scratch.path(), "zz/.drafts/s9.fr", "An invented draft.\n");
    write_workspace(scratch.path(), &models);
    assert_eq!(read(scratch.path(), "coverage.json"), "{}\n");
    assert_eq!(read(scratch.path(), ".cache/v1/entry.json"), "{}\n");
    assert_eq!(
        read(scratch.path(), "zz/.drafts/s9.fr"),
        "An invented draft.\n"
    );

    // A directory that holds `notes.txt`, beside nothing or beside a
    // generated workspace of the same prefix.
    let foreign = Scratch::new("foreign");
    put(foreign.path(), "notes.txt", "Invented notes.\n");
    put(scratch.path(), "zz/demo/notes.txt", "Invented notes.\n");
    for (dir, named) in [
        (foreign.path(), "`notes.txt`"),
        (scratch.path(), "`zz/demo/notes.txt`"),
    ] {
        let before = tree(dir);
        let error = refused(dir, demo_manifest());
        assert!(
            matches!(&error, WriteError::Foreign { found, .. } if found.contains(named)),
            "{error}"
        );
        assert_eq!(tree(dir), before, "{}", dir.display());
    }

    // A generated workspace of another prefix.
    let other = Scratch::new("other-prefix");
    let mut manifest = demo_manifest();
    manifest.prefix = address("/zz/other");
    let writer = create(other.path(), manifest);
    writer
        .write(&Module::Unit(unit(
            "/zz/other/s1",
            None,
            "1",
            "Other",
            "An invented section of another corpus.",
        )))
        .expect("write");
    finish(writer);
    let before = tree(other.path());
    let error = refused(other.path(), demo_manifest());
    assert!(
        matches!(&error, WriteError::Foreign { found, .. } if found.contains("`/zz/other`")),
        "{error}"
    );
    assert_eq!(tree(other.path()), before);

    // A curated workspace.
    let curated = Scratch::new("curated");
    curated_workspace(curated.path(), &[]);
    let before = tree(curated.path());
    let error = refused(curated.path(), demo_manifest());
    assert!(
        matches!(&error, WriteError::Foreign { found, .. } if found.contains("curated")),
        "{error}"
    );
    assert_eq!(tree(curated.path()), before);
}

#[test]
fn a_module_written_twice_in_one_run_and_one_no_generated_workspace_holds_are_refused() {
    let scratch = Scratch::new("twice");
    let models = demo_models();
    let writer = create(scratch.path(), demo_manifest());
    writer.write(&models[2]).expect("write");
    let error = writer.write(&models[2]).expect_err("written twice");
    assert!(
        matches!(&error, WriteError::Twice { path } if path == "zz/demo/t1/s1.fr"),
        "{error}"
    );
    let curated = Module::Curated(CuratedModule {
        address: address("/zz/demo/c1"),
        imports: Vec::new(),
        citations: Vec::new(),
        bindings: Vec::new(),
    });
    let outside = Module::Unit(unit(
        "/zz/other/s1",
        None,
        "1",
        "Other",
        "An invented section of another corpus.",
    ));
    for module in [curated, outside] {
        let error = writer.write(&module).expect_err("refused");
        assert!(matches!(error, WriteError::Module(_)), "{error}");
    }
    assert!(!scratch.join("zz/demo/c1.fr").exists());
    assert!(!scratch.join("zz/other").exists());
    finish(writer);

    // `create` writes only a generated workspace with a release and no
    // dependencies.
    let empty = Scratch::new("not-generated");
    let mut no_release = demo_manifest();
    no_release.release = None;
    let mut with_dependency = demo_manifest();
    with_dependency.dependencies.push(Dependency {
        name: "other".to_owned(),
        path: "../other".to_owned(),
        release: "42-7".to_owned(),
        lock_digest: hash(MEMBER_BLAKE3),
    });
    for manifest in [curated_manifest(), no_release, with_dependency] {
        let error = refused(empty.path(), manifest);
        assert!(matches!(error, WriteError::Manifest(_)), "{error}");
    }
    assert_eq!(tree(empty.path()), BTreeMap::new());
    put(empty.path(), "file", "An invented file.\n");
    let error = refused(&empty.join("file"), demo_manifest());
    assert!(
        matches!(&error, WriteError::Io { error, .. } if error.kind() == std::io::ErrorKind::NotADirectory),
        "{error}"
    );
}

#[test]
fn an_interrupted_run_leaves_a_workspace_the_next_run_accepts() {
    // A first run that stops after it wrote a module, before `finish`: the
    // directory holds the manifest, which `create` wrote at once, the module
    // and no lock, and the check says so.
    let scratch = Scratch::new("interrupted");
    let out = scratch.join("out");
    let alone = Module::Unit(unit(
        "/zz/demo/t9",
        None,
        "9",
        "Alone",
        "An invented section that no container lists.",
    ));
    {
        let writer = create(&out, demo_manifest());
        writer.write(&alone).expect("write");
    }
    assert_eq!(read(&out, "workspace.json"), demo_manifest().to_json());
    assert!(out.join("zz/demo/t9.fr").is_file());
    assert!(!out.join(".fidryn-write").exists());
    assert_diagnostic(
        only(&check(&out)),
        DiagnosticCode::E730,
        "law.lock.json",
        None,
        "has a lock",
    );

    // A run that was killed outright also leaves its temporary files, which
    // the next run clears; a complete run puts the workspace right.
    put(&out, ".fidryn-write/7.tmp", "An invented leftover.\n");
    let lock = write_workspace(&out, &demo_models());
    assert!(!out.join(".fidryn-write").exists());
    assert!(!out.join("zz/demo/t9.fr").exists());
    assert_eq!(lock.modules.len(), 5);
    assert_eq!(check(&out).diagnostics, []);

    // A later run of another release that stops before `finish` leaves the
    // new manifest beside the lock of the run before it.
    let mut next = demo_manifest();
    next.release = Some("42-8".to_owned());
    drop(create(&out, next.clone()));
    assert_eq!(read(&out, "workspace.json"), next.to_json());
    assert_diagnostic(
        only(&check(&out)),
        DiagnosticCode::E730,
        "law.lock.json",
        None,
        "`42-7`",
    );
    write_workspace(&out, &demo_models());
    assert_eq!(check(&out).diagnostics, []);
}

#[test]
#[allow(non_snake_case)]
fn each_layout_defect_is_E730_and_names_the_file() {
    let models = demo_models();
    let generated = |tag: &str| {
        let scratch = Scratch::new(tag);
        write_workspace(scratch.path(), &models);
        scratch
    };

    // A module changed after it was written.
    let scratch = generated("changed");
    let text = read(scratch.path(), "zz/demo/t1/s2.fr");
    let text = text.replacen("edition 2\n", "edition 2\n// An invented comment.\n", 1);
    put(scratch.path(), "zz/demo/t1/s2.fr", &text);
    let report = check(scratch.path());
    assert_diagnostic(
        only(&report),
        DiagnosticCode::E730,
        "zz/demo/t1/s2.fr",
        None,
        "the lock",
    );

    // A listed module deleted. Its chapter lists it, and is not also
    // reported for that.
    let scratch = generated("deleted");
    fs::remove_file(scratch.join("zz/demo/t1/s3.fr")).expect("remove");
    let report = check(scratch.path());
    assert_eq!(report.modules, 4);
    assert_diagnostic(
        only(&report),
        DiagnosticCode::E730,
        "zz/demo/t1/s3.fr",
        None,
        "missing",
    );

    // A module file that the lock does not list.
    let scratch = generated("unlisted");
    let extra = Module::Unit(unit(
        "/zz/demo/t2",
        None,
        "2",
        "Extra",
        "An invented section that no run wrote.",
    ));
    put(scratch.path(), "zz/demo/t2.fr", &render(&extra));
    let report = check(scratch.path());
    assert_diagnostic(
        only(&report),
        DiagnosticCode::E730,
        "zz/demo/t2.fr",
        None,
        "not listed",
    );

    // A generated workspace with no lock.
    let scratch = generated("no-lock");
    fs::remove_file(scratch.join("law.lock.json")).expect("remove");
    let report = check(scratch.path());
    assert_eq!(report.modules, 5);
    assert_diagnostic(
        only(&report),
        DiagnosticCode::E730,
        "law.lock.json",
        None,
        "has a lock",
    );

    // A lock of another release than the manifest's.
    let scratch = generated("lock-release");
    let text = read(scratch.path(), "law.lock.json").replacen("\"42-7\"", "\"42-8\"", 1);
    put(scratch.path(), "law.lock.json", &text);
    assert_diagnostic(
        only(&check(scratch.path())),
        DiagnosticCode::E730,
        "law.lock.json",
        None,
        "`42-8`",
    );

    // Manifests that are not well formed, or not of their kind. The check
    // reads nothing else of a workspace whose manifest it cannot read.
    let unknown_field = |text: &str| text.replacen("{\n", "{\n  \"extra\": 1,\n", 1);
    let as_array = |_: &str| {
        let manifest = demo_manifest();
        format!(
            "[\"fidryn.workspace/v0.1\", \"demo\", \"generated\", \"/zz/demo\", \"42-7\", \"{}\", []]\n",
            manifest.toolchain
        )
    };
    for (tag, change, words) in [
        (
            "unknown-field",
            &unknown_field as &dyn Fn(&str) -> String,
            "unknown field `extra`",
        ),
        ("array", &as_array, "is an array where"),
    ] {
        let scratch = generated(tag);
        let text = change(&read(scratch.path(), "workspace.json"));
        put(scratch.path(), "workspace.json", &text);
        let report = check(scratch.path());
        assert_eq!(report.modules, 0, "{tag}");
        assert_diagnostic(
            only(&report),
            DiagnosticCode::E730,
            "workspace.json",
            None,
            words,
        );
    }
    let mut no_release = demo_manifest();
    no_release.release = None;
    let mut with_dependency = demo_manifest();
    with_dependency.dependencies.push(Dependency {
        name: "other".to_owned(),
        path: "../other".to_owned(),
        release: "42-7".to_owned(),
        lock_digest: hash(MEMBER_BLAKE3),
    });
    for (tag, manifest, words) in [
        ("no-release", no_release, "has a `release`"),
        ("dependency", with_dependency, "no dependencies"),
    ] {
        let scratch = generated(tag);
        put(scratch.path(), "workspace.json", &manifest.to_json());
        let report = check(scratch.path());
        assert_diagnostic(
            only(&report),
            DiagnosticCode::E730,
            "workspace.json",
            None,
            words,
        );
    }

    // In a curated workspace: a module that is not at the path of its
    // address, and one whose address is outside the prefix.
    let curated = |at: &str| {
        render(&Module::Curated(CuratedModule {
            address: address(at),
            imports: Vec::new(),
            citations: Vec::new(),
            bindings: Vec::new(),
        }))
    };
    for (tag, at, rel, words) in [
        (
            "misplaced",
            "/zz/demo/c1",
            "zz/demo/c2.fr",
            "`zz/demo/c1.fr`",
        ),
        ("outside", "/zz/other/c1", "zz/other/c1.fr", "`/zz/demo`"),
    ] {
        let scratch = Scratch::new(tag);
        let text = curated(at);
        curated_workspace(scratch.path(), &[(rel, text.clone())]);
        let report = check(scratch.path());
        assert_diagnostic(
            only(&report),
            DiagnosticCode::E730,
            rel,
            Some(span_after(&text, "module ", at)),
            words,
        );
    }

    // A first-edition `.fr` file.
    let scratch = Scratch::new("first-edition");
    curated_workspace(
        scratch.path(),
        &[
            ("zz/demo/c1.fr", curated("/zz/demo/c1")),
            (
                "zz/demo/old.fr",
                "module Demo.Old version \"0.1.0\" {\n}\n".to_owned(),
            ),
        ],
    );
    let report = check(scratch.path());
    assert_eq!(report.modules, 2);
    assert_diagnostic(
        only(&report),
        DiagnosticCode::E730,
        "zz/demo/old.fr",
        None,
        "first edition",
    );

    // A symbolic link, which the check does not follow.
    #[cfg(unix)]
    {
        let scratch = Scratch::new("link");
        curated_workspace(scratch.path(), &[("zz/demo/c1.fr", curated("/zz/demo/c1"))]);
        std::os::unix::fs::symlink("c1.fr", scratch.join("zz/demo/c2.fr")).expect("link");
        let report = check(scratch.path());
        assert_eq!(report.modules, 1);
        assert_diagnostic(
            only(&report),
            DiagnosticCode::E730,
            "zz/demo/c2.fr",
            None,
            "symbolic link",
        );
    }

    // A curated workspace with a lock, and one with a release.
    let scratch = Scratch::new("curated-lock");
    curated_workspace(scratch.path(), &[("zz/demo/c1.fr", curated("/zz/demo/c1"))]);
    let lock = write_workspace(&scratch.join("generated"), &models);
    put(scratch.path(), "law.lock.json", &lock.to_json());
    fs::remove_dir_all(scratch.join("generated")).expect("remove");
    assert_diagnostic(
        only(&check(scratch.path())),
        DiagnosticCode::E730,
        "law.lock.json",
        None,
        "no lock",
    );
    let scratch = Scratch::new("curated-release");
    curated_workspace(scratch.path(), &[("zz/demo/c1.fr", curated("/zz/demo/c1"))]);
    let mut manifest = curated_manifest();
    manifest.release = Some("42-7".to_owned());
    put(scratch.path(), "workspace.json", &manifest.to_json());
    assert_diagnostic(
        only(&check(scratch.path())),
        DiagnosticCode::E730,
        "workspace.json",
        None,
        "no `release`",
    );
}

/// A change to the demo models that gives one `E720`: in `file`, at the
/// string `quoted` after `before`, with `words` in its message.
struct HierarchyCase {
    what: &'static str,
    change: fn(&mut [Module]),
    file: &'static str,
    before: &'static str,
    quoted: &'static str,
    words: &'static str,
}

#[test]
#[allow(non_snake_case)]
fn hierarchy_defects_are_E720() {
    let chapter = "zz/demo/t1/ch1.fr";
    let section = "zz/demo/t1/s3.fr";
    let cases = [
        HierarchyCase {
            what: "a `within` that names no module",
            change: |models| {
                container_at(models, 1).members.pop();
                unit_at(models, 4).within = Some(address("/zz/demo/t1/ch9"));
            },
            file: section,
            before: "within ",
            quoted: "/zz/demo/t1/ch9",
            words: "no module",
        },
        HierarchyCase {
            what: "a `within` that names a unit",
            change: |models| {
                container_at(models, 1).members.pop();
                unit_at(models, 4).within = Some(address("/zz/demo/t1/s1"));
            },
            file: section,
            before: "within ",
            quoted: "/zz/demo/t1/s1",
            words: "a unit",
        },
        HierarchyCase {
            what: "a unit its container does not list",
            change: |models| {
                container_at(models, 1).members.pop();
            },
            file: section,
            before: "within ",
            quoted: "/zz/demo/t1/ch1",
            words: "does not list",
        },
        HierarchyCase {
            what: "a member that is no module",
            change: |models| {
                container_at(models, 1)
                    .members
                    .push(address("/zz/demo/t1/s9"));
            },
            file: chapter,
            before: "member ",
            quoted: "/zz/demo/t1/s9",
            words: "no module",
        },
        HierarchyCase {
            what: "a member whose `within` is another container",
            change: |models| {
                container_at(models, 0)
                    .members
                    .push(address("/zz/demo/t1/s3"));
                unit_at(models, 4).within = Some(address("/zz/demo/t1"));
            },
            file: chapter,
            before: "member ",
            quoted: "/zz/demo/t1/s3",
            words: "within `/zz/demo/t1`",
        },
    ];
    for case in &cases {
        assert_hierarchy_case(case);
    }
}

/// Writes the demo models with `case`'s change and checks that the one
/// diagnostic of the workspace is the `E720` the case gives.
fn assert_hierarchy_case(case: &HierarchyCase) {
    let scratch = Scratch::new("hierarchy");
    let mut models = demo_models();
    (case.change)(&mut models);
    write_workspace(scratch.path(), &models);
    let report = check(scratch.path());
    assert_eq!(report.diagnostics.len(), 1, "{}: {report:#?}", case.what);
    let text = read(scratch.path(), case.file);
    assert_diagnostic(
        only(&report),
        DiagnosticCode::E720,
        case.file,
        Some(span_after(&text, case.before, case.quoted)),
        case.words,
    );
}

#[test]
fn a_within_that_names_an_alias_is_reported_at_the_within() {
    assert_hierarchy_case(&HierarchyCase {
        what: "a `within` that names an alias of a container",
        change: |models| {
            let chapter = container_at(models, 1);
            chapter.aliases.push(address("/zz/demo/t1/chI"));
            chapter.members.pop();
            unit_at(models, 4).within = Some(address("/zz/demo/t1/chI"));
        },
        file: "zz/demo/t1/s3.fr",
        before: "within ",
        quoted: "/zz/demo/t1/chI",
        words: "an alias of the module `/zz/demo/t1/ch1`",
    });
}

#[test]
fn a_member_with_no_within_is_reported_at_the_member() {
    assert_hierarchy_case(&HierarchyCase {
        what: "a member with no `within`",
        change: |models| {
            unit_at(models, 4).within = None;
        },
        file: "zz/demo/t1/ch1.fr",
        before: "member ",
        quoted: "/zz/demo/t1/s3",
        words: "has no `within`",
    });
}

#[test]
fn a_within_that_names_a_curated_module_is_reported_at_the_within() {
    // A curated workspace may hold units beside its curated modules.
    let scratch = Scratch::new("within-curated");
    let curated = render(&Module::Curated(CuratedModule {
        address: address("/zz/demo/c1"),
        imports: Vec::new(),
        citations: Vec::new(),
        bindings: Vec::new(),
    }));
    let section = render(&Module::Unit(unit(
        "/zz/demo/s1",
        Some("/zz/demo/c1"),
        "1",
        "Within",
        "An invented section within a curated module.",
    )));
    curated_workspace(
        scratch.path(),
        &[
            ("zz/demo/c1.fr", curated),
            ("zz/demo/s1.fr", section.clone()),
        ],
    );
    assert_diagnostic(
        only(&check(scratch.path())),
        DiagnosticCode::E720,
        "zz/demo/s1.fr",
        Some(span_after(&section, "within ", "/zz/demo/c1")),
        "which is a curated module, not a container",
    );
}

#[test]
fn a_lock_that_is_no_lock_is_reported_at_the_lock() {
    let scratch = Scratch::new("lock-no-lock");
    write_workspace(scratch.path(), &demo_models());
    put(scratch.path(), "law.lock.json", "{}\n");
    assert_diagnostic(
        only(&check(scratch.path())),
        DiagnosticCode::E730,
        "law.lock.json",
        None,
        "this file is no lock of a workspace: missing field",
    );
}

#[test]
fn a_lock_of_another_prefix_is_reported_at_the_lock() {
    let scratch = Scratch::new("lock-prefix");
    write_workspace(scratch.path(), &demo_models());
    let text = read(scratch.path(), "law.lock.json").replacen(
        "\"prefix\": \"/zz/demo\"",
        "\"prefix\": \"/zz/other\"",
        1,
    );
    put(scratch.path(), "law.lock.json", &text);
    assert_diagnostic(
        only(&check(scratch.path())),
        DiagnosticCode::E730,
        "law.lock.json",
        None,
        "the lock is of the prefix `/zz/other`, and the manifest of `/zz/demo`",
    );
}

#[test]
fn a_directory_whose_name_begins_with_a_dot_is_not_read() {
    let scratch = Scratch::new("dot");
    write_workspace(scratch.path(), &demo_models());
    put(
        scratch.path(),
        ".hidden/broken.fr",
        "An invented file that is no module.\n",
    );
    put(
        scratch.path(),
        "zz/demo/t1/.drafts/s9.fr",
        "module Demo.Draft version \"0.1.0\" {\n}\n",
    );
    let report = check(scratch.path());
    assert_eq!(report.diagnostics, []);
    assert_eq!(report.modules, 5);
}

#[test]
fn a_file_or_a_link_whose_name_begins_with_a_dot_is_not_read() {
    let scratch = Scratch::new("dot-entries");
    write_workspace(scratch.path(), &demo_models());
    put(scratch.path(), ".gitignore", ".fidryn-cache/\n");
    put(
        scratch.path(),
        "zz/demo/t1/.s9.fr",
        "An invented file that is no module.\n",
    );
    // A cache on another disk, and the lock link of an editor, whose target
    // does not exist.
    #[cfg(unix)]
    let elsewhere = Scratch::new("dot-entries-cache");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(elsewhere.path(), scratch.join(".fidryn-cache")).expect("link");
        std::os::unix::fs::symlink("someone@host.1234", scratch.join("zz/demo/t1/.#s1.fr"))
            .expect("link");
    }
    let report = check(scratch.path());
    assert_eq!(report.diagnostics, []);
    assert_eq!(report.modules, 5);
}

#[test]
fn a_writer_leaves_names_that_begin_with_a_dot_alone() {
    // A directory that holds nothing but such names is empty to `create`,
    // as the working directory of a new repository is.
    let scratch = Scratch::new("fresh-repository");
    put(scratch.path(), ".git/HEAD", "ref: refs/heads/main\n");
    put(scratch.path(), ".gitignore", ".fidryn-cache/\n");
    write_workspace(scratch.path(), &demo_models());
    assert_eq!(check(scratch.path()).diagnostics, []);

    // Files and links of such names anywhere are kept by a later run, which
    // removes the module it does not write.
    put(scratch.path(), "zz/demo/t1/.s9.fr", "An invented draft.\n");
    #[cfg(unix)]
    let elsewhere = Scratch::new("fresh-repository-cache");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(elsewhere.path(), scratch.join(".fidryn-cache")).expect("link");
        std::os::unix::fs::symlink("someone@host.1234", scratch.join("zz/demo/t1/.#s1.fr"))
            .expect("link");
    }
    let kept: Vec<(String, Entry)> = tree(scratch.path())
        .into_iter()
        .filter(|(rel, _)| rel.starts_with('.') || rel.contains("/."))
        .collect();
    let mut models = demo_models();
    models.pop();
    container_at(&mut models, 1).members.pop();
    write_workspace(scratch.path(), &models);
    assert!(!scratch.join("zz/demo/t1/s3.fr").exists());
    let now = tree(scratch.path());
    for (rel, entry) in &kept {
        assert_eq!(now.get(rel), Some(entry), "{rel}");
    }
    assert_eq!(check(scratch.path()).diagnostics, []);

    // A link of any other name is refused, and nothing is changed.
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("s1.fr", scratch.join("zz/demo/t1/s4.fr")).expect("link");
        let before = tree(scratch.path());
        let error = refused(scratch.path(), demo_manifest());
        assert!(
            matches!(&error, WriteError::Foreign { found, .. } if found.contains("`zz/demo/t1/s4.fr`")),
            "{error}"
        );
        assert_eq!(tree(scratch.path()), before);
    }
}

#[test]
fn a_toolchain_mismatch_is_a_warning() {
    let scratch = Scratch::new("toolchain");
    write_workspace(scratch.path(), &demo_models());
    let report = check_workspace(scratch.path(), &options("0.0.0-invented"));
    assert_diagnostic(
        only(&report),
        DiagnosticCode::W760,
        "workspace.json",
        None,
        &format!("`{TOOLCHAIN}`"),
    );
    assert!(only(&report).message.contains("`0.0.0-invented`"));
    assert_eq!(report.errors(), 0);
    assert_eq!(report.warnings(), 1);
    assert_eq!(
        report.summary(),
        "modules: 5; from cache: 0; errors: 0; warnings: 1"
    );
}

#[test]
fn diagnostics_are_sorted_by_path_then_position_and_name_their_file() {
    let scratch = Scratch::new("sorted");
    let mut models = demo_models();
    container_at(&mut models, 1)
        .members
        .push(address("/zz/demo/t1/s9"));
    write_workspace(scratch.path(), &models);
    // A word of the second section changed: the file differs from the lock,
    // and its hash and its operative layer from its text.
    let text = read(scratch.path(), "zz/demo/t1/s2.fr").replacen("large", "huge", 1);
    put(scratch.path(), "zz/demo/t1/s2.fr", &text);
    // A module that the lock does not list, before the others by its path.
    let extra = Module::Unit(unit(
        "/zz/demo/a",
        None,
        "9",
        "Extra",
        "An invented section that no run wrote.",
    ));
    put(scratch.path(), "zz/demo/a.fr", &render(&extra));

    let report = check_workspace(scratch.path(), &options("0.0.0-invented"));
    let found: Vec<(Option<&str>, DiagnosticCode)> = report
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.source_file.as_deref(), diagnostic.code))
        .collect();
    assert_eq!(
        found,
        [
            (Some("workspace.json"), DiagnosticCode::W760),
            (Some("zz/demo/a.fr"), DiagnosticCode::E730),
            (Some("zz/demo/t1/ch1.fr"), DiagnosticCode::E720),
            (Some("zz/demo/t1/s2.fr"), DiagnosticCode::E730),
            (Some("zz/demo/t1/s2.fr"), DiagnosticCode::E710),
            (Some("zz/demo/t1/s2.fr"), DiagnosticCode::E710),
        ],
        "{report:#?}"
    );
    let position = |diagnostic: &Diagnostic| {
        (
            diagnostic.source_file.clone(),
            diagnostic.primary_span.map(|span| (span.start, span.end)),
        )
    };
    assert!(
        report
            .diagnostics
            .windows(2)
            .all(|pair| position(&pair[0]) <= position(&pair[1])),
        "{report:#?}"
    );
    let starts: Vec<Option<u32>> = report.diagnostics[3..]
        .iter()
        .map(|diagnostic| diagnostic.primary_span.map(|span| span.start))
        .collect();
    assert_eq!(starts[0], None);
    assert!(starts[1].is_some() && starts[1] < starts[2], "{starts:?}");
    assert_eq!(report.errors(), 5);
    assert_eq!(report.warnings(), 1);
}

#[test]
fn a_directory_with_no_manifest_is_refused() {
    let scratch = Scratch::new("no-manifest");
    let unit = Module::Unit(unit(
        "/zz/demo/t1/s1",
        None,
        "1",
        "Alone",
        "An invented section in no workspace.",
    ));
    put(scratch.path(), "zz/demo/t1/s1.fr", &render(&unit));
    let report = check(scratch.path());
    assert_eq!(report.modules, 0);
    assert_diagnostic(
        only(&report),
        DiagnosticCode::E730,
        "workspace.json",
        None,
        "no `workspace.json` was found",
    );
    assert_eq!(
        report.summary(),
        "modules: 0; from cache: 0; errors: 1; warnings: 0"
    );
}

/// A text that links to each of `targets`, in order.
fn linked(targets: &[&str]) -> Text {
    let mut runs = vec![Run::Words("See ".to_owned())];
    for (n, target) in targets.iter().enumerate() {
        if n > 0 {
            runs.push(Run::Words(", ".to_owned()));
        }
        runs.push(Run::Ref {
            words: format!("reference {}", n + 1),
            target: (*target).to_owned(),
        });
    }
    runs.push(Run::Words(".".to_owned()));
    Text::from_runs(runs)
}

/// A paragraph that links to each of `targets`, in order.
fn linking(targets: &[&str]) -> Block {
    Block::Para(linked(targets))
}

/// A note of the module `module`, named `name`, in `layer`, that links to
/// each of `targets`.
fn linking_note(module: &str, name: &str, layer: Layer, targets: &[&str]) -> Note {
    Note {
        address: address(&format!("{module}/note/demoNotes/nolaw/{name}")),
        layer,
        group: "demoNotes".to_owned(),
        topics: Vec::new(),
        heading: Some(plain("An Invented Note")),
        blocks: vec![linking(targets)],
    }
}

/// A subsection `a` of the provision at `parent`, holding `blocks`.
fn subsection(parent: &str, blocks: Vec<Block>) -> Item {
    Item::Child(Provision {
        address: address(&format!("{parent}/a")),
        kind: "subsection".to_owned(),
        num: num("a", "(a)"),
        heading: None,
        status: None,
        items: vec![Item::Part {
            role: PartRole::Content,
            blocks,
        }],
    })
}

/// The targets of the references of the section `s6` of the outcome
/// workspace, each with its outcome, as `each_outcome_from_a_fixture` of
/// `fidryn-provision` gives them. The section links to the first nine in
/// its own text and to the others in its subsection `a`.
fn outcome_targets() -> [(&'static str, Outcome); 18] {
    let status = |word: &str| Outcome::TargetStatus(word.to_owned());
    let outside = |body: &str| Outcome::Outside(body.to_owned());
    [
        ("/zz/demo/t1/s1/a", Outcome::Exact),
        ("/zz/demo/t1/s1", Outcome::Exact),
        ("/zz/demo/t1/ch1", Outcome::Exact),
        ("/zz/demo/t1/s2", status("repealed")),
        ("/zz/demo/t1/s4", status("omitted")),
        ("/zz/demo/t1/s5", Outcome::Ambiguous),
        ("/zz/demo/t1/s5/a", Outcome::Ambiguous),
        ("/zz/demo/t1/s1/z", Outcome::UnitOnly),
        ("/zz/demo/t1/s2/a", Outcome::UnitOnly),
        ("/zz/demo/t1/s1\u{2013}9", Outcome::RangeForm),
        ("/zz/demo/t1/s1\u{2013}9/b", Outcome::RangeForm),
        ("/zz/demo/t1/s9", Outcome::NoUnit),
        ("/zz/demo/t9/s1", Outcome::NoUnit),
        ("/zz/demo/t1/s8\u{2013}9", Outcome::NoUnit),
        ("/zz/other/x", outside("/zz/other")),
        ("/yy/stat/116/926", outside("/yy/stat")),
        ("not an address", Outcome::Malformed),
        ("/zz/demo/t1/s1/a ", Outcome::Malformed),
    ]
}

const CHAPTER: &str = "/zz/demo/t1/ch1";
const S6: &str = "/zz/demo/t1/s6";
const AMENDMENT: &str = "/zz/demo/t1/s6/note/demoNotes/nolaw/amendment";
const CODIFICATION: &str = "/zz/demo/t1/s6/note/demoNotes/nolaw/codification";

/// The outcome workspace: the title `t1`, its chapter `ch1`, which lists
/// the units `s1` (with a subsection `a`), `s2` (repealed), `s3` (omitted,
/// with the alias `s4`), `s5` (with a subsection `a`), `s5~2` and `s6`.
/// `s6` links to every target of [`outcome_targets`] in operative text, and
/// to others in an enacted note, an editorial note, its credit and a
/// footnote; the chapter's credit holds one link too.
fn outcome_models() -> Vec<Module> {
    let sections = [
        "/zz/demo/t1/s1",
        "/zz/demo/t1/s2",
        "/zz/demo/t1/s3",
        "/zz/demo/t1/s5",
        "/zz/demo/t1/s5~2",
        S6,
    ];
    let mut chapter = container(
        CHAPTER,
        Some("/zz/demo/t1"),
        "chapter",
        "Chapter 1—",
        "Widgets",
        &sections,
    );
    chapter.credit = Some(linked(&["/yy/stat/116/926"]));
    let section = |at: &str, value: &str| {
        unit(
            at,
            Some(CHAPTER),
            value,
            "Invented",
            "An invented section of the chapter.",
        )
    };
    let mut s1 = section("/zz/demo/t1/s1", "1");
    s1.root.items.push(subsection(
        "/zz/demo/t1/s1",
        vec![Block::Para(plain("An invented subsection."))],
    ));
    let mut s2 = section("/zz/demo/t1/s2", "2");
    s2.root.status = Some("repealed".to_owned());
    let mut s3 = section("/zz/demo/t1/s3", "3");
    s3.root.status = Some("omitted".to_owned());
    s3.aliases.push(address("/zz/demo/t1/s4"));
    let mut s5 = section("/zz/demo/t1/s5", "5");
    s5.root.items.push(subsection(
        "/zz/demo/t1/s5",
        vec![Block::Para(plain("An invented subsection."))],
    ));
    let s5_2 = section("/zz/demo/t1/s5~2", "5");
    let targets = outcome_targets().map(|(target, _)| target);
    let mut s6 = section(S6, "6");
    s6.root.items = vec![
        Item::Part {
            role: PartRole::Content,
            blocks: vec![linking(&targets[..9])],
        },
        subsection(S6, vec![linking(&targets[9..])]),
    ];
    s6.notes = vec![
        linking_note(
            S6,
            "amendment",
            Layer::Enacted,
            &["/zz/demo/t1/s1", "/zz/demo/t1/s9"],
        ),
        linking_note(S6, "codification", Layer::Editorial, &["/zz/demo/t1/s2/a"]),
    ];
    s6.credit = Some(linked(&["/zz/other/x"]));
    s6.footnotes = vec![Footnote {
        ordinal: 1,
        shown: "1".to_owned(),
        blocks: vec![linking(&["/zz/demo/t9/s1"])],
    }];
    vec![
        Module::Container(container(
            "/zz/demo/t1",
            None,
            "title",
            "Title 1—",
            "Invented Things",
            &[CHAPTER],
        )),
        Module::Container(chapter),
        Module::Unit(s1),
        Module::Unit(s2),
        Module::Unit(s3),
        Module::Unit(s5),
        Module::Unit(s5_2),
        Module::Unit(s6),
    ]
}

/// Every reference of the outcome workspace: the address it stands in, its
/// layer, its target and its outcome.
fn outcome_references() -> Vec<(&'static str, Layer, &'static str, Outcome)> {
    let mut all = vec![(
        CHAPTER,
        Layer::Editorial,
        "/yy/stat/116/926",
        Outcome::Outside("/yy/stat".to_owned()),
    )];
    for (n, (target, outcome)) in outcome_targets().into_iter().enumerate() {
        let from = if n < 9 { S6 } else { "/zz/demo/t1/s6/a" };
        all.push((from, Layer::Operative, target, outcome));
    }
    all.extend([
        (
            S6,
            Layer::Editorial,
            "/zz/other/x",
            Outcome::Outside("/zz/other".to_owned()),
        ),
        (AMENDMENT, Layer::Enacted, "/zz/demo/t1/s1", Outcome::Exact),
        (AMENDMENT, Layer::Enacted, "/zz/demo/t1/s9", Outcome::NoUnit),
        (
            CODIFICATION,
            Layer::Editorial,
            "/zz/demo/t1/s2/a",
            Outcome::UnitOnly,
        ),
        (S6, Layer::Editorial, "/zz/demo/t9/s1", Outcome::NoUnit),
    ]);
    all
}

/// The interface of `module` as phase one builds it from its text, read
/// from memory.
fn interface_in_memory(module: &Module) -> Interface {
    let lowered = lower(&parse(&render(module)))
        .unwrap_or_else(|found| panic!("{module:?} does not lower: {found:#?}"));
    interface_of(&lowered.module, &lowered.decl_spans())
}

#[test]
fn an_unresolved_reference_is_never_a_diagnostic() {
    let scratch = Scratch::new("unresolved");
    let mut models = demo_models();
    for index in 2..5 {
        unit_at(&mut models, index).root.items.push(Item::Part {
            role: PartRole::Content,
            blocks: vec![linking(&["/zz/demo/t1/s9", "/zz/other/x"])],
        });
    }
    write_workspace(scratch.path(), &models);
    let report = check(scratch.path());
    assert_eq!(report.diagnostics, []);
    assert_eq!(report.errors(), 0);
    let operative = &report.references[&Layer::Operative];
    assert_eq!(operative["no-unit"], 3);
    assert_eq!(operative["outside"], 3);
    assert_eq!(operative.values().sum::<u64>(), 6);
    assert_eq!(
        report.outside_by_body,
        BTreeMap::from([("/zz/other".to_owned(), 3)])
    );
    assert_eq!(report.unresolved.len(), 6);
    assert!(report.to_json().contains("\"no-unit\": 3"));
}

#[test]
fn references_are_counted_by_layer() {
    let scratch = Scratch::new("layers");
    let mut models = demo_models();
    let s1 = unit_at(&mut models, 2);
    s1.root.items.push(Item::Part {
        role: PartRole::Content,
        blocks: vec![linking(&["/zz/demo/t1/s2"])],
    });
    s1.notes.push(linking_note(
        "/zz/demo/t1/s1",
        "effective",
        Layer::Enacted,
        &["/zz/demo/t1/s9"],
    ));
    s1.credit = Some(linked(&["/zz/other/x"]));
    write_workspace(scratch.path(), &models);
    let report = check(scratch.path());
    assert_eq!(report.diagnostics, []);
    let count = |layer: Layer, outcome: &str| report.references[&layer][outcome];
    let total = |layer: Layer| report.references[&layer].values().sum::<u64>();
    assert_eq!(count(Layer::Operative, "exact"), 1);
    assert_eq!(count(Layer::Enacted, "no-unit"), 1);
    assert_eq!(count(Layer::Editorial, "outside"), 1);
    assert_eq!(
        [Layer::Operative, Layer::Enacted, Layer::Editorial].map(total),
        [1, 1, 1]
    );
    assert_eq!(
        report.references.keys().copied().collect::<Vec<_>>(),
        [Layer::Operative, Layer::Enacted, Layer::Editorial]
    );
    for counts in report.references.values() {
        assert_eq!(
            counts.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            Outcome::NAMES.into_iter().collect::<BTreeSet<_>>()
        );
    }
}

#[test]
fn the_unresolved_list_is_sorted_and_complete() {
    let scratch = Scratch::new("unresolved-list");
    write_workspace(scratch.path(), &outcome_models());
    let report = check(scratch.path());
    assert_eq!(report.diagnostics, []);

    let all = outcome_references();
    let mut expected: Vec<Unresolved> = all
        .iter()
        .filter(|(.., outcome)| *outcome != Outcome::Exact)
        .map(|(from, layer, target, outcome)| Unresolved {
            from: address(from),
            layer: *layer,
            target: (*target).to_owned(),
            outcome: outcome.clone(),
        })
        .collect();
    expected.sort_by(|a, b| (&a.from, &a.target).cmp(&(&b.from, &b.target)));
    assert_eq!(report.unresolved, expected);
    let exact = all.len() - expected.len();
    assert_eq!(exact, 4);
    let counted: u64 = report
        .references
        .values()
        .flat_map(|counts| counts.values())
        .sum();
    assert_eq!(counted, all.len() as u64);

    // One object to a line, with the keys in their order, sorted by `from`
    // and then by `target`.
    let lines = report.unresolved_lines();
    assert_eq!(lines.lines().count(), expected.len());
    assert!(lines.ends_with('\n'));
    let mut keys = Vec::new();
    for (line, unresolved) in lines.lines().zip(&expected) {
        let value: serde_json::Value = serde_json::from_str(line).expect("a line is JSON");
        let object = value.as_object().expect("a line is an object");
        assert_eq!(
            object.keys().map(String::as_str).collect::<Vec<_>>(),
            ["from", "layer", "target", "outcome"],
            "{line}"
        );
        assert_eq!(object["from"], unresolved.from.as_str(), "{line}");
        assert_eq!(object["target"], unresolved.target.as_str(), "{line}");
        assert_eq!(object["outcome"], unresolved.outcome.name(), "{line}");
        keys.push((
            object["from"].as_str().expect("a string").to_owned(),
            object["target"].as_str().expect("a string").to_owned(),
        ));
    }
    assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]), "{keys:#?}");
    assert_eq!(
        lines.lines().next(),
        Some(
            r#"{"from":"/zz/demo/t1/ch1","layer":"editorial","target":"/yy/stat/116/926","outcome":"outside"}"#
        )
    );
}

#[test]
fn phase_two_is_a_function_of_interfaces() {
    // The outcome workspace with two hierarchy defects, which give three
    // diagnostics: a member that is no module, and a unit within the title,
    // which does not list it, while the chapter does.
    let mut models = outcome_models();
    let chapter = container_at(&mut models, 1);
    chapter.members.push(address("/zz/demo/t1/s9"));
    unit_at(&mut models, 6).within = Some(address("/zz/demo/t1"));
    let scratch = Scratch::new("phase-two");
    write_workspace(scratch.path(), &models);
    let report = check(scratch.path());
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.source_file.as_deref(), diagnostic.code))
            .collect::<Vec<_>>(),
        [
            (Some("zz/demo/t1/ch1.fr"), DiagnosticCode::E720),
            (Some("zz/demo/t1/ch1.fr"), DiagnosticCode::E720),
            (Some("zz/demo/t1/s5~2.fr"), DiagnosticCode::E720),
        ],
        "{report:#?}"
    );

    // The same interfaces, built in memory and given in another order.
    let interfaces: Vec<Interface> = models.iter().rev().map(interface_in_memory).collect();
    let phase_two = check_interfaces(&interfaces, &[], &demo_manifest(), TOOLCHAIN);
    assert_eq!(phase_two.diagnostics, report.diagnostics);
    assert_eq!(phase_two.references, report.references);
    assert_eq!(phase_two.outside_by_body, report.outside_by_body);
    assert_eq!(phase_two.unresolved, report.unresolved);
    assert_eq!(
        phase_two.references[&Layer::Operative]["ambiguous"],
        2,
        "{phase_two:#?}"
    );
}

/// The modules of a generated workspace of 200 modules: the title `t1`,
/// nine chapters and 190 sections, the section `n` in the chapter
/// `(n - 1) % 9 + 1`. Each section links to the next section, to a part of
/// itself that does not exist, and to an address of another corpus.
fn many_models() -> Vec<Module> {
    let chapter_of = |n: usize| format!("/zz/demo/t1/ch{}", (n - 1) % 9 + 1);
    let chapters: Vec<String> = (1..=9).map(|k| format!("/zz/demo/t1/ch{k}")).collect();
    let chapter_refs: Vec<&str> = chapters.iter().map(String::as_str).collect();
    let mut models = vec![Module::Container(container(
        "/zz/demo/t1",
        None,
        "title",
        "Title 1—",
        "Invented Things",
        &chapter_refs,
    ))];
    for chapter in &chapters {
        let members: Vec<String> = (1..=190)
            .filter(|n| chapter_of(*n) == *chapter)
            .map(|n| format!("/zz/demo/t1/s{n}"))
            .collect();
        let members: Vec<&str> = members.iter().map(String::as_str).collect();
        models.push(Module::Container(container(
            chapter,
            Some("/zz/demo/t1"),
            "chapter",
            "Chapter—",
            "Numbered Sections",
            &members,
        )));
    }
    for n in 1..=190 {
        let at = format!("/zz/demo/t1/s{n}");
        let mut section = unit(
            &at,
            Some(&chapter_of(n)),
            &n.to_string(),
            "Numbered",
            "An invented numbered section.",
        );
        let next = format!("/zz/demo/t1/s{}", n + 1);
        let part = format!("{at}/z");
        let other = format!("/zz/other/x{}", n % 3);
        section.root.items = vec![Item::Part {
            role: PartRole::Content,
            blocks: vec![
                Block::Para(plain("An invented numbered section.")),
                linking(&[&next, &part, &other]),
            ],
        }];
        models.push(Module::Unit(section));
    }
    models
}

/// The index in [`many_models`] of the section `n`.
fn section_index(n: usize) -> usize {
    9 + n
}

/// Writes the generated workspace of [`many_models`] into `root`, with
/// defects seeded. In the models: a chapter that lists a member that is no
/// module, and a section within another section, which its chapter does not
/// list. In the files: a word changed, a text that is not normalized, and a
/// section deleted. It holds 199 module files.
fn seeded_workspace(root: &Path) {
    let mut models = many_models();
    assert_eq!(models.len(), 200);
    container_at(&mut models, 3)
        .members
        .push(address("/zz/demo/t1/s999"));
    container_at(&mut models, 1)
        .members
        .retain(|member| member.as_str() != "/zz/demo/t1/s100");
    unit_at(&mut models, section_index(100)).within = Some(address("/zz/demo/t1/s1"));
    write_workspace(root, &models);
    for (rel, from, to) in [
        ("zz/demo/t1/s7.fr", "numbered section.", "numbered sectio."),
        (
            "zz/demo/t1/s20.fr",
            "invented numbered",
            "invented  numbered",
        ),
    ] {
        let text = read(root, rel);
        assert!(text.contains(from), "{rel}");
        put(root, rel, &text.replacen(from, to, 1));
    }
    fs::remove_file(root.join("zz/demo/t1/s50.fr")).expect("remove");
}

#[test]
fn the_report_and_the_output_are_the_same_for_one_and_eight_workers() {
    let scratch = Scratch::new("workers");
    seeded_workspace(scratch.path());

    let one = check_workspace(scratch.path(), &with_jobs(1));
    assert_eq!(one.modules, 199);
    let files: BTreeSet<&str> = one
        .diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.source_file.as_deref())
        .collect();
    assert_eq!(
        files,
        BTreeSet::from([
            "zz/demo/t1/ch3.fr",
            "zz/demo/t1/s100.fr",
            "zz/demo/t1/s20.fr",
            "zz/demo/t1/s50.fr",
            "zz/demo/t1/s7.fr",
        ]),
        "{one:#?}"
    );
    let operative = &one.references[&Layer::Operative];
    assert!(operative["exact"] > 100, "{operative:?}");
    assert!(operative["unit-only"] > 100, "{operative:?}");
    assert_eq!(one.outside_by_body.len(), 1);
    let printed = |report: &WorkspaceReport| {
        let mut lines: Vec<String> = report.diagnostics.iter().map(ToString::to_string).collect();
        lines.push(report.summary());
        lines
    };
    for _ in 0..3 {
        let eight = check_workspace(scratch.path(), &with_jobs(8));
        assert_eq!(eight, one);
        assert_eq!(printed(&eight), printed(&one));
        assert_eq!(eight.to_json(), one.to_json());
        assert_eq!(eight.unresolved_lines(), one.unresolved_lines());
    }
}

/// The check report and the unresolved list of the outcome workspace,
/// under `conformance/fixtures/check-report`, are what the check writes;
/// the schema of the report is checked on them there.
#[test]
fn the_committed_check_report_is_what_the_check_writes() {
    let scratch = Scratch::new("check-report");
    write_workspace(&scratch.join("outcomes"), &outcome_models());
    let report = check(&scratch.join("outcomes"));
    assert_eq!(report.diagnostics, []);
    let mut fresh = Scratch::new("check-report-fresh");
    put(fresh.path(), "report.json", &report.to_json());
    put(fresh.path(), "unresolved.jsonl", &report.unresolved_lines());
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/check-report");
    if !fixture.is_dir() || tree(fresh.path()) != tree(&fixture) {
        fresh.keep = true;
        panic!(
            "conformance/fixtures/check-report is not what the check writes of the outcome \
             workspace; what it writes is in {}",
            fresh.path().display()
        );
    }
}

/// How [`check_workspace`] runs with its cache in `cache`, on `jobs` threads.
fn cached(cache: &Path, jobs: usize) -> CheckOptions {
    CheckOptions {
        cache_dir: Some(cache.to_owned()),
        ..with_jobs(jobs)
    }
}

/// `report` with no module counted as read from the cache, which is what a
/// check without a cache reports.
fn uncounted(report: &WorkspaceReport) -> WorkspaceReport {
    WorkspaceReport {
        from_cache: 0,
        ..report.clone()
    }
}

fn address_of(model: &Module) -> &Address {
    match model {
        Module::Unit(unit) => &unit.address,
        Module::Container(container) => &container.address,
        Module::Curated(curated) => &curated.address,
    }
}

/// Writes a curated workspace into `out` that holds the five demo modules,
/// each at the path of its address. It has no lock, so a module file may be
/// changed without disagreeing with one.
fn curated_demo(out: &Path) {
    let files: Vec<(String, String)> = demo_models()
        .iter()
        .map(|model| (module_path(address_of(model)), render(model)))
        .collect();
    let files: Vec<(&str, String)> = files
        .iter()
        .map(|(rel, text)| (rel.as_str(), text.clone()))
        .collect();
    fs::create_dir_all(out).expect("create the workspace's directory");
    curated_workspace(out, &files);
}

/// The BLAKE3 of the bytes of the file `rel` under `root`, and its key in
/// `cache`.
fn digest_and_key(cache: &Cache, root: &Path, rel: &str) -> (Hash32, Hash32) {
    let bytes = fs::read(root.join(rel)).unwrap_or_else(|error| panic!("{rel}: {error}"));
    (Hash32(*blake3::hash(&bytes).as_bytes()), cache.key(&bytes))
}

#[test]
fn a_second_check_reads_every_module_from_the_cache() {
    let scratch = Scratch::new("cache-second");
    let workspace = scratch.join("workspace");
    seeded_workspace(&workspace);
    let cache = scratch.join("cache");
    let uncached = check_workspace(&workspace, &with_jobs(4));
    assert!(uncached.errors() > 0, "{uncached:#?}");

    let first = check_workspace(&workspace, &cached(&cache, 4));
    assert_eq!(first, uncached);
    let second = check_workspace(&workspace, &cached(&cache, 4));
    assert_eq!(second.modules, 199);
    assert_eq!(second.from_cache, second.modules);
    assert_eq!(uncounted(&second), uncached);
    assert_eq!(
        second.summary(),
        uncached
            .summary()
            .replace("from cache: 0", "from cache: 199")
    );
    assert_eq!(
        second.to_json(),
        uncached
            .to_json()
            .replace("\"from_cache\": 0,", "\"from_cache\": 199,")
    );
    assert_eq!(second.unresolved_lines(), uncached.unresolved_lines());

    // An entry holds nothing of the path of its file: the same workspace at
    // another path reads every module from the same cache.
    let copy = scratch.join("copy");
    seeded_workspace(&copy);
    let moved = check_workspace(&copy, &cached(&cache, 4));
    assert_eq!(moved.from_cache, moved.modules);
    assert_eq!(uncounted(&moved), uncached);
}

#[test]
fn editing_one_module_rereads_only_it() {
    let scratch = Scratch::new("cache-edit");
    let workspace = scratch.join("workspace");
    curated_demo(&workspace);
    let cache = scratch.join("cache");
    let first = check_workspace(&workspace, &cached(&cache, 2));
    assert_eq!(first.diagnostics, []);
    assert_eq!((first.modules, first.from_cache), (5, 0));

    let rel = "zz/demo/t1/s2.fr";
    let text = read(&workspace, rel);
    assert!(text.contains("small or large"), "{text}");
    put(
        &workspace,
        rel,
        &text.replacen("small or large", "small, medium or large", 1),
    );
    let second = check_workspace(&workspace, &cached(&cache, 2));
    assert_eq!((second.modules, second.from_cache), (5, 4));
    assert_eq!(uncounted(&second), check(&workspace));
    // The words changed and the hashes the file records did not: the module
    // was read again.
    assert!(!second.diagnostics.is_empty(), "{second:#?}");
    assert!(
        second
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.source_file.as_deref() == Some(rel)),
        "{second:#?}"
    );
    assert_eq!(
        check_workspace(&workspace, &cached(&cache, 2)).from_cache,
        5
    );
}

#[test]
fn diagnostics_come_back_from_the_cache() {
    let scratch = Scratch::new("cache-diagnostics");
    let workspace = scratch.join("workspace");
    curated_demo(&workspace);
    let rel = "zz/demo/t1/s2.fr";
    let text = read(&workspace, rel);
    put(
        &workspace,
        rel,
        &text.replacen("small or large", "small  or large", 1),
    );
    let cache = scratch.join("cache");
    let uncached = check(&workspace);
    let first = check_workspace(&workspace, &cached(&cache, 1));
    let second = check_workspace(&workspace, &cached(&cache, 1));
    assert_eq!((second.modules, second.from_cache), (5, 5));
    assert_eq!(second.diagnostics, uncached.diagnostics);
    assert_eq!(first.diagnostics, uncached.diagnostics);
    assert!(
        second.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::E710
                && diagnostic.message.contains("text is not normalized")
        }),
        "{second:#?}"
    );
    for diagnostic in &second.diagnostics {
        assert_eq!(
            diagnostic.source_file.as_deref(),
            Some(rel),
            "{diagnostic:#?}"
        );
        assert!(diagnostic.primary_span.is_some(), "{diagnostic:#?}");
    }
}

/// `text`, the JSON of an entry, with `change` made to its value.
fn changed_entry(text: &str, change: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(text).expect("an entry is JSON");
    change(&mut value);
    serde_json::to_string(&value).expect("JSON is written")
}

/// A way of damaging an entry: the module file whose entry is damaged, what
/// is done to it, and the damaged text made from the entry's text.
type Damage<'a> = (&'a str, &'a str, Box<dyn Fn(&str) -> String + 'a>);

#[test]
fn a_damaged_entry_is_ignored_and_rewritten() {
    let scratch = Scratch::new("cache-damaged");
    let workspace = scratch.join("workspace");
    curated_demo(&workspace);
    // One module with a diagnostic, so that its entry holds one.
    let s3 = "zz/demo/t1/s3.fr";
    let text = read(&workspace, s3);
    put(&workspace, s3, &text.replacen("any color", "any  color", 1));
    let cache_dir = scratch.join("cache");
    let uncached = check(&workspace);
    assert!(!uncached.diagnostics.is_empty(), "{uncached:#?}");
    assert_eq!(
        check_workspace(&workspace, &cached(&cache_dir, 1)),
        uncached
    );

    let cache = Cache::new(&cache_dir, TOOLCHAIN, BUILD);
    let entry_of = |rel: &str| cache.entry_path(&digest_and_key(&cache, &workspace, rel).1);
    let entry = |rel: &str| fs::read_to_string(entry_of(rel)).expect("an entry is written");
    let damages: Vec<Damage> = vec![
        (
            "zz/demo/t1.fr",
            "cut short",
            Box::new(|text: &str| text[..text.len() / 2].to_owned()),
        ),
        (
            "zz/demo/t1/ch1.fr",
            "JSON of another shape",
            Box::new(|_: &str| "{}\n".to_owned()),
        ),
        (
            "zz/demo/t1/ch1.fr",
            "JSON that is not an object",
            Box::new(|_: &str| "[]\n".to_owned()),
        ),
        (
            "zz/demo/t1/s1.fr",
            "the entry of another key",
            Box::new(|_: &str| entry("zz/demo/t1/s2.fr")),
        ),
        // A reader that serde derives takes the next two.
        (
            "zz/demo/t1/s2.fr",
            "a struct written as an array of its values",
            Box::new(|text: &str| {
                changed_entry(text, |value| {
                    let range = value["interface"]["address_range"].take();
                    assert!(range.is_object(), "{range}");
                    value["interface"]["address_range"] =
                        serde_json::json!([range["start"], range["end"]]);
                })
            }),
        ),
        (
            "zz/demo/t1/s2.fr",
            "the key of an optional field missing",
            Box::new(|text: &str| {
                changed_entry(text, |value| {
                    let interface = value["interface"].as_object_mut().expect("an interface");
                    assert_eq!(interface.remove("container"), Some(serde_json::Value::Null));
                })
            }),
        ),
        (
            "zz/demo/t1/s1.fr",
            "a key the writer does not write",
            Box::new(|text: &str| {
                changed_entry(text, |value| {
                    value["extra"] = serde_json::json!(1);
                })
            }),
        ),
        (
            s3,
            "a diagnostic that names a file",
            Box::new(|text: &str| {
                changed_entry(text, |value| {
                    let diagnostic = value["diagnostics"][0]
                        .as_object_mut()
                        .expect("the entry holds a diagnostic");
                    diagnostic.insert("source_file".to_owned(), serde_json::json!(s3));
                })
            }),
        ),
    ];
    for (rel, what, damage) in &damages {
        let path = entry_of(rel);
        let text = entry(rel);
        let damaged = damage(&text);
        assert_ne!(damaged, text, "{what}");
        fs::write(&path, &damaged).expect("write");
        let again = check_workspace(&workspace, &cached(&cache_dir, 1));
        assert_eq!((again.modules, again.from_cache), (5, 4), "{what}");
        assert_eq!(uncounted(&again), uncached, "{what}");
        // The module was checked again and its entry written again.
        let (digest, key) = digest_and_key(&cache, &workspace, rel);
        assert!(cache.read(&key, &digest).is_some(), "{what}");
        assert_eq!(entry(rel), text, "{what}");
    }
    assert_eq!(
        check_workspace(&workspace, &cached(&cache_dir, 1)).from_cache,
        5
    );
}

#[test]
fn the_key_depends_on_the_toolchain_version() {
    let scratch = Scratch::new("cache-version");
    let workspace = scratch.join("workspace");
    curated_demo(&workspace);
    let cache_dir = scratch.join("cache");
    let built = |version: &str, build: &str| CheckOptions {
        cache_dir: Some(cache_dir.clone()),
        executable: build.to_owned(),
        ..options(version)
    };
    let under = |version: &str| built(version, BUILD);
    assert_eq!(check_workspace(&workspace, &under("0.3.0")).from_cache, 0);
    let other = check_workspace(&workspace, &under("0.3.1"));
    assert_eq!(other.from_cache, 0);
    assert_eq!(other, check_workspace(&workspace, &options("0.3.1")));
    assert_eq!(check_workspace(&workspace, &under("0.3.0")).from_cache, 5);
    // Two builds of one version: no entry that one wrote is read by the
    // other.
    let rebuilt = check_workspace(&workspace, &built("0.3.0", "another invented build"));
    assert_eq!(rebuilt.from_cache, 0);
    assert_eq!(rebuilt, check_workspace(&workspace, &options("0.3.0")));
    assert_eq!(check_workspace(&workspace, &under("0.3.0")).from_cache, 5);

    // The key is the BLAKE3 of four frames, each a tag byte, the length of
    // its value as eight bytes little-endian, and the value.
    let frame = |tag: u8, value: &[u8]| {
        let mut frame = vec![tag];
        frame.extend_from_slice(&(value.len() as u64).to_le_bytes());
        frame.extend_from_slice(value);
        frame
    };
    let bytes = b"edition 2\nAn invented module.\n";
    let expected = |version: &str, build: &str| {
        let mut frames = frame(0x00, b"fidryn.cache/v1");
        frames.extend(frame(0x01, version.as_bytes()));
        frames.extend(frame(0x02, build.as_bytes()));
        frames.extend(frame(0x03, bytes));
        Hash32(*blake3::hash(&frames).as_bytes())
    };
    for (version, build) in [("0.3.0", BUILD), ("0.3.1", BUILD), ("0.3.0", "another")] {
        assert_eq!(
            Cache::new(&cache_dir, version, build).key(bytes),
            expected(version, build),
            "{version} {build}"
        );
    }
    assert_ne!(expected("0.3.0", BUILD), expected("0.3.1", BUILD));
    assert_ne!(expected("0.3.0", BUILD), expected("0.3.0", "another"));
}

#[test]
fn the_count_from_the_cache_does_not_depend_on_the_workers() {
    // Two module files of the same bytes share one key. A check reads no
    // entry that it wrote itself, so neither of them is read from the cache
    // by the check that writes their entry, however the workers are
    // scheduled.
    let scratch = Scratch::new("cache-workers");
    let workspace = scratch.join("workspace");
    curated_demo(&workspace);
    let twin = read(&workspace, "zz/demo/t1/s1.fr");
    put(&workspace, "zz/demo/t1/s9.fr", &twin);
    let uncached = check(&workspace);
    assert_eq!(uncached.modules, 6);
    for jobs in [1, 8] {
        let cache_dir = scratch.join(&format!("cache-{jobs}"));
        let cold = check_workspace(&workspace, &cached(&cache_dir, jobs));
        assert_eq!(cold.from_cache, 0, "cold, {jobs} workers");
        assert_eq!(cold, uncached, "cold, {jobs} workers");
        let warm = check_workspace(&workspace, &cached(&cache_dir, jobs));
        assert_eq!(warm.from_cache, 6, "warm, {jobs} workers");
        assert_eq!(uncounted(&warm), uncached, "warm, {jobs} workers");
    }
}

#[test]
fn two_checks_at_once_leave_a_valid_cache() {
    let scratch = Scratch::new("cache-at-once");
    let workspace = scratch.join("workspace");
    seeded_workspace(&workspace);
    let cache_dir = scratch.join("cache");
    let uncached = check_workspace(&workspace, &with_jobs(4));
    let (one, two) = std::thread::scope(|scope| {
        let one = scope.spawn(|| check_workspace(&workspace, &cached(&cache_dir, 4)));
        let two = scope.spawn(|| check_workspace(&workspace, &cached(&cache_dir, 4)));
        (
            one.join().expect("a check panicked"),
            two.join().expect("a check panicked"),
        )
    });
    assert_eq!(uncounted(&one), uncached);
    assert_eq!(uncounted(&two), uncached);

    // Every file of the cache is the entry of a module file, at
    // `v1/<first two digits>/<key>.json`, and reads back; no temporary file
    // is left.
    let cache = Cache::new(&cache_dir, TOOLCHAIN, BUILD);
    let mut expected = BTreeSet::new();
    for rel in walk(&workspace).modules {
        let (digest, key) = digest_and_key(&cache, &workspace, &rel);
        assert!(cache.read(&key, &digest).is_some(), "{rel}");
        let key = key.to_string();
        expected.insert(format!("v1/{}/{key}.json", &key[..2]));
    }
    assert_eq!(expected.len(), 199);
    let files: BTreeSet<String> = tree(&cache_dir)
        .into_iter()
        .filter(|(_, entry)| matches!(entry, Entry::File(_)))
        .map(|(rel, _)| rel)
        .collect();
    assert_eq!(files, expected);
    assert_eq!(
        check_workspace(&workspace, &cached(&cache_dir, 4)).from_cache,
        199
    );
}
