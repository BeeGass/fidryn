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
//! the BLAKE3 of its bytes; a **curated** workspace has neither.
//!
//! [`check_workspace`] checks a workspace in two phases, each on several
//! threads. Phase one reads, lowers and checks each module by itself and
//! builds its [`Interface`](fidryn_provision::Interface). Phase two checks
//! what holds across the files: the manifest and the lock, where each module
//! stands and the agreement of the files with the lock, and then, from the
//! interfaces alone ([`check_interfaces`]), the hierarchy of containers and
//! their members and the outcome of every reference. The report
//! ([`WorkspaceReport::to_json`]) and the list of the references that are
//! not `exact` ([`WorkspaceReport::unresolved_lines`]) are written on
//! request. This crate names no corpus; `xtask/tests/corpus_neutrality.rs`
//! checks it for this directory.

mod check;
mod discover;
mod index;
mod report;
mod writer;

#[cfg(test)]
mod tests;

pub use check::{CheckOptions, WorkspaceReport, check_workspace};
pub use index::{PhaseTwo, Unresolved, check_interfaces};
pub use report::CHECK_REPORT_SCHEMA;
pub use writer::{GeneratedWriter, WriteError};

/// The file name of the manifest, at the top of a workspace.
const MANIFEST_FILE: &str = "workspace.json";

/// The file name of the lock of a generated workspace, at its top.
const LOCK_FILE: &str = "law.lock.json";
