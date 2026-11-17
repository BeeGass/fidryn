//! Tests of the workspace check and of the writer. Every workspace here is
//! built in a temporary directory from invented models, in the invented
//! corpus `/zz/demo`, and so is every text.

use super::{CheckOptions, GeneratedWriter, WorkspaceReport, WriteError, check_workspace};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::render::render;
use fidryn_provision::workspace::{
    AddressCounts, Artifact, Dependency, Generator, Lock, Manifest, Member, NotImported,
    RetrievalRecord, RetrievedArchive, WorkspaceKind,
};
use fidryn_provision::{
    Address, Block, ContainerModule, CuratedModule, Hash32, Item, Module, Num, PartRole, Provision,
    Run, Text, UnitModule,
};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The version of the running toolchain, which the demo manifest records.
const TOOLCHAIN: &str = env!("CARGO_PKG_VERSION");

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

/// The demo workspace under `conformance/fixtures/workspace` is what the
/// writer writes from the demo models, beside the retrieval record of its
/// archive. The schemas of the three files are checked on it there.
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
    if !fixture.is_dir() || tree(scratch.path()) != tree(&fixture) {
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
