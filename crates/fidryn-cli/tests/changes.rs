//! `fidryn changes OLD_DIR NEW_DIR` prints what changed between two
//! generated workspaces of one prefix, and the bindings of curated
//! workspaces that are stale against the newer one. The workspaces here are
//! invented, in the corpora `/zz/demo`, `/zz/other` and `/zz/notes`, and
//! written by the tests with the writer.

use fidryn_driver::workspace::changes::change_set;
use fidryn_driver::workspace::{CheckOptions, GeneratedWriter};
use fidryn_provision::hash::{hash_provision, layer_hashes_of_unit};
use fidryn_provision::workspace::{
    AddressCounts, Dependency, Generator, Manifest, NotImported, WorkspaceKind,
};
use fidryn_provision::{
    Address, Block, ContainerModule, Hash32, Item, Module, Num, PartRole, Provision, Run, Text,
    UnitModule,
};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

/// Every environment variable that `fidryn` reads to find the directory of
/// its cache.
const CACHE_VARIABLES: [&str; 4] = ["FIDRYN_CACHE_DIR", "XDG_CACHE_HOME", "HOME", "LOCALAPPDATA"];

/// `fidryn ARGS...`, in an environment in which of [`CACHE_VARIABLES`] only
/// those of `set` are set, so that no cache of the user who runs the test is
/// written.
fn fidryn_with(args: &[&OsStr], set: &[(&str, &OsStr)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fidryn"));
    command.args(args);
    for name in CACHE_VARIABLES {
        command.env_remove(name);
    }
    for (name, value) in set {
        command.env(name, value);
    }
    command.output().expect("run fidryn")
}

/// `fidryn changes OLD NEW ARGS...`, with the variables `set`.
fn changes_with(old: &Path, new: &Path, args: &[&str], set: &[(&str, &OsStr)]) -> Output {
    let mut all = vec![OsStr::new("changes"), old.as_os_str(), new.as_os_str()];
    all.extend(args.iter().map(OsStr::new));
    fidryn_with(&all, set)
}

/// `fidryn changes OLD NEW ARGS...`.
fn changes(old: &Path, new: &Path, args: &[&str]) -> Output {
    changes_with(old, new, args, &[])
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
            "fidryn-cli-changes-{}-{n}-{tag}",
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
        num: Some(Num {
            value: value.to_owned(),
            shown: plain(&format!("({value})")),
        }),
        heading: None,
        status: None,
        items,
    }
}

/// The models of a title of the prefix `prefix` with two sections, `s1`,
/// and `s8` with the subsections `a`, which says `s8_a`, and `b`.
fn models(prefix: &str, s8_a: &str) -> Vec<Module> {
    let title = format!("{prefix}/t1");
    let unit = |root: Provision| UnitModule {
        address: root.address.clone(),
        aliases: Vec::new(),
        within: Some(address(&title)),
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        root,
        credit: None,
        notes: Vec::new(),
        footnotes: Vec::new(),
        definitions: Vec::new(),
    };
    let s1 = unit(provision(
        &format!("{title}/s1"),
        "section",
        "1",
        "A widget is a small device.",
        Vec::new(),
    ));
    let s8 = unit(provision(
        &format!("{title}/s8"),
        "section",
        "8",
        "A widget has invented parts.",
        vec![
            provision(
                &format!("{title}/s8/a"),
                "subsection",
                "a",
                s8_a,
                Vec::new(),
            ),
            provision(
                &format!("{title}/s8/b"),
                "subsection",
                "b",
                "Another invented subsection.",
                Vec::new(),
            ),
        ],
    ));
    let container = ContainerModule {
        address: address(&title),
        aliases: Vec::new(),
        within: None,
        source: "demo01.xml".to_owned(),
        positive_law: Some(true),
        kind: "title".to_owned(),
        num: Some(Num {
            value: "1".to_owned(),
            shown: plain("Title 1—"),
        }),
        heading: Some(plain("Invented Things")),
        status: None,
        members: vec![s1.address.clone(), s8.address.clone()],
        credit: None,
        notes: Vec::new(),
        footnotes: Vec::new(),
    };
    vec![
        Module::Container(container),
        Module::Unit(s1),
        Module::Unit(s8),
    ]
}

