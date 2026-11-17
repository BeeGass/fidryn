//! `fidryn ingest`: a generated workspace of law modules, written from the
//! archives of one release of a corpus.
//!
//! This is the one place of the command line that names the corpus it
//! imports, the United States Code, read from the official XML release point
//! published by the Office of the Law Revision Counsel by
//! `fidryn-ingest-uscode`. What it writes, `fidryn-driver` writes: the
//! command joins the two.

use clap::Subcommand;
use fidryn_driver::workspace::{GeneratedWriter, WriteError};
use fidryn_ingest_uscode::{IngestError, IngestOptions, generator, ingest, open};
use fidryn_provision::Address;
use fidryn_provision::workspace::{Manifest, WorkspaceKind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The corpora `fidryn ingest` imports.
#[derive(Subcommand, Debug)]
pub enum Corpus {
    /// Write a generated workspace from the archives of one release of the
    /// official XML release point published by the Office of the Law Revision
    /// Counsel.
    Uscode {
        /// The directory of the archives of the release and their retrieval
        /// record, `retrieval.json`.
        archive_dir: PathBuf,
        /// The directory of the workspace to write.
        #[arg(long, value_name = "DIR")]
        out: PathBuf,
        /// A title to write, by the publisher's designation (`1`, `5a`); may
        /// be given more than once. By default every title of the release.
        #[arg(long = "title", value_name = "T")]
        titles: Vec<String>,
        /// Worker threads (default: the number available).
        #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
        jobs: Option<u32>,
        /// Write nothing; print each file under DIR that differs from what the
        /// run would write, and exit 1 when there is one.
        #[arg(long)]
        verify: bool,
    },
}

/// The name of the workspace of the Code.
const NAME: &str = "uscode";

/// Runs `fidryn ingest`.
pub(crate) fn run(corpus: Corpus) -> ExitCode {
    match corpus {
        Corpus::Uscode {
            archive_dir,
            out,
            titles,
            jobs,
            verify,
        } => uscode(&archive_dir, &out, titles, jobs, verify),
    }
}

/// `fidryn ingest uscode ARCHIVE_DIR --out DIR`: opens the release in
/// `archive_dir`, checking each archive against its retrieval record, and
/// writes the generated workspace of `titles`, or of every title, into
/// `out` on `jobs` threads; then prints one line of what it wrote. With
/// `verify` it writes nothing, prints each path under `out` that differs
/// from what it would write with the word that says how, and exits 1 when
/// there is one; when there is none it prints `ok`.
fn uscode(
    archive_dir: &Path,
    out: &Path,
    titles: Vec<String>,
    jobs: Option<u32>,
    verify: bool,
) -> ExitCode {
    let archives = match open(archive_dir) {
        Ok(archives) => archives,
        Err(error) => return super::fail(error.to_string()),
    };
    // A title that no member holds is refused before anything is written.
    let artifacts = match archives.artifacts_of(&titles) {
        Ok(artifacts) => artifacts,
        Err(error) => return super::fail(error.to_string()),
    };
    let prefix = match Address::parse(fidryn_ingest_uscode::PREFIX) {
        Ok(prefix) => prefix,
        Err(error) => return super::fail(error.to_string()),
    };
    let manifest = Manifest {
        name: NAME.to_owned(),
        kind: WorkspaceKind::Generated,
        prefix,
        release: Some(archives.release().to_owned()),
        toolchain: env!("CARGO_PKG_VERSION").to_owned(),
        dependencies: Vec::new(),
    };
    let writer = if verify {
        GeneratedWriter::compare(out, manifest)
    } else {
        GeneratedWriter::create(out, manifest)
    };
    let writer = match writer {
        Ok(writer) => writer,
        Err(error) => return super::fail(error.to_string()),
    };
    let jobs = match jobs {
        Some(jobs) => usize::try_from(jobs).unwrap_or(usize::MAX),
        None => std::thread::available_parallelism().map_or(1, std::num::NonZero::get),
    };
    let options = IngestOptions { titles, jobs };
    let summary = match ingest(&archives, &options, &|module| {
        writer
            .write(&module)
            .map_err(|error| IngestError::new(error.to_string()))
    }) {
        Ok(summary) => summary,
        Err(error) => return super::fail(error.to_string()),
    };
    let read = artifacts.len();
    match writer.finish(
        generator(),
        artifacts,
        summary.addresses,
        summary.not_imported,
    ) {
        Ok(_) if verify => {
            println!("ok");
            ExitCode::SUCCESS
        }
        Ok(_) => {
            println!(
                "ingested: {} units, {} containers from {read} archives (release {})",
                summary.units,
                summary.containers,
                archives.release()
            );
            ExitCode::SUCCESS
        }
        Err(WriteError::Differs { differences, .. }) => {
            for (path, difference) in differences {
                println!("{} {path}", difference.word());
            }
            ExitCode::from(1)
        }
        Err(error) => super::fail(error.to_string()),
    }
}
