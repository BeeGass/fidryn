//! Fidryn command-line toolchain.

use clap::Parser;
use fidryn_cli::{Cli, run};
use std::process::ExitCode;

fn main() -> ExitCode {
    run(Cli::parse())
}