/// What `s8/a` says in the first version.
const FIRST: &str = "An invented subsection.";

/// Writes `models` into `out` as a generated workspace of the prefix
/// `prefix` and the release `release`.
fn write_workspace(out: &Path, prefix: &str, release: &str, models: &[Module]) {
    let manifest = Manifest {
        name: "demo".to_owned(),
        kind: WorkspaceKind::Generated,
        prefix: address(prefix),
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
}

/// Writes into `notes` a curated workspace of the prefix `/zz/notes`, with
/// no dependency, whose one module `/zz/notes/a` binds `/zz/demo/t1/s8/a`
/// and `/zz/demo/t1/s1` with the hashes of `models`.
fn write_notes(notes: &Path, models: &[Module]) {
    fs::create_dir_all(notes.join("zz/notes")).expect("create a directory");
    let manifest = Manifest {
        name: "notes".to_owned(),
        kind: WorkspaceKind::Curated,
        prefix: address("/zz/notes"),
        release: None,
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        dependencies: Vec::new(),
    };
    fs::write(notes.join("workspace.json"), manifest.to_json()).expect("write");
    let mut module = "edition 2\nmodule \"/zz/notes/a\" {\n".to_owned();
    for (name, unit, at) in [
        ("first", 2, "/zz/demo/t1/s8/a"),
        ("rule", 1, "/zz/demo/t1/s1"),
    ] {
        let Module::Unit(unit) = &models[unit] else {
            panic!("model {unit} is a unit");
        };
        let provision = if unit.address.as_str() == at {
            &unit.root
        } else {
            unit.root
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Child(child) if child.address.as_str() == at => Some(child),
                    _ => None,
                })
                .expect("the subsection")
        };
        module.push_str(&format!(
            "  binding {name} {{\n    binds \"{at}\"\n    release \"v1\"\n    provision_hash \
             \"{}\"\n    notes_hash \"{}\"\n    pending\n  }}\n",
            hash_provision(provision),
            layer_hashes_of_unit(unit).enacted
        ));
    }
    module.push_str("}\n");
    fs::write(notes.join("zz/notes/a.fr"), module).expect("write");
}

/// The two versions, `old/` of the release `v1` and `new/` of `v2`, in which
/// `s8/a` says another word, and `notes/`, whose bindings are written
/// against `v1`.
fn two_versions(tag: &str) -> Scratch {
    let scratch = Scratch::new(tag);
    let old = models("/zz/demo", FIRST);
    write_workspace(&scratch.join("old"), "/zz/demo", "v1", &old);
    write_workspace(
        &scratch.join("new"),
        "/zz/demo",
        "v2",
        &models("/zz/demo", "An invented clause."),
    );
    write_notes(&scratch.join("notes"), &old);
    scratch
}

/// The entries below `dir`, by path, entries whose name begins with a dot
/// included.
fn listing(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        {
            let entry = entry.expect("an entry");
            let rel = format!("{prefix}{}", entry.file_name().to_string_lossy());
            if entry.file_type().expect("the kind of an entry").is_dir() {
                walk(&entry.path(), &format!("{rel}/"), out);
            }
            out.push(rel);
        }
    }
    let mut out = Vec::new();
    walk(dir, "", &mut out);
    out.sort();
    out
}

