//! Workspaces of edition 2 modules.
//!
//! A workspace is a directory with a manifest, `workspace.json`, at its top
//! ([`Manifest`](fidryn_provision::workspace::Manifest)). Every `.fr` file in
//! it, in any directory whose name does not begin with a dot, is a module of
//! edition 2 that stands at the path of its address. A **generated**
//! workspace, which an importer writes with [`GeneratedWriter`], has a
//! release and a lock, `law.lock.json`, that lists every module file with
//! the BLAKE3 of its bytes; a **curated** workspace has neither.
//!
//! [`check_workspace`] reads, lowers and checks each module by itself, and
//! then checks what holds across the files: the manifest and the lock, where
//! each module stands, the agreement of the files with the lock, and the
//! hierarchy of containers and their members. This crate names no corpus;
//! `xtask/tests/corpus_neutrality.rs` checks it for this directory.

mod check;
mod discover;
mod writer;

#[cfg(test)]
mod tests;

pub use check::{CheckOptions, WorkspaceReport, check_workspace};
pub use writer::{GeneratedWriter, WriteError};

/// The file name of the manifest, at the top of a workspace.
const MANIFEST_FILE: &str = "workspace.json";

/// The file name of the lock of a generated workspace, at its top.
const LOCK_FILE: &str = "law.lock.json";
