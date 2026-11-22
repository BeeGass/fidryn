//! Tests of the workspace check and of the writer. Every workspace here is
//! built in a temporary directory from invented models, in the invented
//! corpus `/zz/demo`, and so is every text.

use super::cache::Cache;
use super::discover::walk;
use super::{
    CheckOptions, CoverageError, DependencyInterfaces, Difference, GeneratedWriter, Unresolved,
    UnresolvedCitation, WorkspaceReport, WriteError, check_interfaces, check_workspace,
    read_coverage,
};
use fidryn_check::e2::lower;
use fidryn_check::e2::names::{ResolvedBinding, resolve};
use fidryn_core::{Diagnostic, DiagnosticCode, Span};
use fidryn_provision::interface::ByteRange;
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

mod change_sets;
mod staleness;

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

    // A run writes `coverage.json` at the top again, the ledger of what it
    // wrote, and keeps every directory whose name begins with a dot.
    let ledger = read(scratch.path(), "coverage.json");
    put(scratch.path(), "coverage.json", "{}\n");
    put(scratch.path(), ".cache/v1/entry.json", "{}\n");
    put(scratch.path(), "zz/.drafts/s9.fr", "An invented draft.\n");
    write_workspace(scratch.path(), &models);
    assert_eq!(read(scratch.path(), "coverage.json"), ledger);
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
fn a_writer_refuses_a_module_file_no_run_of_its_prefix_wrote_and_removes_nothing() {
    let scratch = Scratch::new("foreign-modules");
    write_workspace(scratch.path(), &demo_models());
    // An earlier output whose head stands after a comment longer than the
    // first bytes the writer reads is still one: the run writes it again.
    let s3 = read(scratch.path(), "zz/demo/t1/s3.fr");
    let comment = format!("// {}\n", "An invented comment. ".repeat(60));
    put(
        scratch.path(),
        "zz/demo/t1/s3.fr",
        &format!("{comment}{s3}"),
    );
    // Files of other kinds, each of which no run of the prefix wrote; the
    // last holds a module whose head lies further than a writer looks.
    let far = format!(
        "// {}\nedition 2\nmodule \"/zz/demo/t1/s5\" {{\n}}\n",
        "An invented comment. ".repeat(4000)
    );
    let foreign = [
        (
            "zz/demo/t1/old.fr",
            "module Demo.Old version \"0.1.0\" {\n}\n",
            "which is no module of edition 2",
        ),
        (
            "zz/demo/t1/s5.fr",
            far.as_str(),
            "in whose first 65536 bytes no module of edition 2 begins",
        ),
        (
            "zz/demo/t1/marked.fr",
            "edition 2\n// An invented file with the marker and no module.\n",
            "which is no module of edition 2",
        ),
        (
            "zz/demo/other.fr",
            "edition 2\nmodule \"/zz/other/x\" {\n}\n",
            "whose module `/zz/other/x` is not below the prefix `/zz/demo`",
        ),
        (
            "zz/demo/t1/s7.fr",
            "edition 2\nmodule \"/zz/demo/t1/s8\" {\n}\n",
            "whose module `/zz/demo/t1/s8` has the path `zz/demo/t1/s8.fr`",
        ),
        (
            "zz/demo/t1/s9.fr",
            "edition 2 module \"no address\" {\n}\n",
            "whose module address `no address` is no address",
        ),
    ];
    for (rel, text, _) in foreign {
        put(scratch.path(), rel, text);
    }
    let before = tree(scratch.path());
    let error = refused(scratch.path(), demo_manifest());
    let WriteError::Foreign { found, .. } = &error else {
        panic!("{error}");
    };
    for (rel, _, why) in foreign {
        assert!(found.contains(&format!("`{rel}`, {why}")), "{rel}: {found}");
    }
    assert!(!found.contains("s3.fr"), "{found}");
    assert_eq!(tree(scratch.path()), before);

    // A comparing writer reports each of them, so that `--verify` and a run
    // agree: a file where the run writes nothing is extra.
    let writer = GeneratedWriter::compare(scratch.path(), demo_manifest())
        .unwrap_or_else(|error| panic!("{error}"));
    for model in &demo_models() {
        writer
            .write(model)
            .unwrap_or_else(|error| panic!("{error}"));
    }
    let compared = writer.finish(
        generator(),
        artifacts(),
        AddressCounts::default(),
        NotImported::default(),
    );
    let differences = match compared {
        Err(WriteError::Differs { differences, .. }) => differences,
        other => panic!("{other:?}"),
    };
    for (rel, _, _) in foreign {
        assert!(
            differences.contains(&(rel.to_owned(), Difference::Extra)),
            "{rel}: {differences:?}"
        );
    }
    assert_eq!(tree(scratch.path()), before);

    // Without them, the run accepts the directory and writes the module
    // after the comment again.
    for (rel, _, _) in foreign {
        fs::remove_file(scratch.join(rel)).expect("remove a file");
    }
    write_workspace(scratch.path(), &demo_models());
    assert_eq!(read(scratch.path(), "zz/demo/t1/s3.fr"), s3);
    assert_eq!(check(scratch.path()).diagnostics, []);
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

/// The ledger the writer writes of a workspace, `coverage.json`, is what
/// [`read_coverage`] counts again from its files, on any number of workers;
/// its references are counted as the check counts them; a run stopped
/// before the ledger leaves a workspace that checks clean; and a curated
/// workspace, a generated one without a lock and one with a module that
/// does not lower have no ledger.
#[test]
fn the_ledger_the_writer_writes_is_what_is_counted_from_the_files() {
    let scratch = Scratch::new("ledger");
    let out = scratch.join("outcomes");
    let lock = write_workspace(&out, &outcome_models());
    let written = read(&out, "coverage.json");
    for jobs in [1, 8] {
        let counted = read_coverage(&out, jobs).unwrap_or_else(|error| panic!("{error:?}"));
        assert_eq!(counted.to_json(), written, "on {jobs} workers");
    }
    let ledger: serde_json::Value = serde_json::from_str(&written).expect("the ledger is JSON");
    let report: serde_json::Value =
        serde_json::from_str(&check(&out).to_json()).expect("the report is JSON");
    assert_eq!(ledger["totals"]["references"], report["references"]);
    assert_eq!(
        ledger["totals"]["outside_by_body"],
        report["outside_by_body"]
    );
    let modules = ledger["totals"]["units"].as_u64().expect("a count")
        + ledger["totals"]["containers"].as_u64().expect("a count");
    assert_eq!(modules, lock.modules.len() as u64);
    assert_eq!(ledger["release"], "42-7");

    // A run stopped after the lock and before the ledger leaves a
    // workspace whose check does not read the ledger.
    fs::remove_file(out.join("coverage.json")).expect("remove the ledger");
    assert_eq!(check(&out).diagnostics, []);
    assert_eq!(
        read_coverage(&out, 1)
            .unwrap_or_else(|error| panic!("{error:?}"))
            .to_json(),
        written
    );

    let unread = |root: &Path| match read_coverage(root, 1) {
        Err(CoverageError::Unread(diagnostics)) => diagnostics,
        other => panic!("{other:?}"),
    };
    let curated = scratch.join("curated");
    fs::create_dir_all(&curated).expect("create a directory");
    curated_workspace(&curated, &[]);
    assert!(matches!(
        read_coverage(&curated, 1),
        Err(CoverageError::Curated)
    ));
    put(
        &out,
        "zz/demo/t1/s9.fr",
        "edition 2\nmodule \"/zz/demo/t1/s9\" {\n",
    );
    let found = unread(&out);
    assert!(
        !found.is_empty()
            && found
                .iter()
                .all(|found| found.source_file.as_deref() == Some("zz/demo/t1/s9.fr")),
        "{found:#?}"
    );
    fs::remove_file(out.join("law.lock.json")).expect("remove the lock");
    assert_diagnostic(
        only_of(&unread(&out)),
        DiagnosticCode::E730,
        "law.lock.json",
        None,
        "has a lock",
    );
    fs::remove_file(out.join("workspace.json")).expect("remove the manifest");
    assert_diagnostic(
        only_of(&unread(&out)),
        DiagnosticCode::E730,
        "workspace.json",
        None,
        "no workspace",
    );
}

/// The one diagnostic of `diagnostics`.
fn only_of(diagnostics: &[Diagnostic]) -> &Diagnostic {
    match diagnostics {
        [diagnostic] => diagnostic,
        other => panic!("expected one diagnostic, found {other:#?}"),
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

// The names of curated modules. The workspace has the prefix `/zz/notes`
// and the modules `a`, `b` and `c`; `b` cites `/zz/demo/t1/s8/a` as
// `widget`. It has no dependency, so no cited address is found, and every
// binding that resolves is stale; `check_notes` leaves those warnings out.

/// The manifest of a curated workspace of the prefix `/zz/notes`.
fn notes_manifest() -> Manifest {
    Manifest {
        name: "notes".to_owned(),
        prefix: address("/zz/notes"),
        ..curated_manifest()
    }
}

/// The text of the curated module `/zz/notes/<name>` that declares `decls`,
/// in order.
fn note_module(name: &str, decls: &[&str]) -> String {
    let mut text = format!("edition 2\nmodule \"/zz/notes/{name}\" {{\n");
    for line in decls.iter().flat_map(|decl| decl.lines()) {
        text.push_str("  ");
        text.push_str(line);
        text.push('\n');
    }
    text.push_str("}\n");
    text
}

/// A `binding` named `name` whose `binds` names `binds`.
fn binding(name: &str, binds: &str) -> String {
    format!(
        "binding {name} {{\n  binds {binds}\n  release \"demo-1\"\n  \
         provision_hash \"{ARCHIVE_BLAKE3}\"\n  notes_hash \"{MEMBER_BLAKE3}\"\n  pending\n}}"
    )
}

/// The module `b`, which cites `/zz/demo/t1/s8/a` as `widget`.
fn module_b() -> String {
    note_module("b", &["citation widget -> \"/zz/demo/t1/s8/a\""])
}

/// Writes the curated workspace of `modules` into `out`: each `(name,
/// text)` at `zz/notes/<name>.fr`.
fn notes_workspace(out: &Path, modules: &[(&str, &str)]) {
    fs::write(out.join("workspace.json"), notes_manifest().to_json()).expect("write");
    for (name, text) in modules {
        put(out, &format!("zz/notes/{name}.fr"), text);
    }
}

/// The check of the curated workspace of `modules`, which has no
/// dependency, but for the warnings of its stale bindings: every binding
/// that resolves here binds an address below no dependency, and so is stale
/// (`W750`), which the tests of staleness check. These tests check names.
fn check_notes(tag: &str, modules: &[(&str, &str)]) -> WorkspaceReport {
    let scratch = Scratch::new(tag);
    notes_workspace(scratch.path(), modules);
    let mut report = check(scratch.path());
    report
        .diagnostics
        .retain(|diagnostic| diagnostic.code != DiagnosticCode::W750);
    report
}

/// The interface of the module written as `text`, as phase one builds it.
fn interface_of_text(text: &str) -> Interface {
    let lowered =
        lower(&parse(text)).unwrap_or_else(|found| panic!("{text} does not lower: {found:#?}"));
    interface_of(&lowered.module, &lowered.decl_spans())
}

/// What `resolve` gives of the module `name` of `modules`, with each of them
/// importable.
fn resolved_in(modules: &[(&str, &str)], name: &str) -> (Vec<ResolvedBinding>, Vec<Diagnostic>) {
    let interfaces: Vec<Interface> = modules
        .iter()
        .map(|(_, text)| interface_of_text(text))
        .collect();
    let at = modules
        .iter()
        .position(|(each, _)| *each == name)
        .expect("the module is one of them");
    resolve(&interfaces[at], &|address| {
        interfaces
            .iter()
            .find(|interface| interface.address == *address)
    })
}

/// The name of each binding of `resolved` and the address it binds.
fn targets(resolved: &[ResolvedBinding]) -> Vec<(&str, &str)> {
    resolved
        .iter()
        .map(|binding| (binding.binding.item.name.as_str(), binding.target.as_str()))
        .collect()
}

/// The span of `word` where it follows `before` in `text`.
fn span_of(text: &str, before: &str, word: &str) -> Span {
    let needle = format!("{before}{word}");
    let at = text
        .find(&needle)
        .unwrap_or_else(|| panic!("{needle:?} is not in:\n{text}"))
        + before.len();
    let start = u32::try_from(at).expect("a small file");
    let end = start + u32::try_from(word.len()).expect("a short word");
    Span { start, end }
}

/// The one diagnostic of the check of the curated workspace of `modules`,
/// asserted to be an `E200` in the file of the module `name` at `span`
/// whose message holds `words`.
fn one_name_error(
    tag: &str,
    modules: &[(&str, &str)],
    name: &str,
    span: Span,
    words: &str,
) -> Diagnostic {
    let report = check_notes(tag, modules);
    let diagnostic = only(&report).clone();
    assert_diagnostic(
        &diagnostic,
        DiagnosticCode::E200,
        &format!("zz/notes/{name}.fr"),
        Some(span),
        words,
    );
    diagnostic
}

#[test]
fn a_qualified_name_resolves_through_its_alias() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            &binding("widget_binding", "b.widget"),
        ],
    );
    let b = module_b();
    let modules = [("a", a.as_str()), ("b", b.as_str())];
    let (bindings, diagnostics) = resolved_in(&modules, "a");
    assert_eq!(diagnostics, []);
    assert_eq!(targets(&bindings), [("widget_binding", "/zz/demo/t1/s8/a")]);
    assert_eq!(
        bindings[0].binding.range.target,
        range_of(span_of(&a, "binds b.", "widget"))
    );
    assert_eq!(check_notes("names-qualified", &modules).diagnostics, []);
}

