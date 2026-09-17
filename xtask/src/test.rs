use crate::workspace::{
    INTEGRATION_FALLBACK_PACKAGE, INTEGRATION_FALLBACK_TEST, INTEGRATION_PACKAGE, cargo_command,
    has_package, load_metadata, require_package, run_command, run_schema_probes,
};
use anyhow::{Result, bail};
use cargo_metadata::Metadata;
use clap::Args;

#[derive(Debug, Args)]
#[command(dont_delimit_trailing_values = true)]
pub struct TestArgs {
    /// Package to test (repeatable). Extra cargo test args go last.
    #[arg(short = 'p', long = "package", value_name = "SPEC")]
    packages: Vec<String>,

    /// Restrict `-p` tests to `cargo test --lib`
    #[arg(long)]
    unit: bool,

    /// Run `fidryn-integration-tests`, or the fidryn-cli `integration_suite` fallback
    #[arg(long)]
    integration: bool,

    /// `cargo test --workspace --offline`, then schema probes if present
    #[arg(long)]
    all: bool,

    /// Extra arguments forwarded to `cargo test`
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "CARGO_ARG"
    )]
    cargo_args: Vec<String>,
}

pub fn run(args: TestArgs) -> Result<()> {
    if args.packages.is_empty() && !args.integration && !args.all {
        bail!("no test selection; pass -p <package>, --integration, and/or --all");
    }
    if args.unit && args.packages.is_empty() {
        bail!("--unit requires -p <package>");
    }

    let metadata = load_metadata()?;
    for package in &args.packages {
        require_package(&metadata, package)?;
    }

    for package in &args.packages {
        let mut cmd = cargo_command();
        cmd.args(["test", "-p", package]);
        if args.unit {
            cmd.arg("--lib");
        }
        cmd.args(&args.cargo_args);
        run_command(cmd)?;
    }

    if args.integration {
        run_integration(&metadata, &args.cargo_args)?;
    }

    if args.all {
        let mut cmd = cargo_command();
        cmd.args(["test", "--workspace", "--offline"]);
        cmd.args(&args.cargo_args);
        run_command(cmd)?;
        run_schema_probes()?;
    }

    Ok(())
}

fn run_integration(metadata: &Metadata, cargo_args: &[String]) -> Result<()> {
    if has_package(metadata, INTEGRATION_PACKAGE) {
        let mut cmd = cargo_command();
        cmd.args(["test", "-p", INTEGRATION_PACKAGE]);
        cmd.args(cargo_args);
        run_command(cmd)
    } else {
        eprintln!(
            "fidryn-integration-tests is not a workspace member yet; \
             falling back to cargo test -p {INTEGRATION_FALLBACK_PACKAGE} \
             --test {INTEGRATION_FALLBACK_TEST} --offline"
        );
        let mut cmd = cargo_command();
        cmd.args([
            "test",
            "-p",
            INTEGRATION_FALLBACK_PACKAGE,
            "--test",
            INTEGRATION_FALLBACK_TEST,
            "--offline",
        ]);
        cmd.args(cargo_args);
        run_command(cmd)
    }
}
