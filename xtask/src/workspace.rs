use anyhow::{Context, Result, bail};
use cargo_metadata::{Metadata, MetadataCommand};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

pub const INTEGRATION_PACKAGE: &str = "fidryn-integration-tests";
pub const INTEGRATION_FALLBACK_PACKAGE: &str = "fidryn-cli";
pub const INTEGRATION_FALLBACK_TEST: &str = "integration_suite";

pub const SCHEMA_PROBE_SCRIPTS: &[&str] = &[
    "conformance/check_json_contracts.py",
    "conformance/probe_outcome_schema.py",
    "conformance/schema_boundary_probes.py",
];

pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate must live one directory below the workspace root")
        .to_path_buf()
}

pub fn cargo_bin() -> PathBuf {
    option_env!("CARGO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"))
}

pub fn load_metadata() -> Result<Metadata> {
    let root = workspace_root();
    MetadataCommand::new()
        .cargo_path(cargo_bin())
        .current_dir(&root)
        .manifest_path(root.join("Cargo.toml"))
        .no_deps()
        .other_options(["--offline".to_owned()])
        .verbose(false)
        .exec()
        .context("cargo metadata --no-deps --offline failed")
}

pub fn has_package(metadata: &Metadata, name: &str) -> bool {
    metadata
        .workspace_packages()
        .iter()
        .any(|pkg| pkg.name == name)
}

pub fn require_package(metadata: &Metadata, name: &str) -> Result<()> {
    if has_package(metadata, name) {
        Ok(())
    } else {
        bail!("package `{name}` is not a workspace member")
    }
}

pub fn package_has_benches(metadata: &Metadata, name: &str) -> bool {
    metadata
        .workspace_packages()
        .iter()
        .filter(|pkg| pkg.name == name)
        .flat_map(|pkg| pkg.targets.iter())
        .any(cargo_metadata::Target::is_bench)
}

pub fn workspace_has_benches(metadata: &Metadata) -> bool {
    metadata
        .workspace_packages()
        .iter()
        .flat_map(|pkg| pkg.targets.iter())
        .any(cargo_metadata::Target::is_bench)
}

pub fn cargo_command() -> Command {
    let mut cmd = Command::new(cargo_bin());
    cmd.current_dir(workspace_root());
    cmd
}

fn print_command(cmd: &Command) {
    let mut parts = Vec::new();
    parts.push(quote_os(cmd.get_program()));
    for arg in cmd.get_args() {
        parts.push(quote_os(arg));
    }
    eprintln!("+ {}", parts.join(" "));
}

pub fn run_command(mut cmd: Command) -> Result<()> {
    print_command(&cmd);
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn {}", display_program(&cmd)))?;
    check_status(&status)
}

pub fn run_schema_probes() -> Result<()> {
    let root = workspace_root();
    let mut ran_any = false;
    for rel in SCHEMA_PROBE_SCRIPTS {
        let script = root.join(rel);
        if !script.is_file() {
            continue;
        }
        ran_any = true;
        let mut cmd = Command::new("python3");
        cmd.current_dir(&root);
        cmd.arg(&script);
        cmd.arg(&root);
        run_command(cmd)?;
    }
    if !ran_any {
        eprintln!("no schema probe scripts present; skipping");
    }
    Ok(())
}

/// `web/tests/*.test.js` under `root`, sorted, relative to `root`. Empty when
/// the directory does not exist.
fn js_test_files(root: &Path) -> Result<Vec<PathBuf>> {
    let dir = root.join("web").join("tests");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).with_context(|| format!("read {}", dir.display()))? {
        let entry = entry.with_context(|| format!("read {}", dir.display()))?;
        let name = entry.file_name();
        let is_test = name.to_str().is_some_and(|n| n.ends_with(".test.js"));
        if is_test && entry.path().is_file() {
            files.push(Path::new("web").join("tests").join(name));
        }
    }
    files.sort();
    Ok(files)
}

/// Run the JavaScript unit tests in `web/tests/` with `node --test`.
///
/// Node is optional for this workspace: nothing is built or served with it.
/// When `node --version` cannot be spawned the step is skipped.
pub fn run_js_tests() -> Result<()> {
    let probe = Command::new("node")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    if probe.is_err() {
        eprintln!("node not found; skipping JS tests in web/tests");
        return Ok(());
    }
    let root = workspace_root();
    let files = js_test_files(&root)?;
    if files.is_empty() {
        eprintln!("no JS tests in web/tests; skipping");
        return Ok(());
    }
    let mut cmd = Command::new("node");
    cmd.current_dir(&root);
    cmd.arg("--test");
    cmd.args(&files);
    run_command(cmd)
}

fn check_status(status: &ExitStatus) -> Result<()> {
    if status.success() {
        return Ok(());
    }
    match status.code() {
        Some(code) => std::process::exit(code),
        None => bail!("command terminated by signal"),
    }
}

fn display_program(cmd: &Command) -> String {
    quote_os(cmd.get_program())
}

fn quote_os(arg: &OsStr) -> String {
    let s = arg.to_string_lossy();
    if s.is_empty()
        || s.bytes()
            .any(|b| b.is_ascii_whitespace() || matches!(b, b'"' | b'\'' | b'\\' | b'$' | b'`'))
    {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        s.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::js_test_files;
    use std::fs;
    use std::path::PathBuf;

    /// A fresh directory under the system temp dir for one test.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-{name}-{}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("clear scratch dir");
        }
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn js_test_files_lists_only_test_scripts_sorted() {
        let root = scratch("js-tests");
        let tests = root.join("web").join("tests");
        fs::create_dir_all(tests.join("fixtures.test.js"))
            .expect("create a directory named like a test");
        for name in ["site.test.js", "mill.test.js", "helpers.js", "notes.md"] {
            fs::write(tests.join(name), "").expect("write file");
        }
        let files = js_test_files(&root).expect("list tests");
        fs::remove_dir_all(&root).expect("remove scratch dir");
        assert_eq!(
            files,
            vec![
                PathBuf::from("web/tests/mill.test.js"),
                PathBuf::from("web/tests/site.test.js"),
            ]
        );
    }

    #[test]
    fn js_test_files_is_empty_without_web_tests() {
        let root = scratch("no-js-tests");
        let files = js_test_files(&root).expect("list tests");
        fs::remove_dir_all(&root).expect("remove scratch dir");
        assert!(files.is_empty(), "{files:?}");
    }
}