#[test]
fn a_name_brought_into_scope_resolves_unqualified() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" { widget }",
            &binding("widget_binding", "widget"),
        ],
    );
    let b = module_b();
    let modules = [("a", a.as_str()), ("b", b.as_str())];
    let (bindings, diagnostics) = resolved_in(&modules, "a");
    assert_eq!(diagnostics, []);
    assert_eq!(targets(&bindings), [("widget_binding", "/zz/demo/t1/s8/a")]);
    assert_eq!(check_notes("names-brought", &modules).diagnostics, []);
}

#[test]
fn a_local_citation_resolves_unqualified() {
    let a = note_module(
        "a",
        &[
            "citation gadget -> \"/zz/demo/t1/s9\"",
            &binding("gadget_binding", "gadget"),
            &binding("address_binding", "\"/zz/demo/t1/s8/b\""),
        ],
    );
    let modules = [("a", a.as_str())];
    let (bindings, diagnostics) = resolved_in(&modules, "a");
    assert_eq!(diagnostics, []);
    assert_eq!(
        targets(&bindings),
        [
            ("gadget_binding", "/zz/demo/t1/s9"),
            ("address_binding", "/zz/demo/t1/s8/b"),
        ]
    );
    assert_eq!(check_notes("names-local", &modules).diagnostics, []);
}

#[test]
fn an_unqualified_name_that_is_not_in_scope() {
    let a = note_module("a", &[&binding("gizmo_binding", "gizmo")]);
    one_name_error(
        "names-not-in-scope",
        &[("a", &a)],
        "a",
        span_of(&a, "binds ", "gizmo"),
        "`binds` names `gizmo`, which is no citation of this module and no name that an \
         import brings into scope",
    );
    assert_eq!(resolved_in(&[("a", &a)], "a").0, []);
}

#[test]
fn a_binding_binds_a_citation_and_not_an_alias_or_a_binding() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            &binding("by_alias", "b"),
            &binding("by_binding", "by_alias"),
        ],
    );
    let b = module_b();
    let report = check_notes("names-not-a-citation", &[("a", &a), ("b", &b)]);
    let file = "zz/notes/a.fr";
    match report.diagnostics.as_slice() {
        [alias, binding] => {
            assert_diagnostic(
                alias,
                DiagnosticCode::E200,
                file,
                Some(span_of(&a, "binds ", "b")),
                "`binds` names `b`, which is the alias of an import and not a citation",
            );
            assert_diagnostic(
                binding,
                DiagnosticCode::E200,
                file,
                Some(span_of(&a, "binds ", "by_alias")),
                "`binds` names `by_alias`, which is a binding and not a citation",
            );
        }
        other => panic!("expected two diagnostics, found {other:#?}"),
    }
}

#[test]
fn an_alias_that_no_import_declares() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            &binding("widget_binding", "x.widget"),
        ],
    );
    let b = module_b();
    one_name_error(
        "names-no-alias",
        &[("a", &a), ("b", &b)],
        "a",
        span_of(&a, "binds ", "x"),
        "`binds` names `x.widget`, and `x` is the alias of no import of this module",
    );
}

#[test]
fn a_name_the_imported_module_does_not_declare() {
    let b = module_b();

    // In `alias.name`.
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            &binding("gizmo_binding", "b.gizmo"),
        ],
    );
    one_name_error(
        "names-undeclared-qualified",
        &[("a", &a), ("b", &b)],
        "a",
        span_of(&a, "binds b.", "gizmo"),
        "`binds` names `b.gizmo`, and the module `/zz/notes/b`, imported as `b`, declares no \
         citation `gizmo`",
    );

    // In the braces of an import: the binding that uses the name is not
    // reported again.
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" { widget, gizmo }",
            &binding("gizmo_binding", "gizmo"),
        ],
    );
    one_name_error(
        "names-undeclared-braces",
        &[("a", &a), ("b", &b)],
        "a",
        span_of(&a, "widget, ", "gizmo"),
        "the import brings `gizmo` into scope from `/zz/notes/b`, which declares no citation \
         `gizmo`",
    );
    assert_eq!(resolved_in(&[("a", &a), ("b", &b)], "a").0, []);
}

