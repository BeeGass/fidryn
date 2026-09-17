use crate::workspace::{
    cargo_command, load_metadata, run_command, run_schema_probes, workspace_has_benches,
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
    test.args(["test", "--workspace", "--offline"]);
    run_command(test)?;

    let mut clippy = cargo_command();
    clippy.args(["clippy", "--workspace", "--offline", "--", "-D", "warnings"]);
    run_command(clippy)?;

    run_schema_probes()?;

    if workspace_has_benches(&metadata) {
        let mut bench = cargo_command();
        bench.args(["bench", "--no-run", "--offline"]);
        run_command(bench)?;
    } else {
        eprintln!("no bench targets in the workspace; skipping cargo bench --no-run");
    }

    Ok(())
}
