//! Helpers shared by the unit tests of the checks in this crate.

use std::fs;
use std::path::{Path, PathBuf};

pub const HEADER: &str = "    jurisdiction Test\n    source_snapshot \"scratch\"\n    effective_at 2026-01-01\n    recorded_at 2026-01-01T00:00:00Z\n    outside_scope { everything_else }\n";

pub fn module(name: &str, body: &str) -> String {
    format!("module {name} version \"0.1.0\" {{\n{HEADER}    {body}\n}}\n")
}

/// A fresh empty directory under the system temp directory.
pub fn scratch(prefix: &str, name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{prefix}{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

pub fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}