#[test]
fn one_name_from_two_sources() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" { widget }",
            "citation widget -> \"/zz/demo/t1/s9\"",
            &binding("widget_binding", "widget"),
        ],
    );
    let b = module_b();
    let diagnostic = one_name_error(
        "names-two-sources",
        &[("a", &a), ("b", &b)],
        "a",
        span_of(&a, "citation ", "widget"),
        "the name `widget` is declared twice: as a citation, and before that as a name \
         brought into scope from `/zz/notes/b`",
    );
    assert_eq!(diagnostic.related_spans, [span_of(&a, "{ ", "widget")]);
    // A name that stands for two things binds neither.
    assert_eq!(resolved_in(&[("a", &a), ("b", &b)], "a").0, []);
}

#[test]
fn an_alias_and_a_citation_of_one_name() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            "citation b -> \"/zz/demo/t1/s9\"",
        ],
    );
    let b = module_b();
    let diagnostic = one_name_error(
        "names-alias-citation",
        &[("a", &a), ("b", &b)],
        "a",
        span_of(&a, "citation ", "b"),
        "the name `b` is declared twice: as a citation, and before that as the alias of the \
         import of `/zz/notes/b`",
    );
    assert_eq!(diagnostic.related_spans, [span_of(&a, " as ", "b")]);
}

#[test]
fn an_alias_declared_twice() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            "import \"/zz/notes/c\" as b",
            &binding("widget_binding", "b.widget"),
        ],
    );
    let b = module_b();
    let c = note_module("c", &[]);
    let diagnostic = one_name_error(
        "names-alias-twice",
        &[("a", &a), ("b", &b), ("c", &c)],
        "a",
        span_of(&a, "\"/zz/notes/c\" as ", "b"),
        "the name `b` is declared twice: as the alias of the import of `/zz/notes/c`, and \
         before that as the alias of the import of `/zz/notes/b`",
    );
    assert_eq!(
        diagnostic.related_spans,
        [span_of(&a, "\"/zz/notes/b\" as ", "b")]
    );
}

#[test]
fn an_alias_that_another_import_brings_into_scope() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" { widget }",
            "import \"/zz/notes/c\" as widget",
        ],
    );
    let b = module_b();
    let c = note_module("c", &[]);
    let diagnostic = one_name_error(
        "names-alias-brought",
        &[("a", &a), ("b", &b), ("c", &c)],
        "a",
        span_of(&a, "\"/zz/notes/c\" as ", "widget"),
        "the name `widget` is declared twice: as the alias of the import of `/zz/notes/c`, and \
         before that as a name brought into scope from `/zz/notes/b`",
    );
    assert_eq!(diagnostic.related_spans, [span_of(&a, "{ ", "widget")]);
}

#[test]
fn a_binding_named_as_an_alias() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            &binding("b", "\"/zz/demo/t1/s9\""),
        ],
    );
    let b = module_b();
    let diagnostic = one_name_error(
        "names-binding-alias",
        &[("a", &a), ("b", &b)],
        "a",
        span_of(&a, "binding ", "b"),
        "the name `b` is declared twice: as a binding, and before that as the alias of the \
         import of `/zz/notes/b`",
    );
    assert_eq!(diagnostic.related_spans, [span_of(&a, " as ", "b")]);
}

#[test]
fn a_citation_before_a_name_of_its_text_in_braces() {
    let a = note_module(
        "a",
        &[
            "citation widget -> \"/zz/demo/t1/s9\"",
            "import \"/zz/notes/b\" { widget }",
        ],
    );
    let b = module_b();
    let diagnostic = one_name_error(
        "names-citation-brought",
        &[("a", &a), ("b", &b)],
        "a",
        span_of(&a, "{ ", "widget"),
        "the name `widget` is declared twice: as a name brought into scope from \
         `/zz/notes/b`, and before that as a citation",
    );
    assert_eq!(
        diagnostic.related_spans,
        [span_of(&a, "citation ", "widget")]
    );
}

#[test]
fn a_name_reached_only_through_another_modules_import() {
    let c = note_module("c", &["import \"/zz/notes/b\" { widget }"]);
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/c\" as c",
            &binding("widget_binding", "c.widget"),
        ],
    );
    let b = module_b();
    one_name_error(
        "names-through-import",
        &[("a", &a), ("b", &b), ("c", &c)],
        "a",
        span_of(&a, "binds c.", "widget"),
        "`binds` names `c.widget`, and the module `/zz/notes/c`, imported as `c`, declares no \
         citation `widget`; it imports `widget`, and what a module imports is not visible \
         through it",
    );
}

#[test]
#[allow(non_snake_case)]
fn an_import_of_a_module_that_does_not_exist_is_E720() {
    // Nothing that names a citation through the import is reported again.
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/nothing\" as n",
            "import \"/zz/notes/none\" { widget }",
            &binding("by_alias", "n.widget"),
            &binding("by_name", "widget"),
        ],
    );
    let report = check_notes("names-no-module", &[("a", &a)]);
    match report.diagnostics.as_slice() {
        [nothing, none] => {
            for (diagnostic, missing) in [(nothing, "/zz/notes/nothing"), (none, "/zz/notes/none")]
            {
                assert_diagnostic(
                    diagnostic,
                    DiagnosticCode::E720,
                    "zz/notes/a.fr",
                    Some(span_after(&a, "import ", missing)),
                    &format!(
                        "`import` names `{missing}`, which is no module of the workspace or of \
                         a dependency"
                    ),
                );
            }
        }
        other => panic!("expected two diagnostics, found {other:#?}"),
    }
}

#[test]
fn an_import_of_a_module_file_that_does_not_lower_is_not_reported_again() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/d\" as d",
            &binding("widget_binding", "d.widget"),
        ],
    );
    let d = "edition 2\nmodule \"/zz/notes/d\" {\n  unknown\n}\n";
    let report = check_notes("names-unread", &[("a", &a), ("d", d)]);
    let diagnostic = only(&report);
    assert_eq!(diagnostic.code, DiagnosticCode::E100, "{diagnostic:#?}");
    assert_eq!(diagnostic.source_file.as_deref(), Some("zz/notes/d.fr"));
}

#[test]
fn two_modules_may_import_each_other() {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            "citation gadget -> \"/zz/demo/t1/s9\"",
            &binding("widget_binding", "b.widget"),
        ],
    );
    let b = note_module(
        "b",
        &[
            "import \"/zz/notes/a\" { gadget }",
            "citation widget -> \"/zz/demo/t1/s8/a\"",
            &binding("gadget_binding", "gadget"),
        ],
    );
    let modules = [("a", a.as_str()), ("b", b.as_str())];
    assert_eq!(check_notes("names-each-other", &modules).diagnostics, []);
    let (from_a, diagnostics) = resolved_in(&modules, "a");
    assert_eq!(diagnostics, []);
    assert_eq!(targets(&from_a), [("widget_binding", "/zz/demo/t1/s8/a")]);
    let (from_b, diagnostics) = resolved_in(&modules, "b");
    assert_eq!(diagnostics, []);
    assert_eq!(targets(&from_b), [("gadget_binding", "/zz/demo/t1/s9")]);
}

/// A curated workspace whose module `a` has a name not in scope, an import
/// of a module that does not exist, and a name that `b` does not declare.
fn defective_notes() -> Vec<(&'static str, String)> {
    let a = note_module(
        "a",
        &[
            "import \"/zz/notes/b\" as b",
            "import \"/zz/notes/nothing\" as n",
            &binding("gizmo_binding", "gizmo"),
            &binding("sprocket_binding", "b.sprocket"),
        ],
    );
    vec![("a", a), ("b", module_b())]
}

#[test]
fn names_are_resolved_from_interfaces_in_any_order_and_from_the_cache() {
    let files = defective_notes();
    let modules: Vec<(&str, &str)> = files
        .iter()
        .map(|(name, text)| (*name, text.as_str()))
        .collect();
    let scratch = Scratch::new("names-phase-two");
    let workspace = scratch.join("workspace");
    fs::create_dir_all(&workspace).expect("create the workspace's directory");
    notes_workspace(&workspace, &modules);
    let report = check(&workspace);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [
            DiagnosticCode::E720,
            DiagnosticCode::E200,
            DiagnosticCode::E200
        ],
        "{report:#?}"
    );

    // The same interfaces, in memory, in either order.
    let interfaces: Vec<Interface> = modules
        .iter()
        .map(|(_, text)| interface_of_text(text))
        .collect();
    for order in [interfaces.clone(), interfaces.into_iter().rev().collect()] {
        let phase_two = check_interfaces(&order, &[], &notes_manifest(), TOOLCHAIN);
        assert_eq!(phase_two.diagnostics, report.diagnostics);
    }

    // The ranges of the parts come back from the cache.
    let cache = scratch.join("cache");
    let first = check_workspace(&workspace, &cached(&cache, 2));
    let second = check_workspace(&workspace, &cached(&cache, 2));
    assert_eq!(second.from_cache, 2);
    assert_eq!(uncounted(&first), report);
    assert_eq!(uncounted(&second), report);
}