#[test]
fn changes_prints_the_change_set_as_a_commit_message_or_as_json() {
    let scratch = two_versions("forms");
    let (old, new, notes) = (
        scratch.join("old"),
        scratch.join("new"),
        scratch.join("notes"),
    );
    let options = CheckOptions {
        jobs: 1,
        cache_dir: None,
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        executable: String::new(),
    };
    let expected = change_set(&old, &new, std::slice::from_ref(&notes), &options)
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    let bindings = notes.to_str().expect("a UTF-8 path");

    let text = changes(&old, &new, &["--no-cache", "--bindings", bindings]);
    assert_eq!(text.status.code(), Some(0), "{}", stderr(&text));
    assert_eq!(stderr(&text), "");
    assert_eq!(stdout(&text), expected.to_text());
    assert!(
        stdout(&text).starts_with("/zz/demo: v1 to v2\n\n"),
        "{}",
        stdout(&text)
    );
    assert!(
        stdout(&text).contains("\noperative.amended_text:\n/zz/demo/t1/s8/a\n"),
        "{}",
        stdout(&text)
    );
    assert!(
        stdout(&text).ends_with(
            "\nstale_bindings:\n/zz/demo/t1/s8/a: first of /zz/notes/a in notes (provision text \
             changed)\n"
        ),
        "{}",
        stdout(&text)
    );

    let json = changes(
        &old,
        &new,
        &["--no-cache", "--json", "--bindings", bindings],
    );
    assert_eq!(json.status.code(), Some(0), "{}", stderr(&json));
    assert_eq!(stderr(&json), "");
    assert_eq!(stdout(&json), expected.to_json());
    let value: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("JSON");
    assert_eq!(value["schema"], "fidryn.change-set/v0.1");

    // The same bytes for any number of workers.
    for jobs in ["1", "8"] {
        let again = changes(
            &old,
            &new,
            &[
                "--no-cache",
                "--json",
                "--jobs",
                jobs,
                "--bindings",
                bindings,
            ],
        );
        assert_eq!(stdout(&again), stdout(&json), "{jobs} workers");
    }

    // By default every workspace is read through one cache, `fidryn` in the
    // cache directory of the user, and nothing is written into any of them;
    // a second run reads from it and prints the same.
    let user = scratch.join("user");
    let set = [
        ("XDG_CACHE_HOME", user.as_os_str()),
        ("LOCALAPPDATA", user.as_os_str()),
    ];
    let before = [&old, &new, &notes].map(|dir| listing(dir));
    let first = changes_with(&old, &new, &["--bindings", bindings], &set);
    assert_eq!(first.status.code(), Some(0), "{}", stderr(&first));
    assert_eq!(stderr(&first), "");
    assert_eq!(stdout(&first), stdout(&text));
    assert_eq!([&old, &new, &notes].map(|dir| listing(dir)), before);
    assert!(
        listing(&user.join("fidryn"))
            .iter()
            .any(|rel| rel.starts_with("v1/")),
        "{:?}",
        listing(&user)
    );
    let second = changes_with(&old, &new, &["--bindings", bindings], &set);
    assert_eq!(stdout(&second), stdout(&text));
    assert_eq!(stderr(&second), "");
    assert_eq!([&old, &new, &notes].map(|dir| listing(dir)), before);

    // A cache named once is the cache of every workspace.
    let shared = scratch.join("cache");
    let named = changes(
        &old,
        &new,
        &["--cache-dir", shared.to_str().expect("a UTF-8 path")],
    );
    assert_eq!(named.status.code(), Some(0), "{}", stderr(&named));
    assert!(listing(&shared).iter().any(|rel| rel.starts_with("v1/")));
}

