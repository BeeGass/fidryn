//! The git that the tests of this crate start. A test makes its repository
//! in a temporary directory, and the git it starts must find that one, or
//! the repository it names on purpose, and nothing that the caller's
//! environment names: run from inside a hook of git, where `GIT_DIR` names
//! the repository the hook runs for, a fixture would otherwise commit and
//! tag there. Every git command of a test is built by [`git`]. The
//! variables are those that the test of the replay tool,
//! `conformance/test_replay_repository.py`, removes for its git commands;
//! the two lists are kept equal.

use std::ffi::OsString;
use std::process::Command;

/// The variables by which git finds a repository, its objects or its
/// configuration: those of the section "The Git Repository" of git(1) that
/// say where these are, and every one that `git rev-parse --local-env-vars`
/// lists, but for `GIT_PREFIX`, which git sets for what it runs and does not
/// read, and the configuration family ([`GIT_CONFIGURATION`]).
pub const GIT_LOCATIONS: [&str; 14] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
    "GIT_IMPLICIT_WORK_TREE",
    "GIT_GRAFT_FILE",
    "GIT_SHALLOW_FILE",
    "GIT_NO_REPLACE_OBJECTS",
    "GIT_REPLACE_REF_BASE",
];

/// The first letters of the name of every variable of git's configuration:
/// those that name a file of it or keep one from being read, and those that
/// carry the settings `git -c` gives to what it runs. Each that the caller's
/// environment holds is removed.
const CONFIGURATION_FAMILY: &str = "GIT_CONFIG";

/// The members of the configuration family that git reads by name, removed
/// whether or not the caller's environment holds them. `GIT_CONFIG_KEY_<n>`
/// and `GIT_CONFIG_VALUE_<n>` are read only as far as `GIT_CONFIG_COUNT`
/// says.
pub const GIT_CONFIGURATION: [&str; 6] = [
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_NOSYSTEM",
];

/// `git`, for a test: a command that runs git without [`GIT_LOCATIONS`] and
/// without any variable of git's configuration, and inherits the rest of the
/// environment of the process. Nothing of the process's own environment is
/// changed.
pub fn git() -> Command {
    without_the_caller(
        Command::new("git"),
        std::env::vars_os().map(|(name, _)| name),
    )
}

/// `command` with [`GIT_LOCATIONS`], [`GIT_CONFIGURATION`] and every name of
/// `environment` that begins with [`CONFIGURATION_FAMILY`] removed from the
/// environment it runs in.
fn without_the_caller(
    mut command: Command,
    environment: impl IntoIterator<Item = OsString>,
) -> Command {
    for name in GIT_LOCATIONS.iter().chain(&GIT_CONFIGURATION) {
        command.env_remove(name);
    }
    for name in environment {
        if name
            .as_encoded_bytes()
            .starts_with(CONFIGURATION_FAMILY.as_bytes())
        {
            command.env_remove(name);
        }
    }
    command
}

#[test]
fn git_of_a_test_carries_no_place_of_the_caller() {
    use std::collections::BTreeSet;
    use std::ffi::OsStr;

    fn removed(command: &Command) -> BTreeSet<OsString> {
        command
            .get_envs()
            .filter(|(_, value)| value.is_none())
            .map(|(name, _)| name.to_owned())
            .collect()
    }

    let command = git();
    assert_eq!(command.get_program(), "git");
    let gone = removed(&command);
    for name in GIT_LOCATIONS.iter().chain(&GIT_CONFIGURATION) {
        assert!(gone.contains(OsStr::new(name)), "{name} is not removed");
    }
    assert!(
        command.get_envs().all(|(_, value)| value.is_none()),
        "a variable is set"
    );

    // Every variable of the configuration family that the caller's
    // environment holds is removed too, and nothing else of it.
    let caller = [
        "GIT_CONFIG_KEY_0",
        "GIT_CONFIG_VALUE_0",
        "GIT_AUTHOR_NAME",
        "GIT_EDITOR",
        "PATH",
        "HOME",
    ]
    .map(OsString::from);
    let gone = removed(&without_the_caller(Command::new("git"), caller));
    for name in ["GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0"] {
        assert!(gone.contains(OsStr::new(name)), "{name} is not removed");
    }
    for name in ["GIT_AUTHOR_NAME", "GIT_EDITOR", "PATH", "HOME"] {
        assert!(!gone.contains(OsStr::new(name)), "{name} is removed");
    }
}
