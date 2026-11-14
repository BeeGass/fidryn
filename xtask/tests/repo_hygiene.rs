//! Files a newcomer looks for exist and say what they should.

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_owned()
}

#[test]
fn a_security_policy_exists_and_names_the_reporting_route() {
    let policy = fs::read_to_string(root().join("SECURITY.md")).expect("SECURITY.md");
    assert!(policy.contains("Report a vulnerability"), "{policy}");
    assert!(
        !policy.contains('@'),
        "do not publish an address the owner has not chosen"
    );
}

#[test]
fn the_lockfile_is_tracked_and_nothing_ignores_it() {
    let ignore = fs::read_to_string(root().join(".gitignore")).expect(".gitignore");
    assert!(!ignore.contains("Cargo.lock"), "{ignore}");
    assert!(root().join("Cargo.lock").is_file());
}