#[test]
#[allow(non_snake_case)]
fn an_address_that_two_modules_declare_is_E700_in_the_later_one() {
    // `s8` has the child `s8/a` and the note `s8/note/demoNotes/nolaw/a`.
    // The unit `s8/a` declares the first as its own address, and the unit
    // `s8/note/demoNotes/nolaw` the second as the address of its child.
    let s8_at = "/zz/demo/t1/s8";
    let mut s8 = unit(s8_at, None, "8", "Widgets", "An invented section.");
    s8.root
        .items
        .push(subsection(s8_at, vec![linking(&["/zz/demo/t1/s1"])]));
    s8.notes.push(linking_note(
        s8_at,
        "a",
        Layer::Enacted,
        &["/zz/demo/t1/s1"],
    ));
    let nolaw_at = "/zz/demo/t1/s8/note/demoNotes/nolaw";
    let mut nolaw = unit(nolaw_at, None, "1", "Not a Note", "An invented unit.");
    nolaw
        .root
        .items
        .push(subsection(nolaw_at, vec![linking(&["/zz/demo/t1/s1"])]));
    let models = [
        Module::Unit(s8),
        Module::Unit(unit(
            "/zz/demo/t1/s8/a",
            None,
            "8a",
            "Not a Subsection",
            "An invented unit.",
        )),
        Module::Unit(nolaw),
    ];
    let files: Vec<(String, String)> = models
        .iter()
        .map(|model| (module_path(address_of(model)), render(model)))
        .collect();
    let files: Vec<(&str, String)> = files
        .iter()
        .map(|(rel, text)| (rel.as_str(), text.clone()))
        .collect();
    let scratch = Scratch::new("declared-twice");
    curated_workspace(scratch.path(), &files);
    let report = check(scratch.path());
    let nolaw_file = module_path(&address(nolaw_at));
    assert_eq!(nolaw_file, "zz/demo/t1/s8/note/demo_notes/nolaw.fr");
    match report.diagnostics.as_slice() {
        [unit, provision] => {
            assert_diagnostic(
                unit,
                DiagnosticCode::E700,
                "zz/demo/t1/s8/a.fr",
                Some(span_after(&files[1].1, "module ", "/zz/demo/t1/s8/a")),
                "the address `/zz/demo/t1/s8/a` is declared in `zz/demo/t1/s8.fr` and again in \
                 `zz/demo/t1/s8/a.fr`, and an address is declared by one module only",
            );
            assert_diagnostic(
                provision,
                DiagnosticCode::E700,
                &nolaw_file,
                None,
                &format!(
                    "the address `{nolaw_at}/a` is declared in `zz/demo/t1/s8.fr` and again \
                     in `{nolaw_file}`"
                ),
            );
        }
        other => panic!("expected two diagnostics, found {other:#?}"),
    }

    // The same from the interfaces in another order.
    let interfaces: Vec<Interface> = models.iter().rev().map(interface_in_memory).collect();
    let phase_two = check_interfaces(&interfaces, &[], &curated_manifest(), TOOLCHAIN);
    assert_eq!(phase_two.diagnostics, report.diagnostics);
}

/// `span` as the byte range an interface records.
fn range_of(span: Span) -> ByteRange {
    ByteRange {
        start: span.start,
        end: span.end,
    }
}

// Dependencies. A curated workspace of the prefix `/zz/notes`, in `notes/`
// of a scratch directory, depends on the generated workspace of the prefix
// `/zz/demo` in `demo/` beside it, named `demo` at `../demo`.

/// The models of the dependency: the demo models, and the unit `s8` of the
/// chapter, with the subsection `a`.
fn dependency_models() -> Vec<Module> {
    let mut models = demo_models();
    container_at(&mut models, 1)
        .members
        .push(address("/zz/demo/t1/s8"));
    let mut s8 = unit(
        "/zz/demo/t1/s8",
        Some(CHAPTER),
        "8",
        "Parts",
        "A widget has invented parts.",
    );
    s8.root.items.push(subsection(
        "/zz/demo/t1/s8",
        vec![Block::Para(plain("An invented subsection."))],
    ));
    models.push(Module::Unit(s8));
    models
}

/// The BLAKE3 of the bytes of the lock of the workspace in `dir`.
fn lock_digest_of(dir: &Path) -> Hash32 {
    let bytes = fs::read(dir.join("law.lock.json")).expect("read the lock");
    Hash32(*blake3::hash(&bytes).as_bytes())
}

/// Writes the generated workspace of `manifest` and `models` into `out`,
/// and gives the BLAKE3 of the bytes of its lock.
fn write_generated(out: &Path, manifest: Manifest, models: &[Module]) -> Hash32 {
    let writer = create(out, manifest);
    for model in models {
        writer
            .write(model)
            .unwrap_or_else(|error| panic!("write: {error}"));
    }
    finish(writer);
    lock_digest_of(out)
}

/// Writes the generated workspace of the prefix `/zz/demo`, of the release
/// `release`, and `models` into `out`, and gives the digest of its lock.
fn write_release(out: &Path, release: &str, models: &[Module]) -> Hash32 {
    let manifest = Manifest {
        release: Some(release.to_owned()),
        ..demo_manifest()
    };
    write_generated(out, manifest, models)
}

/// The entry of the dependency `name` at `path`, pinned to `release` and
/// the lock digest `lock_digest`.
fn dependency(name: &str, path: &str, release: &str, lock_digest: Hash32) -> Dependency {
    Dependency {
        name: name.to_owned(),
        path: path.to_owned(),
        release: release.to_owned(),
        lock_digest,
    }
}

/// Writes into `out` the curated workspace of the prefix `/zz/notes` with
/// `dependencies`, and each `(name, text)` of `modules` at
/// `zz/notes/<name>.fr`.
fn depending(out: &Path, dependencies: Vec<Dependency>, modules: &[(&str, &str)]) {
    fs::create_dir_all(out).expect("create the workspace's directory");
    let manifest = Manifest {
        dependencies,
        ..notes_manifest()
    };
    fs::write(out.join("workspace.json"), manifest.to_json()).expect("write");
    for (name, text) in modules {
        put(out, &format!("zz/notes/{name}.fr"), text);
    }
}

/// A scratch directory with the dependency in `demo/`, of the release
/// `42-7`, and the curated workspace of `modules` in `notes/`, which depends
/// on it.
fn with_dependency(tag: &str, modules: &[(&str, &str)]) -> Scratch {
    let scratch = Scratch::new(tag);
    let digest = write_release(&scratch.join("demo"), "42-7", &dependency_models());
    depending(
        &scratch.join("notes"),
        vec![dependency("demo", "../demo", "42-7", digest)],
        modules,
    );
    scratch
}

/// The count of each outcome of the citations of `report` that is not 0, in
/// the order of the outcomes.
fn citation_counts(citations: &BTreeMap<String, u64>) -> Vec<(&str, u64)> {
    Outcome::NAMES
        .into_iter()
        .map(|name| (name, citations[name]))
        .filter(|(_, count)| *count > 0)
        .collect()
}

/// Asserts that `report` has one diagnostic, of `code`, about the manifest
/// as a whole, and that its message holds each of `words`.
fn about_the_manifest(report: &WorkspaceReport, code: DiagnosticCode, words: &[&str]) {
    let diagnostic = only(report);
    assert_eq!(diagnostic.code, code, "{diagnostic:#?}");
    assert_eq!(
        diagnostic.source_file.as_deref(),
        Some("workspace.json"),
        "{diagnostic:#?}"
    );
    assert_eq!(diagnostic.primary_span, None, "{diagnostic:#?}");
    for word in words {
        assert!(
            diagnostic.message.contains(word),
            "{word:?} is not in {:?}",
            diagnostic.message
        );
    }
}

/// The keys of the check report of `report`, in their order.
fn report_keys(report: &WorkspaceReport) -> Vec<String> {
    let value: serde_json::Value = serde_json::from_str(&report.to_json()).expect("JSON");
    value
        .as_object()
        .expect("an object")
        .keys()
        .cloned()
        .collect()
}

#[test]
fn a_curated_workspace_cites_its_dependency_and_checks_clean() {
    // The binding records the hashes of what it binds, so it is not stale.
    let a = note_module(
        "a",
        &[
            "import \"/zz/demo/t1/s8\" as s8",
            "citation widget -> \"/zz/demo/t1/s8/a\"",
            &staleness::written_against(
                "widget_binding",
                "widget",
                "42-7",
                &dependency_models(),
                "/zz/demo/t1/s8/a",
            ),
        ],
    );
    let scratch = with_dependency("deps-clean", &[("a", &a)]);
    let report = check(&scratch.join("notes"));
    assert_eq!(report.diagnostics, []);
    assert_eq!(citation_counts(&report.citations), [("exact", 1)]);
    assert_eq!(report.unresolved_citations, []);
    assert_eq!(report.unresolved_lines(), "");
    // Only the module files of the workspace itself are counted.
    assert_eq!(
        report.summary(),
        "modules: 1; from cache: 0; errors: 0; warnings: 0"
    );
    assert_eq!(
        report_keys(&report),
        [
            "schema",
            "modules",
            "from_cache",
            "errors",
            "warnings",
            "references",
            "citations",
            "outside_by_body"
        ]
    );
    assert!(
        report
            .to_json()
            .contains("\"citations\": {\n    \"exact\": 1,\n    \"target-status\": 0,"),
        "{}",
        report.to_json()
    );
}

