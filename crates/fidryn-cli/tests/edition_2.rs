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

/// How a command ended: its exit status, then what it printed to stdout and
/// to stderr.
#[cfg(unix)]
type Ended = (Option<i32>, String, String);

/// Runs `fidryn` with `args`, `INPUT` among them standing for `input`, its
/// output written to files in `dir`; `None` when it has not ended within
/// half a minute, when it is killed.
#[cfg(unix)]
fn run_on(dir: &Path, input: &Path, args: &[&str]) -> Option<Ended> {
    let stdout_file = dir.join("stdout.txt");
    let stderr_file = dir.join("stderr.txt");
    let mut child = Command::new(env!("CARGO_BIN_EXE_fidryn"))
        .args(args.iter().map(|arg| {
            if *arg == "INPUT" {
                input.as_os_str()
            } else {
                OsStr::new(arg)
            }
        }))
        .stdin(std::process::Stdio::null())
        .stdout(fs::File::create(&stdout_file).expect("create a file"))
        .stderr(fs::File::create(&stderr_file).expect("create a file"))
        .spawn()
        .expect("run fidryn");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait for fidryn") {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let read = |path: &Path| String::from_utf8_lossy(&fs::read(path).expect("read")).into_owned();
    status.map(|status| (status.code(), read(&stdout_file), read(&stderr_file)))
}

/// What `fidryn` gives with `args` of `bytes` given through a named pipe in
/// `dir`, which a thread of the test writes once, as `cat FILE |` does.
#[cfg(unix)]
fn through_a_pipe(dir: &Path, bytes: &[u8], args: &[&str]) -> Option<Ended> {
    use std::io::Write;
    let pipe = dir.join("input.pipe");
    let _ = fs::remove_file(&pipe);
    let made = Command::new("mkfifo")
        .arg(&pipe)
        .status()
        .expect("run mkfifo");
    assert!(made.success(), "mkfifo {}", pipe.display());
    let writer = {
        let (pipe, bytes) = (pipe.clone(), bytes.to_vec());
        std::thread::spawn(move || {
            if let Ok(mut file) = fs::OpenOptions::new().write(true).open(&pipe) {
                let _ = file.write_all(&bytes);
            }
        })
    };
    let ended = run_on(dir, &pipe, args);
    // A writer still waiting for a reader is let go: opened for reading and
    // writing, a pipe waits for no one.
    let _ = fs::OpenOptions::new().read(true).write(true).open(&pipe);
    writer.join().expect("the writer ends");
    ended
}

#[cfg(unix)]
#[test]
fn an_input_given_through_a_pipe_is_read_as_the_same_bytes_from_a_file() {
    let dir = std::env::temp_dir().join(format!("fidryn-edition-2-pipe-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create the scratch directory");
    // An invented module of the first edition, and an invented trace.
    let module = "module Examples.Piped version \"0.1.0\" {\n    query q() -> Bool {\n        \
                  goal Evaluate { true }\n    }\n}\n";
    let trace = "{\"trace\":\"t1\",\"nodes\":[{\"id\":\"t1\",\"detail\":\"an invented step\",\
                 \"parents\":[],\"kind\":\"Evaluation\"}]}\n";
    for (args, bytes) in [
        (vec!["fmt", "INPUT"], module),
        (vec!["check", "INPUT"], module),
        (vec!["explain", "INPUT", "--format", "json"], trace),
    ] {
        let file = dir.join("input.txt");
        fs::write(&file, bytes).expect("write the input");
        let from_a_file = run_on(&dir, &file, &args).expect("a command on a file ends");
        assert_eq!(from_a_file.0, Some(0), "{args:?}: {from_a_file:?}");
        assert!(!from_a_file.1.is_empty(), "{args:?}");
        let from_a_pipe = through_a_pipe(&dir, bytes.as_bytes(), &args)
            .unwrap_or_else(|| panic!("{args:?}: the command on a pipe did not end"));
        assert_eq!(from_a_pipe, from_a_file, "{args:?}");
    }
    let _ = fs::remove_dir_all(&dir);
}
