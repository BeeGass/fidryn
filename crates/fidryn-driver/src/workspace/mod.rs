//! Workspaces of edition 2 modules.
//!
//! A workspace is a directory with a manifest, `workspace.json`, at its top
//! ([`Manifest`](fidryn_provision::workspace::Manifest)). Every `.fr` file in
//! it is a module of edition 2 that stands at the path of its address. An
//! entry whose name begins with a dot, whether a file, a directory or a
//! symbolic link, is not the workspace's: the check does not read it and the
//! writer leaves it alone. A **generated**
//! workspace, which an importer writes with [`GeneratedWriter`], has a
//! release and a lock, `law.lock.json`, that lists every module file with
//! the BLAKE3 of its bytes; a **curated** workspace has neither, and may
//! depend on generated workspaces, each pinned to a release and to the
//! digest of its lock.
//!
//! [`check_workspace`] checks a workspace in two phases, each on several
//! threads. Phase one reads, lowers and checks each module by itself and
//! builds its [`Interface`](fidryn_provision::Interface); its result for a
//! file is kept in a cache on disk under a key of the file's bytes, so that
//! a later check takes it from there ([`CheckOptions::cache_dir`]). Phase
//! two checks
//! what holds across the files: the manifest and the lock, where each module
//! stands and the agreement of the files with the lock, each dependency by
//! itself, and then, from the interfaces alone ([`check_interfaces`]), the
//! hierarchy of containers and their members, the names of curated modules,
//! and the outcome of every reference and every citation. The report
//! ([`WorkspaceReport::to_json`]) and the list of the references and the
//! citations that are not `exact` ([`WorkspaceReport::unresolved_lines`])
//! are written on request. [`read_modules`] reads the modules of a workspace
//! into the model, for what prints their text, and
//! [`changes::change_set`] compares two generated workspaces of one prefix.
//! A generated workspace also holds its coverage ledger, `coverage.json`,
//! which the writer writes after the lock and [`read_coverage`] counts again
//! from the files; no check reads it. This crate names no corpus;
//! `xtask/tests/corpus_neutrality.rs` checks it for this directory.

mod cache;
pub mod changes;
mod check;
mod deps;
mod discover;
mod index;
mod read;
mod report;
mod writer;

#[cfg(test)]
mod tests;

pub use check::{CheckOptions, WorkspaceReport, check_workspace};
pub use deps::lies_inside;
pub use index::{DependencyInterfaces, PhaseTwo, Unresolved, UnresolvedCitation, check_interfaces};
pub use read::{CoverageError, read_coverage, read_modules};
pub use report::CHECK_REPORT_SCHEMA;
pub use writer::{Difference, GeneratedWriter, WriteError};

/// The file name of the manifest, at the top of a workspace.
const MANIFEST_FILE: &str = "workspace.json";

/// The file name of the lock of a generated workspace, at its top.
const LOCK_FILE: &str = "law.lock.json";

/// The file name of the coverage ledger of a generated workspace, at its
/// top, which the writer writes after the lock and no check reads.
const COVERAGE_FILE: &str = "coverage.json";