#[test]
#[allow(non_snake_case)]
fn a_different_release_is_E740_and_names_both() {
    let scratch = Scratch::new("deps-release");
    let pinned = write_release(&scratch.join("r1"), "r1", &dependency_models());
    // A copy regenerated as `r2`, with one word changed.
    let mut models = dependency_models();
    *unit_at(&mut models, 2) = unit(
        "/zz/demo/t1/s1",
        Some(CHAPTER),
        "1",
        "Definitions",
        "In this invented chapter, a widget is a tiny device.",
    );
    let found = write_release(&scratch.join("demo"), "r2", &models);
    assert_ne!(pinned, found);
    let a = note_module(
        "a",
        &[
            "import \"/zz/demo/t1/s8\" as s8",
            "citation widget -> \"/zz/demo/t1/s8/a\"",
        ],
    );
    // Pinned to the other release and digest, to the other release, and to
    // the other digest.
    let pins = [("r1", pinned), ("r1", found), ("r2", pinned)];
    for (n, (release, digest)) in pins.into_iter().enumerate() {
        let notes = scratch.join(&format!("notes-{n}"));
        depending(
            &notes,
            vec![dependency("demo", "../demo", release, digest)],
            &[("a", &a)],
        );
        let report = check(&notes);
        about_the_manifest(
            &report,
            DiagnosticCode::E740,
            &[
                "the dependency `demo` at `../demo`",
                &format!("is pinned to the release `{release}` and the lock digest `{digest}`"),
                &format!("its lock is of the release `r2` and has the digest `{found}`"),
            ],
        );
        assert_eq!(report.errors(), 1);
        // The import and the citation into the dependency are not reported
        // again, and the citation is not counted.
        assert_eq!(citation_counts(&report.citations), []);
        assert_eq!(report.unresolved_lines(), "");
    }
}

#[test]
#[allow(non_snake_case)]
fn a_missing_directory_and_a_dependency_without_a_lock_are_E740() {
    let scratch = Scratch::new("deps-not-generated");
    let digest = write_release(&scratch.join("demo"), "42-7", &dependency_models());
    put(scratch.path(), "a-file", "An invented file.\n");
    fs::create_dir_all(scratch.join("empty")).expect("create a directory");
    put(&scratch.join("not-manifest"), "workspace.json", "{}\n");
    put(
        &scratch.join("curated"),
        "workspace.json",
        &curated_manifest().to_json(),
    );
    write_release(&scratch.join("no-lock"), "42-7", &dependency_models());
    fs::remove_file(scratch.join("no-lock/law.lock.json")).expect("remove the lock");
    write_release(&scratch.join("bad-lock"), "42-7", &dependency_models());
    put(
        &scratch.join("bad-lock"),
        "law.lock.json",
        "{\n  \"schema\": \"fidryn.law-lock/v0.1\"\n}\n",
    );
    // Each directory, what the message says of it, and whether its prefix
    // is known. A citation below the prefix of a dependency that failed is
    // left out; when that prefix is not known, so is every citation below no
    // known prefix. A citation of the workspace's own module is counted, and
    // the import into the dependency is reported in neither case.
    let cases = [
        ("nothing", "and there is no directory there", false),
        ("a-file", "and there is no directory there", false),
        ("empty", "which holds no `workspace.json`", false),
        (
            "not-manifest",
            "whose `workspace.json` is no manifest",
            false,
        ),
        (
            "curated",
            "which holds a curated workspace, and a dependency is a generated workspace",
            true,
        ),
        (
            "no-lock",
            "which holds a generated workspace without a lock, `law.lock.json`",
            true,
        ),
        ("bad-lock", "whose `law.lock.json` is no lock", true),
    ];
    let a = note_module(
        "a",
        &[
            "import \"/zz/demo/t1/s99\" as gone",
            "citation widget -> \"/zz/demo/t1/s8/a\"",
            "citation other -> \"/zz/other/x\"",
            "citation mine -> \"/zz/notes/a\"",
        ],
    );
    for (dir, words, known) in cases {
        let notes = scratch.join(&format!("notes-{dir}"));
        let path = format!("../{dir}");
        depending(
            &notes,
            vec![dependency("demo", &path, "42-7", digest)],
            &[("a", &a)],
        );
        let report = check(&notes);
        about_the_manifest(
            &report,
            DiagnosticCode::E740,
            &[
                &format!("the dependency `demo` names the directory `{path}`, "),
                words,
            ],
        );
        let counts = citation_counts(&report.citations);
        if known {
            assert_eq!(counts, [("exact", 1), ("outside", 1)], "{dir}");
        } else {
            assert_eq!(counts, [("exact", 1)], "{dir}");
        }
    }
}

#[test]
#[allow(non_snake_case)]
fn overlapping_prefixes_and_an_absolute_path_are_E730() {
    let scratch = Scratch::new("deps-overlap");
    let digest = write_release(&scratch.join("demo"), "42-7", &dependency_models());
    let demo = || dependency("demo", "../demo", "42-7", digest);
    let tail = "; no prefix of a workspace and of its dependencies is, contains or lies within \
                another";

    // The prefix of the dependency and that of the workspace.
    let relations = [
        ("/zz/demo", "which is also the prefix of this workspace"),
        (
            "/zz",
            "which lies within the prefix `/zz` of this workspace",
        ),
        (
            "/zz/demo/t1",
            "which contains the prefix `/zz/demo/t1` of this workspace",
        ),
    ];
    for (n, (prefix, words)) in relations.into_iter().enumerate() {
        let notes = scratch.join(&format!("notes-{n}"));
        fs::create_dir_all(&notes).expect("create the workspace's directory");
        let manifest = Manifest {
            prefix: address(prefix),
            dependencies: vec![demo()],
            ..notes_manifest()
        };
        fs::write(notes.join("workspace.json"), manifest.to_json()).expect("write");
        about_the_manifest(
            &check(&notes),
            DiagnosticCode::E730,
            &[
                "the dependency `demo` has the prefix `/zz/demo`, ",
                words,
                tail,
            ],
        );
    }

    // Two dependencies: the later is reported, naming the earlier, which is
    // looked up as before.
    let copy = write_release(&scratch.join("copy"), "42-7", &dependency_models());
    let wide_manifest = Manifest {
        name: "wide".to_owned(),
        prefix: address("/zz"),
        ..demo_manifest()
    };
    let wide = write_generated(&scratch.join("wide"), wide_manifest, &dependency_models());
    // A citation below the prefix of `demo` is looked up there; one below
    // the prefix of the dependency that failed and below no other is left
    // out, and one below no prefix at all is `outside`.
    let cites = note_module(
        "a",
        &[
            "citation widget -> \"/zz/demo/t1/s8/a\"",
            "citation other -> \"/zz/other/x\"",
        ],
    )
    .replace("/zz/notes/a", "/yy/notes/a");
    let others = [
        (
            dependency("copy", "../copy", "42-7", copy),
            "the dependency `copy` has the prefix `/zz/demo`, which is also the prefix of the \
             dependency `demo`",
            vec![("exact", 1), ("outside", 1)],
        ),
        (
            dependency("wide", "../wide", "42-7", wide),
            "the dependency `wide` has the prefix `/zz`, which contains the prefix `/zz/demo` of \
             the dependency `demo`",
            vec![("exact", 1)],
        ),
    ];
    for (other, words, counts) in others {
        let notes = scratch.join(&format!("notes-{}", other.name));
        fs::create_dir_all(&notes).expect("create the workspace's directory");
        let manifest = Manifest {
            prefix: address("/yy/notes"),
            dependencies: vec![demo(), other],
            ..notes_manifest()
        };
        fs::write(notes.join("workspace.json"), manifest.to_json()).expect("write");
        put(&notes, "yy/notes/a.fr", &cites);
        let report = check(&notes);
        about_the_manifest(&report, DiagnosticCode::E730, &[words, tail]);
        assert_eq!(citation_counts(&report.citations), counts);
    }
    let a = note_module("a", &["citation widget -> \"/zz/demo/t1/s8/a\""]);

    // A path that is absolute.
    let absolute = scratch.join("demo").display().to_string();
    let notes = scratch.join("notes-absolute");
    depending(
        &notes,
        vec![dependency("demo", &absolute, "42-7", digest)],
        &[("a", &a)],
    );
    about_the_manifest(
        &check(&notes),
        DiagnosticCode::E730,
        &[&format!(
            "the dependency `demo` has the path `{absolute}`, which is absolute, and the path of \
             a dependency is relative to the directory of the manifest"
        )],
    );
}

