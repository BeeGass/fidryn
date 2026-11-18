//! A binding of a curated module records the hashes of what it binds, which
//! `fidryn text DIR ADDRESS --hashes` prints, and `fidryn check DIR` warns
//! (`W750`) when what it was written against has changed. The workspaces here
//! are invented, in the corpora `/zz/demo` and `/zz/notes`: a generated
//! workspace that the tests write with the writer, in two versions, and a
//! curated workspace that depends on it.

use fidryn_driver::workspace::GeneratedWriter;
use fidryn_provision::hash::{hash_provision, layer_hashes_of_unit};
use fidryn_provision::workspace::{
    AddressCounts, Dependency, Generator, Manifest, NotImported, WorkspaceKind,
};
use fidryn_provision::{
    Address, Block, ContainerModule, Hash32, Item, Layer, Module, Note, Num, PartRole, Provision,
    Run, Text, UnitModule,
};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// `fidryn ARGS...`, with no variable that could name the directory of a
/// cache, so that no cache of the user who runs the test is written.
fn fidryn(args: &[&OsStr]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fidryn"));
    command.args(args);
    for name in ["FIDRYN_CACHE_DIR", "XDG_CACHE_HOME", "HOME", "LOCALAPPDATA"] {
        command.env_remove(name);
    }
    command.output().expect("run fidryn")
}

