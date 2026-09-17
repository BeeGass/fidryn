//! Fidryn workspace task runner (`cargo xtask`).

mod bench;
mod ci;
mod test;
mod workspace;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "xtask",
    version,
    about = "Fidryn workspace task runner",
    arg_required_else_help = true
)]
struct Xtask {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run `cargo test` for selected packages or the workspace
    Test(test::TestArgs),
    /// Run `cargo bench --offline`
    Bench(bench::BenchArgs),
    /// Workspace CI: tests, clippy -D warnings, schema probes
    Ci(ci::CiArgs),
}

fn main() -> Result<()> {
    match Xtask::parse().command {
        Command::Test(args) => test::run(args),
        Command::Bench(args) => bench::run(args),
        Command::Ci(args) => ci::run(args),
    }
}
