//! The recorded example status matches a fresh survey of the tree.

use std::process::Command;

#[test]
fn examples_check_passes_on_the_committed_tree() {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .arg("examples")
        .output()
        .expect("spawn xtask examples");
    assert!(
        out.status.success(),
        "run `cargo xtask examples --update`\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
