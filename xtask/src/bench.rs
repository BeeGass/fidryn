use crate::workspace::{
    cargo_command, load_metadata, package_has_benches, require_package, run_command,
    workspace_has_benches,
};
use anyhow::{Result, bail};
use clap::Args;

#[derive(Debug, Args)]
#[command(dont_delimit_trailing_values = true)]
pub struct BenchArgs {
    /// Compile benches without running them (`cargo bench --no-run`)
    #[arg(long)]
    no_run: bool,

    /// Package to bench (repeatable)
    #[arg(short = 'p', long = "package", value_name = "SPEC")]
    packages: Vec<String>,

    /// Extra arguments forwarded to `cargo bench`
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "CARGO_ARG"
    )]
    cargo_args: Vec<String>,
}

pub fn run(args: BenchArgs) -> Result<()> {
    let metadata = load_metadata()?;
    if args.packages.is_empty() {
        if !workspace_has_benches(&metadata) {
            bail!("no bench targets in the workspace");
        }
    } else {
        for package in &args.packages {
            require_package(&metadata, package)?;
            if !package_has_benches(&metadata, package) {
                bail!("package `{package}` has no bench targets");
            }
        }
    }

    let mut cmd = cargo_command();
    cmd.args(["bench", "--offline"]);
    if args.no_run {
        cmd.arg("--no-run");
    }
    for package in &args.packages {
        cmd.args(["-p", package]);
    }
    cmd.args(&args.cargo_args);
    run_command(cmd)
}