/// `fidryn text DIR ARGS...`.
fn text(dir: &Path, args: &[&str]) -> Output {
    let mut all = vec![OsStr::new("text"), dir.as_os_str()];
    all.extend(args.iter().map(OsStr::new));
    fidryn(&all)
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("the output is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

static SCRATCH: AtomicU64 = AtomicU64::new(0);

/// A directory of its own under the temporary directory, removed when
/// dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Scratch {
        let n = SCRATCH.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fidryn-cli-bindings-{}-{n}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch directory");
        Scratch(dir)
    }

    fn join(&self, rel: &str) -> PathBuf {
        self.0.join(rel)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn address(text: &str) -> Address {
    Address::parse(text).unwrap_or_else(|error| panic!("{text}: {error}"))
}

fn plain(text: &str) -> Text {
    Text {
        runs: vec![Run::Words(text.to_owned())],
    }
}

fn num(value: &str, shown: &str) -> Option<Num> {
    Some(Num {
        value: value.to_owned(),
        shown: plain(shown),
    })
}

/// A provision at `at` of the kind `kind`, numbered `value`, that says
/// `words`, with the children `children`.
fn provision(
    at: &str,
    kind: &str,
    value: &str,
    words: &str,
    children: Vec<Provision>,
) -> Provision {
    let mut items = vec![Item::Part {
        role: PartRole::Content,
        blocks: vec![Block::Para(plain(words))],
    }];
    items.extend(children.into_iter().map(Item::Child));
    Provision {
        address: address(at),
        kind: kind.to_owned(),
        num: num(value, &format!("({value})")),
        heading: None,
        status: None,
        items,
    }
}

/// An enacted note `name` of the module at `module` that says `words`.
fn enacted(module: &str, name: &str, words: &str) -> Note {
    Note {
        address: address(&format!("{module}/note/demoNotes/nolaw/{name}")),
        layer: Layer::Enacted,
        group: "demoNotes".to_owned(),
        topics: Vec::new(),
        heading: Some(plain("An Invented Note")),
        blocks: vec![Block::Para(plain(words))],
    }
}

/// A unit of the title whose top provision is `root`.
fn unit(root: Provision) -> UnitModule {
    UnitModule {
        address: root.address.clone(),
        aliases: Vec::new(),
        within: Some(address("/zz/demo/t1")),
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        root,
        credit: None,
        notes: Vec::new(),
        footnotes: Vec::new(),
        definitions: Vec::new(),
    }
}

/// The models of the first version of the dependency: the title
/// `/zz/demo/t1`, with the alias `/zz/demo/tI` and an enacted note; the
/// unit `s3`, with the alias `/zz/demo/t1/s4`; and the unit `s8`, with the
/// subsections `a` and `b` and an enacted note. The second version says
/// `s8_a` in `s8/a`.
fn models(s8_a: &str) -> Vec<Module> {
    let mut s3 = unit(provision(
        "/zz/demo/t1/s3",
        "section",
        "3",
        "A widget may be of any color.",
        Vec::new(),
    ));
    s3.aliases.push(address("/zz/demo/t1/s4"));
    let mut s8 = unit(provision(
        "/zz/demo/t1/s8",
        "section",
        "8",
        "A widget has invented parts.",
        vec![
            provision("/zz/demo/t1/s8/a", "subsection", "a", s8_a, Vec::new()),
            provision(
                "/zz/demo/t1/s8/b",
                "subsection",
                "b",
                "Another invented subsection.",
                Vec::new(),
            ),
        ],
    ));
    s8.notes.push(enacted(
        "/zz/demo/t1/s8",
        "effective",
        "An invented act took effect on an invented day.",
    ));
    let title = ContainerModule {
        address: address("/zz/demo/t1"),
        aliases: vec![address("/zz/demo/tI")],
        within: None,
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        kind: "title".to_owned(),
        num: num("1", "Title 1—"),
        heading: Some(plain("Invented Things")),
        status: None,
        members: vec![address("/zz/demo/t1/s3"), address("/zz/demo/t1/s8")],
        credit: None,
        notes: vec![enacted(
            "/zz/demo/t1",
            "amendment",
            "An invented act amended this title.",
        )],
        footnotes: Vec::new(),
    };
    vec![Module::Container(title), Module::Unit(s3), Module::Unit(s8)]
}

/// What `s8/a` says in the first version.
const FIRST: &str = "An invented subsection.";

/// Writes `models` into `out` as a generated workspace of the release
/// `release`, and gives the BLAKE3 of the bytes of its lock.
fn write_workspace(out: &Path, release: &str, models: &[Module]) -> Hash32 {
    let manifest = Manifest {
        name: "demo".to_owned(),
        kind: WorkspaceKind::Generated,
        prefix: address("/zz/demo"),
        release: Some(release.to_owned()),
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        dependencies: Vec::new(),
    };
    let writer = GeneratedWriter::create(out, manifest).expect("create the workspace");
    for model in models {
        writer.write(model).expect("write a module");
    }
    writer
        .finish(
            Generator {
                name: "demo-writer".to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            },
            Vec::new(),
            AddressCounts::default(),
            NotImported::default(),
        )
        .expect("finish the workspace");
    let lock = fs::read(out.join("law.lock.json")).expect("read the lock");
    Hash32(*blake3::hash(&lock).as_bytes())
}

/// Writes into `notes` a curated workspace of the prefix `/zz/notes` that
/// depends on the workspace `demo` at `../demo`, pinned to the release
/// `release` and the lock digest `digest`, and whose one module,
/// `/zz/notes/a`, declares `decls`.
fn write_notes(notes: &Path, release: &str, digest: Hash32, decls: &[String]) {
    fs::create_dir_all(notes.join("zz/notes")).expect("create a directory");
    let manifest = Manifest {
        name: "notes".to_owned(),
        kind: WorkspaceKind::Curated,
        prefix: address("/zz/notes"),
        release: None,
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        dependencies: vec![Dependency {
            name: "demo".to_owned(),
            path: "../demo".to_owned(),
            release: release.to_owned(),
            lock_digest: digest,
        }],
    };
    fs::write(notes.join("workspace.json"), manifest.to_json()).expect("write");
    let mut module = "edition 2\nmodule \"/zz/notes/a\" {\n".to_owned();
    for line in decls.iter().flat_map(|decl| decl.lines()) {
        module.push_str("  ");
        module.push_str(line);
        module.push('\n');
    }
    module.push_str("}\n");
    fs::write(notes.join("zz/notes/a.fr"), module).expect("write");
}

/// What `fidryn text DIR ADDRESS --hashes` prints, which must succeed and
/// print nothing to stderr.
fn hashes(dir: &Path, at: &str) -> String {
    let printed = text(dir, &[at, "--hashes"]);
    assert!(
        printed.status.success(),
        "{at}: exit {:?}: {}",
        printed.status.code(),
        stderr(&printed)
    );
    assert_eq!(stderr(&printed), "", "{at}");
    stdout(&printed)
}

/// A binding named `name` of `binds`, written against the release `release`
/// with `printed`, the two lines that `--hashes` printed.
fn binding(name: &str, binds: &str, release: &str, printed: &str) -> String {
    format!("binding {name} {{\n  binds {binds}\n  release \"{release}\"\n{printed}  pending\n}}")
        .replace("\nprovision_hash", "\n  provision_hash")
        .replace("\nnotes_hash", "\n  notes_hash")
}

/// `fidryn check DIR --no-cache`, with `args` after it.
fn check(dir: &Path, args: &[&OsStr]) -> Output {
    let mut all = vec![
        OsStr::new("check"),
        dir.as_os_str(),
        OsStr::new("--no-cache"),
    ];
    all.extend_from_slice(args);
    fidryn(&all)
}

#[test]
fn the_hashes_that_text_prints_make_a_binding_that_is_not_stale() {
    let scratch = Scratch::new("not-stale");
    let demo = scratch.join("demo");
    let written = models(FIRST);
    let digest = write_workspace(&demo, "v1", &written);

    // Exactly the two lines a binding records: the tree hash of the
    // provision, or of the top provision of the unit an alias names, and the
    // enacted layer of its unit, as the models give them.
    let Module::Unit(s3) = &written[1] else {
        panic!("the second model is the unit s3");
    };
    let Module::Unit(s8) = &written[2] else {
        panic!("the third model is the unit s8");
    };
    let Item::Child(s8_b) = &s8.root.items[2] else {
        panic!("s8 has the subsection b");
    };
    for (at, tree, unit) in [
        ("/zz/demo/t1/s8", &s8.root, s8),
        ("/zz/demo/t1/s8/b", s8_b, s8),
        ("/zz/demo/t1/s4", &s3.root, s3),
    ] {
        assert_eq!(
            hashes(&demo, at),
            format!(
                "provision_hash \"{}\"\nnotes_hash \"{}\"\n",
                hash_provision(tree),
                layer_hashes_of_unit(unit).enacted
            ),
            "{at}"
        );
    }

    // Bindings of a section, a subsection, a unit by its alias, and one by
    // a name, each with what `--hashes` printed, are not stale.
    let mut decls = vec!["citation widget -> \"/zz/demo/t1/s8/a\"".to_owned()];
    for (name, binds, at) in [
        ("section", "\"/zz/demo/t1/s8\"", "/zz/demo/t1/s8"),
        ("second", "\"/zz/demo/t1/s8/b\"", "/zz/demo/t1/s8/b"),
        ("aliased", "\"/zz/demo/t1/s4\"", "/zz/demo/t1/s4"),
        ("by_name", "widget", "/zz/demo/t1/s8/a"),
    ] {
        decls.push(binding(name, binds, "v1", &hashes(&demo, at)));
    }
    let notes = scratch.join("notes");
    write_notes(&notes, "v1", digest, &decls);
    let checked = check(&notes, &[]);
    assert_eq!(checked.status.code(), Some(0), "{}", stderr(&checked));
    assert_eq!(stderr(&checked), "");
    assert_eq!(
        stdout(&checked),
        "modules: 1; from cache: 0; errors: 0; warnings: 0\nok\n"
    );
}

#[test]
fn staleness_is_a_warning_and_says_what_a_reader_needs() {
    // The binding is written against `v1`; `demo/` holds `v2`, which
    // changes one word of the subsection, and the manifest pins `v2`.
    let scratch = Scratch::new("stale");
    let v1 = scratch.join("v1");
    write_workspace(&v1, "v1", &models(FIRST));
    let demo = scratch.join("demo");
    let digest = write_workspace(&demo, "v2", &models("An invented clause."));
    let printed = hashes(&v1, "/zz/demo/t1/s8/a");
    assert_ne!(hashes(&demo, "/zz/demo/t1/s8/a"), printed);
    let notes = scratch.join("notes");
    let decls = [binding(
        "widget_binding",
        "\"/zz/demo/t1/s8/a\"",
        "v1",
        &printed,
    )];
    write_notes(&notes, "v2", digest, &decls);
    let report = scratch.join("report.json");
    let checked = check(&notes, &[OsStr::new("--report"), report.as_os_str()]);

    // A warning: exit 0, `ok`, and counted in the summary line and in the
    // report.
    assert_eq!(checked.status.code(), Some(0), "{}", stderr(&checked));
    assert_eq!(
        stdout(&checked),
        "modules: 1; from cache: 0; errors: 0; warnings: 1\nok\n"
    );
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report).expect("the report is written"))
            .expect("the report is JSON");
    assert_eq!(
        (written["errors"].as_u64(), written["warnings"].as_u64()),
        (Some(0), Some(1))
    );

    // One line, in the file of the module, that names the binding, the
    // address, the reason, the release it was written against and the
    // release of the dependency, and says what to do.
    let said = stderr(&checked);
    assert_eq!(said.lines().count(), 1, "{said}");
    let file = notes.join("zz/notes/a.fr");
    assert_eq!(
        said,
        format!(
            "{}: W750 StaleBinding: the binding `widget_binding` of `/zz/demo/t1/s8/a` is stale \
             (provision text changed) in the release `v2` of the dependency `demo`, and was \
             written against the release `v1`; read the provision again, and write into the \
             binding the hashes that `fidryn text` prints for it with `--hashes`\n",
            file.display()
        )
    );
}

