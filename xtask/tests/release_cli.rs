use pulldown_cmark::{Event, HeadingLevel, Parser, Tag};
use std::process::Command;

fn command(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn release_commands_expose_read_only_check_and_notes_interfaces() {
    let check = command(&["release-check", "--help"]);
    assert!(check.status.success());
    assert!(String::from_utf8(check.stdout).unwrap().contains("--base"));
    let notes = command(&["release-notes", "--help"]);
    assert!(notes.status.success());
}

#[test]
fn current_checkout_release_metadata_passes_without_a_base() {
    let output = command(&["release-check"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn release_notes_stdout_is_only_the_classified_entry_body() {
    let output = command(&["release-notes"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let notes = String::from_utf8(output.stdout).unwrap();
    let mut classes = 0;
    for event in Parser::new(&notes) {
        if let Event::Start(Tag::Heading { level, .. }) = event {
            assert_ne!(level, HeadingLevel::H1);
            assert_ne!(level, HeadingLevel::H2);
            classes += usize::from(level == HeadingLevel::H3);
        }
    }
    assert!(classes > 0);
    assert!(!notes.contains("workspace versions and changelog verified"));
}

#[test]
fn symbolic_base_names_are_refused() {
    let output = command(&["release-check", "--base", "HEAD"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("40- or 64-digit"));
}