#[test]
#[allow(non_snake_case)]
fn two_dependencies_of_one_name_are_E730() {
    let a = note_module("a", &["citation widget -> \"/zz/demo/t1/s8/a\""]);
    let scratch = with_dependency("deps-one-name", &[("a", &a)]);
    let copy = write_release(&scratch.join("copy"), "42-7", &dependency_models());
    let notes = scratch.join("notes");
    let digest = lock_digest_of(&scratch.join("demo"));
    depending(
        &notes,
        vec![
            dependency("demo", "../demo", "42-7", digest),
            dependency("demo", "../copy", "42-7", copy),
        ],
        &[("a", &a)],
    );
    let report = check(&notes);
    about_the_manifest(
        &report,
        DiagnosticCode::E730,
        &[
            "the dependency `demo` is listed twice, at `../demo` and at `../copy`, and each \
           dependency has a name of its own",
        ],
    );
    // The first is looked up as before.
    assert_eq!(citation_counts(&report.citations), [("exact", 1)]);

    // An entry refused for its absolute path still has its name, and a
    // later entry of that name is listed twice.
    let absolute = scratch.join("copy").display().to_string();
    depending(
        &notes,
        vec![
            dependency("demo", &absolute, "42-7", copy),
            dependency("demo", "../demo", "42-7", digest),
        ],
        &[("a", &a)],
    );
    let report = check(&notes);
    match report.diagnostics.as_slice() {
        [first, second] => {
            assert_eq!(
                (first.code, second.code),
                (DiagnosticCode::E730, DiagnosticCode::E730),
                "{report:#?}"
            );
            let messages = [first.message.as_str(), second.message.as_str()];
            assert!(
                messages
                    .iter()
                    .any(|message| message.contains("which is absolute")),
                "{messages:#?}"
            );
            assert!(
                messages.iter().any(|message| message.contains(&format!(
                    "the dependency `demo` is listed twice, at `{absolute}` and at `../demo`"
                ))),
                "{messages:#?}"
            );
        }
        other => panic!("expected two diagnostics, found {other:#?}"),
    }
}

#[test]
fn a_target_that_is_no_address_is_malformed_beside_a_failed_dependency() {
    // A unit of the workspace links to a target that is no address and
    // begins with the prefix of the dependency, and to one that begins
    // with no prefix; both are `malformed` whether or not the prefix of the
    // dependency that failed is known.
    let scratch = Scratch::new("deps-malformed");
    let digest = write_release(&scratch.join("demo"), "42-7", &dependency_models());
    let mut s1 = unit("/zz/notes/s1", None, "1", "Notes", "Invented notes.");
    s1.root.items.push(Item::Part {
        role: PartRole::Content,
        blocks: vec![linking(&["/zz/demo/t1/s1/a ", "not an address"])],
    });
    let s1_text = render(&Module::Unit(s1));
    let entries = [
        dependency("demo", "../demo", "r9", digest),
        dependency("demo", "../nothing", "42-7", digest),
    ];
    for (n, entry) in entries.into_iter().enumerate() {
        let notes = scratch.join(&format!("notes-{n}"));
        depending(&notes, vec![entry], &[("s1", &s1_text)]);
        let report = check(&notes);
        assert_eq!(report.errors(), 1, "{report:#?}");
        let operative = &report.references[&Layer::Operative];
        assert_eq!(operative["malformed"], 2, "{report:#?}");
        assert_eq!(operative.values().sum::<u64>(), 2, "{report:#?}");
    }
}

#[test]
fn a_dependency_with_an_error_is_reported_once() {
    let scratch = Scratch::new("deps-own-error");
    let demo = scratch.join("demo");
    // A dependency that records another toolchain, a warning, with a
    // module file changed since its lock was written and a file of the
    // first edition, errors.
    let manifest = Manifest {
        toolchain: "0.0.1".to_owned(),
        ..demo_manifest()
    };
    let digest = write_generated(&demo, manifest, &dependency_models());
    let rel = "zz/demo/t1/s2.fr";
    put(
        &demo,
        rel,
        &read(&demo, rel).replacen("small or large", "small, medium or large", 1),
    );
    put(
        &demo,
        "zz/demo/t1/old.fr",
        "module Demo.Old version \"0.1.0\" {\n}\n",
    );
    let own = check(&demo);
    let errors = own.errors();
    assert!(errors >= 3, "{own:#?}");
    assert_eq!(own.warnings(), 1, "{own:#?}");

    let a = note_module(
        "a",
        &[
            "import \"/zz/demo/t1/s99\" as gone",
            "citation widget -> \"/zz/demo/t1/s8/a\"",
        ],
    );
    let notes = scratch.join("notes");
    depending(
        &notes,
        vec![dependency("demo", "../demo", "42-7", digest)],
        &[("a", &a)],
    );
    let report = check(&notes);
    about_the_manifest(
        &report,
        DiagnosticCode::E740,
        &[&format!(
            "the dependency `demo` at `../demo` has {errors} errors of its own, which `fidryn \
             check` of its directory shows"
        )],
    );
    assert_eq!((report.errors(), report.warnings()), (1, 0));
    assert_eq!(citation_counts(&report.citations), []);
    assert_eq!(report.unresolved_lines(), "");

    // Phase two leaves out what lies below the prefix of a dependency that
    // failed, and reports no import into it; when that prefix is not known,
    // it leaves out what lies below no known prefix.
    let b = note_module(
        "b",
        &[
            "citation mine -> \"/zz/notes/b\"",
            "citation other -> \"/zz/other/x\"",
        ],
    );
    let interfaces = [interface_of_text(&a), interface_of_text(&b)];
    let failures = [
        (
            Some(address("/zz/demo")),
            vec![("exact", 1), ("outside", 1)],
        ),
        (None, vec![("exact", 1)]),
    ];
    for (prefix, counts) in failures {
        let failed = DependencyInterfaces::Failed { prefix };
        let phase_two = check_interfaces(&interfaces, &[failed], &notes_manifest(), TOOLCHAIN);
        assert_eq!(phase_two.diagnostics, []);
        assert_eq!(citation_counts(&phase_two.citations), counts);
    }
}

#[test]
fn references_into_a_dependency_get_outcomes() {
    let scratch = Scratch::new("deps-outcomes");
    let demo_digest = write_release(&scratch.join("demo"), "42-7", &dependency_models());
    // A second dependency, of the prefix `/zz/more`, with one container.
    let more_models = [Module::Container(container(
        "/zz/more/t1",
        None,
        "title",
        "Title 1—",
        "More Invented Things",
        &[],
    ))];
    let more_manifest = Manifest {
        name: "more".to_owned(),
        prefix: address("/zz/more"),
        ..demo_manifest()
    };
    let more_digest = write_generated(&scratch.join("more"), more_manifest, &more_models);
    let a = note_module(
        "a",
        &[
            "citation widget -> \"/zz/demo/t1/s8/a\"",
            "citation part -> \"/zz/demo/t1/s8/z\"",
            "citation title -> \"/zz/demo/t9\"",
            "citation more -> \"/zz/more/t1\"",
            "citation sibling -> \"/zz/notes/b\"",
            "citation other -> \"/zz/other/x\"",
        ],
    );
    let b = note_module("b", &["citation section -> \"/zz/demo/t1/s1\""]);
    // A unit of the workspace links into the dependency too.
    let mut s1 = unit("/zz/notes/s1", None, "1", "Notes", "Invented notes.");
    s1.root.items.push(Item::Part {
        role: PartRole::Content,
        blocks: vec![linking(&["/zz/demo/t1/s8/z", "/zz/demo/t9"])],
    });
    let s1 = Module::Unit(s1);
    let s1_text = render(&s1);
    let notes = scratch.join("notes");
    let dependencies = vec![
        dependency("demo", "../demo", "42-7", demo_digest),
        dependency("more", "../more", "42-7", more_digest),
    ];
    depending(
        &notes,
        dependencies.clone(),
        &[("a", &a), ("b", &b), ("s1", &s1_text)],
    );
    let report = check(&notes);
    assert_eq!(report.diagnostics, []);
    assert_eq!(
        citation_counts(&report.citations),
        [
            ("exact", 4),
            ("unit-only", 1),
            ("no-unit", 1),
            ("outside", 1)
        ]
    );
    let operative = &report.references[&Layer::Operative];
    assert_eq!((operative["unit-only"], operative["no-unit"]), (1, 1));
    let unresolved = |citation: &str, target: &str, outcome: Outcome| UnresolvedCitation {
        from: address("/zz/notes/a"),
        citation: citation.to_owned(),
        target: address(target),
        outcome,
    };
    assert_eq!(
        report.unresolved_citations,
        [
            unresolved("part", "/zz/demo/t1/s8/z", Outcome::UnitOnly),
            unresolved("title", "/zz/demo/t9", Outcome::NoUnit),
            unresolved(
                "other",
                "/zz/other/x",
                Outcome::Outside("/zz/other".to_owned())
            ),
        ]
    );
    assert_eq!(
        report.unresolved_lines(),
        concat!(
            r#"{"from":"/zz/notes/a","citation":"part","target":"/zz/demo/t1/s8/z","outcome":"unit-only"}"#,
            "\n",
            r#"{"from":"/zz/notes/a","citation":"title","target":"/zz/demo/t9","outcome":"no-unit"}"#,
            "\n",
            r#"{"from":"/zz/notes/a","citation":"other","target":"/zz/other/x","outcome":"outside"}"#,
            "\n",
            r#"{"from":"/zz/notes/s1","layer":"operative","target":"/zz/demo/t1/s8/z","outcome":"unit-only"}"#,
            "\n",
            r#"{"from":"/zz/notes/s1","layer":"operative","target":"/zz/demo/t9","outcome":"no-unit"}"#,
            "\n",
        )
    );
    assert!(
        report.to_json().contains(
            "\"citations\": {\n    \"exact\": 4,\n    \"target-status\": 0,\n    \
             \"ambiguous\": 0,\n    \"unit-only\": 1,\n    \"range-form\": 0,\n    \
             \"no-unit\": 1,\n    \"outside\": 1,\n    \"malformed\": 0\n  },"
        ),
        "{}",
        report.to_json()
    );
    // The same on eight workers.
    assert_eq!(check_workspace(&notes, &with_jobs(8)), report);

    // The same from the interfaces, in memory, in any order of the
    // dependencies and of the modules of each.
    let own = vec![
        interface_of_text(&a),
        interface_of_text(&b),
        interface_in_memory(&s1),
    ];
    let demo: Vec<Interface> = dependency_models()
        .iter()
        .map(interface_in_memory)
        .collect();
    let more: Vec<Interface> = more_models.iter().map(interface_in_memory).collect();
    let given = |interfaces: &[Interface], prefix: &str| DependencyInterfaces::Loaded {
        name: prefix.rsplit('/').next().unwrap_or_default().to_owned(),
        release: "42-7".to_owned(),
        prefix: address(prefix),
        interfaces: interfaces.to_vec(),
    };
    let reversed = |interfaces: &[Interface]| interfaces.iter().rev().cloned().collect::<Vec<_>>();
    let manifest = Manifest {
        dependencies,
        ..notes_manifest()
    };
    let orders = [
        (
            own.clone(),
            vec![given(&demo, "/zz/demo"), given(&more, "/zz/more")],
        ),
        (
            reversed(&own),
            vec![
                given(&more, "/zz/more"),
                given(&reversed(&demo), "/zz/demo"),
            ],
        ),
    ];
    for (interfaces, dependencies) in orders {
        let phase_two = check_interfaces(&interfaces, &dependencies, &manifest, TOOLCHAIN);
        assert_eq!(phase_two.diagnostics, report.diagnostics);
        assert_eq!(phase_two.references, report.references);
        assert_eq!(phase_two.unresolved, report.unresolved);
        assert_eq!(phase_two.citations, report.citations);
        assert_eq!(phase_two.unresolved_citations, report.unresolved_citations);
    }
}

