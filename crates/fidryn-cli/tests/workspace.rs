//! `fidryn check DIR` checks a workspace of edition 2 modules. The workspace
//! here is the invented demo workspace under
//! `conformance/fixtures/workspace/demo`, in the invented corpus `/zz/demo`,
//! which the tests of `fidryn-driver` hold to what the writer writes. A
//! check of it in place is run with `--no-cache`, so that nothing is
//! written into the repository; a check that keeps a cache runs on a copy.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn demo_workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/workspace/demo")
}

/// `fidryn check` with `args`, and with `FIDRYN_CACHE_DIR` set to `cache`,
/// or not set when it is `None`.
fn check_in(args: &[&OsStr], cache: Option<&OsStr>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fidryn"));
    command.arg("check").args(args);
    match cache {
        Some(cache) => command.env("FIDRYN_CACHE_DIR", cache),
        None => command.env_remove("FIDRYN_CACHE_DIR"),
    };
    command.output().expect("run fidryn")
}

/// `fidryn check DIR --no-cache`.
fn check(dir: &Path) -> Output {
    check_in(&[dir.as_os_str(), OsStr::new("--no-cache")], None)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
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
            "fidryn-cli-workspace-{}-{n}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch directory");
        Scratch(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Copies every file and directory below `from` to `to`, but for those
/// whose names begin with a dot, which are not a workspace's own, such as a
/// cache that a check of the fixture in place left.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create a directory");
    for entry in fs::read_dir(from).unwrap_or_else(|error| panic!("{}: {error}", from.display())) {
        let entry = entry.expect("read an entry");
        if entry.file_name().as_encoded_bytes().first() == Some(&b'.') {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.file_type().expect("the kind of an entry").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}

#[test]
fn a_workspace_from_the_writer_checks_clean() {
    let checked = check(&demo_workspace());
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(
        stdout(&checked),
        "modules: 5; from cache: 0; errors: 0; warnings: 0\nok\n"
    );
    assert_eq!(stderr(&checked), "");
}

#[test]
fn a_toolchain_mismatch_is_a_warning() {
    let scratch = Scratch::new("toolchain");
    copy_tree(&demo_workspace(), scratch.path());
    let manifest = scratch.path().join("workspace.json");
    let text = fs::read_to_string(&manifest).expect("read the manifest");
    let recorded = format!("\"toolchain\": \"{}\"", env!("CARGO_PKG_VERSION"));
    assert!(text.contains(&recorded), "{text}");
    fs::write(
        &manifest,
        text.replace(&recorded, "\"toolchain\": \"0.0.0-invented\""),
    )
    .expect("write the manifest");

    let checked = check(scratch.path());
    assert_eq!(checked.status.code(), Some(0), "{}", stderr(&checked));
    assert_eq!(
        stdout(&checked),
        "modules: 5; from cache: 0; errors: 0; warnings: 1\nok\n"
    );
    let message = stderr(&checked);
    assert!(
        message.starts_with(&format!("{}: W760 ToolchainMismatch: ", manifest.display())),
        "{message}"
    );
    assert_eq!(message.lines().count(), 1, "{message}");
}

#[test]
fn a_directory_with_no_manifest_is_refused() {
    let scratch = Scratch::new("no-manifest");
    let checked = check(scratch.path());
    assert_eq!(checked.status.code(), Some(1), "{}", stderr(&checked));
    assert_eq!(
        stdout(&checked),
        "modules: 0; from cache: 0; errors: 1; warnings: 0\n"
    );
    let message = stderr(&checked);
    assert!(
        message.starts_with(&format!(
            "{}: E730 WorkspaceViolation: no `workspace.json` was found",
            scratch.path().join("workspace.json").display()
        )),
        "{message}"
    );
}

fn check_with(args: &[&OsStr]) -> Output {
    check_in(args, None)
}

#[test]
fn the_report_and_the_unresolved_list_are_written_on_request() {
    let scratch = Scratch::new("report");
    let report = scratch.path().join("report.json");
    let unresolved = scratch.path().join("unresolved.jsonl");
    let workspace = demo_workspace();
    let checked = check_with(&[
        workspace.as_os_str(),
        OsStr::new("--no-cache"),
        OsStr::new("--jobs"),
        OsStr::new("3"),
        OsStr::new("--report"),
        report.as_os_str(),
        OsStr::new("--unresolved"),
        unresolved.as_os_str(),
    ]);
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(
        stdout(&checked),
        "modules: 5; from cache: 0; errors: 0; warnings: 0\nok\n"
    );
    let text = fs::read_to_string(&report).expect("the report is written");
    assert!(
        text.starts_with("{\n  \"schema\": \"fidryn.check-report/v0.1\",\n  \"modules\": 5,\n"),
        "{text}"
    );
    assert!(text.ends_with("\n}\n"), "{text}");
    assert_eq!(
        fs::read_to_string(&unresolved).expect("the list is written"),
        ""
    );
}

#[test]
fn the_flags_of_a_workspace_check_are_refused_for_a_file() {
    let module = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/fidryn-check/tests/fixtures/e2/unit.fr");
    let flags: [&[&str]; 5] = [
        &["--jobs", "2"],
        &["--cache-dir", "2"],
        &["--no-cache"],
        &["--report", "2"],
        &["--unresolved", "2"],
    ];
    for flag in flags {
        let mut args = vec![module.as_os_str()];
        args.extend(flag.iter().map(OsStr::new));
        let checked = check_with(&args);
        assert_eq!(
            checked.status.code(),
            Some(1),
            "{flag:?}: {}",
            stderr(&checked)
        );
        assert!(
            stderr(&checked).contains("belong to the check of a workspace"),
            "{flag:?}: {}",
            stderr(&checked)
        );
        assert_eq!(stdout(&checked), "", "{flag:?}");
    }
    let checked = check_with(&[
        demo_workspace().as_os_str(),
        OsStr::new("--jobs"),
        OsStr::new("0"),
    ]);
    assert_eq!(checked.status.code(), Some(2), "{}", stderr(&checked));
    // A cache directory and no cache are not given together.
    let checked = check_with(&[
        demo_workspace().as_os_str(),
        OsStr::new("--no-cache"),
        OsStr::new("--cache-dir"),
        OsStr::new("2"),
    ]);
    assert_eq!(checked.status.code(), Some(2), "{}", stderr(&checked));
}

/// Every entry below `root`, a file with its bytes and a directory with
/// `None`, by its path relative to `root`.
fn entries_below(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        for entry in fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()))
        {
            let entry = entry.expect("read an entry");
            let name = entry.file_name().into_string().expect("a UTF-8 name");
            let rel = format!("{prefix}{name}");
            if entry.file_type().expect("the kind of an entry").is_dir() {
                out.insert(rel.clone(), None);
                walk(&entry.path(), &format!("{rel}/"), out);
            } else {
                out.insert(rel, Some(fs::read(entry.path()).expect("read a file")));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, "", &mut out);
    out
}

/// The output of a clean check of the demo workspace, `cached` of whose
/// modules were read from the cache.
fn clean(cached: usize) -> String {
    format!("modules: 5; from cache: {cached}; errors: 0; warnings: 0\nok\n")
}

#[test]
fn no_cache_writes_nothing() {
    let scratch = Scratch::new("no-cache");
    let workspace = scratch.path().join("workspace");
    copy_tree(&demo_workspace(), &workspace);
    let named = scratch.path().join("named-cache");
    let before = entries_below(scratch.path());
    for _ in 0..2 {
        let checked = check_in(
            &[workspace.as_os_str(), OsStr::new("--no-cache")],
            Some(named.as_os_str()),
        );
        assert!(checked.status.success(), "{}", stderr(&checked));
        assert_eq!(stdout(&checked), clean(0));
        assert_eq!(stderr(&checked), "");
        assert_eq!(entries_below(scratch.path()), before);
    }
}

#[test]
fn the_cache_directory_is_chosen_in_the_stated_order() {
    let scratch = Scratch::new("cache-order");
    let workspace = scratch.path().join("workspace");
    copy_tree(&demo_workspace(), &workspace);
    let flagged = scratch.path().join("flagged-cache");
    let named = scratch.path().join("named-cache");
    let default = workspace.join(".fidryn-cache");
    let run = |args: &[&OsStr], cache: Option<&OsStr>, cached: usize| {
        let mut args = args.to_vec();
        args.insert(0, workspace.as_os_str());
        let checked = check_in(&args, cache);
        assert!(checked.status.success(), "{args:?}: {}", stderr(&checked));
        assert_eq!(stdout(&checked), clean(cached), "{args:?}");
        assert_eq!(stderr(&checked), "", "{args:?}");
    };

    // `--cache-dir` before `FIDRYN_CACHE_DIR`.
    let flag = [OsStr::new("--cache-dir"), flagged.as_os_str()];
    run(&flag, Some(named.as_os_str()), 0);
    run(&flag, Some(named.as_os_str()), 5);
    assert!(flagged.join("v1").is_dir());
    assert!(!named.exists());
    assert!(!default.exists());

    // `FIDRYN_CACHE_DIR` before the default.
    run(&[], Some(named.as_os_str()), 0);
    run(&[], Some(named.as_os_str()), 5);
    assert!(named.join("v1").is_dir());
    assert!(!default.exists());

    // Then `.fidryn-cache` in the workspace, which a `FIDRYN_CACHE_DIR` that
    // is empty also leaves; the check writes nothing else in the workspace.
    let before = entries_below(&workspace);
    run(&[], None, 0);
    run(&[], Some(OsStr::new("")), 5);
    assert!(default.join("v1").is_dir());
    let mut after = entries_below(&workspace);
    after.retain(|rel, _| !rel.starts_with(".fidryn-cache"));
    assert_eq!(after, before);
}

#[test]
fn a_cache_that_cannot_be_written_is_said_once_and_the_check_goes_on() {
    let scratch = Scratch::new("cache-unwritable");
    let workspace = scratch.path().join("workspace");
    copy_tree(&demo_workspace(), &workspace);
    // A file stands where the cache's directory would be made.
    let blocked = scratch.path().join("blocked");
    fs::write(&blocked, "An invented file where a directory would be.\n").expect("write");
    let checked = check_in(
        &[
            workspace.as_os_str(),
            OsStr::new("--cache-dir"),
            blocked.as_os_str(),
        ],
        None,
    );
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(stdout(&checked), clean(0));
    let message = stderr(&checked);
    assert_eq!(message.lines().count(), 1, "{message}");
    // The message names the cache's directory, not the entry of whichever
    // write failed first.
    let expected = format!(
        "the cache could not be written, and the check went on without it: {}: ",
        blocked.display()
    );
    assert!(message.starts_with(&expected), "{message}");
}

#[test]
fn a_cache_inside_a_dependency_is_said_once_and_not_used() {
    // An invented curated workspace that depends on a copy of the demo
    // workspace, with its cache's directory inside that copy.
    let scratch = Scratch::new("cache-in-dependency");
    let demo = scratch.path().join("demo");
    copy_tree(&demo_workspace(), &demo);
    let lock = fs::read(demo.join("law.lock.json")).expect("read the lock");
    let notes = scratch.path().join("notes");
    fs::create_dir_all(notes.join("zz/notes")).expect("create a directory");
    let manifest = format!(
        "{{\n  \"schema\": \"fidryn.workspace/v0.1\",\n  \"name\": \"notes\",\n  \"kind\": \
         \"curated\",\n  \"prefix\": \"/zz/notes\",\n  \"toolchain\": \"{}\",\n  \
         \"dependencies\": [\n    {{\n      \"name\": \"demo\",\n      \"path\": \"../demo\",\n      \
         \"release\": \"42-7\",\n      \"lock_digest\": \"{}\"\n    }}\n  ]\n}}\n",
        env!("CARGO_PKG_VERSION"),
        blake3::hash(&lock).to_hex()
    );
    fs::write(notes.join("workspace.json"), manifest).expect("write");
    fs::write(
        notes.join("zz/notes/a.fr"),
        "edition 2\nmodule \"/zz/notes/a\" {\n  citation sizes -> \"/zz/demo/t1/s2\"\n}\n",
    )
    .expect("write");
    let cache = demo.join(".fidryn-cache");
    let before = entries_below(&demo);
    for _ in 0..2 {
        let checked = check_in(
            &[
                notes.as_os_str(),
                OsStr::new("--cache-dir"),
                cache.as_os_str(),
            ],
            None,
        );
        assert!(checked.status.success(), "{}", stderr(&checked));
        assert_eq!(
            stdout(&checked),
            "modules: 1; from cache: 0; errors: 0; warnings: 0\nok\n"
        );
        let message = stderr(&checked);
        assert_eq!(message.lines().count(), 1, "{message}");
        let expected = format!(
            "the cache could not be used, and the check went on without it: {}: the directory \
             lies inside the directory of the dependency `demo`, `../demo`",
            cache.display()
        );
        assert!(message.starts_with(&expected), "{message}");
        assert_eq!(entries_below(&demo), before);
    }
}
