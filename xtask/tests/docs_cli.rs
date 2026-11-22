//! Documentation links resolve and complete module samples pass the checker.

use std::process::Command;

#[test]
fn docs_static_check_passes_on_the_committed_tree() {
    let out = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["docs", "--skip-commands"])
        .output()
        .expect("spawn xtask docs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
