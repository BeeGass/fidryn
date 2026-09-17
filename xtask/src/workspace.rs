use anyhow::{Context, Result, bail};
use cargo_metadata::{Metadata, MetadataCommand};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

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