#[test]
fn checking_writes_nothing_into_the_dependency() {
    let a = note_module("a", &["citation widget -> \"/zz/demo/t1/s8/a\""]);
    let scratch = with_dependency("deps-no-write", &[("a", &a)]);
    let demo = scratch.join("demo");
    // Entries whose names begin with a dot, which are not the dependency's:
    // a repository's directory, a file, and a cache directory of its own.
    put(&demo, ".git/HEAD", "ref: refs/heads/invented\n");
    put(&demo, ".notes", "Invented notes.\n");
    put(&demo, ".fidryn-cache/README", "An invented file.\n");
    let before = tree(&demo);
    let notes = scratch.join("notes");
    // A cache named inside the depending workspace.
    let cache = notes.join(".fidryn-cache");
    let uncached = check_workspace(&notes, &with_jobs(2));
    let first = check_workspace(&notes, &cached(&cache, 2));
    let second = check_workspace(&notes, &cached(&cache, 2));
    assert_eq!(tree(&demo), before);
    assert_eq!(uncached.diagnostics, []);
    assert_eq!(citation_counts(&uncached.citations), [("exact", 1)]);
    assert_eq!(first, uncached);
    // Only the workspace's own module is counted, and read from the cache.
    assert_eq!((second.modules, second.from_cache), (1, 1));
    assert_eq!(uncounted(&second), uncached);
    // The entry of each module file of the dependency is in the cache of
    // the depending workspace.
    let keys = Cache::new(&cache, TOOLCHAIN, BUILD);
    let lock = Lock::from_json(&read(&demo, "law.lock.json")).expect("the lock");
    assert_eq!(lock.modules.len(), 6);
    for rel in lock.modules.keys() {
        let (_, key) = digest_and_key(&keys, &demo, rel);
        assert!(keys.entry_path(&key).is_file(), "{rel}");
    }
}

/// `report` as a check without a cache gives it, which says nothing of a
/// cache it did not use.
fn without_cache_note(report: &WorkspaceReport) -> WorkspaceReport {
    WorkspaceReport {
        cache_unused: None,
        ..report.clone()
    }
}

#[test]
fn a_cache_inside_a_dependency_is_not_used() {
    let a = note_module("a", &["citation widget -> \"/zz/demo/t1/s8/a\""]);
    let scratch = with_dependency("deps-cache-inside", &[("a", &a)]);
    let demo = scratch.join("demo");
    let notes = scratch.join("notes");
    put(&demo, ".fidryn-cache/README", "An invented file.\n");
    let uncached = check_workspace(&notes, &with_jobs(2));
    assert_eq!(uncached.diagnostics, []);
    assert_eq!(uncached.cache_unused, None);
    let inside = "the directory lies inside the directory of the dependency `demo`, `../demo`";
    let mut caches = vec![
        // The dependency's own cache, which a check of its directory keeps.
        (demo.join(".fidryn-cache"), inside),
        // A directory below one that does not exist yet.
        (demo.join("new/cache"), inside),
        // The dependency's directory itself, named through `..`.
        (
            notes.join("../demo"),
            "the directory is the directory of the dependency `demo`, `../demo`",
        ),
        // A directory that does not exist, whose `..` leads into it.
        (scratch.join("new/../demo/cache"), inside),
    ];
    #[cfg(unix)]
    {
        // A link beside the dependency that leads into it.
        std::os::unix::fs::symlink(&demo, scratch.join("link")).expect("link");
        caches.push((scratch.join("link/cache"), inside));
    }
    let before = tree(&demo);
    for (cache, words) in &caches {
        for _ in 0..2 {
            let report = check_workspace(&notes, &cached(cache, 2));
            let why = report.cache_unused.as_deref().unwrap_or_default();
            assert_eq!(
                why,
                format!(
                    "{}: {words}, and nothing is written inside a dependency",
                    cache.display()
                ),
                "{}",
                cache.display()
            );
            assert_eq!(without_cache_note(&report), uncached, "{}", cache.display());
        }
        assert_eq!(tree(&demo), before, "{}", cache.display());
    }
    assert!(!scratch.join("new").exists());
}

#[test]
fn a_cache_that_holds_a_dependency_is_not_used() {
    // The dependency stands where the cache writes its entries, `v1` in the
    // cache's directory, and holds entries whose names begin with a dot.
    let scratch = Scratch::new("deps-cache-holds");
    let cache = scratch.join("cache");
    let demo = cache.join("v1");
    let digest = write_release(&demo, "42-7", &dependency_models());
    put(&demo, ".git/HEAD", "ref: refs/heads/invented\n");
    put(&demo, ".notes", "Invented notes.\n");
    let notes = scratch.join("notes");
    let a = note_module("a", &["citation widget -> \"/zz/demo/t1/s8/a\""]);
    depending(
        &notes,
        vec![dependency("demo", "../cache/v1", "42-7", digest)],
        &[("a", &a)],
    );
    let uncached = check_workspace(&notes, &with_jobs(2));
    assert_eq!(uncached.diagnostics, []);
    assert_eq!(uncached.cache_unused, None);
    assert_eq!(citation_counts(&uncached.citations), [("exact", 1)]);
    let caches = [
        // The directory whose `v1` the dependency is.
        cache.clone(),
        // The same directory, named through `..`.
        notes.join("../cache"),
        // The directory that holds both workspaces.
        scratch.path().to_owned(),
    ];
    let before = tree(&demo);
    for dir in &caches {
        for _ in 0..2 {
            let report = check_workspace(&notes, &cached(dir, 2));
            assert_eq!(
                report.cache_unused.as_deref().unwrap_or_default(),
                format!(
                    "{}: the directory of the dependency `demo`, `../cache/v1`, lies inside \
                     the directory, and nothing is written inside a dependency",
                    dir.display()
                ),
                "{}",
                dir.display()
            );
            assert_eq!(without_cache_note(&report), uncached, "{}", dir.display());
        }
        assert_eq!(tree(&demo), before, "{}", dir.display());
    }
    // No entry was written anywhere, beside the dependency either.
    assert!(!scratch.join("v1").exists());
}

#[test]
fn a_workspace_inside_its_dependency_keeps_no_cache() {
    // The depending workspace stands in a directory of its dependency and
    // names it as `..`, so that a cache named inside the depending workspace
    // is inside the dependency.
    let scratch = Scratch::new("deps-nested");
    let demo = scratch.join("demo");
    let digest = write_release(&demo, "42-7", &dependency_models());
    let notes = demo.join("notes");
    let a = note_module("a", &["citation widget -> \"/zz/demo/t1/s8/a\""]);
    depending(
        &notes,
        vec![dependency("demo", "..", "42-7", digest)],
        &[("a", &a)],
    );
    let before = tree(&demo);
    let uncached = check_workspace(&notes, &with_jobs(2));
    let report = check_workspace(&notes, &cached(&notes.join(".fidryn-cache"), 2));
    assert_eq!(tree(&demo), before);
    assert!(report.cache_unused.is_some(), "{report:#?}");
    assert_eq!(without_cache_note(&report), uncached);
    // The files of the depending workspace are files of the dependency, which
    // fails its own check.
    assert!(
        uncached
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::E740
                && diagnostic.message.contains("of its own")),
        "{uncached:#?}"
    );
}