#[test]
fn a_cache_that_cannot_be_used_or_written_is_said_once() {
    let scratch = two_versions("cache-notes");
    let (old, new, notes) = (
        scratch.join("old"),
        scratch.join("new"),
        scratch.join("notes"),
    );
    let bindings = notes.to_str().expect("a UTF-8 path");
    let unchanged = stdout(&changes(
        &old,
        &new,
        &["--no-cache", "--bindings", bindings],
    ));

    // A file stands where the directory of the cache would be made: no
    // workspace can write it, and the command says so once.
    let blocked = scratch.join("blocked");
    fs::write(&blocked, "An invented file where a directory would be.\n").expect("write");
    let blocked_text = blocked.to_str().expect("a UTF-8 path");
    let written = changes(
        &old,
        &new,
        &["--cache-dir", blocked_text, "--bindings", bindings],
    );
    assert_eq!(written.status.code(), Some(0), "{}", stderr(&written));
    assert_eq!(stdout(&written), unchanged);
    let message = stderr(&written);
    assert_eq!(message.lines().count(), 1, "{message}");
    assert!(
        message.starts_with(&format!(
            "the cache could not be written, and the check went on without it: {}: ",
            blocked.display()
        )),
        "{message}"
    );

    // The workspace of bindings depends on the new side, and a cache inside
    // it is not used for that workspace; the two sides use it.
    let manifest = Manifest {
        name: "notes".to_owned(),
        kind: WorkspaceKind::Curated,
        prefix: address("/zz/notes"),
        release: None,
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        dependencies: vec![Dependency {
            name: "demo".to_owned(),
            path: "../new".to_owned(),
            release: "v2".to_owned(),
            lock_digest: Hash32([0; 32]),
        }],
    };
    fs::write(notes.join("workspace.json"), manifest.to_json()).expect("write");
    let inside = new.join(".cache");
    let inside_text = inside.to_str().expect("a UTF-8 path");
    let unused = changes(
        &old,
        &new,
        &["--cache-dir", inside_text, "--bindings", bindings],
    );
    assert_eq!(unused.status.code(), Some(0), "{}", stderr(&unused));
    assert_eq!(stdout(&unused), unchanged);
    let message = stderr(&unused);
    assert_eq!(message.lines().count(), 1, "{message}");
    assert!(
        message.starts_with(&format!(
            "the cache could not be used, and the check went on without it: {}: the directory \
             lies inside the directory of the dependency `demo`, `../new`",
            inside.display()
        )),
        "{message}"
    );
}

#[test]
fn changes_refuses_what_it_does_not_compare() {
    let scratch = two_versions("refused");
    let (old, new, notes) = (
        scratch.join("old"),
        scratch.join("new"),
        scratch.join("notes"),
    );
    let other = scratch.join("other");
    write_workspace(&other, "/zz/other", "v2", &models("/zz/other", FIRST));
    let empty = scratch.join("empty");
    fs::create_dir_all(&empty).expect("create a directory");
    // A side with an error: a module file changed after its lock.
    let broken = scratch.join("broken");
    write_workspace(&broken, "/zz/demo", "v2", &models("/zz/demo", FIRST));
    let s1 = broken.join("zz/demo/t1/s1.fr");
    let changed = fs::read_to_string(&s1)
        .expect("read")
        .replace("small device", "tiny device");
    fs::write(&s1, changed).expect("write");

    let old_text = old.to_str().expect("a UTF-8 path");
    // Each refusal is one sentence; a side with an error ends the command
    // with its diagnostics.
    for (what, output, phrase, one_line) in [
        (
            "a curated workspace",
            changes(&notes, &new, &["--no-cache"]),
            "is the manifest of a curated workspace",
            true,
        ),
        (
            "two prefixes",
            changes(&old, &other, &["--no-cache"]),
            "the prefix `/zz/other`",
            true,
        ),
        (
            "no workspace",
            changes(&old, &empty, &["--no-cache"]),
            "no `workspace.json` was found in the directory, so it is no workspace",
            true,
        ),
        (
            "bindings of a generated workspace",
            changes(&old, &new, &["--no-cache", "--bindings", old_text]),
            "is the manifest of a generated workspace",
            true,
        ),
        (
            "an error of a side",
            changes(&old, &broken, &["--no-cache"]),
            "s1.fr: E730 WorkspaceViolation: this file has changed since the lock was written",
            false,
        ),
    ] {
        assert_eq!(output.status.code(), Some(1), "{what}: {}", stderr(&output));
        assert_eq!(stdout(&output), "", "{what}");
        let errors = stderr(&output);
        let lines: Vec<&str> = errors.lines().collect();
        if one_line {
            assert_eq!(lines.len(), 1, "{what}: {lines:?}");
        }
        assert!(
            lines.iter().any(|line| line.contains(phrase)),
            "{what}: {errors}"
        );
    }
}
