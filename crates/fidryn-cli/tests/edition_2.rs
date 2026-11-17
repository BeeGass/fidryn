//! The command line chooses the front end by the edition marker. The module
//! of edition 2 here is the invented golden unit of `fidryn-check`, in the
//! invented corpus `/zz/demo`.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REFUSAL: &str = "an edition 2 module holds provisions and is not compiled for evaluation; \
                       `fidryn check` reads it";

fn workspace_file(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn fidryn<I, S>(args: I) -> Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    Command::new(env!("CARGO_BIN_EXE_fidryn"))
        .args(args)
        .output()
        .expect("run fidryn")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A directory of its own under the temporary directory, removed when
/// dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir = std::env::temp_dir().join(format!("fidryn-edition-2-{}", std::process::id()));
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

#[test]
fn check_accepts_an_edition_2_file_and_refuses_to_run_one() {
    let unit = workspace_file("crates/fidryn-check/tests/fixtures/e2/unit.fr");
    let unit = unit.as_os_str();
    let checked = fidryn([OsStr::new("check"), unit]);
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");

    let scratch = Scratch::new();
    let changed = scratch.path().join("changed.fr");
    let text = fs::read_to_string(unit).expect("read the golden unit");
    assert!(text.contains("a small device"));
    fs::write(
        &changed,
        text.replacen("a small device", "a small devise", 1),
    )
    .expect("write the changed copy");
    let checked = fidryn([OsStr::new("check"), changed.as_os_str()]);
    assert_eq!(checked.status.code(), Some(1), "{}", stderr(&checked));
    let message = stderr(&checked);
    assert!(
        message.contains(&changed.display().to_string()) && message.contains("E710"),
        "{message}"
    );
    assert!(checked.stdout.is_empty());

    let instant = "2026-01-01T00:00:00Z";
    let case = scratch.path().join("case.json");
    let template = scratch.path().join("template.txt");
    for args in [
        vec![
            OsStr::new("run"),
            unit,
            OsStr::new("--query"),
            OsStr::new("evaluate"),
            OsStr::new("--case"),
            case.as_os_str(),
            OsStr::new("--valid-at"),
            OsStr::new(instant),
            OsStr::new("--known-at"),
            OsStr::new(instant),
        ],
        vec![
            OsStr::new("verify"),
            unit,
            OsStr::new("--property"),
            OsStr::new("determinate"),
        ],
        vec![OsStr::new("fmt"), unit],
        vec![
            OsStr::new("render"),
            unit,
            OsStr::new("--template"),
            template.as_os_str(),
        ],
    ] {
        let command = args[0].to_string_lossy().into_owned();
        let refused = fidryn(args);
        assert_eq!(
            refused.status.code(),
            Some(1),
            "{command}: {}",
            stderr(&refused)
        );
        assert!(
            stderr(&refused).contains(REFUSAL),
            "{command}: {}",
            stderr(&refused)
        );
    }

    let first_edition = workspace_file("examples/california/abc-test/abc-test.fr");
    let checked = fidryn([OsStr::new("check"), first_edition.as_os_str()]);
    assert!(checked.status.success(), "{}", stderr(&checked));
    assert_eq!(String::from_utf8_lossy(&checked.stdout), "ok\n");
}
