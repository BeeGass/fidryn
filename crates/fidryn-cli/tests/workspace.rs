//! `fidryn check DIR` checks a workspace of edition 2 modules. The workspace
//! here is the invented demo workspace under
//! `conformance/fixtures/workspace/demo`, in the invented corpus `/zz/demo`,
//! which the tests of `fidryn-driver` hold to what the writer writes.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn demo_workspace() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/workspace/demo")
}

fn check(dir: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fidryn"))
        .args([OsStr::new("check"), dir.as_os_str()])
        .output()
        .expect("run fidryn")
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

/// Copies every file and directory below `from` to `to`.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create a directory");
    for entry in fs::read_dir(from).unwrap_or_else(|error| panic!("{}: {error}", from.display())) {
        let entry = entry.expect("read an entry");
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
