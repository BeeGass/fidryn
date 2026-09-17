use std::process::Command;

fn xtask() -> Command {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
}

fn output(args: &[&str]) -> std::process::Output {
    xtask()
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to spawn xtask {args:?}: {err}"))
}

fn stdout_ok(args: &[&str]) -> String {
    let output = output(args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "xtask {args:?} failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    stdout.into_owned()
}

fn stderr_err(args: &[&str]) -> String {
    let output = output(args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "xtask {args:?} succeeded unexpectedly\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    stderr.into_owned()
}

#[test]
fn help_lists_subcommands() {
    let help = stdout_ok(&["--help"]);
    assert!(help.contains("test"), "{help}");
    assert!(help.contains("bench"), "{help}");
    assert!(help.contains("ci"), "{help}");
}

#[test]
fn test_help_describes_selection_flags() {
    let help = stdout_ok(&["test", "--help"]);
    assert!(help.contains("-p"), "{help}");
    assert!(help.contains("--package"), "{help}");
    assert!(help.contains("--unit"), "{help}");
    assert!(help.contains("--integration"), "{help}");
    assert!(help.contains("--all"), "{help}");
}

#[test]
fn test_without_selection_fails() {
    let stderr = stderr_err(&["test"]);
    assert!(
        stderr.contains("no test selection"),
        "expected empty-selection error, got:\n{stderr}"
    );
}

#[test]
fn test_unit_without_package_fails() {
    let stderr = stderr_err(&["test", "--unit"]);
    assert!(
        stderr.contains("no test selection") || stderr.contains("--unit requires"),
        "expected empty-selection or --unit error, got:\n{stderr}"
    );
}

#[test]
fn unknown_package_fails() {
    let stderr = stderr_err(&["test", "-p", "not-a-fidryn-crate"]);
    assert!(
        stderr.contains("not-a-fidryn-crate"),
        "expected unknown-package error, got:\n{stderr}"
    );
}

#[test]
fn no_subcommand_fails() {
    let output = output(&[]);
    assert!(
        !output.status.success(),
        "xtask with no subcommand must not succeed"
    );
}

#[test]
fn bench_help_lists_no_run() {
    let help = stdout_ok(&["bench", "--help"]);
    assert!(help.contains("--no-run"), "{help}");
    assert!(help.contains("--package") || help.contains("-p"), "{help}");
}

#[test]
fn bench_package_without_targets_fails() {
    let stderr = stderr_err(&["bench", "-p", "fidryn-eval"]);
    assert!(
        stderr.contains("no bench targets"),
        "expected empty bench selection to fail, got:\n{stderr}"
    );
}