#[test]
fn hashes_are_printed_for_a_provision_and_refused_for_anything_else() {
    let scratch = Scratch::new("refused");
    let demo = scratch.join("demo");
    write_workspace(&demo, "v1", &models(FIRST));
    // A note, a container, an alias of a container, an address of nothing,
    // and a text that is no address: exit 1, nothing on stdout, and what the
    // address is on stderr.
    for (at, said) in [
        (
            "/zz/demo/t1/s8/note/demoNotes/nolaw/effective",
            "`/zz/demo/t1/s8/note/demoNotes/nolaw/effective` is the address of a note, and \
             `--hashes` prints the hashes of a provision, or of a unit named by an alias",
        ),
        (
            "/zz/demo/t1",
            "`/zz/demo/t1` is the address of a container, and `--hashes` prints the hashes of a \
             provision, or of a unit named by an alias",
        ),
        (
            "/zz/demo/tI",
            "`/zz/demo/tI` is an alias of the container `/zz/demo/t1`, and `--hashes` prints the \
             hashes of a provision, or of a unit named by an alias",
        ),
        (
            "/zz/demo/t1/s9",
            "the workspace holds no provision, container or note at `/zz/demo/t1/s9`",
        ),
        ("zz/demo/t1/s8", "`zz/demo/t1/s8` is no address"),
    ] {
        let refused = text(&demo, &[at, "--hashes"]);
        assert_eq!(refused.status.code(), Some(1), "{at}: {}", stderr(&refused));
        assert_eq!(stdout(&refused), "", "{at}");
        assert!(
            stderr(&refused).contains(said),
            "{at}: {}",
            stderr(&refused)
        );
    }
    // `--hashes` needs an address, and is not given with `--json`.
    for args in [
        &["--hashes"][..],
        &["/zz/demo/t1/s8", "--hashes", "--json"][..],
    ] {
        let refused = text(&demo, args);
        assert_eq!(
            refused.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&refused)
        );
        assert_eq!(stdout(&refused), "", "{args:?}");
    }
}