#[test]
fn a_cache_beside_a_dependency_is_used() {
    let a = note_module("a", &["citation widget -> \"/zz/demo/t1/s8/a\""]);
    let scratch = with_dependency("deps-cache-beside", &[("a", &a)]);
    let demo = scratch.join("demo");
    let notes = scratch.join("notes");
    // Beside the dependency, with a name that begins with the dependency's.
    let cache = scratch.join("demo-cache");
    let before = tree(&demo);
    let first = check_workspace(&notes, &cached(&cache, 2));
    let second = check_workspace(&notes, &cached(&cache, 2));
    assert_eq!(
        (first.cache_unused.as_ref(), second.cache_unused.as_ref()),
        (None, None)
    );
    assert_eq!(first.diagnostics, []);
    assert_eq!((second.modules, second.from_cache), (1, 1));
    assert_eq!(uncounted(&second), first);
    assert!(cache.join("v1").is_dir());
    assert_eq!(tree(&demo), before);
}

/// How a test puts something other than a regular file of a bounded size
/// where a workspace has its manifest or its lock.
#[cfg(unix)]
#[derive(Clone, Copy, Debug)]
enum NotRegular {
    /// A symbolic link to a named pipe beside it, which waits for ever when
    /// it is followed and opened.
    Link,
    /// A named pipe, which no one writes.
    Pipe,
    /// A file one byte larger than the bound of 64 MiB, of no bytes but
    /// zeros, which takes no room on the disk.
    TooLarge,
}

/// Makes a named pipe at `path`.
#[cfg(unix)]
fn make_pipe(path: &Path) {
    let made = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .expect("run mkfifo");
    assert!(made.success(), "mkfifo {}", path.display());
}

#[cfg(unix)]
impl NotRegular {
    /// Puts this at `path`, where the file `path` was, which is moved aside;
    /// gives the named pipes it made.
    fn put(self, path: &Path) -> Vec<PathBuf> {
        fs::rename(path, path.with_extension("kept")).expect("move the file aside");
        match self {
            NotRegular::Link => {
                let pipe = path.with_extension("pipe");
                make_pipe(&pipe);
                std::os::unix::fs::symlink(&pipe, path).expect("link");
                vec![pipe]
            }
            NotRegular::Pipe => {
                make_pipe(path);
                vec![path.to_owned()]
            }
            NotRegular::TooLarge => {
                fs::File::create(path)
                    .and_then(|file| file.set_len(64 * 1024 * 1024 + 1))
                    .expect("make a large file");
                Vec::new()
            }
        }
    }

    /// The end of the sentence that refuses it.
    fn refusal(self) -> &'static str {
        match self {
            NotRegular::Link | NotRegular::Pipe => {
                "neither a file nor a directory; a workspace check follows no symbolic link"
            }
            NotRegular::TooLarge => "larger than 67108864 bytes (64 MiB)",
        }
    }
}

/// The check of the workspace at `root` with `options`, run on a thread of
/// its own, or `None` when it has not ended within a minute. A check that
/// waits on a named pipe is then let go by opening each pipe of `pipes` for
/// writing, so that no test waits for ever.
#[cfg(unix)]
fn check_within_a_minute(
    root: &Path,
    options: CheckOptions,
    pipes: &[PathBuf],
) -> Option<WorkspaceReport> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let root = root.to_owned();
    let worker = std::thread::spawn(move || {
        let _ = sender.send(check_workspace(&root, &options));
    });
    match receiver.recv_timeout(std::time::Duration::from_secs(60)) {
        Ok(report) => {
            worker.join().expect("the check ends");
            Some(report)
        }
        Err(_) => {
            for pipe in pipes {
                let _ = fs::OpenOptions::new().write(true).open(pipe);
            }
            None
        }
    }
}

#[cfg(unix)]
#[test]
fn a_manifest_or_a_lock_is_read_only_as_a_regular_file_of_a_bounded_size() {
    let a = note_module("a", &["citation widget -> \"/zz/demo/t1/s8/a\""]);
    for kind in [NotRegular::Link, NotRegular::Pipe, NotRegular::TooLarge] {
        for file in ["workspace.json", "law.lock.json"] {
            // In a workspace that is checked.
            let scratch = Scratch::new("not-regular");
            write_workspace(scratch.path(), &demo_models());
            let pipes = kind.put(&scratch.join(file));
            let Some(report) = check_within_a_minute(scratch.path(), with_jobs(2), &pipes) else {
                panic!("{kind:?} as `{file}`: the check did not end");
            };
            let about: Vec<&Diagnostic> = report
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.source_file.as_deref() == Some(file))
                .collect();
            assert_eq!(about.len(), 1, "{kind:?} as `{file}`: {report:#?}");
            assert_eq!(about[0].code, DiagnosticCode::E730, "{kind:?} as `{file}`");
            assert!(
                about[0].message.contains(kind.refusal()),
                "{kind:?} as `{file}`: {}",
                about[0].message
            );
            if file == "workspace.json" {
                assert_eq!(report.diagnostics.len(), 1, "{report:#?}");
                // The writer of a generated workspace refuses the directory
                // as well, without opening the entry.
                let (sender, receiver) = std::sync::mpsc::channel();
                let out = scratch.path().to_owned();
                std::thread::spawn(move || {
                    let created = GeneratedWriter::create(&out, demo_manifest());
                    let _ = sender.send(created.err().map(|error| error.to_string()));
                });
                let held = match kind {
                    NotRegular::Link | NotRegular::Pipe => {
                        "holds a `workspace.json` that is neither a file nor a directory"
                    }
                    NotRegular::TooLarge => {
                        "holds a `workspace.json` that is larger than 67108864 bytes"
                    }
                };
                match receiver.recv_timeout(std::time::Duration::from_secs(60)) {
                    Ok(Some(error)) => assert!(error.contains(held), "{kind:?}: {error}"),
                    Ok(None) => panic!("{kind:?}: the writer accepted the directory"),
                    Err(_) => {
                        for pipe in &pipes {
                            let _ = fs::OpenOptions::new().write(true).open(pipe);
                        }
                        panic!("{kind:?}: the writer did not end");
                    }
                }
            }

            // In a dependency of a curated workspace.
            let scratch = with_dependency("not-regular-dependency", &[("a", &a)]);
            let pipes = kind.put(&scratch.join("demo").join(file));
            let notes = scratch.join("notes");
            let Some(report) = check_within_a_minute(&notes, with_jobs(2), &pipes) else {
                panic!("{kind:?} as the dependency's `{file}`: the check did not end");
            };
            let refused = report
                .diagnostics
                .iter()
                .find(|diagnostic| diagnostic.code == DiagnosticCode::E740)
                .unwrap_or_else(|| panic!("{kind:?} as `{file}`: {report:#?}"));
            assert_eq!(refused.source_file.as_deref(), Some("workspace.json"));
            let clause = match kind {
                NotRegular::Link | NotRegular::Pipe => {
                    "is neither a file nor a directory, and a workspace check follows no \
                     symbolic link"
                }
                NotRegular::TooLarge => {
                    "is larger than 67108864 bytes (64 MiB), the most a workspace check reads \
                     of it"
                }
            };
            assert_eq!(
                refused.message,
                format!(
                    "the dependency `demo` names the directory `../demo`, whose `{file}` {clause}"
                ),
                "{kind:?}"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn an_entry_of_the_cache_that_is_no_regular_file_of_a_bounded_size_is_a_miss() {
    let scratch = Scratch::new("cache-not-regular");
    write_workspace(&scratch.join("ws"), &demo_models());
    let cache = scratch.join("cache");
    let uncached = check_workspace(&scratch.join("ws"), &with_jobs(1));
    let first = check_workspace(&scratch.join("ws"), &cached(&cache, 1));
    assert_eq!(first, uncached);
    let keys = Cache::new(&cache, TOOLCHAIN, BUILD);
    let (_, key) = digest_and_key(&keys, &scratch.join("ws"), "zz/demo/t1/s1.fr");
    let entry = keys.entry_path(&key);
    for kind in [NotRegular::Link, NotRegular::Pipe, NotRegular::TooLarge] {
        let pipes = kind.put(&entry);
        let Some(report) = check_within_a_minute(&scratch.join("ws"), cached(&cache, 1), &pipes)
        else {
            panic!("{kind:?} as an entry: the check did not end");
        };
        // The entry is a miss, and is written again as a regular file.
        assert_eq!(report.from_cache, uncached.modules - 1, "{kind:?}");
        assert_eq!(uncounted(&report), uncached, "{kind:?}");
        let written = fs::symlink_metadata(&entry).expect("the entry");
        assert!(written.file_type().is_file(), "{kind:?}");
        assert!(written.len() < 1024 * 1024, "{kind:?}");
        let _ = fs::remove_file(entry.with_extension("kept"));
    }
}
