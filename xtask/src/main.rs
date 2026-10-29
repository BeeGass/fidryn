//! Fidryn workspace task runner (`cargo xtask`).

mod bench;
mod ci;
mod governance;
mod package;
mod release;
mod site;
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
    /// Workspace CI: tests, clippy -D warnings, conformance probes, Python checks, site check, JS tests, bench build
    Ci(ci::CiArgs),
    /// Render the public site from docs/*.md (--check verifies the committed site)
    Site(site::SiteArgs),
    /// Render or reconcile repository protection rules
    Governance(governance::GovernanceArgs),
    /// Build a checked native release archive and checksum manifests
    Package(package::PackageArgs),
    /// Check the workspace version, changelog and optional main promotion base
    ReleaseCheck(release::ReleaseCheckArgs),
    /// Print the changelog entry for the current workspace version
    ReleaseNotes(release::ReleaseNotesArgs),
}

fn main() -> Result<()> {
    match Xtask::parse().command {
        Command::Test(args) => test::run(args),
        Command::Bench(args) => bench::run(args),
        Command::Ci(args) => ci::run(args),
        Command::Site(args) => site::run(args),
        Command::Governance(args) => governance::run(args),
        Command::Package(args) => package::run(args),
        Command::ReleaseCheck(args) => release::run_check(args),
        Command::ReleaseNotes(args) => release::run_notes(args),
    }
}
