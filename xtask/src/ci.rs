use crate::workspace::{
    cargo_command, load_metadata, run_command, run_js_tests, run_python_checks, run_schema_probes,
    workspace_has_benches,
};
use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct CiArgs {
    /// Run `cargo fmt --all -- --check` before tests
    #[arg(long)]
    fmt: bool,
}

pub fn run(args: CiArgs) -> Result<()> {
    let metadata = load_metadata()?;

    if args.fmt {
        let mut fmt = cargo_command();
        fmt.args(["fmt", "--all", "--", "--check"]);
        run_command(fmt)?;
    }

    let mut test = cargo_command();
    test.args([
        "test",
        "--workspace",
        "--all-features",
        "--locked",
        "--offline",
    ]);
    run_command(test)?;

    let mut clippy = cargo_command();
    clippy.args([
        "clippy",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--locked",
        "--offline",
        "--",
        "-D",
        "warnings",
    ]);
    run_command(clippy)?;

    let mut docs = cargo_command();
    docs.env("RUSTDOCFLAGS", "-D warnings");
    docs.args([
        "doc",
        "--workspace",
        "--all-features",
        "--no-deps",
        "--locked",
        "--offline",
    ]);
    run_command(docs)?;

    run_schema_probes()?;
    run_python_checks()?;
    let mut probes = std::process::Command::new("uv");
    probes.current_dir(crate::workspace::workspace_root());
    probes.args([
        "run",
        "--locked",
        "python",
        "-m",
        "unittest",
        "discover",
        "-s",
        "conformance",
        "-p",
        "test_*.py",
    ]);
    run_command(probes)?;
    crate::site::check(&crate::workspace::workspace_root())?;

    run_js_tests()?;
    if workspace_has_benches(&metadata) {
        let mut bench = cargo_command();
        bench.args(["bench", "--no-run", "--locked", "--offline"]);
        run_command(bench)?;
    } else {
        eprintln!("no bench targets in the workspace; skipping cargo bench --no-run");
    }

    Ok(())
}
