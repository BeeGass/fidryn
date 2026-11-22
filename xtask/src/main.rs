//! Fidryn workspace task runner (`cargo xtask`).

mod bench;
mod ci;
mod docs_check;
mod examples;
mod governance;
mod licenses;
mod package;
mod release;
mod site;
mod test;
#[cfg(test)]
mod test_git;
#[cfg(test)]
mod test_support;
mod uscode_sample;
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
    /// Workspace CI: tests, clippy -D warnings, conformance probes, Python checks, the sample of the Code, site check, example status and documentation checks, JS tests, bench build
    Ci(ci::CiArgs),
    /// Check or regenerate the recorded check status of every .fr file
    Examples(examples::ExamplesArgs),
    /// Check documentation links, module samples and the commands documents show
    Docs(docs_check::DocsArgs),
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
    /// Check the sample of the Code: its archives against their retrieval record, its workspace against what the ingester writes from them on one worker and on eight, its check, its profile and its counts of references, and its text against the independent script (--update writes the workspace first)
    UscodeSample(uscode_sample::UscodeSampleArgs),
}

fn main() -> Result<()> {
    match Xtask::parse().command {
        Command::Test(args) => test::run(args),
        Command::Bench(args) => bench::run(args),
        Command::Ci(args) => ci::run(args),
        Command::Examples(args) => examples::run(args),
        Command::Docs(args) => docs_check::run(args),
        Command::Site(args) => site::run(args),
        Command::Governance(args) => governance::run(args),
        Command::Package(args) => package::run(args),
        Command::ReleaseCheck(args) => release::run_check(args),
        Command::ReleaseNotes(args) => release::run_notes(args),
        Command::UscodeSample(args) => uscode_sample::run(args),
    }
}
