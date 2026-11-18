//! `fidryn check DIR` checks a workspace of edition 2 modules. The workspace
//! here is the invented demo workspace under
//! `conformance/fixtures/workspace/demo`, in the invented corpus `/zz/demo`,
//! which the tests of `fidryn-driver` hold to what the writer writes. A
//! check of it in place is run with `--no-cache`, and a check that keeps a
//! cache runs on a copy, with every variable that could name the cache's
//! directory either removed or set to a directory of the test, so that no
//! test writes into the repository or into the cache of the user who runs
//! it.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn demo_workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/workspace/demo")
}

/// Every environment variable that `fidryn` reads to find the directory of
/// its cache.
const CACHE_VARIABLES: [&str; 4] = ["FIDRYN_CACHE_DIR", "XDG_CACHE_HOME", "HOME", "LOCALAPPDATA"];

/// `fidryn check` with `args`, run in the directory `cwd`, in an environment
/// in which of [`CACHE_VARIABLES`] only those of `set` are set.
fn check_env_in(cwd: &Path, args: &[&OsStr], set: &[(&str, &OsStr)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fidryn"));
    command.current_dir(cwd).arg("check").args(args);
    for name in CACHE_VARIABLES {
        command.env_remove(name);
    }
    for (name, value) in set {
        command.env(name, value);
    }
    command.output().expect("run fidryn")
}

/// `fidryn check` with `args`, in an environment in which of
/// [`CACHE_VARIABLES`] only those of `set` are set.
fn check_env(args: &[&OsStr], set: &[(&str, &OsStr)]) -> Output {
    check_env_in(&std::env::temp_dir(), args, set)
}

/// `fidryn check` with `args`, and with `FIDRYN_CACHE_DIR` set to `cache`,
/// or with no variable that names a cache when it is `None`.
fn check_in(args: &[&OsStr], cache: Option<&OsStr>) -> Output {
    match cache {
        Some(cache) => check_env(args, &[("FIDRYN_CACHE_DIR", cache)]),
        None => check_env(args, &[]),
    }
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

/// Checks the copy `workspace` of the demo workspace twice, in the directory
/// `cwd`, with `args` and an environment in which of [`CACHE_VARIABLES`]
/// only those of `set` are set, and holds the first check to read nothing
/// from the cache and the second to read every module from it, both clean
/// and silent.
fn twice(cwd: &Path, workspace: &Path, args: &[&OsStr], set: &[(&str, &OsStr)]) {
    let mut all = vec![workspace.as_os_str()];
    all.extend_from_slice(args);
    for cached in [0, 5] {
        let checked = check_env_in(cwd, &all, set);
        assert!(checked.status.success(), "{set:?}: {}", stderr(&checked));
        assert_eq!(stdout(&checked), clean(cached), "{args:?} {set:?}");
        assert_eq!(stderr(&checked), "", "{args:?} {set:?}");
    }
}

/// The line that says the check keeps no cache because no directory is
/// named for one, as `fidryn check` prints it on this kind of system.
fn no_cache_named() -> String {
    let variables = if cfg!(windows) {
        "`LOCALAPPDATA` is not an absolute path"
    } else {
        "neither `XDG_CACHE_HOME` nor `HOME` is an absolute path"
    };
    format!(
        "the cache could not be used, and the check went on without it: no directory is named \
         for it: `--cache-dir` is not given, `FIDRYN_CACHE_DIR` is not set, and {variables}\n"
    )
}

#[cfg(not(windows))]
#[test]
fn the_cache_directory_is_chosen_in_the_stated_order() {
    let scratch = Scratch::new("cache-order");
    let workspace = scratch.path().join("workspace");
    copy_tree(&demo_workspace(), &workspace);
    let before = entries_below(&workspace);
    let [flagged, named, xdg, home] =
        ["flagged", "named", "xdg", "home"].map(|name| scratch.path().join(name));
    let every = [
        ("FIDRYN_CACHE_DIR", named.as_os_str()),
        ("XDG_CACHE_HOME", xdg.as_os_str()),
        ("HOME", home.as_os_str()),
    ];
    let cwd = scratch.path();

    // `--cache-dir` before every variable.
    twice(
        cwd,
        &workspace,
        &[OsStr::new("--cache-dir"), flagged.as_os_str()],
        &every,
    );
    assert!(flagged.join("v1").is_dir());
    assert!(!named.exists() && !xdg.exists() && !home.exists());

    // `FIDRYN_CACHE_DIR` before the cache directory of the user.
    twice(cwd, &workspace, &[], &every);
    assert!(named.join("v1").is_dir());
    assert!(!xdg.exists() && !home.exists());

    // An empty `FIDRYN_CACHE_DIR` is not set: then `fidryn` in
    // `XDG_CACHE_HOME`, before `HOME`.
    twice(
        cwd,
        &workspace,
        &[],
        &[
            ("FIDRYN_CACHE_DIR", OsStr::new("")),
            ("XDG_CACHE_HOME", xdg.as_os_str()),
            ("HOME", home.as_os_str()),
        ],
    );
    assert!(xdg.join("fidryn/v1").is_dir());
    assert!(!home.exists());

    // An `XDG_CACHE_HOME` that is empty or relative is not taken, and nothing
    // is made relative to the directory the command runs in: then
    // `.cache/fidryn` in `HOME`.
    for (value, home) in [
        ("", &home),
        ("relative-cache", &scratch.path().join("home-2")),
    ] {
        twice(
            cwd,
            &workspace,
            &[],
            &[
                ("XDG_CACHE_HOME", OsStr::new(value)),
                ("HOME", home.as_os_str()),
            ],
        );
        assert!(home.join(".cache/fidryn/v1").is_dir());
    }
    assert!(!scratch.path().join("relative-cache").exists());

    // The check wrote nothing into the workspace.
    assert_eq!(entries_below(&workspace), before);
}

#[cfg(windows)]
#[test]
fn the_cache_directory_is_chosen_in_the_stated_order() {
    let scratch = Scratch::new("cache-order");
    let workspace = scratch.path().join("workspace");
    copy_tree(&demo_workspace(), &workspace);
    let before = entries_below(&workspace);
    let [flagged, named, local] =
        ["flagged", "named", "local"].map(|name| scratch.path().join(name));
    let every = [
        ("FIDRYN_CACHE_DIR", named.as_os_str()),
        ("LOCALAPPDATA", local.as_os_str()),
    ];
    let cwd = scratch.path();
    twice(
        cwd,
        &workspace,
        &[OsStr::new("--cache-dir"), flagged.as_os_str()],
        &every,
    );
    assert!(flagged.join("v1").is_dir());
    assert!(!named.exists() && !local.exists());
    twice(cwd, &workspace, &[], &every);
    assert!(named.join("v1").is_dir());
    assert!(!local.exists());
    twice(
        cwd,
        &workspace,
        &[],
        &[
            ("FIDRYN_CACHE_DIR", OsStr::new("")),
            ("LOCALAPPDATA", local.as_os_str()),
        ],
    );
    assert!(local.join("fidryn").join("v1").is_dir());
    assert_eq!(entries_below(&workspace), before);
}

#[test]
fn with_no_directory_named_for_the_cache_the_check_keeps_none_and_says_so_once() {
    let scratch = Scratch::new("cache-none");
    let workspace = scratch.path().join("workspace");
    copy_tree(&demo_workspace(), &workspace);
    let before = entries_below(scratch.path());
    let uncached = check(&workspace);
    for set in [
        vec![],
        vec![
            ("FIDRYN_CACHE_DIR", OsStr::new("")),
            ("XDG_CACHE_HOME", OsStr::new("")),
            ("HOME", OsStr::new("")),
            ("LOCALAPPDATA", OsStr::new("")),
        ],
        vec![
            ("XDG_CACHE_HOME", OsStr::new("relative-cache")),
            ("HOME", OsStr::new("relative-home")),
            ("LOCALAPPDATA", OsStr::new("relative-local")),
        ],
    ] {
        for _ in 0..2 {
            let checked = check_env_in(scratch.path(), &[workspace.as_os_str()], &set);
            assert!(checked.status.success(), "{set:?}: {}", stderr(&checked));
            assert_eq!(stdout(&checked), stdout(&uncached), "{set:?}");
            assert_eq!(stdout(&checked), clean(0), "{set:?}");
            assert_eq!(stderr(&checked), no_cache_named(), "{set:?}");
            assert_eq!(entries_below(scratch.path()), before, "{set:?}");
        }
    }
}

/// The path of every entry of the cache below `dir`, with its JSON.
fn cache_entries(dir: &Path) -> Vec<(PathBuf, serde_json::Value)> {
    entries_below(dir)
        .into_iter()
        .filter(|(rel, bytes)| rel.ends_with(".json") && bytes.is_some())
        .map(|(rel, bytes)| {
            let text = String::from_utf8(bytes.expect("a file")).expect("UTF-8");
            let value = serde_json::from_str(&text).expect("an entry is JSON");
            (dir.join(rel), value)
        })
        .collect()
}

#[test]
fn a_cache_prepared_inside_the_workspace_changes_nothing_of_a_check_with_the_default_cache() {
    // A workspace that carries a cache, as a clone of a repository that
    // committed one would. A text of it was changed after its hashes were
    // written, and its lock records the new bytes, so that only the hashes
    // are wrong.
    let scratch = Scratch::new("prepared-cache");
    let workspace = scratch.path().join("workspace");
    copy_tree(&demo_workspace(), &workspace);
    let s1 = workspace.join("zz/demo/t1/s1.fr");
    let text = fs::read_to_string(&s1).expect("read a module");
    assert!(text.contains("small device"), "{text}");
    let changed = text.replace("small device", "large device");
    fs::write(&s1, &changed).expect("change a module");
    let lock = workspace.join("law.lock.json");
    let digest = |text: &str| blake3::hash(text.as_bytes()).to_hex().to_string();
    let recorded = fs::read_to_string(&lock).expect("read the lock");
    assert!(recorded.contains(&digest(&text)), "{recorded}");
    fs::write(&lock, recorded.replace(&digest(&text), &digest(&changed))).expect("write");
    let uncached = check(&workspace);
    assert_eq!(uncached.status.code(), Some(1), "{}", stderr(&uncached));
    assert!(stderr(&uncached).contains("E710"), "{}", stderr(&uncached));

    // The entries a check writes into `.fidryn-cache` when it is told to,
    // and then the entry of the changed module altered to hold no error.
    let prepared = workspace.join(".fidryn-cache");
    let named = [
        workspace.as_os_str(),
        OsStr::new("--cache-dir"),
        prepared.as_os_str(),
    ];
    assert_eq!(check_env(&named, &[]).status.code(), Some(1));
    let mut altered = 0;
    for (path, mut entry) in cache_entries(&prepared) {
        if entry["diagnostics"]
            .as_array()
            .is_some_and(|found| !found.is_empty())
        {
            entry["diagnostics"] = serde_json::Value::Array(Vec::new());
            fs::write(&path, format!("{entry}\n")).expect("alter an entry");
            altered += 1;
        }
    }
    assert_eq!(altered, 1);
    // Named, the cache is trusted, as the reference says a cache named
    // inside a workspace is.
    let trusted = check_env(&named, &[]);
    assert_eq!(trusted.status.code(), Some(0), "{}", stderr(&trusted));
    assert_eq!(stdout(&trusted), clean(5));

    // The default cache is the user's, and the check is that of a check
    // without a cache: the same diagnostics, the same errors.
    let user = scratch.path().join("user");
    let set = [
        ("XDG_CACHE_HOME", user.as_os_str()),
        ("LOCALAPPDATA", user.as_os_str()),
    ];
    let before = entries_below(&workspace);
    for cached in [0, 5] {
        let checked = check_env(&[workspace.as_os_str()], &set);
        assert_eq!(checked.status.code(), Some(1), "{}", stderr(&checked));
        assert_eq!(stderr(&checked), stderr(&uncached));
        assert_eq!(
            stdout(&checked),
            stdout(&uncached).replace("from cache: 0", &format!("from cache: {cached}"))
        );
    }
    assert!(user.join("fidryn").join("v1").is_dir());
    assert_eq!(entries_below(&workspace), before);
}

#[cfg(unix)]
#[test]
fn a_link_named_like_a_cache_in_the_workspace_is_not_followed() {
    // A link of the name of the cache a check once kept inside a workspace,
    // which leads out of it.
    let scratch = Scratch::new("cache-link");
    let workspace = scratch.path().join("workspace");
    copy_tree(&demo_workspace(), &workspace);
    let outside = scratch.path().join("outside");
    fs::create_dir_all(&outside).expect("create a directory");
    std::os::unix::fs::symlink("../outside", workspace.join(".fidryn-cache")).expect("link");
    let user = scratch.path().join("user");
    twice(
        scratch.path(),
        &workspace,
        &[],
        &[("XDG_CACHE_HOME", user.as_os_str())],
    );
    assert_eq!(
        fs::read_dir(&outside).expect("list a directory").count(),
        0,
        "nothing is written through the link"
    );
    assert!(user.join("fidryn/v1").is_dir());
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

/// Writes into `notes` an invented curated workspace of the prefix
/// `/zz/notes` that depends on a copy of the demo workspace at `path`,
/// relative to `notes`, pinned to its lock, `demo`, and that cites one of its
/// sections.
fn depending_on_the_demo(notes: &Path, path: &str, demo: &Path) {
    let lock = fs::read(demo.join("law.lock.json")).expect("read the lock");
    fs::create_dir_all(notes.join("zz/notes")).expect("create a directory");
    let manifest = format!(
        "{{\n  \"schema\": \"fidryn.workspace/v0.1\",\n  \"name\": \"notes\",\n  \"kind\": \
         \"curated\",\n  \"prefix\": \"/zz/notes\",\n  \"toolchain\": \"{}\",\n  \
         \"dependencies\": [\n    {{\n      \"name\": \"demo\",\n      \"path\": \"{path}\",\n      \
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
}

#[test]
fn a_cache_inside_a_dependency_is_said_once_and_not_used() {
    // An invented curated workspace that depends on a copy of the demo
    // workspace, with its cache's directory inside that copy.
    let scratch = Scratch::new("cache-in-dependency");
    let demo = scratch.path().join("demo");
    copy_tree(&demo_workspace(), &demo);
    let notes = scratch.path().join("notes");
    depending_on_the_demo(&notes, "../demo", &demo);
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

#[test]
fn a_cache_that_holds_a_dependency_is_said_once_and_not_used() {
    // The copy of the demo workspace stands where the cache would write its
    // entries, `v1` in the cache's directory, beside entries of its own
    // whose names begin with a dot.
    let scratch = Scratch::new("cache-holds-dependency");
    let cache = scratch.path().join("cache");
    let demo = cache.join("v1");
    copy_tree(&demo_workspace(), &demo);
    fs::create_dir_all(demo.join(".git")).expect("create a directory");
    fs::write(demo.join(".git/HEAD"), "ref: refs/heads/invented\n").expect("write");
    fs::write(demo.join(".notes"), "Invented notes.\n").expect("write");
    let notes = scratch.path().join("notes");
    depending_on_the_demo(&notes, "../cache/v1", &demo);
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
        assert_eq!(
            stderr(&checked),
            format!(
                "the cache could not be used, and the check went on without it: {}: the \
                 directory of the dependency `demo`, `../cache/v1`, lies inside the directory, \
                 and nothing is written inside a dependency\n",
                cache.display()
            )
        );
        assert_eq!(entries_below(&demo), before);
    }
}
